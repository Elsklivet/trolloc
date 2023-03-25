// Based partially on Philipp Oppermann's 
// blog post on Rust memory allocators:
// https://os.phil-opp.com/allocator-designs/#implementation-1

#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "alloc")]
extern crate alloc;

const MAX_HEAP_SIZE: usize = 4096;

// use libc::malloc;
use core::{alloc::{GlobalAlloc, Layout}, mem::{align_of, self}, ptr::*, panic};

#[link(name = "msvcrt")]
#[link(name = "libcmt")]
extern "C" {
    pub fn malloc(size: usize) -> *mut u8;
    pub fn free(ptr: *mut u8);
}

type BlockPointer = *mut Block;

struct BlockHeader {
    size: usize,
    prev: BlockPointer,
}

struct FreeNode {
    /// Previous free block
    prev: BlockPointer,
    /// Next free block
    next: BlockPointer,
}

pub struct Block {
    header: BlockHeader,
    /// Pointers for free blocks
    // TODO: Make this overlap payload space
    free_node: FreeNode,
}

struct FreeList {
    free_list_head: BlockPointer,
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
    free_list: FreeList,
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
            next_free: 0 as *mut u8,
            free_list: FreeList {
                free_list_head: 0 as *mut _,
            },
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

    /// Search the free list for a spot that fits
    /// 
    /// Returns a pointer to the block that we are going to allocate as well as its actual start address
    pub fn search_free_list(&mut self, size: usize, alignment: usize) -> Option<(BlockPointer, usize)> {
        todo!()
    }

    /// Initialize the heap
    pub unsafe fn heap_init(&mut self) { 
        // Initialize heap
        self.initialized = true;
        self.next_free = self.heap.as_mut_ptr();
        // self.first_block = malloc(self.heap_size) as BlockPointer;
        // (*(self.first_block)).header = BlockHeader {
        //     size: self.heap_size,
        //     prev: 0 as *mut Block,
        // };
    }

    pub unsafe fn heap_destroy(&mut self) {
        // free(self.first_block as *mut u8);
    }

    /// Add a memory region to the free list
    unsafe fn free_list_add(&mut self, address: usize, size: usize) {
        todo!()
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
        let req_size = layout.size();
        let req_align = layout.align();

        if self.next_free as usize + req_size > self.heap_end() {
            panic!("Out of memory!");
        }

        self.next_free = (self.next_free as usize + req_size) as *mut u8;

        ptr
    }

}

unsafe impl GlobalAlloc for Trollocator {
    /// Allocate a fully zeroed block. 
    /// 
    /// Roughly equivalent to calloc.
    unsafe fn alloc_zeroed(&self, layout: core::alloc::Layout) -> *mut u8 {
        
        let size = layout.size();

        // SAFETY: the safety contract for `alloc` must be upheld by the caller.
        let ptr = self.alloc(layout);
        if !ptr.is_null() {
            // SAFETY: as allocation succeeded, the region from `ptr`
            // of size `size` is guaranteed to be valid for writes.
            core::ptr::write_bytes(
                ptr, 
                0, 
                size
            );
        }
        
        ptr
    }

    /// Reallocate a block given a new size.
    unsafe fn realloc(&self, ptr: *mut u8, layout: core::alloc::Layout, new_size: usize) -> *mut u8 {

        // SAFETY: the caller must ensure that the `new_size` does not overflow.
        // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
        let new_layout = core::alloc::Layout::from_size_align_unchecked(new_size, layout.align());

        // SAFETY: the caller must ensure that `new_layout` is greater than zero.
        let new_ptr = self.alloc(new_layout);

        if !new_ptr.is_null() {

            // SAFETY: the previously allocated block cannot overlap the newly allocated block.
            // The safety contract for `dealloc` must be upheld by the caller.
            core::ptr::copy_nonoverlapping(
                ptr, 
                new_ptr, 
                core::cmp::min(layout.size(), 
                new_size)
            );

            self.dealloc(ptr, layout);
        }

        new_ptr
    }

    /// Allocate a block.
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.next_free
    }

    /// Deallocate a block.
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {

    }
}
