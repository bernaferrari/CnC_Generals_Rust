//! Actual encoded native owner continuation, independent of template names.
use super::*;
use crate::game_logic::native_lifecycle::NativeLifecycleState;
use crate::game_logic::{Team, ThingTemplate};
use glam::Vec3;

fn template(name: &str, delay: u32) -> ThingTemplate {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&format!(
        "Object {name}\nKindOf = IMMOBILE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = LifetimeUpdate LifeA\nMinLifetime = {delay}\nMaxLifetime = {delay}\nEnd\nBehavior = DeletionUpdate DeleteB\nMinLifetime = 7000\nMaxLifetime = 7000\nEnd\nBehavior = HeightDieUpdate HeightC\nTargetHeight = 4\nOnlyWhenMovingDown = Yes\nInitialDelay = 1000\nEnd\nEnd"
    ), "native-lifecycle-save.ini").unwrap();
    GameLogic::build_template_from_object_definition(
        name,
        parser.get_definition(name).unwrap(),
        None,
    )
}

#[test]
fn encoded_native_modules_restore_fresh_and_in_place_without_constructor_draws() {
    let name = "SavedLifecycleOrdinary";
    let mut source = GameLogic::new();
    source.frame = 11;
    source.native_hulk_max_lifetime_override = 77;
    source.templates.insert(name.into(), template(name, 5000));
    let id = source
        .create_object(name, Team::USA, Vec3::new(3.0, 50.0, 7.0))
        .unwrap();
    let runtime = source
        .objects
        .get_mut(&id)
        .unwrap()
        .native_lifecycle
        .as_mut()
        .unwrap();
    runtime.states[2] = NativeLifecycleState::HeightDie {
        earliest_frame: 41,
        last_position: Vec3::new(3.0, 51.0, 7.0),
        has_died: false,
        particles_destroyed: true,
    };
    let expected = runtime.clone();
    source.frame = 40;
    let builder = super::super::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    for definitions_loaded in [false, true] {
        let mut receiving = GameLogic::new();
        receiving.native_hulk_max_lifetime_override = 999;
        if definitions_loaded {
            receiving
                .templates
                .insert(name.into(), template(name, 5000));
        }
        builder
            .restore_from_snapshot(&decoded, &mut receiving)
            .unwrap();
        assert_eq!(
            receiving.objects[&id].native_lifecycle.as_ref(),
            Some(&expected)
        );
        assert_eq!(
            receiving.objects[&id]
                .thing()
                .template
                .native_lifecycle_definitions
                .as_deref(),
            Some(expected.definitions.as_ref())
        );
        assert_eq!(receiving.native_hulk_max_lifetime_override, 77);
    }
    source.native_hulk_max_lifetime_override = 888;
    source
        .objects
        .get_mut(&id)
        .unwrap()
        .native_lifecycle
        .as_mut()
        .unwrap()
        .states[0] = NativeLifecycleState::Lifetime {
        die_frame: 9999,
        active: false,
    };
    builder
        .restore_from_snapshot(&decoded, &mut source)
        .unwrap();
    assert_eq!(
        source.objects[&id].native_lifecycle.as_ref(),
        Some(&expected)
    );
    assert_eq!(source.native_hulk_max_lifetime_override, 77);
}

#[test]
fn receiving_definition_mismatch_keeps_world_and_override_unchanged() {
    let name = "SavedLifecycleMismatch";
    let mut source = GameLogic::new();
    source.templates.insert(name.into(), template(name, 5000));
    source.create_object(name, Team::USA, Vec3::ZERO).unwrap();
    source.native_hulk_max_lifetime_override = 77;
    let builder = super::super::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let mut receiving = GameLogic::new();
    receiving
        .templates
        .insert(name.into(), template(name, 6000));
    receiving.frame = 99;
    receiving.native_hulk_max_lifetime_override = 123;
    let id = receiving
        .create_object(name, Team::China, Vec3::new(9.0, 8.0, 7.0))
        .unwrap();
    let expected = receiving.objects[&id].native_lifecycle.clone();
    assert!(
        builder
            .restore_from_snapshot(&saved, &mut receiving)
            .is_err()
    );
    assert_eq!(receiving.frame, 99);
    assert_eq!(receiving.native_hulk_max_lifetime_override, 123);
    assert_eq!(receiving.objects[&id].native_lifecycle, expected);
    assert_eq!(
        receiving.objects[&id].get_position(),
        Vec3::new(9.0, 8.0, 7.0)
    );
}

#[test]
fn bound_empty_deceptive_name_stays_empty_after_fresh_encoded_restore() {
    let name = "PoisonFieldSmall";
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&format!("Object {name}\nKindOf = IMMOBILE\nBody = ActiveBody BodyTag\nMaxHealth = 100\nEnd\nEnd"), "native-empty-save.ini").unwrap();
    let mut source = GameLogic::new();
    source.templates.insert(
        name.into(),
        GameLogic::build_template_from_object_definition(
            name,
            parser.get_definition(name).unwrap(),
            None,
        ),
    );
    let id = source.create_object(name, Team::USA, Vec3::ZERO).unwrap();
    let builder = super::super::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut receiving = GameLogic::new();
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    let object = &receiving.objects[&id];
    let runtime = object.native_lifecycle.as_ref().unwrap();
    assert!(runtime.definitions.is_empty());
    assert!(runtime.states.is_empty());
    assert!(object.lifetime_update.is_none());
    assert!(object.height_die.is_none());
}

fn single_lifetime_template(name: &str) -> ThingTemplate {
    let mut parser = crate::assets::IniParser::new();
    parser.parse_ini_content(&format!("Object {name}\nBody = ActiveBody BodyTag\nMaxHealth = 100\nEnd\nBehavior = LifetimeUpdate SavedLife\nMinLifetime = 5000\nMaxLifetime = 5000\nEnd\nEnd"), "legacy-lifecycle-save.ini").unwrap();
    GameLogic::build_template_from_object_definition(
        name,
        parser.get_definition(name).unwrap(),
        None,
    )
}

#[test]
fn old_singleton_load_resave_and_reload_keeps_one_legacy_adapter() {
    let name = "OldLifetimeOwner";
    let mut source = GameLogic::new();
    source
        .templates
        .insert(name.into(), ThingTemplate::new(name));
    let id = source.create_object(name, Team::USA, Vec3::ZERO).unwrap();
    source.objects.get_mut(&id).unwrap().lifetime_update = Some(
        crate::game_logic::host_lifetime_update::HostLifetimeUpdateData {
            expire_at_frame: 161,
            active: true,
        },
    );
    let builder = super::super::SnapshotBuilder::new();
    let mut saved = builder.create_world_snapshot(&source).unwrap();
    // Remove only the structurally framed NLFC segment to model a pre-native
    // save, retaining CPRJ, BDKD and every subsequent companion verbatim.
    let mut offset =
        super::super::combat_projectile_persist::prefix_len(&saved.lifecycle_tail).unwrap();
    for tag in [b"CPRJ", b"BDKD"] {
        assert_eq!(&saved.lifecycle_tail[offset..offset + 4], tag);
        let length = u32::from_le_bytes(
            saved.lifecycle_tail[offset + 8..offset + 12]
                .try_into()
                .unwrap(),
        ) as usize;
        offset += 12 + length;
    }
    assert_eq!(&saved.lifecycle_tail[offset..offset + 4], b"NLFC");
    let length = u32::from_le_bytes(
        saved.lifecycle_tail[offset + 8..offset + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    saved.lifecycle_tail.drain(offset..offset + 12 + length);
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let old = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut receiving = GameLogic::new();
    receiving
        .templates
        .insert(name.into(), single_lifetime_template(name));
    builder.restore_from_snapshot(&old, &mut receiving).unwrap();
    assert!(receiving.objects[&id].native_lifecycle.is_none());
    assert_eq!(
        receiving.objects[&id]
            .lifetime_update
            .as_ref()
            .unwrap()
            .expire_at_frame,
        161
    );
    let resaved = builder.create_world_snapshot(&receiving).unwrap();
    let bytes = bincode_legacy::serialize(&resaved).unwrap();
    let resaved = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    for definitions_loaded in [false, true] {
        let mut final_world = GameLogic::new();
        if definitions_loaded {
            final_world
                .templates
                .insert(name.into(), single_lifetime_template(name));
        }
        builder
            .restore_from_snapshot(&resaved, &mut final_world)
            .unwrap();
        assert!(final_world.objects[&id].native_lifecycle.is_none());
        assert_eq!(
            final_world.objects[&id]
                .lifetime_update
                .as_ref()
                .unwrap()
                .expire_at_frame,
            161
        );
        assert!(
            final_world.objects[&id]
                .lifetime_update
                .as_ref()
                .unwrap()
                .active
        );
    }
    // Missing, mismatched and multiple receiving modules are rejected while
    // the direct receiver still describes its pre-load world.
    for (receiving_template, remove_runtime) in [
        (single_lifetime_template(name), true),
        (template(name, 5000), false),
        (
            {
                let mut t = single_lifetime_template(name);
                t.native_lifecycle_definitions.as_mut().unwrap()[0].kind =
                    crate::game_logic::native_lifecycle_definition::NativeLifecycleKind::HeightDie(
                        Default::default(),
                    );
                t
            },
            false,
        ),
    ] {
        let mut candidate = GameLogic::new();
        candidate.templates.insert(name.into(), receiving_template);
        candidate.frame = 99;
        candidate.native_hulk_max_lifetime_override = 123;
        let owner = candidate
            .create_object(name, Team::China, Vec3::new(9.0, 8.0, 7.0))
            .unwrap();
        let before = candidate.objects[&owner].native_lifecycle.clone();
        let mut malformed: super::super::WorldSnapshot =
            bincode_legacy::deserialize(&bincode_legacy::serialize(&old).unwrap()).unwrap();
        if remove_runtime {
            let mut tail = super::super::lifecycle_tail::capture_lifecycle_tail(&source).unwrap();
            tail.envelopes[0]
                .module_states
                .retain(|module| module.tag != "LifetimeUpdate");
            malformed.lifecycle_tail = super::super::lifecycle_tail::encode_lifecycle_tail(&tail);
            super::super::combat_projectile_persist::append(&mut malformed.lifecycle_tail, &source)
                .unwrap();
            super::super::body_kind_persist::append(&mut malformed.lifecycle_tail, &source)
                .unwrap();
        }
        assert!(
            builder
                .restore_from_snapshot(&malformed, &mut candidate)
                .is_err()
        );
        assert_eq!(candidate.frame, 99);
        assert_eq!(candidate.native_hulk_max_lifetime_override, 123);
        assert_eq!(candidate.objects[&owner].native_lifecycle, before);
    }
}

#[test]
fn old_absence_imports_receiving_bound_empty_and_resaves_explicitly() {
    let name = "OldAbsentPoisonFieldSmall";
    let mut source = GameLogic::new();
    source
        .templates
        .insert(name.into(), ThingTemplate::new(name));
    let id = source.create_object(name, Team::USA, Vec3::ZERO).unwrap();
    // This plain legacy name must not acquire inferred expiry during admission.
    source.objects.get_mut(&id).unwrap().lifetime_update = None;
    source.objects.get_mut(&id).unwrap().height_die = None;
    let builder = super::super::SnapshotBuilder::new();
    let mut old = builder.create_world_snapshot(&source).unwrap();
    let mut offset =
        super::super::combat_projectile_persist::prefix_len(&old.lifecycle_tail).unwrap();
    for tag in [b"CPRJ", b"BDKD"] {
        assert_eq!(&old.lifecycle_tail[offset..offset + 4], tag);
        let length = u32::from_le_bytes(
            old.lifecycle_tail[offset + 8..offset + 12]
                .try_into()
                .unwrap(),
        ) as usize;
        offset += 12 + length;
    }
    assert_eq!(&old.lifecycle_tail[offset..offset + 4], b"NLFC");
    let length = u32::from_le_bytes(
        old.lifecycle_tail[offset + 8..offset + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    old.lifecycle_tail.drain(offset..offset + 12 + length);
    let mut receiving = GameLogic::new();
    let mut definition = ThingTemplate::new(name);
    definition.native_lifecycle_definitions = Some(Box::new([]));
    receiving.templates.insert(name.into(), definition);
    builder.restore_from_snapshot(&old, &mut receiving).unwrap();
    assert!(
        receiving.objects[&id]
            .native_lifecycle
            .as_ref()
            .unwrap()
            .states
            .is_empty()
    );
    let saved = builder.create_world_snapshot(&receiving).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = super::super::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut fresh = GameLogic::new();
    builder.restore_from_snapshot(&decoded, &mut fresh).unwrap();
    let runtime = fresh.objects[&id].native_lifecycle.as_ref().unwrap();
    assert!(runtime.definitions.is_empty());
    assert!(runtime.states.is_empty());
    assert!(fresh.objects[&id].lifetime_update.is_none());
    assert!(fresh.objects[&id].height_die.is_none());
}
