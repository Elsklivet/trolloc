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
            // for cnt in 0..4096usize {
                let bingus = malloc(&mut ALLOCATOR, core::mem::size_of::<u8>());
                // *bingus = cnt as u8; // first byte of count
                // assert_eq!(cnt as u8, *bingus);

                *bingus = 5u8;
                assert_eq!(5u8, *bingus);

                free(&mut ALLOCATOR, bingus);

                let bongus = malloc(&mut ALLOCATOR, core::mem::size_of::<u8>());

                *bongus = 7u8;
                assert_eq!(7u8, *bongus);

                free(&mut ALLOCATOR, bongus);
            // }
        }
        unsafe { ALLOCATOR.heap_destroy(); }
    }
}