use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, SpecialPowerType};
use crate::game_logic::{
    GameLogic, Player, SpecialPowerModuleKind, SpecialPowerModuleMetadata, Team, ThingTemplate,
};
use glam::Vec3;

fn test_module(power: SpecialPowerType, template: &str) -> SpecialPowerModuleMetadata {
    SpecialPowerModuleMetadata {
        source_index: 0,
        module_tag: Some("ModuleTag_SpecialPower".into()),
        module_kind: SpecialPowerModuleKind::OclSpecialPower,
        special_power_template: template.into(),
        special_power_template_id: 1,
        command_power: Some(power),
        reload_time_frames: 0,
        required_science: None,
        public_timer: false,
        shared_n_sync: false,
        shortcut_power: false,
        update_module_starts_attack: false,
        starts_paused: false,
        scripted_special_power_only: false,
    }
}

/// C++ `SpecialPowerStore::canUseSpecialPower` (`SpecialPower.cpp:308`) —
/// execute must not fall back to any selected unit when none carry the module.
#[test]
fn frenzy_execute_rejects_selection_without_module() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::China, "China", true));
    let mut tank = ThingTemplate::new("Hq4vpduCasterTank");
    tank.set_health(100.0);
    logic.templates.insert("Hq4vpduCasterTank".into(), tank);
    let tank_id = logic
        .create_object("Hq4vpduCasterTank", Team::China, Vec3::ZERO)
        .expect("tank");

    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.special_power_source_object(&[tank_id], &SpecialPowerType::Frenzy),
            None
        );
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power_at_location(
                &[tank_id],
                &SpecialPowerType::Frenzy,
                Vec3::new(50.0, 0.0, 50.0),
            ),
            CommandResult::InvalidCommand,
            "any-unit Frenzy fallback must be removed"
        );
    }

    let mut cc = ThingTemplate::new("Hq4vpduCasterCC");
    cc.set_health(5000.0);
    cc.special_power_modules
        .push(test_module(SpecialPowerType::Frenzy, "SuperweaponFrenzy"));
    logic.templates.insert("Hq4vpduCasterCC".into(), cc);
    let cc_id = logic
        .create_object("Hq4vpduCasterCC", Team::China, Vec3::new(20.0, 0.0, 0.0))
        .expect("cc");

    let exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.special_power_source_object(&[tank_id, cc_id], &SpecialPowerType::Frenzy),
        Some(cc_id),
        "source must be the SpecialPowerModule owner, not the first selected tank"
    );
}

/// C++ CashHackSpecialPower.cpp:76-82 / DefectorSpecialPower.cpp:69-76 —
/// location fire and illegal object targets must not start recharge.
#[test]
fn cash_hack_and_defector_consume_only_on_valid_object() {
    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::China, "China", true));
    logic.add_player(Player::new(1, Team::USA, "USA", false));
    if let Some(p) = logic.get_player_mut(1) {
        p.resources.supplies = 8_000;
    }

    let mut cash_mod = test_module(SpecialPowerType::CashHack, "SuperweaponCashHack");
    cash_mod.reload_time_frames = 7_200;
    let mut def_mod = test_module(SpecialPowerType::Defector, "SpecialPowerDefector");
    def_mod.reload_time_frames = 300;

    let mut cc = ThingTemplate::new("HqPvymnChinaCC");
    cc.set_health(5000.0);
    cc.add_kind_of(crate::game_logic::KindOf::Structure);
    cc.special_power_modules.push(cash_mod);
    cc.special_power_modules.push(def_mod);
    logic.templates.insert("HqPvymnChinaCC".into(), cc);

    let mut tank = ThingTemplate::new("HqPvymnTank");
    tank.set_health(200.0);
    tank.add_kind_of(crate::game_logic::KindOf::Vehicle);
    logic.templates.insert("HqPvymnTank".into(), tank);

    let mut depot = ThingTemplate::new("HqPvymnDepot");
    depot.set_health(2000.0);
    depot
        .add_kind_of(crate::game_logic::KindOf::Structure)
        .add_kind_of(crate::game_logic::KindOf::FSSupplyCenter);
    depot.capturable = true;
    logic.templates.insert("HqPvymnDepot".into(), depot);

    let caster = logic
        .create_object("HqPvymnChinaCC", Team::China, Vec3::ZERO)
        .expect("cc");
    let tank_id = logic
        .create_object("HqPvymnTank", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("tank");
    let depot_id = logic
        .create_object("HqPvymnDepot", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("depot");
    // C++ SpecialPowerModule ctor (SpecialPowerModule.cpp:86-101) arms every
    // pre-built non-SharedNSync module with its authored ReloadTime, so a
    // fresh object is NOT ready. Simulate the reload having elapsed.
    if let Some(o) = logic.host_object_mut(caster) {
        o.set_special_power_ready_seconds(&SpecialPowerType::CashHack, 0.0);
        o.set_special_power_ready_seconds(&SpecialPowerType::Defector, 0.0);
    }

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power_at_location(
                &[caster],
                &SpecialPowerType::CashHack,
                Vec3::new(90.0, 0.0, 90.0),
            ),
            CommandResult::InvalidCommand
        );
    }
    assert!(
        logic.is_special_power_ready_for(caster, &SpecialPowerType::CashHack),
        "ground-click CashHack must not start recharge"
    );

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power_at_location(
                &[caster],
                &SpecialPowerType::Defector,
                Vec3::new(90.0, 0.0, 90.0),
            ),
            CommandResult::InvalidCommand
        );
    }
    assert!(
        logic.is_special_power_ready_for(caster, &SpecialPowerType::Defector),
        "ground-click Defector must not start recharge"
    );

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power_at_object(
                &[caster],
                &SpecialPowerType::CashHack,
                tank_id,
            ),
            CommandResult::InvalidCommand
        );
    }
    assert!(
        logic.is_special_power_ready_for(caster, &SpecialPowerType::CashHack),
        "CashHack on a tank must not start recharge"
    );

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power_at_object(
                &[caster],
                &SpecialPowerType::Defector,
                depot_id,
            ),
            CommandResult::InvalidCommand
        );
    }
    assert!(
        logic.is_special_power_ready_for(caster, &SpecialPowerType::Defector),
        "Defector on a building must not start recharge"
    );

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power_at_object(
                &[caster],
                &SpecialPowerType::CashHack,
                depot_id,
            ),
            CommandResult::Success
        );
    }
    assert!(
        !logic.is_special_power_ready_for(caster, &SpecialPowerType::CashHack),
        "valid CashHack object fire must consume the charge"
    );
}

/// C++ SpecialAbilityMicrowaveDisableBuilding is SPECIAL_HACKER_DISABLE_BUILDING.
/// The command must start the disable-building channel, not hard-reject.
#[test]
fn microwave_disable_building_starts_hdb_channel() {
    use crate::command_system::PowerTarget;
    use crate::game_logic::{
        AIState, HackerDisableBuildingMetadata, HackerDisableChannelPhase, KindOf,
    };

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::China, "China", false));
    logic.add_player(Player::new(1, Team::USA, "USA", false));

    let mut tank = ThingTemplate::new("HqY2oosMicrowaveTank");
    tank.set_health(400.0);
    tank.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable);
    tank.special_power_modules.push(test_module(
        SpecialPowerType::MicrowaveDisableBuilding,
        "SpecialAbilityMicrowaveDisableBuilding",
    ));
    tank.hacker_disable_building = Some(HackerDisableBuildingMetadata {
        special_power_template: "SpecialAbilityMicrowaveDisableBuilding".to_string(),
        update_module_starts_attack: true,
        starts_paused: false,
        scripted_special_power_only: false,
        reload_time_frames: 120,
        required_science: None,
        shared_n_sync: false,
        start_ability_range: 150.0,
        ability_abort_range: 10_000_000.0,
        approach_requires_los: true,
        unpack_time_ms: 1,
        preparation_time_ms: 1,
        persistent_prep_time_ms: 1,
        effect_duration_ms: 1,
        pack_time_ms: 1,
        pack_unpack_variation_factor: 0.0,
        persistence_requires_recharge: false,
    });
    logic.templates.insert("HqY2oosMicrowaveTank".into(), tank);

    let mut building = ThingTemplate::new("HqY2oosEnemyBuilding");
    building.set_health(2000.0);
    building.add_kind_of(KindOf::Structure);
    building.capturable = true;
    logic
        .templates
        .insert("HqY2oosEnemyBuilding".into(), building);

    let tank_id = logic
        .create_object_for_player("HqY2oosMicrowaveTank", 0, Vec3::new(200.0, 0.0, 0.0))
        .expect("microwave tank");
    let building_id = logic
        .create_object_for_player("HqY2oosEnemyBuilding", 1, Vec3::ZERO)
        .expect("enemy building");

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[tank_id],
                &SpecialPowerType::MicrowaveDisableBuilding,
                &PowerTarget::Object(building_id),
            ),
            CommandResult::Success,
            "Microwave Disable Building must not be hard-rejected"
        );
    }
    let issued = logic.host_object(tank_id).expect("tank after issue");
    assert_eq!(issued.ai_state, AIState::SpecialAbility);
    assert_eq!(issued.target, Some(building_id));
    assert_eq!(
        issued
            .hacker_disable_channel
            .expect("microwave must start the disable-building channel")
            .phase,
        HackerDisableChannelPhase::Approaching
    );
    assert!(
        !logic
            .host_object(building_id)
            .expect("building")
            .is_hacked_disabled(),
        "click must not apply a remote/instant disable"
    );
}

#[test]
fn baikonur_location_does_not_open_door_object_does_not_spend() {
    use crate::command_system::PowerTarget;
    use crate::game_logic::KindOf;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::GLA, "GLA", true));

    let mut tower_mod = test_module(
        SpecialPowerType::BaikonurRocket,
        "SuperweaponLaunchBaikonurRocket",
    );
    tower_mod.module_kind = SpecialPowerModuleKind::BaikonurLaunchPower;
    tower_mod.reload_time_frames = 7_200;

    let mut tower = ThingTemplate::new("HqMsvesBaikonur");
    tower.set_health(5000.0);
    tower.add_kind_of(KindOf::Structure);
    tower.special_power_modules.push(tower_mod);
    logic.templates.insert("HqMsvesBaikonur".into(), tower);

    let mut dummy = ThingTemplate::new("HqMsvesDummy");
    dummy.set_health(100.0);
    dummy.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("HqMsvesDummy".into(), dummy);

    let tower_id = logic
        .create_object_for_player("HqMsvesBaikonur", 0, Vec3::ZERO)
        .expect("tower");
    let dummy_id = logic
        .create_object_for_player("HqMsvesDummy", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("dummy");
    // C++ ctor arms the pre-built module (SpecialPowerModule.cpp:86-101);
    // simulate the authored reload having elapsed before the click.
    if let Some(o) = logic.host_object_mut(tower_id) {
        o.set_special_power_ready_seconds(&SpecialPowerType::BaikonurRocket, 0.0);
    }

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[tower_id],
                &SpecialPowerType::BaikonurRocket,
                &PowerTarget::Object(dummy_id),
            ),
            CommandResult::InvalidCommand
        );
    }
    assert!(
        logic.is_special_power_ready_for(tower_id, &SpecialPowerType::BaikonurRocket),
        "object click must not spend the Baikonur charge"
    );
    let bits = logic.host_object(tower_id).unwrap().model_condition_bits;
    assert_eq!(bits, 0, "object click must not open the door");

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[tower_id],
                &SpecialPowerType::BaikonurRocket,
                &PowerTarget::Location(Vec3::new(100.0, 0.0, 50.0)),
            ),
            CommandResult::Success
        );
    }
    let bits = logic.host_object(tower_id).unwrap().model_condition_bits;
    assert_eq!(bits, 0, "location fire must not set DOOR_1_OPENING");
    assert!(
        logic
            .host_objects()
            .values()
            .any(|o| o.template_name == "BaikonurRocketDetonation")
    );

    assert!(
        !logic.is_special_power_ready_for(tower_id, &SpecialPowerType::BaikonurRocket),
        "location fire consumes the charge"
    );
}

#[test]
fn battleship_object_target_locks_object_not_position() {
    use crate::command_system::PowerTarget;
    use crate::game_logic::KindOf;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::GLA, "GLA", false));

    let mut ship_mod = test_module(
        SpecialPowerType::BattleshipBombardment,
        "SpecialPowerBattleshipBombardment",
    );
    ship_mod.module_kind = SpecialPowerModuleKind::FireWeaponPower;
    ship_mod.reload_time_frames = 300;

    let mut ship = ThingTemplate::new("HqFwovsBattleship");
    ship.set_health(2000.0);
    ship.add_kind_of(KindOf::Vehicle);
    ship.special_power_modules.push(ship_mod);
    logic.templates.insert("HqFwovsBattleship".into(), ship);

    let mut tgt = ThingTemplate::new("HqFwovsTarget");
    tgt.set_health(400.0);
    tgt.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("HqFwovsTarget".into(), tgt);

    let ship_id = logic
        .create_object_for_player("HqFwovsBattleship", 0, Vec3::ZERO)
        .expect("ship");
    let tgt_id = logic
        .create_object_for_player("HqFwovsTarget", 1, Vec3::new(80.0, 0.0, 0.0))
        .expect("tgt");
    if let Some(o) = logic.host_object_mut(ship_id) {
        o.turret_enabled = true;
    }
    // C++ ctor arms the pre-built module (SpecialPowerModule.cpp:86-101);
    // simulate the authored reload having elapsed before the click.
    if let Some(o) = logic.host_object_mut(ship_id) {
        o.set_special_power_ready_seconds(&SpecialPowerType::BattleshipBombardment, 0.0);
    }

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[ship_id],
                &SpecialPowerType::BattleshipBombardment,
                &PowerTarget::Object(tgt_id),
            ),
            CommandResult::Success
        );
    }
    let ship = logic.host_object(ship_id).expect("ship after fire");
    assert_eq!(ship.target, Some(tgt_id));
    assert!(ship.target_location.is_none());
    assert_eq!(ship.turret_target_id, Some(tgt_id));
    assert!(
        ship.fire_weapon_power
            .as_ref()
            .is_some_and(|r| r.target_object_id == Some(tgt_id) && !r.has_location)
    );
}

#[test]
fn leftover_object_click_gates_match_action_manager() {
    use crate::command_executor::special_power::{leftover_can_do_special_power, leftover_can_do_special_power_at_location, leftover_can_do_special_power_at_object};
    use crate::command_system::SpecialPowerType;
    use gamelogic::common::Relationship;

    let click = |power: SpecialPowerType, rel: Relationship, vehicle: bool| {
        leftover_can_do_special_power_at_object(&power, rel, vehicle, false, false)
    };

    assert!(!click(
        SpecialPowerType::BattleshipBombardment,
        Relationship::Allies,
        false,
    ));
    assert!(click(
        SpecialPowerType::BattleshipBombardment,
        Relationship::Enemies,
        false,
    ));
    assert!(click(
        SpecialPowerType::BattleshipBombardment,
        Relationship::Neutral,
        false,
    ));
    assert!(click(
        SpecialPowerType::MissileDefenderLaserGuided,
        Relationship::Enemies,
        true,
    ));
    assert!(click(
        SpecialPowerType::LaserGuidedHowitzer,
        Relationship::Enemies,
        true,
    ));
    assert!(!click(
        SpecialPowerType::MissileDefenderLaserGuided,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::MissileDefenderLaserGuided,
        Relationship::Allies,
        true,
    ));
    assert!(!click(
        SpecialPowerType::MissileDefenderLaserGuided,
        Relationship::Neutral,
        true,
    ));
    assert!(!click(
        SpecialPowerType::Frenzy,
        Relationship::Allies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::Airstrike,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::DaisyCutter,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::Paradrop,
        Relationship::Neutral,
        false,
    ));
    assert!(!click(
        SpecialPowerType::CrateDrop,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::ParticleCannon,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::NuclearMissile,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::LeafletDrop,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::GpsScrambler,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::EmergencyRepair,
        Relationship::Allies,
        true,
    ));
    assert!(!click(
        SpecialPowerType::SneakAttack,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::Ambush,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::CleanupArea,
        Relationship::Neutral,
        false,
    ));
    assert!(!click(
        SpecialPowerType::TankParadrop,
        Relationship::Enemies,
        true,
    ));
    assert!(!click(
        SpecialPowerType::CiaIntelligence,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::CommunicationsDownload,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::DetonateDirtyNuke,
        Relationship::Enemies,
        false,
    ));
    assert!(!click(
        SpecialPowerType::BaikonurRocket,
        Relationship::Enemies,
        false,
    ));
    assert!(leftover_can_do_special_power(
        &SpecialPowerType::CiaIntelligence
    ));
    assert!(leftover_can_do_special_power(
        &SpecialPowerType::CommunicationsDownload
    ));
    assert!(leftover_can_do_special_power(
        &SpecialPowerType::DetonateDirtyNuke
    ));
    assert!(leftover_can_do_special_power(
        &SpecialPowerType::BurtonRemoteCharges
    ));
    assert!(leftover_can_do_special_power(
        &SpecialPowerType::BaikonurRocket
    ));
    assert!(!leftover_can_do_special_power(
        &SpecialPowerType::SpySatellite
    ));
    let empty = Vec3::new(10.0, 0.0, 10.0);
    assert!(!leftover_can_do_special_power_at_location(
        &SpecialPowerType::CiaIntelligence,
        empty,
        0,
    ));
    assert!(!leftover_can_do_special_power_at_location(
        &SpecialPowerType::CommunicationsDownload,
        empty,
        0,
    ));
    assert!(!leftover_can_do_special_power_at_location(
        &SpecialPowerType::DetonateDirtyNuke,
        empty,
        0,
    ));
    assert!(!leftover_can_do_special_power_at_location(
        &SpecialPowerType::BurtonRemoteCharges,
        empty,
        0,
    ));
    assert!(leftover_can_do_special_power_at_location(
        &SpecialPowerType::BaikonurRocket,
        empty,
        0,
    ));

    // C++ ActionManager.cpp:1569-1590 dead / FOGGED preamble before type switch.
    assert!(!leftover_can_do_special_power_at_object(
        &SpecialPowerType::BattleshipBombardment,
        Relationship::Enemies,
        false,
        true,
        false,
    ));
    assert!(!leftover_can_do_special_power_at_object(
        &SpecialPowerType::BattleshipBombardment,
        Relationship::Enemies,
        false,
        false,
        true,
    ));
    assert!(!leftover_can_do_special_power_at_object(
        &SpecialPowerType::MissileDefenderLaserGuided,
        Relationship::Enemies,
        true,
        false,
        true,
    ));
    assert!(!leftover_can_do_special_power_at_object(
        &SpecialPowerType::CashHack,
        Relationship::Enemies,
        false,
        false,
        true,
    ));
    assert!(!leftover_can_do_special_power_at_object(
        &SpecialPowerType::Defector,
        Relationship::Enemies,
        true,
        true,
        false,
    ));
    assert!(!leftover_can_do_special_power_at_object(
        &SpecialPowerType::TankHunterTnt,
        Relationship::Enemies,
        true,
        false,
        true,
    ));
    assert!(leftover_can_do_special_power_at_object(
        &SpecialPowerType::TankHunterTnt,
        Relationship::Enemies,
        true,
        false,
        false,
    ));
    assert!(!leftover_can_do_special_power_at_object(
        &SpecialPowerType::RangerCaptureBuilding,
        Relationship::Enemies,
        false,
        false,
        true,
    ));
    assert!(!leftover_can_do_special_power_at_object(
        &SpecialPowerType::HackerDisableBuilding,
        Relationship::Enemies,
        false,
        false,
        true,
    ));

    assert!(leftover_can_do_special_power_at_object(
        &SpecialPowerType::BattleshipBombardment,
        Relationship::Enemies,
        false,
        false,
        false,
    ));
}

/// C++ ActionManager.cpp:1569-1590: human object-target specials refuse
/// FOGGED ghosts before the type switch. Capture / Hacker early returns
/// share the same leftover preamble.
#[test]
fn object_target_click_rejects_fogged_ghost() {
    use crate::command_system::PowerTarget;
    use crate::game_logic::KindOf;
    use gamelogic::common::ObjectShroudStatus;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    let _lock = crate::fow_rendering::shroud_test_isolation_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
    }

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::China, "HqA76exHuman", true));
    logic.add_player(Player::new(1, Team::USA, "HqA76exEnemy", false));
    if let Some(p) = logic.get_player_mut(1) {
        p.resources.supplies = 8_000;
    }

    let mut cash_mod = test_module(SpecialPowerType::CashHack, "SuperweaponCashHack");
    cash_mod.reload_time_frames = 7_200;
    let mut ship_mod = test_module(
        SpecialPowerType::BattleshipBombardment,
        "SpecialPowerBattleshipBombardment",
    );
    ship_mod.module_kind = SpecialPowerModuleKind::FireWeaponPower;
    ship_mod.reload_time_frames = 300;
    let mut tnt_mod = test_module(
        SpecialPowerType::TankHunterTnt,
        "SpecialAbilityTankHunterTNTAttack",
    );
    tnt_mod.module_kind = SpecialPowerModuleKind::SpecialAbility;
    tnt_mod.reload_time_frames = 225;
    tnt_mod.update_module_starts_attack = true;

    let mut cc = ThingTemplate::new("HqA76exChinaCC");
    cc.set_health(5000.0);
    cc.add_kind_of(KindOf::Structure);
    cc.special_power_modules.push(cash_mod);
    logic.templates.insert("HqA76exChinaCC".into(), cc);

    let mut ship = ThingTemplate::new("HqA76exBattleship");
    ship.set_health(2000.0);
    ship.add_kind_of(KindOf::Vehicle);
    ship.special_power_modules.push(ship_mod);
    logic.templates.insert("HqA76exBattleship".into(), ship);

    let mut hunter = ThingTemplate::new("ChinaInfantryTankHunter");
    hunter.set_health(100.0);
    hunter.add_kind_of(KindOf::Infantry);
    hunter.special_power_modules.push(tnt_mod);
    logic
        .templates
        .insert("ChinaInfantryTankHunter".into(), hunter);

    let mut depot = ThingTemplate::new("HqA76exDepot");
    depot.set_health(2000.0);
    depot
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSSupplyCenter);
    depot.capturable = true;
    logic.templates.insert("HqA76exDepot".into(), depot);

    let mut tank = ThingTemplate::new("HqA76exTank");
    tank.set_health(400.0);
    tank.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("HqA76exTank".into(), tank);

    let caster = logic
        .create_object_for_player("HqA76exChinaCC", 0, Vec3::ZERO)
        .expect("cc");
    let ship_id = logic
        .create_object_for_player("HqA76exBattleship", 0, Vec3::new(10.0, 0.0, 0.0))
        .expect("ship");
    let hunter_id = logic
        .create_object_for_player("ChinaInfantryTankHunter", 0, Vec3::new(20.0, 0.0, 0.0))
        .expect("hunter");
    let depot_id = logic
        .create_object_for_player("HqA76exDepot", 1, Vec3::new(80.0, 0.0, 0.0))
        .expect("depot");
    let tank_id = logic
        .create_object_for_player("HqA76exTank", 1, Vec3::new(90.0, 0.0, 0.0))
        .expect("tank");
    // C++ ctor arms pre-built non-SharedNSync modules
    // (SpecialPowerModule.cpp:86-101); simulate the reload having elapsed.
    if let Some(o) = logic.host_object_mut(caster) {
        o.set_special_power_ready_seconds(&SpecialPowerType::CashHack, 0.0);
    }
    if let Some(o) = logic.host_object_mut(ship_id) {
        o.set_special_power_ready_seconds(&SpecialPowerType::BattleshipBombardment, 0.0);
    }
    if let Some(o) = logic.host_object_mut(hunter_id) {
        o.set_special_power_ready_seconds(&SpecialPowerType::TankHunterTnt, 0.0);
    }

    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.set_host_object_shroud_status(0, depot_id.0, ObjectShroudStatus::Fogged);
        shroud.set_host_object_shroud_status(0, tank_id.0, ObjectShroudStatus::Fogged);
    }

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[caster],
                &SpecialPowerType::CashHack,
                &PowerTarget::Object(depot_id),
            ),
            CommandResult::InvalidCommand,
            "FOGGED supply ghost must refuse CashHack"
        );
        assert_eq!(
            exec.execute_special_power(
                &[ship_id],
                &SpecialPowerType::BattleshipBombardment,
                &PowerTarget::Object(tank_id),
            ),
            CommandResult::InvalidCommand,
            "FOGGED tank ghost must refuse Battleship"
        );
        assert_eq!(
            exec.execute_special_power(
                &[hunter_id],
                &SpecialPowerType::TankHunterTnt,
                &PowerTarget::Object(tank_id),
            ),
            CommandResult::InvalidCommand,
            "FOGGED tank ghost must refuse Tank Hunter TNT"
        );
    }
    assert!(
        logic.is_special_power_ready_for(caster, &SpecialPowerType::CashHack),
        "FOGGED CashHack click must not consume charge"
    );
    assert!(
        logic.is_special_power_ready_for(ship_id, &SpecialPowerType::BattleshipBombardment),
        "FOGGED Battleship click must not consume charge"
    );
    assert!(
        logic.pending_special_ability(hunter_id).is_none(),
        "FOGGED TNT click must not queue a plant"
    );

    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.set_host_object_shroud_status(0, depot_id.0, ObjectShroudStatus::Clear);
        shroud.set_host_object_shroud_status(0, tank_id.0, ObjectShroudStatus::Clear);
    }

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[caster],
                &SpecialPowerType::CashHack,
                &PowerTarget::Object(depot_id),
            ),
            CommandResult::Success,
            "CLEAR depot must accept CashHack"
        );
    }
    assert!(
        !logic.is_special_power_ready_for(caster, &SpecialPowerType::CashHack),
        "valid CLEAR CashHack must consume the charge"
    );

    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
    }
}

#[test]
fn location_power_unit_click_leftover_gates_shroud() {
    use crate::command_system::PowerTarget;
    use crate::game_logic::KindOf;
    use gamelogic::system::shroud_manager::get_shroud_manager;

    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(512.0, 512.0);
    }

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::China, "China", false));

    let mut a10_mod = test_module(
        SpecialPowerType::Airstrike,
        "SuperweaponA10ThunderboltMissileStrike",
    );
    a10_mod.reload_time_frames = 300;

    let mut cc = ThingTemplate::new("HqHr2aeCommandCenter");
    cc.set_health(5000.0);
    cc.special_power_modules.push(a10_mod);
    logic.templates.insert("HqHr2aeCommandCenter".into(), cc);

    let mut enemy = ThingTemplate::new("HqHr2aeEnemy");
    enemy.set_health(200.0);
    enemy.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("HqHr2aeEnemy".into(), enemy);

    let cc_id = logic
        .create_object_for_player("HqHr2aeCommandCenter", 0, Vec3::ZERO)
        .expect("cc");
    let enemy_id = logic
        .create_object_for_player("HqHr2aeEnemy", 1, Vec3::new(80.0, 0.0, 40.0))
        .expect("enemy");
    // GameLogic::new() resets the shroud world (clear_all drops the grid);
    // (re)initialize the 512x shroud grid so the leftover cell-shroud gate
    // sees CELLSHROUD_SHROUDED the way a loaded C++ map does.
    {
        let mut shroud = get_shroud_manager().lock().expect("shroud");
        shroud.clear_all();
        shroud.init_shroud_grid(512.0, 512.0);
    }
    // simulate the authored reload having elapsed before the click.
    if let Some(o) = logic.host_object_mut(cc_id) {
        o.set_special_power_ready_seconds(&SpecialPowerType::Airstrike, 0.0);
    }

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[cc_id],
                &SpecialPowerType::Airstrike,
                &PowerTarget::Object(enemy_id),
            ),
            CommandResult::InvalidLocation,
            "C++/leftover refuse NEED_TARGET_POS unit clicks on CELLSHROUD_SHROUDED"
        );
    }
    assert!(
        logic.is_special_power_ready_for(cc_id, &SpecialPowerType::Airstrike),
        "shrouded location-power unit click must not consume charge"
    );

    if let Ok(mut shroud) = get_shroud_manager().lock() {
        shroud.clear_all();
    }
}

#[test]
fn location_power_unit_click_leftover_gates_underwater_paradrop() {
    use crate::command_system::PowerTarget;
    use crate::game_logic::KindOf;
    use gamelogic::common::{AsciiString, ICoord3D};
    use gamelogic::polygon_trigger::PolygonTrigger;
    use gamelogic::system::map_loader::MapData;

    // C++ ActionManager.cpp:1459-1468: paradrop / crate-drop / tank-paradrop
    // refuse underwater. Unit-under-cursor is AT_LOCATION at the object's pos.
    struct ResetLeftoverTerrain;
    impl Drop for ResetLeftoverTerrain {
        fn drop(&mut self) {
            if let Ok(mut tl) = gamelogic::terrain::get_terrain_logic().write() {
                tl.reset();
            }
        }
    }
    let _reset_terrain = ResetLeftoverTerrain;
    if let Ok(mut tl) = gamelogic::terrain::get_terrain_logic().write() {
        tl.reset();
    }
    {
        let mut trigger = PolygonTrigger::new(3, AsciiString::from("HqHr2aeLake"), Vec::new());
        trigger.set_water_area(true);
        trigger.add_point(ICoord3D::new(0, 0, 12));
        trigger.add_point(ICoord3D::new(200, 0, 12));
        trigger.add_point(ICoord3D::new(200, 200, 12));
        trigger.add_point(ICoord3D::new(0, 200, 12));
        let mut map_data = MapData::new();
        map_data.polygon_triggers.push(trigger);
        if let Ok(mut tl) = gamelogic::terrain::get_terrain_logic().write() {
            tl.load_map_data(map_data);
        }
    }

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));

    let mut para_mod = test_module(SpecialPowerType::Paradrop, "SuperweaponParadropAmerica");
    para_mod.reload_time_frames = 300;

    let mut cc = ThingTemplate::new("HqHr2aeParaCC");
    cc.set_health(5000.0);
    cc.special_power_modules.push(para_mod);
    logic.templates.insert("HqHr2aeParaCC".into(), cc);

    let mut enemy = ThingTemplate::new("HqHr2aeWaterUnit");
    enemy.set_health(200.0);
    enemy.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("HqHr2aeWaterUnit".into(), enemy);

    let cc_id = logic
        .create_object_for_player("HqHr2aeParaCC", 0, Vec3::ZERO)
        .expect("cc");
    let enemy_id = logic
        .create_object_for_player("HqHr2aeWaterUnit", 0, Vec3::new(80.0, 0.0, 40.0))
        .expect("enemy");
    // C++ ctor arms the pre-built module (SpecialPowerModule.cpp:86-101);
    // simulate the authored reload having elapsed before the click.
    if let Some(o) = logic.host_object_mut(cc_id) {
        o.set_special_power_ready_seconds(&SpecialPowerType::Paradrop, 0.0);
    }

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[cc_id],
                &SpecialPowerType::Paradrop,
                &PowerTarget::Object(enemy_id),
            ),
            CommandResult::InvalidLocation,
            "C++/leftover refuse NEED_TARGET_POS unit clicks underwater for paradrop"
        );
    }
    assert!(
        logic.is_special_power_ready_for(cc_id, &SpecialPowerType::Paradrop),
        "underwater location-power unit click must not consume charge"
    );

    if let Ok(mut tl) = gamelogic::terrain::get_terrain_logic().write() {
        tl.reset();
    }
}

#[test]
fn battleship_object_click_rejects_allies_without_consuming() {
    use crate::command_system::PowerTarget;
    use crate::game_logic::KindOf;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));

    let mut ship_mod = test_module(
        SpecialPowerType::BattleshipBombardment,
        "SpecialPowerBattleshipBombardment",
    );
    ship_mod.module_kind = SpecialPowerModuleKind::FireWeaponPower;
    ship_mod.reload_time_frames = 300;

    let mut ship = ThingTemplate::new("HqI9iw1Battleship");
    ship.set_health(2000.0);
    ship.add_kind_of(KindOf::Vehicle);
    ship.special_power_modules.push(ship_mod);
    logic.templates.insert("HqI9iw1Battleship".into(), ship);

    let mut tgt = ThingTemplate::new("HqI9iw1Ally");
    tgt.set_health(400.0);
    tgt.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("HqI9iw1Ally".into(), tgt);

    let ship_id = logic
        .create_object_for_player("HqI9iw1Battleship", 0, Vec3::ZERO)
        .expect("ship");
    let ally_id = logic
        .create_object_for_player("HqI9iw1Ally", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("ally");
    // C++ ctor arms the pre-built module (SpecialPowerModule.cpp:86-101);
    // simulate the authored reload having elapsed before the click.
    if let Some(o) = logic.host_object_mut(ship_id) {
        o.set_special_power_ready_seconds(&SpecialPowerType::BattleshipBombardment, 0.0);
    }

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[ship_id],
                &SpecialPowerType::BattleshipBombardment,
                &PowerTarget::Object(ally_id),
            ),
            CommandResult::InvalidCommand,
            "C++ ActionManager refuses allied battleship object clicks"
        );
    }
    let ship = logic.host_object(ship_id).expect("ship");
    assert!(ship.fire_weapon_power.is_none());
    assert!(
        logic.is_special_power_ready_for(ship_id, &SpecialPowerType::BattleshipBombardment),
        "illegal ally click must not start recharge"
    );
}

#[test]
fn laser_object_click_requires_enemy_vehicle() {
    use crate::command_system::PowerTarget;
    use crate::game_logic::KindOf;
    use crate::game_logic::host_missile_defender::{
        missile_defender_laser_guided_weapon, missile_defender_primary_weapon,
    };
    use gamelogic::common::Relationship;

    let mut logic = GameLogic::new();
    let mut usa = Player::new(0, Team::USA, "USA", true);
    let mut gla = Player::new(1, Team::GLA, "GLA", false);
    usa.set_map_relationship(1, Relationship::Enemies);
    gla.set_map_relationship(0, Relationship::Enemies);
    logic.add_player(usa);
    logic.add_player(gla);

    let mut laser_mod = test_module(
        SpecialPowerType::MissileDefenderLaserGuided,
        "SpecialAbilityMissileDefenderLaserGuidedMissiles",
    );
    laser_mod.module_kind = SpecialPowerModuleKind::SpecialAbility;

    let mut md = ThingTemplate::new("AmericaInfantryMissileDefender");
    md.set_health(100.0);
    md.add_kind_of(KindOf::Infantry);
    md.special_power_modules.push(laser_mod);
    logic
        .templates
        .insert("AmericaInfantryMissileDefender".into(), md);

    let mut tank = ThingTemplate::new("Hq4jxbcEnemyTank");
    tank.set_health(400.0);
    tank.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("Hq4jxbcEnemyTank".into(), tank);

    let mut rebel = ThingTemplate::new("Hq4jxbcRebel");
    rebel.set_health(100.0);
    rebel.add_kind_of(KindOf::Infantry);
    logic.templates.insert("Hq4jxbcRebel".into(), rebel);

    let mut ally_t = ThingTemplate::new("Hq4jxbcAllyTank");
    ally_t.set_health(400.0);
    ally_t.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("Hq4jxbcAllyTank".into(), ally_t);

    let md_id = logic
        .create_object_for_player("AmericaInfantryMissileDefender", 0, Vec3::ZERO)
        .expect("md");
    if let Some(o) = logic.host_object_mut(md_id) {
        o.weapon = Some(missile_defender_primary_weapon());
        o.secondary_weapon = Some(missile_defender_laser_guided_weapon());
    }
    let enemy_tank = logic
        .create_object_for_player("Hq4jxbcEnemyTank", 1, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy tank");
    let enemy_inf = logic
        .create_object_for_player("Hq4jxbcRebel", 1, Vec3::new(50.0, 0.0, 0.0))
        .expect("rebel");
    let ally_tank = logic
        .create_object_for_player("Hq4jxbcAllyTank", 0, Vec3::new(60.0, 0.0, 0.0))
        .expect("ally tank");

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_special_power(
                &[md_id],
                &SpecialPowerType::MissileDefenderLaserGuided,
                &PowerTarget::Object(enemy_inf),
            ),
            CommandResult::InvalidCommand,
            "leftover ActionManager refuses infantry laser lock"
        );
        assert_eq!(
            exec.execute_special_power(
                &[md_id],
                &SpecialPowerType::MissileDefenderLaserGuided,
                &PowerTarget::Object(ally_tank),
            ),
            CommandResult::InvalidCommand,
            "leftover ActionManager refuses allied laser lock"
        );
        assert_eq!(
            exec.execute_special_power(
                &[md_id],
                &SpecialPowerType::MissileDefenderLaserGuided,
                &PowerTarget::Object(enemy_tank),
            ),
            CommandResult::Success,
            "leftover ActionManager allows enemy vehicle laser lock"
        );
    }
}
