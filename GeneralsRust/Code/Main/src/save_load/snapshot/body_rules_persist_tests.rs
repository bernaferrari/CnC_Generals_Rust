use super::*;
use crate::game_logic::body_damage_rules::{BodyDamagePolicy, BodyDamageRules};
use crate::game_logic::host_enum_table_residual::HostBodyDamageType as State;
use crate::save_load::snapshot::{SnapshotBuilder, decode_bincode_world_snapshot};
fn world(damaged: f32, really: f32) -> GameLogic {
    let mut world = GameLogic::new();
    let ini = format!(
        "Object FlightSource\nKindOf = STRUCTURE\nBody = ActiveBody BodyTag\nMaxHealth = 5000\nInitialHealth = 5000\nEnd\nEnd\nObject {}\nKindOf = VEHICLE AIRCRAFT\nBody = ActiveBody BodyTag\nMaxHealth = 600\nInitialHealth = 600\nEnd\nEnd\nObject {}\nKindOf = PROJECTILE\nBody = ActiveBody BodyTag\nMaxHealth = 40\nInitialHealth = 40\nEnd\nEnd\n",
        crate::game_logic::special_power_strikes::A10_TRANSPORT,
        crate::game_logic::special_power_strikes::A10_PAYLOAD_TEMPLATE,
    );
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "body-rules-snapshot.ini".into(),
                ini,
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(crate::game_logic::Player::new(
        1,
        crate::game_logic::Team::USA,
        "Owner",
        true,
    ));
    world.admit_body_damage_baseline(BodyDamagePolicy::Receiving(
        BodyDamageRules::from_thresholds(damaged, really).unwrap(),
    ));
    world
}
#[test]
fn encoded_body_rules_restore_before_receiving_objects_and_reset_to_receiver_content() {
    let mut source = world(0.8, 0.4);
    let id = source
        .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
        .unwrap();
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .internal_change_health(-1250.0);
    assert_eq!(source.objects[&id].body_damage_state, State::Damaged);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let wire = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = decode_bincode_world_snapshot(&wire).unwrap();
    let mut receiving = world(0.5, 0.2);
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert_eq!(receiving.body_damage_policy(), source.body_damage_policy());
    receiving
        .objects
        .get_mut(&id)
        .unwrap()
        .internal_change_health(0.0);
    assert_eq!(receiving.objects[&id].body_damage_state, State::Damaged);
    assert_eq!(
        receiving.body_damage_baseline(),
        BodyDamagePolicy::Receiving(BodyDamageRules::from_thresholds(0.5, 0.2).unwrap())
    );
    receiving.reset();
    assert_eq!(
        receiving.body_damage_policy(),
        receiving.body_damage_baseline()
    );
}
#[test]
fn malformed_body_frame_fails_full_restore_before_old_actor_frame_or_policy_changes() {
    let mut source = world(0.8, 0.4);
    source
        .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
        .unwrap();
    let builder = SnapshotBuilder::new();
    let valid = builder.create_world_snapshot(&source).unwrap();
    let mut receiving = world(0.5, 0.2);
    let old = receiving
        .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
        .unwrap();
    receiving.set_current_frame(77);
    let token = receiving.objects[&old].visual_draw_identity_token.clone();
    let policy = receiving.body_damage_policy();
    let (start, end) = frame(&valid.lifecycle_tail).unwrap();
    for mode in 0..4 {
        let mut bad = builder.create_world_snapshot(&source).unwrap();
        match mode {
            0 => bad.lifecycle_tail[start + 1..start + 5].copy_from_slice(&f32::NAN.to_le_bytes()),
            1 => bad.lifecycle_tail[start] = 9,
            2 => {
                bad.lifecycle_tail.drain(start..end);
            }
            _ => {
                let duplicate = bad.lifecycle_tail[start..end].to_vec();
                bad.lifecycle_tail.splice(end..end, duplicate);
            }
        }
        assert!(builder.restore_from_snapshot(&bad, &mut receiving).is_err());
        assert_eq!(receiving.get_current_frame(), 77);
        assert_eq!(receiving.body_damage_policy(), policy);
        assert!(std::sync::Arc::ptr_eq(
            &token,
            &receiving.objects[&old].visual_draw_identity_token
        ));
        assert_eq!(receiving.objects[&old].health.current, 5000.0);
    }
}
#[test]
fn v30_without_body_capsule_uses_receiving_content_explicitly_and_a10v2_stays_readable() {
    let mut source = world(0.8, 0.4);
    let source_id = source
        .create_object_for_player("FlightSource", 1, glam::Vec3::ZERO)
        .unwrap();
    source
        .queue_special_power_strike(
            &crate::command_system::SpecialPowerType::Airstrike,
            source_id,
            glam::Vec3::new(400.0, 0.0, 500.0),
        )
        .unwrap();
    assert!(
        !source.a10_strike_flight_reg.deliveries.is_empty(),
        "actual queued A10 delivery, not empty registry compatibility"
    );
    assert!(source.a10_strike_flight_reg.transports_spawned > 0);
    let builder = SnapshotBuilder::new();
    let mut old = builder.create_world_snapshot(&source).unwrap();
    let (start, end) = frame(&old.lifecycle_tail).unwrap();
    old.lifecycle_tail.drain(start..end);
    old.version = 30;
    let mut receiving = world(0.5, 0.2);
    let expected = receiving.body_damage_policy();
    let bytes = bincode_legacy::serialize(&old).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    assert_eq!(receiving.body_damage_policy(), expected);
    let original = &source.a10_strike_flight_reg;
    let restored = &receiving.a10_strike_flight_reg;
    assert_eq!(restored.transports_spawned, original.transports_spawned);
    assert_eq!(restored.missiles_scheduled, original.missiles_scheduled);
    assert_eq!(restored.next_delivery_id, original.next_delivery_id);
    assert_eq!(restored.transport_bindings, original.transport_bindings);
    assert_eq!(restored.deliveries.len(), original.deliveries.len());
    for (id, row) in &original.deliveries {
        let got = restored
            .deliveries
            .get(id)
            .expect("saved delivery survives encoded v30/A10v2 import");
        assert_eq!(got.source_object, row.source_object);
        assert_eq!(got.strike_id, row.strike_id);
        assert_eq!(got.active_transports, row.active_transports);
        assert_eq!(got.payload_attempts_consumed, row.payload_attempts_consumed);
        assert_eq!(got.launch_complete, row.launch_complete);
        assert_eq!(got.completed, row.completed);
    }
}
#[test]
fn threshold_payload_magic_bits_do_not_become_frame_headers() {
    let scalar = f32::from_le_bytes(*b"BDRF");
    assert!(scalar.is_finite());
    let source = world(scalar, 0.4);
    let mut bytes = Vec::new();
    append(&mut bytes, &source);
    assert_eq!(
        prepare(&bytes, 31).unwrap(),
        Some(source.body_damage_policy())
    );
    assert_eq!(suffix_start(&bytes).unwrap(), 0);
}
