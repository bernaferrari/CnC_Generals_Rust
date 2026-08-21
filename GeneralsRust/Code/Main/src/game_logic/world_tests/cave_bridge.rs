//! Live-path CaveSystem + bridge scaffold/rubble tests.

use super::*;
use crate::game_logic::host_bridge_behavior::BRIDGE_SCAFFOLD_TEMPLATE;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::{ContainModuleKind, KindOf, Team, ThingTemplate};

fn ensure_cave_template(logic: &mut GameLogic, name: &str) {
    if logic.templates.contains_key(name) {
        return;
    }
    let mut t = ThingTemplate::new(name);
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(500.0);
    t.contain_module.kind = ContainModuleKind::Cave;
    logic.templates.insert(name.to_string(), t);
}

fn create_cave(logic: &mut GameLogic, name: &str, pos: Vec3, index: i32) -> ObjectId {
    ensure_cave_template(logic, name);
    let id = logic
        .create_object(name, Team::Neutral, pos)
        .expect("cave");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.install_cave_contain_residual(index);
    }
    logic
        .cave_system
        .register_cave(id, index, Team::Neutral);
    id
}

fn ensure_infantry(logic: &mut GameLogic) {
    if logic.templates.contains_key("AmericaInfantryRanger") {
        return;
    }
    let mut t = ThingTemplate::new("AmericaInfantryRanger");
    t.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert("AmericaInfantryRanger".to_string(), t);
}

#[test]
fn cave_index_shares_inventory_and_set_cave_index() {
    // C++ CaveContain.cpp:43-47 + ScriptActions.cpp:5063 SET_CAVE_INDEX.
    let mut logic = GameLogic::new();
    ensure_infantry(&mut logic);
    let a = create_cave(&mut logic, "CaveA", Vec3::new(0.0, 0.0, 0.0), 0);
    let b = create_cave(&mut logic, "CaveB", Vec3::new(80.0, 0.0, 0.0), 0);
    let unit = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("ranger");
    assert!(logic.cave_system.record_enter(0, unit, a, Team::USA).0);
    assert!(logic.cave_system.is_in_network(0, unit));
    assert!(!logic.try_set_cave_index(b, 2));
    let _ = logic.exit_cave_unit(unit, b);
    assert!(logic.try_set_cave_index(b, 2));
    assert_eq!(logic.host_object(b).unwrap().cave_index, 2);
}

#[test]
fn first_occupant_captures_network_ranger_capture_does_not_kick() {
    // C++ CaveContain.cpp:254-336 / CaveContain.h:83 isKickOutOnCapture=false.
    let mut logic = GameLogic::new();
    ensure_infantry(&mut logic);
    let a = create_cave(&mut logic, "CaveCapA", Vec3::ZERO, 4);
    let b = create_cave(&mut logic, "CaveCapB", Vec3::new(40.0, 0.0, 0.0), 4);
    let unit = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("ranger");
    let (ok, ev) = logic.cave_system.record_enter(4, unit, a, Team::USA);
    assert!(ok);
    logic.apply_cave_capture_event(4, ev);
    assert_eq!(logic.host_object(a).unwrap().team, Team::USA);
    assert_eq!(logic.host_object(b).unwrap().team, Team::USA);
    logic.on_capture_kick_passengers(a, Team::USA, Team::China);
    assert!(logic.cave_system.is_in_network(4, unit));
}

#[test]
fn last_cave_destroy_caves_in_instead_of_eject() {
    // C++ CaveContain.cpp:197-211 + TunnelTracker.cpp:187-220.
    let mut logic = GameLogic::new();
    ensure_infantry(&mut logic);
    let a = create_cave(&mut logic, "CaveDieA", Vec3::ZERO, 1);
    let unit = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("ranger");
    assert!(logic.cave_system.record_enter(1, unit, a, Team::USA).0);
    if let Some(u) = logic.host_object_mut(unit) {
        u.set_contained_by(Some(a));
    }
    if let Some(c) = logic.host_object_mut(a) {
        let _ = c.add_occupant(unit);
        c.health.current = 0.0;
        c.status.destroyed = true;
    }
    logic.mark_object_for_destruction(a, None);
    logic.process_destroy_list();
    assert!(logic.cave_system.honesty_cave_in_ok());
    let u = logic.host_object(unit);
    assert!(u.is_none() || u.is_some_and(|o| o.status.destroyed || o.health.current <= 0.0));
}

#[test]
fn ini_cave_index_registers_on_create() {
    // C++ CaveContain::onCreate copies CaveIndex; onBuildComplete registers.
    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("IniCave");
    t.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(500.0);
    t.contain_module.kind = ContainModuleKind::Cave;
    t.contain_module.cave_index = 5;
    logic.templates.insert("IniCave".to_string(), t);
    let id = logic
        .create_object("IniCave", Team::Neutral, Vec3::ZERO)
        .expect("ini cave");
    assert_eq!(logic.host_object(id).unwrap().cave_index, 5);
    assert_eq!(logic.cave_system.index_of(id), Some(5));
}

#[test]
fn leftover_set_cave_index_drains_onto_live_host() {
    // C++ ScriptActions.cpp:5063 SET_CAVE_INDEX on empty leftover registry.
    let mut logic = GameLogic::new();
    let a = create_cave(&mut logic, "CaveScriptA", Vec3::ZERO, 0);
    let b = create_cave(&mut logic, "CaveScriptB", Vec3::new(40.0, 0.0, 0.0), 0);
    assert_eq!(logic.cave_system.index_of(a), Some(0));
    assert_eq!(logic.cave_system.index_of(b), Some(0));

    use gamelogic::scripting::core::{
        Parameter, ParameterType, ScriptAction, ScriptActionType,
    };
    use gamelogic::scripting::executor::{
        ScriptActionDispatcher, ScriptActionResult, ScriptContext,
    };
    use std::sync::{Arc, RwLock};

    let _ = gamelogic::scripting::take_host_set_cave_index_requests();
    let mut action = ScriptAction::new(ScriptActionType::SetCaveIndex);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "CaveScriptB".into(),
        ))
        .expect("cave name");
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .expect("cave index");
    let mut dispatcher =
        ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        dispatcher.execute_action(&action).expect("SET_CAVE_INDEX"),
        ScriptActionResult::Success
    );
    logic.apply_host_set_cave_index_requests();
    assert_eq!(logic.host_object(b).unwrap().cave_index, 2);
    assert_eq!(logic.cave_system.index_of(b), Some(2));
    assert_eq!(logic.cave_system.index_of(a), Some(0));
}

#[test]
fn dozer_bridge_repair_spawns_scaffold() {
    // C++ DozerAIUpdate.cpp:665-688 createBridgeScaffolding.
    let mut logic = GameLogic::new();
    let mut bridge = ThingTemplate::new("TestBridgeSpan");
    bridge
        .add_kind_of(KindOf::Structure)
        .set_health(200.0);
    logic.templates.insert("TestBridgeSpan".into(), bridge);
    let mut scaf = ThingTemplate::new(BRIDGE_SCAFFOLD_TEMPLATE);
    scaf.add_kind_of(KindOf::Structure).set_health(1.0);
    logic
        .templates
        .insert(BRIDGE_SCAFFOLD_TEMPLATE.to_string(), scaf);
    let id = logic
        .create_object("TestBridgeSpan", Team::USA, Vec3::ZERO)
        .expect("bridge");
    if let Some(o) = logic.host_object_mut(id) {
        o.health.current = 1.0;
    }
    logic.bridge_behavior.register_span(
        id,
        Vec3::new(-20.0, 0.0, -20.0),
        Vec3::new(20.0, 0.0, -20.0),
        Vec3::new(-20.0, 0.0, 20.0),
        Vec3::new(20.0, 0.0, 20.0),
    );
    assert!(logic.bridge_behavior.create_scaffolding(id));
    assert!(logic.bridge_behavior.is_scaffold_in_motion(id));
}

#[test]
fn bridge_rubble_restamps_and_splat_kills() {
    // C++ TerrainLogic.cpp Bridge::updateDamageState :852-909.
    let mut logic = GameLogic::new();
    ensure_infantry(&mut logic);
    let mut bridge = ThingTemplate::new("RubbleBridge");
    bridge
        .add_kind_of(KindOf::Structure)
        .set_health(100.0);
    logic.templates.insert("RubbleBridge".into(), bridge);
    let bid = logic
        .create_object("RubbleBridge", Team::USA, Vec3::new(50.0, 0.0, 50.0))
        .expect("bridge");
    let uid = logic
        .create_object(
            "AmericaInfantryRanger",
            Team::USA,
            Vec3::new(50.0, 0.0, 50.0),
        )
        .expect("deck unit");
    if let Some(b) = logic.host_object_mut(bid) {
        b.health.current = 0.0;
    }
    logic.sync_host_bridge_rubble_and_scaffolds();
    let unit = logic.host_object(uid).expect("unit");
    assert!(unit.health.current <= 0.0 || unit.status.destroyed);
    assert_eq!(unit.status.death_type, HostDeathType::Splatted);
    assert!(logic.bridge_behavior.honesty_rubble_ok());
}
