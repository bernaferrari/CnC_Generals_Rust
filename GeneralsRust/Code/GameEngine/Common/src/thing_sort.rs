//! Compatibility module for C++ Common/ThingSort.h
//! Re-exports EditorSortingType from common::thing::thing_template.
//!
//! PARITY_NOTE: The Rust enum preserves the C++ order and discriminants:
//! NONE, STRUCTURE, INFANTRY, VEHICLE, SHRUBBERY, MISC_MAN_MADE,
//! MISC_NATURAL, DEBRIS, SYSTEM, AUDIO, TEST, FOR_REVIEW, ROAD, WAYPOINT.

pub use crate::common::thing::thing_template::EditorSortingType;
