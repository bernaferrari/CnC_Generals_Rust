//! Behavior suite extracted from `projectiles_air`.
use super::*;

#[test]
fn unpause_special_power_upgrade_enables_capture() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{CapturePowerKind, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.capture_power = CapturePowerKind::Ranger;
    t.capture_start_ability_range = Some(5.0);
    t.capture_unpack_time_ms = Some(3_000);
    t.capture_preparation_time_ms = Some(20_000);
    t.capture_pack_time_ms = Some(2_000);
    t.capture_starts_paused = true;
    t.capture_upgrade_trigger = Some("Upgrade_InfantryCaptureBuilding".to_string());
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    assert!(
        !logic.is_special_power_ready_for(id, &SpecialPowerType::RangerCaptureBuilding),
        "capture must start paused"
    );
    logic.apply_upgrade_to_object(id, "Upgrade_InfantryCaptureBuilding");
    let obj = logic.objects.get(&id).unwrap();
    assert!(!obj.is_special_power_countdown_paused(&SpecialPowerType::RangerCaptureBuilding));
    let rem = obj
        .special_power_cooldowns
        .get(&SpecialPowerType::RangerCaptureBuilding)
        .copied()
        .unwrap_or(0.0);
    assert!(
        (rem - 15.0).abs() < 0.01,
        "C++ unpause keeps ctor ReloadTime (15000ms), rem={rem}"
    );
}

#[test]
fn helix_nuke_unpause_hits_helix_nuke_bomb_not_nuclear_missile() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Nuke_ChinaVehicleHelix");
    t.set_health(300.0);
    t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("Nuke_ChinaVehicleHelix".into(), t);
    let id = logic
        .create_object("Nuke_ChinaVehicleHelix", Team::China, glam::Vec3::ZERO)
        .unwrap();
    let obj = logic.objects.get(&id).unwrap();
    assert_eq!(
        obj.special_power_paused
            .get(&SpecialPowerType::HelixNukeBomb)
            .copied()
            .unwrap_or(0),
        1,
        "ctor StartsPaused must pause HelixNukeBomb once"
    );
    assert!(
        !obj.is_special_power_countdown_paused(&SpecialPowerType::NuclearMissile),
        "unpause path must not park NuclearMissile on a Helix"
    );
    assert!(
        !obj.is_special_power_countdown_paused(&SpecialPowerType::HelixNapalmBomb),
        "Nuke Helix must not pause the napalm bomb a Nuke general cannot unpause"
    );
    let rem = obj
        .special_power_cooldowns
        .get(&SpecialPowerType::HelixNukeBomb)
        .copied()
        .unwrap_or(0.0);
    assert!(
        (rem - 10.0).abs() < 0.01,
        "ctor startPowerRecharge must arm 10000ms ReloadTime, rem={rem}"
    );
    logic.apply_upgrade_to_object(id, "Nuke_Upgrade_HelixNukeBomb");
    let obj = logic.objects.get(&id).unwrap();
    assert!(!obj.is_special_power_countdown_paused(&SpecialPowerType::HelixNukeBomb));
    let rem = obj
        .special_power_cooldowns
        .get(&SpecialPowerType::HelixNukeBomb)
        .copied()
        .unwrap_or(0.0);
    assert!(
        (rem - 10.0).abs() < 0.01,
        "unpause must keep remaining ReloadTime, rem={rem}"
    );
}

#[test]
fn starts_paused_module_and_capture_flag_do_not_double_pause() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::{
        CapturePowerKind, KindOf, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team,
        ThingTemplate,
    };
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.set_health(100.0);
    t.add_kind_of(KindOf::Infantry);
    t.capture_power = CapturePowerKind::Ranger;
    t.capture_starts_paused = true;
    t.capture_upgrade_trigger = Some("Upgrade_InfantryCaptureBuilding".to_string());
    t.special_power_modules.push(SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_Capture".into()),
        module_kind: SpecialPowerModuleKind::SpecialAbility,
        special_power_template: "SpecialAbilityRangerCaptureBuilding".into(),
        special_power_template_id: 1,
        command_power: Some(SpecialPowerType::RangerCaptureBuilding),
        reload_time_frames: 450,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: true,
        starts_paused: true,
        scripted_special_power_only: false,
    });
    logic.templates.insert("AmericaInfantryRanger".into(), t);
    let id = logic
        .create_object("AmericaInfantryRanger", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let obj = logic.objects.get(&id).unwrap();
    assert_eq!(
        obj.special_power_paused
            .get(&SpecialPowerType::RangerCaptureBuilding)
            .copied()
            .unwrap_or(0),
        1,
        "ctor + SpecialPowerCreate must not stack StartsPaused"
    );
    logic.apply_upgrade_to_object(id, "Upgrade_InfantryCaptureBuilding");
    let obj = logic.objects.get(&id).unwrap();
    assert!(!obj.is_special_power_countdown_paused(&SpecialPowerType::RangerCaptureBuilding));
}

#[test]
fn defector_special_power_defects_enemy() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("AmericaCommandCenter");
    template.set_health(1000.0);
    template.add_kind_of(KindOf::Structure);
    logic
        .templates
        .insert("AmericaCommandCenter".into(), template);
    let mut tank = ThingTemplate::new("TestTank");
    tank.set_health(200.0);
    tank.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("TestTank".into(), tank);

    let caster = logic
        .create_object("AmericaCommandCenter", Team::USA, glam::Vec3::ZERO)
        .unwrap();
    let victim = logic
        .create_object("TestTank", Team::GLA, glam::Vec3::new(50.0, 0.0, 0.0))
        .unwrap();
    assert!(logic.activate_defector(caster, victim));
    let v = logic.objects.get(&victim).unwrap();
    assert_eq!(v.team, Team::USA);
    assert!(v.is_undetected_defector());
    assert!(logic.honesty_defector_ok());
}

#[test]
fn model_condition_upgrade_sets_bit_on_object() {
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("Demo_Technical");
    t.set_health(200.0);
    t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("Demo_Technical".into(), t);
    let id = logic
        .create_object("Demo_Technical", Team::GLA, glam::Vec3::ZERO)
        .unwrap();
    let before = logic.objects.get(&id).unwrap().model_condition_bits;
    logic.apply_upgrade_to_object(id, "Upgrade_DemoArmor");
    let after = logic.objects.get(&id).unwrap().model_condition_bits;
    assert_ne!(
        before, after,
        "ModelConditionUpgrade must set ConditionFlag bit"
    );
}

#[test]
fn baikonur_launch_door_and_detonation() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_baikonur_launch::BAIKONUR_DETONATION_OBJECT;
    use crate::game_logic::host_enum_table_residual::door_1_opening_model_bit;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut tower = ThingTemplate::new("BaikonurLaunchTower");
    tower.set_health(5000.0);
    tower.add_kind_of(KindOf::Structure);
    logic.templates.insert("BaikonurLaunchTower".into(), tower);

    let tower_id = logic
        .create_object(
            "BaikonurLaunchTower",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    // Make special power ready residual.
    if let Some(o) = logic.objects.get_mut(&tower_id) {
        o.set_special_power_ready(true);
    }

    assert!(logic.activate_baikonur_launch_door(tower_id));
    let bit = door_1_opening_model_bit();
    let obj = logic.objects.get(&tower_id).unwrap();
    assert_ne!(obj.model_condition_bits & (1u128 << bit), 0);
    assert!(logic.honesty_baikonur_ok());

    let loc = glam::Vec3::new(100.0, 0.0, 50.0);
    assert!(logic.activate_baikonur_detonation(tower_id, loc));
    assert!(logic.baikonur_launches().honesty_detonation_ok());
    assert!(
        logic
            .objects
            .values()
            .any(|o| o.template_name == BAIKONUR_DETONATION_OBJECT)
    );
    assert!(
        logic.special_power_strikes.neutron_slow_death_field_count() >= 1
            || logic
                .special_power_strikes
                .neutron_slow_death_spawned_total()
                >= 1
    );

    // Command path residual.
    logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::BaikonurRocket,
            target: PowerTarget::Location(glam::Vec3::new(200.0, 0.0, 0.0)),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tower_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    // execute via process may need command_executor - direct activate already tested.
    let _ = SpecialPowerType::BaikonurRocket;
}

#[test]
fn spectre_orbit_spawns_howitzer_shell_objects() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::{
        SPECTRE_HOWITZER_FIRE_SOUND, SPECTRE_HOWITZER_FOLLOW_LAG_FRAMES,
        SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES, SPECTRE_HOWITZER_SHELL_OBJECT,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut sc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    sc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), sc);
    let caster = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let field_id = logic.special_power_strikes.spawn_orbit_field(
        caster,
        Team::USA,
        Vec3::new(200.0, 0.0, 200.0),
        logic.frame,
        1,
    );
    // Force howitzer stream due this frame.
    if let Some(f) = logic
        .special_power_strikes
        .orbit_fields_mut()
        .iter_mut()
        .find(|f| f.id == field_id)
    {
        f.next_tick_frame = logic.frame;
    }
    // C++ SpectreGunshipUpdate.cpp:609-623: the howitzer only fires once the
    // gattling strafe wind settles for > HowitzerFollowLag (12f); wind the aim
    // the same per-frame way world_tick does before the due tick.
    for _ in 0..SPECTRE_HOWITZER_FOLLOW_LAG_FRAMES.saturating_add(1) {
        logic.special_power_strikes.advance_orbit_strafe(logic.frame);
    }
    logic
        .special_power_strikes
        .record_orbit_tick_complete(field_id, 0.0, 0, 0, logic.frame);
    logic.spawn_spectre_howitzer_shell_objects_for_new_spawns();
    assert!(
        logic.queued_audio_events.iter().any(|e| {
            e.event_type == SPECTRE_HOWITZER_FIRE_SOUND && e.object_id == Some(caster) && !e.stop
        }),
        "howitzer volley must queue HowitzerFireSound on the gunship: {:?}",
        logic.queued_audio_events
    );
    assert!(
        logic
            .special_power_strikes
            .honesty_howitzer_shell_object_spawn_ok()
    );
    assert!(logic.special_power_strikes.howitzer_shell_objects_spawned() >= 1);
    let shell = logic
        .host_objects()
        .values()
        .find(|o| o.spectre_howitzer_shell)
        .expect("SpectreHowitzerShell");
    assert_eq!(shell.template_name, SPECTRE_HOWITZER_SHELL_OBJECT);
    let sid = shell.id;
    logic.frame = logic
        .frame
        .saturating_add(SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES + 2);
    logic.update_spectre_howitzer_shell_objects();
    assert!(
        logic
            .host_object(sid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
}

#[test]
fn scud_storm_anthrax_beta_spawns_poison_field_upgraded_large() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::{
        SCUD_POISON_UPGRADED_OBJECT_NAME, SCUD_STORM_POISON_DURATION_FRAMES, ScudStormAnthraxTier,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut scud = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    scud.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), scud);
    let caster = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let tid = logic
        .special_power_strikes
        .spawn_scud_poison_field_with_tier(
            caster,
            Team::GLA,
            Vec3::new(160.0, 0.0, 160.0),
            logic.frame,
            1,
            ScudStormAnthraxTier::AnthraxBeta,
        );
    logic.spawn_anthrax_toxin_field_objects_for_new_fields();
    let field = logic
        .special_power_strikes
        .toxin_fields()
        .iter()
        .find(|f| f.id == tid)
        .expect("toxin field");
    assert_eq!(field.object_template, SCUD_POISON_UPGRADED_OBJECT_NAME);
    assert!(ScudStormAnthraxTier::AnthraxBeta.is_upgraded());
    let obj = logic
        .host_objects()
        .values()
        .find(|o| o.anthrax_toxin_field && o.template_name == SCUD_POISON_UPGRADED_OBJECT_NAME)
        .expect("PoisonFieldUpgradedLarge");
    assert!((obj.health.maximum - 120.0).abs() < 0.01);
    let oid = obj.id;
    logic.frame = SCUD_STORM_POISON_DURATION_FRAMES + 5;
    logic.update_anthrax_toxin_field_objects();
    assert!(
        logic
            .host_object(oid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
}

#[test]
fn scud_storm_spawns_poison_field_large_object() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::{
        SCUD_POISON_OBJECT_NAME, SCUD_STORM_POISON_DURATION_FRAMES,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut scud = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    scud.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), scud);
    let caster = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let tid = logic.special_power_strikes.spawn_scud_poison_field(
        caster,
        Team::GLA,
        Vec3::new(150.0, 0.0, 150.0),
        logic.frame,
        1,
    );
    logic.spawn_anthrax_toxin_field_objects_for_new_fields();
    assert!(logic.special_power_strikes.honesty_toxin_object_spawn_ok());
    let field = logic
        .special_power_strikes
        .toxin_fields()
        .iter()
        .find(|f| f.id == tid)
        .expect("toxin field");
    assert_eq!(field.object_template, SCUD_POISON_OBJECT_NAME);
    let obj = logic
        .host_objects()
        .values()
        .find(|o| o.anthrax_toxin_field && o.template_name == SCUD_POISON_OBJECT_NAME)
        .expect("PoisonFieldLarge object");
    assert_eq!(field.object_id, Some(obj.id));
    let oid = obj.id;
    logic.frame = SCUD_STORM_POISON_DURATION_FRAMES + 5;
    logic.update_anthrax_toxin_field_objects();
    assert!(
        logic
            .host_object(oid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
}

#[test]
fn anthrax_bomb_spawns_toxin_field_object() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::{
        ANTHRAX_TOXIN_DURATION_FRAMES, ANTHRAX_TOXIN_OBJECT_NAME,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut scud = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    scud.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), scud);
    let caster = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let tid = logic.special_power_strikes.spawn_toxin_field(
        caster,
        Team::GLA,
        Vec3::new(120.0, 0.0, 120.0),
        logic.frame,
        1,
    );
    logic.spawn_anthrax_toxin_field_objects_for_new_fields();
    assert!(logic.special_power_strikes.honesty_toxin_object_spawn_ok());
    assert!(logic.special_power_strikes.toxin_objects_spawned() >= 1);
    let obj = logic
        .host_objects()
        .values()
        .find(|o| o.anthrax_toxin_field)
        .expect("toxin field object");
    assert_eq!(obj.template_name, ANTHRAX_TOXIN_OBJECT_NAME);
    let oid = obj.id;
    let bound = logic
        .special_power_strikes
        .toxin_fields()
        .iter()
        .find(|f| f.id == tid)
        .and_then(|f| f.object_id);
    assert_eq!(bound, Some(oid));
    logic.frame = ANTHRAX_TOXIN_DURATION_FRAMES + 5;
    logic.update_anthrax_toxin_field_objects();
    assert!(
        logic
            .host_object(oid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
}

#[test]
fn nuclear_missile_spawns_radiation_field_object() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::{
        NUKE_RADIATION_DURATION_FRAMES, NUKE_RADIATION_OBJECT_NAME,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut silo = crate::game_logic::ThingTemplate::new("ChinaNuclearMissileLauncher");
    silo.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("ChinaNuclearMissileLauncher".into(), silo);
    let caster = logic
        .create_object(
            "ChinaNuclearMissileLauncher",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let rid = logic.special_power_strikes.spawn_radiation_field(
        caster,
        Team::China,
        Vec3::new(100.0, 0.0, 100.0),
        logic.frame,
        1,
    );
    logic.spawn_nuke_radiation_field_objects_for_new_fields();
    assert!(
        logic
            .special_power_strikes
            .honesty_radiation_object_spawn_ok()
    );
    assert!(logic.special_power_strikes.radiation_objects_spawned() >= 1);
    let obj = logic
        .host_objects()
        .values()
        .find(|o| o.nuke_radiation_field)
        .expect("radiation field object");
    assert_eq!(obj.template_name, NUKE_RADIATION_OBJECT_NAME);
    let oid = obj.id;
    let bound = logic
        .special_power_strikes
        .radiation_fields()
        .iter()
        .find(|f| f.id == rid)
        .and_then(|f| f.object_id);
    assert_eq!(bound, Some(oid));
    logic.frame = NUKE_RADIATION_DURATION_FRAMES + 5;
    logic.update_nuke_radiation_field_objects();
    assert!(
        logic
            .host_object(oid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
}

#[test]
fn radiation_field_object_carries_cleanup_hazard_kindof_and_hazardous_armor() {
    use crate::game_logic::KindOf;
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_armor_residual::apply_residual_armor;
    use crate::game_logic::special_power_strikes::NUKE_RADIATION_OBJECT_NAME;
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut silo = crate::game_logic::ThingTemplate::new("ChinaNuclearMissileLauncher");
    silo.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("ChinaNuclearMissileLauncher".into(), silo);
    let caster = logic
        .create_object(
            "ChinaNuclearMissileLauncher",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    logic.special_power_strikes.spawn_radiation_field(
        caster,
        Team::China,
        Vec3::new(100.0, 0.0, 100.0),
        logic.frame,
        1,
    );
    logic.spawn_nuke_radiation_field_objects_for_new_fields();
    let obj = logic
        .host_objects()
        .values()
        .find(|o| o.nuke_radiation_field)
        .expect("radiation field object");
    // C++ NukeRadiationFieldWeapon KindOf residual: the ambulance
    // CleanupHazardUpdate partition scan (CleanupHazardUpdate.cpp:250) only
    // targets KINDOF_CLEANUP_HAZARD objects; INERT/NO_COLLIDE round out retail.
    for kind in [
        KindOf::Immobile,
        KindOf::CleanupHazard,
        KindOf::Inert,
        KindOf::NoCollide,
    ] {
        assert!(obj.is_kind_of(kind), "radiation field missing {kind:?}");
    }
    // C++ HazardousMaterialArmor: HAZARD_CLEANUP 100%, FLAME 0% (flame cannot
    // clean). Pre-fix fallback was StructureArmor (HAZARD_CLEANUP 0%, FLAME 50%).
    assert_eq!(apply_residual_armor(obj, DamageType::Flame, 100.0), 0.0);
    let cleanup = apply_residual_armor(obj, DamageType::HazardCleanup, 100.0);
    assert!((cleanup - 100.0).abs() < 0.01, "HAZARD_CLEANUP must be 100%, got {cleanup}");
    let tmpl = logic
        .templates
        .get(NUKE_RADIATION_OBJECT_NAME)
        .expect("synthesized radiation template");
    assert!(tmpl
        .armor_sets
        .iter()
        .any(|s| s.armor.as_deref() == Some("HazardousMaterialArmor")));
}

#[test]
fn toxin_field_object_carries_cleanup_hazard_kindof_and_hazardous_armor() {
    use crate::game_logic::KindOf;
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_armor_residual::apply_residual_armor;
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut scud = crate::game_logic::ThingTemplate::new("GLAScudStorm");
    scud.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), scud);
    let caster = logic
        .create_object("GLAScudStorm", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    logic.special_power_strikes.spawn_toxin_field(
        caster,
        Team::GLA,
        Vec3::new(120.0, 0.0, 120.0),
        logic.frame,
        1,
    );
    logic.spawn_anthrax_toxin_field_objects_for_new_fields();
    let obj = logic
        .host_objects()
        .values()
        .find(|o| o.anthrax_toxin_field)
        .expect("toxin field object");
    for kind in [
        KindOf::Immobile,
        KindOf::CleanupHazard,
        KindOf::Inert,
        KindOf::NoCollide,
    ] {
        assert!(obj.is_kind_of(kind), "toxin field missing {kind:?}");
    }
    // C++ HazardousMaterialArmor: flame cannot clean poison fields either.
    assert_eq!(apply_residual_armor(obj, DamageType::Flame, 100.0), 0.0);
    let cleanup = apply_residual_armor(obj, DamageType::HazardCleanup, 100.0);
    assert!((cleanup - 100.0).abs() < 0.01, "HAZARD_CLEANUP must be 100%, got {cleanup}");
}

#[test]
fn point_defense_intercept_spawns_laser_beam_object() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_point_defense::{
        PDL_LASER_BEAM_DEFAULT, PDL_LASER_BEAM_LIFETIME_FRAMES,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut paladin = crate::game_logic::ThingTemplate::new("AmericaTankPaladin");
    paladin.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("AmericaTankPaladin".into(), paladin);
    let mut missile = crate::game_logic::ThingTemplate::new("ScudStormMissile");
    missile.add_kind_of(KindOf::Projectile).set_health(50.0);
    logic.templates.insert("ScudStormMissile".into(), missile);
    let carrier = logic
        .create_object("AmericaTankPaladin", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let threat = logic
        .create_object("ScudStormMissile", Team::GLA, Vec3::new(30.0, 20.0, 0.0))
        .unwrap();
    // Force ready and run intercept.
    logic.point_defense_next_ready_frame.insert(carrier, 0);
    logic.frame = 1;
    logic.update_point_defense_intercept();
    assert!(
        logic.point_defense_residual_intercepts >= 1,
        "PDL must intercept residual missile"
    );
    assert!(logic.honesty_point_defense_laser_beam_ok());
    assert!(logic.point_defense_laser_beams_spawned >= 1);
    let beam = logic
        .host_objects()
        .values()
        .find(|o| o.point_defense_laser_beam)
        .expect("PDL beam object");
    assert_eq!(beam.template_name, PDL_LASER_BEAM_DEFAULT);
    let bid = beam.id;
    assert!(logic.point_defense_laser_beams_spawned >= 1);
    logic.frame = logic
        .frame
        .saturating_add(PDL_LASER_BEAM_LIFETIME_FRAMES + 2);
    logic.update_point_defense_laser_beam_objects();
    assert!(
        logic
            .host_object(bid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
}

#[test]
fn particle_cannon_spawns_connector_laser_objects() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::{
        PARTICLE_CONNECTOR_INTENSE_LASER, PARTICLE_CONNECTOR_MEDIUM_LASER,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut puc = crate::game_logic::ThingTemplate::new("AmericaParticleUplinkCannon");
    puc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaParticleUplinkCannon".into(), puc);
    let caster = logic
        .create_object(
            "AmericaParticleUplinkCannon",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .unwrap();
    let bid = logic.special_power_strikes.spawn_beam_field(
        caster,
        Team::USA,
        Vec3::new(400.0, 0.0, 400.0),
        logic.frame,
        1,
    );
    logic.spawn_particle_connector_laser_objects_for_new_beams();
    assert!(
        logic
            .special_power_strikes
            .honesty_connector_object_spawn_ok()
    );
    assert!(logic.special_power_strikes.connector_objects_spawned() >= 2);
    let names: Vec<_> = logic
        .host_objects()
        .values()
        .filter(|o| o.particle_connector_laser)
        .map(|o| o.template_name.clone())
        .collect();
    assert!(names.iter().any(|n| n == PARTICLE_CONNECTOR_MEDIUM_LASER));
    assert!(names.iter().any(|n| n == PARTICLE_CONNECTOR_INTENSE_LASER));
    let field = logic
        .special_power_strikes
        .beam_fields()
        .iter()
        .find(|f| f.id == bid)
        .expect("beam");
    assert_eq!(field.connector_object_ids.len(), 2);
    let ids = field.connector_object_ids.clone();
    let exp = field.expires_frame;
    logic.frame = exp + 2;
    logic.update_particle_connector_laser_objects();
    for id in ids {
        assert!(
            logic
                .host_object(id)
                .map(|o| !o.is_alive() || o.status.destroyed)
                .unwrap_or(true)
        );
    }
}

#[test]
fn particle_cannon_spawns_orbital_laser_object() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::PARTICLE_ORBITAL_LASER_NAME;
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut puc = crate::game_logic::ThingTemplate::new("AmericaParticleUplinkCannon");
    puc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaParticleUplinkCannon".into(), puc);
    let caster = logic
        .create_object(
            "AmericaParticleUplinkCannon",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    let bid = logic.special_power_strikes.spawn_beam_field(
        caster,
        Team::USA,
        Vec3::new(300.0, 0.0, 300.0),
        logic.frame,
        1,
    );
    logic.spawn_particle_orbital_laser_objects_for_new_beams();
    assert!(logic.special_power_strikes.honesty_beam_object_spawn_ok());
    assert!(logic.special_power_strikes.beam_objects_spawned() >= 1);
    let laser = logic
        .host_objects()
        .values()
        .find(|o| o.particle_orbital_laser)
        .expect("OrbitalLaser");
    assert_eq!(laser.template_name, PARTICLE_ORBITAL_LASER_NAME);
    assert!((laser.get_position().y - 500.0).abs() < 0.01);
    let lid = laser.id;
    let bound = logic
        .special_power_strikes
        .beam_fields()
        .iter()
        .find(|f| f.id == bid)
        .and_then(|f| f.object_id);
    assert_eq!(bound, Some(lid));
    let exp = logic
        .special_power_strikes
        .beam_fields()
        .iter()
        .find(|f| f.id == bid)
        .map(|f| f.expires_frame)
        .unwrap();
    logic.frame = exp + 2;
    logic.update_particle_orbital_laser_objects();
    assert!(
        logic
            .host_object(lid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
}

#[test]
fn particle_uplink_spawns_trail_remnant_objects() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::{
        PARTICLE_REMNANT_DURATION_FRAMES, PARTICLE_REMNANT_OBJECT_NAME,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut puc = crate::game_logic::ThingTemplate::new("AmericaParticleUplinkCannon");
    puc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaParticleUplinkCannon".into(), puc);
    let caster = logic
        .create_object(
            "AmericaParticleUplinkCannon",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    // Spawn remnant field residual directly via strike registry.
    let rid = logic.special_power_strikes.spawn_remnant_field(
        caster,
        Team::USA,
        Vec3::new(50.0, 0.0, 50.0),
        logic.frame,
        1,
        1,
    );
    // Manually mark as spawned this frame (spawn_remnant_field already pushes).
    logic.spawn_particle_trail_remnant_objects_for_new_fields();
    assert!(
        logic
            .special_power_strikes
            .honesty_remnant_object_spawn_ok()
    );
    assert!(logic.special_power_strikes.remnant_objects_spawned() >= 1);
    let obj = logic
        .host_objects()
        .values()
        .find(|o| o.particle_trail_remnant)
        .expect("trail remnant object");
    assert_eq!(obj.template_name, PARTICLE_REMNANT_OBJECT_NAME);
    let oid = obj.id;
    let bound = logic
        .special_power_strikes
        .remnant_fields()
        .iter()
        .find(|f| f.id == rid)
        .and_then(|f| f.object_id);
    assert_eq!(bound, Some(oid));
    logic.frame = PARTICLE_REMNANT_DURATION_FRAMES + 5;
    logic.update_particle_trail_remnant_objects();
    assert!(
        logic
            .host_object(oid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
}

#[test]
fn particle_cannon_emp_mid_beam_starts_decay_and_stops_killing() {
    use crate::game_logic::KindOf;
    use crate::game_logic::special_power_strikes::{
        PARTICLE_BEAM_DURATION_FRAMES, PARTICLE_BEAM_TOTAL_PULSES, PARTICLE_WIDTH_GROW_FRAMES,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut puc = crate::game_logic::ThingTemplate::new("AmericaParticleUplinkCannon");
    puc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic
        .templates
        .insert("AmericaParticleUplinkCannon".into(), puc);
    let caster = logic
        .create_object(
            "AmericaParticleUplinkCannon",
            Team::USA,
            Vec3::new(-400.0, 0.0, 0.0),
        )
        .unwrap();
    let victim = logic
        .create_object("TestTank", Team::GLA, Vec3::ZERO)
        .unwrap();
    let spawn = logic.frame;
    let bid = logic
        .special_power_strikes
        .spawn_beam_field(caster, Team::USA, Vec3::ZERO, spawn, 1);

    let mut hit = false;
    for _ in 0..PARTICLE_BEAM_DURATION_FRAMES {
        logic.update_particle_beam_fields();
        let health = logic
            .host_object(victim)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        if health < 250.0 {
            hit = true;
            break;
        }
        logic.frame = logic.frame.saturating_add(1);
    }
    assert!(hit, "beam must start damaging the tank before EMP");
    let health_at_emp = logic
        .host_object(victim)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(health_at_emp > 0.0, "tank must still be alive at EMP");
    let pulses_at_emp = logic
        .special_power_strikes
        .beam_fields()
        .iter()
        .find(|f| f.id == bid)
        .map(|f| f.pulses_made)
        .unwrap_or(0);
    assert!(pulses_at_emp < PARTICLE_BEAM_TOTAL_PULSES);

    let abort_frame = logic.frame;
    if let Some(puc_obj) = logic.objects.get_mut(&caster) {
        puc_obj.apply_disabled_emp(abort_frame.saturating_add(300));
    }

    for _ in 0..PARTICLE_BEAM_DURATION_FRAMES {
        logic.frame = logic.frame.saturating_add(1);
        logic.update_particle_beam_fields();
    }

    let field = logic
        .special_power_strikes
        .beam_fields()
        .iter()
        .find(|f| f.id == bid);
    if let Some(f) = field {
        assert_eq!(
            f.start_decay_frame, abort_frame,
            "C++ m_startDecayFrame = now on EMP"
        );
        assert_eq!(
            f.expires_frame,
            abort_frame.saturating_add(PARTICLE_WIDTH_GROW_FRAMES)
        );
        assert!(
            f.pulses_made == pulses_at_emp,
            "EMP must stop further kill pulses, had {} then {}",
            pulses_at_emp,
            f.pulses_made
        );
    } else {
        assert!(
            logic.frame >= abort_frame.saturating_add(PARTICLE_WIDTH_GROW_FRAMES),
            "pruned beam must have finished abort decay"
        );
    }
    let victim_obj = logic.host_object(victim).expect("victim");
    assert!(
        victim_obj.is_alive() && !victim_obj.status.destroyed,
        "EMP mid-beam must stop the laser from finishing the kill"
    );
    assert!(
        victim_obj.health.current >= health_at_emp - 0.01,
        "no post-EMP beam damage: health {} -> {}",
        health_at_emp,
        victim_obj.health.current
    );
}

#[test]
fn emp_pulse_spawns_effect_spheroid_residual() {
    use crate::game_logic::KindOf;
    use crate::game_logic::host_emp_pulse::{
        EMP_PULSE_EFFECT_SPHEROID, EMP_SPHEROID_LIFETIME_FRAMES,
    };
    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::China);
    let mut cc = crate::game_logic::ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let caster = logic
        .create_object("ChinaCommandCenter", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    assert!(logic.apply_emp_pulse_at(0, Vec3::new(100.0, 0.0, 100.0), Some(caster)));
    assert!(logic.emp_pulses().honesty_spheroid_ok());
    let sph = logic
        .host_objects()
        .values()
        .find(|o| o.emp_pulse_spheroid)
        .expect("spheroid");
    assert_eq!(sph.template_name, EMP_PULSE_EFFECT_SPHEROID);
    let sid = sph.id;
    logic.frame = EMP_SPHEROID_LIFETIME_FRAMES + 5;
    logic.update_emp_pulse_spheroids();
    assert!(
        logic
            .host_object(sid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
}

#[test]
fn emp_pulse_residual_disables_vehicles_in_radius() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_emp_pulse::{
        EMP_PULSE_DISABLED_DURATION_FRAMES, HOST_EMP_PULSE_RADIUS,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);

    // C++ refuses an any-unit cast without the authored SpecialPowerModule
    // (SpecialPower.cpp:308 canUseSpecialPower) — author the retail
    // SuperweaponEMPPulse module block on the caster template.
    author_superweapon_special_power_module(
        &mut game_logic,
        "TestTank",
        SpecialPowerType::EmpPulse,
        "SuperweaponEMPPulse",
        450,
    );
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_EMPPulse");
    }
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(500.0, 0.0, 500.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        // The authored module armed its ReloadTime at creation (C++
        // SpecialPowerModule ctor: m_availableOnFrame = now + ReloadTime);
        // mark that per-module timer elapsed (setReadyFrame(0) residual).
        caster.set_special_power_ready_seconds(&SpecialPowerType::EmpPulse, 0.0);
    }

    let vehicle_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("vehicle");
    let far_vehicle_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(400.0, 0.0, 0.0))
        .expect("far vehicle");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("infantry");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .expect("barracks");

    // Give vehicle a weapon so can_attack residual is meaningful.
    for id in [vehicle_id, far_vehicle_id] {
        let unit = game_logic.host_object_mut(id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 25.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    let vehicle_hp = game_logic
        .host_object(vehicle_id)
        .expect("vehicle")
        .health
        .current;

    assert!(!game_logic.honesty_emp_pulse_ok());
    assert_eq!(game_logic.emp_pulses().activation_count(), 0);

    let impact = Vec3::new(0.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::EmpPulse,
            target: PowerTarget::Location(impact),
        },
        player_id: 0,
        command_id: 77,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    // DeliverPayload residual: cargo plane + bomb before EMPUpdate disable.
    for f in 0..400 {
        game_logic.frame = f;
        game_logic.update_emp_pulse_flights();
        if game_logic.honesty_emp_pulse_disable_ok() {
            break;
        }
    }

    assert!(
        game_logic.honesty_emp_pulse_activate_ok(),
        "EmpPulse residual must record activation honesty"
    );
    assert!(
        game_logic.honesty_emp_pulse_disable_ok(),
        "EmpPulse residual must record disable honesty"
    );
    assert!(
        game_logic.honesty_emp_pulse_ok(),
        "EmpPulse host residual path honesty"
    );
    assert_eq!(game_logic.emp_pulses().activation_count(), 1);
    assert!(
        (game_logic.emp_pulses().activations()[0].radius - HOST_EMP_PULSE_RADIUS).abs() < 0.01,
        "retail residual radius 200"
    );

    // In-radius vehicle: DISABLED_EMP, cannot move/attack, no HP damage.
    let vehicle = game_logic.host_object(vehicle_id).expect("vehicle");
    assert!(
        vehicle.is_emp_disabled(),
        "in-radius vehicle must be DISABLED_EMP"
    );
    assert!(vehicle.is_disabled());
    assert!(!vehicle.can_move(), "EMP vehicle cannot move");
    assert!(!vehicle.can_attack(), "EMP vehicle cannot attack");
    assert_eq!(
        vehicle.health.current, vehicle_hp,
        "EMP residual must not damage vehicle HP"
    );
    assert_eq!(
        vehicle.status.disabled_emp_until_frame,
        game_logic.frame + EMP_PULSE_DISABLED_DURATION_FRAMES
    );

    // Out-of-radius vehicle: unaffected.
    let far = game_logic.host_object(far_vehicle_id).expect("far");
    assert!(
        !far.is_emp_disabled(),
        "out-of-radius vehicle must not be EMP'd"
    );
    assert!(far.can_move());
    assert!(far.can_attack());

    // Infantry residual: not disabled (EMPUpdate skips non-vehicle/structure).
    let infantry = game_logic.host_object(infantry_id).expect("infantry");
    assert!(
        !infantry.is_emp_disabled(),
        "infantry must not receive DISABLED_EMP residual"
    );

    // Faction structure residual: disabled.
    let barracks = game_logic.host_object(barracks_id).expect("barracks");
    assert!(
        barracks.is_emp_disabled(),
        "faction barracks must be DISABLED_EMP"
    );

    // Combat path: EMP'd vehicle must not fire.
    {
        let vehicle = game_logic.host_object_mut(vehicle_id).expect("vehicle");
        vehicle.target = Some(far_vehicle_id);
        vehicle.set_ai_state(AIState::Attacking);
        vehicle.set_status_attacking(true);
    }
    let far_hp_before = game_logic
        .host_object(far_vehicle_id)
        .expect("far")
        .health
        .current;
    game_logic.update_combat(&[vehicle_id, far_vehicle_id], 1.0 / 30.0);
    let far_hp_after = game_logic
        .host_object(far_vehicle_id)
        .expect("far")
        .health
        .current;
    assert_eq!(
        far_hp_before, far_hp_after,
        "EMP'd vehicle must not damage via combat residual"
    );

    // Expire residual timer → vehicle recovers.
    let until = game_logic
        .host_object(vehicle_id)
        .expect("vehicle")
        .status
        .disabled_emp_until_frame;
    assert!(until > game_logic.frame);
    game_logic.frame = until;
    game_logic.update_ai(&[vehicle_id, barracks_id], 1.0 / 60.0);
    let recovered = game_logic.host_object(vehicle_id).expect("vehicle");
    assert!(
        !recovered.is_emp_disabled(),
        "DISABLED_EMP must clear after DisabledDuration"
    );
    assert!(recovered.can_move(), "recovered vehicle can move again");
    assert!(recovered.can_attack(), "recovered vehicle can attack again");
    let barracks_recovered = game_logic.host_object(barracks_id).expect("barracks");
    assert!(
        !barracks_recovered.is_emp_disabled(),
        "structure EMP must clear after duration"
    );
}

#[test]
fn emp_pulse_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    // C++ SpecialPower.cpp:308: the authored SpecialPowerModule is the cast
    // authority; activation then starts per-module recharge (ready flag
    // clears via startPowerRecharge).
    author_superweapon_special_power_module(
        &mut game_logic,
        "TestTank",
        SpecialPowerType::EmpPulse,
        "SuperweaponEMPPulse",
        450,
    );
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_EMPPulse");
    }

    // player_id 0 → Team::USA residual ownership.
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        // Creation-armed per-module ReloadTime marked elapsed (setReadyFrame(0)).
        caster.set_special_power_ready_seconds(&SpecialPowerType::EmpPulse, 0.0);
    }
    // Place a target so disable honesty can trip (caster is skipped as self).
    let _target = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(5.0, 0.0, 0.0))
        .expect("target");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::EmpPulse,
            target: PowerTarget::Location(Vec3::new(0.0, 0.0, 0.0)),
        },
        player_id: 0,
        command_id: 8,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "EmpPulse must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    // DeliverPayload residual may delay EMPUpdate honesty until bomb impact.
    for f in 0..400 {
        game_logic.frame = f;
        game_logic.update_emp_pulse_flights();
        if game_logic.honesty_emp_pulse_activate_ok() {
            break;
        }
    }
    assert!(
        game_logic.honesty_emp_pulse_activate_ok()
            || game_logic.emp_pulse_flight_reg.transports_spawned >= 1,
        "EmpPulse residual must record activation honesty or cargo residual"
    );
}

#[test]
fn frenzy_residual_buffs_allies_and_boosts_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_frenzy::{HOST_FRENZY_RADIUS, HostFrenzyLevel};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);

    // Caster + ally on China (retail Frenzy faction residual).
    // C++ SpecialPower.cpp:308: the authored SuperweaponFrenzy
    // SpecialPowerModule is the cast authority.
    author_superweapon_special_power_module(
        &mut game_logic,
        "TestTank",
        SpecialPowerType::Frenzy,
        "SuperweaponFrenzy",
        450,
    );
    ensure_test_player_for_team(&mut game_logic, Team::China);
    if let Some(p) = game_logic.get_player_mut(1) {
        p.unlock_science("SCIENCE_Frenzy1");
    }
    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(500.0, 0.0, 500.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        // Creation-armed per-module ReloadTime marked elapsed (setReadyFrame(0)).
        caster.set_special_power_ready_seconds(&SpecialPowerType::Frenzy, 0.0);
    }

    let ally_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(10.0, 0.0, 0.0))
        .expect("ally");
    let far_ally_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(400.0, 0.0, 0.0))
        .expect("far ally");
    let enemy_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(15.0, 0.0, 0.0))
        .expect("enemy");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .expect("barracks");

    // Bind residual weapons so combat damage is measurable.
    for id in [ally_id, far_ally_id, enemy_id] {
        let unit = game_logic.host_object_mut(id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 20.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    assert!(!game_logic.honesty_frenzy_ok());
    assert_eq!(game_logic.frenzies().activation_count(), 0);
    assert!(!game_logic.host_object(ally_id).unwrap().is_frenzy_buffed());

    let impact = Vec3::new(0.0, 0.0, 0.0);
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Frenzy,
            target: PowerTarget::Location(impact),
        },
        // player_id residual unused for team filter when caster present.
        player_id: 1,
        command_id: 88,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_frenzy_activate_ok(),
        "Frenzy residual must record activation honesty"
    );
    assert!(
        game_logic.honesty_frenzy_buff_ok(),
        "Frenzy residual must record buff honesty"
    );
    assert!(
        game_logic.honesty_frenzy_ok(),
        "Frenzy host residual path honesty"
    );
    assert_eq!(game_logic.frenzies().activation_count(), 1);
    assert!(
        (game_logic.frenzies().activations()[0].radius - HOST_FRENZY_RADIUS).abs() < 0.01,
        "retail residual radius 200"
    );
    assert_eq!(
        game_logic.frenzies().activations()[0].level,
        HostFrenzyLevel::One
    );

    // In-radius ally: FRENZY residual buff + 110% damage mult.
    let ally = game_logic.host_object(ally_id).expect("ally");
    assert!(
        ally.is_frenzy_buffed(),
        "in-radius ally must receive FRENZY residual buff"
    );
    assert_eq!(ally.weapon_bonus_frenzy_level, 1);
    assert!((ally.frenzy_damage_multiplier() - 1.10).abs() < 0.001);
    assert_eq!(
        ally.weapon_bonus_frenzy_until_frame,
        game_logic.frame + HostFrenzyLevel::One.duration_frames()
    );

    // Out-of-radius ally: unaffected.
    let far = game_logic.host_object(far_ally_id).expect("far");
    assert!(
        !far.is_frenzy_buffed(),
        "out-of-radius ally must not receive Frenzy residual"
    );

    // Enemy residual: not buffed (same-team filter).
    let enemy = game_logic.host_object(enemy_id).expect("enemy");
    assert!(
        !enemy.is_frenzy_buffed(),
        "enemy must not receive Frenzy residual"
    );

    // Structure residual: ForbiddenAffectKindOf STRUCTURE.
    let barracks = game_logic.host_object(barracks_id).expect("barracks");
    assert!(
        !barracks.is_frenzy_buffed(),
        "structure must not receive Frenzy residual"
    );

    // Observable combat effect: frenzied ally deals 110% damage.
    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;
    {
        let ally = game_logic.host_object_mut(ally_id).expect("ally");
        ally.target = Some(enemy_id);
        ally.set_ai_state(AIState::Attacking);
        ally.set_status_attacking(true);
        // Place in range without path chase residual.
        ally.set_position(Vec3::new(10.0, 0.0, 0.0));
    }
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_position(Vec3::new(15.0, 0.0, 0.0));
    }
    game_logic.update_combat(&[ally_id, enemy_id], 1.0 / 30.0);
    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;
    let dealt = enemy_hp_before - enemy_hp_after;
    assert!(
        (dealt - 22.0).abs() < 0.05,
        "Frenzy residual must deal 110% damage (20 * 1.1 = 22), got {dealt}"
    );

    // Expire residual timer → buff clears.
    let until = game_logic
        .host_object(ally_id)
        .expect("ally")
        .weapon_bonus_frenzy_until_frame;
    assert!(until > game_logic.frame);
    game_logic.frame = until;
    game_logic.update_ai(&[ally_id], 1.0 / 60.0);
    let recovered = game_logic.host_object(ally_id).expect("ally");
    assert!(
        !recovered.is_frenzy_buffed(),
        "FRENZY residual must clear after BonusDuration"
    );
    assert!((recovered.frenzy_damage_multiplier() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn frenzy_buffs_allied_faction_units_not_just_same_team() {
    use crate::game_logic::host_frenzy::HostFrenzyLevel;
    use crate::game_logic::{Player, Weapon};

    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    let mut china = Player::new(1, Team::China, "China", true);
    let mut usa = Player::new(0, Team::USA, "USA", false);
    china.alliance_team = 7;
    usa.alliance_team = 7;
    china.is_alive = true;
    usa.is_alive = true;
    logic.add_player(china);
    logic.add_player(usa);

    let caster = logic
        .create_object_for_player("TestTank", 1, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let ally = logic
        .create_object_for_player("TestTank", 0, Vec3::new(20.0, 0.0, 0.0))
        .expect("usa ally");
    {
        let unit = logic.host_object_mut(ally).expect("ally");
        unit.weapon = Some(Weapon {
            damage: 20.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    assert!(logic.activate_frenzy(
        1,
        Vec3::new(10.0, 0.0, 0.0),
        Some(caster),
        HostFrenzyLevel::One,
    ));
    assert!(
        logic.host_object(ally).unwrap().is_frenzy_buffed(),
        "2v2 allied USA tank must receive China Frenzy"
    );
}

#[test]
fn frenzy_walks_garrison_occupants_of_in_range_structure() {
    use crate::game_logic::Weapon;
    use crate::game_logic::host_frenzy::HostFrenzyLevel;

    let mut logic = GameLogic::new();
    ensure_test_tank_template(&mut logic);
    ensure_test_infantry_template(&mut logic);
    ensure_test_garrison_template(&mut logic);
    ensure_test_player_for_team(&mut logic, Team::China);

    let caster = logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let bunker = logic
        .create_object("TestBunker", Team::China, Vec3::new(15.0, 0.0, 0.0))
        .expect("bunker");
    let rider = logic
        .create_object("TestInfantry", Team::China, Vec3::new(800.0, 0.0, 0.0))
        .expect("rider");
    {
        let unit = logic.host_object_mut(rider).expect("rider");
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 80.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }
    {
        let bunker_obj = logic.host_object_mut(bunker).unwrap();
        if !bunker_obj.add_occupant(rider) {
            if let Some(bd) = bunker_obj.building_data.as_mut() {
                bd.max_garrison = bd.max_garrison.max(5);
                if !bd.garrisoned_units.contains(&rider) {
                    bd.garrisoned_units.push(rider);
                }
            } else if !bunker_obj.occupants.contains(&rider) {
                bunker_obj.occupants.push(rider);
            }
        }
    }

    assert!(logic.activate_frenzy(
        1,
        Vec3::new(10.0, 0.0, 0.0),
        Some(caster),
        HostFrenzyLevel::One,
    ));
    assert!(
        !logic.host_object(bunker).unwrap().is_frenzy_buffed(),
        "STRUCTURE container itself is ForbiddenAffectKindOf"
    );
    assert!(
        logic.host_object(rider).unwrap().is_frenzy_buffed(),
        "garrison occupant must receive Frenzy via contained walk"
    );
}

#[test]
fn frenzy_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    // C++ SpecialPower.cpp:308 module authority + per-module recharge on
    // activation (startPowerRecharge clears the ready flag).
    author_superweapon_special_power_module(
        &mut game_logic,
        "TestTank",
        SpecialPowerType::Frenzy,
        "SuperweaponFrenzy",
        450,
    );
    ensure_test_player_for_team(&mut game_logic, Team::China);
    if let Some(p) = game_logic.get_player_mut(1) {
        p.unlock_science("SCIENCE_Frenzy1");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        // Creation-armed per-module ReloadTime marked elapsed (setReadyFrame(0)).
        caster.set_special_power_ready_seconds(&SpecialPowerType::Frenzy, 0.0);
        // Caster can self-buff residual (WeaponBonusUpdate iterates all allies
        // including source when legal CAN_ATTACK). Ensure weapon so can_attack.
        caster.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::Frenzy,
            target: PowerTarget::Location(Vec3::new(0.0, 0.0, 0.0)),
        },
        player_id: 1,
        command_id: 9,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![caster_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert_eq!(
        game_logic.special_power_strikes().strike_count(),
        0,
        "Frenzy must not enqueue superweapon residual strikes"
    );
    assert!(
        !game_logic
            .host_object(caster_id)
            .unwrap()
            .special_power_ready
    );
    assert!(
        game_logic.honesty_frenzy_activate_ok(),
        "Frenzy residual must record activation honesty"
    );
    assert!(
        game_logic
            .host_object(caster_id)
            .unwrap()
            .is_frenzy_buffed(),
        "caster ally in radius must receive Frenzy residual"
    );
}

#[test]
fn strategy_center_battle_plan_residual_applies_unit_bonuses() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_strategy_center::{
        BOMBARDMENT_DAMAGE_MULT, HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR, HostBattlePlan,
        SEARCH_AND_DESTROY_RANGE_MULT, STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR,
        is_strategy_center_template,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);
    ensure_test_aircraft_template(&mut game_logic);

    // Strategy Center residual template (structure + FS_STRATEGY_CENTER kind).
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0)
        .set_cost(2500, 0);
    sc_template.sight_range = 200.0;
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    // C++ SpecialPower.cpp:308: the authored SuperweaponBattlePlan*
    // SpecialPowerModule on the Strategy Center is the cast authority for the
    // DoSpecialPower bombardment activation below.
    author_superweapon_special_power_module(
        &mut game_logic,
        "AmericaStrategyCenter",
        SpecialPowerType::BattlePlanBombardment,
        "SuperweaponBattlePlanBombardment",
        450,
    );
    assert!(is_strategy_center_template("AmericaStrategyCenter"));

    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let center_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    {
        let center = game_logic.host_object_mut(center_id).expect("center");
        // Creation-armed per-module ReloadTime marked elapsed (setReadyFrame(0)).
        center.set_special_power_ready_seconds(&SpecialPowerType::BattlePlanBombardment, 0.0);
        center.object_type = ObjectType::Building;
    }

    let ally_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ally");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("infantry");
    let enemy_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("barracks");
    let aircraft_id = game_logic
        .create_object("TestAircraft", Team::USA, Vec3::new(50.0, 0.0, 0.0))
        .expect("aircraft");

    for id in [ally_id, infantry_id, enemy_id] {
        let unit = game_logic.host_object_mut(id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 20.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }
    {
        let air = game_logic.host_object_mut(aircraft_id).expect("air");
        air.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    assert!(!game_logic.honesty_battle_plan_ok());
    assert_eq!(game_logic.battle_plans().selection_count(), 0);

    // --- Bombardment residual via DoSpecialPower ---
    game_logic.queue_command(GameCommand {
        command_type: CommandType::DoSpecialPower {
            power_type: SpecialPowerType::BattlePlanBombardment,
            target: PowerTarget::None,
        },
        player_id: 0,
        command_id: 501,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![center_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic.honesty_battle_plan_select_ok(),
        "Battle plan residual must record selection honesty"
    );
    // Delayed ACTIVE residual: buffs not applied until unpack completes.
    assert!(
        !game_logic
            .host_object(ally_id)
            .unwrap()
            .weapon_bonus_battle_plan_bombardment,
        "army buffs must wait for unpack ACTIVE residual"
    );
    assert_eq!(
        game_logic.battle_plans().active_plan_for_player(0),
        None,
        "plan_affecting_army residual only after ACTIVE"
    );
    advance_battle_plan_door_to_active(&mut game_logic);
    assert!(
        game_logic.honesty_battle_plan_buff_ok(),
        "Battle plan residual must record army buff honesty after ACTIVE"
    );
    assert!(
        game_logic.honesty_battle_plan_ok(),
        "Strategy Center host residual path honesty"
    );
    assert!(
        game_logic.honesty_battle_plan_delayed_active_ok(),
        "delayed setBattlePlan ACTIVE residual honesty"
    );
    assert_eq!(game_logic.battle_plans().selection_count(), 1);
    assert_eq!(
        game_logic.battle_plans().active_plan_for_player(0),
        Some(HostBattlePlan::Bombardment)
    );

    let ally = game_logic.host_object(ally_id).expect("ally");
    assert!(
        ally.weapon_bonus_battle_plan_bombardment,
        "ally tank must receive Bombardment residual after ACTIVE"
    );
    assert!((ally.battle_plan_damage_multiplier() - BOMBARDMENT_DAMAGE_MULT).abs() < 0.001);
    let infantry = game_logic.host_object(infantry_id).expect("infantry");
    assert!(
        infantry.weapon_bonus_battle_plan_bombardment,
        "ally infantry must receive Bombardment residual"
    );
    // Enemy residual: not buffed (same-team filter).
    assert!(
        !game_logic
            .host_object(enemy_id)
            .unwrap()
            .weapon_bonus_battle_plan_bombardment,
        "enemy must not receive battle plan residual"
    );
    // Structure residual: InvalidMember STRUCTURE.
    assert!(
        !game_logic
            .host_object(barracks_id)
            .unwrap()
            .weapon_bonus_battle_plan_bombardment,
        "structure must not receive army battle plan residual"
    );
    // Aircraft residual: InvalidMember AIRCRAFT.
    assert!(
        !game_logic
            .host_object(aircraft_id)
            .unwrap()
            .weapon_bonus_battle_plan_bombardment,
        "aircraft must not receive army battle plan residual"
    );

    // Observable combat effect: Bombardment ally deals 120% damage.
    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;
    {
        let ally = game_logic.host_object_mut(ally_id).expect("ally");
        ally.target = Some(enemy_id);
        ally.set_ai_state(AIState::Attacking);
        ally.set_status_attacking(true);
        ally.set_position(Vec3::new(10.0, 0.0, 0.0));
    }
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_position(Vec3::new(15.0, 0.0, 0.0));
        enemy.thing.template.armor = 0.0;
    }
    crate::game_logic::host_damage_log::clear();
    game_logic.update_combat(&[ally_id, enemy_id], 1.0 / 30.0);
    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .expect("enemy")
        .health
        .current;
    let dealt = test_observed_damage_to(enemy_id, enemy_hp_before, enemy_hp_after);
    assert!(
        (dealt - 24.0).abs() < 0.05,
        "Bombardment residual must deal 120% damage (20 * 1.2 = 24), got {dealt}"
    );

    // --- HoldTheLine residual: armor damage scalar 0.9 + center max-health ×2 ---
    {
        let center = game_logic.host_object_mut(center_id).expect("center");
        center.set_special_power_ready(true);
        center.special_power_cooldown_remaining = 0.0;
    }
    let center_max_before = game_logic.host_object(center_id).unwrap().max_health;
    assert!(
        game_logic.activate_battle_plan(0, HostBattlePlan::HoldTheLine, Some(center_id)),
        "HoldTheLine residual must activate"
    );
    // Pack clears Bombardment immediately; new buffs after pack+unpack ACTIVE.
    assert!(
        !game_logic
            .host_object(ally_id)
            .unwrap()
            .weapon_bonus_battle_plan_bombardment,
        "PACKING residual must clear prior army buffs"
    );
    assert!(game_logic.honesty_battle_plan_pack_clear_ok());
    advance_battle_plan_switch_to_active(&mut game_logic);
    let ally = game_logic.host_object(ally_id).expect("ally");
    assert!(ally.weapon_bonus_battle_plan_hold_the_line);
    assert!(!ally.weapon_bonus_battle_plan_bombardment);
    assert!(
        (ally.battle_plan_armor_damage_scalar() - HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR).abs() < 0.001
    );
    let center = game_logic.host_object(center_id).expect("center");
    assert!(
        (center.max_health - center_max_before * STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR)
            .abs()
            < 0.5,
        "HoldTheLine residual must double Strategy Center max health after ACTIVE"
    );

    // Observable armor effect: ally takes 90% damage under HoldTheLine.
    {
        let ally = game_logic.host_object_mut(ally_id).expect("ally");
        ally.thing.template.armor = 0.0;
        ally.health.current = 100.0;
        ally.max_health = 100.0;
        ally.health.maximum = 100.0;
    }
    let ally_hp_before = game_logic.host_object(ally_id).unwrap().health.current;
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.weapon = Some(Weapon {
            damage: 20.0,
            range: 150.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        enemy.target = Some(ally_id);
        enemy.set_ai_state(AIState::Attacking);
        enemy.set_status_attacking(true);
        enemy.set_position(Vec3::new(15.0, 0.0, 0.0));
        enemy.thing.template.armor = 0.0;
    }
    {
        let ally = game_logic.host_object_mut(ally_id).expect("ally");
        ally.set_position(Vec3::new(10.0, 0.0, 0.0));
        ally.target = None;
        ally.set_status_attacking(false);
    }
    crate::game_logic::host_damage_log::clear();
    crate::game_logic::host_damage_log::clear();
    game_logic.update_combat(&[enemy_id, ally_id], 1.0 / 30.0);
    let ally_hp_after = game_logic.host_object(ally_id).unwrap().health.current;
    let taken = test_observed_damage_to(ally_id, ally_hp_before, ally_hp_after);
    assert!(
        (taken - 18.0).abs() < 0.05,
        "HoldTheLine residual must take 90% damage (20 * 0.9 = 18), got {taken}"
    );

    // --- SearchAndDestroy residual: RANGE 120% ---
    {
        let center = game_logic.host_object_mut(center_id).expect("center");
        center.set_special_power_ready(true);
        center.special_power_cooldown_remaining = 0.0;
    }
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::SearchAndDestroy, Some(center_id)));
    advance_battle_plan_switch_to_active(&mut game_logic);
    let ally = game_logic.host_object(ally_id).expect("ally");
    assert!(ally.weapon_bonus_battle_plan_search_and_destroy);
    assert!(!ally.weapon_bonus_battle_plan_hold_the_line);
    assert!((ally.battle_plan_range_multiplier() - SEARCH_AND_DESTROY_RANGE_MULT).abs() < 0.001);
    let center = game_logic.host_object(center_id).expect("center");
    assert!(
        center.is_detector,
        "SearchAndDestroy residual must enable Strategy Center stealth detect after ACTIVE"
    );
    // Range residual: ally can hit target beyond base 100 range up to 120.
    // Plan-switch BattlePlanChangeParalyze residual freezes troops for 150 frames;
    // clear for combat observation of RANGE residual (paralyze tested separately).
    // Flat synthetic LOS for range residual: clear coarse height samples and
    // structure static blocks so AttackNeedsLineOfSight fails-open without a map.
    game_logic.pathfinding_height_samples = None;
    game_logic.pathfinding_system.clear_static_blocks();
    {
        let ally = game_logic.host_object_mut(ally_id).expect("ally");
        ally.set_status_disabled_paralyzed(false);
        ally.status.disabled_paralyzed_until_frame = 0;
        ally.weapon = Some(Weapon {
            damage: 20.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        ally.set_position(Vec3::new(0.0, 0.0, 0.0));
        ally.target = Some(enemy_id);
        ally.set_ai_state(AIState::Attacking);
        ally.set_status_attacking(true);
        ally.attack_substate = crate::game_logic::AttackSubState::AimAtTarget;
        ally.status.is_aiming_weapon = false;
        ally.status.is_firing_weapon = false;
    }
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_position(Vec3::new(110.0, 0.0, 0.0)); // 110 > 100 base, < 120 residual
        enemy.thing.template.armor = 0.0;
        enemy.health.current = 100.0;
        enemy.target = None;
        enemy.set_status_attacking(false);
    }
    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    crate::game_logic::host_damage_log::clear();
    game_logic.update_combat(&[ally_id, enemy_id], 1.0 / 30.0);
    let enemy_hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    let sn_dealt = test_observed_damage_to(enemy_id, enemy_hp_before, enemy_hp_after);
    assert!(
        sn_dealt > 0.5,
        "SearchAndDestroy residual RANGE 120% must allow fire at 110 (> base 100), dealt {sn_dealt}"
    );
}

#[test]
fn strategy_center_battle_plan_paralyze_residual_on_plan_change() {
    use crate::game_logic::host_strategy_center::{BATTLE_PLAN_PARALYZE_FRAMES, HostBattlePlan};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_aircraft_template(&mut game_logic);

    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let center_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    let ally_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("ally");
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("infantry");
    let enemy_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");
    let aircraft_id = game_logic
        .create_object("TestAircraft", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("aircraft");
    for id in [ally_id, infantry_id, enemy_id, aircraft_id] {
        let u = game_logic.host_object_mut(id).expect("unit");
        u.weapon = Some(Weapon {
            damage: 20.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }

    // First select: unpack → ACTIVE applies buffs; no BattlePlanChangeParalyze.
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(center_id)));
    advance_battle_plan_door_to_active(&mut game_logic);
    assert!(!game_logic.honesty_battle_plan_paralyze_ok());
    assert_eq!(game_logic.battle_plans().paralyze_count(), 0);
    assert!(
        !game_logic
            .host_object(ally_id)
            .unwrap()
            .is_paralyzed_disabled(),
        "first select must not paralyze army"
    );

    // Plan switch: PACKING → setBattlePlan(NONE) paralyzes legal members.
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::HoldTheLine, Some(center_id)));
    assert!(
        game_logic.honesty_battle_plan_paralyze_ok(),
        "plan change PACKING residual must record BattlePlanChangeParalyze honesty"
    );
    assert!(game_logic.battle_plans().paralyze_count() >= 2);
    {
        let ally = game_logic.host_object(ally_id).expect("ally");
        assert!(ally.is_paralyzed_disabled(), "ally tank must be paralyzed");
        assert!(ally.is_disabled());
        assert_eq!(
            ally.status.disabled_paralyzed_until_frame,
            game_logic.frame + BATTLE_PLAN_PARALYZE_FRAMES
        );
    }
    assert!(
        game_logic
            .host_object(infantry_id)
            .unwrap()
            .is_paralyzed_disabled(),
        "ally infantry must be paralyzed"
    );
    assert!(
        !game_logic
            .host_object(enemy_id)
            .unwrap()
            .is_paralyzed_disabled(),
        "enemy must not be paralyzed"
    );
    assert!(
        !game_logic
            .host_object(aircraft_id)
            .unwrap()
            .is_paralyzed_disabled(),
        "aircraft InvalidMember must not be paralyzed"
    );

    // Observable: paralyzed unit cannot fire residual.
    let enemy_hp_before = game_logic.host_object(enemy_id).unwrap().health.current;
    {
        let ally = game_logic.host_object_mut(ally_id).expect("ally");
        ally.target = Some(enemy_id);
        ally.set_ai_state(AIState::Attacking);
        ally.set_status_attacking(true);
        ally.set_position(Vec3::new(10.0, 0.0, 0.0));
    }
    {
        let enemy = game_logic.host_object_mut(enemy_id).expect("enemy");
        enemy.set_position(Vec3::new(15.0, 0.0, 0.0));
        enemy.thing.template.armor = 0.0;
    }
    crate::game_logic::host_damage_log::clear();
    game_logic.update_combat(&[ally_id, enemy_id], 1.0 / 30.0);
    let enemy_hp_mid = game_logic.host_object(enemy_id).unwrap().health.current;
    let dealt_mid = test_observed_damage_to(enemy_id, enemy_hp_before, enemy_hp_mid);
    assert!(
        dealt_mid.abs() < 0.05,
        "paralyzed residual must block ally fire, dealt {dealt_mid}"
    );

    // Expire DISABLED_PARALYZED residual after 150 frames.
    game_logic.frame = game_logic.frame.saturating_add(BATTLE_PLAN_PARALYZE_FRAMES);
    let expire_frame = game_logic.frame;
    if let Some(ally) = game_logic.host_object_mut(ally_id) {
        ally.tick_disabled_paralyzed(expire_frame);
    }
    assert!(
        !game_logic
            .host_object(ally_id)
            .unwrap()
            .is_paralyzed_disabled(),
        "paralyze must expire after BattlePlanChangeParalyzeTime"
    );

    // After expiry, fire residual works again (HoldTheLine has no damage mult).
    {
        let ally = game_logic.host_object_mut(ally_id).expect("ally");
        ally.target = Some(enemy_id);
        ally.set_ai_state(AIState::Attacking);
        ally.set_status_attacking(true);
        ally.weapon = Some(Weapon {
            damage: 20.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
    }
    crate::game_logic::host_damage_log::clear();
    game_logic.update_combat(&[ally_id, enemy_id], 1.0 / 30.0);
    let enemy_hp_after = game_logic.host_object(enemy_id).unwrap().health.current;
    let dealt_after = test_observed_damage_to(enemy_id, enemy_hp_mid, enemy_hp_after);
    assert!(
        dealt_after > 0.5,
        "after paralyze expiry ally must fire again (dealt={dealt_after})"
    );
}

#[test]
fn retail_eject_pilot_metadata_drives_death_and_hijacker_interface() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::host_usa_pilot::{
        EJECT_PILOT_TEMPLATE, significantly_above_terrain_threshold,
    };
    use crate::game_logic::{
        EjectPilotCreationList, EjectPilotDeathTypes, EjectPilotExemptStatus,
        EjectPilotVeterancyLevels, VeterancyLevel,
    };

    clear_test_template_voices();
    set_test_per_unit_sound("AmericaVehicleHumvee", "VoiceEject", "HumveeVoiceEject");
    set_test_per_unit_sound("AmericaVehicleHumvee", "SoundEject", "HumveeSoundEject");

    // This checkout ships no retail BIG/INI data (windows_game/
    // extracted_big_files_v2 does not exist), so author the exact retail
    // INIZH blocks inline instead of reading them: the Humvee/Raptor
    // `Behavior = EjectPilotDie` module (C++ EjectPilotDie.cpp — module
    // presence is the getEjectPilotDieInterface hijacker authority; the
    // module's own InvulnerableTime default stays 0 while OCL_EjectPilot*
    // carries the real 2000 ms grant) and the AmericaInfantryPilot Object.
    register_retail_eject_pilot_ocls();
    let mut humvee_template = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    author_eject_pilot_die_module(
        &mut humvee_template,
        EjectPilotDeathTypes::All,
        EjectPilotVeterancyLevels::All,
        EjectPilotExemptStatus::None,
    );
    let mut raptor_template = ThingTemplate::new("AmericaJetRaptor");
    raptor_template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    author_eject_pilot_die_module(
        &mut raptor_template,
        EjectPilotDeathTypes::All,
        EjectPilotVeterancyLevels::All,
        EjectPilotExemptStatus::None,
    );
    let mut pilot_template = ThingTemplate::new(EJECT_PILOT_TEMPLATE);
    pilot_template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);

    let humvee_metadata = humvee_template
        .eject_pilot_die
        .expect("retail Humvee EjectPilotDie metadata");
    assert_eq!(
        humvee_metadata.ground_creation_list,
        Some(EjectPilotCreationList::OnGround)
    );
    assert_eq!(
        humvee_metadata.air_creation_list,
        Some(EjectPilotCreationList::ViaParachute)
    );
    assert_eq!(
        humvee_metadata.invulnerable_time_ms,
        Some(0),
        "module default stays distinct from the selected OCL's 2000 ms grant"
    );
    assert!(raptor_template.eject_pilot_die.is_some());

    let mut game_logic = GameLogic::new();
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA 0", true));
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_template);
    game_logic
        .templates
        .insert("AmericaJetRaptor".to_string(), raptor_template);
    game_logic
        .templates
        .insert(EJECT_PILOT_TEMPLATE.to_string(), pilot_template);

    // A real parsed ground module exposes the hijacker interface before its
    // death OCL runs, then the selected OCL gives the pilot its parsed 60f
    // invulnerability (not the module's default 0 ms).
    let humvee_id = game_logic
        .create_object_for_player("AmericaVehicleHumvee", 0, Vec3::new(50.0, 0.0, 50.0))
        .expect("retail Humvee");
    assert!(game_logic.vehicle_supports_hijacker_ride(humvee_id));
    {
        let humvee = game_logic
            .host_object_mut(humvee_id)
            .expect("Humvee object");
        humvee.experience.level = VeterancyLevel::Veteran;
        humvee.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(humvee_id, Some(Team::GLA));
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == "HumveeVoiceEject"
                && e.position == Some(Vec3::new(50.0, 0.0, 50.0))
                && e.player_index == Some(0)
                && e.object_id.is_none()
        }),
        "VoiceEject must resolve from the dying vehicle: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic.queued_audio_events.iter().any(|e| {
            e.event_type == "HumveeSoundEject"
                && e.position == Some(Vec3::new(50.0, 0.0, 50.0))
                && e.object_id.is_none()
        }),
        "SoundEject must resolve from the dying vehicle: {:?}",
        game_logic.queued_audio_events
    );
    assert!(
        game_logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != "VoiceEject" && e.event_type != "SoundEject"),
        "must not queue VoiceEject/SoundEject slot tokens: {:?}",
        game_logic.queued_audio_events
    );
    game_logic.process_destroy_list();

    let pilot_id = game_logic
        .objects
        .iter()
        .find(|(_, object)| {
            object.is_alive()
                && object.template_name == EJECT_PILOT_TEMPLATE
                && object.owner_player_id == Some(0)
        })
        .map(|(id, _)| *id)
        .expect("retail ground OCL must create an owned pilot");
    let pilot = game_logic.host_object(pilot_id).expect("ejected pilot");
    assert_eq!(pilot.experience.level, VeterancyLevel::Veteran);
    assert_eq!(
        pilot.status.eject_invulnerable_until_frame,
        game_logic.frame + 60,
        "retail OCL_EjectPilotOnGround InvulnerableTime=2000 ms is parsed as 60 frames"
    );
    assert!(!pilot.is_parachuting());

    // Aircraft may carry this module too.  C++ selects the air list solely
    // by significant height, so this cannot retain the old vehicle-only
    // eligibility guard.
    let raptor_id = game_logic
        .create_object_for_player(
            "AmericaJetRaptor",
            0,
            Vec3::new(100.0, significantly_above_terrain_threshold() + 40.0, 0.0),
        )
        .expect("retail Raptor");
    assert!(game_logic.vehicle_supports_hijacker_ride(raptor_id));
    {
        let raptor = game_logic
            .host_object_mut(raptor_id)
            .expect("Raptor object");
        raptor.experience.level = VeterancyLevel::Veteran;
        raptor.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(raptor_id, Some(Team::GLA));
    game_logic.process_destroy_list();
    assert!(
        game_logic.objects.values().any(|object| {
            object.is_alive()
                && object.template_name == EJECT_PILOT_TEMPLATE
                && object.owner_player_id == Some(0)
                && object.is_parachuting()
        }),
        "retail aircraft EjectPilotDie must select OCL_EjectPilotViaParachute"
    );

    // A name-only impostor gets neither the interface nor a death spawn.
    let mut name_only = ThingTemplate::new("AmericaVehicleHumveeNameOnly");
    name_only.add_kind_of(KindOf::Vehicle).set_health(200.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumveeNameOnly".to_string(), name_only);
    let name_only_id = game_logic
        .create_object_for_player(
            "AmericaVehicleHumveeNameOnly",
            0,
            Vec3::new(150.0, 0.0, 0.0),
        )
        .expect("name-only vehicle");
    assert!(!game_logic.vehicle_supports_hijacker_ride(name_only_id));
    let ejections_before_name_only = game_logic.usa_pilot_residual().ejections;
    game_logic
        .host_object_mut(name_only_id)
        .expect("name-only object")
        .status
        .destroyed = true;
    game_logic.mark_object_for_destruction(name_only_id, Some(Team::GLA));
    game_logic.process_destroy_list();
    assert_eq!(
        game_logic.usa_pilot_residual().ejections,
        ejections_before_name_only,
        "a matching basename is not an EjectPilotDie module"
    );

    // The interface does not depend on whether the selected OCL is supported.
    // Parse an otherwise retail-shaped module with an unknown list: it remains
    // hijack-visible, but its physical death outcome must fail closed.
    let unknown_source = r#"
Object ParserShapedUnknownEject
  KindOf = VEHICLE
  Behavior = EjectPilotDie ModuleTag_01
    GroundCreationList = OCL_NotSupportedByHost
  End
End
"#;
    let mut unknown_parser = crate::assets::IniParser::new();
    unknown_parser
        .parse_ini_content(unknown_source, "unknown_eject.ini")
        .expect("parse unknown EjectPilotDie fixture");
    let unknown_template = GameLogic::build_template_from_object_definition(
        "ParserShapedUnknownEject",
        unknown_parser
            .get_definition("ParserShapedUnknownEject")
            .expect("unknown EjectPilotDie definition"),
        None,
    );
    assert!(unknown_template.eject_pilot_die.is_some());
    game_logic
        .templates
        .insert("ParserShapedUnknownEject".to_string(), unknown_template);
    let unknown_id = game_logic
        .create_object_for_player("ParserShapedUnknownEject", 0, Vec3::new(200.0, 0.0, 0.0))
        .expect("unknown module vehicle");
    assert!(
        game_logic.vehicle_supports_hijacker_ride(unknown_id),
        "module presence, not OCL support, is C++ targetCanEject authority"
    );
    let ejections_before_unknown = game_logic.usa_pilot_residual().ejections;
    game_logic
        .host_object_mut(unknown_id)
        .expect("unknown module object")
        .status
        .destroyed = true;
    game_logic.mark_object_for_destruction(unknown_id, Some(Team::GLA));
    game_logic.process_destroy_list();
    assert_eq!(
        game_logic.usa_pilot_residual().ejections,
        ejections_before_unknown,
        "unknown selected OCL must not synthesize an ejection"
    );
    clear_test_template_voices();
}

#[test]
fn eject_pilot_veterancy_levels_all_minus_regular_residual() {
    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_usa_pilot::EJECT_PILOT_TEMPLATE;

    let mut game_logic = GameLogic::new();
    // C++ EjectPilotDie: authored module + OCL_EjectPilot* lists + live owner
    // player (RequiresLivePlayer = Yes); DieMux VeterancyLevels = ALL -REGULAR.
    ensure_eject_pilot_residual_fixture(&mut game_logic);

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    author_eject_pilot_die_module(
        &mut humvee_tpl,
        EjectPilotDeathTypes::All,
        EjectPilotVeterancyLevels::AllExceptRegular,
        EjectPilotExemptStatus::None,
    );
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    // Rookie / REGULAR residual → no eject.
    let rookie_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("rookie humvee");
    {
        let h = game_logic.host_object_mut(rookie_id).expect("rookie");
        h.experience.level = VeterancyLevel::Rookie;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(rookie_id, Some(Team::GLA));
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_pilot_eject_veterancy_gate_ok(),
        "REGULAR gate residual must record block honesty"
    );
    assert_eq!(
        game_logic.usa_pilot_residual().ejections,
        0,
        "Rookie vehicle must not eject pilot"
    );
    assert_eq!(
        game_logic
            .objects
            .values()
            .filter(|o| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
            .count(),
        0,
        "no pilot spawn for REGULAR residual"
    );

    // Veteran residual → eject.
    let vet_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(80.0, 0.0, 80.0),
        )
        .expect("vet humvee");
    {
        let h = game_logic.host_object_mut(vet_id).expect("vet");
        h.experience.level = VeterancyLevel::Veteran;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(vet_id, Some(Team::GLA));
    game_logic.process_destroy_list();
    assert_eq!(
        game_logic.usa_pilot_residual().ejections,
        1,
        "Veteran vehicle must eject pilot residual"
    );
    assert!(game_logic.honesty_pilot_eject_ok());

    // Elite residual → eject.
    let elite_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(160.0, 0.0, 160.0),
        )
        .expect("elite humvee");
    {
        let h = game_logic.host_object_mut(elite_id).expect("elite");
        h.experience.level = VeterancyLevel::Elite;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(elite_id, Some(Team::GLA));
    game_logic.process_destroy_list();
    assert_eq!(
        game_logic.usa_pilot_residual().ejections,
        2,
        "Elite vehicle must eject pilot residual"
    );
}

#[test]
fn eject_pilot_die_mux_death_types_and_hijacked_residual() {
    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_usa_pilot::{EJECT_PILOT_TEMPLATE, HostDeathType};

    let mut game_logic = GameLogic::new();
    // C++ EjectPilotDie: authored module + OCL lists + live owner player;
    // DieMux DeathTypes = ALL_EXCEPT_CRUSHED_SPLATTED, ExemptStatus = HIJACKED.
    ensure_eject_pilot_residual_fixture(&mut game_logic);

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    author_eject_pilot_die_module(
        &mut humvee_tpl,
        EjectPilotDeathTypes::AllExceptCrushedAndSplatted,
        EjectPilotVeterancyLevels::All,
        EjectPilotExemptStatus::Hijacked,
    );
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    // CRUSHED death residual → no eject.
    let crushed_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("crushed humvee");
    {
        let h = game_logic.host_object_mut(crushed_id).expect("crushed");
        h.experience.level = VeterancyLevel::Veteran;
        let _ = h.take_damage(h.max_health * 2.0);
        // take_damage sets death_type from damage class — restore DieMux residual.
        h.status.death_type = HostDeathType::Crushed;
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(crushed_id, Some(Team::GLA));
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_pilot_eject_death_type_gate_ok(),
        "DeathTypes CRUSHED residual must record block honesty"
    );
    assert_eq!(
        game_logic.usa_pilot_residual().ejections,
        0,
        "CRUSHED death must not eject pilot"
    );
    assert_eq!(
        game_logic
            .objects
            .values()
            .filter(|o| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
            .count(),
        0,
        "no pilot spawn on crushed residual"
    );

    // SPLATTED death residual → no eject.
    let splat_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(80.0, 0.0, 80.0),
        )
        .expect("splat humvee");
    {
        let h = game_logic.host_object_mut(splat_id).expect("splat");
        h.experience.level = VeterancyLevel::Veteran;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.death_type = HostDeathType::Splatted;
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(splat_id, Some(Team::GLA));
    game_logic.process_destroy_list();
    assert_eq!(
        game_logic.usa_pilot_residual().eject_death_type_blocks,
        2,
        "SPLATTED death must also record death-type block"
    );
    assert_eq!(
        game_logic.usa_pilot_residual().ejections,
        0,
        "SPLATTED death must not eject pilot"
    );

    // HIJACKED residual → no eject.
    let hijack_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(160.0, 0.0, 160.0),
        )
        .expect("hijacked humvee");
    {
        let h = game_logic.host_object_mut(hijack_id).expect("hijacked");
        h.experience.level = VeterancyLevel::Veteran;
        h.apply_hijacked();
        let _ = h.take_damage(h.max_health * 2.0);
        // Preserve HIJACKED after damage residual.
        h.set_status_hijacked(true);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(hijack_id, Some(Team::GLA));
    game_logic.process_destroy_list();
    assert!(
        game_logic.honesty_pilot_eject_hijacked_gate_ok(),
        "ExemptStatus HIJACKED residual must record block honesty"
    );
    assert!(
        game_logic.honesty_pilot_eject_die_mux_ok(),
        "DieMux residual honesty must fire for death-type or hijacked"
    );
    assert_eq!(
        game_logic.usa_pilot_residual().ejections,
        0,
        "HIJACKED vehicle must not eject pilot"
    );

    // Normal combat death residual still ejects.
    let normal_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(240.0, 0.0, 240.0),
        )
        .expect("normal humvee");
    {
        let h = game_logic.host_object_mut(normal_id).expect("normal");
        h.experience.level = VeterancyLevel::Veteran;
        h.status.death_type = HostDeathType::Normal;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(normal_id, Some(Team::GLA));
    game_logic.process_destroy_list();
    assert_eq!(
        game_logic.usa_pilot_residual().ejections,
        1,
        "Normal combat death must still eject pilot residual"
    );
    assert!(
        game_logic
            .objects
            .values()
            .any(|o| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE),
        "normal death residual must spawn pilot"
    );
}

#[test]
fn pilot_find_vehicle_collide_module_would_like_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    // Authority channels default off on the fresh GameLogic context —
    // host-state residual honesty without shadow writeback.

    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_usa_pilot::{
        PILOT_FIND_VEHICLE_SCAN_FRAMES, significantly_above_terrain_threshold,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    // C++ PilotFindVehicleUpdate gates the scan on the authored
    // VeterancyCrateCollide pilot module, not the basename.
    author_pilot_recrew_module(&mut pilot_tpl);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.experience.level = VeterancyLevel::Veteran; // levels_to_gain = 1
        p.set_ai_state(AIState::Idle);
    }

    // Closer Heroic unmanned tank — CollideModule canGainExp residual rejects.
    // Spawn USA then Neutral so PartitionFilterPlayer residual keeps owner.
    let heroic_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("heroic tank");
    {
        let t = game_logic.host_object_mut(heroic_id).expect("heroic");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Heroic;
        t.health.current = t.max_health;
    }

    // Elevated unmanned tank further out — isSignificantlyAboveTerrain residual rejects.
    // Do not set airborne_target: recrewable path still treats it as ground vehicle,
    // but CollideModule residual blocks via height gate alone.
    let elevated_id = game_logic
        .create_object(
            "TestTank",
            Team::USA,
            Vec3::new(12.0, significantly_above_terrain_threshold() + 10.0, 0.0),
        )
        .expect("elevated tank");
    {
        let t = game_logic.host_object_mut(elevated_id).expect("elevated");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        t.health.current = t.max_health;
    }

    // Valid Rookie unmanned tank within Enter residual range (~selection radii + 4).
    let ok_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("ok tank");
    {
        let t = game_logic.host_object_mut(ok_id).expect("ok");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        t.health.current = t.max_health;
    }

    assert!(!game_logic.honesty_pilot_find_vehicle_ok());
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    game_logic.update_ai(&[pilot_id, heroic_id, elevated_id, ok_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_pilot_find_vehicle_collide_ok(),
        "CollideModule residual must reject Heroic / elevated candidates"
    );
    assert!(
        game_logic.usa_pilot_residual().find_vehicle_collide_rejects >= 2,
        "expect ≥2 CollideModule residual rejects (heroic + elevated), got {}",
        game_logic.usa_pilot_residual().find_vehicle_collide_rejects
    );
    assert!(
        game_logic.honesty_pilot_find_vehicle_ok(),
        "PilotFindVehicle residual must still Enter valid Rookie vehicle"
    );

    // Complete Enter if not finished same-frame.
    if game_logic
        .host_object(pilot_id)
        .map(|p| !p.status.destroyed)
        .unwrap_or(false)
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after scan");
        assert_eq!(
            p.target,
            Some(ok_id),
            "must skip Heroic/elevated and target valid Rookie vehicle"
        );
        game_logic.update_ai(&[pilot_id, heroic_id, elevated_id, ok_id], 1.0 / 30.0);
    }

    let ok = game_logic.host_object(ok_id).expect("ok after recrew");
    assert!(!ok.is_unmanned(), "valid vehicle must be recrewed");
    assert_eq!(ok.team, Team::USA, "recrew transfers pilot team");
    assert!(
        game_logic.host_object(heroic_id).unwrap().is_unmanned(),
        "Heroic residual must remain unmanned (CollideModule reject)"
    );
    assert!(
        game_logic.host_object(elevated_id).unwrap().is_unmanned(),
        "elevated residual must remain unmanned (CollideModule reject)"
    );
    assert!(
        game_logic
            .host_object(pilot_id)
            .map(|p| p.status.destroyed)
            .unwrap_or(true),
        "pilot consumed after CollideModule-gated auto-recrew residual"
    );

}

#[test]
fn eject_pilot_invulnerable_time_residual() {
    use crate::game_logic::VeterancyLevel;
    use crate::game_logic::host_usa_pilot::{
        EJECT_PILOT_INVULNERABLE_FRAMES, EJECT_PILOT_TEMPLATE,
    };

    let mut game_logic = GameLogic::new();
    // C++ EjectPilotDie: authored module + OCL lists + live owner player;
    // DieMux VeterancyLevels = ALL -REGULAR (Veteran passes the gate).
    ensure_eject_pilot_residual_fixture(&mut game_logic);

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    author_eject_pilot_die_module(
        &mut humvee_tpl,
        EjectPilotDeathTypes::All,
        EjectPilotVeterancyLevels::AllExceptRegular,
        EjectPilotExemptStatus::None,
    );
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let humvee_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(50.0, 0.0, 50.0),
        )
        .expect("humvee");

    {
        let h = game_logic.host_object_mut(humvee_id).expect("humvee");
        // VeterancyLevels = ALL -REGULAR residual required for eject path.
        h.experience.level = VeterancyLevel::Veteran;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(humvee_id, Some(Team::GLA));
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_pilot_invulnerable_ok(),
        "InvulnerableTime residual must grant honesty on eject"
    );

    let pilot_id = game_logic
        .objects
        .iter()
        .find(|(_, o)| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
        .map(|(id, _)| *id)
        .expect("ejected pilot");

    {
        let pilot = game_logic.host_object(pilot_id).expect("pilot");
        assert!(
            pilot.is_eject_invulnerable(),
            "pilot must be invulnerable immediately after eject"
        );
        assert_eq!(
            pilot.status.eject_invulnerable_until_frame,
            game_logic.frame + EJECT_PILOT_INVULNERABLE_FRAMES,
            "InvulnerableTime residual until-frame must match 60 frames"
        );
    }

    let hp_before = game_logic.host_object(pilot_id).unwrap().health.current;
    let (destroyed, blocked) = game_logic.apply_host_damage(pilot_id, 50.0);
    assert!(!destroyed, "invulnerable pilot must not die");
    assert!(blocked, "InvulnerableTime must block damage");
    assert!(
        game_logic.honesty_pilot_invulnerable_block_ok(),
        "block honesty must record"
    );
    let hp_mid = game_logic.host_object(pilot_id).unwrap().health.current;
    assert!(
        (hp_mid - hp_before).abs() < 0.001,
        "HP must be unchanged during InvulnerableTime, got {hp_mid} vs {hp_before}"
    );

    // Expire InvulnerableTime residual.
    let expire = game_logic.frame + EJECT_PILOT_INVULNERABLE_FRAMES;
    game_logic.frame = expire;
    if let Some(pilot) = game_logic.host_object_mut(pilot_id) {
        pilot.tick_eject_invulnerable(expire);
    }
    assert!(
        !game_logic
            .host_object(pilot_id)
            .unwrap()
            .is_eject_invulnerable(),
        "InvulnerableTime must expire after 60 frames"
    );

    let (destroyed2, blocked2) = game_logic.apply_host_damage(pilot_id, 25.0);
    assert!(!blocked2, "post-expiry damage must not be blocked");
    assert!(!destroyed2);
    let hp_after = game_logic.host_object(pilot_id).unwrap().health.current;
    assert!(
        hp_after < hp_mid - 0.5,
        "post-expiry pilot must take damage"
    );
}
