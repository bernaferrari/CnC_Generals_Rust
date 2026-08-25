#![cfg(test)]

use super::super::*;
use super::temporary_weapon_status::{
    apply_private_fire_mutation, load_ammo_now, promote_temporary_weapon_status,
    store_fields_for_weapon_name,
};
use super::weapon_visual_capture::select_weapon_template_fx;
use super::weapon_visual_freeze::{DrawModuleFireFxClass, classify_draw_module_declaration};
use crate::game_logic::host_temporary_weapon_behavior::{
    FireWeaponDamageTypeMask, FireWeaponUpgradeMuxMetadata, FireWeaponWhenDamagedMetadata,
    FireWeaponWhenDamagedRuntimeState, FireWeaponWhenDamagedWeaponRole, FireWeaponWhenDeadMetadata,
    FireWeaponWhenDeadRuntimeState, TemporaryWeaponConstructionDefaults, TemporaryWeaponRuntimeKey,
    TemporaryWeaponRuntimeSpec, TemporaryWeaponRuntimeState, TemporaryWeaponSlot,
    TemporaryWeaponStatus,
};

fn ready_weapon_on(
    template: &str,
    role: FireWeaponWhenDamagedWeaponRole,
    module_source_index: u32,
) -> TemporaryWeaponRuntimeState {
    let spec = TemporaryWeaponRuntimeSpec {
        key: TemporaryWeaponRuntimeKey {
            module_source_index,
            role,
        },
        weapon_template_name: template.to_string(),
        weapon_slot: TemporaryWeaponSlot::Primary,
    };
    let mut state = TemporaryWeaponRuntimeState::from_cxx_constructor(
        &spec,
        TemporaryWeaponConstructionDefaults {
            clip_size: 2,
            clip_reload_frames: 0,
            shots_per_barrel: 1,
            ..TemporaryWeaponConstructionDefaults::default()
        },
        0,
    );
    load_ammo_now(
        &mut state,
        TemporaryWeaponConstructionDefaults {
            clip_size: 2,
            clip_reload_frames: 0,
            shots_per_barrel: 1,
            ..TemporaryWeaponConstructionDefaults::default()
        },
        0,
    );
    state
}

fn ready_weapon(
    template: &str,
    role: FireWeaponWhenDamagedWeaponRole,
) -> TemporaryWeaponRuntimeState {
    ready_weapon_on(template, role, 0)
}

fn damaged_object(id: u32, weapon: TemporaryWeaponRuntimeState) -> Object {
    let mut template = ThingTemplate::new("TempWeaponHost");
    template.fire_weapon_when_damaged_behaviors = vec![FireWeaponWhenDamagedMetadata {
        module_source_index: 0,
        module_tag: None,
        starts_active: true,
        damage_types: FireWeaponDamageTypeMask::ALL,
        damage_amount: 5.0,
        upgrade_mux: FireWeaponUpgradeMuxMetadata::default(),
        weapon_template_names: std::array::from_fn(|index| {
            (index == weapon.key.role.index()).then(|| weapon.weapon_template_name.clone())
        }),
    }];
    let mut object = Object::new(template, ObjectId(id), Team::USA);
    object.health.current = 80.0;
    object.health.maximum = 100.0;
    object.max_health = 100.0;
    let mut runtime = FireWeaponWhenDamagedRuntimeState {
        module_source_index: 0,
        upgrade_executed: true,
        ..FireWeaponWhenDamagedRuntimeState::default()
    };
    assert!(runtime.replace_weapon_state(weapon));
    object.temporary_weapon_runtime.damaged = vec![runtime];
    object
}

#[test]
fn damaged_reaction_fires_when_ready_and_threshold_met() {
    let mut logic = GameLogic::new();
    logic.frame = 10;
    let weapon = ready_weapon(
        "CrusaderTankGun",
        FireWeaponWhenDamagedWeaponRole::ReactionPristine,
    );
    let source = ObjectId(11);
    logic.objects.insert(source, damaged_object(11, weapon));
    let hits = logic.execute_temporary_weapon_on_damage(source, 10.0, 0);
    let _ = hits;
    let object = logic.host_object(source).expect("source");
    let state = object.temporary_weapon_runtime.damaged[0]
        .weapon(TemporaryWeaponRuntimeKey {
            module_source_index: 0,
            role: FireWeaponWhenDamagedWeaponRole::ReactionPristine,
        })
        .expect("role");
    if store_fields_for_weapon_name("CrusaderTankGun").is_some() {
        assert_eq!(state.ammo_in_clip, 1);
        assert_eq!(state.last_fire_frame, 10);
        assert!(logic.weapon_discharge_next_sequence_for_snapshot() > 1);
    } else {
        assert_eq!(state.ammo_in_clip, 2);
    }
}

#[test]
fn damaged_reaction_does_not_fire_below_damage_amount() {
    let mut logic = GameLogic::new();
    logic.frame = 10;
    let weapon = ready_weapon(
        "CrusaderTankGun",
        FireWeaponWhenDamagedWeaponRole::ReactionPristine,
    );
    let source = ObjectId(12);
    logic.objects.insert(source, damaged_object(12, weapon));
    assert_eq!(logic.execute_temporary_weapon_on_damage(source, 1.0, 0), 0);
    let object = logic.host_object(source).expect("source");
    let state = object.temporary_weapon_runtime.damaged[0]
        .weapon(TemporaryWeaponRuntimeKey {
            module_source_index: 0,
            role: FireWeaponWhenDamagedWeaponRole::ReactionPristine,
        })
        .expect("role");
    assert_eq!(state.ammo_in_clip, 2);
    assert_eq!(state.last_fire_frame, 0);
}

#[test]
fn damaged_reaction_respects_between_shots_cooldown() {
    let mut logic = GameLogic::new();
    logic.frame = 4;
    let mut weapon = ready_weapon(
        "CrusaderTankGun",
        FireWeaponWhenDamagedWeaponRole::ReactionPristine,
    );
    weapon.status = TemporaryWeaponStatus::BetweenFiringShots;
    weapon.when_we_can_fire_again = 20;
    let source = ObjectId(13);
    logic.objects.insert(source, damaged_object(13, weapon));
    assert_eq!(logic.execute_temporary_weapon_on_damage(source, 10.0, 0), 0);
}

#[test]
fn dead_behavior_fires_once_and_skips_under_construction() {
    let mut logic = GameLogic::new();
    logic.frame = 8;
    let mut template = ThingTemplate::new("TempDeadHost");
    template.fire_weapon_when_dead_behaviors = vec![FireWeaponWhenDeadMetadata {
        module_source_index: 0,
        module_tag: None,
        starts_active: true,
        death_weapon: Some("CrusaderTankGun".into()),
        upgrade_mux: FireWeaponUpgradeMuxMetadata::default(),
        death_types: Default::default(),
        veterancy_levels: Default::default(),
        exempt_status: Default::default(),
        required_status: Default::default(),
    }];
    let mut object = Object::new(template, ObjectId(14), Team::USA);
    object.temporary_weapon_runtime.dead = vec![FireWeaponWhenDeadRuntimeState {
        module_source_index: 0,
        upgrade_executed: true,
    }];
    object.status.destroyed = true;
    let source = ObjectId(14);
    logic.objects.insert(source, object);
    let _ = logic.execute_temporary_weapon_on_die(source);
    assert!(
        logic
            .host_object(source)
            .expect("source")
            .fire_weapon_when_dead_fired
    );
    let second = logic.execute_temporary_weapon_on_die(source);
    assert_eq!(second, 0);

    if let Some(object) = logic.objects.get_mut(&source) {
        object.fire_weapon_when_dead_fired = false;
        object.status.under_construction = true;
    }
    assert_eq!(logic.execute_temporary_weapon_on_die(source), 0);
}

/// C++ FireWeaponWhenDeadBehavior.cpp:65-88 — TriggeredBy activates; ConflictsWith
/// only skips when the object owns the conflicting upgrade.
#[test]
fn dead_behavior_honors_triggered_by_and_conflicts_with_ownership() {
    let mut logic = GameLogic::new();
    logic.frame = 8;
    let mux_default = FireWeaponUpgradeMuxMetadata {
        conflicts_with: vec!["Upgrade_GLABombTruckHighExplosiveBomb".into()],
        ..Default::default()
    };
    let mux_he = FireWeaponUpgradeMuxMetadata {
        triggered_by: vec!["Upgrade_GLABombTruckHighExplosiveBomb".into()],
        ..Default::default()
    };
    let mut template = ThingTemplate::new("GLAVehicleBombTruck");
    template.fire_weapon_when_dead_behaviors = vec![
        FireWeaponWhenDeadMetadata {
            module_source_index: 0,
            module_tag: None,
            starts_active: true,
            death_weapon: Some("BombTruckDefaultDeathWeapon".into()),
            upgrade_mux: mux_default,
            death_types: Default::default(),
            veterancy_levels: Default::default(),
            exempt_status: Default::default(),
            required_status: Default::default(),
        },
        FireWeaponWhenDeadMetadata {
            module_source_index: 1,
            module_tag: None,
            starts_active: false,
            death_weapon: Some("BombTruckHighExplosionDeathWeapon".into()),
            upgrade_mux: mux_he,
            death_types: Default::default(),
            veterancy_levels: Default::default(),
            exempt_status: Default::default(),
            required_status: Default::default(),
        },
    ];
    let mut object = Object::new(template, ObjectId(15), Team::GLA);
    object.temporary_weapon_runtime.dead = vec![
        FireWeaponWhenDeadRuntimeState {
            module_source_index: 0,
            upgrade_executed: false,
        },
        FireWeaponWhenDeadRuntimeState {
            module_source_index: 1,
            upgrade_executed: false,
        },
    ];
    object.status.destroyed = true;
    let source = ObjectId(15);
    logic.objects.insert(source, object);
    let first = logic.execute_temporary_weapon_on_die(source);
    assert!(
        first > 0,
        "StartsActive default with empty conflict ownership must fire"
    );
    assert!(
        logic
            .host_object(source)
            .expect("source")
            .fire_weapon_when_dead_fired
    );

    let mut object = logic.objects.remove(&source).expect("obj");
    object.fire_weapon_when_dead_fired = false;
    object.apply_upgrade_tag("Upgrade_GLABombTruckHighExplosiveBomb");
    object.temporary_weapon_runtime.dead[0].upgrade_executed = false;
    object.temporary_weapon_runtime.dead[1].upgrade_executed = false;
    logic.objects.insert(source, object);
    let upgraded = logic.execute_temporary_weapon_on_die(source);
    assert!(
        upgraded > 0,
        "TriggeredBy HE must activate exclusive death module"
    );
    assert!(
        logic
            .host_object(source)
            .expect("source")
            .fire_weapon_when_dead_fired
    );
}

/// C++ FireWeaponWhenDeadBehavior::onDie — every matching module fires.
/// Exclusive mux selects default vs HE; Bio still fires when owned.
#[test]
fn dead_behavior_fires_every_matching_module() {
    let mut logic = GameLogic::new();
    logic.frame = 8;
    let mux_default = FireWeaponUpgradeMuxMetadata {
        conflicts_with: vec!["Upgrade_GLABombTruckHighExplosiveBomb".into()],
        ..Default::default()
    };
    let mux_he = FireWeaponUpgradeMuxMetadata {
        triggered_by: vec!["Upgrade_GLABombTruckHighExplosiveBomb".into()],
        ..Default::default()
    };
    let mux_bio = FireWeaponUpgradeMuxMetadata {
        triggered_by: vec!["Upgrade_GLABombTruckBioBomb".into()],
        ..Default::default()
    };
    let mut template = ThingTemplate::new("GLAVehicleBombTruckMulti");
    template.fire_weapon_when_dead_behaviors = vec![
        FireWeaponWhenDeadMetadata {
            module_source_index: 0,
            module_tag: None,
            starts_active: true,
            death_weapon: Some("CrusaderTankGun".into()),
            upgrade_mux: mux_default,
            death_types: Default::default(),
            veterancy_levels: Default::default(),
            exempt_status: Default::default(),
            required_status: Default::default(),
        },
        FireWeaponWhenDeadMetadata {
            module_source_index: 1,
            module_tag: None,
            starts_active: false,
            death_weapon: Some("PaladinTankGun".into()),
            upgrade_mux: mux_he,
            death_types: Default::default(),
            veterancy_levels: Default::default(),
            exempt_status: Default::default(),
            required_status: Default::default(),
        },
        FireWeaponWhenDeadMetadata {
            module_source_index: 2,
            module_tag: None,
            starts_active: false,
            death_weapon: Some("CrusaderTankGun".into()),
            upgrade_mux: mux_bio,
            death_types: Default::default(),
            veterancy_levels: Default::default(),
            exempt_status: Default::default(),
            required_status: Default::default(),
        },
    ];
    let mut object = Object::new(template, ObjectId(16), Team::GLA);
    object.temporary_weapon_runtime.dead = vec![
        FireWeaponWhenDeadRuntimeState {
            module_source_index: 0,
            upgrade_executed: false,
        },
        FireWeaponWhenDeadRuntimeState {
            module_source_index: 1,
            upgrade_executed: false,
        },
        FireWeaponWhenDeadRuntimeState {
            module_source_index: 2,
            upgrade_executed: false,
        },
    ];
    object.status.destroyed = true;
    object.apply_upgrade_tag("Upgrade_GLABombTruckHighExplosiveBomb");
    object.apply_upgrade_tag("Upgrade_GLABombTruckBioBomb");
    let source = ObjectId(16);
    logic.objects.insert(source, object);
    let hits = logic.execute_temporary_weapon_on_die(source);
    assert!(
        hits > 0,
        "hq-ys6gk: HE+Bio must fire leftover death modules"
    );
    assert!(
        logic
            .host_object(source)
            .expect("source")
            .fire_weapon_when_dead_fired
    );
}

#[test]
fn dead_behavior_store_miss_does_not_stamp_fired() {
    let mut logic = GameLogic::new();
    logic.frame = 8;
    let mut template = ThingTemplate::new("TempDeadMissingWeapon");
    template.fire_weapon_when_dead_behaviors = vec![FireWeaponWhenDeadMetadata {
        module_source_index: 0,
        module_tag: None,
        starts_active: true,
        death_weapon: Some("DefinitelyNotARealWeaponTemplate".into()),
        upgrade_mux: FireWeaponUpgradeMuxMetadata::default(),
        death_types: Default::default(),
        veterancy_levels: Default::default(),
        exempt_status: Default::default(),
        required_status: Default::default(),
    }];
    let mut object = Object::new(template, ObjectId(17), Team::GLA);
    object.temporary_weapon_runtime.dead = vec![FireWeaponWhenDeadRuntimeState {
        module_source_index: 0,
        upgrade_executed: true,
    }];
    object.status.destroyed = true;
    let source = ObjectId(17);
    logic.objects.insert(source, object);
    assert_eq!(logic.execute_temporary_weapon_on_die(source), 0);
    assert!(
        !logic
            .host_object(source)
            .expect("source")
            .fire_weapon_when_dead_fired,
        "hq-ys6gk: store miss must not stamp fired (residual blast still allowed)"
    );
}

#[test]
fn damaged_reaction_rejects_non_matching_damage_type() {
    let mut logic = GameLogic::new();
    logic.frame = 10;
    let weapon = ready_weapon(
        "CrusaderTankGun",
        FireWeaponWhenDamagedWeaponRole::ReactionPristine,
    );
    let mut object = damaged_object(16, weapon);
    object.thing.template.fire_weapon_when_damaged_behaviors[0].damage_types =
        FireWeaponDamageTypeMask(1u64 << 6); // C++ DAMAGE_FLAME
    let source = ObjectId(16);
    logic.objects.insert(source, object);
    assert_eq!(
        logic.execute_temporary_weapon_on_damage(source, 10.0, 0),
        0,
        "EXPLOSION must not spark a FLAME-only reaction"
    );
}

/// C++ FireWeaponWhenDamagedBehavior::onDamage — every ready module fires.
#[test]
fn damaged_reaction_fires_every_ready_module() {
    let mut logic = GameLogic::new();
    logic.frame = 10;
    let first = ready_weapon_on(
        "CrusaderTankGun",
        FireWeaponWhenDamagedWeaponRole::ReactionPristine,
        0,
    );
    let second = ready_weapon_on(
        "CrusaderTankGun",
        FireWeaponWhenDamagedWeaponRole::ReactionPristine,
        1,
    );
    let mut template = ThingTemplate::new("TempWeaponHostMulti");
    template.fire_weapon_when_damaged_behaviors = vec![
        FireWeaponWhenDamagedMetadata {
            module_source_index: 0,
            module_tag: None,
            starts_active: true,
            damage_types: FireWeaponDamageTypeMask::ALL,
            damage_amount: 5.0,
            upgrade_mux: FireWeaponUpgradeMuxMetadata::default(),
            weapon_template_names: std::array::from_fn(|index| {
                (index == first.key.role.index()).then(|| first.weapon_template_name.clone())
            }),
        },
        FireWeaponWhenDamagedMetadata {
            module_source_index: 1,
            module_tag: None,
            starts_active: true,
            damage_types: FireWeaponDamageTypeMask::ALL,
            damage_amount: 5.0,
            upgrade_mux: FireWeaponUpgradeMuxMetadata::default(),
            weapon_template_names: std::array::from_fn(|index| {
                (index == second.key.role.index()).then(|| second.weapon_template_name.clone())
            }),
        },
    ];
    let mut object = Object::new(template, ObjectId(21), Team::USA);
    object.health.current = 80.0;
    object.health.maximum = 100.0;
    object.max_health = 100.0;
    let mut runtime0 = FireWeaponWhenDamagedRuntimeState {
        module_source_index: 0,
        upgrade_executed: true,
        ..FireWeaponWhenDamagedRuntimeState::default()
    };
    let mut runtime1 = FireWeaponWhenDamagedRuntimeState {
        module_source_index: 1,
        upgrade_executed: true,
        ..FireWeaponWhenDamagedRuntimeState::default()
    };
    assert!(runtime0.replace_weapon_state(first));
    assert!(runtime1.replace_weapon_state(second));
    object.temporary_weapon_runtime.damaged = vec![runtime0, runtime1];
    let source = ObjectId(21);
    logic.objects.insert(source, object);
    let keys = logic.damaged_reaction_plan(source, 10.0, 0);
    assert_eq!(
        keys.len(),
        2,
        "hq-4eg3v: leftover/C++ fire every ready module"
    );
    assert_eq!(keys[0].module_source_index, 0);
    assert_eq!(keys[1].module_source_index, 1);
    let _ = logic.execute_temporary_weapon_on_damage(source, 10.0, 0);
    let object = logic.host_object(source).expect("source");
    let state0 = object.temporary_weapon_runtime.damaged[0]
        .weapon(TemporaryWeaponRuntimeKey {
            module_source_index: 0,
            role: FireWeaponWhenDamagedWeaponRole::ReactionPristine,
        })
        .expect("role0");
    let state1 = object.temporary_weapon_runtime.damaged[1]
        .weapon(TemporaryWeaponRuntimeKey {
            module_source_index: 1,
            role: FireWeaponWhenDamagedWeaponRole::ReactionPristine,
        })
        .expect("role1");
    if store_fields_for_weapon_name("CrusaderTankGun").is_some() {
        assert_eq!(state0.ammo_in_clip, 1, "first module must fire");
        assert_eq!(state1.ammo_in_clip, 1, "second module must fire");
        assert_eq!(state0.last_fire_frame, 10);
        assert_eq!(state1.last_fire_frame, 10);
    }
}

/// C++ FireWeaponWhenDamagedBehavior::update — every ready continuous module fires.
#[test]
fn damaged_continuous_fires_every_ready_module() {
    let mut logic = GameLogic::new();
    logic.frame = 10;
    let first = ready_weapon_on(
        "CrusaderTankGun",
        FireWeaponWhenDamagedWeaponRole::ContinuousPristine,
        0,
    );
    let second = ready_weapon_on(
        "CrusaderTankGun",
        FireWeaponWhenDamagedWeaponRole::ContinuousPristine,
        1,
    );
    let mut template = ThingTemplate::new("TempWeaponHostMultiCont");
    template.fire_weapon_when_damaged_behaviors = vec![
        FireWeaponWhenDamagedMetadata {
            module_source_index: 0,
            module_tag: None,
            starts_active: true,
            damage_types: FireWeaponDamageTypeMask::ALL,
            damage_amount: 5.0,
            upgrade_mux: FireWeaponUpgradeMuxMetadata::default(),
            weapon_template_names: std::array::from_fn(|index| {
                (index == first.key.role.index()).then(|| first.weapon_template_name.clone())
            }),
        },
        FireWeaponWhenDamagedMetadata {
            module_source_index: 1,
            module_tag: None,
            starts_active: true,
            damage_types: FireWeaponDamageTypeMask::ALL,
            damage_amount: 5.0,
            upgrade_mux: FireWeaponUpgradeMuxMetadata::default(),
            weapon_template_names: std::array::from_fn(|index| {
                (index == second.key.role.index()).then(|| second.weapon_template_name.clone())
            }),
        },
    ];
    let mut object = Object::new(template, ObjectId(22), Team::USA);
    object.health.current = 80.0;
    object.health.maximum = 100.0;
    object.max_health = 100.0;
    let mut runtime0 = FireWeaponWhenDamagedRuntimeState {
        module_source_index: 0,
        upgrade_executed: true,
        ..FireWeaponWhenDamagedRuntimeState::default()
    };
    let mut runtime1 = FireWeaponWhenDamagedRuntimeState {
        module_source_index: 1,
        upgrade_executed: true,
        ..FireWeaponWhenDamagedRuntimeState::default()
    };
    assert!(runtime0.replace_weapon_state(first));
    assert!(runtime1.replace_weapon_state(second));
    object.temporary_weapon_runtime.damaged = vec![runtime0, runtime1];
    let source = ObjectId(22);
    logic.objects.insert(source, object);
    let keys = logic.damaged_continuous_plan(source);
    assert_eq!(
        keys.len(),
        2,
        "hq-4eg3v: leftover/C++ continuous fires every ready module"
    );
    assert_eq!(keys[0].module_source_index, 0);
    assert_eq!(keys[1].module_source_index, 1);
    let _ = logic.execute_temporary_weapon_continuous(source);
    let object = logic.host_object(source).expect("source");
    let state0 = object.temporary_weapon_runtime.damaged[0]
        .weapon(TemporaryWeaponRuntimeKey {
            module_source_index: 0,
            role: FireWeaponWhenDamagedWeaponRole::ContinuousPristine,
        })
        .expect("role0");
    let state1 = object.temporary_weapon_runtime.damaged[1]
        .weapon(TemporaryWeaponRuntimeKey {
            module_source_index: 1,
            role: FireWeaponWhenDamagedWeaponRole::ContinuousPristine,
        })
        .expect("role1");
    if store_fields_for_weapon_name("CrusaderTankGun").is_some() {
        assert_eq!(state0.ammo_in_clip, 1, "first continuous module must fire");
        assert_eq!(state1.ammo_in_clip, 1, "second continuous module must fire");
        assert_eq!(state0.last_fire_frame, 10);
        assert_eq!(state1.last_fire_frame, 10);
    }
}

#[test]
fn fx_selection_uses_detonate_only_when_projectile_detonation() {
    assert_eq!(
        select_weapon_template_fx(false, "FireFX", "DetonateFX"),
        "FireFX"
    );
    assert_eq!(
        select_weapon_template_fx(true, "FireFX", "DetonateFX"),
        "DetonateFX"
    );
}

#[test]
fn non_w3d_object_draw_interface_is_opaque_not_known_no() {
    assert_eq!(
        classify_draw_module_declaration("W3DModelDraw ModuleTag_01"),
        DrawModuleFireFxClass::W3dModelDraw
    );
    assert_eq!(
        classify_draw_module_declaration("W3DLaserDraw"),
        DrawModuleFireFxClass::KnownNoWeaponFireFx
    );
    assert_eq!(
        classify_draw_module_declaration("CustomObjectDraw"),
        DrawModuleFireFxClass::OpaqueObjectDrawInterface
    );
}

#[test]
fn private_fire_mutation_enters_between_shots() {
    let mut state = ready_weapon("Local", FireWeaponWhenDamagedWeaponRole::ReactionDamaged);
    let fields = super::temporary_weapon_status::TemporaryWeaponStoreFields {
        defaults: TemporaryWeaponConstructionDefaults {
            clip_size: 2,
            shots_per_barrel: 1,
            ..TemporaryWeaponConstructionDefaults::default()
        },
        delay_between_shots: 6,
        auto_reloads_clip: true,
        primary_damage: 10.0,
        primary_radius: 5.0,
        secondary_damage: 0.0,
        secondary_radius: 0.0,
    };
    apply_private_fire_mutation(&mut state, fields, 3);
    assert_eq!(state.ammo_in_clip, 1);
    assert_eq!(state.status, TemporaryWeaponStatus::BetweenFiringShots);
    assert_eq!(state.when_we_can_fire_again, 9);
    assert_eq!(
        promote_temporary_weapon_status(&mut state, 8),
        TemporaryWeaponStatus::BetweenFiringShots
    );
    assert_eq!(
        promote_temporary_weapon_status(&mut state, 9),
        TemporaryWeaponStatus::ReadyToFire
    );
}
