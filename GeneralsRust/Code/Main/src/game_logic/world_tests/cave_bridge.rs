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
    let id = logic.create_object(name, Team::Neutral, pos).expect("cave");
    if let Some(obj) = logic.host_object_mut(id) {
        obj.install_cave_contain_residual(index);
    }
    logic.cave_system.register_cave(id, index, Team::Neutral);
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
fn occupant_death_leaves_cave_system_pool_and_reverts_team() {
    // C++ Object.cpp:720-728 onDestroy → CaveContain::removeFromContain
    // tracker remove then onRemoving LastEmpty.
    let mut logic = GameLogic::new();
    ensure_infantry(&mut logic);
    let a = create_cave(&mut logic, "CaveOccDieA", Vec3::ZERO, 7);
    let b = create_cave(&mut logic, "CaveOccDieB", Vec3::new(40.0, 0.0, 0.0), 7);
    let unit = logic
        .create_object("AmericaInfantryRanger", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("ranger");
    let (ok, ev) = logic.cave_system.record_enter(7, unit, a, Team::USA);
    assert!(ok);
    logic.apply_cave_capture_event(7, ev);
    assert_eq!(logic.host_object(a).unwrap().team, Team::USA);
    assert_eq!(logic.host_object(b).unwrap().team, Team::USA);
    if let Some(u) = logic.host_object_mut(unit) {
        u.set_contained_by(Some(a));
        u.health.current = 0.0;
        u.status.destroyed = true;
    }
    if let Some(c) = logic.host_object_mut(a) {
        let _ = c.add_occupant(unit);
    }
    logic.mark_object_for_destruction(unit, None);
    logic.process_destroy_list();
    assert!(
        !logic.cave_system.is_in_network(7, unit),
        "dying occupant must leave CaveSystem pool"
    );
    assert_eq!(logic.cave_system.contain_count(7), 0);
    assert_eq!(logic.host_object(a).unwrap().team, Team::Neutral);
    assert_eq!(logic.host_object(b).unwrap().team, Team::Neutral);
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

    use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
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
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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
    bridge.add_kind_of(KindOf::Structure).set_health(200.0);
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
    bridge.add_kind_of(KindOf::Structure).set_health(100.0);
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

fn ensure_bridge_span_and_tower(logic: &mut GameLogic) {
    if !logic.templates.contains_key("TestBridgeSpan") {
        let mut span = ThingTemplate::new("TestBridgeSpan");
        span.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Bridge)
            .set_health(200.0);
        logic.templates.insert("TestBridgeSpan".into(), span);
    }
    if !logic.templates.contains_key("TestBridgeTower") {
        let mut tower = ThingTemplate::new("TestBridgeTower");
        tower
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::BridgeTower)
            .set_health(100.0);
        logic.templates.insert("TestBridgeTower".into(), tower);
    }
    if !logic.templates.contains_key(BRIDGE_SCAFFOLD_TEMPLATE) {
        let mut scaf = ThingTemplate::new(BRIDGE_SCAFFOLD_TEMPLATE);
        scaf.add_kind_of(KindOf::Structure).set_health(1.0);
        logic
            .templates
            .insert(BRIDGE_SCAFFOLD_TEMPLATE.to_string(), scaf);
    }
    if !logic.templates.contains_key("TestDozer") {
        let mut dozer = ThingTemplate::new("TestDozer");
        dozer
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Dozer)
            .add_kind_of(KindOf::Worker)
            .set_health(300.0);
        logic.templates.insert("TestDozer".into(), dozer);
    }
}

fn spawn_linked_bridge(logic: &mut GameLogic) -> (ObjectId, ObjectId, ObjectId) {
    ensure_bridge_span_and_tower(logic);
    let span = logic
        .create_object("TestBridgeSpan", Team::USA, Vec3::ZERO)
        .expect("span");
    let t0 = logic
        .create_object("TestBridgeTower", Team::USA, Vec3::new(-20.0, 0.0, 0.0))
        .expect("t0");
    let t1 = logic
        .create_object("TestBridgeTower", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .expect("t1");
    logic.bridge_behavior.register_span(
        span,
        Vec3::new(-20.0, 0.0, -8.0),
        Vec3::new(-20.0, 0.0, 8.0),
        Vec3::new(20.0, 0.0, -8.0),
        Vec3::new(20.0, 0.0, 8.0),
    );
    logic
        .bridge_behavior
        .bind_towers(span, [t0, t1, ObjectId(0), ObjectId(0)]);
    (span, t0, t1)
}

#[test]
fn repair_complete_removes_scaffolding() {
    // C++ WorkerAIUpdate.cpp:830 removeBridgeScaffolding.
    let mut logic = GameLogic::new();
    let (span, _, _) = spawn_linked_bridge(&mut logic);
    logic.spawn_bridge_scaffolding(span);
    assert!(logic.bridge_behavior.is_scaffold_present(span));
    let scaffold_ids = logic
        .bridge_behavior
        .span(span)
        .map(|s| s.scaffold_ids.clone())
        .unwrap_or_default();
    assert!(!scaffold_ids.is_empty());

    // TestDozer is registered by spawn_linked_bridge.
    let dozer = logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 12.0))
        .expect("dozer");
    if let Some(d) = logic.host_object_mut(dozer) {
        d.dozer_task_repair_target = Some(span);
        d.set_target(Some(span));
    }
    logic.dozer_internal_task_complete(dozer, true);
    assert!(
        !logic.bridge_behavior.is_scaffold_present(span),
        "repair-complete must clear scaffold_present"
    );
    for sid in scaffold_ids {
        let gone = logic.host_object(sid).is_none_or(|o| o.status.destroyed);
        assert!(gone, "scaffold object {sid:?} must be destroyed");
    }
}

#[test]
fn rubble_span_is_repairable() {
    // C++ DozerAIUpdate.cpp:649-703 heals rubble bridge/tower.
    let mut logic = GameLogic::new();
    let (span, _, _) = spawn_linked_bridge(&mut logic);
    if let Some(s) = logic.host_object_mut(span) {
        s.convert_bridge_to_rubble_husk();
    }
    assert!(!logic.host_object(span).expect("span").is_alive());

    let dozer = logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 10.0))
        .expect("dozer");
    if let Some(s) = logic.host_object_mut(span) {
        s.revive_from_bridge_rubble();
        assert!(
            s.attempt_healing_from_sole_benefactor(25.0, dozer, 2, 1),
            "rubble husk must accept sole-benefactor heal"
        );
    }
    let after = logic.host_object(span).expect("span after heal");
    assert!(after.health.current > 0.0);
    assert!(!after.status.keep_as_rubble);
    assert!(after.is_alive());

    // Heal on the clicked tower mirrors percent onto the rubble span.
    let mut logic = GameLogic::new();
    let (span, t0, _) = spawn_linked_bridge(&mut logic);
    if let Some(s) = logic.host_object_mut(span) {
        s.convert_bridge_to_rubble_husk();
    }
    let t0_max = logic.host_object(t0).expect("t0").health.maximum;
    crate::game_logic::host_bridge_behavior::record_mirror(
        t0,
        t0_max * 0.25,
        t0_max,
        None,
        crate::game_logic::combat::DamageType::Healing.to_store() as u32,
        0,
        crate::game_logic::host_bridge_behavior::HostBridgeMirrorKind::Heal,
    );
    logic.sync_host_bridge_rubble_and_scaffolds();
    let span_obj = logic.host_object(span).expect("mirrored span");
    assert!(
        span_obj.health.current > 0.0,
        "clicked-tower heal must revive rubble span, hp={}",
        span_obj.health.current
    );
    assert!(span_obj.is_alive());
}

#[test]
fn tower_damage_and_heal_mirror_to_span_and_siblings() {
    // C++ BridgeTowerBehavior.cpp:68-215 percent mirror.
    let mut logic = GameLogic::new();
    let (span, t0, t1) = spawn_linked_bridge(&mut logic);
    let before_span = logic.host_object(span).expect("span").health.current;
    let before_t1 = logic.host_object(t1).expect("t1").health.current;
    let t0_max = logic.host_object(t0).expect("t0").health.maximum;
    if let Some(t) = logic.host_object_mut(t0) {
        let _ = t.take_damage_from(t0_max * 0.25, None);
    }
    logic.sync_host_bridge_rubble_and_scaffolds();
    let span_hp = logic.host_object(span).expect("span").health.current;
    let t1_hp = logic.host_object(t1).expect("t1").health.current;
    assert!(
        span_hp < before_span - 1.0,
        "span must take mirrored % damage {before_span} -> {span_hp}"
    );
    assert!(
        t1_hp < before_t1 - 1.0,
        "sibling tower must take mirrored % damage {before_t1} -> {t1_hp}"
    );

    if let Some(t) = logic.host_object_mut(t0) {
        t.revive_from_bridge_rubble();
        let _ = t.take_damage_from_typed(
            t0_max * 0.25,
            None,
            crate::game_logic::combat::DamageType::Healing,
        );
    }
    logic.sync_host_bridge_rubble_and_scaffolds();
    let span_healed = logic.host_object(span).expect("span").health.current;
    let t1_healed = logic.host_object(t1).expect("t1").health.current;
    assert!(
        span_healed > span_hp + 1.0,
        "span must take mirrored % heal {span_hp} -> {span_healed}"
    );
    assert!(
        t1_healed > t1_hp + 1.0,
        "sibling tower must take mirrored % heal {t1_hp} -> {t1_healed}"
    );
}

#[test]
fn killing_tower_collapses_span_and_span_death_kills_towers() {
    // C++ tower onDie kills bridge; bridge onDie kills towers.
    let mut logic = GameLogic::new();
    let (span, t0, t1) = spawn_linked_bridge(&mut logic);
    if let Some(t) = logic.host_object_mut(t0) {
        let max = t.health.maximum;
        let _ = t.take_damage_from(max * 2.0, None);
    }
    logic.sync_host_bridge_rubble_and_scaffolds();
    let span_obj = logic.host_object(span).expect("span");
    let t1_obj = logic.host_object(t1).expect("t1");
    assert!(
        span_obj.status.keep_as_rubble || span_obj.health.current <= 0.0,
        "killing a tower must collapse the span"
    );
    assert!(
        t1_obj.status.keep_as_rubble || t1_obj.health.current <= 0.0,
        "span death must kill sibling towers"
    );

    let mut logic = GameLogic::new();
    let (span, t0, t1) = spawn_linked_bridge(&mut logic);
    logic.destroy_object(span);
    logic.sync_host_bridge_rubble_and_scaffolds();
    let t0_obj = logic.host_object(t0).expect("t0");
    let t1_obj = logic.host_object(t1).expect("t1");
    assert!(
        t0_obj.status.keep_as_rubble || t0_obj.health.current <= 0.0,
        "span destroy_object must kill towers"
    );
    assert!(
        t1_obj.status.keep_as_rubble || t1_obj.health.current <= 0.0,
        "span destroy_object must kill both towers"
    );
}

#[test]
fn live_tick_drains_bridge_mirrors_and_death_links() {
    let step = include_str!("../world_tick/step.rs");
    assert!(
        step.contains("sync_host_bridge_rubble_and_scaffolds"),
        "live update_simulation must drain bridge mirrors/death/scaffolds"
    );
    let repair = include_str!("../world_objects/support_states/update.rs");
    assert!(
        repair.contains("repair_target_rubble") && repair.contains("remove_bridge_scaffolding"),
        "Repairing arm must allow rubble husks and remove scaffolds on complete"
    );
}
