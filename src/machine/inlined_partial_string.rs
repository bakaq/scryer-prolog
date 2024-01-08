#![allow(dead_code)]

use crate::machine::heap::*;
use crate::types::*;

use std::marker::PhantomData;

/// Bytes inlined in the heap as a 0 terminated contiguous array.
/// 
/// I made a separate struct for this because this could be useful for
/// more things than just partial strings.
#[derive(Copy, Clone, Debug)]
pub struct InlinedBytes<'a> {
    buffer: *const u8,
    _phantom: PhantomData<&'a u8>
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
}

fn allocate_inlined_bytes<'a>(heap: &'a mut Heap, bytes: &[u8]) -> InlinedBytes<'a> {
    // Ideally this should be dealt with before here to avoid using
    // unnecessary memory, but this works with bytes.len() = 0 too.
    #[cfg(not(test))]
    debug_assert!(bytes.len() > 0, "why are you allocating 0 bytes???");

    let num_cells = bytes.len() / 8 + 1;
    let initial_len = heap.len();

    heap.reserve(num_cells);
    let buffer = heap.spare_capacity_mut();

    // Sets the last cell as 0, this ensures the sentinel 0
    buffer[num_cells-1].write(0.into());

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
        allocate_inlined_bytes(heap, &[1,2,3,4]);
        assert_eq!(heap.len(), 2);

        // This should allocate 2 cells, because of the sentinel
        allocate_inlined_bytes(heap, &[1,2,3,4,5,6,7,8]);
        assert_eq!(heap.len(), 4);

        // This should also allocate 2 cells
        allocate_inlined_bytes(heap, &[1,2,3,4,5,6,7,8,9,10]);
        assert_eq!(heap.len(), 6);
    }
}
