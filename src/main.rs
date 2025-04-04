use trolloc::gjallocator::Trollocator;

#[global_allocator]
static mut ALLOCATOR: Trollocator = Trollocator::new();

fn main() {
    let _s = format!("hello world");
    println!("{}", _s);

    let mut vec = vec![0u8];

    for i in 0..=128u8 {
        vec.push(i as u8);
    }

    for i in 0..=127u8 {
        vec.push(i as u8);
    }

    let _s2 = format!("hello world 2");
    println!("{}", _s2);

    core::mem::drop(vec);
}