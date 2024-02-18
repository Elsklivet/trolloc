// Based partially on Philipp Oppermann's 
// blog post on Rust memory allocators:
// https://os.phil-opp.com/allocator-designs/#implementation-1

#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "alloc")]
extern crate alloc;

const HEADER_SIZE: usize = core::mem::size_of::<BlockHeader>();
const MIN_BLOCK_SIZE: usize = core::mem::size_of::<Block>();
const MAX_HEAP_SIZE: usize = 0x10000;
pub(crate) const ALIGNMENT: usize = 8;

// use libc::malloc;
use core::{alloc::{Layout}, mem::{align_of, self}, ptr::*, panic};

use crate::xorshift;

#[link(name = "msvcrt")]
#[link(name = "libcmt")]
extern "C" {
    pub fn malloc(size: usize) -> *mut u8;
    pub fn free(ptr: *mut u8);
}

type BlockPointer = *mut Block;

#[repr(C)]
/// Block header
struct BlockHeader {
    /// Leasable size of this block
    size: usize,
    /// Pointer to previous physical block
    prev: BlockPointer,
    /// Whether block is currently free
    free: bool,
}

#[repr(C)]
/// Free list linkage pointers, overlaps payload space.
struct FreeNode {
    /// Previous free block
    prev: BlockPointer,
    /// Next free block
    next: BlockPointer,
}

#[repr(C)]
/// Smallest unit of allocator
pub struct Block {
    /// Header data, including size and pointer to previous block start.
    header: BlockHeader,
    /// Pointers for free blocks
    free_node: FreeNode,
}

/// Core allocator struct
///
/// Represents an instance of the troll allocator. Supports all basic memory allocator functions.
pub struct Trollocator {
    /// Size of the heap in bytes
    heap_size: usize,
    /// First block in the heap
    heap: [u8; MAX_HEAP_SIZE],
    /// Pointer to the next free space
    next_free: *mut u8,
    /// Explicitly linked free list
    free_list_head: BlockPointer,
    /// Number of blocks allocated
    num_alloced_blocks: usize, 
    /// Whether the heap has been initialized yet
    initialized: bool,
}

unsafe impl Sync for Trollocator {}
unsafe impl Send for Trollocator {}

impl Trollocator {
    /// Instantiate a new Trollocator
    ///
    /// Returns a trollocator instance with a zero heap size, no first block, and an empty free list.
    pub const fn new() -> Self {
        Trollocator {
            heap_size: MAX_HEAP_SIZE,
            heap: [0; MAX_HEAP_SIZE],
            next_free: core::ptr::null_mut(),
            free_list_head: core::ptr::null_mut(),
            num_alloced_blocks: 0,
            initialized: false,
        }
    }

    /// Get the heap start
    pub fn heap_start(&self) -> usize {
        // First address of the internal heap is the heap start
        self.heap.as_ptr() as usize
    }

    /// Get the heap end
    pub fn heap_end(&self) -> usize {
        // Last address is first + size
        self.heap_start() + self.heap_size
    }

    /// Initialize the heap
    pub unsafe fn heap_init(&mut self) { 
        // Initialize heap
        self.initialized = true;

        // Make entire heap into one block
        *(Self::as_block_ptr(self.heap.as_mut_ptr() as usize)) = Block {
            header: BlockHeader { size: self.heap_size - HEADER_SIZE, prev: core::ptr::null_mut(), free: true },
            free_node: FreeNode { prev: core::ptr::null_mut(), next: core::ptr::null_mut() }
        };

        // Add block to the free list so it can be returned in a malloc call
        // This is *actually* unsafe, double-mutable-borrowing the heap. 
        self.free_list_add(self.heap.as_ptr() as BlockPointer);
        
        self.next_free = self.heap.as_mut_ptr();
    }

    /// Heap teardown
    pub unsafe fn heap_destroy(&mut self) {
        // free(self.first_block as *mut u8);
    }

    /// Check whether a block fits a request size or not.
    unsafe fn block_fits(block: BlockPointer, size: usize) -> bool {
        ((*block).header.size >= size) && ((size % ALIGNMENT) == 0)
    }

    /// Check whether a block is free or not
    unsafe fn is_free(block: BlockPointer) -> bool {
        (*block).header.free
    }

    /// Return payload pointer from block address
    fn block_to_payload(block: BlockPointer) -> *mut u8 {
        ((block as usize) + HEADER_SIZE) as *mut u8
    }

    /// Return block address from payload address
    fn payload_to_block(address: usize) -> BlockPointer {
        (address - HEADER_SIZE) as BlockPointer
    }

    /// Get the next physical block from a block pointer
    unsafe fn next_physical_block(block_ptr: BlockPointer) -> BlockPointer {
        (block_ptr as usize + HEADER_SIZE + (*block_ptr).header.size) as BlockPointer
    }

    /// Remove a memory region from the free list
    unsafe fn free_list_remove(&mut self, block_ptr: BlockPointer) {
        let free_prev = (*block_ptr).free_node.prev;
        let free_next = (*block_ptr).free_node.next;

        if !free_prev.is_null() {
            (*free_prev).free_node.next = free_next;
        }

        if !free_next.is_null() {
            (*free_next).free_node.next = free_prev;
        }
    }

    /// Add a memory region to the free list
    unsafe fn free_list_add(&mut self, block_ptr: BlockPointer) {
        if self.free_list_head.is_null() {
            // The free list is currently empty, so this is now the only block in the free list.
            self.free_list_head = block_ptr;
        } else {
            // Add at free list head
            let old_head = self.free_list_head;

            // Update free list head
            self.free_list_head = block_ptr;
            (*self.free_list_head).free_node.next = old_head;
            (*self.free_list_head).free_node.prev = core::ptr::null_mut();

            // Update old head's previous
            (*old_head).free_node.prev = self.free_list_head;
        }
    }

    /// Re-interpret an address as a block pointer. Don't misuse this. 🙂
    unsafe fn as_block_ptr(address: usize) -> BlockPointer {
        address as BlockPointer
    }
    
    /// Search the free list for a spot that fits
    /// 
    /// Returns a pointer to the block that we are going to allocate as well as its actual start address
    unsafe fn search_free_list(&mut self, size: usize) -> Option<BlockPointer> {
        // Use find first free
        let mut curr = self.free_list_head;

        // Search all free blocks
        while !curr.is_null() {
            // Check whether this meets size requirements
            if Self::block_fits(curr, size) && Self::is_free(curr) {
                // Block fits!
                return Some(curr);
            } else {
                // Move curr forward
                curr = (*curr).free_node.next;
            }
        }

        // Could not find a fitting block
        None
    }

    /// Coalesce around a block
    unsafe fn coalesce(&mut self, mut block: BlockPointer) {
        // Check if the previous block is free. If so, coalesce into it
        let prev_block = (*block).header.prev;

        if !(*block).header.prev.is_null() && (*(*block).header.prev).header.free {          
            // Make the previous block include current block's size (and header)
            (*prev_block).header.size += HEADER_SIZE + (*block).header.size; 
            // Remove the coalesced block from the free list
            self.free_list_remove(block);
            // Do not add the previous block, it was assumedly already in the free list.
            // Move block pointer to previous block so that next if statement can coalesce both cases
            block = prev_block;
        }

        // Get the next physical block
        let next_block = Self::next_physical_block(block);

        // Bail out if moved past end of heap
        if next_block as usize >= self.heap_end() {
            return;
        }

        // In case we just coalesced the block behind us, make sure we're pointing to the right spot.
        (*next_block).header.prev = block;
        
        // Otherwise, attempt to coalesce this block too
        if (*next_block).header.free {
            // Coalesce the next block into us
            (*block).header.size += HEADER_SIZE + (*next_block).header.size;

            // Remove from free list
            self.free_list_remove(next_block);
        }
    }

    /// Align a layout to Block size 
    /// 
    /// Returns a tuple of alignment size and alignment
    fn align(layout: Layout) -> (usize, usize) {
        let lyt = layout
            .align_to(mem::align_of::<Block>())
            .expect("could not align block layout")
            .pad_to_align();
        
        (
            lyt.size().max(mem::size_of::<FreeNode>()),
            lyt.align()
        )
    }

    // ---------------------------- TROLLING ----------------------------

    /// Free a block with a given malloc index.
    unsafe fn free_random_block(&mut self, index: usize) {
        // Just iterate until a certain malloced block index
        let mut curr_block_ptr: BlockPointer = Self::as_block_ptr(self.heap_start());
        let mut curr_block_index: usize = 0;

        // Go until at correct index
        while curr_block_index < index {
            if !Self::is_free(curr_block_ptr) {
                // Alloced block, index can be incremented
                curr_block_index += 1;
            }

            // Move forward
            curr_block_ptr = Self::next_physical_block(curr_block_ptr);
        }

        // Free it
        self.free(Self::block_to_payload(curr_block_ptr));
    }

    /// Allocate a block of memory with the given layout.
    pub unsafe fn malloc(&mut self, layout: core::alloc::Layout) -> *mut u8 {
        // Align layout to block size
        let actual_layout = Self::align(layout);
        let req_size = actual_layout.0;
        let req_align = actual_layout.1;

        // Actually allocate
        if let Some(fitting_block) = self.search_free_list(req_size) {
            // Split block if possible
            if ((*fitting_block).header.size - req_size) >= MIN_BLOCK_SIZE {
                let original_size = (*fitting_block).header.size;
                // Can split this block: This block is now clamped down to request size,
                // remaining size is used for next block
                (*fitting_block).header.size = req_size;

                // Create a new block at the address after the size of the malloced block 
                // (offset by the header size of the malloced block itself)
                *(((fitting_block as usize) + req_size + HEADER_SIZE) as BlockPointer) = Block {
                    header: BlockHeader { size: original_size - (req_size + HEADER_SIZE), prev: fitting_block, free: true },
                    free_node: FreeNode { prev: core::ptr::null_mut(), next: core::ptr::null_mut() }
                };

                // Add this new block to the free list
                self.free_list_add(((fitting_block as usize) + req_size + HEADER_SIZE) as BlockPointer);
            }

            // Remove this block from the free list
            self.free_list_remove(fitting_block);

            // Mark block allocated
            (*fitting_block).header.free = false;

            let block_address = Self::block_to_payload(fitting_block);
            
            self.num_alloced_blocks += 1;

            // Trolling
            let rand_result: usize = xorshift(block_address as usize);
            let randex: usize = rand_result  % self.num_alloced_blocks;
            let rand_bit: usize = (rand_result % (core::mem::size_of::<usize>() * 8)).checked_sub(1).unwrap_or(0);
            if ((randex & (1 << rand_bit)) >> rand_bit) == 1 {
                self.free_random_block(randex);
            }

            // Return the malloced block
            return block_address;
        } else {
            return core::ptr::null_mut();
        }
    }

    /// Reallocate a block of memory. The pointer argument must be the same pointer that `malloc` returned.
    pub unsafe fn realloc(&mut self, ptr: *mut u8, layout: core::alloc::Layout) -> *mut u8 {
        // The lazy way:
        // 1. Malloc new block.
        // 2. Copy old contents to new block.
        // 3. Free old block.

        // Get block
        let block = Self::payload_to_block(ptr as usize);

        let old_size = (*block).header.size;

        let new_ptr = self.malloc(layout);
        
        if new_ptr.is_null() {
            return core::ptr::null_mut();
        }

        // Copy old stuff over to new stuff
        core::ptr::copy_nonoverlapping::<u8>(ptr, new_ptr, old_size);
        
        // Free old block
        self.free(ptr);

        // Return new block
        new_ptr
    }

    /// Free a block of allocated memory. The argument must be the same pointer that `malloc` returned.
    pub unsafe fn free(&mut self, ptr: *mut u8) {
        // First move back to block pointer
        let block = Self::payload_to_block(ptr as usize);

        // Mark block as free
        (*block).header.free = true;

        self.num_alloced_blocks = self.num_alloced_blocks.checked_sub(1).unwrap_or(0);

        // Now add to free list
        self.free_list_add(block);

        // Coalesce this block
        self.coalesce(block);
    }

}

#[cfg(feature = "std")]
use core::ops::Drop;

#[cfg(feature = "std")]
impl Drop for Trollocator {
    /// Drop should just call teardown
    fn drop(&mut self) {
        unsafe { self.heap_destroy(); }
    }
}
