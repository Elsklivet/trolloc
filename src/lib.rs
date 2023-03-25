#![no_std]

#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "alloc")]
extern crate alloc;

#[macro_use]
extern crate std;

pub mod allocator;
#[cfg(test)]
mod tests;

use allocator::*;

/// Generates a random number using xorshift
/// 
/// Credit: Marsaglia, "Xorshift RNGs", https://www.jstatsoft.org/article/view/v008i14
fn xorshift(state: usize) -> usize {
    let mut x = state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}