use trolloc::gjallocator::Trollocator;

#[global_allocator]
static mut ALLOCATOR: Trollocator = Trollocator::new();

fn main() {
    let _s = format!("hello world");
    println!("{}", _s);
    unsafe { println!("{}", ALLOCATOR.get_alloced_blocks()) };

    let vec = vec![0x55; 1000];

    unsafe { println!("{}", ALLOCATOR.get_alloced_blocks()) };
}