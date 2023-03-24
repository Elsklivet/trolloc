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
            for _ in 0..=100000 { 
                let bingus = ALLOCATOR.alloc(Layout::from_size_align_unchecked(32, 8));
                *bingus = 5;
                let bongus = ALLOCATOR.alloc(Layout::from_size_align_unchecked(32, 8));
                assert_eq!(5, *bongus);
            }
        }
        unsafe { ALLOCATOR.heap_destroy(); }
    }
}