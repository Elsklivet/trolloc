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

        unsafe {
            let bingus = malloc(&mut ALLOCATOR, core::mem::size_of::<u8>());
            
            if bingus.is_null() {
                assert!(false);
            }

            *bingus = 5u8;
            assert_eq!(5u8, *bingus);

            free(&mut ALLOCATOR, bingus);

            let bongus = malloc(&mut ALLOCATOR, core::mem::size_of::<u8>());

            *bongus = 7u8;
            assert_eq!(7u8, *bongus);

            free(&mut ALLOCATOR, bongus);
        }
        unsafe { ALLOCATOR.heap_destroy(); }
    }

    #[test]
    fn free_works() {
        let mut ALLOCATOR = Trollocator::new();
        unsafe { ALLOCATOR.heap_init(); }

        unsafe {
            let bingus = malloc(&mut ALLOCATOR, core::mem::size_of::<u8>());
            *bingus = 5u8;
            assert_eq!(5u8, *bingus);
            let bongus = malloc(&mut ALLOCATOR, core::mem::size_of::<u8>());
            *bongus = 7u8;
            assert_eq!(7u8, *bongus);   

            // Free both and ensure we can fill heap up again
            free(&mut ALLOCATOR, bingus);
            free(&mut ALLOCATOR, bongus);

            for cnt in 0..=1638usize {
                let bingus = malloc(&mut ALLOCATOR, core::mem::size_of::<u8>());
                
                if bingus.is_null() {
                    assert!(false);
                }

                *bingus = cnt as u8; // first byte of count
                assert_eq!(cnt as u8, *bingus);
                free(&mut ALLOCATOR, bingus);
            }
        }
        unsafe { ALLOCATOR.heap_destroy(); }
    }

    #[test]
    fn coalesce_works() {
        let mut ALLOCATOR = Trollocator::new();
        unsafe { ALLOCATOR.heap_init(); }

        unsafe {
            let bingus = malloc(&mut ALLOCATOR, core::mem::size_of::<u8>());
            *bingus = 5u8;
            assert_eq!(5u8, *bingus);
            let bongus = malloc(&mut ALLOCATOR, core::mem::size_of::<u8>());
            *bongus = 7u8;
            assert_eq!(7u8, *bongus);   

            // Free both and ensure coalesce made them into a big block
            free(&mut ALLOCATOR, bingus);
            free(&mut ALLOCATOR, bongus);

            // 65512 is heap size minus header size
            let huge = malloc(&mut ALLOCATOR, 65512);
            
            if huge.is_null() {
                assert!(false);
            }

            *huge = 255u8; 
            assert_eq!(255u8, *huge);
            // bingus was the first block
            assert_eq!(255u8, *bingus);          
        }
        unsafe { ALLOCATOR.heap_destroy(); }
    }

    #[test]
    fn realloc_works() {
        let mut ALLOCATOR = Trollocator::new();
        unsafe { ALLOCATOR.heap_init(); }

        unsafe {
            let bingus_ptr = malloc(&mut ALLOCATOR, 4 * core::mem::size_of::<u8>());
            if bingus_ptr.is_null() {
                assert!(false);
            }
            *bingus_ptr = 1u8;
            *(bingus_ptr.offset(1)) = 2u8;
            *(bingus_ptr.offset(2)) = 3u8;
            *(bingus_ptr.offset(3)) = 4u8;

            let bongus_ptr = realloc(&mut ALLOCATOR, bingus_ptr, 6 * core::mem::size_of::<u8>());
            if bongus_ptr.is_null() {
                assert!(false);
            }
            *(bongus_ptr.offset(4)) = 5u8;
            *(bongus_ptr.offset(5)) = 6u8;

            for i in 1..=6u8 {
                assert_eq!(i, *(bongus_ptr.offset(i as isize - 1)));
            }
         }

        unsafe { ALLOCATOR.heap_destroy(); }
    }
}