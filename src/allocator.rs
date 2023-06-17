// Based partially on Philipp Oppermann's 
// blog post on Rust memory allocators:
// https://os.phil-opp.com/allocator-designs/#implementation-1

#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "alloc")]
extern crate alloc;

const HEADER_SIZE: usize = core::mem::size_of::<BlockHeader>();
const MIN_BLOCK_SIZE: usize = core::mem::size_of::<Block>();
const MAX_HEAP_SIZE: usize = 4096;
pub(crate) const ALIGNMENT: usize = 8;

// use libc::malloc;
use core::{alloc::{Layout}, mem::{align_of, self}, ptr::*, panic};

#[link(name = "msvcrt")]
#[link(name = "libcmt")]
extern "C" {
    pub fn malloc(size: usize) -> *mut u8;
    pub fn free(ptr: *mut u8);
}

type BlockPointer = *mut Block;

/// Block header
struct BlockHeader {
    /// Leasable size of this block
    size: usize,
    /// Pointer to previous physical block
    prev: BlockPointer,
    /// Whether block is currently free
    free: bool,
}

/// Free list linkage pointers, overlaps payload space.
struct FreeNode {
    /// Previous free block
    prev: BlockPointer,
    /// Next free block
    next: BlockPointer,
}

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

    /// Coalesce around a block
    fn coalesce(block: BlockPointer) {
        todo!()
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
    unsafe fn block_to_payload(block: BlockPointer) -> *mut u8 {
        ((block as usize) + HEADER_SIZE) as *mut u8
    }

    /// Return block address from payload address
    unsafe fn payload_to_block(address: usize) -> BlockPointer {
        (address - HEADER_SIZE) as BlockPointer
    }

    /// Add a memory region to the free list
    unsafe fn free_list_add(&mut self, block_ptr: BlockPointer) {
        // Mark block as free
        (*block_ptr).header.free = true;

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

    /// Align a layout to Block size 
    /// 
    /// Returns a tuple of alignment size and alignment
    fn align(layout: Layout) -> (usize, usize) {
        let lyt = layout
            .align_to(mem::align_of::<Block>())
            .expect("could not align block layout")
            .pad_to_align();
        
        (
            lyt.size().max(mem::size_of::<Block>()),
            lyt.align()
        )
    }

    /// Actual malloc function, because I cannot make global alloc work
    pub unsafe fn malloc(&mut self, layout: core::alloc::Layout) -> *mut u8 {
        let ptr = self.next_free;

        // Align layout to block size
        let actual_layout = Self::align(layout);
        let req_size = actual_layout.0;
        let req_align = actual_layout.1;

        // TODO: Actually allocate

        if let Some(fitting_block) = self.search_free_list(req_size) {
            // TODO: Severe oversimplification
            return Self::block_to_payload(fitting_block);
        } else {
            return core::ptr::null_mut();
        }
    }

    pub unsafe fn free(&mut self, ptr: *mut u8) {
        // First move back to block pointer
        let block = Self::payload_to_block(ptr as usize);

        // Now add to free list
        self.free_list_add(block);
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
