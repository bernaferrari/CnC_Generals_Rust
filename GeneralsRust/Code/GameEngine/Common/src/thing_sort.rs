//! Compatibility module for C++ Common/ThingSort.h
//! Re-exports EditorSortingType from common::thing::thing_template.
//!
//! PARITY_NOTE: The C++ ThingSort.h defines EditorSortingType as an enum
//! used in the world builder to categorize objects (Unit, Building,
//! Infrastructure, Civilian, Invalid).  The Rust port in
//! common::thing::thing_template preserves all variants with the same
//! discriminant semantics.  A `name()` method is available in the C++
//! version for display; the Rust enum uses Debug derive for similar purposes.

pub use crate::common::thing::thing_template::EditorSortingType;
