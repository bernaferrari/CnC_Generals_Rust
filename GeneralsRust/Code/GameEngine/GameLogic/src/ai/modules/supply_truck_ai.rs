//! Compatibility shim for supply truck AI.
//!
//! The authoritative implementation lives in `supply_system.rs` to match the
//! original C++ SupplyTruckAIUpdate update module. This module re-exports it
//! so existing AI module wiring can reference the canonical implementation.

pub use crate::supply_system::{SupplyTruckAIUpdate, SupplyTruckAIUpdateData};
