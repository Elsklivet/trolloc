#[cfg(test)]
mod tests {

    use core::alloc::{GlobalAlloc, Layout};

    use crate::*;

    // #[global_allocator]
    // static mut ALLOCATOR: Trollocator = Trollocator::new();

    #[test]
    fn it_works() {
        let mut ALLOCATOR = Trollocator::new();
        unsafe { ALLOCATOR.heap_init(); }
        // let s = format!("lol");
        // assert_eq!(s, "lol");
        unsafe {
            let bingus = ALLOCATOR.malloc(Layout::from_size_align_unchecked(core::mem::size_of::<u8>(), 8));
            *bingus = 5u8;
            let bongus = ALLOCATOR.malloc(Layout::from_size_align_unchecked(core::mem::size_of::<u8>(), 8));
            *bongus = 6u8;
            assert_eq!(5u8, *bingus);
        }
        unsafe { ALLOCATOR.heap_destroy(); }
    }
}