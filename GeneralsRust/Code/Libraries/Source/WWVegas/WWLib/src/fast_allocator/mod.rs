//! Fast allocators from WWLib `fastallocator.h`.
//!
//! C++ used raw `malloc` buckets and intrusive free-list pointers. That is
//! unjustified `unsafe` here: callers do not need pointer identity. Storage is
//! `Vec`; handles are indices.

mod fixed;
mod general;
mod stack;

pub use fixed::{AllocHandle, FastFixedAllocator};
pub use general::{FastAllocatorGeneral, FastSTLAllocator, GeneralAlloc, get_global_allocator};
pub use stack::StackAllocator;
