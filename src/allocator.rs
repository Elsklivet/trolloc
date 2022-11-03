
struct BlockHeader {
    size: usize,
    prev: Option<Box<Block>>,
}

struct FreeNode {
    /// Previous free block
    prev: Option<Box<Block>>,
    /// Next free block
    next: Option<Box<Block>>,
}

struct Block {
    header: BlockHeader,
    /// Pointers for free blocks
    // TODO: Make this overlap payload space
    free_node: FreeNode,
}

struct FreeList {
    free_list_head: Option<Box<Block>>,
}

/// Core allocator struct
///
/// Represents an instance of the troll allocator. Supports all basic memory allocator functions.
pub struct Trollocator {
    /// Size of the heap in bytes
    heap_size: usize,
    /// First block in the heap
    first_block: Option<Box<Block>>,
    /// Explicitly linked free list
    free_list: FreeList,
}

impl Trollocator {
    /// Instantiate a new Trollocator
    ///
    /// Returns a trollocator instance with a zero heap size, no first block, and an empty free list.
    fn new() -> Trollocator {
        Trollocator {
            heap_size: 0,
            first_block: None,
            free_list: FreeList {
                free_list_head: None,
            },
        }
    }
}
