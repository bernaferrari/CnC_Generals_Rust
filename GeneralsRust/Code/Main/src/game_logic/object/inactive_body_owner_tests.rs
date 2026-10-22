//! Actual receiving admission and callbacks; CPP InactiveBody.cpp:23–169.
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::{DamageHitContext, MaxHealthChange};
use crate::game_logic::{GameLogic, ObjectId, Team, ThingTemplate};

fn definition_source(name: &str, inactive: bool) -> String {
    let body = if inactive {
        "Body = InactiveBody BodyTag\nEnd"
    } else {
        "Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd"
    };
    format!("Object {name}\nKindOf = STRUCTURE\n{body}\nEnd")
}

fn template(name: &str, inactive: bool) -> ThingTemplate {
    let body = if inactive {
        "Body = InactiveBody BodyTag\nEnd"
    } else {
        "Body = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd"
    };
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            &format!("Object {name}\nKindOf = STRUCTURE\n{body}\nEnd"),
            "receiving-inactive-body.ini",
        )
        .unwrap();
    GameLogic::build_template_from_object_definition(
        name,
        parser.get_definition(name).unwrap(),
        None,
    )
}

fn admit_definition(world: &mut GameLogic, name: &str, inactive: bool) {
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "receiving-inactive-body.ini".into(),
                definition_source(name, inactive),
            )])
            .unwrap(),
        )
        .unwrap();
}

fn world(name: &str, inactive: bool) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    admit_definition(&mut world, name, inactive);
    let id = world
        .create_object(name, Team::USA, glam::Vec3::ZERO)
        .unwrap();
    (world, id)
}

fn assert_inactive(world: &GameLogic, id: ObjectId) {
    let object = world.host_object(id).unwrap();
    assert!(object.is_inactive_body());
    assert_eq!(object.health.current, 0.0);
    assert_eq!(object.health.maximum, 0.0);
    assert_eq!(object.max_health, 0.0);
    assert_eq!(object.previous_health, 0.0);
    assert_eq!(object.body_initial_health(), 0.0);
    assert_eq!(object.get_health_percentage(), 0.0);
    assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
    assert!(object.status.effectively_dead);
    assert!(!object.is_alive());
}

fn hit(world: &mut GameLogic, id: ObjectId, kind: DamageType, death: HostDeathType) -> bool {
    let result = world
        .apply_owned_damage(
            id,
            30.0,
            None,
            kind,
            death,
            None,
            &DamageHitContext::default(),
        )
        .unwrap();
    assert_eq!(result.hp_lost, 0.0);
    result.destroyed()
}

#[test]
fn authored_body_class_overrides_names_and_foreign_same_id_owners() {
    let name = "OrdinaryBodyOwner";
    let (mut owner, id) = world(name, true);
    let (mut peer, peer_id) = world(name, false);
    assert_eq!(id, peer_id);
    assert_inactive(&owner, id);
    assert!(!peer.host_object(peer_id).unwrap().is_inactive_body());
    assert_eq!(peer.host_object(peer_id).unwrap().health.current, 100.0);
    let clone = owner.host_object(id).unwrap().clone();
    peer.reset();
    drop(peer);
    assert!(clone.is_inactive_body());
    assert_inactive(&owner, id);
    owner.reset();
    assert!(owner.host_objects().is_empty());
    let reset_id = owner
        .create_object(name, Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert_inactive(&owner, reset_id);
    assert!(clone.is_inactive_body());
    // Reset retains the receiving catalog. Different content belongs to a
    // fresh session, while already captured Objects keep their Body class.
    let mut owner = GameLogic::new();
    admit_definition(&mut owner, name, false);
    let replacement = owner
        .create_object(name, Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert!(!owner.host_object(replacement).unwrap().is_inactive_body());
    assert_eq!(
        owner.host_object(replacement).unwrap().health.current,
        100.0
    );

    let (misleading, active) = world("AuthoredActivePoisonField", false);
    assert!(!misleading.host_object(active).unwrap().is_inactive_body());
    assert!(misleading.host_object(active).unwrap().is_alive());
}

#[test]
fn inactive_health_operations_are_noops_and_unresistable_dispatches_die_once() {
    let (mut owner, id) = world("OrdinaryInactiveOperations", true);
    assert_inactive(&owner, id);
    assert!(!owner.host_object(id).unwrap().status.on_die_started);
    for kind in [DamageType::Flame, DamageType::Bullet, DamageType::Healing] {
        assert!(!hit(&mut owner, id, kind, HostDeathType::Normal));
        assert_inactive(&owner, id);
    }
    {
        let object = owner.objects.get_mut(&id).unwrap();
        object.heal(50.0, &mut owner.health_events, 7);
        object.internal_change_health(50.0);
        for mode in [
            MaxHealthChange::PreserveRatio,
            MaxHealthChange::AddToCurrent,
            MaxHealthChange::KeepCurrent,
            MaxHealthChange::FullyHeal,
        ] {
            object.set_body_max_health_with_mode(200.0, mode);
        }
        object.set_initial_health_percent(100);
        object.refresh_model_condition_bits();
        object.restore_body_observation(None);
    }
    assert_inactive(&owner, id);
    assert!(owner.health_events.snapshot_damage().is_empty());
    assert!(owner.health_events.snapshot_heal().is_empty());
    assert_eq!(owner.host_object(id).unwrap().last_damage_timestamp, None);
    assert_eq!(owner.host_object(id).unwrap().last_healing_timestamp, None);
    assert!(hit(
        &mut owner,
        id,
        DamageType::Unresistable,
        HostDeathType::Crushed
    ));
    assert!(owner.host_object(id).unwrap().status.on_die_started);
    assert_eq!(
        owner.host_object(id).unwrap().status.death_type,
        HostDeathType::Crushed
    );
    assert!(owner.host_object(id).unwrap().inactive_body_die_called);
    assert!(!hit(
        &mut owner,
        id,
        DamageType::Unresistable,
        HostDeathType::Normal
    ));
    assert_eq!(
        owner.host_object(id).unwrap().status.death_type,
        HostDeathType::Crushed
    );
    assert_inactive(&owner, id);
}

#[test]
fn encoded_body_kind_restores_with_and_without_receiving_definitions() {
    let name = "SavedOrdinaryInactive";
    let (mut source, id) = world(name, true);
    source.frame = 40;
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    // The Body capsule can supply an unspecified receiving Body class; it
    // cannot replace the independently admitted Die module identity catalog.
    let mut missing_catalog = GameLogic::new();
    missing_catalog.frame = 99;
    let error = builder
        .restore_from_snapshot(&decoded, &mut missing_catalog)
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("receiving Die declarations differ"),
        "{error}"
    );
    assert_eq!(
        missing_catalog.frame, 99,
        "missing declarations reject before mutation"
    );
    assert!(missing_catalog.host_objects().is_empty());
    for loaded_definition in [false, true] {
        let mut receiving = GameLogic::new();
        admit_definition(&mut receiving, name, true);
        let receiving_definition = receiving.templates.get_mut(name).unwrap();
        assert!(receiving_definition.native_die_declarations.is_some());
        if !loaded_definition {
            receiving_definition.body_kind = None;
        }
        assert_eq!(receiving_definition.body_kind.is_some(), loaded_definition);
        builder
            .restore_from_snapshot(&decoded, &mut receiving)
            .unwrap();
        assert_inactive(&receiving, id);
        assert!(!hit(
            &mut receiving,
            id,
            DamageType::Flame,
            HostDeathType::Normal
        ));
        assert!(hit(
            &mut receiving,
            id,
            DamageType::Unresistable,
            HostDeathType::Normal
        ));
    }
    builder
        .restore_from_snapshot(&decoded, &mut source)
        .unwrap();
    assert_inactive(&source, id);
}

#[test]
fn inactive_die_does_not_run_active_body_score_the_kill() {
    use crate::game_logic::{Player, VeterancyLevel};
    for inactive in [false, true] {
        let mut owner = GameLogic::new();
        let mut source_player = Player::new(0, Team::USA, "Source", true);
        source_player.alliance_team = 1;
        let mut victim_player = Player::new(1, Team::China, "Victim", false);
        victim_player.alliance_team = 2;
        owner.add_player(source_player);
        owner.add_player(victim_player);
        owner
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "body-score-owner.ini".into(),
                    format!(
                        "{}\n{}",
                        definition_source("BodyScoreSource", false),
                        definition_source("BodyScoreInactive", inactive)
                    ),
                )])
                .unwrap(),
            )
            .unwrap();
        owner
            .templates
            .get_mut("BodyScoreSource")
            .unwrap()
            .is_trainable = true;
        owner
            .templates
            .get_mut("BodyScoreInactive")
            .unwrap()
            .experience_value = 100.0;
        let source = owner
            .create_object_for_player("BodyScoreSource", 0, glam::Vec3::ZERO)
            .unwrap();
        let victim = owner
            .create_object_for_player("BodyScoreInactive", 1, glam::Vec3::ONE)
            .unwrap();
        let before = owner.host_object(source).unwrap().experience.current;
        assert!(
            owner
                .apply_owned_damage(
                    victim,
                    200.0,
                    Some(source),
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    None,
                    &DamageHitContext::default()
                )
                .unwrap()
                .destroyed()
        );
        assert!(owner.host_object(victim).unwrap().status.on_die_started);
        assert_eq!(
            owner.host_object(victim).unwrap().kill_experience_awarded,
            !inactive
        );
        // The ordinary ActiveBody control proves this source/side is eligible
        // for 100 XP; InactiveBody calls only onDie and skips that phase.
        assert_eq!(
            owner.host_object(source).unwrap().experience.current,
            before + if inactive { 0.0 } else { 100.0 }
        );
        if inactive {
            assert_eq!(
                owner.host_object(source).unwrap().experience.level,
                VeterancyLevel::Rookie
            );
        }
    }
}

#[test]
fn bound_body_kind_does_not_change_positional_template_bytes() {
    let bound = template("BodyWire", true);
    let mut unbound = bound.clone();
    unbound.body_kind = None;
    assert_eq!(
        bincode_legacy::serialize(&bound).unwrap(),
        bincode_legacy::serialize(&unbound).unwrap()
    );
}

#[test]
fn changed_receiving_body_class_rejects_before_mutating_current_world() {
    let name = "SavedKindMismatch";
    let (source, _) = world(name, true);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut receiving, id) = world(name, false);
    receiving.frame = 99;
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut receiving)
            .is_err()
    );
    assert_eq!(receiving.frame, 99);
    assert_eq!(receiving.host_object(id).unwrap().health.current, 100.0);
    assert!(!receiving.host_object(id).unwrap().is_inactive_body());
}

fn exercise_native_field_body(spawner: usize) {
    use crate::game_logic::host_inferno_cannon::INFERNO_FIRE_FIELD_TEMPLATE;
    use crate::game_logic::special_power_strikes::{
        ANTHRAX_TOXIN_OBJECT_NAME, NUKE_RADIATION_OBJECT_NAME,
    };
    let names = [
        INFERNO_FIRE_FIELD_TEMPLATE,
        ANTHRAX_TOXIN_OBJECT_NAME,
        NUKE_RADIATION_OBJECT_NAME,
    ];
    for inactive in [true, false] {
        let mut owner = GameLogic::new();
        owner.frame = 11;
        let source = names
            .iter()
            .map(|name| definition_source(name, inactive))
            .collect::<Vec<_>>()
            .join("\n");
        owner
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "native-field-body.ini".into(),
                    source.clone(),
                )])
                .unwrap(),
            )
            .unwrap();
        let fire = owner
            .spawn_inferno_fire_field_object(3, glam::Vec3::ZERO, false, Team::USA)
            .unwrap();
        owner.special_power_strikes.spawn_toxin_field(
            fire,
            Team::USA,
            glam::Vec3::ZERO,
            owner.frame,
            0,
        );
        owner.spawn_anthrax_toxin_field_objects_for_new_fields();
        let toxin = owner.special_power_strikes.toxin_fields()[0]
            .object_id
            .unwrap();
        owner.special_power_strikes.spawn_radiation_field(
            fire,
            Team::USA,
            glam::Vec3::ZERO,
            owner.frame,
            0,
        );
        owner.spawn_nuke_radiation_field_objects_for_new_fields();
        let radiation = owner.special_power_strikes.radiation_fields()[0]
            .object_id
            .unwrap();
        let ids = [fire, toxin, radiation];
        for id in [ids[spawner]] {
            let object = owner.host_object(id).unwrap();
            if inactive {
                assert_inactive(&owner, id);
            } else {
                assert!(!object.is_inactive_body());
                assert_eq!(object.health.current, 100.0);
                assert_eq!(object.health.maximum, 100.0);
                assert!(object.is_alive());
                assert!(!object.status.effectively_dead);
            }
            assert!(!object.status.on_die_started);
        }
        assert!(owner.host_object(fire).unwrap().inferno_fire_field);
        assert!(owner.host_object(toxin).unwrap().anthrax_toxin_field);
        assert!(owner.host_object(radiation).unwrap().nuke_radiation_field);
        assert_eq!(owner.host_object(toxin).unwrap().producer_id, Some(fire));
        assert_eq!(
            owner.host_object(radiation).unwrap().producer_id,
            Some(fire)
        );
        let builder = crate::save_load::SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&owner).unwrap();
        let bytes = bincode_legacy::serialize(&snapshot).unwrap();
        let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
        let mut restored = GameLogic::new();
        // Re-admit the actual authored Object definitions, including empty
        // authored Die catalogs, independently of the saved runtime records.
        restored
            .admit_receiving_module_content(
                crate::assets::ReceivingObjectContent::from_sources([(
                    "native-field-body.ini".into(),
                    source,
                )])
                .unwrap(),
            )
            .unwrap();
        for name in names {
            assert!(restored.templates[name].native_die_declarations.is_some());
        }
        builder
            .restore_from_snapshot(&decoded, &mut restored)
            .unwrap();
        for id in [ids[spawner]] {
            if inactive {
                assert_inactive(&restored, id);
                assert!(!hit(
                    &mut restored,
                    id,
                    DamageType::Flame,
                    HostDeathType::Normal
                ));
            } else {
                assert!(!restored.host_object(id).unwrap().is_inactive_body());
                assert_eq!(restored.host_object(id).unwrap().health.current, 100.0);
            }
        }
    }
}

#[test]
fn native_fire_field_preserves_receiving_body_and_save() {
    exercise_native_field_body(0);
}
#[test]
fn native_toxin_field_preserves_receiving_body_and_save() {
    exercise_native_field_body(1);
}
#[test]
fn native_radiation_field_preserves_receiving_body_and_save() {
    exercise_native_field_body(2);
}
