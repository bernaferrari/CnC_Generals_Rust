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
    let modules = "Behavior = FireWeaponWhenDeadBehavior Default\n StartsActive = Yes\n ConflictsWith = Upgrade_GLABombTruckHighExplosiveBomb\n DeathWeapon = TempDeadMuxDefault\nEnd\n\
        Behavior = FireWeaponWhenDeadBehavior Upgraded\n StartsActive = No\n TriggeredBy = Upgrade_GLABombTruckHighExplosiveBomb\n DeathWeapon = TempDeadMuxHE\nEnd";
    let weapons = [("TempDeadMuxDefault", 1000), ("TempDeadMuxHE", 2000)]
        .into_iter().map(|(name, damage)| format!(
            "Weapon {name}\n PrimaryDamage = {damage}\n PrimaryDamageRadius = 50\n DamageType = EXPLOSION\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n WeaponSpeed = 999999\n FireOCL = OCL_TemporaryConsumer\nEnd\n"
        )).collect::<String>();
    for upgraded in [false, true] {
        let (mut logic, source, victim) =
            authored_temporary_world_with_victim_health(modules, &weapons, 3000);
        if upgraded {
            logic.objects.get_mut(&source).unwrap()
                .apply_upgrade_tag("Upgrade_GLABombTruckHighExplosiveBomb");
        }
        logic.apply_owned_kill(
            source, crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        ).unwrap();
        assert_eq!(
            logic.objects[&victim].health.current,
            if upgraded { 1000.0 } else { 2000.0 },
            "only the applicable death weapon deals its authored damage",
        );
        assert_eq!(temporary_children(&logic), 1, "only one death FireOCL executes");
        logic.fire_temporary_weapons_for_pending_deaths();
        assert_eq!(temporary_children(&logic), 1, "pending-death pump must not replay onDie");
    }
}

/// C++ FireWeaponWhenDeadBehavior::onDie — every matching module fires.
/// Exclusive mux selects default vs HE; Bio still fires when owned.
#[test]
fn dead_behavior_fires_every_matching_module() {
    let modules = "Behavior = FireWeaponWhenDeadBehavior Default\n StartsActive = Yes\n ConflictsWith = Upgrade_GLABombTruckHighExplosiveBomb\n DeathWeapon = TempDeadMultiBio\nEnd\n\
        Behavior = FireWeaponWhenDeadBehavior HE\n StartsActive = No\n TriggeredBy = Upgrade_GLABombTruckHighExplosiveBomb\n DeathWeapon = TempDeadMultiHE\nEnd\n\
        Behavior = FireWeaponWhenDeadBehavior Bio\n StartsActive = No\n TriggeredBy = Upgrade_GLABombTruckBioBomb\n DeathWeapon = TempDeadMultiBio\nEnd";
    let weapons = ["TempDeadMultiHE", "TempDeadMultiBio"].into_iter().map(|name| format!(
        "Weapon {name}\n PrimaryDamage = 60\n PrimaryDamageRadius = 5\n DamageType = EXPLOSION\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n WeaponSpeed = 999999\n FireOCL = OCL_TemporaryConsumer\nEnd\n"
    )).collect::<String>();
    let (mut logic, source, victim) =
        authored_temporary_world_with_victim_health(modules, &weapons, 1000);
    let object = logic.objects.get_mut(&source).unwrap();
    object.apply_upgrade_tag("Upgrade_GLABombTruckHighExplosiveBomb");
    object.apply_upgrade_tag("Upgrade_GLABombTruckBioBomb");
    logic.apply_owned_kill(
        source, crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
    ).unwrap();
    assert_eq!(logic.objects[&victim].health.current, 880.0);
    assert_eq!(temporary_children(&logic), 2, "HE and Bio each execute their template");
    logic.fire_temporary_weapons_for_pending_deaths();
    assert_eq!(logic.objects[&victim].health.current, 880.0);
    assert_eq!(temporary_children(&logic), 2);
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
        "a missing authored weapon must not report a successful firing"
    );
}

fn retail_named_authored_die_health(modules: Vec<FireWeaponWhenDeadMetadata>) -> f32 {
    use crate::assets::ini_parser::BehaviorModuleDefinition;
    use crate::game_logic::object::OnDieInput;

    let mut logic = GameLogic::new();
    let mut weapon = gamelogic::weapon::WeaponTemplate::new("TestAuthoredDeathWeapon".into());
    weapon.primary_damage = 25.0;
    weapon.primary_damage_radius = 10.0;
    weapon.damage_dealt_at_self_position = true;
    logic.weapon_definitions = Some(std::collections::HashMap::from([
        ("testauthoreddeathweapon".into(), std::sync::Arc::new(weapon)),
    ]));
    let mut template = ThingTemplate::new("GLAVehicleBombTruck");
    let declarations: Vec<_> = modules.iter().map(|_| BehaviorModuleDefinition {
        class_name: "FireWeaponWhenDeadBehavior".into(),
        module_tag: None,
        attributes: Default::default(),
    }).chain(std::iter::once(BehaviorModuleDefinition {
        class_name: "DestroyDie".into(),
        module_tag: None,
        attributes: Default::default(),
    })).collect();
    template.native_die_declarations = Some(
        crate::game_logic::native_die_definition::bind(&declarations).into_boxed_slice(),
    );
    template.fire_weapon_when_dead_behaviors = modules;
    let source = ObjectId(21000);
    let mut object = Object::new(template, source, Team::GLA);
    object.status.destroyed = true;
    logic.objects.insert(source, object);
    let mut victim_template = ThingTemplate::new("AuthoredDieAdmissionVictim");
    victim_template.set_health(1000.0);
    logic.templates.insert(victim_template.name.clone(), victim_template);
    let victim = logic.create_object(
        "AuthoredDieAdmissionVictim", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0),
    ).unwrap();
    let input = OnDieInput {
        source: None,
        damage_type: crate::game_logic::combat::DamageType::Unresistable,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        actual_damage_dealt: 100.0,
        actual_damage_clipped: 100.0,
    };
    logic.mark_object_for_destruction_with_topple(source, None, input, None).unwrap();
    assert!(!logic.objects[&source].fire_weapon_when_dead_fired);
    let health_after_callbacks = logic.objects[&victim].health.current;
    logic.fire_temporary_weapons_for_pending_deaths();
    logic.process_destroy_list();
    assert_eq!(logic.objects[&victim].health.current, health_after_callbacks);
    health_after_callbacks
}

fn authored_die_module(index: u32, weapon: &str) -> FireWeaponWhenDeadMetadata {
    FireWeaponWhenDeadMetadata {
        module_source_index: index,
        module_tag: None,
        starts_active: true,
        death_weapon: Some(weapon.into()),
        upgrade_mux: FireWeaponUpgradeMuxMetadata::default(),
        death_types: Default::default(),
        veterancy_levels: Default::default(),
        exempt_status: Default::default(),
        required_status: Default::default(),
    }
}

#[test]
fn retail_bomb_truck_name_without_die_module_does_not_detonate() {
    assert_eq!(retail_named_authored_die_health(Vec::new()), 1000.0);
    // The older, unbound template path must not infer a weapon from the name either.
    let mut logic = GameLogic::new();
    let source = ObjectId(21001);
    logic.objects.insert(source, Object::new(
        ThingTemplate::new("GLAVehicleBombTruck"), source, Team::GLA,
    ));
    let mut victim_template = ThingTemplate::new("UnboundRetailNameVictim");
    victim_template.set_health(1000.0);
    logic.templates.insert(victim_template.name.clone(), victim_template);
    let victim = logic.create_object(
        "UnboundRetailNameVictim", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0),
    ).unwrap();
    logic.mark_object_for_destruction(source, None).unwrap();
    logic.process_destroy_list();
    assert_eq!(logic.objects[&victim].health.current, 1000.0);
}

#[test]
fn missing_death_weapon_neither_detonates_nor_suppresses_later_module() {
    assert_eq!(
        retail_named_authored_die_health(vec![authored_die_module(0, "MissingAuthoredWeapon")]),
        1000.0,
    );
    assert_eq!(
        retail_named_authored_die_health(vec![
            authored_die_module(0, "MissingAuthoredWeapon"),
            authored_die_module(1, "TestAuthoredDeathWeapon"),
        ]),
        975.0,
    );
}

#[test]
fn filtered_death_module_does_not_fire_or_suppress_a_later_module() {
    let mut filtered = authored_die_module(0, "TestAuthoredDeathWeapon");
    filtered.death_types = crate::game_logic::host_temporary_weapon_behavior::FireWeaponDeathTypeMask::NONE;
    assert_eq!(retail_named_authored_die_health(vec![filtered.clone()]), 1000.0);
    assert_eq!(
        retail_named_authored_die_health(vec![filtered, authored_die_module(1, "TestAuthoredDeathWeapon")]),
        975.0,
    );
}

#[test]
fn authored_death_callbacks_fire_each_applicable_module_before_removal() {
    use crate::assets::ini_parser::BehaviorModuleDefinition;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::object::OnDieInput;

    // Bind the definitions to this GameLogic, not the free INI registration
    // helper's temporary WorldServices. Death must use the receiving catalog.
    let mut logic = GameLogic::new();
    let mut weapons = std::collections::HashMap::new();
    for (name, damage) in [
        ("AuthoredDeathDefault", 30.0),
        ("AuthoredDeathUpgraded", 60.0),
        ("AuthoredDeathExtra", 20.0),
    ] {
        let mut weapon = gamelogic::weapon::WeaponTemplate::new(name.into());
        weapon.primary_damage = damage;
        weapon.primary_damage_radius = 10.0;
        weapon.damage_dealt_at_self_position = true;
        weapons.insert(name.to_ascii_lowercase(), std::sync::Arc::new(weapon));
    }
    logic.weapon_definitions = Some(weapons);
    let mut template = ThingTemplate::new("CustomAuthoredDemolitionUnit");
    let module = |source_index, weapon: &str, starts_active, upgrade_mux| {
        FireWeaponWhenDeadMetadata {
            module_source_index: source_index,
            module_tag: None,
            starts_active,
            death_weapon: Some(weapon.into()),
            upgrade_mux,
            death_types: Default::default(),
            veterancy_levels: Default::default(),
            exempt_status: Default::default(),
            required_status: Default::default(),
        }
    };
    template.fire_weapon_when_dead_behaviors = vec![
        module(0, "AuthoredDeathDefault", true, FireWeaponUpgradeMuxMetadata {
            conflicts_with: vec!["Upgrade_TestHE".into()],
            ..Default::default()
        }),
        module(1, "AuthoredDeathUpgraded", false, FireWeaponUpgradeMuxMetadata {
            triggered_by: vec!["Upgrade_TestHE".into()],
            ..Default::default()
        }),
        module(2, "AuthoredDeathExtra", false, FireWeaponUpgradeMuxMetadata {
            triggered_by: vec!["Upgrade_TestBio".into()],
            ..Default::default()
        }),
    ];
    let declarations: Vec<_> = (0..3).map(|_| BehaviorModuleDefinition {
        class_name: "FireWeaponWhenDeadBehavior".into(),
        module_tag: None,
        attributes: Default::default(),
    }).chain(std::iter::once(BehaviorModuleDefinition {
        class_name: "DestroyDie".into(),
        module_tag: None,
        attributes: Default::default(),
    })).collect();
    template.native_die_declarations = Some(
        crate::game_logic::native_die_definition::bind(&declarations).into_boxed_slice()
    );
    let source = ObjectId(20001);
    let mut object = Object::new(template, source, Team::GLA);
    object.apply_upgrade_tag("Upgrade_TestHE");
    object.apply_upgrade_tag("Upgrade_TestBio");
    object.status.destroyed = true;
    logic.objects.insert(source, object);
    let mut victim_template = ThingTemplate::new("AuthoredDeathVictim");
    victim_template.set_health(1000.0);
    logic.templates.insert(victim_template.name.clone(), victim_template);
    let victim = logic.create_object(
        "AuthoredDeathVictim", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0)
    ).unwrap();
    let input = OnDieInput {
        source: None,
        damage_type: crate::game_logic::combat::DamageType::Unresistable,
        death_type: HostDeathType::Normal,
        actual_damage_dealt: 100.0,
        actual_damage_clipped: 100.0,
    };
    logic.mark_object_for_destruction_with_topple(source, None, input, None).unwrap();
    let source_after = logic.objects.get(&source).expect("death callback source remains installed");
    assert!(source_after.native_die_runtime.completed.contains(&1));
    assert!(source_after.native_die_runtime.completed.contains(&2));
    assert!(source_after.temporary_weapon_runtime.dead[1].upgrade_executed);
    assert!(source_after.temporary_weapon_runtime.dead[2].upgrade_executed);
    assert_eq!(logic.objects[&victim].health.current, 920.0);
    logic.fire_temporary_weapons_for_pending_deaths();
    logic.process_destroy_list();
    assert_eq!(logic.objects[&victim].health.current, 920.0);
}

#[test]
fn retail_he_bio_death_fires_one_blast_and_one_poison_field() {
    use crate::assets::ini_parser::BehaviorModuleDefinition;
    use crate::game_logic::object::OnDieInput;
    use gamelogic::effects::ObjectCreationList;
    use gamelogic::object_creation_list::GenericObjectCreationNugget;
    use std::sync::Arc;

    let mut logic = GameLogic::new();
    // Retail HE damage and combined HE/Bio effect are distinct die modules.
    let mut blast = gamelogic::weapon::WeaponTemplate::new("BombTruckHighExplosionBombDamage".into());
    blast.primary_damage = 2000.0;
    blast.primary_damage_radius = 50.0;
    blast.secondary_damage = 200.0;
    blast.secondary_damage_radius = 85.0;
    blast.damage_dealt_at_self_position = true;
    let mut weapon = gamelogic::weapon::WeaponTemplate::new("BombTruckHighExplosiveBioBombEffect".into());
    weapon.fire_ocl_names[0] = Some("OCL_PoisonFieldMedium".into());
    let mut poison = gamelogic::weapon::WeaponTemplate::new("MediumPoisonFieldWeapon".into());
    poison.primary_damage = 2.0;
    poison.primary_damage_radius = 80.0;
    poison.damage_type = gamelogic::damage::DamageType::Poison;
    poison.death_type = gamelogic::damage::DeathType::Poisoned;
    poison.affects_mask.insert(gamelogic::weapon::WeaponAffectsMask::DOESNT_AFFECT_AIRBORNE);
    poison.min_delay_between_shots = 15;
    poison.max_delay_between_shots = 15;
    let mut core = gamelogic::weapon::WeaponTemplate::new("HazardFieldCoreWeapon".into());
    core.primary_damage = 200.0;
    core.primary_damage_radius = 10.0;
    core.damage_type = gamelogic::damage::DamageType::HazardCleanup;
    core.clip_size = 1;
    core.clip_reload_time = 3000;
    logic.weapon_definitions = Some(std::collections::HashMap::from([
        ("bombtruckhighexplosionbombdamage".into(), Arc::new(blast)),
        ("bombtruckhighexplosivebiobombeffect".into(), Arc::new(weapon.clone())),
        ("mediumpoisonfieldweapon".into(), Arc::new(poison)),
        ("hazardfieldcoreweapon".into(), Arc::new(core)),
    ]));
    logic.admit_weapon_fire_definition("BombTruckHighExplosiveBioBombEffect", &weapon).unwrap();
    let mut ocl = ObjectCreationList::new();
    ocl.add_nugget(Arc::new(GenericObjectCreationNugget {
        names: vec!["PoisonFieldMedium".into()],
        ..Default::default()
    }));
    logic.admit_weapon_ocl_definition("OCL_PoisonFieldMedium", ocl).unwrap();
    let field_modules = [
        BehaviorModuleDefinition {
            class_name: "FireWeaponUpdate".into(),
            module_tag: Some("Poison".into()),
            attributes: [("Weapon".into(), "MediumPoisonFieldWeapon".into())].into(),
        },
        BehaviorModuleDefinition {
            class_name: "LifetimeUpdate".into(),
            module_tag: Some("Lifetime".into()),
            attributes: [("MinLifetime".into(), "30000".into()), ("MaxLifetime".into(), "30000".into())].into(),
        },
        BehaviorModuleDefinition {
            class_name: "FireWeaponUpdate".into(),
            module_tag: Some("Core".into()),
            attributes: [("Weapon".into(), "HazardFieldCoreWeapon".into())].into(),
        },
    ];
    let mut field = ThingTemplate::new("PoisonFieldMedium");
    field.native_lifecycle_definitions = Some(
        crate::game_logic::native_lifecycle_definition::parse_native_lifecycle_definitions(&field_modules)
            .unwrap().into_boxed_slice(),
    );
    logic.templates.insert(field.name.clone(), field);
    let mut template = ThingTemplate::new("GLAVehicleBombTruck");
    let module = |module_source_index, death_weapon: &str, upgrade_mux| FireWeaponWhenDeadMetadata {
        module_source_index,
        module_tag: None,
        starts_active: false,
        death_weapon: Some(death_weapon.into()),
        upgrade_mux,
        death_types: Default::default(),
        veterancy_levels: Default::default(),
        exempt_status: Default::default(),
        required_status: Default::default(),
    };
    template.fire_weapon_when_dead_behaviors = vec![
        module(0, "BombTruckHighExplosionBombDamage", FireWeaponUpgradeMuxMetadata {
            triggered_by: vec!["Upgrade_GLABombTruckHighExplosiveBomb".into()],
            ..Default::default()
        }),
        module(1, "BombTruckHighExplosiveBioBombEffect", FireWeaponUpgradeMuxMetadata {
            triggered_by: vec!["Upgrade_GLABombTruckHighExplosiveBomb".into(), "Upgrade_GLABombTruckBioBomb".into()],
            conflicts_with: vec!["Upgrade_GLAAnthraxBeta".into()],
            requires_all_triggers: true,
            ..Default::default()
        }),
    ];
    let declarations = ["FireWeaponWhenDeadBehavior", "FireWeaponWhenDeadBehavior", "DestroyDie"].map(|class_name| {
        BehaviorModuleDefinition { class_name: class_name.into(), module_tag: None, attributes: Default::default() }
    });
    template.native_die_declarations = Some(
        crate::game_logic::native_die_definition::bind(&declarations).into_boxed_slice()
    );
    let source = ObjectId(20003);
    let mut object = Object::new(template, source, Team::GLA);
    object.apply_upgrade_tag("Upgrade_GLABombTruckBioBomb");
    object.apply_upgrade_tag("Upgrade_GLABombTruckHighExplosiveBomb");
    object.status.destroyed = true;
    logic.objects.insert(source, object);
    let mut victim_template = ThingTemplate::new("RetailBombTruckBlastVictim");
    victim_template.set_health(5000.0);
    logic.templates.insert(victim_template.name.clone(), victim_template);
    let victim = logic.create_object(
        "RetailBombTruckBlastVictim", Team::USA, glam::Vec3::new(30.0, 0.0, 0.0),
    ).unwrap();
    let input = OnDieInput {
        source: None,
        damage_type: crate::game_logic::combat::DamageType::Unresistable,
        death_type: crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        actual_damage_dealt: 100.0,
        actual_damage_clipped: 100.0,
    };
    logic.mark_object_for_destruction_with_topple(source, None, input, None).unwrap();
    let children = |world: &GameLogic| world.host_objects().registered_values()
        .filter(|object| object.template_name == "PoisonFieldMedium").count();
    assert_eq!(children(&logic), 1);
    assert_eq!(logic.objects[&victim].health.current, 3000.0);
    logic.fire_temporary_weapons_for_pending_deaths();
    logic.process_destroy_list();
    assert_eq!(children(&logic), 1);
    assert_eq!(logic.objects[&victim].health.current, 3000.0);
    let field_id = logic.host_objects().registered()
        .find_map(|(id, object)| (object.template_name == "PoisonFieldMedium").then_some(*id))
        .expect("authored poison field");
    logic.frame = 1;
    logic.tick_native_lifecycle(field_id);
    assert_eq!(logic.objects[&victim].health.current, 2998.0);

    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&logic).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let snapshot = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut loaded = GameLogic::new();
    loaded.weapon_definitions = logic.weapon_definitions.clone();
    loaded.templates = logic.templates.clone();
    builder.restore_from_snapshot(&snapshot, &mut loaded).unwrap();
    loaded.frame = 15;
    loaded.tick_native_lifecycle(field_id);
    assert_eq!(loaded.objects[&victim].health.current, 2998.0);
    loaded.frame = 16;
    loaded.tick_native_lifecycle(field_id);
    assert_eq!(loaded.objects[&victim].health.current, 2996.0);
}

#[test]
fn authored_fire_weapon_update_suppresses_for_real_weapon_exclusive_delay() {
    use crate::game_logic::native_lifecycle_definition::{
        FireWeaponUpdateDefinition, NativeLifecycleDefinition, NativeLifecycleKind,
    };
    let mut logic = GameLogic::new();
    let mut pulse = gamelogic::weapon::WeaponTemplate::new("ExclusiveFieldPulse".into());
    pulse.primary_damage = 5.0;
    pulse.primary_damage_radius = 20.0;
    pulse.min_delay_between_shots = 15;
    pulse.max_delay_between_shots = 15;
    logic.weapon_definitions = Some(std::collections::HashMap::from([
        ("exclusivefieldpulse".into(), std::sync::Arc::new(pulse))
    ]));
    let mut field = ThingTemplate::new("ExclusiveAuthoredField");
    field.native_lifecycle_definitions = Some(vec![NativeLifecycleDefinition {
        module_tag: Some("Pulse".into()),
        kind: NativeLifecycleKind::FireWeapon(FireWeaponUpdateDefinition {
            weapon_name: "ExclusiveFieldPulse".into(),
            initial_delay: 0,
            exclusive_delay: 10,
        }),
    }].into_boxed_slice());
    logic.templates.insert(field.name.clone(), field);
    let mut victim_template = ThingTemplate::new("ExclusiveFieldVictim");
    victim_template.set_health(100.0);
    logic.templates.insert(victim_template.name.clone(), victim_template);
    let source = logic.create_object("ExclusiveAuthoredField", Team::GLA, glam::Vec3::ZERO).unwrap();
    let victim = logic.create_object("ExclusiveFieldVictim", Team::USA, glam::Vec3::new(3.0, 0.0, 0.0)).unwrap();
    logic.objects.get_mut(&source).unwrap().weapon = Some(Weapon {
        last_fire_frame: 5,
        ..Default::default()
    });
    logic.frame = 14;
    logic.tick_native_lifecycle(source);
    assert_eq!(logic.objects[&victim].health.current, 100.0);
    logic.frame = 15;
    logic.tick_native_lifecycle(source);
    assert_eq!(logic.objects[&victim].health.current, 95.0);
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
    object.template_mut().fire_weapon_when_damaged_behaviors[0].damage_types =
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
/// This is an admitted-owner module visitation fixture. Its own ENEMIES-only
/// instant weapon has no victim; it does not assert Crusader projectile parity.
#[test]
fn damaged_reaction_fires_every_ready_module() {
    let mut logic = GameLogic::new();
    logic.frame = 10;
    let weapon_name = "OwnedMultiModuleReaction539";
    let mut definitions = HashMap::new();
    assert_eq!(
        crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
            &mut definitions,
            &format!(
                "Weapon {weapon_name}\n PrimaryDamage = 60\n PrimaryDamageRadius = 20\n DamageType = EXPLOSION\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n ClipSize = 2\n ClipReloadTime = 0\n DelayBetweenShots = 0\nEnd\n"
            ),
            crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
        ),
        1,
    );
    logic
        .admit_weapon_content_snapshot(
            definitions
                .into_values()
                .map(|rule| (rule.name.clone(), rule))
                .collect(),
            &[],
        )
        .unwrap();
    let first = ready_weapon_on(
        weapon_name,
        FireWeaponWhenDamagedWeaponRole::ReactionPristine,
        0,
    );
    let second = ready_weapon_on(
        weapon_name,
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
    // Use real world admission: raw insertion leaves generation0, which
    // first discharge would stamp and correctly invalidate a captured lease.
    assert_eq!(logic.add_object(object), source);
    assert_eq!(logic.objects.registered_len(), 1);
    let identity = logic.live_draw_playback_identity(logic.host_object(source).unwrap());
    let keys = logic.damaged_reaction_plan(source, 10.0, 0);
    assert_eq!(
        keys.len(),
        2,
        "hq-4eg3v: leftover/C++ fire every ready module"
    );
    assert_eq!(keys[0].module_source_index, 0);
    assert_eq!(keys[1].module_source_index, 1);
    assert_eq!(logic.execute_temporary_weapon_on_damage(source, 10.0, 0), 0);
    let object = logic.host_object(source).expect("source");
    assert_eq!(
        object.health.current, 80.0,
        "neither module changes the Body role"
    );
    assert_eq!(logic.live_draw_playback_identity(object), identity);
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
    assert_eq!(state0.ammo_in_clip, 1, "first module must fire");
    assert_eq!(state1.ammo_in_clip, 1, "second module must fire");
    assert_eq!(state0.last_fire_frame, 10);
    assert_eq!(state1.last_fire_frame, 10);
}

/// C++ FireWeaponWhenDamagedBehavior::update — every ready continuous module fires.
#[test]
fn damaged_continuous_fires_every_ready_module() {
    let modules = "Behavior = FireWeaponWhenDamagedBehavior First\n StartsActive = Yes\n ContinuousWeaponPristine = OwnedTemporary\n End\n\
        Behavior = FireWeaponWhenDamagedBehavior Second\n StartsActive = Yes\n ContinuousWeaponPristine = OwnedTemporary\n End";
    let weapons = "Weapon OwnedTemporary\n PrimaryDamage = 17\n PrimaryDamageRadius = 10\n DamageType = EXPLOSION\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n WeaponSpeed = 999999\n ClipSize = 2\n ClipReloadTime = 0\n DelayBetweenShots = 600\nEnd";
    let (mut logic, source, victim) = authored_temporary_world(modules, weapons);
    logic.frame = 10;
    // A lazily initialized visual generation is not a new gameplay object.
    // Keep this regression for the unstamped source previously inserted here.
    logic.objects.get_mut(&source).unwrap().visual_object_generation = 0;
    let keys = logic.damaged_continuous_plan(source);
    assert_eq!(keys.len(), 2);
    assert_ne!(keys[0].module_source_index, keys[1].module_source_index);
    logic.execute_temporary_weapon_continuous(source);
    assert_eq!(logic.objects[&victim].health.current, 66.0);
    let object = logic.host_object(source).unwrap();
    assert_ne!(object.visual_object_generation, 0);
    for runtime in &object.temporary_weapon_runtime.damaged {
        let weapon = runtime.weapon(TemporaryWeaponRuntimeKey {
            module_source_index: runtime.module_source_index,
            role: FireWeaponWhenDamagedWeaponRole::ContinuousPristine,
        }).unwrap();
        assert_eq!(weapon.ammo_in_clip, 1, "each continuous module must fire");
        assert_eq!(weapon.last_fire_frame, 10);
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

#[test]
fn receiving_temporary_reaction_uses_body_state_after_same_current_max_health_change() {
    use crate::game_logic::body_damage_rules::{BodyDamagePolicy, BodyDamageRules};
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType as State;
    let mut logic = GameLogic::new();
    logic.admit_body_damage_baseline(BodyDamagePolicy::Receiving(
        BodyDamageRules::from_thresholds(0.8, 0.4).unwrap(),
    ));
    logic.add_player(crate::game_logic::Player::new(1, Team::USA, "Owner", true));
    let role = FireWeaponWhenDamagedWeaponRole::ReactionDamaged;
    let mut object = damaged_object(23, ready_weapon("AuthoredNoGlobalWeaponRequired", role));
    object.owner_player_id = Some(1);
    let id = logic.try_add_object(object).unwrap();
    assert_eq!(logic.objects[&id].body_damage_state, State::Damaged);
    // ActiveBody::setMaxHealth(SAME_CURRENTHEALTH) does not reclassify unless
    // current HP clips. Its consumers must read the actual Body state.
    logic
        .objects
        .get_mut(&id)
        .unwrap()
        .set_body_max_health(200.0);
    let actor = &logic.objects[&id];
    assert_eq!(actor.health.current, 80.0);
    assert_eq!(actor.body_damage_state, State::Damaged);
    assert_eq!(
        actor.classify_body_damage(actor.health.current, actor.health.maximum),
        State::ReallyDamaged
    );
    let selected = logic.damaged_reaction_plan(id, 10.0, 0);
    assert_eq!(
        selected.len(),
        1,
        "canonical DAMAGED reaction survives a maximum-only change"
    );
    assert_eq!(selected[0].role, role);
}

fn authored_temporary_world(modules: &str, weapons: &str) -> (GameLogic, ObjectId, ObjectId) {
    authored_temporary_world_with_victim_health(modules, weapons, 100)
}

fn authored_temporary_world_with_victim_health(
    modules: &str,
    weapons: &str,
    victim_health: u32,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_body_damage_baseline(
        crate::game_logic::body_damage_rules::BodyDamagePolicy::Receiving(
            crate::game_logic::body_damage_rules::BodyDamageRules::from_thresholds(0.8, 0.4).unwrap(),
        ),
    );
    world.admit_receiving_module_content(
        crate::assets::ReceivingObjectContent::from_sources([(
            "temporary-consumer.ini".into(),
            format!("Object TemporaryConsumer\n KindOf = IMMOBILE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n{modules}\nEnd\n\
                Object TemporaryVictim\n KindOf = IMMOBILE\n Body = ActiveBody BodyTag\n MaxHealth = {victim_health}\n InitialHealth = {victim_health}\n End\nEnd\n\
                Object TemporaryChild\n KindOf = IMMOBILE\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n"),
        )]).unwrap(),
    ).unwrap();
    let mut definitions = std::collections::HashMap::new();
    crate::assets::ini_template_loader::parse_weapon_definitions_ini_into(
        &mut definitions,
        weapons,
        crate::assets::ini_template_loader::WeaponDefinitionLoadType::Normal,
    );
    world.admit_weapon_content_snapshot(
        definitions.into_values().map(|rule| (rule.name.clone(), rule)).collect(),
        &[],
    ).unwrap();
    world.admit_object_creation_list_sources([
        "ObjectCreationList OCL_TemporaryConsumer\n CreateObject\n ObjectNames = TemporaryChild\n Count = 1\n End\nEnd\n".into(),
    ]).unwrap();
    for (id, team) in [(0, Team::China), (1, Team::USA)] {
        let mut player = Player::new(id, team, "TemporaryOwner", id == 0);
        player.alliance_team = id as i32;
        world.add_player(player);
    }
    let source = world.create_object_for_player("TemporaryConsumer", 0, glam::Vec3::ZERO).unwrap();
    let victim = world.create_object_for_player(
        "TemporaryVictim", 1, glam::Vec3::new(3.0, 0.0, 0.0),
    ).unwrap();
    (world, source, victim)
}

fn authored_temporary_hit(world: &mut GameLogic, source: ObjectId) {
    world.apply_owned_damage(
        source, 5.0, None,
        crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        None, &crate::game_logic::object::DamageHitContext::default(),
    ).unwrap();
}

fn temporary_children(world: &GameLogic) -> usize {
    world.host_objects().registered()
        .filter(|(_, object)| object.template_name == "TemporaryChild").count()
}

/// FireWeaponWhenDamagedBehavior.cpp:147-192 -> Weapon.cpp:2614,2617-2647:
/// the real Body callback fires the receiving template (including its FireOCL)
/// before spending that module's private clip and starting its own delay.
#[test]
fn authored_reaction_delivers_receiving_fire_ocl_and_private_clock_in_two_worlds() {
    let modules = "Behavior = FireWeaponWhenDamagedBehavior Reaction\n StartsActive = Yes\n DamageAmount = 1\n ReactionWeaponPristine = OwnedTemporary\n End";
    let weapon = |damage| format!(
        "Weapon OwnedTemporary\n PrimaryDamage = {damage}\n PrimaryDamageRadius = 10\n DamageType = EXPLOSION\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n WeaponSpeed = 999999\n ClipSize = 2\n ClipReloadTime = 0\n DelayBetweenShots = 600\n FireOCL = OCL_TemporaryConsumer\nEnd\n"
    );
    let (mut world, source, victim) = authored_temporary_world(modules, &weapon(17));
    let (mut peer, peer_source, peer_victim) = authored_temporary_world(modules, &weapon(29));
    assert_eq!((source, victim), (peer_source, peer_victim));
    world.frame = 10;
    authored_temporary_hit(&mut world, source);
    assert_eq!(world.objects[&victim].health.current, 83.0);
    assert_eq!(temporary_children(&world), 1);
    assert_eq!(peer.objects[&peer_victim].health.current, 100.0);
    assert_eq!(temporary_children(&peer), 0);
    let runtime = &world.objects[&source].temporary_weapon_runtime.damaged[0];
    let state = runtime.weapon(TemporaryWeaponRuntimeKey {
        module_source_index: runtime.module_source_index,
        role: FireWeaponWhenDamagedWeaponRole::ReactionPristine,
    }).unwrap();
    assert_eq!(state.ammo_in_clip, 1);
    assert_eq!(state.last_fire_frame, 10);
    assert_eq!(state.when_we_can_fire_again, 28);
    world.frame = 27;
    authored_temporary_hit(&mut world, source);
    assert_eq!(temporary_children(&world), 1);
    world.frame = 28;
    authored_temporary_hit(&mut world, source);
    assert_eq!(temporary_children(&world), 2);
    assert_eq!(world.objects[&victim].health.current, 66.0);
    peer.frame = 100;
    authored_temporary_hit(&mut peer, peer_source);
    assert_eq!(peer.objects[&peer_victim].health.current, 71.0);
    assert_eq!(temporary_children(&peer), 1);
    assert_eq!(world.objects[&victim].health.current, 66.0);
}

/// Each update reads BodyDamageType at its own callback, not at the start of
/// an all-module batch (FireWeaponWhenDamagedBehavior.cpp:208-237). The earlier
/// SELF shot changes the state seen by the later module.
#[test]
fn authored_continuous_callbacks_reselect_role_after_previous_self_damage() {
    let modules = "Behavior = FireWeaponWhenDamagedBehavior First\n StartsActive = Yes\n ContinuousWeaponPristine = SelfTemporary\n End\n\
        Behavior = FireWeaponWhenDamagedBehavior Second\n StartsActive = Yes\n ContinuousWeaponPristine = WrongTemporary\n ContinuousWeaponReallyDamaged = RightTemporary\n End";
    let weapons = [
        // Weapon.cpp:1330 allows self, then 1360-1372 still requires ALLIES.
        ("SelfTemporary", 70, "SELF ALLIES"),
        ("WrongTemporary", 3, "ENEMIES"),
        ("RightTemporary", 17, "ENEMIES"),
    ].into_iter().map(|(name, damage, affects)| format!(
        "Weapon {name}\n PrimaryDamage = {damage}\n PrimaryDamageRadius = 10\n DamageType = UNRESISTABLE\n RadiusDamageAffects = {affects}\n ProjectileObject = NONE\n WeaponSpeed = 999999\n ClipSize = 2\n ClipReloadTime = 0\n DelayBetweenShots = 600\nEnd\n"
    )).collect::<String>();
    let (mut world, source, victim) = authored_temporary_world(modules, &weapons);
    let (peer, peer_source, peer_victim) = authored_temporary_world(modules, &weapons);
    world.frame = 10;
    world.execute_temporary_weapon_continuous(source);
    assert_eq!(world.objects[&source].health.current, 30.0);
    assert_eq!(world.objects[&victim].health.current, 83.0);
    let second = &world.objects[&source].temporary_weapon_runtime.damaged[1];
    for (role, ammo) in [
        (FireWeaponWhenDamagedWeaponRole::ContinuousPristine, 2),
        (FireWeaponWhenDamagedWeaponRole::ContinuousReallyDamaged, 1),
    ] {
        assert_eq!(second.weapon(TemporaryWeaponRuntimeKey {
            module_source_index: second.module_source_index, role,
        }).unwrap().ammo_in_clip, ammo);
    }
    assert_eq!(peer.objects[&peer_source].health.current, 100.0);
    assert_eq!(peer.objects[&peer_victim].health.current, 100.0);
}

/// Unlike onDie's explicit conflict check, onDamage only checks the latched
/// UpgradeMux state (FireWeaponWhenDamagedBehavior.cpp:149, .h:137;
/// UpgradeModule.cpp:105-146). An already active module remains active.
#[test]
fn authored_active_damage_module_keeps_firing_after_conflicting_upgrade() {
    let modules = "Behavior = FireWeaponWhenDamagedBehavior Reaction\n StartsActive = Yes\n ConflictsWith = Upgrade_Conflict\n DamageAmount = 1\n ReactionWeaponPristine = OwnedTemporary\n End";
    let weapons = "Weapon OwnedTemporary\n PrimaryDamage = 17\n PrimaryDamageRadius = 10\n DamageType = EXPLOSION\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n WeaponSpeed = 999999\n ClipSize = 2\n ClipReloadTime = 0\n DelayBetweenShots = 600\nEnd";
    let (mut world, source, victim) = authored_temporary_world(modules, weapons);
    world.objects.get_mut(&source).unwrap().apply_upgrade_tag("Upgrade_Conflict");
    authored_temporary_hit(&mut world, source);
    assert_eq!(world.objects[&victim].health.current, 83.0);
}

/// Weapon.cpp:2739-2742 returns PRE_ATTACK without storing it. Otherwise a
/// later query between the preattack and reload deadlines invents PRE_ATTACK.
#[test]
fn temporary_preattack_query_preserves_underlying_reload_status() {
    let mut state = ready_weapon("Local", FireWeaponWhenDamagedWeaponRole::ReactionPristine);
    state.status = TemporaryWeaponStatus::BetweenFiringShots;
    state.when_pre_attack_finished = 5;
    state.when_we_can_fire_again = 10;
    assert_eq!(promote_temporary_weapon_status(&mut state, 4), TemporaryWeaponStatus::PreAttack);
    assert_eq!(state.status, TemporaryWeaponStatus::BetweenFiringShots);
    assert_eq!(promote_temporary_weapon_status(&mut state, 5), TemporaryWeaponStatus::BetweenFiringShots);
    assert_eq!(promote_temporary_weapon_status(&mut state, 10), TemporaryWeaponStatus::ReadyToFire);
}

/// Object::onDamage reenters synchronously through a SELF weapon. The nested
/// callback spends the later module's clip before the outer walk reaches it;
/// Weapon.cpp:2614 precedes the firing module's mutation at 2617.
#[test]
fn authored_reaction_reentry_uses_current_body_and_private_module_deadline() {
    let modules = "Behavior = FireWeaponWhenDamagedBehavior First\n StartsActive = Yes\n DamageAmount = 1\n ReactionWeaponPristine = SelfTemporary\n End\n\
        Behavior = FireWeaponWhenDamagedBehavior Second\n StartsActive = Yes\n DamageAmount = 1\n ReactionWeaponReallyDamaged = RightTemporary\n End";
    let weapons = [
        // SELF removes the self veto; ALLIES admits the source relationship.
        ("SelfTemporary", 70, "SELF ALLIES"),
        ("RightTemporary", 17, "ENEMIES"),
    ].into_iter().map(|(name, damage, affects)| format!(
        "Weapon {name}\n PrimaryDamage = {damage}\n PrimaryDamageRadius = 10\n DamageType = UNRESISTABLE\n RadiusDamageAffects = {affects}\n ProjectileObject = NONE\n WeaponSpeed = 999999\n ClipSize = 2\n ClipReloadTime = 0\n DelayBetweenShots = 600\nEnd\n"
    )).collect::<String>();
    let (mut world, source, victim) = authored_temporary_world(modules, &weapons);
    world.frame = 10;
    authored_temporary_hit(&mut world, source);
    assert_eq!(world.objects[&source].health.current, 25.0);
    assert_eq!(world.objects[&victim].health.current, 83.0);
    for (index, role) in [
        (0, FireWeaponWhenDamagedWeaponRole::ReactionPristine),
        (1, FireWeaponWhenDamagedWeaponRole::ReactionReallyDamaged),
    ] {
        let runtime = &world.objects[&source].temporary_weapon_runtime.damaged[index];
        let weapon = runtime.weapon(TemporaryWeaponRuntimeKey {
            module_source_index: runtime.module_source_index, role,
        }).unwrap();
        assert_eq!(weapon.ammo_in_clip, 1);
        assert_eq!(weapon.last_fire_frame, 10);
        assert_eq!(weapon.when_we_can_fire_again, 28);
    }
}

/// C++ FireWeaponWhenDeadBehavior.cpp:65-88 consults the controlling player's
/// upgrades and adds a live conflict veto which the damage callback lacks.
#[test]
fn authored_temporary_player_upgrades_are_receiving_owned_and_death_conflicts_stay_live() {
    let modules = "Behavior = FireWeaponWhenDamagedBehavior Reaction\n StartsActive = No\n TriggeredBy = Upgrade_Activate\n ConflictsWith = Upgrade_Conflict\n DamageAmount = 1\n ReactionWeaponPristine = OwnedTemporary\n End\n\
        Behavior = FireWeaponWhenDeadBehavior Death\n StartsActive = No\n TriggeredBy = Upgrade_Activate\n ConflictsWith = Upgrade_Conflict\n DeathWeapon = OwnedTemporary\n End";
    let weapons = "Weapon OwnedTemporary\n PrimaryDamage = 17\n PrimaryDamageRadius = 10\n DamageType = EXPLOSION\n RadiusDamageAffects = ENEMIES\n ProjectileObject = NONE\n WeaponSpeed = 999999\n ClipSize = 2\n ClipReloadTime = 0\n DelayBetweenShots = 0\n FireOCL = OCL_TemporaryConsumer\nEnd";
    let (mut world, source, victim) = authored_temporary_world(modules, weapons);
    let (mut peer, peer_source, peer_victim) = authored_temporary_world(modules, weapons);
    world.players.get_mut(&0).unwrap().completed_upgrades.insert("Upgrade_Activate".into());
    authored_temporary_hit(&mut world, source);
    authored_temporary_hit(&mut peer, peer_source);
    assert_eq!(world.objects[&victim].health.current, 83.0);
    assert_eq!(peer.objects[&peer_victim].health.current, 100.0);
    world.players.get_mut(&0).unwrap().completed_upgrades.insert("Upgrade_Conflict".into());
    authored_temporary_hit(&mut world, source);
    assert_eq!(world.objects[&victim].health.current, 66.0);
    world.apply_owned_kill(
        source, crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
    ).unwrap();
    assert_eq!(world.objects[&victim].health.current, 66.0);
    assert_eq!(temporary_children(&world), 2);
    peer.players.get_mut(&0).unwrap().completed_upgrades.insert("Upgrade_Activate".into());
    peer.apply_owned_kill(
        peer_source, crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
    ).unwrap();
    assert_eq!(peer.objects[&peer_victim].health.current, 83.0);
    assert_eq!(temporary_children(&peer), 1);
}
