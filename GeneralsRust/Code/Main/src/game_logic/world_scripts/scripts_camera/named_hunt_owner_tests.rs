//! ScriptActions.cpp1966 / AIUpdate.cpp3611: commands belong to the driving session.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

use super::named_command_test_support::{execute, world};

fn hunt(world: &mut GameLogic, name: &str) {
    assert_eq!(
        execute(world, &[(ScriptActionType::NamedHunt, name, "")]),
        1
    );
}

#[test]
fn hunt_action_walk_calibration() {
    let (mut world, _, _) = world();
    assert_eq!(execute(&mut world, &[]), 1);
}

#[test]
fn hunt_enters_driving_object_without_leaving_group() {
    let (mut first, id, _) = world();
    let (mut foreign, foreign_id, _) = world();
    assert_eq!(id, foreign_id);
    hunt(&mut first, "NamedUnit");
    let unit = first.host_object(id).unwrap();
    assert!(unit.hunting);
    assert_eq!(unit.ai_state, AIState::Patrolling);
    assert_eq!(
        unit.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(unit.formation_id, 19);
    assert_eq!(unit.formation_offset, glam::Vec2::new(4.0, 6.0));
    assert!(!unit.is_panicking, "NORMAL must precede Hunt");
    assert_eq!(
        unit.cur_locomotor_name.as_deref(),
        Some("BasicHumanLocomotor")
    );
    assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_NORMAL"));
    assert!(!foreign.host_object(id).unwrap().hunting);
    foreign.reset();
    assert!(first.host_object(id).unwrap().hunting);
}

#[test]
fn hunt_reentry_clears_previous_route_target_and_scan() {
    let (mut world, id, target) = world();
    hunt(&mut world, "NamedUnit");
    world.frame = 100;
    world
        .host_object_mut(id)
        .unwrap()
        .unit_ai_runtime
        .set_hunt_scan_deadline(Some(9000));
    world.host_object_mut(id).unwrap().set_target(Some(target));
    world.host_object_mut(id).unwrap().pending_move = Some(Vec3::splat(200.0));
    assert!(world.append_unit_waypoint(id, Vec3::new(200.0, 0.0, 200.0)));
    hunt(&mut world, "NamedUnit");
    world.reissue_pending_moves();
    let unit = world.host_object(id).unwrap();
    assert!(unit.hunting);
    assert_eq!(unit.target, None);
    assert_eq!(unit.pending_move, None);
    assert!(unit.movement.path.is_empty());
    assert!(
        unit.unit_ai_runtime
            .hunt_scan_deadline()
            .is_some_and(|n| (100..=130).contains(&n))
    );
    assert_eq!(unit.formation_id, 19);
}

#[test]
fn hunt_snapshot_continues_on_restored_owner() {
    let (mut source, id, _) = world();
    hunt(&mut source, "NamedUnit");
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(restored.host_object(id).unwrap().hunting);
    assert_eq!(
        restored.host_object(id).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(
        restored
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .hunt_scan_deadline(),
        source
            .host_object(id)
            .unwrap()
            .unit_ai_runtime
            .hunt_scan_deadline()
    );
    // Both continue the saved Hunt parent, with no new command. Model the
    // nested attack finishing: the real AI phase must resume its Hunt scan.
    for world in [&mut source, &mut restored] {
        let unit = world.host_object_mut(id).unwrap();
        unit.set_target(None);
        unit.set_ai_state(AIState::Idle);
        world.frame = 1;
        world.update_ai(&[id], 1.0 / 30.0);
        assert!(world.host_object(id).unwrap().hunting);
        assert_ne!(world.host_object(id).unwrap().ai_state, AIState::Idle);
    }
    assert_eq!(
        source.host_object(id).unwrap().ai_state,
        restored.host_object(id).unwrap().ai_state
    );
    source.reset();
    hunt(&mut restored, "NamedUnit");
    assert!(restored.host_object(id).unwrap().hunting);
    assert_eq!(restored.host_object(id).unwrap().formation_id, 19);
}

#[test]
fn missing_hunt_is_an_authoritative_noop() {
    let (mut world, id, _) = world();
    hunt(&mut world, "Absent");
    assert!(!world.host_object(id).unwrap().hunting);
    assert!(world.host_object(id).unwrap().is_panicking);
}

#[test]
fn existing_hunt_parent_survives_snapshot_without_reissuing_order() {
    let (mut source, id, _) = world();
    // Existing production admission calibrates this save regression independently
    // of the new script-driver route (also executable on the OLD source).
    assert!(source.unit_command_patrol(id));
    assert!(source.host_object(id).unwrap().hunting);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, _, _) = world();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(restored.host_object(id).unwrap().hunting);
    assert_eq!(
        restored.host_object(id).unwrap().auto_acquire_when_idle,
        source.host_object(id).unwrap().auto_acquire_when_idle
    );
}

// Tests below additionally exercise command admission on the owned route.
#[test]
fn sleeping_computer_rejects_hunt_after_normal_selection() {
    let (mut world, id, _) = world();
    world.players.get_mut(&1).unwrap().is_human = false;
    // Display locality is independent of the simulation controller.
    world.players.get_mut(&1).unwrap().is_local = true;
    world
        .host_object_mut(id)
        .unwrap()
        .set_ai_attitude(crate::game_logic::host_strategy_center::HostAiAttitude::Sleep);
    hunt(&mut world, "NamedUnit");
    let unit = world.host_object(id).unwrap();
    assert!(!unit.is_panicking);
    assert_eq!(
        unit.cur_locomotor_name.as_deref(),
        Some("BasicHumanLocomotor")
    );
    assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_NORMAL"));
    assert!(!unit.hunting);
    assert_eq!(unit.formation_id, 19);
}

#[test]
fn sleeping_human_accepts_script_hunt() {
    let (mut world, id, _) = world();
    world.players.get_mut(&1).unwrap().is_local = false;
    world
        .host_object_mut(id)
        .unwrap()
        .set_ai_attitude(crate::game_logic::host_strategy_center::HostAiAttitude::Sleep);
    hunt(&mut world, "NamedUnit");
    assert!(world.host_object(id).unwrap().hunting);
}

#[test]
fn effectively_dead_unit_rejects_hunt_after_normal_selection() {
    let (mut world, id, _) = world();
    world.host_object_mut(id).unwrap().status.effectively_dead = true;
    hunt(&mut world, "NamedUnit");
    assert!(!world.host_object(id).unwrap().hunting);
    assert!(!world.host_object(id).unwrap().is_panicking);
}

#[test]
fn nonmobile_and_projectile_units_reject_hunt() {
    for projectile in [false, true] {
        let (mut world, _, _) = world();
        let mut template = ThingTemplate::new("RejectedHunt");
        template
            .set_health(100.0)
            .set_primary_weapon(Weapon::default());
        if projectile {
            template
                .add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Projectile);
        } else {
            template
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Immobile);
        }
        template.set_authored_ai_update_interface(Some(true));
        world.templates.insert(template.name.clone(), template);
        let id = world
            .create_object_for_player("RejectedHunt", 1, Vec3::ZERO)
            .unwrap();
        world.host_object_mut(id).unwrap().name = "Rejected".into();
        assert!(world.host_object(id).unwrap().has_ai_update_interface());
        assert_eq!(
            world.host_object(id).unwrap().is_mobile_for_ai_command(),
            projectile
        );
        hunt(&mut world, "Rejected");
        assert!(!world.host_object(id).unwrap().hunting);
    }
}

#[test]
fn hunt_exits_attack_submachine_before_reentering_parent() {
    let (mut world, id, target) = world();
    hunt(&mut world, "NamedUnit");
    let unit = world.host_object_mut(id).unwrap();
    unit.set_ai_state(AIState::Attacking);
    unit.set_target(Some(target));
    unit.status.is_firing_weapon = true;
    unit.status.is_aiming_weapon = true;
    unit.status.attacking = true;
    unit.status.ignoring_stealth = true;
    unit.model_condition_bits |=
        1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_ATTACKING;
    unit.leech_range_active_primary = true;
    unit.leech_range_active_secondary = true;
    unit.turret_enabled = true;
    unit.set_turret_target_object(Some(target), true);
    hunt(&mut world, "NamedUnit");
    let unit = world.host_object(id).unwrap();
    assert!(unit.hunting);
    assert!(!unit.status.is_firing_weapon);
    assert!(!unit.status.is_aiming_weapon);
    assert!(!unit.status.attacking);
    assert!(!unit.status.ignoring_stealth);
    assert_eq!(
        unit.model_condition_bits
            & (1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_ATTACKING),
        0
    );
    assert!(!unit.leech_range_active_primary);
    assert!(!unit.leech_range_active_secondary);
    assert_eq!(unit.turret_target_id, None);
}

#[test]
fn hunt_this_object_is_exclusively_the_supplied_owned_id() {
    use gamelogic::scripting::engine::{ScriptExecutionDriver, ScriptNamedCommand};
    let (mut world, id, other) = world();
    let (foreign, foreign_id, _) = self::world();
    assert_eq!(id, foreign_id);
    let token = gamelogic::scripting::core::THIS_OBJECT;
    for supplied in [Some(u32::MAX), None] {
        HostScriptExecutionDriver::new(&mut world)
            .named_command(ScriptNamedCommand::Hunt { unit: token }, supplied)
            .unwrap()
            .unwrap();
        assert!(!world.host_object(id).unwrap().hunting);
    }
    HostScriptExecutionDriver::new(&mut world)
        .named_command(ScriptNamedCommand::Hunt { unit: token }, Some(id.0))
        .unwrap()
        .unwrap();
    assert!(world.host_object(id).unwrap().hunting);
    assert!(!world.host_object(other).unwrap().hunting);
    assert!(!foreign.host_object(id).unwrap().hunting);
}

#[test]
fn admitted_no_ai_template_is_inert_despite_mobile_type() {
    let (mut world, id, _) = world();
    world
        .host_object_mut(id)
        .unwrap()
        .template_mut()
        .set_authored_ai_update_interface(Some(false));
    assert!(!world.host_object(id).unwrap().has_ai_update_interface());
    hunt(&mut world, "NamedUnit");
    assert!(!world.host_object(id).unwrap().hunting);
    assert_eq!(
        world.host_object(id).unwrap().cur_locomotor_name.as_deref(),
        Some("RedguardLocomotor")
    );
}

#[test]
fn immobile_infantry_rejects_hunt_even_with_owned_ai() {
    let (mut world, id, _) = world();
    world
        .host_object_mut(id)
        .unwrap()
        .template_mut()
        .add_kind_of(KindOf::Immobile);
    assert!(world.host_object(id).unwrap().has_ai_update_interface());
    assert!(!world.host_object(id).unwrap().is_mobile_for_ai_command());
    hunt(&mut world, "NamedUnit");
    let unit = world.host_object(id).unwrap();
    assert!(!unit.hunting);
    assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_NORMAL"));
    assert_eq!(unit.formation_id, 19);
}

#[test]
fn every_disabled_mask_rejects_hunt_after_normal_selection() {
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
    for (index, disable) in setters.into_iter().enumerate() {
        let (mut world, id, _) = world();
        disable(&mut world.host_object_mut(id).unwrap().status);
        hunt(&mut world, "NamedUnit");
        let unit = world.host_object(id).unwrap();
        assert!(!unit.hunting, "disabled bit {index}");
        assert_eq!(
            unit.last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI
        );
        assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_NORMAL"));
        assert_eq!(unit.formation_id, 19);
    }
}

#[test]
fn construction_and_weapon_jam_flags_do_not_replace_the_disabled_mask() {
    let (mut world, id, _) = world();
    let unit = world.host_object_mut(id).unwrap();
    unit.status.under_construction = true;
    unit.status.weapons_jammed = true;
    assert!(unit.is_mobile_for_ai_command());
    hunt(&mut world, "NamedUnit");
    assert!(world.host_object(id).unwrap().hunting);
}
