//! # Trolloc
//! 
//! /ˈtrolɑk/
//! 
//! A dynamic memory allocator that does a little bit of trolling.
//! 
//! The *Trolloc* project is a really simple memory allocator that was designed and created for
//! SIGBOVIK. The real objective was to make a global memory allocator (after learning this was
//! possible in Rust) that could cause "safe" code to experience a segmentation fault in Rust.
//! 
//! The idea was to create a memory allocator that would randomly free blocks whenever a user
//! tries to allocate. This way, the seemingly safe Rust program would expect that dynamically
//! allocated blocks were still valid that were, in fact, already freed. This not only forces
//! random use-after-free bugs, but also that Rust's inserted drop code would double-free blocks.
//! 
//! The top-level functions in this library are mostly deprecated because they use the original
//! reference implementation in [`allocator`](crate::allocator). This implementation does not
//! satisfy the requirements for Rust's [`GlobalAlloc`](core::alloc::GlobalAlloc) trait, which
//! allows an allocator to be directly used by a safe Rust program. For the most up-to-date,
//! correct implementation, refer to [`gjallocator`](crate::gjallocator).

#![no_std]

#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "alloc")]
extern crate alloc;

#[macro_use]
extern crate std;

pub mod allocator;
pub mod gjallocator;
#[cfg(test)]
mod tests;

use core::alloc::Layout;
use allocator::*;

/// Generates a random number using xorshift
/// 
/// Credit: Marsaglia, "Xorshift RNGs", https://www.jstatsoft.org/article/view/v008i14
pub fn xorshift(state: usize) -> usize {
    let mut x = state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

/// Generates a random 64-bit number using wyrand.
///
/// Credit: Branden Brown (https://github.com/zephyrtronium)
///         Wang Yi (https://github.com/wangyi-fudan/wyhash)
pub fn wyrand(x: u64) -> u64 {
    let x = x + 0xa0761d6478bd642f;
    let v = (x as u128) * (x as u128 ^ 0xe7037ed1a0b428db);
    (v ^ v >> 64) as u64
}

#[deprecated]
/// Mallocs a block of the specified size using the given allocator.
pub fn malloc(allocer: &mut Trollocator, size: usize) -> *mut u8 {
    unsafe { allocer.malloc(Layout::from_size_align_unchecked(size, allocator::ALIGNMENT)) }
}

#[deprecated]
/// Reallocates a block of memory to be the specified size.
/// 
/// The pointer argument must be the **exact** same pointer returned by `malloc`.
pub fn realloc(allocer: &mut Trollocator, ptr: *mut u8, size: usize) -> *mut u8 {
    unsafe { allocer.realloc(ptr, Layout::from_size_align_unchecked(size, allocator::ALIGNMENT))} 
}

#[deprecated]
/// Frees a block of memory using the given allocator. 
/// 
/// The pointer argument must be the **exact** same pointer returned by `malloc`.
pub fn free(allocer: &mut Trollocator, ptr: *mut u8) {
    unsafe { allocer.free(ptr); }
}