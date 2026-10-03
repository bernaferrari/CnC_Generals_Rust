//! Bounded Chinook-to-SupplyTruck snapshot delegation. Full AI CRC and
//! post-load locomotor/pathfinder reconstruction remain separate contracts.

use super::{ChinookAIUpdate, ChinookAIUpdateData, ChinookFlightStatus};
use crate::common::Coord3D;
use crate::modules::SupplyTruckAIInterface;
use crate::supply_system::SupplyTruckState;
use game_engine::common::system::Snapshotable;
use game_engine::system::xfer_load::XferLoad;
use game_engine::system::xfer_save::XferSave;
use std::io::Cursor;

/// C++ ChinookAIUpdate.cpp:1329–1332 delegates CRC to its SupplyTruck base.
/// This fault probe observes the actual base's version-validation error. It
/// intentionally uses the existing load adapter to expose the accepted maximum;
/// it does not equate a save payload or this incomplete base with C++ CRC bytes.
#[test]
fn chinook_crc_propagates_the_actual_supply_base_validation_error() {
    let chinook = ChinookAIUpdate::new(ChinookAIUpdateData::default(), 77, 1);
    let mut base_probe = XferLoad::new(Cursor::new([2u8]), 1);
    let expected = chinook
        .base
        .crc(&mut base_probe)
        .expect_err("actual SupplyTruck base rejects version two");
    assert!(expected.contains("should be no higher than '1'"));
    let mut derived_probe = XferLoad::new(Cursor::new([2u8]), 1);
    assert_eq!(
        chinook.crc(&mut derived_probe),
        Err(expected),
        "Chinook must execute and propagate its real base callback"
    );
    assert_eq!(base_probe.bytes_read(), 1);
    assert_eq!(derived_probe.bytes_read(), 1);
}

fn saved(runtime: &mut ChinookAIUpdate) -> Vec<u8> {
    let mut bytes = Vec::new();
    runtime
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    bytes
}

/// Preserve the existing uninitialized-SupplyTruck Rust wire envelope while
/// correcting delegation. C++ ChinookAIUpdate.cpp:1344–1360 pins the derived
/// version, base-before-derived order, flight enum, healing ID and original
/// coordinates; complete C++ AI/StateMachine save compatibility is not claimed.
#[test]
fn chinook_base_delegation_keeps_existing_v2_xfer_bytes_and_restored_fields() {
    let mut original = ChinookAIUpdate::new(ChinookAIUpdateData::default(), 77, 1);
    original.base.set_state(SupplyTruckState::Busy);
    original.base.set_preferred_dock(0x1234);
    original.base.set_force_wanting_state(true);
    original.flight_status = ChinookFlightStatus::Landing;
    original.airfield_for_healing = 0x5678;
    original.record_original_position(Coord3D::new(10.0, 20.0, 30.0));

    let mut expected = vec![2, 1]; // derived version two, supply version one
    expected.extend(1u32.to_le_bytes()); // existing fallback state: Busy
    expected.extend(0x1234u32.to_le_bytes()); // preferred dock
    expected.extend(0i32.to_le_bytes()); // carried boxes
    expected.extend([1, 0]); // force wanting, no pending command
    expected.extend(3i32.to_le_bytes()); // CHINOOK_LANDING
    expected.extend(0x5678u32.to_le_bytes()); // healing airfield
    for coordinate in [10.0f32, 20.0, 30.0] {
        expected.extend(coordinate.to_le_bytes());
    }
    assert_eq!(expected.len(), 36);
    let bytes = saved(&mut original);
    assert_eq!(bytes, expected);

    let mut restored = ChinookAIUpdate::new(ChinookAIUpdateData::default(), 77, 1);
    let mut load = XferLoad::new(Cursor::new(bytes.as_slice()), 1);
    restored.xfer(&mut load).unwrap();
    assert_eq!(load.bytes_read(), bytes.len() as u64);
    // The actual supply base post-load callback is currently a no-op. This
    // continuation protects retained state, without inventing missing effects.
    restored.load_post_process().unwrap();
    assert_eq!(restored.base.get_state(), SupplyTruckState::Busy);
    assert_eq!(restored.base.get_preferred_dock(), Some(0x1234));
    assert!(restored.base.is_forced_into_wanting_state());
    assert_eq!(restored.flight_status, ChinookFlightStatus::Landing);
    assert_eq!(restored.airfield_for_healing, 0x5678);
    assert_eq!(
        restored.get_original_position(),
        Coord3D::new(10.0, 20.0, 30.0)
    );
    assert_eq!(saved(&mut restored), expected);
}
