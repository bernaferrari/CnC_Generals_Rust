use super::super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::object::DamageHitContext;
use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

fn world() -> (GameLogic, [ObjectId; 3]) {
    let mut world = GameLogic::new();
    for (id, team) in [(1, Team::USA), (2, Team::GLA), (3, Team::China)] {
        world.add_player(Player::new(id, team, "SavedHistory", id == 1));
    }
    let mut ids = Vec::new();
    for (name, player, kind) in [
        ("SavedHistoryVictim", 1, KindOf::Infantry),
        ("SavedHistoryUnit", 2, KindOf::Infantry),
        ("SavedHistoryProjectile", 3, KindOf::Projectile),
    ] {
        let mut template = ThingTemplate::new(name);
        template.set_health(1000.0).add_kind_of(kind);
        world.templates.insert(name.into(), template);
        ids.push(
            world
                .create_object_for_player(name, player, glam::Vec3::ZERO)
                .unwrap(),
        );
    }
    (world, ids.try_into().unwrap())
}

fn hit(world: &mut GameLogic, victim: ObjectId, source: ObjectId, frame: u32, kind: DamageType) {
    world.frame = frame;
    let context = DamageHitContext::new(world.host_object(source), None, kind);
    world
        .apply_owned_damage(
            victim,
            1.0,
            Some(source),
            kind,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &context,
        )
        .unwrap();
}

#[test]
fn normal_world_codec_preserves_selected_attacker_and_next_frame_future() {
    let (mut original, [victim, unit, projectile]) = world();
    hit(
        &mut original,
        victim,
        unit,
        u32::MAX,
        DamageType::Unresistable,
    );
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&original).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let mut loaded = GameLogic::new();
    loaded.templates = original.templates.clone();
    builder
        .restore_from_snapshot(&decoded, &mut loaded)
        .unwrap();
    assert_eq!(
        loaded.host_object(victim).unwrap().last_damage_source,
        Some(unit)
    );
    assert_eq!(
        loaded.host_object(victim).unwrap().last_damage_timestamp,
        Some(u32::MAX)
    );
    assert_eq!(
        loaded.host_object(victim).unwrap().previous_health,
        original.host_object(victim).unwrap().previous_health
    );
    assert_eq!(loaded.host_object(victim).unwrap().previous_health, 1000.0);
    for owner in [&mut original, &mut loaded] {
        hit(owner, victim, projectile, 0, DamageType::Falling);
        assert_eq!(
            owner.host_object(victim).unwrap().last_damage_source,
            Some(unit)
        );
        assert_eq!(
            owner.host_object(victim).unwrap().last_damage_timestamp,
            Some(u32::MAX)
        );
        assert_eq!(
            owner.host_object(victim).unwrap().last_damage_info_type,
            Some(DamageType::Unresistable)
        );
        assert!(owner.get_player(1).unwrap().get_attacked_by(2));
        assert!(!owner.get_player(1).unwrap().get_attacked_by(3));
    }
    assert_eq!(
        original.host_object(victim).unwrap().health.current,
        loaded.host_object(victim).unwrap().health.current
    );
    // Current source disappearance is resolved afresh after restoration.
    original.host_objects_mut().remove(&unit);
    loaded.host_objects_mut().remove(&unit);
    for owner in [&mut original, &mut loaded] {
        hit(owner, victim, projectile, 0, DamageType::Falling);
        assert_eq!(
            owner.host_object(victim).unwrap().last_damage_source,
            Some(projectile)
        );
        assert!(owner.get_player(1).unwrap().get_attacked_by(3));
    }
}

#[test]
fn older_v14_and_v15_history_defaults_do_not_retain_receiving_prior_source() {
    for version in [14, 15] {
        let (mut receiving, [victim, unit, _]) = world();
        hit(&mut receiving, victim, unit, 77, DamageType::Falling);
        let current = capture(&receiving);
        let encoded = if version == 14 {
            bincode_legacy::serialize(&ObjectXferPersistPayloadV14 {
                objects: current.objects,
                script_status: current.script_status,
                structures: current.structures,
                damage_info: current.damage_info,
            })
            .unwrap()
        } else {
            bincode_legacy::serialize(&ObjectXferPersistPayloadV15 {
                objects: current.objects,
                script_status: current.script_status,
                structures: current.structures,
                damage_info: current.damage_info,
                transport: current.transport,
            })
            .unwrap()
        };
        let mut bytes = OXOB_MAGIC.to_vec();
        append_u32(&mut bytes, version);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        apply_from_lifecycle_tail(&bytes, &mut receiving).unwrap();
        let object = receiving.host_object(victim).unwrap();
        assert_eq!(object.last_damage_source, None);
        assert_eq!(object.last_damage_timestamp, None);
        assert_eq!(object.previous_health, object.health.current);
        assert_eq!(object.last_damage_info_type, Some(DamageType::Falling));
    }
}

#[test]
fn malformed_saved_history_is_rejected_before_receiving_state_mutation() {
    let (mut receiving, [victim, unit, _]) = world();
    hit(&mut receiving, victim, unit, 83, DamageType::Falling);
    let pose = receiving
        .host_object(victim)
        .unwrap()
        .get_transform_matrix();
    for corrupt in 0..7 {
        let mut payload = capture(&receiving);
        if corrupt == 0 {
            payload.body_history[0].source_id = Some(0);
        } else if corrupt == 1 {
            payload.body_history.push(payload.body_history[0].clone());
        } else if corrupt == 2 {
            payload.body_history[0].previous_health = f32::NAN;
        } else if corrupt == 3 {
            payload.body_history[0].previous_health = f32::INFINITY;
        } else if corrupt == 4 {
            payload.body_history[0].damage_state = 99;
        } else if corrupt == 5 {
            payload.body_history[0].initial_health = f32::NAN;
        } else {
            payload.body_history[0].initial_health = f32::NEG_INFINITY;
        }
        let encoded = bincode_legacy::serialize(&payload).unwrap();
        let mut bytes = OXOB_MAGIC.to_vec();
        append_u32(&mut bytes, OXOB_VERSION);
        append_u32(&mut bytes, encoded.len() as u32);
        bytes.extend_from_slice(&encoded);
        assert!(apply_from_lifecycle_tail(&bytes, &mut receiving).is_err());
        let object = receiving.host_object(victim).unwrap();
        assert_eq!(object.last_damage_source, Some(unit));
        assert_eq!(object.last_damage_timestamp, Some(83));
        assert_eq!(object.get_transform_matrix(), pose);
        assert!(receiving.get_player(1).unwrap().get_attacked_by(2));
    }
}

#[test]
fn normal_encoded_world_preserves_same_current_health_body_enum_and_ai_dead_flag() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    let (mut original, [victim, _, _]) = world();
    {
        let object = original.host_object_mut(victim).unwrap();
        assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
        // CPP SAME_CURRENTHEALTH deliberately does not recalculate state when
        // increasing maximum. Its saved enum can differ from a fresh HP ratio.
        object.set_body_max_health(4000.0);
        object.status.effectively_dead = true;
    }
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&original).unwrap();
    let encoded = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let (mut restored, _) = world();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    let object = restored.host_object(victim).unwrap();
    assert_eq!(
        (object.health.current, object.health.maximum),
        (1000.0, 4000.0)
    );
    assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
    assert!(
        object.status.effectively_dead,
        "saved AI-dead with positive HP must not be cleared"
    );
}

#[test]
fn normal_save_preserves_distinct_initial_health_and_both_kill_receipt_values() {
    let (mut original, [victim, unit, _]) = world();
    original.host_object_mut(victim).unwrap().initial_health = 333.0;
    original
        .host_object_mut(victim)
        .unwrap()
        .kill_experience_awarded = true;
    original
        .host_object_mut(unit)
        .unwrap()
        .kill_experience_awarded = false;
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&original).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let (mut restored, _) = world();
    // Reused owner state must not survive the authoritative saved false.
    restored
        .host_object_mut(unit)
        .unwrap()
        .kill_experience_awarded = true;
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.host_object(victim).unwrap().initial_health, 333.0);
    assert_eq!(restored.host_object(victim).unwrap().health.maximum, 1000.0);
    assert!(
        restored
            .host_object(victim)
            .unwrap()
            .kill_experience_awarded
    );
    assert!(!restored.host_object(unit).unwrap().kill_experience_awarded);
    for world in [&mut original, &mut restored] {
        world
            .host_object_mut(victim)
            .unwrap()
            .set_initial_health_percent(50);
        let object = world.host_object(victim).unwrap();
        assert_eq!(object.health.current, 166.5);
        assert_eq!(object.previous_health, 1000.0);
        assert!(object.kill_experience_awarded);
        assert!(!object.status.on_die_started);
    }
}

#[test]
fn exact_v16_predecessor_bytes_decode_without_inventing_new_body_facts() {
    let (mut receiving, [victim, _, _]) = world();
    let current = capture(&receiving);
    let old_history: Vec<_> = current
        .body_history
        .into_iter()
        .map(
            |entry| body_damage_history_persist::BodyDamageHistoryPersistV16 {
                object_id: entry.object_id,
                source_id: entry.source_id,
                timestamp: entry.timestamp,
                previous_health: entry.previous_health,
                damage_state: entry.damage_state,
            },
        )
        .collect();
    let first = &old_history[0];
    assert_eq!(
        bincode_legacy::serialize(first).unwrap(),
        bincode_legacy::serialize(&(
            first.object_id,
            first.source_id,
            first.timestamp,
            first.previous_health,
            first.damage_state
        ))
        .unwrap()
    );
    let old = ObjectXferPersistPayloadV16 {
        objects: current.objects,
        script_status: current.script_status,
        structures: current.structures,
        damage_info: current.damage_info,
        transport: current.transport,
        body_history: old_history,
    };
    let encoded = bincode_legacy::serialize(&old).unwrap();
    let mut bytes = OXOB_MAGIC.to_vec();
    append_u32(&mut bytes, 16);
    append_u32(&mut bytes, encoded.len() as u32);
    bytes.extend_from_slice(&encoded);
    receiving.host_object_mut(victim).unwrap().initial_health = 777.0;
    receiving
        .host_object_mut(victim)
        .unwrap()
        .kill_experience_awarded = true;
    apply_from_lifecycle_tail(&bytes, &mut receiving).unwrap();
    assert_eq!(
        receiving.host_object(victim).unwrap().initial_health,
        777.0,
        "V16 has no original initialHealth to recover"
    );
    assert!(
        !receiving
            .host_object(victim)
            .unwrap()
            .kill_experience_awarded,
        "V16 has no consumed kill receipt, not inferred from dead state"
    );
}
