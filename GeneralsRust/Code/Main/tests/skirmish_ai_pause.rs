//! Exact controlling-player pause authority through normal host commands/ticks.
//! C++ AIGroup.cpp:238-259 and AIUpdate.cpp:4287-4327 use the object's controller.
//! Missing host AI registration intentionally retains the existing inactive policy.
use generals_main::ai::AIDifficulty;
use generals_main::command_system::{CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate, Weapon,
};
use glam::Vec3;
use serde_json::{Value, json};
use std::time::{Duration, UNIX_EPOCH};

const UNIT: &str = "PauseUnit";
const TARGET: &str = "PauseTarget";
const PRODUCER: &str = "PauseWarFactory";
const WORKER: &str = "PauseDozer";

fn world() -> GameLogic {
    let mut logic = GameLogic::new();
    for id in 0..4 {
        let mut player = Player::new(
            id,
            if id == 3 { Team::GLA } else { Team::USA },
            "Pause",
            id == 0,
        );
        player.alliance_team = if id == 3 { 45 } else { 31 };
        logic.add_player(player);
    }
    for (name, kind) in [
        (UNIT, KindOf::Vehicle),
        (TARGET, KindOf::Vehicle),
        (PRODUCER, KindOf::Structure),
        (WORKER, KindOf::Worker),
    ] {
        let mut template = ThingTemplate::new(name);
        template
            .set_health(1000.0)
            .set_cost(125, 0)
            .add_kind_of(kind)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable);
        template.build_time = 30.0;
        if name == UNIT {
            template.set_primary_weapon(weapon());
        } else {
            template.set_primary_weapon_none();
        }
        if name == WORKER {
            template
                .add_kind_of(KindOf::Dozer)
                .add_kind_of(KindOf::Vehicle);
        }
        logic.templates.insert(name.into(), template);
    }
    logic
}

fn weapon() -> Weapon {
    Weapon {
        damage: 25.0,
        range: 300.0,
        reload_time: 0.0,
        projectile_speed: 0.0,
        splash_radius: 0.0,
        ..Weapon::default()
    }
}

fn create(logic: &mut GameLogic, template: &str, owner: u32, x: f32) -> ObjectId {
    logic
        .create_object_for_player(template, owner, Vec3::new(x, 0.0, 0.0))
        .unwrap()
}

fn register(logic: &mut GameLogic, owner: u32, active: bool) {
    let team = logic.get_player(owner).unwrap().team;
    logic.add_ai_opponent(owner, team, AIDifficulty::Easy);
    logic.set_ai_active(owner, active);
    assert_eq!(logic.is_host_ai_active(owner), active);
}

fn attack(logic: &mut GameLogic, owner: u32, attacker: ObjectId, target: ObjectId) {
    logic.queue_command(GameCommand {
        command_type: CommandType::AttackObject { target_id: target },
        player_id: owner,
        command_id: 1,
        timestamp: UNIX_EPOCH + Duration::from_secs(1),
        selected_units: vec![attacker],
        modifier_keys: ModifierKeys::default(),
    });
    logic.process_commands();
}

fn paused_for(logic: &GameLogic, id: ObjectId) -> bool {
    logic.skirmish_ai_auto_engage_paused(id)
}

fn combat(logic: &GameLogic, id: ObjectId) -> Value {
    let o = logic.host_object(id).unwrap();
    json!({"target":o.target.map(|id| id.0),"location":o.target_location.map(|p| p.to_array()),
        "force":o.force_attack,"ai":format!("{:?}",o.ai_state),
        "movement":format!("{:?}",o.movement),"queue":o.building_data.as_ref().map(|b| &b.production_queue)})
}

#[test]
fn human_orders_survive_same_faction_unregistered_active_and_paused_ai() {
    for state in [None, Some(true), Some(false)] {
        let mut logic = world();
        if let Some(active) = state {
            register(&mut logic, 1, active);
        }
        let attacker = create(&mut logic, UNIT, 0, 0.0);
        let target = create(&mut logic, TARGET, 3, 10.0);
        attack(&mut logic, 0, attacker, target);
        assert_eq!(logic.host_object(attacker).unwrap().target, Some(target));
        for _ in 0..2 {
            logic.update();
        }
        let a = logic.host_object(attacker).unwrap();
        assert_eq!(a.target, Some(target), "foreign AI state={state:?}");
        assert_eq!(a.ai_state, AIState::Attacking);
        assert_eq!(a.weapon_discharge_marker().sequence, 1);
        assert_eq!(logic.host_object(target).unwrap().health.current, 975.0);
    }
}

#[test]
fn exact_nonlocal_owner_preserves_active_inactive_and_missing_registration_policy() {
    for state in [None, Some(true), Some(false)] {
        let mut logic = world();
        // An unrelated unregistered USA player must not suppress active owner1.
        if let Some(active) = state {
            register(&mut logic, 1, active);
        }
        let attacker = create(&mut logic, UNIT, 1, 0.0);
        let target = create(&mut logic, TARGET, 3, 10.0);
        attack(&mut logic, 1, attacker, target);
        assert_eq!(logic.host_object(attacker).unwrap().target, Some(target));
        for _ in 0..2 {
            logic.update();
        }
        let a = logic.host_object(attacker).unwrap();
        let active = state == Some(true);
        assert_eq!(a.target, active.then_some(target), "own AI state={state:?}");
        assert_eq!(
            a.weapon_discharge_marker().sequence,
            u32::from(active) as u64
        );
        assert_eq!(
            logic.host_object(target).unwrap().health.current,
            if active { 975.0 } else { 1000.0 }
        );
    }
}

#[test]
fn two_same_faction_ai_players_pause_independently_in_both_orders() {
    for (paused, active) in [(1, 2), (2, 1)] {
        let mut logic = world();
        register(&mut logic, 1, true);
        register(&mut logic, 2, true);
        let first = create(&mut logic, UNIT, paused, 0.0);
        let second = create(&mut logic, UNIT, active, 0.0);
        let target = create(&mut logic, TARGET, 3, 10.0);
        attack(&mut logic, paused, first, target);
        attack(&mut logic, active, second, target);
        logic.pause_skirmish_ai_and_clear_combat(paused);
        assert_eq!(logic.host_object(first).unwrap().target, None);
        assert_eq!(logic.host_object(second).unwrap().target, Some(target));
        for _ in 0..2 {
            logic.update();
        }
        assert_eq!(
            logic
                .host_object(second)
                .unwrap()
                .weapon_discharge_marker()
                .sequence,
            1
        );
        assert_eq!(
            logic
                .host_object(first)
                .unwrap()
                .weapon_discharge_marker()
                .sequence,
            0
        );
        assert_eq!(logic.host_object(target).unwrap().health.current, 975.0);
        logic.pause_skirmish_ai_and_clear_combat(active);
        assert_eq!(logic.host_object(second).unwrap().target, None);
    }
}

#[test]
fn local_registered_inactive_ai_does_not_pause_human() {
    let mut logic = world();
    register(&mut logic, 0, false);
    let attacker = create(&mut logic, UNIT, 0, 0.0);
    let target = create(&mut logic, TARGET, 3, 10.0);
    assert!(!paused_for(&logic, attacker));
    attack(&mut logic, 0, attacker, target);
    for _ in 0..2 {
        logic.update();
    }
    assert_eq!(
        logic
            .host_object(attacker)
            .unwrap()
            .weapon_discharge_marker()
            .sequence,
        1
    );
}

#[test]
fn owner_resolution_has_no_faction_fallback_or_new_alive_faction_filters() {
    let mut logic = world();
    register(&mut logic, 1, false);
    let attacker = create(&mut logic, UNIT, 1, 0.0);
    for (owner, alive, faction, expected) in [
        (None, true, Team::USA, false),
        (Some(99), true, Team::USA, false),
        (Some(1), true, Team::USA, true),
        (Some(1), false, Team::USA, true),
        (Some(1), true, Team::GLA, true),
        (Some(0), true, Team::USA, false),
    ] {
        logic.get_player_mut(1).unwrap().is_alive = alive;
        let a = logic.host_object_mut(attacker).unwrap();
        a.owner_player_id = owner;
        a.team = faction;
        assert_eq!(
            paused_for(&logic, attacker),
            expected,
            "owner={owner:?} alive={alive} faction={faction:?}"
        );
    }
}

fn seed_combat(logic: &mut GameLogic, id: ObjectId, target: ObjectId, worker: bool) {
    let o = logic.host_object_mut(id).unwrap();
    o.target = Some(target);
    o.target_location = Some(Vec3::new(20.0, 0.0, 0.0));
    o.set_status_force_attack(true);
    if worker {
        o.move_to(Vec3::new(200.0, 0.0, 0.0));
        o.set_ai_state(AIState::Constructing);
    } else {
        o.set_ai_state(AIState::Attacking);
    }
}

#[test]
fn immediate_pause_only_clears_living_exact_owner_and_refunds_once() {
    let mut logic = world();
    register(&mut logic, 1, true);
    register(&mut logic, 2, true);
    let target = create(&mut logic, TARGET, 3, 10.0);
    let mut owned = Vec::new();
    let mut foreign = Vec::new();
    for owner in [0, 1, 2] {
        for template in [UNIT, PRODUCER, WORKER] {
            let id = create(&mut logic, template, owner, 0.0);
            if template == PRODUCER {
                assert!(logic.enqueue_production(id, UNIT.into()));
            }
            seed_combat(&mut logic, id, target, template == WORKER);
            if owner == 1 {
                owned.push(id);
            } else {
                foreign.push(id);
            }
        }
    }
    let dead = create(&mut logic, PRODUCER, 1, 0.0);
    assert!(logic.enqueue_production(dead, UNIT.into()));
    seed_combat(&mut logic, dead, target, false);
    logic.host_object_mut(dead).unwrap().health.current = 0.0;
    foreign.push(dead); // The living-object filter must preserve the dead queue.
    for owner in [None, Some(99)] {
        let id = create(&mut logic, UNIT, 1, 0.0);
        logic.host_object_mut(id).unwrap().owner_player_id = owner;
        seed_combat(&mut logic, id, target, false);
        foreign.push(id);
    }
    let before: Vec<_> = foreign.iter().map(|&id| combat(&logic, id)).collect();
    let balances: Vec<_> = (0..3)
        .map(|id| logic.get_player(id).unwrap().resources.supplies)
        .collect();
    logic.pause_skirmish_ai_and_clear_combat(99);
    assert_eq!(
        foreign
            .iter()
            .map(|&id| combat(&logic, id))
            .collect::<Vec<_>>(),
        before
    );
    logic.pause_skirmish_ai_and_clear_combat(1);
    assert!(!logic.is_host_ai_active(1));
    assert!(logic.is_host_ai_active(2));
    for id in owned {
        let o = logic.host_object(id).unwrap();
        assert_eq!(o.target, None);
        assert_eq!(o.target_location, None);
        assert!(!o.force_attack);
        assert_eq!(o.ai_state, AIState::Idle);
        if let Some(b) = &o.building_data {
            assert!(b.production_queue.is_empty());
        }
        if o.is_kind_of(KindOf::Worker) {
            assert_eq!(o.movement.target_position, None);
        }
    }
    assert_eq!(
        foreign
            .iter()
            .map(|&id| combat(&logic, id))
            .collect::<Vec<_>>(),
        before
    );
    for id in 0..3 {
        assert_eq!(
            logic.get_player(id).unwrap().resources.supplies,
            balances[id as usize] + if id == 1 { 125 } else { 0 }
        );
    }
    let refunded = logic.get_player(1).unwrap().resources.supplies;
    logic.pause_skirmish_ai_and_clear_combat(1);
    assert_eq!(logic.get_player(1).unwrap().resources.supplies, refunded);
    assert_eq!(
        foreign
            .iter()
            .map(|&id| combat(&logic, id))
            .collect::<Vec<_>>(),
        before
    );
}

#[test]
fn clear_keeps_existing_production_validation_for_dead_or_mismatched_owner() {
    for dead_owner in [false, true] {
        let mut logic = world();
        let producer = create(&mut logic, PRODUCER, 1, 0.0);
        let target = create(&mut logic, TARGET, 3, 10.0);
        assert!(logic.enqueue_production(producer, UNIT.into()));
        seed_combat(&mut logic, producer, target, false);
        if dead_owner {
            logic.get_player_mut(1).unwrap().is_alive = false;
        } else {
            logic.host_object_mut(producer).unwrap().team = Team::GLA;
        }
        let balance = logic.get_player(1).unwrap().resources.supplies;
        logic.pause_skirmish_ai_and_clear_combat(1);
        let o = logic.host_object(producer).unwrap();
        assert_eq!(o.target, None);
        assert_eq!(o.ai_state, AIState::Idle);
        assert_eq!(o.building_data.as_ref().unwrap().production_queue.len(), 1);
        assert_eq!(logic.get_player(1).unwrap().resources.supplies, balance);
    }
}

#[test]
fn foreign_same_faction_selection_remains_rejected() {
    let mut logic = world();
    let attacker = create(&mut logic, UNIT, 1, 0.0);
    let target = create(&mut logic, TARGET, 3, 10.0);
    let before = combat(&logic, attacker);
    attack(&mut logic, 0, attacker, target);
    assert_eq!(combat(&logic, attacker), before);
}

fn specialty(name: &str, paused_owner: Option<u32>) -> (u64, f32) {
    use generals_main::game_logic::DeployStyleMetadata;
    use generals_main::game_logic::host_strategy_center::{HostBattlePlan, HostBattlePlanRegistry};
    let mut logic = world();
    register(&mut logic, 1, true);
    register(&mut logic, 2, true);
    let mut template = ThingTemplate::new(name);
    template
        .set_health(1000.0)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_primary_weapon(weapon());
    match name {
        "PauseDefense" => {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::FSBaseDefense);
        }
        "PauseStrategy" => {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::FSStrategyCenter);
        }
        "AmericaVehicleSentryDrone" => {
            template.add_kind_of(KindOf::Vehicle);
            template.deploy_style_metadata = Some(DeployStyleMetadata {
                turrets_function_only_when_deployed: true,
                ..DeployStyleMetadata::default()
            });
        }
        _ => {
            template.add_kind_of(KindOf::Vehicle);
        }
    }
    logic.templates.insert(name.into(), template);
    let attacker = create(&mut logic, name, 1, 0.0);
    let target = create(&mut logic, TARGET, 3, 150.0);
    if name == "PauseStrategy" {
        // Isolate the pause gate from the separate player_id_for_team ambiguity.
        // Every USA row has the same plan; controller resolution is unchanged.
        let mut plans = HostBattlePlanRegistry::new();
        for id in 0..3 {
            plans.set_active_plan(id, HostBattlePlan::Bombardment);
        }
        logic.restore_battle_plans(plans);
        // The Strategy Center spawn path deliberately removes its gun until
        // Bombardment is active. Seed that matching active-plan combat state;
        // the fixture exercises admission, not the independent door animation.
        let center = logic.host_object_mut(attacker).unwrap();
        center.weapon = Some(weapon());
        center.turret_enabled = true;
    }
    if let Some(owner) = paused_owner {
        logic.set_ai_active(owner, false);
    }
    for _ in 0..4 {
        logic.update();
    }
    (
        logic
            .host_object(attacker)
            .unwrap()
            .weapon_discharge_marker()
            .sequence,
        logic.host_object(target).unwrap().health.current,
    )
}

#[test]
fn all_four_specialty_admission_gates_use_the_exact_attacker() {
    let mut failures = Vec::new();
    for name in [
        "PauseDefense",
        "PauseStrategy",
        "AmericaVehicleSentryDrone",
        "AmericaVehicleHellfireDrone",
    ] {
        let active = specialty(name, None);
        let foreign_pause = specialty(name, Some(2));
        let own_pause = specialty(name, Some(1));
        println!(
            "specialty {name}: active={active:?} foreign_pause={foreign_pause:?} own_pause={own_pause:?}"
        );
        if !(active.0 > 0 && active.1 < 1000.0) {
            failures.push(format!("{name}: active control must discharge: {active:?}"));
        }
        if foreign_pause != active {
            failures.push(format!(
                "{name}: foreign pause changed discharge: {foreign_pause:?} vs {active:?}"
            ));
        }
        if own_pause != (0, 1000.0) {
            failures.push(format!(
                "{name}: own pause failed to suppress discharge: {own_pause:?}"
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
