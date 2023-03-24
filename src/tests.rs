#[cfg(test)]
mod tests {

    use crate::*;

    #[global_allocator]
    static mut ALLOCATOR: Trollocator = Trollocator::new();

    #[test]
    fn it_works() {
        unsafe { ALLOCATOR.heap_init(); }
        let s = format!("lol");

        assert_eq!(s, "lol");
        unsafe { ALLOCATOR.heap_destroy(); }
    }
}