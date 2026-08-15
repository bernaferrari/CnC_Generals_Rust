//! hq-nnuu 2026-08-15: live-test fixtures for C++ Enter.
//!
//! C++ `Object::getTransportSlotCount` is 0 unless Object INI sets
//! `TransportSlotCount` (ActionManager.cpp canEnterObject CHECK_CAPACITY).

use super::super::*;

/// C++ infantry default: `TransportSlotCount = 1`.
pub(super) fn set_infantry_transport_slot(template: &mut ThingTemplate) {
    template.transport_slot_count = Some(1);
}
