use core::alloc::Layout;

use crate::allocator::linked_list::LinkedList;
use crate::allocator::util::*;
use crate::allocator::LocalAlloc;

/// A simple allocator that allocates based on size classes.
///   bin 0 (2^3 bytes)    : handles allocations in (0, 2^3]
///   bin 1 (2^4 bytes)    : handles allocations in (2^3, 2^4]
///   ...
///   bin 29 (2^22 bytes): handles allocations in (2^31, 2^32]
///
///   map_to_bin(size) -> k

pub struct Allocator {
    bins: [LinkedList; 29], // 32 bins, starting at 2^3
    free_start: usize,
    free_end: usize,
}

/// Maps a size to a given bin
fn map_to_bin(size: usize) -> usize {
    let mut s = size;

    let t: [usize; 6] = [
        0xFFFFFFFF00000000,
        0x00000000FFFF0000,
        0x000000000000FF00,
        0x00000000000000F0,
        0x000000000000000C,
        0x0000000000000002
    ];

    let mut y: usize = if (s & (s - 1)) == 0 { 0 } else { 1 };
    let mut j = 32;

    for i in 0..6 {
        let k = if (s & t[i]) == 0 { 0 } else { j };
        y += k;
        s >>= k;
        j >>= 1;
    }

    return y.saturating_sub(3); // Subtract 3, smallest is 0
}

/// Map a bin index to the size of that bin
fn map_to_size(bin: usize) -> usize {
    return 1 << (bin + 3);
}

impl Allocator {
    /// Creates a new bin allocator that will allocate memory from the region
    /// starting at address `start` and ending at address `end`.
    pub fn new(start: usize, end: usize) -> Allocator {
        return Allocator {
            bins: [LinkedList::new(); 29],
            free_start: start,
            free_end: end
        }
    }

    /// Gets a block from giant free memory chunk
    fn get_block_from_free_mem(&mut self, layout: Layout) -> Option<*mut usize> {
        let block_start = align_up(self.free_start, layout.align());
        let desired_size = map_to_size(map_to_bin(layout.size()));

        match block_start.checked_add(desired_size) {
            Some(res) => {
                if res > self.free_end {
                    return None;
                }
                self.free_start = res;

                return Some(block_start as *mut usize);
            },
            None => None
        }
    }

    /// Insert a block back into the linked lists
    fn insert_block(&mut self, block: *mut usize, size: usize) {
        let bin_index = map_to_bin(size);
        let curr_size = map_to_size(bin_index);

        let nodes = self.bins[bin_index];

        // Merge with neighbors, insert larger blocks
        for node in self.bins[bin_index].iter_mut() {
            // If is to the right of block
            if node.value() as usize + curr_size == block as usize {
                let neighbor = node.pop();
                self.insert_block(neighbor, size * 2);

                return;
            } else if block as usize + curr_size == node.value() as usize {
                // Pop neighbor, but throw away
                node.pop();
                self.insert_block(block, size * 2);

                return;
            }
        }

        unsafe { self.bins[bin_index].push(block); }
    }

    /// Recursively reinsert blocks into the linked lists, inserting parts as you go up
    fn reinsert_block(&mut self, block: *mut usize, bin_index: usize, size: usize) {
        if size == 0 {
            return;
        }

        let insert_size = map_to_size(bin_index);

        self.insert_block(block, insert_size);

        let remaining_block = (block as usize + insert_size) as *mut usize;

        self.reinsert_block(remaining_block, bin_index+1, size - insert_size);
    }
}

impl LocalAlloc for Allocator {
    /// Allocates memory. Returns a pointer meeting the size and alignment
    /// properties of `layout.size()` and `layout.align()`.
    ///
    /// If this method returns an `Ok(addr)`, `addr` will be non-null address
    /// pointing to a block of storage suitable for holding an instance of
    /// `layout`. In particular, the block will be at least `layout.size()`
    /// bytes large and will be aligned to `layout.align()`. The returned block
    /// of storage may or may not have its contents initialized or zeroed.
    ///
    /// # Safety
    ///
    /// The _caller_ must ensure that `layout.size() > 0` and that
    /// `layout.align()` is a power of two. Parameters not meeting these
    /// conditions may result in undefined behavior.
    ///
    /// # Errors
    ///
    /// Returning null pointer (`core::ptr::null_mut`)
    /// indicates that either memory is exhausted
    /// or `layout` does not meet this allocator's
    /// size or alignment constraints.
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let starting_bin_index = map_to_bin(layout.size());
        let desired_size = map_to_size(starting_bin_index);

        let mut curr_bin = starting_bin_index;

        while curr_bin < self.bins.len() {
            let curr_size = map_to_size(curr_bin);

            for node in self.bins[curr_bin].iter_mut() {
                // Check alignment, if not aligned, ignore that ish
                if is_aligned(node.value() as usize, layout.align()) {
                    let return_block = node.pop();

                    if curr_bin == starting_bin_index {
                        return return_block as *mut u8;
                    } else {
                        let to_reinsert = (return_block as usize + desired_size) as *mut usize;
                        let size = map_to_size(curr_bin) - desired_size;

                        self.reinsert_block(to_reinsert, starting_bin_index, size);

                        return return_block as *mut u8;
                    }
                }
            }

            curr_bin += 1;
        }

        // Didn't find suitable block in any bin
        let block = self.get_block_from_free_mem(layout);

        match block {
            Some(block) => {
                return block as *mut u8;
            },
            None => {
                core::ptr::null_mut()
            }
        }
    }

    /// Deallocates the memory referenced by `ptr`.
    ///
    /// # Safety
    ///
    /// The _caller_ must ensure the following:
    ///
    ///   * `ptr` must denote a block of memory currently allocated via this
    ///     allocator
    ///   * `layout` must properly represent the original layout used in the
    ///     allocation call that returned `ptr`
    ///
    /// Parameters not meeting these conditions may result in undefined
    /// behavior.
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let bin = map_to_bin(layout.size());
        let size = map_to_size(bin);

        self.insert_block(ptr as *mut usize, size);
    }
}

// FIXME: Implement `Debug` for `Allocator`.
impl core::fmt::Debug for Allocator {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Allocator (free_start={}, free_en{})", self.free_start, self.free_end)
    }
}
