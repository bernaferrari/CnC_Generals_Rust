//! Whole saved Object/Body/SlowDeath records, actual receiving modules and heap.
use super::*;
use crate::modules::ai_native_world_tests::{lethal_damage, ordinary_damage};

fn death_catalog() -> Catalog {
    let mut catalog = Catalog::new();
    let source = ordinary_damage::damage_actor(false);
    let target = lethal_damage::slow_actor();
    catalog.things = HashMap::from([
        (source.get_name().to_string(), source),
        (target.get_name().to_string(), target),
    ]);
    catalog.weapons.clear();
    catalog.locomotors.clear();
    catalog.weapon_sets = Arc::new(
        ObjectWeaponSetCatalog::from_definitions(&catalog.things, &catalog.weapons).unwrap(),
    );
    catalog.module_factory = game_engine::common::thing::module_factory::ModuleFactory::new();
    crate::contain_module_overrides::register_native_module_overrides(&mut catalog.module_factory)
        .unwrap();
    for template in catalog.things.values() {
        for entry in template.get_behavior_module_info() {
            catalog.module_factory.add_module_internal(
                None,
                None,
                game_engine::common::thing::module::ModuleType::Behavior,
                &entry.name,
                entry.interface_flags(),
            );
        }
    }
    catalog
}
fn fresh_death(catalog: &Catalog, records: Option<&[ObjectSnapshotRecord]>) -> NativeMatch {
    let mut game = fresh_with_templates(
        catalog,
        records,
        "OwnedDamagePeer",
        "NativeSlowVictim",
        false,
        105,
    )
    .unwrap();
    // Controller0 is remote in this fixture; no local-loss presentation branch.
    game.players.set_local_player_index(1);
    if records.is_none() {
        let target = game.target.clone();
        game.operate(17, |world| {
            world.admit_prepared_slow_death(&target).unwrap()
        });
    }
    game
}
fn history(game: &NativeMatch) -> (u32, String) {
    let body = game.target.read().unwrap().get_body_module().unwrap();
    let body = body.lock().unwrap();
    let hit = body.get_last_damage_info().unwrap();
    (
        hit.input.source_id,
        hit.input
            .source_template
            .as_ref()
            .unwrap()
            .get_name()
            .to_string(),
    )
}
#[test]
fn full_dead_slow_object_restore_uses_receiving_history_and_original_final_wake() {
    let sending = death_catalog();
    let receiving = death_catalog();
    let mut original = fresh_death(&sending, None);
    let peer = fresh_death(&death_catalog(), None);
    let target = original.target.clone();
    original.operate(100, |world| {
        world
            .attempt_damage_owned(&target, &mut lethal_damage::fatal(1))
            .unwrap()
    });
    assert!(target.read().unwrap().is_effectively_dead());
    assert!(!target.read().unwrap().is_destroyed());
    let records = capture_at(&mut original, &sending, 105);
    let peer_guard = peer.target.write().unwrap();
    let mut restored = fresh_death(&receiving, Some(&records));
    assert_eq!(restored.target.read().unwrap().get_health(), 0.0);
    assert!(restored.target.read().unwrap().is_effectively_dead());
    assert!(!restored.target.read().unwrap().is_destroyed());
    assert_eq!(history(&original), history(&restored));
    {
        let body = restored.target.read().unwrap().get_body_module().unwrap();
        let body = body.lock().unwrap();
        let info = body.get_last_damage_info().unwrap();
        let template = info.input.source_template.as_ref().unwrap();
        assert!(
            Arc::ptr_eq(template, receiving.things.get("OwnedDamagePeer").unwrap()),
            "damage history must bind the receiving definition, not a process factory or sender Arc"
        );
    }
    assert_eq!(
        original.operate(119, |world| world.tick_owned_updates(119).unwrap()),
        1
    );
    assert_eq!(
        restored.operate(119, |world| world.tick_owned_updates(119).unwrap()),
        1
    );
    assert!(!original.target.read().unwrap().is_destroyed());
    assert!(!restored.target.read().unwrap().is_destroyed());
    assert_eq!(
        original.operate(120, |world| world.tick_owned_updates(120).unwrap()),
        1
    );
    assert_eq!(
        restored.operate(120, |world| world.tick_owned_updates(120).unwrap()),
        1
    );
    assert!(original.target.read().unwrap().is_destroyed());
    assert!(restored.target.read().unwrap().is_destroyed());
    drop(peer_guard);
    assert_eq!(peer.target.read().unwrap().get_health(), 100.0);
    assert!(!peer.target.read().unwrap().is_effectively_dead());
}

fn module_payload(game: &NativeMatch, name: &str) -> Vec<u8> {
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;
    let entry = game
        .target
        .read()
        .unwrap()
        .module_by_name(&name.into())
        .unwrap();
    let mut bytes = Vec::new();
    entry.with_module(|module| {
        module
            .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
            .unwrap()
    });
    bytes
}
fn payload_offset(record: &ObjectSnapshotRecord, payload: &[u8]) -> usize {
    let positions: Vec<_> = record
        .bytes
        .windows(payload.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == payload).then_some(index))
        .collect();
    assert_eq!(
        positions.len(),
        1,
        "exact installed module payload must be unique in the original Object block"
    );
    positions[0]
}
fn rejected_receiver(catalog: &Catalog, records: &[ObjectSnapshotRecord], expected: &str) {
    let error = fresh_with_templates(
        catalog,
        Some(records),
        "OwnedDamagePeer",
        "NativeSlowVictim",
        false,
        105,
    )
    .err()
    .expect("unsupported decoded state must reject the detached receiver");
    assert!(
        error.contains(expected),
        "unexpected receiving error: {error}"
    );
}
#[test]
fn corrupt_full_slow_record_rejects_airborne_flags_and_invalid_scale_before_schedule() {
    let sending = death_catalog();
    let receiving = death_catalog();
    let mut original = fresh_death(&sending, None);
    let peer = fresh_death(&death_catalog(), None);
    let target = original.target.clone();
    original.operate(100, |world| {
        world
            .attempt_damage_owned(&target, &mut lethal_damage::fatal(1))
            .unwrap()
    });
    let payload = module_payload(&original, "SlowDeathBehavior");
    // CPP v1 tail: acceleratedTimeScale (Real), then flags (UnsignedInt).
    let tail = payload.len();
    let peer_guard = peer.target.write().unwrap();
    for (offset, bytes, message) in [
        (
            tail - 4,
            (1u32 | (1 << 2)).to_le_bytes(),
            "saved SlowDeath flags",
        ),
        (
            tail - 4,
            (1u32 | (1 << 3)).to_le_bytes(),
            "saved SlowDeath flags",
        ),
        (
            tail - 4,
            (1u32 | (1 << 31)).to_le_bytes(),
            "saved SlowDeath flags",
        ),
        (tail - 8, f32::NAN.to_le_bytes(), "SlowDeath time scale"),
        (tail - 8, (-1.0f32).to_le_bytes(), "SlowDeath time scale"),
        (tail - 8, 0.0f32.to_le_bytes(), "SlowDeath time scale"),
    ] {
        let mut records = capture_at(&mut original, &sending, 105);
        let base = payload_offset(&records[0], &payload);
        records[0].bytes[base + offset..base + offset + 4].copy_from_slice(&bytes);
        rejected_receiver(&receiving, &records, message);
    }
    assert_eq!(module_payload(&original, "SlowDeathBehavior"), payload);
    drop(peer_guard);
    assert_eq!(peer.target.read().unwrap().get_health(), 100.0);
    assert!(!peer.target.read().unwrap().is_effectively_dead());
    assert_eq!(
        original.operate(120, |world| world.tick_owned_updates(120).unwrap()),
        1
    );
    assert!(original.target.read().unwrap().is_destroyed());
}
#[test]
fn full_body_history_missing_receiving_template_rejects_without_ambient_fallback() {
    let sending = death_catalog();
    let receiving = death_catalog();
    let mut original = fresh_death(&sending, None);
    let peer = fresh_death(&death_catalog(), None);
    let target = original.target.clone();
    original.operate(100, |world| {
        world
            .attempt_damage_owned(&target, &mut lethal_damage::fatal(1))
            .unwrap()
    });
    let payload = module_payload(&original, "ActiveBody");
    let source_name = b"OwnedDamagePeer";
    let absent_name = b"AbsentHistoryXX";
    assert_eq!(source_name.len(), absent_name.len());
    let name_offset = payload
        .windows(source_name.len())
        .position(|bytes| bytes == source_name)
        .unwrap();
    let mut records = capture_at(&mut original, &sending, 105);
    let base = payload_offset(&records[0], &payload);
    records[0].bytes[base + name_offset..base + name_offset + source_name.len()]
        .copy_from_slice(absent_name);
    let peer_guard = peer.target.write().unwrap();
    rejected_receiver(&receiving, &records, "AbsentHistoryXX");
    assert_eq!(history(&original), (1, "OwnedDamagePeer".into()));
    drop(peer_guard);
    assert_eq!(peer.target.read().unwrap().get_health(), 100.0);
    assert!(!peer.target.read().unwrap().is_effectively_dead());
    assert_eq!(
        original.operate(120, |world| world.tick_owned_updates(120).unwrap()),
        1
    );
    assert!(original.target.read().unwrap().is_destroyed());
}
