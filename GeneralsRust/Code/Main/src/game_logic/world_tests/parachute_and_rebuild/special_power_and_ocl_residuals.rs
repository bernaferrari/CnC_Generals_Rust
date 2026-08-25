//! Behavior suite extracted from `parachute_and_rebuild`.
use super::*;

#[test]
fn tank_hunter_tnt_and_laser_howitzer_special_power_residuals() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_missile_defender::{
        is_missile_defender_template, missile_defender_laser_guided_weapon,
        missile_defender_primary_weapon,
    };
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::host_tank_hunter::{
        TNT_START_ABILITY_RANGE, is_tank_hunter_template, tnt_in_start_range,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::TankHunterTnt),
        Some("SPECIAL_TANKHUNTER_TNT_ATTACK")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::LaserGuidedHowitzer),
        Some("SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES")
    );
    assert!(is_tank_hunter_template("ChinaInfantryTankHunter"));
    assert!(tnt_in_start_range(TNT_START_ABILITY_RANGE));
    assert!(!tnt_in_start_range(TNT_START_ABILITY_RANGE + 1.0));

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut th = ThingTemplate::new("ChinaInfantryTankHunter");
    th.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("ChinaInfantryTankHunter".into(), th);
    let mut structure = ThingTemplate::new("GLATunnelNetwork");
    structure.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("GLATunnelNetwork".into(), structure);

    let src = logic
        .create_object(
            "ChinaInfantryTankHunter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("th");
    let tgt = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(3.0, 0.0, 0.0),
        )
        .expect("struct");
    // Direct plant path residual (in range).
    let planted = logic.place_timed_demo_charge(
        Team::China,
        glam::Vec3::new(3.0, 0.0, 0.0),
        Some(src),
        Some(tgt),
        None,
    );
    assert!(planted.is_some());

    // Laser howitzer shares MD laser residual path for laser-capable infantry.
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let md_id = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("md");
    if let Some(o) = logic.host_object_mut(md_id) {
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
    }
    let enemy = logic
        .create_object(
            "GLATunnelNetwork",
            Team::GLA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("e2");
    assert!(logic.activate_missile_defender_laser_guided(md_id, enemy));
    assert!(is_missile_defender_template(
        "AmericaInfantryMissileDefender"
    ));
    let _ = SpecialPowerType::TankHunterTnt;
    let _ = SpecialPowerType::LaserGuidedHowitzer;
}

#[test]
fn missile_defender_laser_guided_spawns_laser_beam_object() {
    use crate::game_logic::host_missile_defender::{
        LASER_GUIDED_ATTACH_BONE, LASER_GUIDED_BEAM_LIFETIME_FRAMES, LASER_GUIDED_SPECIAL_OBJECT,
        missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let mut tgt_t = ThingTemplate::new("AmericaTankCrusader");
    tgt_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("AmericaTankCrusader".into(), tgt_t);

    let shooter = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    {
        let o = logic.host_object_mut(shooter).unwrap();
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
    }
    let target = logic
        .create_object("AmericaTankCrusader", Team::GLA, Vec3::new(100.0, 0.0, 0.0))
        .unwrap();

    assert!(logic.activate_missile_defender_laser_guided(shooter, target));
    assert!(logic.honesty_missile_defender_laser_beam_ok());
    assert!(logic.missile_defender_laser_beams_spawned >= 1);
    let beam = logic
        .host_objects()
        .values()
        .find(|o| o.missile_defender_laser_beam)
        .expect("LaserBeam special object");
    assert_eq!(beam.template_name, LASER_GUIDED_SPECIAL_OBJECT);
    assert_eq!(beam.producer_id, Some(shooter));
    let bid = beam.id;
    // Prep window expiry destroys residual LaserBeam.
    logic.frame = logic
        .frame
        .saturating_add(LASER_GUIDED_BEAM_LIFETIME_FRAMES + 2);
    logic.update_missile_defender_laser_beam_objects();
    assert!(
        logic
            .host_object(bid)
            .map(|o| !o.is_alive() || o.status.destroyed)
            .unwrap_or(true)
    );
    let _ = LASER_GUIDED_ATTACH_BONE;
}

#[test]
fn missile_defender_laser_endpoints_follow_moving_target() {
    use crate::game_logic::host_missile_defender::{
        missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let mut tgt_t = ThingTemplate::new("AmericaTankCrusader");
    tgt_t.add_kind_of(KindOf::Vehicle).set_health(400.0);
    tgt_t.geometry_info.authored = true;
    tgt_t.geometry_info.height = 20.0;
    logic.templates.insert("AmericaTankCrusader".into(), tgt_t);

    let shooter = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            Vec3::new(0.0, 0.0, 0.0),
        )
        .unwrap();
    {
        let o = logic.host_object_mut(shooter).unwrap();
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
    }
    let target = logic
        .create_object("AmericaTankCrusader", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();

    assert!(logic.activate_missile_defender_laser_guided(shooter, target));
    let first = logic
        .weapon_lasers
        .iter()
        .find(|l| l.from_id == shooter && l.to_id == Some(target))
        .map(|l| l.to_pos())
        .expect("beam");

    if let Some(o) = logic.host_object_mut(target) {
        o.set_position(Vec3::new(140.0, 0.0, 30.0));
    }
    logic.update_leftover_laser_guided_channels(0.1);
    let second = logic
        .weapon_lasers
        .iter()
        .find(|l| l.from_id == shooter && l.to_id == Some(target))
        .map(|l| l.to_pos())
        .expect("beam after move");
    assert!(
        (second.0 - first.0).abs() > 1.0 || (second.2 - first.2).abs() > 1.0,
        "MD laser end must follow a moving target, got {first:?} then {second:?}"
    );
}

#[test]
fn lotus_disable_laser_endpoints_follow_moving_vehicle() {
    use crate::game_logic::host_hero_abilities::LOTUS_DISABLE_SPECIAL_OBJECT;
    use crate::game_logic::host_weapon_laser::ResidualWeaponLaser;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    let mut lotus = ThingTemplate::new("GLAInfantryBlackLotus");
    lotus.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("GLAInfantryBlackLotus".into(), lotus);
    let mut veh = ThingTemplate::new("AmericaTankCrusader");
    veh.add_kind_of(KindOf::Vehicle).set_health(400.0);
    veh.geometry_info.authored = true;
    veh.geometry_info.height = 16.0;
    logic.templates.insert("AmericaTankCrusader".into(), veh);

    let caster = logic
        .create_object("GLAInfantryBlackLotus", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let target = logic
        .create_object("AmericaTankCrusader", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let (from, to) = logic
        .special_ability_laser_endpoints(caster, target)
        .expect("endpoints");
    logic.weapon_lasers.push(ResidualWeaponLaser {
        laser_name: LOTUS_DISABLE_SPECIAL_OBJECT.to_string(),
        laser_bone_name: String::new(),
        from_id: caster,
        to_id: Some(target),
        from_x: from.x,
        from_y: from.y,
        from_z: from.z,
        to_x: to.x,
        to_y: to.y,
        to_z: to.z,
        expires_frame: logic.frame.saturating_add(180),
        scroll_offset: 0.0,
    });
    let first = logic.weapon_lasers[0].to_pos();
    if let Some(o) = logic.host_object_mut(target) {
        o.set_position(Vec3::new(90.0, 0.0, 25.0));
    }
    assert!(logic.reinit_special_ability_laser(caster, target, None));
    let second = logic.weapon_lasers[0].to_pos();
    assert!(
        (second.0 - first.0).abs() > 1.0 || (second.2 - first.2).abs() > 1.0,
        "Lotus BinaryDataStream end must follow the vehicle, got {first:?} then {second:?}"
    );
}

#[test]
fn missile_defender_laser_guided_special_locks_secondary() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_missile_defender::{
        LASER_GUIDED_START_ABILITY_RANGE, is_missile_defender_template,
        missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
    };
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::MissileDefenderLaserGuided),
        Some("SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES")
    );
    assert!(is_missile_defender_template(
        "AmericaInfantryMissileDefender"
    ));

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let mut enemy = ThingTemplate::new("GLAInfantryRebel");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), enemy);

    let src = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("md");
    if let Some(o) = logic.host_object_mut(src) {
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
        o.active_weapon_slot = 0;
    }
    let tgt = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(LASER_GUIDED_START_ABILITY_RANGE - 10.0, 0.0, 0.0),
        )
        .expect("enemy");

    assert!(logic.activate_missile_defender_laser_guided(src, tgt));
    logic.update_ai(&[src, tgt], 1.1);
    let o = logic.host_object(src).unwrap();
    assert_eq!(o.active_weapon_slot, 1);
    assert_eq!(o.target, Some(tgt));
    assert!(logic.missile_defender_residual_laser_specials >= 1);
    let _ = SpecialPowerType::MissileDefenderLaserGuided;
}

#[test]
fn missile_defender_laser_oor_click_approaches() {
    use crate::game_logic::host_hero_abilities::{LeftoverSaKind, LeftoverSaPhase};
    use crate::game_logic::host_missile_defender::{
        LASER_GUIDED_START_ABILITY_RANGE, missile_defender_laser_guided_weapon,
        missile_defender_primary_weapon,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let mut enemy = ThingTemplate::new("GLAInfantryRebel");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), enemy);
    let src = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("md");
    if let Some(o) = logic.host_object_mut(src) {
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
    }
    let tgt = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(LASER_GUIDED_START_ABILITY_RANGE + 80.0, 0.0, 0.0),
        )
        .expect("enemy");
    assert!(
        logic.activate_missile_defender_laser_guided(src, tgt),
        "OOR laser click must start approach"
    );
    let ch = logic
        .hero_abilities
        .leftover_channel(src)
        .copied()
        .expect("approach channel");
    assert_eq!(ch.kind, LeftoverSaKind::LaserGuided);
    assert_eq!(ch.phase, LeftoverSaPhase::Facing);
    assert_eq!(
        logic.missile_defender_laser_beams_spawned, 0,
        "beam waits for StartAbilityRange"
    );
    let md = logic.host_object(src).unwrap();
    assert_eq!(md.ai_state, AIState::SpecialAbility);
    assert!(
        md.movement.target_position.is_some() || md.requested_destination.is_some(),
        "must walk toward the laser target"
    );
}

#[test]
fn missile_defender_laser_persist_survives_attacking() {
    use crate::game_logic::host_hero_abilities::{LeftoverSaKind, LeftoverSaPhase};
    use crate::game_logic::host_missile_defender::{
        LASER_GUIDED_START_ABILITY_RANGE, missile_defender_laser_guided_weapon,
        missile_defender_primary_weapon,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let mut enemy = ThingTemplate::new("GLAInfantryRebel");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), enemy);
    let src = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("md");
    if let Some(o) = logic.host_object_mut(src) {
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
    }
    let tgt = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(LASER_GUIDED_START_ABILITY_RANGE - 10.0, 0.0, 0.0),
        )
        .expect("enemy");
    assert!(logic.activate_missile_defender_laser_guided(src, tgt));
    logic.update_ai(&[src, tgt], 1.1);
    assert!(logic.missile_defender_residual_laser_specials >= 1);
    let ch = logic
        .hero_abilities
        .leftover_channel(src)
        .copied()
        .expect("persist channel");
    assert_eq!(ch.kind, LeftoverSaKind::LaserGuided);
    assert_eq!(ch.phase, LeftoverSaPhase::Preparing);
    logic.update_ai(&[src, tgt], 1.0 / 30.0);
    assert!(
        logic.hero_abilities.leftover_channel(src).is_some(),
        "CMD_FROM_AI Attacking must not abort PersistentPrepTime"
    );
}

#[test]
fn start_preparation_notifies_script_engine_triggered_only() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_missile_defender::{
        missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    use gamelogic::scripting::engine::{initialize_script_engine, with_script_engine_mut};
    let _ = initialize_script_engine();
    let mut logic = GameLogic::new();
    ensure_test_player_for_team(&mut logic, Team::USA);
    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);
    let mut enemy = ThingTemplate::new("GLAInfantryRebel");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), enemy);
    let src = logic
        .create_object(
            "AmericaInfantryMissileDefender",
            Team::USA,
            glam::Vec3::ZERO,
        )
        .expect("md");
    if let Some(o) = logic.host_object_mut(src) {
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
        o.owner_player_id = Some(0);
    }
    let tgt = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(80.0, 0.0, 0.0),
        )
        .expect("enemy");
    assert!(logic.activate_missile_defender_laser_guided(src, tgt));
    let (triggered, completed) = with_script_engine_mut(|engine| {
        (
            engine.is_special_power_triggered(
                0,
                "SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES",
                false,
                src.0,
            ),
            engine.is_special_power_complete(
                0,
                "SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES",
                false,
                src.0,
            ),
        )
    })
    .unwrap_or((false, false));
    assert!(triggered, "startPreparation must notify TRIGGERED");
    assert!(!completed, "startPreparation must not notify COMPLETED");
    let _ = SpecialPowerType::MissileDefenderLaserGuided;
}

#[test]
fn host_upgrade_complete_helix_nuke_bomb_tags_helix() {
    use crate::game_logic::host_helix_napalm::UPGRADE_HELIX_NUKE_BOMB;
    use crate::game_logic::host_upgrades::HostUpgradeKind;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        HostUpgradeKind::from_name("Nuke_Upgrade_HelixNukeBomb"),
        HostUpgradeKind::HelixNukeBomb
    );
    assert_eq!(
        HostUpgradeKind::from_name("Upgrade_HelixNapalmBomb"),
        HostUpgradeKind::HelixNapalmBomb
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut helix = ThingTemplate::new("ChinaHelix");
    helix
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(400.0);
    logic.templates.insert("ChinaHelix".into(), helix);
    let id = logic
        .create_object("ChinaHelix", Team::China, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    let n = logic.apply_helix_bomb_upgrade_to_team(
        Team::China,
        "Nuke_Upgrade_HelixNukeBomb",
        UPGRADE_HELIX_NUKE_BOMB,
    );
    assert!(n >= 1);
    assert!(
        logic
            .host_object(id)
            .unwrap()
            .has_upgrade_tag(UPGRADE_HELIX_NUKE_BOMB)
    );
    assert!(
        logic
            .activate_helix_napalm_bomb(id, glam::Vec3::new(5.0, 0.0, 0.0))
            .is_some()
    );
}

#[test]
fn helix_nuke_bomb_maps_to_helix_napalm_residual() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_helix_napalm::{
        UPGRADE_HELIX_NUKE_BOMB, honesty_helix_nuke_bomb_residual_pack_ok,
    };
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_helix_nuke_bomb_residual_pack_ok());
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::HelixNukeBomb),
        Some("SPECIAL_HELIX_NAPALM_BOMB")
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut helix = ThingTemplate::new("ChinaHelix");
    helix
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(400.0);
    logic.templates.insert("ChinaHelix".into(), helix);
    let mut enemy = ThingTemplate::new("GLAInfantryRebel");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), enemy);
    let src = logic
        .create_object("ChinaHelix", Team::China, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("helix");
    // Without upgrade: fail-closed.
    assert!(
        logic
            .activate_helix_napalm_bomb(src, glam::Vec3::new(10.0, 0.0, 0.0))
            .is_none()
    );
    if let Some(o) = logic.host_object_mut(src) {
        o.apply_upgrade_tag(UPGRADE_HELIX_NUKE_BOMB);
    }
    let _e = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("enemy");
    assert!(
        logic
            .activate_helix_napalm_bomb(src, glam::Vec3::new(10.0, 0.0, 0.0))
            .is_some()
    );
    let _ = SpecialPowerType::HelixNukeBomb;
}

#[test]
fn communications_download_maps_to_cia_intelligence_residual() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_cia_intelligence::{
        COMMUNICATIONS_DOWNLOAD_RELOAD_MS, COMMUNICATIONS_DOWNLOAD_SPECIAL_ENUM,
        honesty_communications_download_residual_pack_ok,
    };
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_communications_download_residual_pack_ok());
    assert_eq!(COMMUNICATIONS_DOWNLOAD_RELOAD_MS, 10_000);
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::CommunicationsDownload),
        Some(COMMUNICATIONS_DOWNLOAD_SPECIAL_ENUM)
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::GLA, "GLA", false));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut enemy = ThingTemplate::new("GLAInfantryRebel");
    enemy.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), enemy);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let _e = logic
        .create_object(
            "GLAInfantryRebel",
            Team::GLA,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("enemy");
    assert!(logic.activate_cia_intelligence(0, Team::USA, Some(src)));
    // CommunicationsDownload shares host SpyVision residual path.
    assert!(logic.activate_cia_intelligence(0, Team::USA, Some(src)));
    let _ = SpecialPowerType::CommunicationsDownload;
}

#[test]
fn general_special_power_aliases_map_to_host_residuals() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        CarpetBombFactionTier, HostSuperweaponKind, honesty_general_special_power_alias_pack_ok,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_general_special_power_alias_pack_ok());

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");

    for power in [
        SpecialPowerType::AirForceDaisyCutter,
        SpecialPowerType::AirForceAirstrike,
        SpecialPowerType::AirForceSpectreGunship,
        SpecialPowerType::SuperweaponParticleCannon,
        SpecialPowerType::NukeNeutronMissile,
        SpecialPowerType::BaikonurRocket,
        SpecialPowerType::LaserCannon,
    ] {
        assert!(
            logic
                .queue_special_power_strike(&power, src, glam::Vec3::new(70.0, 0.0, 0.0))
                .is_some(),
            "power {power:?} must queue host residual"
        );
    }

    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::NukeChinaCarpetBomb,
            src,
            glam::Vec3::new(90.0, 0.0, 0.0),
        )
        .expect("nuke china carpet");
    assert_eq!(
        logic.special_power_strike_carpet_tier(id),
        Some(CarpetBombFactionTier::China)
    );

    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::StealthGpsScrambler),
        None
    ); // GPS is not a superweapon strike
    assert!(logic.activate_gps_scrambler(0, glam::Vec3::new(20.0, 0.0, 0.0), Some(src)));
}

#[test]
fn superweapon_crate_drop_spawns_ten_200_dollar_crates() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_money_crate::{
        SUPERWEAPON_CRATE_DROP_COUNT, SUPERWEAPON_CRATE_DROP_MONEY,
        honesty_superweapon_crate_drop_residual_pack_ok,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_superweapon_crate_drop_residual_pack_ok());
    assert_eq!(SUPERWEAPON_CRATE_DROP_COUNT, 10);
    assert_eq!(SUPERWEAPON_CRATE_DROP_MONEY, 200);

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let n = logic.activate_crate_drop(0, glam::Vec3::new(50.0, 0.0, 0.0), Some(src));
    assert_eq!(n, SUPERWEAPON_CRATE_DROP_COUNT);
    assert_eq!(
        logic.last_crate_drop_spawned(),
        SUPERWEAPON_CRATE_DROP_COUNT
    );
    let crates = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.contains("200Dollar") && o.is_alive())
        .count();
    assert_eq!(crates as u32, SUPERWEAPON_CRATE_DROP_COUNT);
    let _ = SpecialPowerType::CrateDrop;
}

#[test]
fn napalm_strike_and_general_paradrop_terror_cell_map_host_residuals() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_ambush::HostAmbushKind;
    use crate::game_logic::host_paradrop::HostParadropKind;
    use crate::game_logic::special_power_strikes::{
        HostSuperweaponKind, NAPALM_STRIKE_OCL, NAPALM_STRIKE_PRIMARY_DAMAGE,
        NAPALM_STRIKE_RELOAD_MS, honesty_napalm_strike_residual_pack_ok,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert!(honesty_napalm_strike_residual_pack_ok());
    assert_eq!(NAPALM_STRIKE_RELOAD_MS, 600_000);
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::NapalmStrike),
        Some(HostSuperweaponKind::NapalmStrike)
    );
    assert!(
        (HostSuperweaponKind::NapalmStrike.max_damage() - NAPALM_STRIKE_PRIMARY_DAMAGE).abs() < 0.1
    );
    assert!(
        (HostSuperweaponKind::NapalmStrike.max_damage()
            - HostSuperweaponKind::DaisyCutter.max_damage())
        .abs()
            > 1.0
    );
    assert_eq!(
        HostParadropKind::from_command_power(&SpecialPowerType::InfantryParadrop),
        Some(HostParadropKind::InfantryParadrop)
    );
    assert_eq!(
        HostParadropKind::from_command_power(&SpecialPowerType::TankParadrop),
        Some(HostParadropKind::TankParadrop)
    );
    assert_eq!(
        HostAmbushKind::from_command_power(&SpecialPowerType::TerrorCell),
        Some(HostAmbushKind::GLARebelAmbush)
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    assert!(
        logic
            .queue_special_power_strike(
                &SpecialPowerType::NapalmStrike,
                src,
                glam::Vec3::new(90.0, 0.0, 0.0),
            )
            .is_some()
    );
    assert_eq!(logic.ocl_special_power_reg.last_ocl, NAPALM_STRIKE_OCL);
    assert_ne!(
        logic.ocl_special_power_reg.last_ocl,
        "SUPERWEAPON_DaisyCutter"
    );
    let spawned_daisy_b52 = logic.objects.values().any(|o| {
        o.template_name.contains("AmericaJetB52") || o.template_name.contains("DaisyCutterBomb")
    });
    assert!(
        !spawned_daisy_b52,
        "NapalmStrike must not spawn Fuel Air Bomb DaisyCutter transport/payload"
    );
    assert!(
        logic
            .objects
            .values()
            .any(|o| o.template_name.contains("ChinaJetCargoPlane"))
    );
    assert!(
        logic
            .queue_paradrop(
                &SpecialPowerType::InfantryParadrop,
                src,
                glam::Vec3::new(110.0, 0.0, 0.0),
            )
            .is_some()
    );
    assert!(
        logic
            .queue_ambush(
                &SpecialPowerType::TerrorCell,
                src,
                glam::Vec3::new(130.0, 0.0, 0.0),
            )
            .is_some()
    );
}

#[test]
fn black_market_and_dirty_nuke_queue_nuclear_missile_residual() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        BLACK_MARKET_NUKE_RELOAD_MS, DIRTY_NUKE_RELOAD_MS, HostSuperweaponKind,
        honesty_black_market_and_dirty_nuke_residual_pack_ok,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_black_market_and_dirty_nuke_residual_pack_ok());
    assert_eq!(BLACK_MARKET_NUKE_RELOAD_MS, 600_000);
    assert_eq!(DIRTY_NUKE_RELOAD_MS, 30_000);
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::BlackMarketNuke),
        Some(HostSuperweaponKind::NuclearMissile)
    );
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::DetonateDirtyNuke),
        Some(HostSuperweaponKind::NuclearMissile)
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut sc = ThingTemplate::new("GLACommandCenter");
    sc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(4000.0);
    logic.templates.insert("GLACommandCenter".into(), sc);
    let src = logic
        .create_object(
            "GLACommandCenter",
            Team::GLA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("sc");
    assert!(
        logic
            .queue_special_power_strike(
                &SpecialPowerType::BlackMarketNuke,
                src,
                glam::Vec3::new(80.0, 0.0, 0.0),
            )
            .is_some()
    );
    assert!(
        logic
            .queue_special_power_strike(
                &SpecialPowerType::DetonateDirtyNuke,
                src,
                glam::Vec3::new(120.0, 0.0, 0.0),
            )
            .is_some()
    );
}

#[test]
fn airforce_carpet_bomb_forces_airforce_payload_tier() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        CARPET_BOMB_COUNT_AIRF, CarpetBombFactionTier, HostSuperweaponKind,
        honesty_airf_carpet_bomb_residual_pack_ok,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_airf_carpet_bomb_residual_pack_ok());
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::AirForceCarpetBomb),
        Some(HostSuperweaponKind::CarpetBomb)
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    // China caster + AirF power still forces AirForce payload residual.
    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::AirForceCarpetBomb,
            src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("airf carpet");
    assert_eq!(
        logic.special_power_strike_carpet_tier(id),
        Some(CarpetBombFactionTier::AirForce)
    );
    assert_eq!(
        CarpetBombFactionTier::AirForce.bomb_count(),
        CARPET_BOMB_COUNT_AIRF
    );
}

#[test]
fn early_china_carpet_bomb_forces_china_payload_tier() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        CARPET_BOMB_COUNT_CHINA, CarpetBombFactionTier, HostSuperweaponKind,
        honesty_early_china_carpet_bomb_residual_pack_ok,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_early_china_carpet_bomb_residual_pack_ok());
    assert_eq!(
        HostSuperweaponKind::from_command_power(&SpecialPowerType::EarlyChinaCarpetBomb),
        Some(HostSuperweaponKind::CarpetBomb)
    );

    let mut logic = GameLogic::new();
    // USA caster + EarlyChinaCarpetBomb still forces China payload residual.
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::EarlyChinaCarpetBomb,
            src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("early china carpet");
    assert_eq!(
        logic.special_power_strike_carpet_tier(id),
        Some(CarpetBombFactionTier::China)
    );
    assert_eq!(
        CarpetBombFactionTier::China.bomb_count(),
        CARPET_BOMB_COUNT_CHINA
    );
}

#[test]
fn early_frenzy_and_emergency_repair_powers_activate() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_emergency_repair::{
        HostEmergencyRepairLevel, highest_emergency_repair_level_from_sciences,
    };
    use crate::game_logic::host_frenzy::{HostFrenzyLevel, highest_frenzy_level_from_sciences};
    use crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name;
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::EarlyFrenzy),
        Some("EARLY_SPECIAL_FRENZY")
    );
    assert_eq!(
        host_command_power_cpp_enum_name(&SpecialPowerType::EarlyEmergencyRepair),
        Some("EARLY_SPECIAL_REPAIR_VEHICLES")
    );
    assert_eq!(
        highest_frenzy_level_from_sciences(["Early_SCIENCE_Frenzy2"]),
        HostFrenzyLevel::Two
    );
    assert_eq!(
        highest_emergency_repair_level_from_sciences(["Early_SCIENCE_EmergencyRepair3"]),
        HostEmergencyRepairLevel::Three
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("Early_SCIENCE_Frenzy1".to_string());
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("Early_SCIENCE_EmergencyRepair1".to_string());
    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let mut tank = ThingTemplate::new("ChinaTankBattleMaster");
    tank.add_kind_of(KindOf::Vehicle).set_health(400.0);
    logic.templates.insert("ChinaTankBattleMaster".into(), tank);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let veh = logic
        .create_object(
            "ChinaTankBattleMaster",
            Team::China,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("tank");
    // Damage vehicle so repair has a target.
    if let Some(o) = logic.host_object_mut(veh) {
        let _ = o.take_damage(100.0);
    }
    assert!(logic.activate_frenzy(
        0,
        glam::Vec3::new(10.0, 0.0, 0.0),
        Some(src),
        HostFrenzyLevel::One,
    ));
    assert!(logic.activate_emergency_repair(
        0,
        glam::Vec3::new(10.0, 0.0, 0.0),
        Some(src),
        HostEmergencyRepairLevel::One,
    ));
    let _ = SpecialPowerType::EarlyFrenzy;
    let _ = SpecialPowerType::EarlyEmergencyRepair;
}

#[test]
fn early_leaflet_drop_queues_same_delay_residual() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_leaflet_drop::{
        HostLeafletDropKind, LEAFLET_DELAY_FRAMES, honesty_early_leaflet_drop_residual_pack_ok,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert!(honesty_early_leaflet_drop_residual_pack_ok());
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let id = logic
        .queue_leaflet_drop(
            &SpecialPowerType::EarlyLeafletDrop,
            src,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("early leaflet");
    let m = logic.host_leaflet_drops().get(id).expect("mission");
    assert_eq!(m.kind, HostLeafletDropKind::UsaEarlyLeafletDrop);
    assert_eq!(
        m.impact_frame.saturating_sub(m.activate_frame),
        LEAFLET_DELAY_FRAMES
    );
}

#[test]
fn host_upgrade_complete_fanaticism_does_not_map_to_nationalism() {
    use crate::game_logic::host_battlemaster::{
        TERRAIN_DECAL_HORDE, UPGRADE_FANATICISM, leftover_horde_decal_type,
        leftover_horde_fanaticism_bonus,
    };
    use crate::game_logic::host_upgrades::HostUpgradeKind;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        HostUpgradeKind::from_name("Upgrade_Fanaticism"),
        HostUpgradeKind::Fanaticism
    );
    assert_eq!(
        HostUpgradeKind::from_name("Upgrade_ChinaNationalism"),
        HostUpgradeKind::Nationalism
    );
    assert!(!leftover_horde_fanaticism_bonus(false, true));
    assert_eq!(
        leftover_horde_decal_type(true, false, true),
        TERRAIN_DECAL_HORDE
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut rg = ThingTemplate::new("ChinaInfantryRedguard");
    rg.add_kind_of(KindOf::Infantry).set_health(120.0);
    logic.templates.insert("ChinaInfantryRedguard".into(), rg);
    let id = logic
        .create_object(
            "ChinaInfantryRedguard",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("rg");
    let n = logic.apply_fanaticism_to_team(Team::China, UPGRADE_FANATICISM);
    assert!(n >= 1, "fanaticism tag residual n={n}");
    let obj = logic.host_object(id).unwrap();
    assert!(obj.has_upgrade_tag(UPGRADE_FANATICISM));
    assert!(!obj.weapon_bonus_nationalism);
    assert!(!obj.weapon_bonus_fanaticism);
}

#[test]
fn superweapon_cash_hack_science_tier_steals_amount() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_hero_abilities::{
        CASH_HACK_MONEY_AMOUNT_DEFAULT, CASH_HACK_MONEY_AMOUNT_TIER3, SCIENCE_CASH_HACK_3,
        cash_hack_money_from_sciences,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert_eq!(
        cash_hack_money_from_sciences(["SCIENCE_CashHack1"]),
        CASH_HACK_MONEY_AMOUNT_DEFAULT
    );
    assert_eq!(
        cash_hack_money_from_sciences([SCIENCE_CASH_HACK_3]),
        CASH_HACK_MONEY_AMOUNT_TIER3
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    logic
        .players
        .insert(1, Player::new(1, Team::USA, "USA", false));
    logic.players.get_mut(&0).unwrap().resources.supplies = 0;
    logic.players.get_mut(&1).unwrap().resources.supplies = 10_000;
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(SCIENCE_CASH_HACK_3.to_string());

    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");

    let mut depot = ThingTemplate::new("AmericaSupplyCenter");
    depot
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSupplyCenter)
        .set_health(2000.0);
    depot.capturable = true;
    logic.templates.insert("AmericaSupplyCenter".into(), depot);
    let victim = logic
        .create_object(
            "AmericaSupplyCenter",
            Team::USA,
            glam::Vec3::new(80.0, 0.0, 0.0),
        )
        .expect("victim");

    // Location-only fire is a C++ no-op (CashHackSpecialPower.cpp:76-82).
    assert_eq!(logic.activate_cash_hack(0, Some(src), None), None);

    let stolen = logic
        .activate_cash_hack(0, Some(src), Some(victim))
        .expect("valid cash-generator steal");
    assert_eq!(stolen, CASH_HACK_MONEY_AMOUNT_TIER3);
    assert_eq!(
        logic.last_cash_hack_request_amount(),
        CASH_HACK_MONEY_AMOUNT_TIER3
    );
    assert_eq!(logic.last_cash_hack_stolen_amount(), stolen);
    assert_eq!(
        logic.players.get(&0).unwrap().effective_supplies(),
        CASH_HACK_MONEY_AMOUNT_TIER3
    );
    assert_eq!(
        logic.players.get(&1).unwrap().effective_supplies(),
        10_000 - CASH_HACK_MONEY_AMOUNT_TIER3
    );

    // Object-target path residual (second steal from the same victim player).
    let stolen2 = logic
        .activate_cash_hack(0, Some(src), Some(victim))
        .expect("second valid steal");
    assert_eq!(stolen2, CASH_HACK_MONEY_AMOUNT_TIER3);
    let _ = SpecialPowerType::CashHack;
}

#[test]
fn carpet_bomb_faction_tier_from_team_and_airforce_science() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        CARPET_BOMB_COUNT, CARPET_BOMB_COUNT_AIRF, CARPET_BOMB_COUNT_CHINA, CarpetBombFactionTier,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    assert_eq!(
        CarpetBombFactionTier::from_team(Team::China).bomb_count(),
        CARPET_BOMB_COUNT_CHINA
    );
    assert_eq!(
        CarpetBombFactionTier::from_team(Team::USA).bomb_count(),
        CARPET_BOMB_COUNT
    );
    assert_eq!(
        CarpetBombFactionTier::AirForce.bomb_count(),
        CARPET_BOMB_COUNT_AIRF
    );
    assert_eq!(
        CarpetBombFactionTier::highest_from_team_and_sciences(
            Team::USA,
            ["AirF_SUPERWEAPON_CarpetBomb"],
        ),
        CarpetBombFactionTier::AirForce
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    let mut cc = ThingTemplate::new("ChinaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("ChinaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "ChinaCommandCenter",
            Team::China,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::CarpetBomb,
            src,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .expect("carpet");
    assert_eq!(
        logic.special_power_strike_carpet_tier(id),
        Some(CarpetBombFactionTier::China)
    );

    let mut logic_us = GameLogic::new();
    logic_us
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic_us
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("AirF_SUPERWEAPON_CarpetBomb".to_string());
    let mut us_cc = ThingTemplate::new("AmericaCommandCenter");
    us_cc
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic_us
        .templates
        .insert("AmericaCommandCenter".into(), us_cc);
    let us_src = logic_us
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("us cc");
    let us_id = logic_us
        .queue_special_power_strike(
            &SpecialPowerType::CarpetBomb,
            us_src,
            glam::Vec3::new(140.0, 0.0, 0.0),
        )
        .expect("us carpet");
    assert_eq!(
        logic_us.special_power_strike_carpet_tier(us_id),
        Some(CarpetBombFactionTier::AirForce)
    );
}

#[test]
fn a10_science_tier_spawns_ocl_jets_from_map_edge() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        A10_SCIENCE_TIER3, A10_TRANSPORT, A10StrikeScienceTier, HostSpecialPowerStrikeRegistry,
        HostSuperweaponKind,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    assert_eq!(
        A10StrikeScienceTier::highest_from_sciences([A10_SCIENCE_TIER3]).formation_size(),
        3
    );
    // Delayed circular blob is not retail — OCL jets apply missile/vulcan.
    let l1 =
        HostSpecialPowerStrikeRegistry::damage_at_distance(HostSuperweaponKind::A10Strike, 0.0);
    let l3 = HostSpecialPowerStrikeRegistry::damage_at_distance_with_tiers(
        HostSuperweaponKind::A10Strike,
        0.0,
        crate::game_logic::special_power_strikes::ScudStormAnthraxTier::Base,
        A10StrikeScienceTier::Level3,
    );
    assert!(l1.abs() < 0.1 && l3.abs() < 0.1, "l1={l1} l3={l3}");

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(A10_SCIENCE_TIER3.to_string());
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");
    let id = logic
        .queue_special_power_strike(
            &SpecialPowerType::Airstrike,
            src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("a10");
    assert_eq!(
        logic.special_power_strike_a10_tier(id),
        Some(A10StrikeScienceTier::Level3)
    );
    let jets: Vec<_> = logic
        .host_objects()
        .values()
        .filter(|o| o.a10_strike_transport.is_some())
        .collect();
    assert_eq!(jets.len(), 3, "science 3 must spawn FormationSize 3 jets");
    assert!(jets.iter().all(|o| o.template_name == A10_TRANSPORT));
    let (min, max) = logic.world_bounds();
    for jet in &jets {
        let p = jet.get_position();
        let on_edge = (p.x - min.x).abs() < 1.0
            || (p.x - max.x).abs() < 1.0
            || (p.z - min.z).abs() < 1.0
            || (p.z - max.z).abs() < 1.0;
        assert!(on_edge, "A10 jet must spawn on map edge, pos={p:?}");
    }
}

#[test]
fn tank_and_infantry_paradrop_use_faction_templates() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_paradrop::{
        HostParadropKind, INFA_PARADROP_TEMPLATE, TANK_PARADROP_TEMPLATE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("SCIENCE_TankParadrop2".into());
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("Infa_SCIENCE_InfantryParadrop3".into());
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");

    let tank_id = logic
        .queue_paradrop(
            &SpecialPowerType::TankParadrop,
            src,
            glam::Vec3::new(80.0, 0.0, 0.0),
        )
        .expect("tank drop");
    let tank = logic.host_paradrops.get(tank_id).expect("tank mission");
    assert_eq!(tank.kind, HostParadropKind::TankParadrop);
    assert_eq!(tank.unit_template, TANK_PARADROP_TEMPLATE);
    assert_eq!(tank.unit_count, 2);

    let inf_id = logic
        .queue_paradrop(
            &SpecialPowerType::InfantryParadrop,
            src,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .expect("infa drop");
    let inf = logic.host_paradrops.get(inf_id).expect("infa mission");
    assert_eq!(inf.kind, HostParadropKind::InfantryParadrop);
    assert_eq!(inf.unit_template, INFA_PARADROP_TEMPLATE);
    assert_eq!(inf.unit_count, 20);
}

#[test]
fn emergency_repair_and_frenzy_science_tier_from_unlocked() {
    use crate::game_logic::host_emergency_repair::{
        HostEmergencyRepairLevel, SCIENCE_EMERGENCY_REPAIR3,
        highest_emergency_repair_level_from_sciences,
    };
    use crate::game_logic::host_frenzy::{
        HostFrenzyLevel, SCIENCE_FRENZY2, SCIENCE_FRENZY3, highest_frenzy_level_from_sciences,
    };
    assert_eq!(
        highest_emergency_repair_level_from_sciences([SCIENCE_EMERGENCY_REPAIR3]),
        HostEmergencyRepairLevel::Three
    );
    assert_eq!(
        highest_frenzy_level_from_sciences([SCIENCE_FRENZY2, SCIENCE_FRENZY3]),
        HostFrenzyLevel::Three
    );
    assert_eq!(
        highest_frenzy_level_from_sciences(["SCIENCE_CashBounty1"]),
        HostFrenzyLevel::One
    );

    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(SCIENCE_EMERGENCY_REPAIR3.to_string());
    let sciences = logic.player_unlocked_sciences(0);
    assert_eq!(
        highest_emergency_repair_level_from_sciences(sciences.iter().map(|s| s.as_str())),
        HostEmergencyRepairLevel::Three
    );
}

#[test]
fn ambush_science_tier_selects_unit_count() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_ambush::{
        AmbushScienceTier, GLA_AMBUSH1_UNIT_COUNT, GLA_AMBUSH3_UNIT_COUNT, SCIENCE_AMBUSH3,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));
    let mut palace = ThingTemplate::new("GLAPalace");
    palace.add_kind_of(KindOf::Structure).set_health(3000.0);
    logic.templates.insert("GLAPalace".into(), palace);
    let mut rebel = ThingTemplate::new("GLAInfantryRebel");
    rebel.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLAInfantryRebel".into(), rebel);

    let src = logic
        .create_object("GLAPalace", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("palace");

    let id1 = logic
        .queue_ambush(
            &SpecialPowerType::Ambush,
            src,
            glam::Vec3::new(80.0, 0.0, 0.0),
        )
        .expect("q1");
    assert_eq!(
        logic.ambush_mission_unit_count(id1),
        Some(GLA_AMBUSH1_UNIT_COUNT)
    );

    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(SCIENCE_AMBUSH3.to_string());
    let id3 = logic
        .queue_ambush(
            &SpecialPowerType::Ambush,
            src,
            glam::Vec3::new(160.0, 0.0, 0.0),
        )
        .expect("q3");
    assert_eq!(
        logic.ambush_mission_unit_count(id3),
        Some(GLA_AMBUSH3_UNIT_COUNT)
    );
    assert_eq!(
        AmbushScienceTier::highest_from_sciences([SCIENCE_AMBUSH3]).rebel_count(),
        GLA_AMBUSH3_UNIT_COUNT
    );
    assert_eq!(
        logic
            .host_ambushes()
            .get(id1)
            .map(|m| m.unit_template.as_str()),
        Some("GLAInfantryRebel")
    );
    logic.players.get_mut(&0).unwrap().unlocked_sciences.clear();
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert("Chem_SCIENCE_RebelAmbush1".to_string());
    let idc = logic
        .queue_ambush(
            &SpecialPowerType::Ambush,
            src,
            glam::Vec3::new(240.0, 0.0, 0.0),
        )
        .expect("qc");
    assert_eq!(
        logic
            .host_ambushes()
            .get(idc)
            .map(|m| m.unit_template.as_str()),
        Some("Chem_GLAInfantryRebel")
    );
    assert_eq!(logic.ambush_mission_unit_count(idc), Some(4));
}

#[test]
fn paradrop_science_tier_selects_unit_count() {
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::host_paradrop::{
        PARADROP_RANGER_COUNT_L1, PARADROP_RANGER_COUNT_L3, ParadropScienceTier, SCIENCE_PARADROP1,
        SCIENCE_PARADROP3,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let src = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("cc");

    // Default tier 1 without science still queues residual L1 count via highest_from_sciences default.
    let id1 = logic
        .queue_paradrop(
            &SpecialPowerType::Paradrop,
            src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .expect("q1");
    assert_eq!(
        logic.paradrop_mission_unit_count(id1),
        Some(PARADROP_RANGER_COUNT_L1)
    );

    // Unlock tier 3 science.
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(SCIENCE_PARADROP3.to_string());
    let id3 = logic
        .queue_paradrop(
            &SpecialPowerType::Paradrop,
            src,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("q3");
    assert_eq!(
        logic.paradrop_mission_unit_count(id3),
        Some(PARADROP_RANGER_COUNT_L3)
    );
    assert_eq!(
        ParadropScienceTier::highest_from_sciences([SCIENCE_PARADROP1, SCIENCE_PARADROP3])
            .ranger_count(),
        PARADROP_RANGER_COUNT_L3
    );
}

#[test]
fn host_upgrade_complete_cash_bounty_sets_player_percent() {
    use crate::game_logic::host_cash_bounty::{CASH_BOUNTY1_PERCENT, CASH_BOUNTY3_PERCENT};
    use crate::game_logic::{
        KindOf, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team, ThingTemplate,
    };
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::GLA, "GLA", true));

    let n = logic.apply_cash_bounty_upgrade_to_team(Team::GLA, "Upgrade_CashBounty");
    assert_eq!(n, 1);
    let p = logic.players.get(&0).unwrap();
    assert!((p.cash_bounty_percent - 0.0).abs() < 0.001);
    assert!(p.unlocked_sciences.contains("SCIENCE_CashBounty1"));

    let mut palace = ThingTemplate::new("GLAPalace");
    palace.add_kind_of(KindOf::Structure).set_health(3000.0);
    palace
        .special_power_modules
        .push(SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ModuleTag_15".into()),
            module_kind: SpecialPowerModuleKind::CashBountyPower,
            special_power_template: "SpecialAbilityCashBounty1".into(),
            special_power_template_id: 1,
            command_power: None,
            reload_time_frames: 0,
            required_science: Some("SCIENCE_CashBounty1".into()),
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        });
    palace
        .special_power_modules
        .push(SpecialPowerModuleMetadata {
            source_index: 1,
            module_tag: Some("ModuleTag_17".into()),
            module_kind: SpecialPowerModuleKind::CashBountyPower,
            special_power_template: "SpecialAbilityCashBounty3".into(),
            special_power_template_id: 3,
            command_power: None,
            reload_time_frames: 0,
            required_science: Some("SCIENCE_CashBounty3".into()),
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
        });
    logic.templates.insert("GLAPalace".into(), palace);
    let _ = logic
        .create_object_for_player("GLAPalace", 0, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("palace");
    assert!(
        (logic.players.get(&0).unwrap().cash_bounty_percent - CASH_BOUNTY1_PERCENT).abs() < 0.001
    );

    let n3 = logic.apply_cash_bounty_upgrade_to_team(Team::GLA, "SCIENCE_CashBounty3");
    assert_eq!(n3, 1);
    let p = logic.players.get(&0).unwrap();
    assert!((p.cash_bounty_percent - CASH_BOUNTY3_PERCENT).abs() < 0.001);
}

#[test]
fn host_upgrade_complete_slave_drone_attaches_to_humvee() {
    use crate::game_logic::host_slave_drones::{
        UPGRADE_AMERICA_BATTLE_DRONE, UPGRADE_AMERICA_SCOUT_DRONE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));

    let mut humvee = ThingTemplate::new("AmericaVehicleHumvee");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleHumvee".into(), humvee);

    let mid = logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("humvee");

    let n = logic.apply_slave_drone_upgrade_to_team(Team::USA, UPGRADE_AMERICA_SCOUT_DRONE);
    assert_eq!(n, 1);
    assert!(
        logic
            .host_object(mid)
            .unwrap()
            .has_upgrade_tag(UPGRADE_AMERICA_SCOUT_DRONE)
    );
    // Scout drone object should exist.
    let drones: Vec<_> = logic
        .objects
        .values()
        .filter(|o| {
            o.template_name.to_ascii_lowercase().contains("scoutdrone")
                || o.template_name.contains("ScoutDrone")
        })
        .collect();
    assert!(!drones.is_empty(), "scout drone must spawn");
    assert!(logic.honesty_scout_drone_attach_ok());

    // Battle drone on second master.
    let mid2 = logic
        .create_object(
            "AmericaVehicleHumvee",
            Team::USA,
            glam::Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("humvee2");
    let n2 = logic.apply_slave_drone_upgrade_to_team(Team::USA, UPGRADE_AMERICA_BATTLE_DRONE);
    assert!(n2 >= 1);
    assert!(
        logic
            .host_object(mid2)
            .unwrap()
            .has_upgrade_tag(UPGRADE_AMERICA_BATTLE_DRONE)
    );
}

#[test]
fn host_upgrade_complete_chemical_suits_reduces_poison() {
    use crate::game_logic::combat::DamageType;
    use crate::game_logic::host_armor_residual::{
        CHEM_SUIT_HUMAN_ARMOR_POISON, apply_residual_armor,
    };
    use crate::game_logic::host_upgrades::UPGRADE_AMERICA_CHEMICAL_SUITS;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let id = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("ranger");
    let before = {
        let o = logic.host_object(id).unwrap();
        apply_residual_armor(o, DamageType::Toxin, 100.0)
    };
    let n = logic.apply_chemical_suits_to_team(Team::USA, UPGRADE_AMERICA_CHEMICAL_SUITS);
    assert!(n >= 1);
    let after = {
        let o = logic.host_object(id).unwrap();
        apply_residual_armor(o, DamageType::Toxin, 100.0)
    };
    // Chem suit poison coeff 0.20 vs human default (higher).
    assert!(
        after < before,
        "chem suits must reduce poison residual ({after} vs {before})"
    );
    assert!((after - 100.0 * CHEM_SUIT_HUMAN_ARMOR_POISON).abs() < 1.0);
}

#[test]
fn host_upgrade_complete_moab_and_satellite_hack_unlock() {
    use crate::game_logic::Team;
    use crate::game_logic::host_upgrades::{
        UPGRADE_AMERICA_MOAB, UPGRADE_CHINA_SATELLITE_HACK_TWO,
    };
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "China", true));
    assert_eq!(
        logic.apply_player_unlock_upgrade(Team::USA, UPGRADE_AMERICA_MOAB, UPGRADE_AMERICA_MOAB),
        1
    );
    assert!(
        logic
            .players
            .get(&0)
            .unwrap()
            .unlocked_sciences
            .contains(UPGRADE_AMERICA_MOAB)
    );
    assert!(logic.apply_satellite_hack_to_team(Team::China, UPGRADE_CHINA_SATELLITE_HACK_TWO) >= 1);
    assert!(
        logic
            .players
            .get(&1)
            .unwrap()
            .unlocked_sciences
            .contains(UPGRADE_CHINA_SATELLITE_HACK_TWO)
    );
}

#[test]
fn host_upgrade_complete_mines_radar_fortified() {
    use crate::game_logic::host_upgrades::{
        FORTIFIED_STRUCTURE_ADD_MAX_HEALTH, UPGRADE_CHINA_MINES, UPGRADE_GLA_FORTIFIED_STRUCTURE,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::China, "China", true));
    logic
        .players
        .insert(1, Player::new(1, Team::GLA, "GLA", true));
    logic
        .players
        .insert(2, Player::new(2, Team::USA, "USA", true));

    let n_m =
        logic.apply_player_unlock_upgrade(Team::China, UPGRADE_CHINA_MINES, UPGRADE_CHINA_MINES);
    assert_eq!(n_m, 1);
    assert!(
        logic
            .players
            .get(&0)
            .unwrap()
            .unlocked_sciences
            .contains(UPGRADE_CHINA_MINES)
    );

    let mut bm = ThingTemplate::new("GLABlackMarket");
    bm.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic.templates.insert("GLABlackMarket".into(), bm);
    let bid = logic
        .create_object("GLABlackMarket", Team::GLA, glam::Vec3::new(0.0, 0.0, 0.0))
        .expect("bm");
    let before = logic.host_object(bid).unwrap().max_health;
    let n_f = logic.apply_fortified_structure_to_team(Team::GLA, UPGRADE_GLA_FORTIFIED_STRUCTURE);
    assert_eq!(n_f, 1);
    let after = logic.host_object(bid).unwrap().max_health;
    assert!((after - before - FORTIFIED_STRUCTURE_ADD_MAX_HEALTH).abs() < 0.1);

    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let cid = logic
        .create_object(
            "AmericaCommandCenter",
            Team::USA,
            glam::Vec3::new(50.0, 0.0, 0.0),
        )
        .expect("cc");
    let n_r = logic.apply_radar_research_to_team(Team::USA, "Upgrade_AmericaRadar");
    assert!(n_r >= 1);
    assert!(
        logic
            .host_object(cid)
            .unwrap()
            .has_upgrade_tag("Upgrade_AmericaRadar")
    );
    assert!(
        logic
            .players
            .get(&2)
            .unwrap()
            .unlocked_sciences
            .contains("Upgrade_AmericaRadar")
    );
}

#[test]
fn host_upgrade_complete_drone_and_aircraft_armor() {
    use crate::game_logic::host_mig::{
        MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH, UPGRADE_CHINA_AIRCRAFT_ARMOR,
    };
    use crate::game_logic::host_slave_drones::{
        SlaveDroneKind, UPGRADE_AMERICA_DRONE_ARMOR, drone_armor_add_max_health,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "China", true));

    let mut battle = ThingTemplate::new("AmericaBattleDrone");
    battle.add_kind_of(KindOf::Vehicle).set_health(100.0);
    logic.templates.insert("AmericaBattleDrone".into(), battle);
    let mut mig = ThingTemplate::new("ChinaJetMIG");
    mig.add_kind_of(KindOf::Aircraft).set_health(160.0);
    logic.templates.insert("ChinaJetMIG".into(), mig);

    let did = logic
        .create_object(
            "AmericaBattleDrone",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("drone");
    let before = logic.host_object(did).unwrap().max_health;
    let n_d = logic.apply_drone_armor_to_team(Team::USA, UPGRADE_AMERICA_DRONE_ARMOR);
    assert_eq!(n_d, 1);
    let after = logic.host_object(did).unwrap().max_health;
    assert!((after - before - drone_armor_add_max_health(SlaveDroneKind::Battle)).abs() < 0.1);

    let mid = logic
        .create_object("ChinaJetMIG", Team::China, glam::Vec3::new(20.0, 0.0, 0.0))
        .expect("mig");
    let mb = logic.host_object(mid).unwrap().max_health;
    let n_a = logic.apply_aircraft_armor_to_team(Team::China, UPGRADE_CHINA_AIRCRAFT_ARMOR);
    assert_eq!(n_a, 1);
    let ma = logic.host_object(mid).unwrap().max_health;
    assert!((ma - mb - MIG_AIRCRAFT_ARMOR_ADD_MAX_HEALTH).abs() < 0.1);
}

#[test]
fn host_upgrade_complete_advanced_training_and_tactical_nuke_mig() {
    use crate::game_logic::host_unit_training::{
        UPGRADE_AMERICA_ADVANCED_TRAINING, residual_xp_gain_with_advanced_training,
    };
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .insert(1, Player::new(1, Team::China, "China", true));

    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);
    let mut mig = ThingTemplate::new("Nuke_ChinaJetMIG");
    mig.add_kind_of(KindOf::Aircraft).set_health(200.0);
    logic.templates.insert("Nuke_ChinaJetMIG".into(), mig);

    let rid = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("ranger");
    let mid = logic
        .create_object(
            "Nuke_ChinaJetMIG",
            Team::China,
            glam::Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("mig");

    let n_at = logic.apply_advanced_training_to_team(Team::USA, UPGRADE_AMERICA_ADVANCED_TRAINING);
    assert!(n_at >= 1);
    assert!(
        logic
            .players
            .get(&0)
            .unwrap()
            .unlocked_sciences
            .contains(UPGRADE_AMERICA_ADVANCED_TRAINING)
    );
    assert!(
        logic
            .host_object(rid)
            .unwrap()
            .has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING)
    );
    // Honesty: XP scalar residual path.
    assert!((residual_xp_gain_with_advanced_training(50.0, true) - 100.0).abs() < 0.01);

    let n_nuke = logic.apply_tactical_nuke_mig_to_team(Team::China, "Upgrade_ChinaTacticalNukeMig");
    assert_eq!(n_nuke, 1);
    assert!(
        logic
            .host_object(mid)
            .unwrap()
            .has_upgrade_tag("Upgrade_ChinaTacticalNukeMig")
    );
}

#[test]
fn newly_trained_unit_gets_advanced_training_scalar() {
    // C++ ExperienceScalarUpgrade on create after Upgrade_AmericaAdvancedTraining.
    use crate::game_logic::host_unit_training::UPGRADE_AMERICA_ADVANCED_TRAINING;
    use crate::game_logic::{KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic
        .players
        .insert(0, Player::new(0, Team::USA, "USA", true));
    logic
        .players
        .get_mut(&0)
        .unwrap()
        .unlocked_sciences
        .insert(UPGRADE_AMERICA_ADVANCED_TRAINING.to_string());

    let mut ranger = ThingTemplate::new("AmericaInfantryRanger");
    ranger.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".into(), ranger);

    let rid = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            glam::Vec3::new(0.0, 0.0, 0.0),
        )
        .expect("ranger");
    let obj = logic.host_object(rid).unwrap();
    assert!(obj.has_upgrade_tag(UPGRADE_AMERICA_ADVANCED_TRAINING));
    assert!(
        (obj.experience_scalar - 2.0).abs() < 0.001,
        "AddXPScalar 1.0 on 1.0 base, got {}",
        obj.experience_scalar
    );
}

#[test]
fn ocl_science_tiers_use_controlling_player_not_faction_union() {
    // C++ OCLSpecialPower::findOCL — getControllingPlayer()->hasScience only.
    use crate::command_system::SpecialPowerType;
    use crate::game_logic::special_power_strikes::{
        A10_SCIENCE_TIER3, A10_TRANSPORT, A10StrikeScienceTier, ARTILLERY_SCIENCE_TIER3,
        ArtilleryBarrageScienceTier, ScudStormAnthraxTier,
    };
    use crate::game_logic::{KindOf, Player, Team, ThingTemplate};

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA-A", true));
    logic.add_player(Player::new(1, Team::USA, "USA-B", false));
    logic.add_player(Player::new(2, Team::China, "China-A", false));
    logic.add_player(Player::new(3, Team::China, "China-B", false));
    logic.add_player(Player::new(4, Team::GLA, "GLA-A", false));
    logic.add_player(Player::new(5, Team::GLA, "GLA-B", false));

    logic
        .players
        .get_mut(&1)
        .unwrap()
        .unlocked_sciences
        .insert(A10_SCIENCE_TIER3.into());
    logic
        .players
        .get_mut(&3)
        .unwrap()
        .unlocked_sciences
        .insert(ARTILLERY_SCIENCE_TIER3.into());
    logic
        .players
        .get_mut(&5)
        .unwrap()
        .unlocked_sciences
        .insert("Upgrade_GLAAnthraxBeta".into());

    let mut usa_cc = ThingTemplate::new("AmericaCommandCenter");
    usa_cc
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".into(), usa_cc);
    let mut china_cc = ThingTemplate::new("ChinaCommandCenter");
    china_cc
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .set_health(5000.0);
    logic
        .templates
        .insert("ChinaCommandCenter".into(), china_cc);
    let mut gla_cc = ThingTemplate::new("GLAScudStorm");
    gla_cc.add_kind_of(KindOf::Structure).set_health(5000.0);
    logic.templates.insert("GLAScudStorm".into(), gla_cc);

    let usa_src = logic
        .create_object_for_player("AmericaCommandCenter", 0, glam::Vec3::ZERO)
        .unwrap();
    let china_src = logic
        .create_object_for_player("ChinaCommandCenter", 2, glam::Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let gla_src = logic
        .create_object_for_player("GLAScudStorm", 4, glam::Vec3::new(20.0, 0.0, 0.0))
        .unwrap();

    let a10 = logic
        .queue_special_power_strike(
            &SpecialPowerType::Airstrike,
            usa_src,
            glam::Vec3::new(100.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic.special_power_strike_a10_tier(a10),
        Some(A10StrikeScienceTier::Level1)
    );
    let jets = logic
        .host_objects()
        .values()
        .filter(|o| o.a10_strike_transport.is_some())
        .count();
    assert_eq!(jets, 1, "ally A10-3 must not upgrade this player's strike");
    assert!(
        logic
            .host_objects()
            .values()
            .filter(|o| o.a10_strike_transport.is_some())
            .all(|o| o.template_name == A10_TRANSPORT)
    );

    let arty = logic
        .queue_special_power_strike(
            &SpecialPowerType::Artillery,
            china_src,
            glam::Vec3::new(120.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic
            .special_power_strikes()
            .get(arty)
            .map(|s| s.artillery_tier),
        Some(ArtilleryBarrageScienceTier::Level1)
    );

    let scud = logic
        .queue_special_power_strike(
            &SpecialPowerType::ScudStorm,
            gla_src,
            glam::Vec3::new(140.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic
            .special_power_strikes()
            .get(scud)
            .map(|s| s.scud_anthrax_tier),
        Some(ScudStormAnthraxTier::Base)
    );

    let usa_up = logic
        .create_object_for_player("AmericaCommandCenter", 1, glam::Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let a10_up = logic
        .queue_special_power_strike(
            &SpecialPowerType::Airstrike,
            usa_up,
            glam::Vec3::new(160.0, 0.0, 0.0),
        )
        .unwrap();
    assert_eq!(
        logic.special_power_strike_a10_tier(a10_up),
        Some(A10StrikeScienceTier::Level3)
    );
}
