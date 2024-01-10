#![allow(dead_code)]

use crate::machine::heap::*;
//use crate::types::*;

use std::marker::PhantomData;

/// Bytes inlined in the heap as a 0 terminated contiguous array.
///
/// I made a separate struct for this because this could be useful for
/// more things than just partial strings.
#[derive(Copy, Clone, Debug)]
pub struct InlinedBytes<'a> {
    buffer: *const u8,
    _phantom: PhantomData<&'a u8>,
}

impl InlinedBytes<'_> {
    /// Gets the inlined bytes at `addr` in the heap.
    ///
    /// # Safety
    ///
    /// `addr` must point to a valid buffer for InlinedBytes.
    pub unsafe fn from_addr(heap: &Heap, addr: usize) -> Self {
        Self {
            // SAFETY: addr points to a valid buffer.
            buffer: unsafe { heap.as_ptr().add(addr).cast() },
            _phantom: PhantomData,
        }
    }

    /// Gets the inlined bytes at `addr` in the heap, offset by `offset` bytes.
    ///
    /// # Safety
    ///
    /// `addr` must point to a valid buffer for InlinedBytes, and `offset` must be
    /// less than the length of that buffer.
    pub unsafe fn from_addr_offset(heap: &Heap, addr: usize, offset: usize) -> Self {
        Self {
            // SAFETY: addr points to a valid buffer, and offset is within it.
            buffer: unsafe { heap.as_ptr().add(addr).cast::<u8>().add(offset) },
            _phantom: PhantomData,
        }
    }

    /// Return the length of the buffer.
    ///
    /// Note that this always traverses the entire buffer to find the sentinel.
    fn len(&self) -> usize {
        let mut len = 0;
        loop {
            // SAFETY: We know that the buffer is valid, so we will find a 0 and stop
            // this loop before acessing memory outside of the buffer.
            let val = unsafe { *self.buffer.add(len) };
            if val == 0 {
                break len;
            }
            len += 1;
        }
    }

    /// Returns a slice to the bytes.
    ///
    /// Note that this always traverses the entire buffer to find the sentinel.
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: The buffer is valid
        unsafe { std::slice::from_raw_parts(self.buffer, self.len()) }
    }

    /// Interprets the bytes as a `&str` without checking UTF-8.
    ///
    /// Note that this always traverses the entire buffer to find the sentinel.
    ///
    /// # Safety
    ///
    /// The bytes must be valid UTF-8.
    pub unsafe fn as_str_unchecked(&self) -> &str {
        // SAFETY: The bytes are valid UTF-8.
        unsafe { std::str::from_utf8_unchecked(self.as_bytes()) }
    }

    /// Interprets the bytes as a `&str` checking UTF-8.
    ///
    /// Note that this always traverses the entire buffer TWICE! One time to find
    /// the sentinel, and another to check UTF-8.
    pub fn as_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.as_bytes())
    }
}

fn allocate_inlined_bytes<'a>(heap: &'a mut Heap, bytes: &[u8]) -> InlinedBytes<'a> {
    let num_cells = bytes.len() / 8 + 1;
    let initial_len = heap.len();

    heap.reserve(num_cells);
    let buffer = heap.spare_capacity_mut();

    // Sets the last cell as 0, this ensures the sentinel 0
    buffer[num_cells - 1].write(0.into());

    // SAFETY: We have reserved all this memory above
    unsafe {
        // Copies all the the bytes of the string
        std::ptr::copy(bytes.as_ptr(), buffer.as_mut_ptr().cast(), bytes.len());
        heap.set_len(initial_len + num_cells);
    }

    // SAFETY: We just put a valid buffer there
    unsafe { InlinedBytes::from_addr(&heap, initial_len) }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::machine::mock_wam::*;

    #[test]
    fn allocating_inlined_bytes() {
        let mut wam = MockWAM::new();
        let heap = &mut wam.machine_st.heap;

        assert_eq!(heap.len(), 0);

        // This should allocate 1 cell, for the sentinel
        allocate_inlined_bytes(heap, &[]);
        assert_eq!(heap.len(), 1);

        // This should allocate only one cell
        allocate_inlined_bytes(heap, &[1, 2, 3, 4]);
        assert_eq!(heap.len(), 2);

        // This should allocate 2 cells, because of the sentinel
        allocate_inlined_bytes(heap, &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(heap.len(), 4);

        // This should also allocate 2 cells
        allocate_inlined_bytes(heap, &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(heap.len(), 6);
    }

    #[test]
    fn inlined_bytes_as_bytes() {
        let mut wam = MockWAM::new();
        let heap = &mut wam.machine_st.heap;

        let test_bytes_list = [
            &[] as &[u8],
            &[1, 2, 3, 4],
            &[1, 2, 3, 4, 5, 6, 7, 8],
            &[1, 2, 3, 4, 5, 6, 7, 8, 9],
        ];

        for test_bytes in test_bytes_list {
            let bytes = allocate_inlined_bytes(heap, test_bytes);
            assert_eq!(bytes.as_bytes(), test_bytes);
        }
    }

    #[test]
    fn inlined_bytes_as_str() {
        let mut wam = MockWAM::new();
        let heap = &mut wam.machine_st.heap;

        let valid_strs = ["", "1234", "12345678", "123456789", "漢字"];

        for valid_str in valid_strs {
            let str_bytes = valid_str.as_bytes();
            let bytes = allocate_inlined_bytes(heap, str_bytes);
            assert_eq!(bytes.as_str(), Ok(valid_str));
        }

        let invalid_strs: &[&[u8]] = &[
            &[0xc3, 0x28],
            &[0xa0, 0xa1],
            &[0xe2, 0x28, 0xa1],
            &[0xe2, 0x82, 0x28],
            &[0xf0, 0x28, 0x8c, 0xbc],
            &[0xf0, 0x90, 0x28, 0xbc],
            &[0xf0, 0x28, 0x8c, 0x28],
        ];

        for invalid_str in invalid_strs {
            let bytes = allocate_inlined_bytes(heap, invalid_str);
            assert!(bytes.as_str().is_err());
        }
    }

    #[test]
    fn inlined_bytes_as_str_unchecked() {
        let mut wam = MockWAM::new();
        let heap = &mut wam.machine_st.heap;

        let valid_strs = ["", "1234", "12345678", "123456789", "漢字"];

        for valid_str in valid_strs {
            let str_bytes = valid_str.as_bytes();
            let bytes = allocate_inlined_bytes(heap, str_bytes);
            assert_eq!(unsafe { bytes.as_str_unchecked() }, valid_str);
        }
    }
}
