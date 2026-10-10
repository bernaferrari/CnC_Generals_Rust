//! CPP ScriptActions.cpp1861 / AIUpdate.cpp4014: Guard belongs to its session.
use super::named_command_test_support::{execute, world};
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::ScriptActionType;

fn guard(world: &mut GameLogic, name: &str) {
    assert_eq!(
        execute(world, &[(ScriptActionType::NamedGuard, name, "")]),
        1
    );
}

#[derive(Debug, PartialEq)]
struct GuardObservation {
    state: AIState,
    anchor: Option<Vec3>,
    guard_target: Option<ObjectId>,
    mode: GuardMode,
    source: u32,
    scan_deadline: Option<u32>,
    attack_target: Option<ObjectId>,
    position: Vec3,
    moving: bool,
}

fn projection(world: &GameLogic, id: ObjectId) -> GuardObservation {
    let unit = world.host_object(id).unwrap();
    GuardObservation {
        state: unit.ai_state.clone(),
        anchor: unit.guard_position,
        guard_target: unit.guard_target,
        mode: unit.guard_mode,
        source: unit.last_command_source,
        scan_deadline: unit.unit_ai_runtime.guard_scan_deadline(),
        attack_target: unit.target,
        position: unit.get_position(),
        moving: unit.status.moving,
    }
}

#[test]
fn guard_action_walk_calibration() {
    let (mut world, _, _) = world();
    assert_eq!(execute(&mut world, &[]), 1);
}

#[test]
fn guard_enters_driving_object_at_current_position_with_normal_script_order() {
    let (mut first, id, _) = world();
    let (mut second, second_id, _) = world();
    assert_eq!(id, second_id);
    let position = first.host_object(id).unwrap().get_position();
    first
        .host_object_mut(id)
        .unwrap()
        .set_guard_mode(GuardMode::WithoutPursuit);
    guard(&mut first, "NamedUnit");
    let unit = first.host_object(id).unwrap();
    assert_eq!(unit.formation_id, 0);
    assert_eq!(unit.formation_offset, glam::Vec2::ZERO);
    assert_eq!(
        unit.cur_locomotor_name.as_deref(),
        Some("BasicHumanLocomotor")
    );
    assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_NORMAL"));
    assert_eq!(unit.guard_position, Some(position));
    assert_eq!(unit.guard_target, None);
    assert_eq!(unit.guard_mode, GuardMode::Normal);
    assert_eq!(unit.ai_state, AIState::GuardingArea);
    assert_eq!(
        unit.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(second.host_object(id).unwrap().formation_id, 19);
    assert_eq!(second.host_object(id).unwrap().guard_position, None);
    second.reset();
    assert_eq!(
        first.host_object(id).unwrap().guard_position,
        Some(position)
    );
}

#[test]
fn guard_reentry_replaces_previous_object_area_and_route() {
    let (mut world, id, target) = world();
    guard(&mut world, "NamedUnit");
    let unit = world.host_object_mut(id).unwrap();
    unit.set_guard_target(Some(target));
    unit.guard_area_trigger = Some("PreviousArea".into());
    unit.set_guard_mode(GuardMode::FlyingUnitsOnly);
    unit.set_target(Some(target));
    unit.set_target_location(Some(Vec3::splat(400.0)));
    unit.set_force_attack(true);
    unit.pending_move = Some(Vec3::splat(300.0));
    unit.requested_destination = Some(Vec3::splat(300.0));
    unit.waiting_for_path = true;
    unit.movement.path = vec![Vec3::splat(300.0)];
    unit.guard_chase_phase = 1;
    unit.guard_chase_give_up_frame = 999;
    unit.unit_ai_runtime.set_guard_scan_deadline(Some(900));
    unit.unit_ai_runtime
        .observe_guard_anchor(Vec3::splat(900.0));
    let position = Vec3::new(120.0, 7.0, 130.0);
    unit.set_position(position);
    world.frame = 100;
    guard(&mut world, "NamedUnit");
    let unit = world.host_object(id).unwrap();
    assert_eq!(unit.guard_position, Some(position));
    assert_eq!(unit.guard_target, None);
    assert_eq!(unit.guard_area_trigger, None);
    assert_eq!(unit.guard_mode, GuardMode::Normal);
    assert_eq!(unit.target, None);
    assert_eq!(unit.target_location, None);
    assert!(!unit.force_attack);
    assert_eq!(unit.pending_move, None);
    assert_ne!(unit.requested_destination, Some(Vec3::splat(300.0)));
    assert!(!unit.movement.path.contains(&Vec3::splat(300.0)));
    assert_eq!(unit.guard_chase_phase, 0);
    assert_eq!(unit.guard_chase_give_up_frame, 0);
    assert_eq!(unit.unit_ai_runtime.guard_anchor(), None);
    assert!(
        unit.unit_ai_runtime
            .guard_scan_deadline()
            .is_some_and(|frame| (100..=160).contains(&frame))
    );
}

#[test]
fn guard_snapshot_continues_without_a_reissued_command() {
    let (mut source, id, _) = world();
    // Different factions do not imply hostility: map admission owns diplomacy.
    source
        .players
        .get_mut(&1)
        .unwrap()
        .set_map_relationship(2, gamelogic::common::Relationship::Enemies);
    guard(&mut source, "NamedUnit");
    let saved = projection(&source, id);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(projection(&restored, id), saved);
    assert_eq!(saved.mode, GuardMode::Normal);
    assert_eq!(
        saved.source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    let mut acquired = false;
    for frame in 1..90 {
        for world in [&mut source, &mut restored] {
            world.frame = frame;
            world.update_ai(&[id], 1.0 / 30.0);
        }
        acquired |= source.host_object(id).unwrap().target.is_some();
        assert_eq!(
            projection(&restored, id),
            projection(&source, id),
            "continuation frame {frame}"
        );
    }
    assert!(
        acquired,
        "saved Guard must acquire a real enemy through the AI phase"
    );
    let continued = projection(&restored, id);
    source.reset();
    assert_eq!(projection(&restored, id), continued);
}

#[test]
fn guard_missing_exact_name_and_absent_ai_are_inert() {
    let (mut world, id, _) = world();
    for name in ["Absent", "namedunit", ""] {
        guard(&mut world, name);
    }
    world
        .host_object_mut(id)
        .unwrap()
        .template_mut()
        .set_authored_ai_update_interface(Some(false));
    guard(&mut world, "NamedUnit");
    let unit = world.host_object(id).unwrap();
    assert_eq!(unit.formation_id, 19);
    assert_eq!(unit.guard_position, None);
    assert_eq!(
        unit.cur_locomotor_name.as_deref(),
        Some("RedguardLocomotor")
    );
}

#[test]
fn guard_does_not_idle_passengers() {
    let (mut world, id, target) = world();
    let passenger = world
        .create_object_for_player("OwnedNamedInfantry", 1, Vec3::ZERO)
        .unwrap();
    world
        .host_object_mut(passenger)
        .unwrap()
        .set_target(Some(target));
    world.host_object_mut(id).unwrap().occupants.push(passenger);
    guard(&mut world, "NamedUnit");
    assert_eq!(world.host_object(passenger).unwrap().target, Some(target));
}

// Tests below additionally exercise command admission on the owned route.
#[test]
fn guard_rejected_admission_still_leaves_formation_and_selects_normal() {
    let setters: [fn(&mut Object); 5] = [
        |u| u.status.effectively_dead = true,
        |u| {
            u.health.current = 0.0;
        },
        |u| {
            u.template_mut().add_kind_of(KindOf::Immobile);
        },
        |u| {
            u.template_mut().add_kind_of(KindOf::Projectile);
        },
        |u| u.status.disabled_held = true,
    ];
    for reject in setters {
        let (mut world, id, _) = world();
        reject(world.host_object_mut(id).unwrap());
        guard(&mut world, "NamedUnit");
        let unit = world.host_object(id).unwrap();
        assert_eq!(unit.formation_id, 0);
        assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_NORMAL"));
        assert_eq!(unit.guard_position, None);
        assert_eq!(
            unit.last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI
        );
    }
}

#[test]
fn guard_sleep_gate_uses_controller_instead_of_locality() {
    for human in [false, true] {
        let (mut world, id, _) = world();
        let player = world.players.get_mut(&1).unwrap();
        player.is_human = human;
        player.is_local = !human;
        world
            .host_object_mut(id)
            .unwrap()
            .set_ai_attitude(crate::game_logic::host_strategy_center::HostAiAttitude::Sleep);
        guard(&mut world, "NamedUnit");
        let unit = world.host_object(id).unwrap();
        assert_eq!(unit.formation_id, 0);
        assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_NORMAL"));
        assert_eq!(unit.guard_position.is_some(), human);
    }
}

#[test]
fn guard_exits_old_hunt_and_attack_submachine() {
    let (mut world, id, target) = world();
    assert!(world.unit_command_patrol(id));
    let unit = world.host_object_mut(id).unwrap();
    unit.set_ai_state(AIState::Attacking);
    unit.set_target(Some(target));
    unit.status.is_firing_weapon = true;
    unit.status.is_aiming_weapon = true;
    unit.status.attacking = true;
    unit.status.ignoring_stealth = true;
    unit.leech_range_active_primary = true;
    unit.turret_enabled = true;
    unit.set_turret_target_object(Some(target), true);
    guard(&mut world, "NamedUnit");
    let unit = world.host_object(id).unwrap();
    assert!(!unit.hunting);
    assert_eq!(unit.unit_ai_runtime.hunt_scan_deadline(), None);
    assert!(!unit.status.is_firing_weapon);
    assert!(!unit.status.is_aiming_weapon);
    assert!(!unit.status.attacking);
    assert!(!unit.status.ignoring_stealth);
    assert!(!unit.leech_range_active_primary);
    assert_eq!(unit.turret_target_id, None);
}

#[test]
fn guard_this_object_uses_only_the_supplied_owned_id() {
    use gamelogic::scripting::engine::{ScriptExecutionDriver, ScriptNamedCommand};
    let (mut world, id, other) = world();
    for supplied in [None, Some(u32::MAX)] {
        HostScriptExecutionDriver::new(&mut world)
            .named_command(
                ScriptNamedCommand::Guard {
                    unit: gamelogic::scripting::core::THIS_OBJECT,
                },
                supplied,
            )
            .unwrap()
            .unwrap();
        assert_eq!(world.host_object(id).unwrap().guard_position, None);
    }
    HostScriptExecutionDriver::new(&mut world)
        .named_command(
            ScriptNamedCommand::Guard {
                unit: gamelogic::scripting::core::THIS_OBJECT,
            },
            Some(id.0),
        )
        .unwrap()
        .unwrap();
    assert!(world.host_object(id).unwrap().guard_position.is_some());
    assert_eq!(world.host_object(other).unwrap().guard_position, None);
}

#[test]
fn guard_script_anchor_is_not_clipped_to_playable_bounds() {
    let (mut world, id, _) = world();
    let position = Vec3::new(-25.0, 7.0, 1050.0);
    world.host_object_mut(id).unwrap().set_position(position);
    guard(&mut world, "NamedUnit");
    assert_eq!(
        world.host_object(id).unwrap().guard_position,
        Some(position)
    );
}

#[test]
fn guard_reentry_clears_only_its_own_inner_common_target() {
    let (mut first, id, target) = world();
    let (mut other, _, _) = world();
    for world in [&mut first, &mut other] {
        let unit = world.host_object_mut(id).unwrap();
        unit.team_instance_name = "OwnedGuardTeam".into();
        unit.guard_chase_phase = 1;
        world
            .team_common_attack_targets
            .insert("OwnedGuardTeam".into(), target);
    }
    guard(&mut first, "NamedUnit");
    assert!(
        !first
            .team_common_attack_targets
            .contains_key("OwnedGuardTeam")
    );
    assert_eq!(
        other.team_common_attack_targets.get("OwnedGuardTeam"),
        Some(&target)
    );
}

#[test]
fn guard_disabled_masks_reject_after_script_preparation() {
    let setters: [fn(&mut ObjectStatus); 11] = [
        |s| s.disabled_underpowered = true,
        |s| s.disabled_unmanned = true,
        |s| s.disabled_hacked = true,
        |s| s.disabled_emp = true,
        |s| s.disabled_paralyzed = true,
        |s| s.disabled_subdued = true,
        |s| s.disabled_freefall = true,
        |s| s.disabled_default = true,
        |s| s.disabled_script_disabled = true,
        |s| s.disabled_script_underpowered = true,
        |s| s.disabled_held = true,
    ];
    for reject in setters {
        let (mut world, id, _) = world();
        reject(&mut world.host_object_mut(id).unwrap().status);
        guard(&mut world, "NamedUnit");
        let unit = world.host_object(id).unwrap();
        assert_eq!(unit.formation_id, 0);
        assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_NORMAL"));
        assert_eq!(unit.guard_position, None);
        assert_eq!(
            unit.last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI
        );
    }
}
