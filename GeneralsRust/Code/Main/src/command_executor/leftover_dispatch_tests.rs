use crate::command_executor::{CommandExecutor, leftover::*};
use crate::command_executor::validate::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, ObjectType, PendingSpecialAbility, Resources, Team,
    radar_notifications::RadarKind,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::AsciiString;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};
use crate::command_system::ModifierKeys;
use crate::game_logic::Player;
use std::time::SystemTime;

fn command(player_id: u32, command_type: CommandType, selected: Vec<ObjectId>) -> GameCommand {
    GameCommand {
        command_type,
        player_id,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: selected,
        modifier_keys: ModifierKeys::default(),
    }
}

#[test]
fn switch_weapons_locks_button_slot() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    let mut tpl = crate::game_logic::ThingTemplate::new("HumveeSW");
    tpl.set_health(200.0);
    logic.templates.insert("HumveeSW".into(), tpl);
    let id = logic
        .create_object_for_player("HumveeSW", 0, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(id).expect("obj");
        unit.weapon = Some(crate::game_logic::Weapon {
            damage: 1.0,
            range: 10.0,
            ..crate::game_logic::Weapon::default()
        });
        unit.secondary_weapon = Some(crate::game_logic::Weapon {
            damage: 5.0,
            range: 20.0,
            ..crate::game_logic::Weapon::default()
        });
        unit.active_weapon_slot = 0;
    }
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(0, CommandType::SwitchWeapons { slot: 1 }, vec![id]))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    let unit = logic.host_object(id).expect("obj");
    assert_eq!(unit.weapon_lock_slot, 1);
    assert_eq!(unit.active_weapon_slot, 1);
}

#[test]
fn enable_retaliation_reaches_host_player() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    assert!(
        !logic
            .get_player(0)
            .unwrap()
            .logical_retaliation_mode_enabled
    );
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::EnableRetaliationMode {
                player_index: 0,
                enabled: true,
            },
            vec![],
        ))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    assert!(
        logic
            .get_player(0)
            .unwrap()
            .logical_retaliation_mode_enabled
    );
}

#[test]
fn leftover_dispatch_tick_posts_enable_retaliation() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    logic.frame = 30;
    game_engine::common::global_data::write().client_retaliation_mode_enabled = true;
    logic.leftover_dispatch_tick();
    assert!(logic.has_pending_commands());
    logic.process_commands();
    assert!(
        logic
            .get_player(0)
            .unwrap()
            .logical_retaliation_mode_enabled
    );
}

#[test]
fn self_destruct_transfers_to_living_ally() {
    let mut logic = GameLogic::new();
    let mut p0 = Player::new(0, Team::USA, "P0", true);
    let mut p1 = Player::new(1, Team::USA, "P1", false);
    p0.alliance_team = 1;
    p1.alliance_team = 1;
    p0.resources.supplies = 4_000;
    p1.resources.supplies = 1_000;
    logic.get_players_mut().insert(0, p0);
    logic.get_players_mut().insert(1, p1);
    let mut tpl = crate::game_logic::ThingTemplate::new("RangerSD");
    tpl.set_health(100.0);
    logic.templates.insert("RangerSD".into(), tpl);
    let id = logic
        .create_object_for_player("RangerSD", 0, Vec3::ZERO)
        .expect("unit");
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::SelfDestruct {
                transfer_to_ally: true,
            },
            vec![],
        ))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    assert!(!logic.get_player(0).unwrap().is_alive);
    assert_eq!(logic.host_object(id).unwrap().owner_player_id, Some(1));
    assert_eq!(logic.get_player(0).unwrap().effective_supplies(), 0);
    assert_eq!(logic.get_player(1).unwrap().effective_supplies(), 5_000);
}

#[test]
fn self_destruct_without_ally_wipes_cash() {
    let mut logic = GameLogic::new();
    let mut p0 = Player::new(0, Team::USA, "P0", true);
    p0.resources.supplies = 2_500;
    logic.get_players_mut().insert(0, p0);
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::SelfDestruct {
                transfer_to_ally: false,
            },
            vec![],
        ))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    assert!(!logic.get_player(0).unwrap().is_alive);
    assert_eq!(logic.get_player(0).unwrap().effective_supplies(), 0);
}

#[test]
fn view_command_center_uses_own_player_not_same_faction() {
    // C++ viewCommandCenter iterates localPlayer objects only.
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    logic
        .get_players_mut()
        .insert(1, Player::new(1, Team::USA, "P1", false));
    let mut cc = crate::game_logic::ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::CommandCenter)
        .add_kind_of(KindOf::Structure)
        .set_health(1000.0);
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let mine = logic
        .create_object_for_player("AmericaCommandCenter", 0, Vec3::new(10.0, 0.0, 10.0))
        .expect("own CC");
    let theirs = logic
        .create_object_for_player("AmericaCommandCenter", 1, Vec3::new(500.0, 0.0, 500.0))
        .expect("enemy CC");
    let _ = (mine, theirs);
    let mine_pos = logic
        .player_command_center_position(0)
        .expect("own CC pose");
    assert!((mine_pos.x - 10.0).abs() < 0.1);
    assert!((mine_pos.z - 10.0).abs() < 0.1);
    let theirs_pos = logic.player_command_center_position(1).expect("p1 CC pose");
    assert!((theirs_pos.x - 500.0).abs() < 0.1);
}

#[test]
fn place_beacon_spawns_world_object_and_respects_cap() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    let loc = Vec3::new(10.0, 0.0, 12.0);
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::PlaceBeacon {
                location: loc,
                text: "here".into(),
            },
            vec![],
        ))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    let beacons: Vec<_> = logic
        .host_objects()
        .iter()
        .filter(|(_, o)| o.template_name.to_ascii_lowercase().contains("beacon"))
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(beacons.len(), 1);
    assert_eq!(host_beacon_caption(beacons[0]).as_deref(), Some("here"));

    let max = host_max_beacons_per_player().max(1);
    for i in 1..max {
        let r = CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::PlaceBeacon {
                    location: Vec3::new(20.0 + i as f32, 0.0, 0.0),
                    text: String::new(),
                },
                vec![],
            ))
            .expect("exec");
        assert_eq!(r, CommandResult::Success);
    }
    let overflow = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::PlaceBeacon {
                location: Vec3::new(99.0, 0.0, 99.0),
                text: String::new(),
            },
            vec![],
        ))
        .expect("exec");
    assert_eq!(overflow, CommandResult::InvalidCommand);
}

#[test]
fn set_beacon_text_updates_selected_caption() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::PlaceBeacon {
                location: Vec3::new(4.0, 0.0, 5.0),
                text: String::new(),
            },
            vec![],
        ))
        .unwrap();
    let id = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.template_name.to_ascii_lowercase().contains("beacon"))
        .map(|(id, _)| *id)
        .expect("beacon");
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::SetBeaconText { text: "go".into() },
            vec![id],
        ))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    assert_eq!(host_beacon_caption(id).as_deref(), Some("go"));
}

#[test]
fn enemy_place_hides_drawable_and_skips_host_beacons() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "Local", true));
    let mut enemy = Player::new(1, Team::GLA, "Enemy", false);
    enemy.alliance_team = 2;
    logic.get_players_mut().insert(1, enemy);
    logic.get_players_mut().get_mut(&0).unwrap().alliance_team = 1;
    let result = CommandExecutor::new(&mut logic, 1)
        .execute_command(command(
            1,
            CommandType::PlaceBeacon {
                location: Vec3::new(30.0, 0.0, 40.0),
                text: String::new(),
            },
            vec![],
        ))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    let id = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.template_name.to_ascii_lowercase().contains("beacon"))
        .map(|(id, _)| *id)
        .expect("beacon");
    assert!(host_beacon_is_hidden(id));
    assert!(logic.host_object(id).unwrap().drawable_hidden);
    assert!(
        logic.host_beacons().is_empty(),
        "enemy hide must not freeze onto host_beacons"
    );
}

#[test]
fn remove_beacon_empty_selection_is_noop_and_silent() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::PlaceBeacon {
                location: Vec3::new(8.0, 0.0, 9.0),
                text: String::new(),
            },
            vec![],
        ))
        .unwrap();
    let before = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.to_ascii_lowercase().contains("beacon"))
        .count();
    let audio_before = logic.queued_audio_events.len();
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(0, CommandType::RemoveBeacon, vec![]))
        .expect("exec");
    assert_eq!(result, CommandResult::InvalidCommand);
    let after = logic
        .host_objects()
        .values()
        .filter(|o| o.template_name.to_ascii_lowercase().contains("beacon"))
        .count();
    assert_eq!(before, after);
    assert_eq!(logic.queued_audio_events.len(), audio_before);
}

#[test]
fn remove_selected_beacon_is_silent() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::PlaceBeacon {
                location: Vec3::new(8.0, 0.0, 9.0),
                text: String::new(),
            },
            vec![],
        ))
        .unwrap();
    let id = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.template_name.to_ascii_lowercase().contains("beacon"))
        .map(|(id, _)| *id)
        .expect("beacon");
    let audio_before = logic.queued_audio_events.len();
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(0, CommandType::RemoveBeacon, vec![id]))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    assert_eq!(logic.queued_audio_events.len(), audio_before);
    assert!(logic.host_object(id).is_none() || !logic.host_object(id).unwrap().is_alive());
}

#[test]
fn beacon_client_update_pulses_after_frequency() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::PlaceBeacon {
                location: Vec3::new(11.0, 0.0, 12.0),
                text: String::new(),
            },
            vec![],
        ))
        .unwrap();
    tick_live_beacon_client_updates(&mut logic);
    logic.frame = logic.frame.saturating_add(31);
    tick_live_beacon_client_updates(&mut logic);
    let radar_system = game_engine::common::system::radar::get_radar_system();
    let radar = radar_system.read().expect("radar");
    assert!(
        radar.get_active_events().iter().any(|e| e.event_type
            == game_engine::common::system::radar::RadarEventType::BeaconPulse
            || e.event_type == game_engine::common::system::radar::RadarEventType::Information),
        "visible beacon must pulse or have place INFORMATION"
    );
}

#[test]
fn place_beacon_button_greys_at_max() {
    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    assert!(host_local_player_can_place_beacon(&logic, 0));
    let max = host_max_beacons_per_player().max(1);
    for i in 0..max {
        CommandExecutor::new(&mut logic, 0)
            .execute_command(command(
                0,
                CommandType::PlaceBeacon {
                    location: Vec3::new(i as f32 * 5.0, 0.0, 0.0),
                    text: String::new(),
                },
                vec![],
            ))
            .unwrap();
    }
    assert!(!host_local_player_can_place_beacon(&logic, 0));
}

#[test]
fn repair_mid_build_keeps_pending_build_and_idle_resumes() {
    // C++ DozerAIUpdate.cpp:1948 newTask slots + 1314 isBuildMostImportant.
    use crate::game_logic::ThingTemplate;
    let mut logic = GameLogic::new();
    logic.frame = 10;
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    let mut dozer_tpl = ThingTemplate::new("DozerParkBuild");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Worker)
        .set_health(300.0);
    logic.templates.insert("DozerParkBuild".into(), dozer_tpl);
    let mut bld = ThingTemplate::new("ScaffoldParkBuild");
    bld.add_kind_of(KindOf::Structure).set_health(500.0);
    logic
        .templates
        .insert("ScaffoldParkBuild".into(), bld.clone());
    logic.templates.insert("DamagedParkBuild".into(), bld);
    let dozer = logic
        .create_object_for_player("DozerParkBuild", 0, Vec3::ZERO)
        .expect("dozer");
    let scaffold = logic
        .create_object_for_player("ScaffoldParkBuild", 0, Vec3::new(20.0, 0.0, 0.0))
        .expect("scaffold");
    let damaged = logic
        .create_object_for_player("DamagedParkBuild", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("damaged");
    {
        let sc = logic.host_object_mut(scaffold).expect("sc");
        sc.status.under_construction = true;
        sc.builder_id = Some(dozer);
    }
    {
        let dmg = logic.host_object_mut(damaged).expect("dmg");
        let _ = dmg.take_damage(200.0);
    }
    {
        let dz = logic.host_object_mut(dozer).expect("dz");
        dz.target = Some(scaffold);
        dz.set_ai_state(AIState::Constructing);
    }
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::Repair { target_id: damaged },
            vec![dozer],
        ))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    {
        let dz = logic.host_object(dozer).expect("dz");
        assert_eq!(dz.ai_state, AIState::Repairing);
        assert_eq!(dz.target, Some(damaged));
        assert_eq!(
            dz.dozer_task_build_target,
            Some(scaffold),
            "REPAIR must keep BUILD pending"
        );
        assert_eq!(dz.dozer_task_repair_target, Some(damaged));
    }
    logic.dozer_internal_task_complete(dozer, true);
    if let Some(dz) = logic.host_object_mut(dozer) {
        dz.set_target(None);
        dz.set_ai_state(AIState::Idle);
    }
    assert!(
        logic.dozer_idle_resume_pending_build(dozer),
        "idle isBuildMostImportant must resume parked BUILD"
    );
    let dz = logic.host_object(dozer).expect("dz");
    assert_eq!(dz.ai_state, AIState::Constructing);
    assert_eq!(dz.target, Some(scaffold));
    assert_eq!(dz.dozer_task_build_target, Some(scaffold));
}

#[test]
fn same_frame_build_repair_tie_resumes_build() {
    // hq-ylhfz: C++ getMostRecentCommand uses `order_frame > mostRecentFrame`,
    // so BUILD (walked first) wins a same-frame BUILD+REPAIR tie.
    use crate::game_logic::ThingTemplate;
    let mut logic = GameLogic::new();
    logic.frame = 10;
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    let mut dozer_tpl = ThingTemplate::new("DozerSameFrameTie");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Worker)
        .set_health(300.0);
    logic
        .templates
        .insert("DozerSameFrameTie".into(), dozer_tpl);
    let mut bld = ThingTemplate::new("ScaffoldSameFrameTie");
    bld.add_kind_of(KindOf::Structure).set_health(500.0);
    logic
        .templates
        .insert("ScaffoldSameFrameTie".into(), bld.clone());
    logic.templates.insert("DamagedSameFrameTie".into(), bld);
    let dozer = logic
        .create_object_for_player("DozerSameFrameTie", 0, Vec3::ZERO)
        .expect("dozer");
    let scaffold = logic
        .create_object_for_player("ScaffoldSameFrameTie", 0, Vec3::new(20.0, 0.0, 0.0))
        .expect("scaffold");
    let damaged = logic
        .create_object_for_player("DamagedSameFrameTie", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("damaged");
    {
        let sc = logic.host_object_mut(scaffold).expect("sc");
        sc.status.under_construction = true;
        sc.builder_id = Some(dozer);
    }
    {
        let dmg = logic.host_object_mut(damaged).expect("dmg");
        let _ = dmg.take_damage(200.0);
    }
    logic.dozer_new_task_build(dozer, scaffold);
    logic.dozer_new_task_repair(dozer, damaged);
    {
        let dz = logic.host_object(dozer).expect("dz");
        assert_eq!(
            dz.dozer_task_build_order_frame,
            dz.dozer_task_repair_order_frame
        );
        assert_eq!(dz.dozer_task_build_target, Some(scaffold));
        assert_eq!(dz.dozer_task_repair_target, Some(damaged));
    }
    if let Some(dz) = logic.host_object_mut(dozer) {
        dz.set_target(None);
        dz.set_ai_state(AIState::Idle);
    }
    assert!(
        logic.dozer_idle_resume_pending_build(dozer),
        "hq-ylhfz: same-frame BUILD+REPAIR must resume BUILD"
    );
    let dz = logic.host_object(dozer).expect("dz");
    assert_eq!(dz.ai_state, AIState::Constructing);
    assert_eq!(dz.target, Some(scaffold));
}

#[test]
fn idle_resumes_parked_repair_when_most_recent() {
    // hq-ja2nm: C++ isRepairMostImportant resumes the parked REPAIR slot.
    use crate::game_logic::ThingTemplate;
    let mut logic = GameLogic::new();
    logic.frame = 10;
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    let mut dozer_tpl = ThingTemplate::new("DozerIdleRepair");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Worker)
        .set_health(300.0);
    logic.templates.insert("DozerIdleRepair".into(), dozer_tpl);
    let mut bld = ThingTemplate::new("DamagedIdleRepair");
    bld.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("DamagedIdleRepair".into(), bld);
    let dozer = logic
        .create_object_for_player("DozerIdleRepair", 0, Vec3::ZERO)
        .expect("dozer");
    let damaged = logic
        .create_object_for_player("DamagedIdleRepair", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("damaged");
    {
        let dmg = logic.host_object_mut(damaged).expect("dmg");
        let _ = dmg.take_damage(200.0);
    }
    logic.dozer_new_task_repair(dozer, damaged);
    if let Some(dz) = logic.host_object_mut(dozer) {
        dz.set_target(None);
        dz.set_ai_state(AIState::Idle);
    }
    assert!(
        logic.dozer_idle_resume_pending_build(dozer),
        "hq-ja2nm: idle isRepairMostImportant must resume parked REPAIR"
    );
    let dz = logic.host_object(dozer).expect("dz");
    assert_eq!(dz.ai_state, AIState::Repairing);
    assert_eq!(dz.target, Some(damaged));
    assert_eq!(dz.dozer_task_repair_target, Some(damaged));
}

#[test]
fn idle_resumes_repair_parked_by_newer_build() {
    // hq-ja2nm: BUILD parks REPAIR; after BUILD is dropped, idle resumes REPAIR.
    use crate::game_logic::ThingTemplate;
    let mut logic = GameLogic::new();
    logic.frame = 8;
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    let mut dozer_tpl = ThingTemplate::new("DozerParkedRepair");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Worker)
        .set_health(300.0);
    logic
        .templates
        .insert("DozerParkedRepair".into(), dozer_tpl);
    let mut bld = ThingTemplate::new("ScaffoldParkedRepair");
    bld.add_kind_of(KindOf::Structure).set_health(500.0);
    logic
        .templates
        .insert("ScaffoldParkedRepair".into(), bld.clone());
    logic.templates.insert("DamagedParkedRepair".into(), bld);
    let dozer = logic
        .create_object_for_player("DozerParkedRepair", 0, Vec3::ZERO)
        .expect("dozer");
    let scaffold = logic
        .create_object_for_player("ScaffoldParkedRepair", 0, Vec3::new(20.0, 0.0, 0.0))
        .expect("scaffold");
    let damaged = logic
        .create_object_for_player("DamagedParkedRepair", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("damaged");
    {
        let sc = logic.host_object_mut(scaffold).expect("sc");
        sc.status.under_construction = true;
        sc.builder_id = Some(dozer);
    }
    {
        let dmg = logic.host_object_mut(damaged).expect("dmg");
        let _ = dmg.take_damage(200.0);
    }
    logic.dozer_new_task_repair(dozer, damaged);
    logic.frame = 9;
    logic.dozer_new_task_build(dozer, scaffold);
    if let Some(dz) = logic.host_object_mut(dozer) {
        dz.set_target(None);
        dz.set_ai_state(AIState::Idle);
    }
    assert!(
        logic.dozer_idle_resume_pending_build(dozer),
        "newer BUILD still wins getMostRecentCommand"
    );
    assert_eq!(
        logic.host_object(dozer).unwrap().ai_state,
        AIState::Constructing
    );
    logic.dozer_internal_task_complete(dozer, false);
    if let Some(dz) = logic.host_object_mut(dozer) {
        dz.set_target(None);
        dz.set_ai_state(AIState::Idle);
    }
    assert!(
        logic.dozer_idle_resume_pending_build(dozer),
        "hq-ja2nm: after BUILD clears, idle must resume parked REPAIR"
    );
    let dz = logic.host_object(dozer).expect("dz");
    assert_eq!(dz.ai_state, AIState::Repairing);
    assert_eq!(dz.target, Some(damaged));
}

#[test]
fn player_stop_cancels_current_build_so_idle_does_not_resume() {
    // hq-msoee: C++ Worker/Dozer aiDoCommand default arm cancels
    // getCurrentTask() on CMD_FROM_PLAYER Stop/Move/Attack/Dock.
    use crate::game_logic::ThingTemplate;
    let mut logic = GameLogic::new();
    logic.frame = 10;
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    let mut dozer_tpl = ThingTemplate::new("DozerCancelBuild");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Worker)
        .set_health(300.0);
    logic.templates.insert("DozerCancelBuild".into(), dozer_tpl);
    let mut bld = ThingTemplate::new("ScaffoldCancelBuild");
    bld.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("ScaffoldCancelBuild".into(), bld);
    let dozer = logic
        .create_object_for_player("DozerCancelBuild", 0, Vec3::ZERO)
        .expect("dozer");
    let scaffold = logic
        .create_object_for_player("ScaffoldCancelBuild", 0, Vec3::new(20.0, 0.0, 0.0))
        .expect("scaffold");
    {
        let sc = logic.host_object_mut(scaffold).expect("sc");
        sc.status.under_construction = true;
        sc.builder_id = Some(dozer);
    }
    logic.dozer_new_task_build(dozer, scaffold);
    if let Some(dz) = logic.host_object_mut(dozer) {
        dz.target = Some(scaffold);
        dz.set_ai_state(AIState::Constructing);
        dz.set_actively_constructing(true);
    }
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(0, CommandType::Stop, vec![dozer]))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    {
        let dz = logic.host_object(dozer).expect("dz");
        assert_eq!(
            dz.dozer_task_build_target, None,
            "hq-msoee: player Stop must cancel current BUILD slot"
        );
    }
    {
        let sc = logic.host_object(scaffold).expect("sc");
        assert_eq!(
            sc.builder_id,
            Some(dozer),
            "C++ cancelTask does not clear builder_id"
        );
        assert!(sc.status.under_construction);
    }
    if let Some(dz) = logic.host_object_mut(dozer) {
        dz.set_ai_state(AIState::Idle);
    }
    assert!(
        !logic.dozer_idle_resume_pending_build(dozer),
        "hq-msoee: cancelled BUILD must not auto-resume"
    );
}

#[test]
fn execute_build_records_build_slot_and_docks_off_center() {
    // hq-gkpuk: C++ construct:1717 newTask BUILD.
    // hq-6gy32: C++ findGoodBuildOrRepairPosition half majorRadius + ignoreObstacle.
    use crate::game_logic::ThingTemplate;
    let mut logic = GameLogic::new();
    logic.frame = 12;
    let mut player = Player::new(0, Team::USA, "P0", true);
    player.resources.supplies = 100_000;
    logic.get_players_mut().insert(0, player);
    let mut dozer_tpl = ThingTemplate::new("AmericaVehicleDozerSlot");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Worker)
        .set_health(300.0);
    logic
        .templates
        .insert("AmericaVehicleDozerSlot".into(), dozer_tpl);
    let mut bld = ThingTemplate::new("AmericaBarracksSlot");
    bld.add_kind_of(KindOf::Structure)
        .set_cost(50, 0)
        .set_health(500.0);
    logic.templates.insert("AmericaBarracksSlot".into(), bld);
    let dozer = logic
        .create_object_for_player("AmericaVehicleDozerSlot", 0, Vec3::new(200.0, 0.0, 0.0))
        .expect("dozer");
    if let Some(dz) = logic.host_object_mut(dozer) {
        dz.selection_radius = 8.0;
    }
    let site = Vec3::new(0.0, 0.0, 0.0);
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::Build {
                template_name: "AmericaBarracksSlot".into(),
                location: site,
            },
            vec![dozer],
        ))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    let scaffold = logic
        .host_objects()
        .iter()
        .find(|(_, o)| o.template_name == "AmericaBarracksSlot")
        .map(|(id, _)| *id)
        .expect("scaffold");
    {
        let dz = logic.host_object(dozer).expect("dz");
        assert_eq!(
            dz.dozer_task_build_target,
            Some(scaffold),
            "hq-gkpuk: new construct must write BUILD slot"
        );
        assert!(dz.dozer_task_build_order_frame >= 12);
        assert_eq!(dz.target, Some(scaffold));
        assert_eq!(dz.ai_state, AIState::Constructing);
        let dest = dz
            .movement
            .target_position
            .or_else(|| dz.movement.path.last().copied())
            .unwrap_or(site);
        let pad = logic.host_object(scaffold).unwrap().get_position();
        let dx = dest.x - pad.x;
        let dz_ = dest.z - pad.z;
        assert!(
            dx * dx + dz_ * dz_ > 1.0,
            "hq-6gy32: dozer must dock off pad center, dest={dest:?} pad={pad:?}"
        );
    }
    {
        let sc = logic.host_object(scaffold).expect("sc");
        assert_eq!(sc.builder_id, Some(dozer));
        assert!(sc.status.under_construction);
    }
    if let Some(dz) = logic.host_object_mut(dozer) {
        dz.set_target(None);
        dz.set_ai_state(AIState::Idle);
        dz.set_position(Vec3::new(400.0, 0.0, 0.0));
    }
    assert!(
        logic.dozer_idle_resume_pending_build(dozer),
        "hq-gkpuk: idle isBuildMostImportant must resume a brand-new pad"
    );
}

#[test]
fn gather_cancels_build_and_blocks_idle_resume_while_supply() {
    // hq-5nio2: gather enters exclusive AS_SUPPLY_TRUCK.
    use crate::game_logic::{SupplyTruckState, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.frame = 22;
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::GLA, "P0", true));
    let mut worker_tpl = ThingTemplate::new("GLAWorkerGatherCancel");
    worker_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Harvester)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAWorkerGatherCancel".into(), worker_tpl);
    let mut pile = ThingTemplate::new("SupplyWarehouseGather");
    pile.add_kind_of(KindOf::Harvestable)
        .add_kind_of(KindOf::Structure)
        .set_health(200.0);
    logic.templates.insert("SupplyWarehouseGather".into(), pile);
    let mut bld = ThingTemplate::new("ScaffoldGatherCancel");
    bld.add_kind_of(KindOf::Structure).set_health(500.0);
    logic.templates.insert("ScaffoldGatherCancel".into(), bld);
    let worker = logic
        .create_object_for_player("GLAWorkerGatherCancel", 0, Vec3::ZERO)
        .expect("worker");
    let warehouse = logic
        .create_object_for_player("SupplyWarehouseGather", 0, Vec3::new(40.0, 0.0, 0.0))
        .expect("wh");
    let scaffold = logic
        .create_object_for_player("ScaffoldGatherCancel", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("scaffold");
    {
        let sc = logic.host_object_mut(scaffold).expect("sc");
        sc.status.under_construction = true;
        sc.builder_id = Some(worker);
    }
    logic.dozer_new_task_build(worker, scaffold);
    if let Some(w) = logic.host_object_mut(worker) {
        w.target = Some(scaffold);
        w.set_ai_state(AIState::Constructing);
    }
    let result = CommandExecutor::new(&mut logic, 0)
        .execute_command(command(
            0,
            CommandType::Gather {
                target_id: warehouse,
            },
            vec![worker],
        ))
        .expect("exec");
    assert_eq!(result, CommandResult::Success);
    {
        let w = logic.host_object(worker).expect("w");
        assert!(
            w.dozer_task_build_target.is_none(),
            "hq-5nio2: gather must cancel current BUILD"
        );
        assert_eq!(w.ai_state, AIState::Gathering);
    }
    if let Some(w) = logic.host_object_mut(worker) {
        w.set_ai_state(AIState::Idle);
        w.supply_truck_state = SupplyTruckState::Regrouping;
    }
    assert!(
        !logic.dozer_idle_resume_pending_build(worker),
        "hq-5nio2: AS_SUPPLY_TRUCK must not run idle dozer resume"
    );
}

#[test]
fn new_task_snaps_action_dock_off_pad_via_find_position_around() {
    // hq-z2plo: C++ findGoodBuildOrRepairPosition runs findPositionAround
    // so the stored ACTION dock is not the in-pad half-radius seed.
    use crate::game_logic::ThingTemplate;
    let mut logic = GameLogic::new();
    logic.frame = 4;
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    let mut dozer_tpl = ThingTemplate::new("AmericaVehicleDozerSnap");
    dozer_tpl
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(250.0);
    logic
        .templates
        .insert("AmericaVehicleDozerSnap".into(), dozer_tpl);
    let mut pad_tpl = ThingTemplate::new("AmericaWarFactorySnap");
    pad_tpl.add_kind_of(KindOf::Structure).set_health(1500.0);
    logic
        .templates
        .insert("AmericaWarFactorySnap".into(), pad_tpl);
    let dozer = logic
        .create_object_for_player(
            "AmericaVehicleDozerSnap",
            0,
            glam::Vec3::new(200.0, 0.0, 0.0),
        )
        .expect("dozer");
    let pad = logic
        .create_object_for_player("AmericaWarFactorySnap", 0, glam::Vec3::ZERO)
        .expect("pad");
    if let Some(p) = logic.host_object_mut(pad) {
        p.selection_radius = 80.0;
        p.status.under_construction = true;
    }
    if let Some(d) = logic.host_object_mut(dozer) {
        d.selection_radius = 8.0;
    }
    logic.dozer_new_task_build(dozer, pad);
    let dock = logic
        .host_object(dozer)
        .and_then(|d| d.dozer_dock_action)
        .expect("ACTION dock");
    let seed = crate::game_logic::host_repair::dozer_repair_approach_position(
        glam::Vec3::new(200.0, 0.0, 0.0),
        glam::Vec3::ZERO,
        80.0,
    );
    let dist = (dock.x * dock.x + dock.z * dock.z).sqrt();
    assert!(
        dist > 80.0,
        "findPositionAround must leave the pad, dock={dock:?} seed={seed:?}"
    );
    assert!(
        (dock - seed).length() > 1.0,
        "stored dock must not stay on the raw half-radius seed"
    );
}

#[test]
fn mine_clear_order_keeps_boxes_until_already_attacking() {
    // hq-6je29: initial clear-mines does not drop; mid-attack command does.
    use crate::game_logic::ThingTemplate;
    let mut logic = GameLogic::new();
    logic.frame = 8;
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::GLA, "P0", true));
    let mut worker_tpl = ThingTemplate::new("GLAWorkerMineBox");
    worker_tpl
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Worker)
        .add_kind_of(KindOf::Harvester)
        .set_health(100.0);
    logic
        .templates
        .insert("GLAWorkerMineBox".into(), worker_tpl);
    let worker = logic
        .create_object_for_player("GLAWorkerMineBox", 0, Vec3::ZERO)
        .expect("worker");
    if let Some(w) = logic.host_object_mut(worker) {
        w.set_stored_supplies(75);
        w.set_weapon_set_mine_clearing_detail(true);
    }
    logic.drop_worker_supply_boxes_for_mine_clear(worker);
    assert_eq!(
        logic.host_object(worker).unwrap().stored_resources.supplies,
        75,
        "hq-6je29: initial order is not isClearingMines"
    );
    if let Some(w) = logic.host_object_mut(worker) {
        w.set_ai_state(AIState::Attacking);
        w.status.attacking = true;
    }
    logic.drop_worker_supply_boxes_for_mine_clear(worker);
    assert_eq!(
        logic.host_object(worker).unwrap().stored_resources.supplies,
        0,
        "hq-6je29: mid-attack command drops boxes"
    );
}
