#![no_std]

#[cfg(feature = "std")]
extern crate core;

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod allocator;

use allocator::*;
