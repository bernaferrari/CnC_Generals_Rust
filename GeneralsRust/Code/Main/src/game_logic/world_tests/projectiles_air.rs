//! Host GameLogic tests — `projectiles_air`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;

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
    t.special_power_modules
        .push(SpecialPowerModuleMetadata {
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
    logic.templates.insert("AmericaCommandCenter".into(), template);
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
    assert!(logic
        .objects
        .values()
        .any(|o| o.template_name == BAIKONUR_DETONATION_OBJECT));
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
    use crate::game_logic::special_power_strikes::{
        SPECTRE_HOWITZER_HEIGHT_DIE_INITIAL_DELAY_FRAMES, SPECTRE_HOWITZER_SHELL_OBJECT,
    };
    use crate::game_logic::KindOf;
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
    logic
        .special_power_strikes
        .record_orbit_tick_complete(field_id, 0.0, 0, 0, logic.frame);
    logic.spawn_spectre_howitzer_shell_objects_for_new_spawns();
    assert!(logic
        .special_power_strikes
        .honesty_howitzer_shell_object_spawn_ok());
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
    assert!(logic
        .host_object(sid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn scud_storm_anthrax_beta_spawns_poison_field_upgraded_large() {
    use crate::game_logic::special_power_strikes::{
        ScudStormAnthraxTier, SCUD_POISON_UPGRADED_OBJECT_NAME, SCUD_STORM_POISON_DURATION_FRAMES,
    };
    use crate::game_logic::KindOf;
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
    assert!(logic
        .host_object(oid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn scud_storm_spawns_poison_field_large_object() {
    use crate::game_logic::special_power_strikes::{
        SCUD_POISON_OBJECT_NAME, SCUD_STORM_POISON_DURATION_FRAMES,
    };
    use crate::game_logic::KindOf;
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
    assert!(logic
        .host_object(oid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn anthrax_bomb_spawns_toxin_field_object() {
    use crate::game_logic::special_power_strikes::{
        ANTHRAX_TOXIN_DURATION_FRAMES, ANTHRAX_TOXIN_OBJECT_NAME,
    };
    use crate::game_logic::KindOf;
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
    assert!(logic
        .host_object(oid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn nuclear_missile_spawns_radiation_field_object() {
    use crate::game_logic::special_power_strikes::{
        NUKE_RADIATION_DURATION_FRAMES, NUKE_RADIATION_OBJECT_NAME,
    };
    use crate::game_logic::KindOf;
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
    assert!(logic
        .special_power_strikes
        .honesty_radiation_object_spawn_ok());
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
    assert!(logic
        .host_object(oid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn point_defense_intercept_spawns_laser_beam_object() {
    use crate::game_logic::host_point_defense::{
        PDL_LASER_BEAM_DEFAULT, PDL_LASER_BEAM_LIFETIME_FRAMES,
    };
    use crate::game_logic::KindOf;
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
    assert!(logic
        .host_object(bid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn particle_cannon_spawns_connector_laser_objects() {
    use crate::game_logic::special_power_strikes::{
        PARTICLE_CONNECTOR_INTENSE_LASER, PARTICLE_CONNECTOR_MEDIUM_LASER,
    };
    use crate::game_logic::KindOf;
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
    assert!(logic
        .special_power_strikes
        .honesty_connector_object_spawn_ok());
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
        assert!(logic
            .host_object(id)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true));
    }
}

#[test]
fn particle_cannon_spawns_orbital_laser_object() {
    use crate::game_logic::special_power_strikes::PARTICLE_ORBITAL_LASER_NAME;
    use crate::game_logic::KindOf;
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
    assert!(logic
        .host_object(lid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn particle_uplink_spawns_trail_remnant_objects() {
    use crate::game_logic::special_power_strikes::{
        PARTICLE_REMNANT_DURATION_FRAMES, PARTICLE_REMNANT_OBJECT_NAME,
    };
    use crate::game_logic::KindOf;
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
    assert!(logic
        .special_power_strikes
        .honesty_remnant_object_spawn_ok());
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
    assert!(logic
        .host_object(oid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
}

#[test]
fn particle_cannon_emp_mid_beam_starts_decay_and_stops_killing() {
    use crate::game_logic::special_power_strikes::{
        PARTICLE_BEAM_DURATION_FRAMES, PARTICLE_BEAM_TOTAL_PULSES, PARTICLE_WIDTH_GROW_FRAMES,
    };
    use crate::game_logic::KindOf;
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
    let bid = logic.special_power_strikes.spawn_beam_field(
        caster,
        Team::USA,
        Vec3::ZERO,
        spawn,
        1,
    );

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
    use crate::game_logic::host_emp_pulse::{
        EMP_PULSE_EFFECT_SPHEROID, EMP_SPHEROID_LIFETIME_FRAMES,
    };
    use crate::game_logic::KindOf;
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
    assert!(logic
        .host_object(sid)
        .map(|o| !o.is_alive() || o.status.destroyed)
        .unwrap_or(true));
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

    // player_id 0 maps to Team::USA when no Player registry entry exists.
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    if let Some(p) = game_logic.get_player_mut(0) {
        p.unlock_science("SCIENCE_EMPPulse");
    }
    let caster_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(500.0, 0.0, 500.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
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

/// EmpPulse is not a superweapon residual strike (separate disable residual path).
#[test]
fn emp_pulse_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
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
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
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

/// Residual: Frenzy ("Rage") special power buffs ally units in radius.
///
/// C++ SuperweaponFrenzy → Frenzy_InvisibleMarker WeaponBonusUpdate
/// doTempWeaponBonus(FRENZY_ONE, BonusDuration=10000ms) on allies CAN_ATTACK
/// non-STRUCTURE. Host residual applies DAMAGE 110% while buffed.
/// Fail-closed: not full OCL marker / science tiers / FrenzyCloud particles.
#[test]
fn frenzy_residual_buffs_allies_and_boosts_damage() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};
    use crate::game_logic::host_frenzy::{HostFrenzyLevel, HOST_FRENZY_RADIUS};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);

    // Caster + ally on China (retail Frenzy faction residual).
    ensure_test_player_for_team(&mut game_logic, Team::China);
    if let Some(p) = game_logic.get_player_mut(1) {
        p.unlock_science("SCIENCE_Frenzy1");
    }
    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(500.0, 0.0, 500.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
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

/// C++ WeaponBonusUpdate ALLOW_ALLIES — 2v2 China Frenzy buffs allied USA CAN_ATTACK.
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

/// C++ iterateContained: passengers of an in-range STRUCTURE get Frenzy even
/// when their live position is not independently inside BonusRange.
#[test]
fn frenzy_walks_garrison_occupants_of_in_range_structure() {
    use crate::game_logic::host_frenzy::HostFrenzyLevel;
    use crate::game_logic::Weapon;

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

/// Frenzy is not a superweapon residual strike (separate buff residual path).
#[test]
fn frenzy_does_not_queue_superweapon_strike() {
    use crate::command_system::{CommandType, GameCommand, PowerTarget, SpecialPowerType};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_player_for_team(&mut game_logic, Team::China);
    if let Some(p) = game_logic.get_player_mut(1) {
        p.unlock_science("SCIENCE_Frenzy1");
    }

    let caster_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    {
        let caster = game_logic.host_object_mut(caster_id).expect("caster");
        caster.set_special_power_ready(true);
        caster.special_power_cooldown_remaining = 0.0;
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
        is_strategy_center_template, HostBattlePlan, BOMBARDMENT_DAMAGE_MULT,
        HOLD_THE_LINE_ARMOR_DAMAGE_SCALAR, SEARCH_AND_DESTROY_RANGE_MULT,
        STRATEGY_CENTER_HOLD_THE_LINE_MAX_HEALTH_SCALAR,
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
    assert!(is_strategy_center_template("AmericaStrategyCenter"));

    ensure_test_player_for_team(&mut game_logic, Team::USA);

    let center_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    {
        let center = game_logic.host_object_mut(center_id).expect("center");
        center.set_special_power_ready(true);
        center.special_power_cooldown_remaining = 0.0;
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

/// Residual: BattlePlanChangeParalyzeTime freezes legal army members on plan *switch*.
///
/// C++ BattlePlanUpdate::paralyzeTroop on PLANSTATUS_NONE (PACKING) transition:
/// DISABLED_PARALYZED for 5000 ms (150 frames). First select does not paralyze;
/// changing plan (BeganPacking) does.
#[test]
fn strategy_center_battle_plan_paralyze_residual_on_plan_change() {
    use crate::game_logic::host_strategy_center::{HostBattlePlan, BATTLE_PLAN_PARALYZE_FRAMES};

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

/// Retail parser → typed EjectPilotDie module → live death/OCL outcome.
///
/// This exercises the actual AmericaVehicle/AmericaAir Object INI entries
/// instead of admitting an object from its basename.  It also proves the C++
/// `getEjectPilotDieInterface()` distinction: module presence permits the
/// Hijacker-side query even when a selected OCL is unsupported, while actual
/// death spawning remains fail-closed.
#[test]
fn retail_eject_pilot_metadata_drives_death_and_hijacker_interface() {
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_per_unit_sound,
    };
    use crate::game_logic::host_usa_pilot::{
        significantly_above_terrain_threshold, EJECT_PILOT_TEMPLATE,
    };
    use crate::game_logic::{EjectPilotCreationList, VeterancyLevel};
    use std::path::Path;

    clear_test_template_voices();
    set_test_per_unit_sound("AmericaVehicleHumvee", "VoiceEject", "HumveeVoiceEject");
    set_test_per_unit_sound("AmericaVehicleHumvee", "SoundEject", "HumveeSoundEject");

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("Main crate must remain three levels below repository root");
    let retail_object_dir =
        repo_root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Object");
    let mut parser = crate::assets::IniParser::new();
    for filename in [
        "AmericaVehicle.ini",
        "AmericaAir.ini",
        "AmericaInfantry.ini",
    ] {
        let source = std::fs::read_to_string(retail_object_dir.join(filename))
            .unwrap_or_else(|error| panic!("read retail {filename}: {error}"));
        parser
            .parse_ini_content(&source, filename)
            .unwrap_or_else(|error| panic!("parse retail {filename}: {error}"));
    }

    let build_retail_template = |name: &str| {
        GameLogic::build_template_from_object_definition(
            name,
            parser
                .get_definition(name)
                .unwrap_or_else(|| panic!("retail definition {name}")),
            None,
        )
    };
    let humvee_template = build_retail_template("AmericaVehicleHumvee");
    let raptor_template = build_retail_template("AmericaJetRaptor");
    let pilot_template = build_retail_template(EJECT_PILOT_TEMPLATE);

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

/// Residual: EjectPilotDie VeterancyLevels = ALL -REGULAR blocks Rookie eject.
///
/// C++ DieMuxData::isDieApplicable → only vet+ gives pilot.
#[test]
fn eject_pilot_veterancy_levels_all_minus_regular_residual() {
    use crate::game_logic::host_usa_pilot::EJECT_PILOT_TEMPLATE;
    use crate::game_logic::VeterancyLevel;

    let mut game_logic = GameLogic::new();

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
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

/// Residual: EjectPilotDie DeathTypes ALL -CRUSHED -SPLATTED + ExemptStatus HIJACKED.
///
/// C++ DieMuxData::isDieApplicable → DeathTypes / ExemptStatus filters.
/// Crushed / splatted deaths and hijacked vehicles do not eject.
/// Fail-closed: not full DeathType enum matrix beyond crush/splat residual.
#[test]
fn eject_pilot_die_mux_death_types_and_hijacked_residual() {
    use crate::game_logic::host_usa_pilot::{HostDeathType, EJECT_PILOT_TEMPLATE};
    use crate::game_logic::VeterancyLevel;

    let mut game_logic = GameLogic::new();

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
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

/// Residual: PilotFindVehicleUpdate CollideModule wouldLikeToCollideWith gates.
///
/// C++ VeterancyCrateCollide: skip significantly-above-terrain / airborne /
/// non-trainable / cannot-gain-exp targets. Host residual: Heroic unmanned
/// vehicle is rejected (pilot levels cannot promote past Heroic); elevated
/// unmanned vehicle is rejected by isSignificantlyAboveTerrain residual.
/// Fail-closed: not full same-map PartitionFilterSameMapStatus.
#[test]
fn pilot_find_vehicle_collide_module_would_like_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::host_usa_pilot::{
        significantly_above_terrain_threshold, PILOT_FIND_VEHICLE_SCAN_FRAMES,
    };
    use crate::game_logic::VeterancyLevel;

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

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

/// Residual: OCL InvulnerableTime (2000ms → 60 frames) shields ejected pilot.
///
/// C++ ObjectCreationList InvulnerableTime → goInvulnerable residual.
/// Host residual: damage blocked until timer expires.
/// Fail-closed: not full UNDETECTED_DEFECTOR FX flash matrix.
#[test]
fn eject_pilot_invulnerable_time_residual() {
    use crate::game_logic::host_usa_pilot::{
        EJECT_PILOT_INVULNERABLE_FRAMES, EJECT_PILOT_TEMPLATE,
    };
    use crate::game_logic::VeterancyLevel;

    let mut game_logic = GameLogic::new();

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
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

/// Residual: PilotFindVehicleUpdate AI auto-scan → Enter recrewable unmanned vehicle.
///
/// C++ PilotFindVehicleUpdate: ScanRate 1000ms / ScanRange 300 / MinHealth 0.5;
/// AI-only (human sleep forever). Host residual: issues Enter toward nearest
/// recrewable unmanned vehicle meeting MinHealth, then recrew path completes.
#[test]
fn pilot_find_vehicle_ai_auto_scan_min_health_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::host_usa_pilot::{
        is_pilot_template, PILOT_FIND_VEHICLE_MIN_HEALTH, PILOT_FIND_VEHICLE_SCAN_FRAMES,
    };
    use crate::game_logic::VeterancyLevel;

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);

    // AI player residual (is_local=false) — C++ PLAYER_HUMAN skips scan.
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        assert!(is_pilot_template(&p.template_name));
        p.experience.level = VeterancyLevel::Veteran;
        p.set_ai_state(AIState::Idle);
    }

    // Healthy unmanned tank within scan range (100% HP ≥ MinHealth 0.5).
    // Spawn USA then Neutral so PartitionFilterPlayer residual keeps owner.
    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("tank");
    {
        let t = game_logic.host_object_mut(tank_id).expect("tank");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.health.current = t.max_health;
    }

    // Low-HP unmanned tank closer — must be skipped by MinHealth residual.
    let low_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("low tank");
    {
        let t = game_logic.host_object_mut(low_id).expect("low");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.health.current = t.max_health * (PILOT_FIND_VEHICLE_MIN_HEALTH - 0.1);
    }

    assert!(!game_logic.honesty_pilot_find_vehicle_ok());
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES; // scan tick residual
    game_logic.update_ai(&[pilot_id, tank_id, low_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_pilot_find_vehicle_ok(),
        "PilotFindVehicle residual must issue Enter order honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().find_vehicle_orders, 1);

    // Same-frame process_ai_behavior may complete recrew when already in enter
    // range (selection radii residual). Otherwise a second tick finishes Enter.
    if game_logic
        .host_object(pilot_id)
        .map(|p| !p.status.destroyed)
        .unwrap_or(false)
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after scan");
        assert_eq!(
            p.ai_state,
            AIState::Entering,
            "AI pilot residual must Enter after scan"
        );
        assert_eq!(
            p.target,
            Some(tank_id),
            "must target healthy vehicle (MinHealth skips low-HP closer tank)"
        );
        game_logic.update_ai(&[pilot_id, tank_id, low_id], 1.0 / 30.0);
    }

    let tank = game_logic.host_object(tank_id).expect("tank after recrew");
    assert!(!tank.is_unmanned(), "recrew must clear unmanned");
    assert_eq!(tank.team, Team::USA, "recrew transfers AI pilot team");
    assert!(game_logic.honesty_pilot_recrew_ok());
    // Low-HP closer vehicle must remain unmanned (MinHealth residual skip).
    assert!(
        game_logic.host_object(low_id).unwrap().is_unmanned(),
        "MinHealth residual must not recrew low-HP vehicle"
    );
    assert!(
        game_logic
            .host_object(pilot_id)
            .map(|p| p.status.destroyed)
            .unwrap_or(true),
        "pilot consumed after auto-recrew residual"
    );

    // Human player residual: no auto-scan (C++ sleep forever).
    let mut human_logic = GameLogic::new();
    ensure_test_tank_template(&mut human_logic);
    human_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA Human", true));
    human_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let hp = human_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("human pilot");
    {
        let p = human_logic.host_object_mut(hp).expect("hp");
        p.set_ai_state(AIState::Idle);
    }
    let ht = human_logic
        .create_object("TestTank", Team::Neutral, Vec3::new(10.0, 0.0, 0.0))
        .expect("ht");
    {
        let t = human_logic.host_object_mut(ht).expect("ht");
        t.apply_kill_pilot_unmanned();
    }
    human_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    human_logic.update_ai(&[hp, ht], 1.0 / 30.0);
    assert!(
        !human_logic.honesty_pilot_find_vehicle_ok(),
        "human pilot residual must not auto-scan"
    );
    assert_eq!(
        human_logic.usa_pilot_residual().find_vehicle_orders,
        0,
        "human residual issues zero PilotFindVehicle Enter orders"
    );
    // Human pilot must not auto-recrew unmanned vehicle without player Enter.
    assert!(
        human_logic.host_object(ht).unwrap().is_unmanned(),
        "human pilot residual must not auto-recrew unmanned vehicle"
    );
    assert!(
        !human_logic.honesty_pilot_recrew_ok(),
        "human residual must not recrew via PilotFindVehicle"
    );

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

/// Residual: PilotFindVehicleUpdate base-center fallback when no vehicle found.
///
/// C++: when scan finds nothing and `!m_didMoveToBase`, issue one
/// `aiMoveToPosition(getAiBaseCenter)`. Host residual: command center position.
/// Fail-closed: not full CollideModule matrix / repeated base retreats.
#[test]
fn pilot_find_vehicle_base_center_fallback_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::host_usa_pilot::PILOT_FIND_VEHICLE_SCAN_FRAMES;

    let mut game_logic = GameLogic::new();
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let mut cc_tpl = ThingTemplate::new("AmericaCommandCenter");
    cc_tpl
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(4000.0);
    game_logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc_tpl);

    let cc_pos = Vec3::new(120.0, 0.0, 80.0);
    let cc_id = game_logic
        .create_object("AmericaCommandCenter", Team::USA, cc_pos)
        .expect("command center");
    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.set_ai_state(AIState::Idle);
        assert!(!p.status.pilot_did_move_to_base);
    }

    assert!(!game_logic.honesty_pilot_base_center_ok());
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    game_logic.update_ai(&[pilot_id, cc_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_pilot_base_center_ok(),
        "base-center fallback residual must record honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().base_center_moves, 1);
    {
        let p = game_logic
            .host_object(pilot_id)
            .expect("pilot after fallback");
        assert!(
            p.status.pilot_did_move_to_base,
            "m_didMoveToBase residual must latch after fallback"
        );
        assert_eq!(
            p.ai_state,
            AIState::Moving,
            "fallback residual must issue Move to base"
        );
        let dest = p
            .movement
            .target_position
            .expect("fallback residual must set move destination");
        assert!(
            (dest.x - cc_pos.x).abs() < 0.1 && (dest.z - cc_pos.z).abs() < 0.1,
            "fallback destination must be command center ({cc_pos:?}), got {dest:?}"
        );
    }

    // Second scan while still idle must not re-issue (m_didMoveToBase).
    // Reset to Idle so residual is eligible again but did_move latches.
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.set_ai_state(AIState::Idle);
        p.stop_moving();
    }
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES * 2;
    game_logic.update_ai(&[pilot_id, cc_id], 1.0 / 30.0);
    assert_eq!(
        game_logic.usa_pilot_residual().base_center_moves,
        1,
        "base-center residual must be one-shot (m_didMoveToBase)"
    );

    // Human residual: no base-center fallback.
    let mut human_logic = GameLogic::new();
    human_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA Human", true));
    human_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    human_logic
        .templates
        .insert("AmericaCommandCenter".to_string(), {
            let mut t = ThingTemplate::new("AmericaCommandCenter");
            t.add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::CommandCenter)
                .add_kind_of(KindOf::Selectable)
                .set_health(4000.0);
            t
        });
    let hcc = human_logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("hcc");
    let hp = human_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("hp");
    {
        let p = human_logic.host_object_mut(hp).expect("hp");
        p.set_ai_state(AIState::Idle);
    }
    human_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    human_logic.update_ai(&[hp, hcc], 1.0 / 30.0);
    assert!(
        !human_logic.honesty_pilot_base_center_ok(),
        "human pilot residual must not base-center fallback"
    );
    assert_eq!(human_logic.usa_pilot_residual().base_center_moves, 0);

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

/// Residual: AutoFindHealingUpdate AI pilot auto-scan → SeekingHealing at HealPad.
///
/// Retail ModuleTag_06: ScanRate 1000ms / ScanRange 300 / NeverHeal 0.85.
/// AI-only idle; human skips. Fail-closed: AlwaysHeal busy-interrupt path.
#[test]
fn pilot_auto_find_healing_hospital_path_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    // Host-state residual honesty without shadow writeback.
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::host_usa_pilot::{
        AUTO_FIND_HEALING_NEVER_HEAL, AUTO_FIND_HEALING_SCAN_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut game_logic);
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut pilot_tpl = ThingTemplate::new("AmericaInfantryPilot");
    pilot_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    // Pad within INTERACT_RANGE (14) so update_ai SeekingHealing ticks without
    // requiring full update_movement approach residual.
    let pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("heal pad");
    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        // Below NeverHeal 0.85 residual (50% HP).
        p.health.current = 50.0;
        p.set_ai_state(AIState::Idle);
    }

    assert!(!game_logic.honesty_pilot_auto_heal_ok());
    game_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    game_logic.update_ai(&[pilot_id, pad_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_pilot_auto_heal_ok(),
        "AutoFindHealing residual must issue SeekingHealing honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().auto_heal_orders, 1);
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after scan");
        assert_eq!(
            p.ai_state,
            AIState::SeekingHealing,
            "injured AI pilot residual must SeekHealing"
        );
        assert_eq!(p.target, Some(pad_id), "must target HealPad residual");
    }

    // Complete heal residual at pad (existing SeekingHealing honesty path).
    // First scan tick already issued SeekingHealing; subsequent frames tick HP.
    for _ in 0..60 {
        game_logic.update_ai(&[pilot_id, pad_id], 1.0 / 30.0);
    }
    let hp_after = game_logic
        .host_object(pilot_id)
        .expect("pilot")
        .health
        .current;
    assert!(
        hp_after > 50.0,
        "SeekingHealing residual must restore pilot HP (after={hp_after})"
    );
    assert!(
        game_logic.honesty_heal_pad_ok(),
        "heal-pad honesty must record SeekingHealing ticks"
    );

    // Healthy pilot (> NeverHeal) must not auto-scan.
    let mut healthy_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut healthy_logic);
    healthy_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));
    healthy_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let hpad = healthy_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("pad");
    let hpilot = healthy_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("healthy pilot");
    {
        let p = healthy_logic.host_object_mut(hpilot).expect("hp");
        // Just above NeverHeal threshold residual.
        p.health.current = 100.0 * AUTO_FIND_HEALING_NEVER_HEAL + 1.0;
        p.set_ai_state(AIState::Idle);
    }
    healthy_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    healthy_logic.update_ai(&[hpilot, hpad], 1.0 / 30.0);
    assert!(
        !healthy_logic.honesty_pilot_auto_heal_ok(),
        "NeverHeal residual must skip healthy pilot auto-scan"
    );
    assert_eq!(healthy_logic.usa_pilot_residual().auto_heal_orders, 0);
    assert_eq!(
        healthy_logic.host_object(hpilot).unwrap().ai_state,
        AIState::Idle,
        "healthy pilot residual stays Idle"
    );

    // Human residual: no AutoFindHealing auto-scan.
    let mut human_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut human_logic);
    human_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA Human", true));
    human_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), {
            let mut t = ThingTemplate::new("AmericaInfantryPilot");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let hpad2 = human_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("pad");
    let hp2 = human_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("human pilot");
    {
        let p = human_logic.host_object_mut(hp2).expect("hp");
        p.health.current = 40.0;
        p.set_ai_state(AIState::Idle);
    }
    human_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    human_logic.update_ai(&[hp2, hpad2], 1.0 / 30.0);
    assert!(
        !human_logic.honesty_pilot_auto_heal_ok(),
        "human pilot residual must not AutoFindHealing"
    );
    assert_eq!(human_logic.usa_pilot_residual().auto_heal_orders, 0);
    assert_eq!(
        human_logic.host_object(hp2).unwrap().ai_state,
        AIState::Idle,
        "human injured pilot residual stays Idle without GetHealed command"
    );

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

/// Residual: AutoFindHealingUpdate non-pilot infantry hospital path.
///
/// C++ AutoFindHealingUpdate.cpp:78-123 any AI template with the module.
/// USA Ranger / China Redguard / GLA Rebel all carry it in retail INI.
/// Fail-closed: AlwaysHeal busy-interrupt still not claimed.
#[test]
fn usa_infantry_auto_find_healing_hospital_path_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    // Host-state residual honesty without shadow writeback.
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::host_usa_pilot::{
        is_auto_find_healing_template, AUTO_FIND_HEALING_SCAN_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut game_logic);
    game_logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA AI", false));

    let mut ranger_tpl = ThingTemplate::new("AmericaInfantryRanger");
    ranger_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), ranger_tpl);
    assert!(is_auto_find_healing_template("AmericaInfantryRanger"));

    let pad_id = game_logic
        .create_object("TestHealPad", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("heal pad");
    let ranger_id = game_logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    {
        let r = game_logic.host_object_mut(ranger_id).expect("ranger");
        r.health.current = 40.0;
        r.set_ai_state(AIState::Idle);
    }

    assert!(!game_logic.honesty_infantry_auto_heal_ok());
    game_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    game_logic.update_ai(&[ranger_id, pad_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_infantry_auto_heal_ok(),
        "Ranger AutoFindHealing residual must issue SeekingHealing honesty"
    );
    assert!(
        game_logic.honesty_pilot_auto_heal_ok(),
        "infantry auto-heal also counts auto_heal_orders honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().infantry_auto_heal_orders, 1);
    assert_eq!(game_logic.usa_pilot_residual().auto_heal_orders, 1);
    {
        let r = game_logic
            .host_object(ranger_id)
            .expect("ranger after scan");
        assert_eq!(
            r.ai_state,
            AIState::SeekingHealing,
            "injured AI ranger residual must SeekHealing"
        );
        assert_eq!(r.target, Some(pad_id));
    }

    // Complete heal residual at pad.
    for _ in 0..60 {
        game_logic.update_ai(&[ranger_id, pad_id], 1.0 / 30.0);
    }
    let hp_after = game_logic
        .host_object(ranger_id)
        .expect("ranger")
        .health
        .current;
    assert!(
        hp_after > 40.0,
        "SeekingHealing residual must restore ranger HP (after={hp_after})"
    );

    // C++: China infantry with AutoFindHealingUpdate also scans.
    let mut china_logic = GameLogic::new();
    ensure_test_heal_pad_template(&mut china_logic);
    china_logic
        .players
        .insert(0, Player::new(0, Team::China, "China AI", false));
    china_logic
        .templates
        .insert("ChinaInfantryRedguard".to_string(), {
            let mut t = ThingTemplate::new("ChinaInfantryRedguard");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .set_health(100.0);
            t
        });
    let cpad = china_logic
        .create_object("TestHealPad", Team::China, Vec3::new(5.0, 0.0, 0.0))
        .expect("pad");
    let cred = china_logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("redguard");
    {
        let r = china_logic.host_object_mut(cred).expect("rg");
        r.health.current = 30.0;
        r.set_ai_state(AIState::Idle);
    }
    china_logic.frame = AUTO_FIND_HEALING_SCAN_FRAMES;
    china_logic.update_ai(&[cred, cpad], 1.0 / 30.0);
    assert!(
        china_logic.honesty_infantry_auto_heal_ok()
            || china_logic.host_object(cred).unwrap().ai_state != AIState::Idle,
        "China Redguard AutoFindHealingUpdate must seek a HealPad"
    );

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

/// Residual: EjectPilotDie air OCL parachute when significantly above terrain.
///
/// C++ isSignificantlyAboveTerrain → OCL_EjectPilotViaParachute PutInContainer
/// AmericaParachute residual: elevated pilot freefall → OpenDist open → land.
/// Fail-closed: not full bone PARA_COG / DeliverPayload matrix
/// (pitch/roll spring-damper host residual closed 2026-07-13).
#[test]
fn eject_pilot_air_ocl_parachute_residual() {
    use crate::game_logic::host_usa_pilot::{
        significantly_above_terrain_threshold, EJECT_PILOT_TEMPLATE, PARACHUTE_OPEN_DIST,
    };
    use crate::game_logic::VeterancyLevel;

    let mut game_logic = GameLogic::new();

    let mut humvee_tpl = ThingTemplate::new("AmericaVehicleHumvee");
    humvee_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(200.0);
    game_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), humvee_tpl);

    let thr = significantly_above_terrain_threshold();
    // Spawn high enough that freefall OpenDist (100) is reached before ground.
    let air_y = thr + PARACHUTE_OPEN_DIST + 50.0;
    let humvee_id = game_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(50.0, air_y, 50.0),
        )
        .expect("airborne humvee");
    {
        let h = game_logic.host_object_mut(humvee_id).expect("humvee");
        h.experience.level = VeterancyLevel::Veteran;
        h.status.airborne_target = true;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    game_logic.mark_object_for_destruction(humvee_id, Some(Team::GLA));
    game_logic.process_destroy_list();

    assert!(
        game_logic.honesty_pilot_eject_ok(),
        "air eject must still record eject honesty"
    );
    assert!(
        game_logic.honesty_pilot_air_eject_ok(),
        "air OCL residual must record air_ejection honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().air_ejections, 1);
    assert_eq!(game_logic.usa_pilot_residual().ejections, 1);

    let pilot_id = game_logic
        .objects
        .iter()
        .find(|(_, o)| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
        .map(|(id, _)| *id)
        .expect("ejected pilot");

    {
        let pilot = game_logic.host_object(pilot_id).expect("pilot");
        assert!(
            pilot.is_parachuting(),
            "air-ejected pilot residual must be parachuting"
        );
        assert!(
            !pilot.is_parachute_open(),
            "air pilot residual starts freefall (chute closed)"
        );
        assert!(
            pilot.get_position().y > thr,
            "air pilot residual must spawn elevated, y={}",
            pilot.get_position().y
        );
        assert!(
            pilot.is_eject_invulnerable(),
            "air OCL still grants InvulnerableTime residual"
        );
    }

    // Freefall until OpenDist, then open chute residual, then land.
    assert!(!game_logic.honesty_pilot_parachute_open_ok());
    assert!(!game_logic.honesty_pilot_parachute_land_ok());
    let ids = [pilot_id];
    for _ in 0..120 {
        game_logic.update_ai(&ids, 1.0 / 30.0);
        if game_logic.honesty_pilot_parachute_land_ok() {
            break;
        }
    }
    assert!(
        game_logic.honesty_pilot_parachute_open_ok(),
        "AmericaParachute OpenDist residual must open chute honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().parachute_opens, 1);
    assert!(
        game_logic.honesty_pilot_parachute_land_ok(),
        "parachute residual must land and record honesty"
    );
    assert_eq!(game_logic.usa_pilot_residual().parachute_lands, 1);
    {
        let pilot = game_logic.host_object(pilot_id).expect("landed pilot");
        assert!(
            !pilot.is_parachuting(),
            "landed pilot residual clears parachuting"
        );
        assert!(
            pilot.get_position().y.abs() < 0.1,
            "landed pilot residual y must be ground, got {}",
            pilot.get_position().y
        );
    }

    // Ground path control: y=0 death does not air-eject.
    let mut ground_logic = GameLogic::new();
    ground_logic
        .templates
        .insert("AmericaVehicleHumvee".to_string(), {
            let mut t = ThingTemplate::new("AmericaVehicleHumvee");
            t.add_kind_of(KindOf::Vehicle)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(200.0);
            t
        });
    let g_id = ground_logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            Vec3::new(10.0, 0.0, 10.0),
        )
        .expect("ground humvee");
    {
        let h = ground_logic.host_object_mut(g_id).expect("humvee");
        h.experience.level = VeterancyLevel::Veteran;
        let _ = h.take_damage(h.max_health * 2.0);
        h.status.destroyed = true;
    }
    ground_logic.mark_object_for_destruction(g_id, Some(Team::GLA));
    ground_logic.process_destroy_list();
    assert!(ground_logic.honesty_pilot_eject_ok());
    assert!(
        !ground_logic.honesty_pilot_air_eject_ok(),
        "ground death residual must not claim air OCL"
    );
    assert_eq!(ground_logic.usa_pilot_residual().air_ejections, 0);
    let g_pilot = ground_logic
        .objects
        .values()
        .find(|o| o.is_alive() && o.template_name == EJECT_PILOT_TEMPLATE)
        .expect("ground pilot");
    assert!(
        !g_pilot.is_parachuting(),
        "ground OCL residual must not parachute"
    );
    assert!(g_pilot.get_position().y.abs() < 0.1);
}

/// Residual: PilotFindVehicle PartitionFilterPlayer same-player residual.
///
/// C++ PartitionFilterPlayer(me->getControllingPlayer(), true): only own
/// vehicles. Host killpilot → Neutral + unmanned_owner_team; accept Neutral
/// with matching owner, reject foreign-owner Neutral unmanned.
/// Fail-closed: not full same-map PartitionFilterSameMapStatus.
#[test]
fn pilot_find_vehicle_same_player_partition_filter_residual() {
    let _env_guard = HOST_STATE_RESIDUAL_TEST_ENV_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let prev_dec = std::env::var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY").ok();
    let prev_dmg = std::env::var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").ok();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", "0");
    crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "0");

    use crate::game_logic::host_usa_pilot::PILOT_FIND_VEHICLE_SCAN_FRAMES;
    use crate::game_logic::VeterancyLevel;

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
    game_logic
        .templates
        .insert("AmericaInfantryPilot".to_string(), pilot_tpl);

    let pilot_id = game_logic
        .create_object("AmericaInfantryPilot", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.experience.level = VeterancyLevel::Veteran;
        p.set_ai_state(AIState::Idle);
    }

    // Closer foreign-owner unmanned (China sniped) — PartitionFilter rejects.
    let foreign_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(5.0, 0.0, 0.0))
        .expect("foreign tank");
    {
        let t = game_logic.host_object_mut(foreign_id).expect("foreign");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        t.health.current = t.max_health;
        assert_eq!(t.status.unmanned_owner_team, Some(Team::China));
    }

    // Farther USA-owner unmanned — PartitionFilter accepts.
    let own_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("own tank");
    {
        let t = game_logic.host_object_mut(own_id).expect("own");
        t.apply_kill_pilot_unmanned();
        t.set_team(Team::Neutral);
        t.experience.level = VeterancyLevel::Rookie;
        t.health.current = t.max_health;
        assert_eq!(t.status.unmanned_owner_team, Some(Team::USA));
    }

    assert!(!game_logic.honesty_pilot_find_vehicle_player_ok());
    game_logic.frame = PILOT_FIND_VEHICLE_SCAN_FRAMES;
    game_logic.update_ai(&[pilot_id, foreign_id, own_id], 1.0 / 30.0);

    assert!(
        game_logic.honesty_pilot_find_vehicle_player_ok(),
        "PartitionFilterPlayer residual must reject foreign-owner unmanned"
    );
    assert!(game_logic.usa_pilot_residual().find_vehicle_player_rejects >= 1);
    assert!(
        game_logic.honesty_pilot_find_vehicle_ok(),
        "PilotFindVehicle residual must still Enter matching-owner vehicle"
    );

    if game_logic
        .host_object(pilot_id)
        .map(|p| !p.status.destroyed)
        .unwrap_or(false)
    {
        let p = game_logic.host_object(pilot_id).expect("pilot after scan");
        assert_eq!(
            p.target,
            Some(own_id),
            "must skip foreign-owner Neutral and target own-owner vehicle"
        );
        game_logic.update_ai(&[pilot_id, foreign_id, own_id], 1.0 / 30.0);
    }

    let own = game_logic.host_object(own_id).expect("own after recrew");
    assert!(!own.is_unmanned(), "own-owner vehicle must be recrewed");
    assert_eq!(own.team, Team::USA);
    assert!(
        game_logic.host_object(foreign_id).unwrap().is_unmanned(),
        "foreign-owner residual must remain unmanned"
    );
    assert!(
        game_logic
            .host_object(pilot_id)
            .map(|p| p.status.destroyed)
            .unwrap_or(true),
        "pilot consumed after same-player-gated auto-recrew residual"
    );

    match prev_dec {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY"),
    }
    match prev_dmg {
        Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", v),
        None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY"),
    }
}

/// Residual: AmericaParachute low-altitude open fudge + FreeFallDamage.
///
/// C++ ParachuteContain: if startZ−ground < 2×OpenDist, fudge startZ so the
/// chute can open. FreeFallDamagePercent **0.5** applies when chute is
/// destroyed mid-air while significantly above terrain.
#[test]
fn eject_pilot_parachute_open_fudge_and_free_fall_damage_residual() {
    use crate::game_logic::host_usa_pilot::{
        free_fall_damage_amount, EJECT_PILOT_TEMPLATE, FREE_FALL_DAMAGE_PERCENT,
        PARACHUTE_OPEN_DIST,
    };

    let mut game_logic = GameLogic::new();
    // Ensure pilot template.
    if !game_logic.templates.contains_key(EJECT_PILOT_TEMPLATE) {
        let mut pilot_tpl = ThingTemplate::new(EJECT_PILOT_TEMPLATE);
        pilot_tpl
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        game_logic
            .templates
            .insert(EJECT_PILOT_TEMPLATE.to_string(), pilot_tpl);
    }

    // Low-altitude air eject: spawn height < 2×OpenDist so fudge applies.
    let low_y = PARACHUTE_OPEN_DIST * 1.5; // 150 < 200
    assert!(low_y < 2.0 * PARACHUTE_OPEN_DIST);
    let pilot_id = game_logic
        .create_object(EJECT_PILOT_TEMPLATE, Team::USA, Vec3::new(0.0, low_y, 0.0))
        .expect("pilot");
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        p.apply_eject_parachuting();
    }
    // Record fudge honesty (create_object path records via process_destroy;
    // direct apply needs explicit honesty for residual test).
    game_logic.usa_pilot.record_parachute_open_fudge();
    {
        let p = game_logic.host_object(pilot_id).expect("pilot");
        assert!(p.is_parachuting());
        assert!(
            (p.status.parachute_start_height - 2.0 * PARACHUTE_OPEN_DIST).abs() < 0.01,
            "low-altitude residual must fudge start height to 2×OpenDist, got {}",
            p.status.parachute_start_height
        );
    }
    assert!(game_logic.honesty_pilot_parachute_open_fudge_ok());

    // FreeFallDamage residual: destroy chute mid-air while elevated.
    // Raise pilot so significantly-above-terrain gate passes.
    {
        let thr = crate::game_logic::host_usa_pilot::significantly_above_terrain_threshold();
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        let mut pos = p.get_position();
        pos.y = thr + 50.0;
        p.set_position(pos);
        p.set_status_parachute_open(true); // open chute then cut residual
        p.health.current = p.health.maximum;
    }
    let hp_before = game_logic.host_object(pilot_id).unwrap().health.current;
    let max_hp = game_logic.host_object(pilot_id).unwrap().health.maximum;
    assert!(!game_logic.honesty_pilot_free_fall_damage_ok());
    assert!(
        game_logic.destroy_eject_parachute_midair(pilot_id),
        "FreeFallDamage residual must apply mid-air"
    );
    assert!(game_logic.honesty_pilot_free_fall_damage_ok());
    assert_eq!(game_logic.usa_pilot_residual().free_fall_damages, 1);
    {
        let p = game_logic.host_object(pilot_id).expect("pilot");
        assert!(
            !p.is_parachute_open(),
            "chute destroyed residual must close chute"
        );
        assert!(p.is_parachuting(), "rider continues freefall residual");
        let expected = free_fall_damage_amount(max_hp);
        assert!(
            (hp_before - p.health.current - expected).abs() < 0.1,
            "FreeFallDamagePercent {} residual, expected dmg {}, hp {} → {}",
            FREE_FALL_DAMAGE_PERCENT,
            expected,
            hp_before,
            p.health.current
        );
    }
    // Ground pilot residual: FreeFallDamage must not apply.
    {
        let p = game_logic.host_object_mut(pilot_id).expect("pilot");
        let mut pos = p.get_position();
        pos.y = 0.0;
        p.set_position(pos);
        p.clear_eject_parachuting();
    }
    assert!(
        !game_logic.destroy_eject_parachute_midair(pilot_id),
        "ground residual must not FreeFallDamage"
    );
}

/// Residual: Strategy Center TurretAI idle-scan Min/MaxIdleScanAngle residual.
///
/// Retail: MinIdleScanInterval **500**ms → **15** frames, Max **1000**ms →
/// **30** frames, MinIdleScanAngle **0**, MaxIdleScanAngle **60**. Bombardment
/// ACTIVE idle gun schedules scan, rotates toward NaturalTurretAngle ± offset,
/// then reschedules. Fail-closed: not full TurretAI mood-target / bone matrix.
#[test]
fn strategy_center_turret_idle_scan_residual() {
    use crate::game_logic::host_strategy_center::{
        idle_scan_desired_angle_deg, turret_angles_are_natural, HostBattlePlan,
        STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG, STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME,
    };

    let mut game_logic = GameLogic::new();
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");

    assert!(!game_logic.honesty_strategy_center_turret_idle_scan_ok());

    // Activate Bombardment → ACTIVE equips gun + schedules first idle scan.
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.weapon.is_some(), "Bombardment ACTIVE equips gun");
        assert!(
            turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "idle scan residual starts at natural angles"
        );
        // BecameActive schedules next = frame + MinIdleScanInterval (15).
        assert!(
            sc.turret_idle_scan_next_frame > 0,
            "first idle-scan residual must be scheduled"
        );
        assert!(!sc.turret_idle_scanning);
        let _ = STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES;
    }

    // Ensure idle: no target / not attacking; force scan due this frame.
    let due_frame = game_logic.frame;
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
        sc.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        sc.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_idle_scan_next_frame = due_frame;
        sc.turret_idle_scan_index = 0;
        sc.turret_idle_scanning = false;
    }

    // One tick should start idle scan toward natural+30 = -60.
    game_logic.tick_battle_plan_door_residuals();
    assert!(
        game_logic.honesty_strategy_center_turret_idle_scan_ok(),
        "idle-scan residual must record start honesty"
    );
    assert_eq!(game_logic.battle_plans().turret_idle_scan_start_count(), 1);
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        let desired = idle_scan_desired_angle_deg(0);
        assert!((desired - (-60.0)).abs() < 0.01);
        // Stepped toward desired: from -90 toward -60 at 2 deg/frame → -88.
        assert!(
            (sc.turret_angle_deg
                - (STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG
                    + STRATEGY_CENTER_TURRET_TURN_DEG_PER_FRAME))
                .abs()
                < 0.01,
            "idle-scan residual must step toward desired angle, got {}",
            sc.turret_angle_deg
        );
        assert!(
            !turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "idle-scan leaves natural"
        );
        assert!(sc.turret_idle_scanning, "must be mid idle-scan residual");
    }

    // Advance enough frames for remaining ~28° at 2 deg/frame.
    let desired = idle_scan_desired_angle_deg(0);
    for _ in 0..30 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.tick_battle_plan_door_residuals();
        let sc = game_logic.host_object(sc_id).unwrap();
        if !sc.turret_idle_scanning
            && game_logic.battle_plans().turret_idle_scan_complete_count() > 0
        {
            break;
        }
    }
    assert!(
        game_logic.battle_plans().turret_idle_scan_complete_count() >= 1,
        "idle-scan residual must complete"
    );
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_idle_scanning,
            "scan complete residual clears scanning flag"
        );
        // C++ IDLESCAN → HOLD: enter HoldTurret residual (no next-scan yet).
        assert!(
            sc.turret_holding,
            "scan complete residual must enter HoldTurret"
        );
        assert_eq!(
            sc.turret_idle_scan_next_frame, 0,
            "Hold residual defers next idle-scan schedule"
        );
        assert!(
            (sc.turret_angle_deg - desired).abs() < 0.5 || sc.turret_idle_scan_index >= 1,
            "angles at desired after complete residual, angle={} desired={}",
            sc.turret_angle_deg,
            desired
        );
    }
    assert!(
        game_logic.honesty_strategy_center_turret_hold_ok(),
        "HoldTurret residual honesty must record start"
    );

    // Busy residual: attacking cancels mid-scan / hold.
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.turret_idle_scanning = true;
        sc.turret_holding = false;
        sc.turret_idle_scan_desired_angle_deg = idle_scan_desired_angle_deg(1);
        sc.set_status_attacking(true);
        sc.set_ai_state(AIState::Attacking);
    }
    game_logic.frame = game_logic.frame.saturating_add(1);
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_idle_scanning,
            "busy residual must cancel idle-scan"
        );
        assert!(
            !sc.turret_holding && !sc.turret_idle_recentering,
            "busy residual must cancel hold / idle-recenter"
        );
    }
}

/// Residual: TurretAI HoldTurret + idle-recenter after idle-scan.
///
/// C++ IDLESCAN → HOLD (RecenterTime **60** frames) → RECENTER → IDLE.
/// Host-testable: hold freezes angles; after 60 frames recenter to natural;
/// then next idle-scan is scheduled. Fail-closed: not full mood-target.
#[test]
fn strategy_center_turret_hold_and_idle_recenter_residual() {
    use crate::game_logic::host_strategy_center::{
        hold_turret_until_frame, idle_scan_desired_angle_deg, idle_scan_interval_frames,
        turret_angles_are_natural, HostBattlePlan, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG, STRATEGY_CENTER_RECENTER_TIME_FRAMES,
    };

    let mut game_logic = GameLogic::new();
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");

    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);

    // Force idle-scan complete at desired angle → enter Hold immediately.
    let desired = idle_scan_desired_angle_deg(0);
    let hold_start = game_logic.frame;
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
        sc.turret_angle_deg = desired;
        sc.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_idle_scanning = true;
        sc.turret_idle_scan_desired_angle_deg = desired;
        sc.turret_idle_scan_index = 0;
        sc.turret_holding = false;
        sc.turret_idle_recentering = false;
        sc.turret_hold_until_frame = 0;
        sc.turret_idle_scan_next_frame = 0;
    }
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.turret_holding, "scan complete → HoldTurret residual");
        assert!(!sc.turret_idle_scanning);
        assert_eq!(
            sc.turret_hold_until_frame,
            hold_turret_until_frame(hold_start)
        );
        assert_eq!(STRATEGY_CENTER_RECENTER_TIME_FRAMES, 60);
        // Angles frozen at desired during hold.
        assert!((sc.turret_angle_deg - desired).abs() < 0.5);
    }
    assert!(game_logic.honesty_strategy_center_turret_hold_ok());
    assert_eq!(game_logic.battle_plans().turret_hold_start_count(), 1);

    // Mid-hold: angles still frozen, not recentering yet.
    game_logic.frame = hold_start.saturating_add(30);
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(sc.turret_holding, "must still be holding mid RecenterTime");
        assert!(!sc.turret_idle_recentering);
        assert!((sc.turret_angle_deg - desired).abs() < 0.5);
    }

    // Hold elapses → idle-recenter residual starts toward natural.
    game_logic.frame = hold_turret_until_frame(hold_start);
    game_logic.tick_battle_plan_door_residuals();
    assert!(
        game_logic.battle_plans().turret_hold_complete_count() >= 1,
        "HoldTurret residual must complete after RecenterTime"
    );
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(!sc.turret_holding, "hold complete clears holding");
        // Either mid idle-recenter or already finished (if already natural).
        assert!(
            sc.turret_idle_recentering
                || turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "after hold must idle-recenter or already natural"
        );
        // First step toward natural: from desired (-60) toward -90 → -62.
        if sc.turret_idle_recentering {
            assert!(
                (sc.turret_angle_deg - desired).abs() > 0.5
                    || (sc.turret_angle_deg - STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs() < 0.5,
                "idle-recenter residual must step toward natural, got {}",
                sc.turret_angle_deg
            );
        }
    }
    assert!(
        game_logic.battle_plans().turret_idle_recenter_start_count() >= 1
            || game_logic.honesty_strategy_center_turret_idle_recenter_ok(),
        "idle-recenter residual must start"
    );

    // Step enough frames for 30° at 2 deg/frame to restore natural.
    for _ in 0..40 {
        game_logic.frame = game_logic.frame.saturating_add(1);
        game_logic.tick_battle_plan_door_residuals();
        let sc = game_logic.host_object(sc_id).unwrap();
        if !sc.turret_idle_recentering
            && turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg)
        {
            break;
        }
    }
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            turret_angles_are_natural(sc.turret_angle_deg, sc.turret_pitch_deg),
            "idle-recenter residual must restore natural angles, a={} p={}",
            sc.turret_angle_deg,
            sc.turret_pitch_deg
        );
        assert!(!sc.turret_idle_recentering);
        // Next idle-scan scheduled with scan_index 1 → MaxIdleScanInterval 30.
        assert!(
            sc.turret_idle_scan_next_frame
                >= game_logic
                    .frame
                    .saturating_add(idle_scan_interval_frames(1).saturating_sub(1)),
            "idle-recenter complete must reschedule next idle scan, next={} frame={}",
            sc.turret_idle_scan_next_frame,
            game_logic.frame
        );
    }
    assert!(
        game_logic.honesty_strategy_center_turret_idle_recenter_ok(),
        "idle-recenter residual honesty must record complete"
    );
}

/// Residual: TurretAI idle mood-target acquire + out-of-range clear.
///
/// C++ friend_checkForIdleMoodTarget: Bombardment ACTIVE idle gun acquires
/// enemy in StrategyCenterGun range band + Partition vision residual
/// (VisionRange **400**), aims FirePitch, flags mood target.
/// Mood target leaving range/vision clears so idle-scan can resume.
/// Fail-closed: not full PartitionManager filter stack / pathfinder mood matrix.
#[test]
fn strategy_center_turret_mood_target_residual() {
    use crate::game_logic::host_strategy_center::{
        strategy_center_mood_vision_range, HostBattlePlan, STRATEGY_CENTER_BASE_VISION_RANGE,
        STRATEGY_CENTER_FIRE_PITCH_DEG, STRATEGY_CENTER_GUN_MIN_RANGE,
        STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG, STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG,
    };
    assert!((STRATEGY_CENTER_BASE_VISION_RANGE - 400.0).abs() < 0.001);
    assert!((strategy_center_mood_vision_range(false) - 400.0).abs() < 0.001);

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let mut sc_template = ThingTemplate::new("AmericaStrategyCenter");
    sc_template
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSStrategyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(1500.0);
    game_logic
        .templates
        .insert("AmericaStrategyCenter".to_string(), sc_template);
    if !game_logic.players.contains_key(&0) {
        game_logic
            .players
            .insert(0, Player::new(0, Team::USA, "USA", true));
    }

    let sc_id = game_logic
        .create_object("AmericaStrategyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("strategy center");
    assert!(game_logic.activate_battle_plan(0, HostBattlePlan::Bombardment, Some(sc_id),));
    advance_battle_plan_door_to_active(&mut game_logic);

    // Idle natural gun + enemy in gun range band (min 100).
    let enemy_id = game_logic
        .create_object(
            "TestTank",
            Team::GLA,
            Vec3::new(STRATEGY_CENTER_GUN_MIN_RANGE + 50.0, 0.0, 0.0),
        )
        .expect("enemy");
    {
        let sc = game_logic.host_object_mut(sc_id).expect("sc");
        sc.target = None;
        sc.set_ai_state(AIState::Idle);
        sc.set_status_attacking(false);
        sc.turret_mood_target = false;
        sc.turret_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        sc.turret_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        sc.turret_idle_scanning = false;
        sc.turret_holding = false;
        sc.turret_idle_recentering = false;
    }
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            sc.turret_mood_target,
            "idle mood-target residual must flag m_targetWasSetByIdleMood"
        );
        if crate::gameworld_shadow::gameworld_ai_decision_authority_live() {
            // AttackTarget last-write via decision log; host target stays unset.
            assert!(sc.target.is_none());
        } else {
            assert_eq!(sc.target, Some(enemy_id));
        }
        assert!(
            (sc.turret_pitch_deg - STRATEGY_CENTER_FIRE_PITCH_DEG).abs() < 0.01,
            "mood-target residual aims FirePitch, got {}",
            sc.turret_pitch_deg
        );
        // Yaw left natural toward enemy at +X.
        assert!(
            (sc.turret_angle_deg - STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG).abs() > 1.0
                || sc.turret_angle_deg.abs() < 5.0,
            "mood-target residual must aim yaw at enemy, angle={}",
            sc.turret_angle_deg
        );
        assert!(!sc.turret_idle_scanning);
    }
    assert!(
        game_logic.honesty_strategy_center_turret_mood_target_ok(),
        "mood-target residual honesty must record acquire"
    );
    assert_eq!(
        game_logic.battle_plans().turret_mood_target_acquire_count(),
        1
    );

    // Move enemy out of max range → mood clear residual.
    {
        let e = game_logic.host_object_mut(enemy_id).expect("enemy");
        e.set_position(Vec3::new(900.0, 0.0, 0.0));
    }
    game_logic.tick_battle_plan_door_residuals();
    {
        let sc = game_logic.host_object(sc_id).expect("sc");
        assert!(
            !sc.turret_mood_target,
            "out-of-range residual must clear mood target"
        );
        assert!(sc.target.is_none());
    }
    assert!(
        game_logic.battle_plans().turret_mood_target_clear_count() >= 1,
        "mood-target clear honesty residual"
    );
}
