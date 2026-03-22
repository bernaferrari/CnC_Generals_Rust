//! Re-export of the canonical crate system from `object::crate_system`.
//!
//! The definitive `CrateTemplate`, `CrateSystem`, and `CrateCreationEntry`
//! types live in `crate::object::crate_system`.  This module re-exports them
//! so that `system::crate_system` remains a valid path for existing code.

pub use crate::object::crate_system::{
    get_crate_system, CrateCreationEntry, CrateSystem, CrateTemplate, THE_CRATE_SYSTEM,
};
