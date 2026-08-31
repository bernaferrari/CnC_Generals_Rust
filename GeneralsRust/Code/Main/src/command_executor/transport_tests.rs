use crate::command_executor::CommandExecutor;
use crate::command_executor::transport::*;
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
use crate::game_logic::{
    ContainAdmission, ContainModuleKind, ContainModuleMetadata, HostGeometryInfo, HostGeometryType,
    ThingTemplate,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::AsciiString;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

fn contain_pair(logic: &mut GameLogic, transport: ObjectId, pax: ObjectId) {
    {
        let t = logic.host_object_mut(transport).unwrap();
        assert!(t.add_occupant(pax));
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
        p.set_ai_state(AIState::Docked);
    }
}

#[test]
fn execute_exit_unloads_only_the_selected_occupant() {
    // C++ GameLogicDispatch.cpp:978-1004 MSG_EXIT exits argument 0 occupant,
    // not every rider on the selected container (that is MSG_EVACUATE).
    let mut logic = GameLogic::new();
    for (name, kind, transport) in [
        ("EXIT_T", KindOf::Vehicle, true),
        ("EXIT_A", KindOf::Infantry, false),
        ("EXIT_B", KindOf::Infantry, false),
    ] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(kind);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        if transport {
            tpl.contain_module = ContainModuleMetadata {
                kind: ContainModuleKind::Transport,
                slots: Some(5),
                ..Default::default()
            };
        }
        logic.templates.insert(name.to_string(), tpl);
    }
    let transport = logic
        .create_object("EXIT_T", Team::USA, Vec3::ZERO)
        .unwrap();
    let a = logic
        .create_object("EXIT_A", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("EXIT_B", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    contain_pair(&mut logic, transport, a);
    contain_pair(&mut logic, transport, b);

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[a]), CommandResult::Success);
    }
    let remaining = logic.host_object(transport).unwrap().contained_units();
    assert_eq!(remaining, vec![b], "EXIT must leave the unclicked occupant");
    assert!(logic.host_object(a).unwrap().contained_by.is_none());
    assert_eq!(logic.host_object(b).unwrap().contained_by, Some(transport));
}

#[test]
fn execute_combat_drop_uses_hover_not_land_evac() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_T");
    t.add_kind_of(KindOf::Aircraft);
    t.add_kind_of(KindOf::Selectable);
    t.set_health(200.0);
    logic.templates.insert("CD_T".to_string(), t);
    let mut p = ThingTemplate::new("CD_P");
    p.add_kind_of(KindOf::Infantry);
    p.add_kind_of(KindOf::Selectable);
    p.set_health(100.0);
    logic.templates.insert("CD_P".to_string(), p);
    let transport = logic
        .create_object("CD_T", Team::USA, Vec3::new(0.0, 100.0, 0.0))
        .unwrap();
    let pax = logic
        .create_object("CD_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.install_combat_chinook_transport();
        let _ = t.add_occupant(pax);
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
        p.set_ai_state(AIState::Docked);
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_combat_drop(
                &[transport],
                &DropTarget::Location(Vec3::new(80.0, 0.0, 80.0))
            ),
            CommandResult::Success
        );
    }
    let t = logic.host_object(transport).unwrap();
    assert!(t.pending_evacuate_on_stop);
    let ai = t.chinook_ai.as_ref().expect("chinook_ai");
    assert_eq!(
        ai.state,
        crate::game_logic::host_combat_chinook::HostChinookAIState::MoveToCombatDrop
    );
    assert_ne!(
        ai.flight_status,
        crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
    );
    assert!(
        logic.host_object(pax).unwrap().contained_by.is_some(),
        "passengers stay aboard until hover rappel"
    );
}

#[test]
fn execute_combat_drop_plays_voice_combat_drop() {
    // C++ CommandXlat.cpp:348-352 MSG_COMBATDROP_AT_* → first Chinook VoiceCombatDrop.
    use crate::game_logic::audio_dispatch_impl::{
        UnitVoiceSlot, clear_test_template_voices, set_test_template_voice,
    };
    clear_test_template_voices();
    set_test_template_voice("CD_VOICE", UnitVoiceSlot::CombatDrop, "TestVoiceCombatDrop");
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_VOICE");
    t.add_kind_of(KindOf::Aircraft);
    t.add_kind_of(KindOf::Selectable);
    t.set_health(200.0);
    logic.templates.insert("CD_VOICE".to_string(), t);
    let mut p = ThingTemplate::new("CD_VOICE_P");
    p.add_kind_of(KindOf::Infantry);
    p.add_kind_of(KindOf::Selectable);
    p.set_health(100.0);
    logic.templates.insert("CD_VOICE_P".to_string(), p);
    let transport = logic
        .create_object("CD_VOICE", Team::USA, Vec3::new(0.0, 100.0, 0.0))
        .unwrap();
    let pax = logic
        .create_object("CD_VOICE_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.install_combat_chinook_transport();
        let _ = t.add_occupant(pax);
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
        p.set_ai_state(AIState::Docked);
    }
    logic.queued_audio_events.clear();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_combat_drop(
                &[transport],
                &DropTarget::Location(Vec3::new(80.0, 0.0, 80.0))
            ),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestVoiceCombatDrop"),
        "execute_combat_drop must play VoiceCombatDrop: {:?}",
        logic.queued_audio_events
    );
    clear_test_template_voices();
}

#[test]
fn combat_drop_arrival_rappels_not_teleport() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_T2");
    t.add_kind_of(KindOf::Aircraft);
    t.set_health(200.0);
    logic.templates.insert("CD_T2".to_string(), t);
    let mut p = ThingTemplate::new("CD_P2");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("CD_P2".to_string(), p);
    let transport = logic
        .create_object("CD_T2", Team::USA, Vec3::new(10.0, 100.0, 10.0))
        .unwrap();
    let pax = logic
        .create_object("CD_P2", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.install_combat_chinook_transport();
        let _ = t.add_occupant(pax);
        if let Some(ai) = t.chinook_ai.as_mut() {
            ai.command_combat_drop([10.0, 10.0, 0.0], None);
            ai.arrive_for_combat_drop();
        }
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
    }
    assert!(logic.evacuate_container_now(transport, false));
    let pax_obj = logic.host_object(pax).unwrap();
    assert!(pax_obj.contained_by.is_none());
    assert!(
        pax_obj.get_position().y > 50.0,
        "rappeller starts at hover, not teleported to ground"
    );
    assert!((pax_obj.movement.max_speed - 30.0).abs() < 0.01);
    assert_eq!(
        logic
            .host_object(transport)
            .unwrap()
            .chinook_ai
            .as_ref()
            .unwrap()
            .ai_free_to_exit(true),
        crate::game_logic::host_combat_chinook::HostChinookFreeToExit::FreeToExit
    );
}

fn bunker_template(name: &str, height: f32, slots: usize) -> ThingTemplate {
    let mut b = ThingTemplate::new(name);
    b.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    b.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height,
        major_radius: 15.0,
        minor_radius: 15.0,
        authored: true,
    };
    b.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(slots),
        admission: ContainAdmission::InfantryOnly,
        allow_allies_inside: true,
        allow_enemies_inside: true,
        allow_neutral_inside: true,
        ..ContainModuleMetadata::default()
    };
    b
}

#[test]
fn combat_drop_rappel_uses_roof_dest_not_y0() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_T3");
    t.add_kind_of(KindOf::Aircraft);
    t.set_health(200.0);
    logic.templates.insert("CD_T3".to_string(), t);
    let mut p = ThingTemplate::new("CD_P3");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("CD_P3".to_string(), p);
    logic
        .templates
        .insert("CD_B3".to_string(), bunker_template("CD_B3", 24.0, 5));
    let transport = logic
        .create_object("CD_T3", Team::USA, Vec3::new(10.0, 100.0, 10.0))
        .unwrap();
    let bunker = logic
        .create_object("CD_B3", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .unwrap();
    let pax = logic
        .create_object("CD_P3", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.install_combat_chinook_transport();
        let _ = t.add_occupant(pax);
        t.set_order_target(Some(bunker));
        if let Some(ai) = t.chinook_ai.as_mut() {
            ai.command_combat_drop([10.0, 10.0, 0.0], Some(24.0));
            ai.arrive_for_combat_drop();
        }
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
    }
    assert!(logic.evacuate_container_now(transport, false));
    let pax_obj = logic.host_object(pax).unwrap();
    assert!(pax_obj.is_rappelling());
    assert!(
        (pax_obj.status.rappel_dest_y - 24.0).abs() < 0.01,
        "dest Y must be roof height, got {}",
        pax_obj.status.rappel_dest_y
    );
    assert!(pax_obj.get_position().y > 50.0);
    assert!((pax_obj.movement.max_speed - 30.0).abs() < 0.01);
    assert_ne!(
        pax_obj.movement.target_position.map(|p| p.y),
        Some(0.0),
        "must not path to Y=0"
    );
}

#[test]
fn combat_drop_rappel_add_to_contain_on_roof() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_T4");
    t.add_kind_of(KindOf::Aircraft);
    t.set_health(200.0);
    logic.templates.insert("CD_T4".to_string(), t);
    let mut p = ThingTemplate::new("CD_P4");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("CD_P4".to_string(), p);
    logic
        .templates
        .insert("CD_B4".to_string(), bunker_template("CD_B4", 20.0, 5));
    let transport = logic
        .create_object("CD_T4", Team::USA, Vec3::new(10.0, 80.0, 10.0))
        .unwrap();
    let bunker = logic
        .create_object("CD_B4", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .unwrap();
    let pax = logic
        .create_object("CD_P4", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.install_combat_chinook_transport();
        let _ = t.add_occupant(pax);
        t.set_order_target(Some(bunker));
        if let Some(ai) = t.chinook_ai.as_mut() {
            ai.command_combat_drop([10.0, 10.0, 0.0], Some(20.0));
            ai.arrive_for_combat_drop();
        }
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
    }
    assert!(logic.evacuate_container_now(transport, false));
    for _ in 0..120 {
        logic.tick_rappel_into(pax);
        if logic.host_object(pax).is_some_and(|o| !o.is_rappelling()) {
            break;
        }
    }
    let pax_obj = logic.host_object(pax).unwrap();
    assert!(!pax_obj.is_rappelling());
    assert_eq!(pax_obj.contained_by, Some(bunker));
    assert_eq!(pax_obj.ai_state, AIState::Garrisoned);
    assert!(
        logic
            .host_object(bunker)
            .unwrap()
            .contained_units()
            .contains(&pax)
    );
}

#[test]
fn combat_drop_rappel_kills_two_and_dies() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_T5");
    t.add_kind_of(KindOf::Aircraft);
    t.set_health(200.0);
    logic.templates.insert("CD_T5".to_string(), t);
    let mut p = ThingTemplate::new("CD_P5");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("CD_P5".to_string(), p);
    let mut e = ThingTemplate::new("CD_E5");
    e.add_kind_of(KindOf::Infantry);
    e.set_health(100.0);
    logic.templates.insert("CD_E5".to_string(), e);
    logic
        .templates
        .insert("CD_B5".to_string(), bunker_template("CD_B5", 16.0, 5));
    let transport = logic
        .create_object("CD_T5", Team::USA, Vec3::new(10.0, 60.0, 10.0))
        .unwrap();
    let bunker = logic
        .create_object("CD_B5", Team::China, Vec3::new(10.0, 0.0, 10.0))
        .unwrap();
    let pax = logic
        .create_object("CD_P5", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let e1 = logic
        .create_object("CD_E5", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    let e2 = logic
        .create_object("CD_E5", Team::China, Vec3::new(3.0, 0.0, 0.0))
        .unwrap();
    {
        let b = logic.host_object_mut(bunker).unwrap();
        assert!(b.add_occupant(e1));
        assert!(b.add_occupant(e2));
    }
    {
        let o = logic.host_object_mut(e1).unwrap();
        o.set_contained_by(Some(bunker));
    }
    {
        let o = logic.host_object_mut(e2).unwrap();
        o.set_contained_by(Some(bunker));
    }
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.install_combat_chinook_transport();
        let _ = t.add_occupant(pax);
        t.set_order_target(Some(bunker));
        if let Some(ai) = t.chinook_ai.as_mut() {
            ai.command_combat_drop([10.0, 10.0, 0.0], Some(16.0));
            ai.arrive_for_combat_drop();
        }
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
    }
    assert!(logic.evacuate_container_now(transport, false));
    for _ in 0..120 {
        logic.tick_rappel_into(pax);
        if logic
            .host_object(pax)
            .is_some_and(|o| !o.is_alive() || !o.is_rappelling())
        {
            break;
        }
    }
    assert!(
        !logic.host_object(e1).unwrap().is_alive(),
        "first occupant must die"
    );
    assert!(
        !logic.host_object(e2).unwrap().is_alive(),
        "second occupant must die"
    );
    assert!(
        !logic.host_object(pax).unwrap().is_alive(),
        "rappeller dies after killing two"
    );
    assert!(
        !logic
            .host_object(bunker)
            .unwrap()
            .contained_units()
            .contains(&pax)
    );
}

#[test]
fn execute_exit_staggers_humvee_occupants_and_goes_aggressive() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("EXIT_HV");
    t.add_kind_of(KindOf::Vehicle);
    t.add_kind_of(KindOf::Selectable);
    t.set_health(200.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("EXIT_HV".to_string(), t);
    for name in ["EXIT_HA", "EXIT_HB"] {
        let mut p = ThingTemplate::new(name);
        p.add_kind_of(KindOf::Infantry);
        p.add_kind_of(KindOf::Selectable);
        p.set_health(100.0);
        logic.templates.insert(name.to_string(), p);
    }
    let transport = logic
        .create_object("EXIT_HV", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let h = logic.host_object_mut(transport).unwrap();
        h.install_humvee_transport();
    }
    let a = logic
        .create_object("EXIT_HA", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("EXIT_HB", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    contain_pair(&mut logic, transport, a);
    contain_pair(&mut logic, transport, b);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[transport]), CommandResult::Success);
    }
    let remaining = logic.host_object(transport).unwrap().contained_units();
    assert_eq!(remaining.len(), 1, "ExitDelay must dump one occupant");
    assert!(
        logic
            .host_object(transport)
            .unwrap()
            .pending_evacuate_on_stop
    );
    assert!(logic.host_object(transport).unwrap().frame_exit_not_busy > 0);
    let out = if remaining[0] == a { b } else { a };
    assert!(logic.host_object(out).unwrap().contained_by.is_none());
    assert_eq!(
        logic.host_object(out).unwrap().ai_attitude(),
        crate::game_logic::host_strategy_center::HostAiAttitude::Aggressive
    );
}

#[test]
fn execute_exit_holds_battle_bus_in_air() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("EXIT_BB");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(8),
        ..Default::default()
    };
    logic.templates.insert("EXIT_BB".to_string(), t);
    let mut p = ThingTemplate::new("EXIT_BP");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("EXIT_BP".to_string(), p);
    let transport = logic
        .create_object("EXIT_BB", Team::USA, Vec3::new(0.0, 12.0, 0.0))
        .unwrap();
    {
        let h = logic.host_object_mut(transport).unwrap();
        h.install_battle_bus_transport();
        h.status.airborne_target = true;
    }
    let pax = logic
        .create_object("EXIT_BP", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    contain_pair(&mut logic, transport, pax);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[transport]), CommandResult::Success);
    }
    assert_eq!(
        logic.host_object(pax).unwrap().contained_by,
        Some(transport),
        "DelayExitInAir must hold the door until the bus lands"
    );
    assert!(
        logic
            .host_object(transport)
            .unwrap()
            .pending_evacuate_on_stop
    );
}

#[test]
fn execute_exit_keeps_dumping_while_humvee_moves() {
    // C++ AIExitState::update has no stop/motion requirement.
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("EXIT_HV_MOVE");
    t.add_kind_of(KindOf::Vehicle);
    t.add_kind_of(KindOf::Selectable);
    t.set_health(200.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("EXIT_HV_MOVE".to_string(), t);
    for name in ["EXIT_HMA", "EXIT_HMB"] {
        let mut p = ThingTemplate::new(name);
        p.add_kind_of(KindOf::Infantry);
        p.add_kind_of(KindOf::Selectable);
        p.set_health(100.0);
        logic.templates.insert(name.to_string(), p);
    }
    let transport = logic
        .create_object("EXIT_HV_MOVE", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let h = logic.host_object_mut(transport).unwrap();
        h.install_humvee_transport();
    }
    let a = logic
        .create_object("EXIT_HMA", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("EXIT_HMB", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    contain_pair(&mut logic, transport, a);
    contain_pair(&mut logic, transport, b);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[transport]), CommandResult::Success);
    }
    assert_eq!(
        logic
            .host_object(transport)
            .unwrap()
            .contained_units()
            .len(),
        1,
        "ExitDelay dumps one occupant immediately"
    );
    assert!(
        logic.host_object(transport).unwrap().pending_stream_exit,
        "remaining riders must be marked to stream without a hull stop"
    );

    {
        let t = logic.host_object_mut(transport).unwrap();
        t.status.moving = true;
        t.movement.path = vec![Vec3::new(400.0, 0.0, 0.0)];
        t.movement.current_path_index = 0;
        t.movement.target_position = Some(Vec3::new(400.0, 0.0, 0.0));
    }
    let delay = crate::game_logic::host_humvee::HUMVEE_EXIT_DELAY_FRAMES;
    logic.set_current_frame(u64::from(logic.frame.saturating_add(delay)));
    logic.update_movement_for_test(&[transport], 1.0 / 30.0);
    let hull = logic.host_object(transport).unwrap();
    assert!(
        hull.contained_units().is_empty(),
        "remaining riders must stream out while the hull is still driving"
    );
    assert!(
        hull.status.moving || !hull.movement.path.is_empty(),
        "hull must still be mid-move after the stagger dump"
    );
}

#[test]
fn execute_exit_and_evacuate_ignored_while_subdued() {
    // C++ privateExit / privateEvacuate return when DISABLED_SUBDUED.
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("EXIT_HV_SUB");
    t.add_kind_of(KindOf::Vehicle);
    t.add_kind_of(KindOf::Selectable);
    t.set_health(200.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("EXIT_HV_SUB".to_string(), t);
    let mut p = ThingTemplate::new("EXIT_HSP");
    p.add_kind_of(KindOf::Infantry);
    p.add_kind_of(KindOf::Selectable);
    p.set_health(100.0);
    logic.templates.insert("EXIT_HSP".to_string(), p);
    let transport = logic
        .create_object("EXIT_HV_SUB", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let h = logic.host_object_mut(transport).unwrap();
        h.install_humvee_transport();
    }
    let pax = logic
        .create_object("EXIT_HSP", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    contain_pair(&mut logic, transport, pax);
    {
        let h = logic.host_object_mut(transport).unwrap();
        h.set_disabled_subdued(true);
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_exit(&[transport]),
            CommandResult::InvalidCommand
        );
        assert_eq!(
            exec.execute_evacuate(&[transport]),
            CommandResult::InvalidCommand
        );
    }
    assert_eq!(
        logic.host_object(pax).unwrap().contained_by,
        Some(transport),
        "Microwave-subdued transport must keep its doors shut"
    );
    assert!(!logic.evacuate_container_now(transport, false));
    assert_eq!(
        logic.host_object(pax).unwrap().contained_by,
        Some(transport)
    );
}

#[test]
fn execute_exit_walks_exit_path_not_idle_ring() {
    // C++ OpenContain::exitObjectViaDoor places at ExitStart and
    // aiFollowPath to ExitEnd. Live must not Idle-teleport a 6-unit ring.
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("WALK_T");
    t.add_kind_of(KindOf::Vehicle);
    t.add_kind_of(KindOf::Selectable);
    t.set_health(200.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("WALK_T".to_string(), t);
    let mut p = ThingTemplate::new("WALK_P");
    p.add_kind_of(KindOf::Infantry);
    p.add_kind_of(KindOf::Selectable);
    p.set_health(100.0);
    logic.templates.insert("WALK_P".to_string(), p);
    let transport = logic
        .create_object("WALK_T", Team::USA, Vec3::new(10.0, 0.0, 4.0))
        .unwrap();
    let pax = logic
        .create_object("WALK_P", Team::USA, Vec3::new(11.0, 0.0, 4.0))
        .unwrap();
    contain_pair(&mut logic, transport, pax);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[transport]), CommandResult::Success);
    }
    let hull = logic.host_object(transport).unwrap().get_position();
    let rider = logic.host_object(pax).unwrap();
    assert!(rider.contained_by.is_none());
    assert_eq!(rider.ai_state, AIState::Moving, "must walk ExitStart/End");
    assert!(rider.status.moving);
    let pos = rider.get_position();
    assert!(
        (pos - hull).length() < 1.0,
        "start at ExitStart/hull, not a 6-unit ring: pos={pos:?} hull={hull:?}"
    );
    let dest = rider.movement.target_position.expect("exit dest");
    assert!(
        (dest - hull).length() > 8.0,
        "dest must be ExitEnd/forward, not a 6-unit ring: dest={dest:?}"
    );
}

#[test]
fn evacuate_container_now_walks_transport_not_idle_ring() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("WALK_EV_T");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("WALK_EV_T".to_string(), t);
    let mut p = ThingTemplate::new("WALK_EV_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("WALK_EV_P".to_string(), p);
    let transport = logic
        .create_object("WALK_EV_T", Team::USA, Vec3::new(3.0, 0.0, 8.0))
        .unwrap();
    let pax = logic
        .create_object("WALK_EV_P", Team::USA, Vec3::new(4.0, 0.0, 8.0))
        .unwrap();
    contain_pair(&mut logic, transport, pax);
    assert!(logic.evacuate_container_now(transport, false));
    let hull = logic.host_object(transport).unwrap().get_position();
    let rider = logic.host_object(pax).unwrap();
    assert!(rider.contained_by.is_none());
    assert_eq!(rider.ai_state, AIState::Moving);
    assert!(rider.status.moving);
    assert!((rider.get_position() - hull).length() < 1.0);
    let dest = rider.movement.target_position.expect("exit dest");
    assert!((dest - hull).length() > 8.0);
}

#[test]
fn execute_exit_airborne_allows_fall_without_hull_velocity_kick() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("WALK_AIR_T");
    t.add_kind_of(KindOf::Aircraft);
    t.set_health(200.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(8),
        ..Default::default()
    };
    logic.templates.insert("WALK_AIR_T".to_string(), t);
    let mut p = ThingTemplate::new("WALK_AIR_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("WALK_AIR_P".to_string(), p);
    let transport = logic
        .create_object("WALK_AIR_T", Team::USA, Vec3::new(0.0, 20.0, 0.0))
        .unwrap();
    {
        let h = logic.host_object_mut(transport).unwrap();
        h.status.airborne_target = true;
        h.movement.velocity = Vec3::new(12.0, 0.0, 0.0);
    }
    let pax = logic
        .create_object("WALK_AIR_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    contain_pair(&mut logic, transport, pax);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[transport]), CommandResult::Success);
    }
    let rider = logic.host_object(pax).unwrap();
    assert!(rider.contained_by.is_none());
    assert_eq!(rider.ai_state, AIState::Moving);
    assert!(
        rider.allow_to_fall,
        "C++ onRemoving setAllowToFall when hull is above terrain"
    );
    assert_eq!(
        rider.motive_frames_remaining, 0,
        "hq-qhzox: default KeepContainerVelocityOnExit is false; airborne must not invent a hull kick"
    );
    assert_eq!(
        rider.physics_accel,
        Vec3::ZERO,
        "hq-qhzox: rider must not inherit Chinook/Helix cruise velocity"
    );
}

#[test]
fn execute_exit_walks_after_occupant_list_cleared() {
    // execute_exit remove_occupant then walks by container id.
    // Must not require contained_by to still be set, and must not Idle-ring.
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("WALK_CLR_T");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("WALK_CLR_T".to_string(), t);
    let mut p = ThingTemplate::new("WALK_CLR_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("WALK_CLR_P".to_string(), p);
    let transport = logic
        .create_object("WALK_CLR_T", Team::USA, Vec3::new(6.0, 0.0, 2.0))
        .unwrap();
    let pax = logic
        .create_object("WALK_CLR_P", Team::USA, Vec3::new(7.0, 0.0, 2.0))
        .unwrap();
    contain_pair(&mut logic, transport, pax);
    let _ = logic.unit_command_remove_occupant(transport, pax);
    {
        let rider = logic.host_object_mut(pax).unwrap();
        rider.set_contained_by(None);
    }
    assert!(logic.unit_command_exit_via_open_contain(pax, transport));
    let hull = logic.host_object(transport).unwrap().get_position();
    let rider = logic.host_object(pax).unwrap();
    assert_eq!(rider.ai_state, AIState::Moving);
    assert!(rider.status.moving);
    assert!((rider.get_position() - hull).length() < 1.0);
    let dest = rider.movement.target_position.expect("exit dest");
    assert!((dest - hull).length() > 8.0);
}

#[test]
fn execute_exit_tunnel_walks_exit_start_not_idle_drop() {
    // C++ TunnelContain does not override exitObjectViaDoor.
    let mut logic = GameLogic::new();
    let mut tn = ThingTemplate::new("GLATunnelNetwork");
    tn.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    tn.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Tunnel,
        slots: Some(10),
        ..Default::default()
    };
    logic.templates.insert("GLATunnelNetwork".into(), tn);
    let mut rebel = ThingTemplate::new("GLARebel");
    rebel.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("GLARebel".into(), rebel);

    let tunnel = logic
        .create_object("GLATunnelNetwork", Team::GLA, Vec3::new(12.0, 0.0, 4.0))
        .unwrap();
    let pax = logic
        .create_object("GLARebel", Team::GLA, Vec3::new(13.0, 0.0, 4.0))
        .unwrap();
    if let Some(t) = logic.host_object_mut(tunnel) {
        t.set_status_under_construction(false);
        t.construction_percent = 1.0;
        let _ = t.add_occupant(pax);
    }
    if let Some(p) = logic.host_object_mut(pax) {
        p.set_contained_by(Some(tunnel));
        p.set_ai_state(AIState::Garrisoned);
    }
    let key = logic.host_object(tunnel).unwrap().tunnel_system_key();
    assert!(logic.tunnel_network.record_enter(key, pax, tunnel));

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[pax]), CommandResult::Success);
    }
    let hull = logic.host_object(tunnel).unwrap().get_position();
    let rider = logic.host_object(pax).unwrap();
    assert!(rider.contained_by.is_none());
    assert_eq!(
        rider.ai_state,
        AIState::Moving,
        "tunnel inventory exit must walk ExitStart/End"
    );
    assert!(rider.status.moving);
    assert!((rider.get_position() - hull).length() < 1.0);
    let dest = rider.movement.target_position.expect("exit dest");
    assert!(
        (dest - hull).length() > 8.0,
        "must path toward ExitEnd, not Idle at the door"
    );
}

#[test]
fn combat_drop_into_rejects_faction_structure() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_T6");
    t.add_kind_of(KindOf::Aircraft);
    t.set_health(200.0);
    logic.templates.insert("CD_T6".into(), t);
    let mut p = ThingTemplate::new("CD_P6");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("CD_P6".into(), p);
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::CommandCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(2000.0);
    cc.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(5),
        admission: ContainAdmission::InfantryOnly,
        ..Default::default()
    };
    logic.templates.insert("AmericaCommandCenter".into(), cc);
    let transport = logic
        .create_object("CD_T6", Team::USA, Vec3::new(0.0, 100.0, 0.0))
        .unwrap();
    let pax = logic
        .create_object("CD_P6", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let bldg = logic
        .create_object(
            "AmericaCommandCenter",
            Team::China,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.install_combat_chinook_transport();
        let _ = t.add_occupant(pax);
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
    }
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_combat_drop(&[transport], &DropTarget::Object(bldg)),
        CommandResult::InvalidCommand
    );
}

#[test]
fn combat_drop_hover_uses_geometry_height_plus_min_drop() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_T7");
    t.add_kind_of(KindOf::Aircraft);
    t.set_health(200.0);
    logic.templates.insert("CD_T7".into(), t);
    let mut p = ThingTemplate::new("CD_P7");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("CD_P7".into(), p);
    logic
        .templates
        .insert("CD_B7".into(), bunker_template("CD_B7", 80.0, 5));
    let transport = logic
        .create_object("CD_T7", Team::USA, Vec3::new(0.0, 100.0, 0.0))
        .unwrap();
    let pax = logic
        .create_object("CD_P7", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    let bunker = logic
        .create_object("CD_B7", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.install_combat_chinook_transport();
        let _ = t.add_occupant(pax);
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_combat_drop(&[transport], &DropTarget::Object(bunker)),
            CommandResult::Success
        );
    }
    let ai = logic
        .host_object(transport)
        .unwrap()
        .chinook_ai
        .as_ref()
        .unwrap();
    assert!(
        (ai.combat_drop_dest_z - 120.0).abs() < 0.01,
        "hover must be geom 80 + MinDropHeight 40, got {}",
        ai.combat_drop_dest_z
    );
}

#[test]
fn regular_chinook_create_installs_transport() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("AmericaVehicleChinook");
    t.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(200.0);
    logic.templates.insert("AmericaVehicleChinook".into(), t);
    let id = logic
        .create_object(
            "AmericaVehicleChinook",
            Team::USA,
            Vec3::new(0.0, 100.0, 0.0),
        )
        .unwrap();
    let obj = logic.host_object(id).unwrap();
    assert_eq!(obj.max_transport, 8);
    assert!(obj.can_contain());
    assert!(obj.chinook_ai.is_some());
    assert!(!obj.is_combat_chinook_style_container());
    assert!(!obj.chinook_ai.as_ref().unwrap().can_issue_attack());
    assert!(!obj.passengers_allowed_to_fire);
}

#[test]
fn last_rappeller_finish_leaves_do_combat_drop() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_T8");
    t.add_kind_of(KindOf::Aircraft);
    t.set_health(200.0);
    logic.templates.insert("CD_T8".into(), t);
    let mut p = ThingTemplate::new("CD_P8");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("CD_P8".into(), p);
    let transport = logic
        .create_object("CD_T8", Team::USA, Vec3::new(10.0, 100.0, 10.0))
        .unwrap();
    let pax = logic
        .create_object("CD_P8", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.install_combat_chinook_transport();
        let _ = t.add_occupant(pax);
        if let Some(ai) = t.chinook_ai.as_mut() {
            ai.command_combat_drop([10.0, 10.0, 0.0], None);
            ai.arrive_for_combat_drop();
        }
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
    }
    assert!(logic.evacuate_container_now(transport, false));
    assert_eq!(
        logic
            .host_object(transport)
            .unwrap()
            .chinook_ai
            .as_ref()
            .unwrap()
            .flight_status,
        crate::game_logic::host_combat_chinook::HostChinookFlightStatus::DoingCombatDrop
    );
    assert!(!logic.evacuate_container_now(transport, false));
    assert_eq!(
        logic
            .host_object(transport)
            .unwrap()
            .chinook_ai
            .as_ref()
            .unwrap()
            .flight_status,
        crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Flying
    );
}

fn garrison_bunker(
    logic: &mut GameLogic,
    bunker_name: &str,
    ranger_name: &str,
    origin: Vec3,
    evac: Option<u8>,
) -> (ObjectId, ObjectId) {
    let mut bunker_t = ThingTemplate::new(bunker_name);
    bunker_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    bunker_t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(5),
        admission: ContainAdmission::InfantryOnly,
        is_enclosing_container: true,
        ..Default::default()
    };
    bunker_t.geometry_info.authored = true;
    bunker_t.geometry_info.major_radius = 20.0;
    bunker_t.geometry_info.minor_radius = 10.0;
    logic.templates.insert(bunker_name.to_string(), bunker_t);
    let mut ranger_t = ThingTemplate::new(ranger_name);
    ranger_t
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0);
    ranger_t.transport_slot_count = Some(1);
    logic.templates.insert(ranger_name.to_string(), ranger_t);
    let bunker = logic.create_object(bunker_name, Team::USA, origin).unwrap();
    if let Some(disp) = evac {
        logic
            .host_object_mut(bunker)
            .unwrap()
            .set_garrison_evac_disposition(disp);
    }
    let ranger = logic
        .create_object(ranger_name, Team::USA, origin + Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(bunker).unwrap();
        assert!(t.add_occupant(ranger));
    }
    {
        let p = logic.host_object_mut(ranger).unwrap();
        p.set_contained_by(Some(bunker));
        p.set_ai_state(AIState::Garrisoned);
    }
    (bunker, ranger)
}

#[test]
fn execute_exit_garrison_walks_burst_not_six_unit_ring() {
    let mut logic = GameLogic::new();
    let origin = Vec3::new(10.0, 0.0, 20.0);
    let (bunker, ranger) = garrison_bunker(&mut logic, "EXIT_GB", "EXIT_GR", origin, None);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[bunker]), CommandResult::Success);
    }
    let r = logic.host_object(ranger).unwrap();
    assert!(r.contained_by.is_none());
    assert!(
        matches!(r.ai_state, AIState::Moving),
        "Evacuate/Exit on a garrison must walk, not Idle on a 6-unit ring"
    );
    assert!(r.status.moving);
    let dest = r.movement.target_position.expect("burst dest");
    assert!(
        (dest - origin).length() > 8.0,
        "burst dest must leave the building, not a 6-unit ring: dest={dest:?}"
    );
    let drop_ring = (r.get_position() - origin).length();
    assert!(
        drop_ring < 2.0,
        "enclosing burst snaps to building origin, not a 6-unit teleport: pos={:?}",
        r.get_position()
    );
}

#[test]
fn execute_evacuate_garrison_walks_burst_not_six_unit_ring() {
    let mut logic = GameLogic::new();
    let origin = Vec3::new(4.0, 0.0, 8.0);
    let (bunker, ranger) = garrison_bunker(&mut logic, "EVAC_GB", "EVAC_GR", origin, None);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_evacuate(&[bunker]), CommandResult::Success);
    }
    let r = logic.host_object(ranger).unwrap();
    assert!(r.contained_by.is_none());
    assert_eq!(r.ai_state, AIState::Moving);
    let dest = r.movement.target_position.expect("evac dest");
    assert!(
        (dest - origin).length() > 8.0,
        "Evacuate button must burst-walk, not a 6-unit ring: dest={dest:?}"
    );
    let delta = r.get_position() - origin;
    let horizontal_from_origin = (delta.x * delta.x + delta.z * delta.z).sqrt();
    assert!(
        horizontal_from_origin < 2.0 || (horizontal_from_origin - 20.0).abs() < 2.0,
        "enclosing burst must start at the center or a C++ cliff-bunker front/back candidate, never a six-unit ring: pos={:?}",
        r.get_position()
    );
}

#[test]
fn execute_evacuate_marks_stealth_garrison_detected() {
    let mut logic = GameLogic::new();
    let origin = Vec3::new(6.0, 0.0, 4.0);
    let mut bunker_t = ThingTemplate::new("EVAC_SG_B");
    bunker_t
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0);
    bunker_t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Garrison,
        slots: Some(5),
        admission: ContainAdmission::InfantryOnly,
        is_enclosing_container: true,
        ..Default::default()
    };
    logic.templates.insert("EVAC_SG_B".into(), bunker_t);
    let mut ninja_t = ThingTemplate::new("EVAC_SG_N");
    ninja_t
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::StealthGarrison)
        .set_health(120.0);
    ninja_t.transport_slot_count = Some(1);
    logic.templates.insert("EVAC_SG_N".into(), ninja_t);
    let bunker = logic.create_object("EVAC_SG_B", Team::USA, origin).unwrap();
    let ninja = logic
        .create_object("EVAC_SG_N", Team::USA, origin + Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(bunker).unwrap();
        assert!(t.add_occupant(ninja));
    }
    {
        let n = logic.host_object_mut(ninja).unwrap();
        n.set_contained_by(Some(bunker));
        n.set_ai_state(AIState::Garrisoned);
        n.set_status_stealthed(true);
        n.set_status_detected(false);
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_evacuate(&[bunker]), CommandResult::Success);
    }
    let n = logic.host_object(ninja).unwrap();
    assert!(n.contained_by.is_none());
    assert!(
        n.status.detected,
        "C++ markAllPassengersDetected must destalth STEALTH_GARRISON on Evac"
    );
}

#[test]
fn execute_exit_garrison_occupant_inventory_walks_burst() {
    // C++ MSG_EXIT / Command_StructureExit: occupant argument, not the bunker.
    let mut logic = GameLogic::new();
    let origin = Vec3::new(30.0, 0.0, 12.0);
    let (_bunker, ranger) = garrison_bunker(&mut logic, "EXIT_GO", "EXIT_GOR", origin, None);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[ranger]), CommandResult::Success);
    }
    let r = logic.host_object(ranger).unwrap();
    assert!(r.contained_by.is_none());
    assert_eq!(r.ai_state, AIState::Moving);
    let dest = r.movement.target_position.expect("inventory dest");
    assert!(
        (dest - origin).length() > 8.0,
        "inventory Exit must burst-walk: dest={dest:?}"
    );
}

#[test]
fn execute_exit_garrison_respects_evac_left() {
    let mut logic = GameLogic::new();
    let (bunker, ranger) = garrison_bunker(&mut logic, "EXIT_GL", "EXIT_GLR", Vec3::ZERO, Some(1));
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[bunker]), CommandResult::Success);
    }
    let r = logic.host_object(ranger).unwrap();
    assert!(matches!(r.ai_state, AIState::Moving));
    let dest = r.movement.target_position.expect("left dest");
    assert!(
        dest.z.abs() >= 50.0,
        "EVAC_TO_LEFT must spread along the side (minor*10), not a 6-unit ring: dest={dest:?}"
    );
    assert!(
        dest.z > 0.0,
        "EVAC_TO_LEFT walk-to is +minor*10 in local Y: dest={dest:?}"
    );
}

#[test]
fn execute_exit_garrison_respects_evac_right() {
    let mut logic = GameLogic::new();
    let (bunker, ranger) =
        garrison_bunker(&mut logic, "EXIT_GRT", "EXIT_GRTR", Vec3::ZERO, Some(2));
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[bunker]), CommandResult::Success);
    }
    let r = logic.host_object(ranger).unwrap();
    assert!(matches!(r.ai_state, AIState::Moving));
    let dest = r.movement.target_position.expect("right dest");
    assert!(
        dest.z.abs() >= 50.0,
        "EVAC_TO_RIGHT must spread along the side (minor*10): dest={dest:?}"
    );
    assert!(
        dest.z < 0.0,
        "EVAC_TO_RIGHT walk-to is -minor*10 in local Y: dest={dest:?}"
    );
}

#[test]
fn execute_exit_garrison_walks_without_garrisoned_ai_state() {
    // Contain-module classification, not only AIState::Garrisoned.
    let mut logic = GameLogic::new();
    let origin = Vec3::new(1.0, 0.0, 1.0);
    let (bunker, ranger) = garrison_bunker(&mut logic, "EXIT_GI", "EXIT_GIR", origin, None);
    logic
        .host_object_mut(ranger)
        .unwrap()
        .set_ai_state(AIState::Idle);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[bunker]), CommandResult::Success);
    }
    let r = logic.host_object(ranger).unwrap();
    assert_eq!(r.ai_state, AIState::Moving);
    let dest = r.movement.target_position.expect("idle-ai dest");
    assert!(
        (dest - origin).length() > 8.0,
        "is_garrison_contain must still burst-walk: dest={dest:?}"
    );
}

#[test]
fn open_contain_exit_ignores_collisions_for_one_second() {
    let mut logic = GameLogic::new();
    logic.frame = 12;
    let mut t = ThingTemplate::new("IGN_T");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(200.0);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::Transport,
        slots: Some(5),
        ..Default::default()
    };
    logic.templates.insert("IGN_T".to_string(), t);
    let mut p = ThingTemplate::new("IGN_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("IGN_P".to_string(), p);
    let transport = logic
        .create_object("IGN_T", Team::USA, Vec3::new(6.0, 0.0, 2.0))
        .unwrap();
    let pax = logic
        .create_object("IGN_P", Team::USA, Vec3::new(7.0, 0.0, 2.0))
        .unwrap();
    contain_pair(&mut logic, transport, pax);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_exit(&[pax]), CommandResult::Success);
    }
    let rider = logic.host_object(pax).unwrap();
    assert_eq!(rider.ignore_collisions_with, None);
    assert_eq!(rider.ignore_collisions_until_frame, 42);
}

#[test]
fn enter_from_container_does_not_yank_occupant() {
    let mut logic = GameLogic::new();
    let origin_a = Vec3::new(0.0, 0.0, 0.0);
    let origin_b = Vec3::new(80.0, 0.0, 0.0);
    let (bunker_a, ranger) = garrison_bunker(&mut logic, "YANK_A", "YANK_R", origin_a, None);
    let (bunker_b, _) = garrison_bunker(&mut logic, "YANK_B", "YANK_R2", origin_b, None);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        let _ = exec.execute_enter(&[ranger], bunker_b);
    }
    let rider = logic.host_object(ranger).unwrap();
    assert_eq!(rider.contained_by, Some(bunker_a));
    assert!(
        logic
            .host_object(bunker_a)
            .unwrap()
            .contained_units()
            .contains(&ranger)
    );
    assert!(
        !logic
            .host_object(bunker_b)
            .unwrap()
            .contained_units()
            .contains(&ranger)
    );
    assert_ne!(rider.ai_state, AIState::Entering);
}

#[test]
fn railed_evacuate_uses_dock_unload_not_open_contain_walk() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("RAIL_F");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(400.0);
    t.dock_kind = crate::game_logic::DockKind::RailedTransport;
    t.railed_transport_slots = Some(10);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::RailedTransport,
        slots: Some(10),
        ..Default::default()
    };
    logic.templates.insert("RAIL_F".to_string(), t);
    let mut p = ThingTemplate::new("RAIL_P");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("RAIL_P".to_string(), p);
    let ferry = logic
        .create_object("RAIL_F", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .unwrap();
    let pax = logic
        .create_object("RAIL_P", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    contain_pair(&mut logic, ferry, pax);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_evacuate(&[ferry]), CommandResult::Success);
    }
    let rider = logic.host_object(pax).unwrap();
    let hull = logic.host_object(ferry).unwrap().get_position();
    assert!(rider.contained_by.is_none());
    assert!(rider.is_held_disabled());
    assert!((rider.get_position() - hull).length() < 0.01);
    assert_eq!(rider.ignore_collisions_until_frame, 0);
    assert_eq!(
        logic.host_object(ferry).unwrap().dock_active_docker,
        Some(pax)
    );
}

#[test]
fn railed_evacuate_refuses_while_in_transit() {
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("RAIL_FT");
    t.add_kind_of(KindOf::Vehicle);
    t.set_health(400.0);
    t.dock_kind = crate::game_logic::DockKind::RailedTransport;
    t.railed_transport_slots = Some(10);
    t.contain_module = ContainModuleMetadata {
        kind: ContainModuleKind::RailedTransport,
        slots: Some(10),
        ..Default::default()
    };
    logic.templates.insert("RAIL_FT".to_string(), t);
    let mut p = ThingTemplate::new("RAIL_PT");
    p.add_kind_of(KindOf::Infantry);
    p.set_health(100.0);
    logic.templates.insert("RAIL_PT".to_string(), p);
    let ferry = logic
        .create_object("RAIL_FT", Team::USA, Vec3::new(4.0, 0.0, 0.0))
        .unwrap();
    let pax = logic
        .create_object("RAIL_PT", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    contain_pair(&mut logic, ferry, pax);
    logic.host_object_mut(ferry).unwrap().railed_in_transit = true;
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_evacuate(&[ferry]),
            CommandResult::InvalidCommand
        );
    }
    assert_eq!(logic.host_object(pax).unwrap().contained_by, Some(ferry));
}
