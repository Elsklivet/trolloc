// ˈjɒloˌkeɪtɜr

const TROLLING_ON: bool = true;
const HEADER_SIZE: usize = core::mem::size_of::<BlockHeader>();
const MIN_BLOCK_SIZE: usize = core::mem::size_of::<Block>();
const MAX_HEAP_SIZE: usize = 0x100000;
pub(crate) const ALIGNMENT: usize = 8;

use core::{alloc::{Layout, GlobalAlloc}, mem::{self}, cell::UnsafeCell};

use crate::wyrand;

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

// Size = 48 bytes, align 8 bytes
#[repr(C)]
pub struct TrollocatorMetadata {
    /// Size of the heap in bytes
    heap_size: usize,
    /// First block in the heap
    heap_start: *mut u8,
    /// Pointer to the next free space
    next_free: *mut u8,
    /// Explicitly linked free list
    free_list_head: BlockPointer,
    /// Number of blocks allocated
    num_alloced_blocks: usize, 
    /// Whether the heap has been initialized yet
    initialized: bool,
}

#[repr(align(8))]
pub struct Trollocator {
    heap: UnsafeCell<[u8; MAX_HEAP_SIZE]>,
}

unsafe impl Sync for Trollocator {}
unsafe impl Send for Trollocator {}

impl Trollocator {
    pub const fn new() -> Self {
        Self {
            heap: UnsafeCell::new([0; MAX_HEAP_SIZE]),
        }
    }

    /// Get metadata by just interpreting the heap as a metadata pointer because who cares
    const fn get_metadata(&self) -> *mut TrollocatorMetadata {
        TrollocatorMetadata::from(self.heap.get().cast::<u8>())
    }

    pub fn get_alloced_blocks(&self) -> usize {
        unsafe { (*self.get_metadata()).num_alloced_blocks }
    }

    /// Get the heap start
    pub fn heap_start(&self) -> usize {
        // First address of the internal heap is the heap start
        unsafe { (*self.get_metadata()).heap_start as usize }
    }

    /// Get the heap end
    pub fn heap_end(&self) -> usize {
        // Last address is first + size
        self.heap_start() + MAX_HEAP_SIZE
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
    unsafe fn free_list_remove(&self, block_ptr: BlockPointer) {
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
    unsafe fn free_list_add(&self, block_ptr: BlockPointer) {
        if (*self.get_metadata()).free_list_head.is_null() {
            // The free list is currently empty, so this is now the only block in the free list.
            (*self.get_metadata()).free_list_head = block_ptr;
        } else {
            // Add at free list head
            let old_head = (*self.get_metadata()).free_list_head;

            // Update free list head
            (*self.get_metadata()).free_list_head = block_ptr;
            (*(*self.get_metadata()).free_list_head).free_node.next = old_head;
            (*(*self.get_metadata()).free_list_head).free_node.prev = core::ptr::null_mut();

            // Update old head's previous
            (*old_head).free_node.prev = (*self.get_metadata()).free_list_head;
        }
    }

    /// Re-interpret an address as a block pointer. Don't misuse this. 🙂
    const unsafe fn as_block_ptr(address: usize) -> BlockPointer {
        address as BlockPointer
    }
    
    /// Search the free list for a spot that fits
    /// 
    /// Returns a pointer to the block that we are going to allocate as well as its actual start address
    unsafe fn search_free_list(&self, size: usize) -> Option<BlockPointer> {
        // Use find first free
        let mut curr = (*self.get_metadata()).free_list_head;

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
    unsafe fn coalesce(&self, mut block: BlockPointer) {
        // Check if the previous block is free. If so, coalesce into it
        let prev_block = (*block).header.prev;

        if !prev_block.is_null() && (*prev_block).header.free {
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

    /// Print the heap to stderr for debugging.
    pub fn print_heap(&self) {
        unsafe {
            let mut curr_block_ptr: BlockPointer = Self::as_block_ptr(self.heap_start());
            let mut curr_block_index: usize = 0;

            while (curr_block_ptr as usize) < (self.heap_end()) {
                eprintln!("--+ {} @ {:p} (size: {}, free: {})", curr_block_index, curr_block_ptr, (*curr_block_ptr).header.size, (*curr_block_ptr).header.free);
                curr_block_ptr = Self::next_physical_block(curr_block_ptr);
                curr_block_index += 1;
            }
        }
    }

    // ---------------------------- TROLLING ----------------------------

    /// Get a block with a given malloc index.
    unsafe fn get_block_by_index(&self, index: usize) -> *mut u8 {
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

        Self::block_to_payload(curr_block_ptr)
    }
}

impl TrollocatorMetadata {
    const fn from(heap: *mut u8) -> *mut Self {
        heap as *mut TrollocatorMetadata
    }
}

unsafe impl GlobalAlloc for Trollocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Use ASLR as a seed for randomness. Thanks Ojas!
        let _stack_marker: u8 = 0b01010101;
        // This is illegal. I do not even care. No one can stop me. Not even the fed. I have no remorse either. I will do it again.
        let metadata = TrollocatorMetadata::from(self.heap.get().cast::<u8>());
        if !(*metadata).initialized {
            // Initialize in alloc because I can??? Lol??? what will you actually do about it? Nothing. Grow up.
            (*metadata).heap_size = MAX_HEAP_SIZE;
            (*metadata).heap_start = self.heap.get().cast::<u8>().wrapping_add(core::mem::size_of::<TrollocatorMetadata>());

            // Make the heap one big block.
            let block = Self::as_block_ptr((*metadata).heap_start as usize);
            (*block).header = BlockHeader { size: MAX_HEAP_SIZE - HEADER_SIZE, prev: core::ptr::null_mut(), free: true };
            (*block).free_node = FreeNode { prev: core::ptr::null_mut(), next: core::ptr::null_mut() };

            (*metadata).next_free = (*metadata).heap_start;
            (*metadata).free_list_head = block;
            (*metadata).num_alloced_blocks = 0;
            (*metadata).initialized = true;
        }

        // Align layout to block size
        let actual_layout = Self::align(layout);
        let req_size = actual_layout.0;


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
            
            (*self.get_metadata()).num_alloced_blocks += 1;

            // Trolling.
            if TROLLING_ON { 
                // Feeding a stack marker address (randomized by ASLR) and block address into wyrand as a seed and using this as the basis of randomness.
                let rand_result: usize = wyrand((&_stack_marker as *const u8 as u64) ^ (block_address as *const u8 as u64)) as usize;
                let randex: usize = rand_result  % (*self.get_metadata()).num_alloced_blocks;
                let rand_bit: usize = (rand_result % (core::mem::size_of::<usize>() * 8)).checked_sub(1).unwrap_or(0);
                if ((randex & (1 << rand_bit)) >> rand_bit) == 1 {
                    let rand_block = self.get_block_by_index(randex);
                    // Get owned. You're owned. Trolled. You're trolled. You're owned and trolled.
                    self.dealloc(rand_block, layout);
                }
            }

            // Return the malloced block
            return block_address;
        } else {
            return core::ptr::null_mut();
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // Notice that I do not care what layout you requested. It is meaningless to me. Like an ant. Like a little menial ant.

        // First move back to block pointer
        let block = Self::payload_to_block(ptr as usize);

        // Mark block as free
        (*block).header.free = true;

        (*self.get_metadata()).num_alloced_blocks = (*self.get_metadata()).num_alloced_blocks.checked_sub(1).unwrap_or(0);

        // Now add to free list
        self.free_list_add(block);

        // Coalesce this block
        self.coalesce(block);
    }

    // I let the functions below just get auto-generated by VS Code.

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        // SAFETY: the safety contract for `alloc` must be upheld by the caller. it will not be.
        let ptr = unsafe { self.alloc(layout) };
        if !ptr.is_null() {
            // SAFETY: no
            unsafe { core::ptr::write_bytes(ptr, 0, size) };
        }
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller must ensure that the `new_size` does not overflow.
        // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        // SAFETY: the caller must ensure that `new_layout` is greater than zero. if they don't, I do not care.
        let new_ptr = unsafe { self.alloc(new_layout) };
        if !new_ptr.is_null() {
            // SAFETY: the previously allocated block cannot overlap the newly allocated block. it might though. your problem now.
            unsafe {
                core::ptr::copy_nonoverlapping(ptr, new_ptr, core::cmp::min(layout.size(), new_size));
                self.dealloc(ptr, layout);
            }
        }
        new_ptr
    }
}