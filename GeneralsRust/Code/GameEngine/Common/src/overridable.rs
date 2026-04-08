//! Compatibility module for C++ Common/Overridable.h
//! Re-exports Overridable trait from common::system.
//!
//! PARITY_NOTE: The C++ Overridable.h defines a templated class
//! `Overridable<T>` with an override chain (original -> override), clone
//! support, and friend factory access.  The Rust port simplifies this to a
//! `pub trait Overridable` in common::system with `is_override()` and
//! `delete_overrides()` methods.  Concrete types (e.g., ThingTemplate)
//! implement this trait directly rather than using the C++ template pattern.

pub use crate::common::system::Overridable;
