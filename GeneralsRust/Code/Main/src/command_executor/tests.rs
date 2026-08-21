#[test]
fn group_move_destinations_spreads_multi_unit() {
    // Wave 955: multi-unit move spreads via group_move_destinations + unit_command_move_free.
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    assert!(
        prod.contains("fn group_move_destinations")
            && prod.contains("group_move_destinations(units, destination)"),
        "multi-unit move must spread destinations"
    );
    let i = prod.find("fn execute_move(").expect("execute_move");
    let w = &prod[i..prod.len().min(i + 2500)];
    assert!(
        w.contains("group_move_destinations")
            && w.contains("unit_command_move_free")
            && !w.contains("assign_unit_path(unit_id, destination, &[])"),
        "execute_move must path to per-unit goals via unit_command_move_free"
    );
}

#[test]
fn service_executor_revalidates_authored_kindof_matrix_not_building_names() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut ground = ThingTemplate::new("GroundSource");
    ground.add_kind_of(KindOf::Vehicle).set_health(200.0);
    let mut aircraft = ThingTemplate::new("AirSource");
    // Retail aircraft satisfy ActionManager's initial VEHICLE gate too.
    aircraft
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Aircraft)
        .set_health(200.0);
    let mut infantry = ThingTemplate::new("InfantrySource");
    infantry.add_kind_of(KindOf::Infantry).set_health(100.0);
    let mut repair_pad = ThingTemplate::new("SourceTaggedGroundDestination");
    repair_pad
        .add_kind_of(KindOf::RepairPad)
        .set_health(1_000.0);
    let mut airfield = ThingTemplate::new("SourceTaggedAirDestination");
    airfield.add_kind_of(KindOf::FSAirfield).set_health(1_000.0);
    let mut heal_pad = ThingTemplate::new("SourceTaggedInfantryDestination");
    heal_pad.add_kind_of(KindOf::HealPad).set_health(1_000.0);
    // This triggers the old BuildingType/name fallback but has no source tag.
    let mut name_only = ThingTemplate::new("RepairHospitalAirfieldWithoutTag");
    name_only.add_kind_of(KindOf::Structure).set_health(1_000.0);

    for template in [
        ground, aircraft, infantry, repair_pad, airfield, heal_pad, name_only,
    ] {
        logic.templates.insert(template.name.clone(), template);
    }

    let ground_ok = logic
        .create_object("GroundSource", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .expect("ground source");
    let ground_reject = logic
        .create_object("GroundSource", Team::USA, Vec3::new(6.0, 0.0, 0.0))
        .expect("ground reject source");
    let aircraft_ok = logic
        .create_object("AirSource", Team::USA, Vec3::new(7.0, 10.0, 0.0))
        .expect("air source");
    let infantry_ok = logic
        .create_object("InfantrySource", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .expect("infantry source");
    let repair_target = logic
        .create_object("SourceTaggedGroundDestination", Team::USA, Vec3::ZERO)
        .expect("repair target");
    let airfield_target = logic
        .create_object(
            "SourceTaggedAirDestination",
            Team::USA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("airfield target");
    let heal_target = logic
        .create_object(
            "SourceTaggedInfantryDestination",
            Team::USA,
            Vec3::new(30.0, 0.0, 0.0),
        )
        .expect("heal target");
    let name_only_target = logic
        .create_object(
            "RepairHospitalAirfieldWithoutTag",
            Team::USA,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("name-only target");

    for id in [ground_ok, ground_reject, aircraft_ok, infantry_ok] {
        logic.host_object_mut(id).expect("source").health.current = 10.0;
    }
    logic
        .host_object_mut(aircraft_ok)
        .expect("aircraft")
        .status
        .airborne_target = true;

    let mut executor = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        executor.execute_get_repaired(&[ground_ok], repair_target),
        CommandResult::Success,
        "ground vehicle requires and accepts authored REPAIR_PAD"
    );
    assert_eq!(
        executor.execute_get_repaired(&[ground_reject], name_only_target),
        CommandResult::InvalidCommand,
        "Repair/Hospital/Airfield spelling without REPAIR_PAD must fail authority"
    );
    assert_eq!(
        executor.execute_get_repaired(&[aircraft_ok], airfield_target),
        CommandResult::Success,
        "airborne VEHICLE+AIRCRAFT requires and accepts authored FS_AIRFIELD"
    );
    assert_eq!(
        executor.execute_get_healed(&[infantry_ok], heal_target),
        CommandResult::Success,
        "infantry requires and accepts authored HEAL_PAD"
    );
}

#[test]
fn group_move_destinations_preserves_relative_offset() {
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    // Minimal mobile templates.
    for name in ["GM_A", "GM_B"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("GM_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let b = logic
        .create_object("GM_B", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .expect("b");
    {
        let oa = logic./* Wave 950 */ host_object_mut(a).unwrap();
        oa.selection_radius = 10.0;
    }
    {
        let ob = logic.host_object_mut(b).unwrap();
        ob.selection_radius = 10.0;
    }

    let click = Vec3::new(100.0, 0.0, 50.0);
    let exec = CommandExecutor::new(&mut logic, 0);
    let goals = exec.group_move_destinations(&[a, b], click);
    assert_eq!(goals.len(), 2);

    // B at x=40 is nearer the click at x=100 than A at x=0 → B is lead.
    let goal_a = goals.iter().find(|(id, _)| *id == a).unwrap().1;
    let goal_b = goals.iter().find(|(id, _)| *id == b).unwrap().1;
    assert!(
        (goal_b - click).length() < 0.01,
        "nearest unit (B) must receive click goal, got {goal_b:?}"
    );
    // A was -40 X from lead/center B → goal keeps ~-40 X from click.
    let offset = goal_a - click;
    assert!(
        offset.x < -20.0 && offset.x > -45.0,
        "relative -X offset preserved, offset={offset:?}"
    );
    assert!(
        offset.z.abs() < 1.0,
        "no invented Z ring offset, offset={offset:?}"
    );
    assert!((goal_a - goal_b).length() > 10.0, "goals must not stack");
}

#[test]
fn scatter_pushes_outward_from_group_center() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["SC_A", "SC_B"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("SC_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("SC_B", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    for id in [a, b] {
        logic.host_object_mut(id).unwrap().selection_radius = 10.0;
    }
    let before_a = logic.host_object(a).unwrap().get_position();
    let before_b = logic.host_object(b).unwrap().get_position();
    let center = (before_a + before_b) * 0.5;

    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(exec.execute_scatter(&[a, b]), CommandResult::Success);

    for id in [a, b] {
        let u = logic.host_object(id).unwrap();
        assert_eq!(u.ai_state, AIState::Moving, "scatter sets Moving");
    }
    for (id, before) in [(a, before_a), (b, before_b)] {
        let u = logic.host_object(id).unwrap();
        let goal = u
            .movement
            .target_position
            .or_else(|| u.movement.path.last().copied())
            .unwrap_or(u.get_position());
        let before_d = (before.x - center.x).hypot(before.z - center.z);
        let after_d = (goal.x - center.x).hypot(goal.z - center.z);
        assert!(
            after_d > before_d + 5.0,
            "scatter should push outward id={id:?} before={before_d} after={after_d} goal={goal:?}"
        );
    }
}

#[test]
fn cheer_uses_three_second_cpp_duration() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("CH_A");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("CH_A".to_string(), tpl);
    let a = logic.create_object("CH_A", Team::USA, Vec3::ZERO).unwrap();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(exec.execute_cheer(&[a]), CommandResult::Success);
    let u = logic.host_object(a).unwrap();
    assert!(
        (u.cheer_timer - 3.0).abs() < 0.01,
        "C++ cheer is 3s (90 frames@30), got {}",
        u.cheer_timer
    );
}

#[test]
fn create_formation_stamps_offsets_not_guard() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["FM_A", "FM_B"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("FM_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("FM_B", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_create_formation(&[a, b]),
        CommandResult::Success
    );
    let ua = logic.host_object(a).unwrap();
    let ub = logic.host_object(b).unwrap();
    assert_ne!(ua.formation_id, 0);
    assert_eq!(ua.formation_id, ub.formation_id);
    // Center at x=20 → offsets -20 and +20
    assert!(
        (ua.formation_offset.x + 20.0).abs() < 0.1,
        "{:?}",
        ua.formation_offset
    );
    assert!(
        (ub.formation_offset.x - 20.0).abs() < 0.1,
        "{:?}",
        ub.formation_offset
    );
    assert_ne!(ua.ai_state, AIState::GuardingArea);

    // Second call dissolves formation (C++ toggle when already formation).
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_create_formation(&[a, b]),
        CommandResult::Success
    );
    assert_eq!(logic.host_object(a).unwrap().formation_id, 0);
    assert_eq!(logic.host_object(b).unwrap().formation_id, 0);
}

#[test]
fn formation_move_uses_stamped_offsets() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["FM_C", "FM_D"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("FM_C", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("FM_D", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_create_formation(&[a, b]),
        CommandResult::Success
    );
    let click = Vec3::new(100.0, 0.0, 50.0);
    let goals = exec.group_move_destinations(&[a, b], click);
    let ga = goals.iter().find(|(id, _)| *id == a).unwrap().1;
    let gb = goals.iter().find(|(id, _)| *id == b).unwrap().1;
    assert!((ga.x - (100.0 - 20.0)).abs() < 0.1, "a goal {ga:?}");
    assert!((gb.x - (100.0 + 20.0)).abs() < 0.1, "b goal {gb:?}");
    assert!((ga.z - 50.0).abs() < 0.1);
    assert!((gb.z - 50.0).abs() < 0.1);
}

#[test]
fn infantry_group_move_uses_column_pack() {
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["INF_A", "INF_B", "INF_C", "INF_D"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Infantry);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    // Cluster near origin; move far +X so column residual engages.
    let ids: Vec<_> = ["INF_A", "INF_B", "INF_C", "INF_D"]
        .iter()
        .enumerate()
        .map(|(i, name)| {
            logic
                .create_object(
                    name,
                    Team::USA,
                    Vec3::new(i as f32 * 5.0, 0.0, (i as f32) * 2.0),
                )
                .unwrap()
        })
        .collect();
    for &id in &ids {
        logic.host_object_mut(id).unwrap().selection_radius = 10.0;
    }
    let click = Vec3::new(300.0, 0.0, 0.0);
    let exec = CommandExecutor::new(&mut logic, 0);
    let goals = exec.group_move_destinations(&ids, click);
    assert_eq!(goals.len(), 4);
    // Column pack: goals should not all share the same XZ (lateral spread).
    let zs: Vec<f32> = goals.iter().map(|(_, g)| g.z).collect();
    let z_span =
        zs.iter().cloned().fold(f32::MIN, f32::max) - zs.iter().cloned().fold(f32::MAX, f32::min);
    assert!(
        z_span > 5.0,
        "infantry column should lateral-spread goals, zs={zs:?}"
    );
    // And not collapse to free-move lead-only click for all.
    let unique_approx = {
        let mut xs: Vec<(i32, i32)> = goals
            .iter()
            .map(|(_, g)| ((g.x * 10.0) as i32, (g.z * 10.0) as i32))
            .collect();
        xs.sort();
        xs.dedup();
        xs.len()
    };
    assert!(
        unique_approx >= 3,
        "expected multiple distinct column goals, got {goals:?}"
    );
}

#[test]
fn guard_mode_without_pursuit_and_flying_only_are_stored() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, GuardTarget};
    use crate::game_logic::{GameLogic, GuardMode, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GM_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("GM_V".to_string(), tpl);
    let a = logic.create_object("GM_V", Team::USA, Vec3::ZERO).unwrap();
    let b = logic
        .create_object("GM_V", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_guard(
                &[a],
                &GuardTarget::Position(Vec3::new(40.0, 0.0, 0.0)),
                GuardMode::WithoutPursuit
            ),
            CommandResult::Success
        );
    }
    assert_eq!(
        logic.host_object(a).unwrap().guard_mode,
        GuardMode::WithoutPursuit
    );
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_guard(
                &[b],
                &GuardTarget::Position(Vec3::new(40.0, 0.0, 0.0)),
                GuardMode::FlyingUnitsOnly
            ),
            CommandResult::Success
        );
    }
    assert_eq!(
        logic.host_object(b).unwrap().guard_mode,
        GuardMode::FlyingUnitsOnly
    );
}

#[test]
fn command_button_maps_guard_modes() {
    use crate::command_system::{command_type_from_button_name, CommandType};
    use crate::game_logic::GuardMode;

    let g = command_type_from_button_name("Command_Guard").unwrap();
    assert!(matches!(
        g,
        CommandType::Guard {
            mode: GuardMode::Normal,
            ..
        }
    ));
    let w = command_type_from_button_name("Command_GuardWithoutPursuit").unwrap();
    assert!(matches!(
        w,
        CommandType::Guard {
            mode: GuardMode::WithoutPursuit,
            ..
        }
    ));
    let f = command_type_from_button_name("Command_GuardFlyingUnitsOnly").unwrap();
    assert!(matches!(
        f,
        CommandType::Guard {
            mode: GuardMode::FlyingUnitsOnly,
            ..
        }
    ));
}

#[test]
fn guard_uses_vision_radius_and_skips_structures() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, GuardTarget};
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for (name, kinds) in [
        ("GD_V", &[KindOf::Vehicle, KindOf::Selectable][..]),
        ("GD_S", &[KindOf::Structure, KindOf::Selectable][..]),
    ] {
        let mut tpl = ThingTemplate::new(name);
        for k in kinds {
            tpl.add_kind_of(*k);
        }
        tpl.set_health(500.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let v = logic
        .create_object("GD_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let s = logic
        .create_object("GD_S", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    {
        let u = logic.host_object_mut(v).unwrap();
        u.vision_range = 120.0;
        u.selection_radius = 10.0;
    }
    let mut exec = CommandExecutor::new(&mut logic, 0);
    let pos = Vec3::new(50.0, 0.0, 0.0);
    assert_eq!(
        exec.execute_guard(
            &[v, s],
            &GuardTarget::Position(pos),
            crate::game_logic::GuardMode::Normal
        ),
        CommandResult::Success
    );
    let u = logic.host_object(v).unwrap();
    assert!(
        (u.guard_radius - 120.0).abs() < 0.1,
        "guard radius should track vision, got {}",
        u.guard_radius
    );
    assert!(matches!(
        u.ai_state,
        AIState::GuardingArea | AIState::Moving
    ));
    // Structure must not enter guard.
    assert_ne!(
        logic.host_object(s).unwrap().ai_state,
        AIState::GuardingArea
    );
}

#[test]
fn patrol_enables_auto_acquire_hunt_residual() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("PT_A");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("PT_A".to_string(), tpl);
    let a = logic.create_object("PT_A", Team::USA, Vec3::ZERO).unwrap();
    logic.host_object_mut(a).unwrap().auto_acquire_when_idle = false;
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(exec.execute_patrol(&[a]), CommandResult::Success);
    let u = logic.host_object(a).unwrap();
    assert_eq!(u.ai_state, AIState::Patrolling);
    assert!(u.auto_acquire_when_idle);
}

#[test]
fn sell_selected_sells_friendly_structures_only() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    for name in ["SL_S", "SL_V"] {
        let mut tpl = ThingTemplate::new(name);
        if name == "SL_S" {
            tpl.add_kind_of(KindOf::Structure);
        } else {
            tpl.add_kind_of(KindOf::Vehicle);
        }
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(500.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let s = logic
        .create_object("SL_S", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let v = logic
        .create_object("SL_V", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_sell_selected(&[s, v], 0),
        CommandResult::Success
    );
    // Structure entered sell residual; vehicle rejected.
    assert!(
        logic.is_object_being_sold(s)
            || logic.host_object(s).map(|o| o.status.sold).unwrap_or(false)
    );
}

#[test]
fn tighten_paths_all_units_to_same_point() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("TZ_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("TZ_V".to_string(), tpl);
    let a = logic
        .create_object("TZ_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("TZ_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let dest = Vec3::new(20.0, 0.0, 0.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert!(exec.should_tighten_group_move(&[a, b], dest));
        assert_eq!(
            exec.execute_tighten_to_position(&[a, b], dest),
            CommandResult::Success
        );
    }
    // Both should target same destination (path last or target_position).
    for id in [a, b] {
        let u = logic.host_object(id).unwrap();
        let goal = u
            .movement
            .path
            .last()
            .copied()
            .or(u.movement.target_position);
        let g = goal.expect("should have path goal");
        assert!(
            (g.x - dest.x).abs() < 1.0 && (g.z - dest.z).abs() < 1.0,
            "unit {id:?} goal {g:?} != {dest:?}"
        );
    }
}

#[test]
fn override_special_power_destination_stores() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SP_O");
    tpl.add_kind_of(KindOf::Structure);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(500.0);
    logic.templates.insert("SP_O".to_string(), tpl);
    let id = logic.create_object("SP_O", Team::USA, Vec3::ZERO).unwrap();
    let loc = Vec3::new(100.0, 0.0, 50.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_override_special_power_destination(&[id], loc),
            CommandResult::Success
        );
    }
    let o = logic.host_object(id).unwrap();
    assert_eq!(o.special_power_override_destination, Some(loc));
}

#[test]
fn set_weapon_set_flag_carbomb_and_upgrade() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("WS_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("WS_V".to_string(), tpl);
    let id = logic.create_object("WS_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_weapon_set_flag(&[id], 2, true),
            CommandResult::Success
        );
        assert_eq!(
            exec.execute_set_weapon_set_flag(&[id], 0, true),
            CommandResult::Success
        );
    }
    let o = logic.host_object(id).unwrap();
    assert!(o.weapon_set_carbomb);
    assert!(o.weapon_set_player_upgrade);
}

#[test]
fn follow_waypoint_path_assigns_multi_point_path() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("WP_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("WP_V".to_string(), tpl);
    let id = logic.create_object("WP_V", Team::USA, Vec3::ZERO).unwrap();
    let wps = vec![
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(20.0, 0.0, 10.0),
        Vec3::new(30.0, 0.0, 0.0),
    ];
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_follow_waypoint_path(&[id], &wps, true, false),
            CommandResult::Success
        );
    }
    let o = logic.host_object(id).unwrap();
    assert!(
        !o.movement.path.is_empty() || o.movement.target_position.is_some(),
        "should have path or target"
    );
}

#[test]
fn attack_position_own_location_and_max_shots() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AP_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.add_kind_of(KindOf::Attackable);
    tpl.set_health(200.0);
    logic.templates.insert("AP_V".to_string(), tpl);
    let id = logic
        .create_object("AP_V", Team::USA, Vec3::new(5.0, 0.0, 7.0))
        .unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
});
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_ground(&[id], None, 3),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert_eq!(u.ai_state, AIState::AttackingGround);
    assert_eq!(u.max_shots_to_fire, 3);
    assert!(u.force_attack);
}

#[test]
fn exact_waypoint_path_sets_exact_flag_and_path() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("EX_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("EX_V".to_string(), tpl);
    let id = logic.create_object("EX_V", Team::USA, Vec3::ZERO).unwrap();
    let wps = vec![
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(20.0, 0.0, 5.0),
        Vec3::new(40.0, 0.0, 0.0),
    ];
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_follow_waypoint_path(&[id], &wps, true, false),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(u.is_exact_path, "exact follow must stamp is_exact_path");
    assert!(
        u.movement.path.len() >= 2,
        "exact path keeps waypoints: {:?}",
        u.movement.path
    );
    // Intermediate point should be present (exact, not collapsed).
    let has_mid = u
        .movement
        .path
        .iter()
        .any(|p| (p.x - 20.0).abs() < 1.0 && (p.z - 5.0).abs() < 1.0);
    assert!(
        has_mid,
        "exact path retains mid waypoint {:?}",
        u.movement.path
    );
}

#[test]
fn group_speed_ignores_really_damaged_and_picks_leader() {
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GS_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("GS_V".to_string(), tpl);
    let healthy = logic
        .create_object("GS_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let damaged = logic
        .create_object("GS_V", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    let slow_healthy = logic
        .create_object("GS_V", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    {
        let h = logic.host_object_mut(healthy).unwrap();
        h.movement.max_speed = 40.0;
        h.movement.max_speed_damaged = 20.0;
        h.health.current = 100.0;
        h.health.maximum = 100.0;
        h.refresh_model_condition_bits();
    }
    {
        let d = logic.host_object_mut(damaged).unwrap();
        d.movement.max_speed = 40.0;
        d.movement.max_speed_damaged = 5.0;
        d.health.current = 10.0; // REALLYDAMAGED
        d.health.maximum = 100.0;
        d.refresh_model_condition_bits();
    }
    {
        let s = logic.host_object_mut(slow_healthy).unwrap();
        s.movement.max_speed = 20.0;
        s.movement.max_speed_damaged = 10.0;
        s.health.current = 100.0;
        s.health.maximum = 100.0;
        s.refresh_model_condition_bits();
    }
    let exec = CommandExecutor::new(&mut logic, 0);
    let spd = exec.group_speed(&[healthy, damaged, slow_healthy]);
    assert!(
        (spd - 20.0).abs() < 0.01,
        "group speed should be slowest healthy (20), not crippled 5; got {spd}"
    );
    let leader = exec
        .group_leader_id(&[healthy, damaged, slow_healthy])
        .expect("leader");
    assert_eq!(leader, slow_healthy);
}

#[test]
fn effective_max_speed_uses_damaged_locomotor() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("ES_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("ES_V".to_string(), tpl);
    let id = logic.create_object("ES_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let o = logic.host_object_mut(id).unwrap();
        o.movement.max_speed = 30.0;
        o.movement.max_speed_damaged = 12.0;
        o.health.current = 100.0;
        o.health.maximum = 100.0;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::Pristine);
        assert!((o.effective_max_speed() - 30.0).abs() < 0.01);
        o.health.current = 10.0;
        o.refresh_model_condition_bits();
        assert_eq!(o.body_damage_state, HostBodyDamageType::ReallyDamaged);
        assert!(
            (o.effective_max_speed() - 12.0).abs() < 0.01,
            "really damaged uses max_speed_damaged"
        );
    }
}

#[test]
fn group_all_ids_and_attitude() {
    use super::CommandExecutor;
    use crate::game_logic::host_strategy_center::HostAiAttitude;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GID_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("GID_V".to_string(), tpl);
    let a = logic.create_object("GID_V", Team::USA, Vec3::ZERO).unwrap();
    let b = logic
        .create_object("GID_V", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    // Kill b
    logic.host_object_mut(b).unwrap().health.current = 0.0;
    let exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(exec.group_all_ids(&[a, b]), vec![a]);
    assert_eq!(exec.group_count(&[a, b]), 1);
    // C++ getAttitude always Passive.
    assert_eq!(exec.group_attitude(&[a]), HostAiAttitude::Passive);
}

#[test]
fn special_power_uses_single_source_object() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, PowerTarget, SpecialPowerType};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SP_SRC");
    tpl.add_kind_of(KindOf::Structure);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(500.0);
    logic.templates.insert("SP_SRC".to_string(), tpl);
    let caster = logic
        .create_object("SP_SRC", Team::USA, Vec3::ZERO)
        .unwrap();
    let other = logic
        .create_object("SP_SRC", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let c = logic.host_object_mut(caster).unwrap();
        c.special_power_cooldowns
            .insert(SpecialPowerType::SpySatellite, 0.0);
        c.special_power_ready = true;
    }
    {
        let o = logic.host_object_mut(other).unwrap();
        o.special_power_cooldowns.clear();
        o.special_power_ready = true;
    }
    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.special_power_source_object(&[other, caster], &SpecialPowerType::SpySatellite),
            Some(caster),
            "source must be the module owner even when other is first in selection"
        );
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        let res = exec.execute_special_power(
            &[other, caster],
            &SpecialPowerType::SpySatellite,
            &PowerTarget::Location(Vec3::new(100.0, 0.0, 100.0)),
        );
        let _ = res; // routing exercised; SharedSyncedTimer may mirror team-wide.
    }
    // Caster still owns the module entry after cast routing.
    assert!(logic
        .host_object(caster)
        .unwrap()
        .special_power_cooldowns
        .contains_key(&SpecialPowerType::SpySatellite));
}

#[test]
fn special_power_and_command_button_source_object() {
    use super::CommandExecutor;
    use crate::command_system::{CommandType, SpecialPowerType};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut move_t = ThingTemplate::new("SRC_M");
    move_t.add_kind_of(KindOf::Vehicle);
    move_t.add_kind_of(KindOf::Selectable);
    move_t.set_health(100.0);
    logic.templates.insert("SRC_M".to_string(), move_t);
    let mut atk_t = ThingTemplate::new("SRC_A");
    atk_t.add_kind_of(KindOf::Vehicle);
    atk_t.add_kind_of(KindOf::Selectable);
    atk_t.set_health(100.0);
    logic.templates.insert("SRC_A".to_string(), atk_t);
    let mover = logic.create_object("SRC_M", Team::USA, Vec3::ZERO).unwrap();
    let attacker = logic
        .create_object("SRC_A", Team::USA, Vec3::new(5.0, 0.0, 0.0))
        .unwrap();
    {
        let a = logic.host_object_mut(attacker).unwrap();
        a.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
});
        a.special_power_cooldowns
            .insert(SpecialPowerType::SpySatellite, 0.0);
    }
    // Ensure mover has no weapon / no SP map entry.
    {
        let m = logic.host_object_mut(mover).unwrap();
        m.weapon = None;
        m.special_power_cooldowns.clear();
    }
    let exec = CommandExecutor::new(&mut logic, 0);
    let sp = exec.special_power_source_object(&[mover, attacker], &SpecialPowerType::SpySatellite);
    assert_eq!(
        sp,
        Some(attacker),
        "SP source should be attacker with cooldown map; mover={mover:?} attacker={attacker:?} sp={sp:?}"
    );
    let src = exec.command_button_source_object(
        &[mover, attacker],
        &CommandType::AttackObject { target_id: mover },
    );
    assert_eq!(
        src,
        Some(attacker),
        "attack button source needs weapon; src={src:?}"
    );
    let move_src = exec.command_button_source_object(
        &[mover, attacker],
        &CommandType::Move {
            destination: Vec3::new(1.0, 0.0, 0.0),
        },
    );
    assert!(move_src.is_some());
}

#[test]
fn attack_move_sets_max_shots_and_path_flag() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AM_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("AM_V".to_string(), tpl);
    let id = logic.create_object("AM_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let o = logic.host_object_mut(id).unwrap();
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 150.0,
            ..Weapon::default()
});
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_move(&[id], Vec3::new(200.0, 0.0, 0.0), 5),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert_eq!(u.ai_state, AIState::AttackMoving);
    assert!(u.is_attack_path);
    assert_eq!(u.max_shots_to_fire, 5);
    assert!(u.auto_acquire_when_idle);
}

#[test]
fn group_geometry_and_formation_move() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GG_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("GG_V".to_string(), tpl);
    let a = logic
        .create_object("GG_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("GG_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.group_count(&[a, b]), 2);
        let (min, max, center) = exec.group_min_max_and_center(&[a, b]).unwrap();
        assert!((center.x - 20.0).abs() < 0.1);
        assert!((max.x - min.x - 40.0).abs() < 0.1);
        assert!(exec.group_speed(&[a, b]) >= 0.0);
        assert_eq!(
            exec.execute_create_formation(&[a, b]),
            CommandResult::Success
        );
        let dest = Vec3::new(300.0, 0.0, 0.0);
        assert!(exec.compute_ground_path_should_group(&[a, b], dest));
        assert_eq!(
            exec.execute_move_formation_to_position(&[a, b], dest),
            CommandResult::Success
        );
    }
    let ga = logic
        .host_object(a)
        .unwrap()
        .movement
        .path
        .last()
        .copied()
        .or(logic.host_object(a).unwrap().movement.target_position)
        .unwrap();
    let gb = logic
        .host_object(b)
        .unwrap()
        .movement
        .path
        .last()
        .copied()
        .or(logic.host_object(b).unwrap().movement.target_position)
        .unwrap();
    assert!(
        (ga.x - gb.x).abs() > 20.0,
        "formation move keeps offset ga={ga:?} gb={gb:?}"
    );
}

#[test]
fn follow_path_alias_paths_units() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("FP_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("FP_V".to_string(), tpl);
    let id = logic.create_object("FP_V", Team::USA, Vec3::ZERO).unwrap();
    let path = vec![Vec3::new(10.0, 0.0, 0.0), Vec3::new(50.0, 0.0, 0.0)];
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_follow_path(&[id], &path),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(!u.movement.path.is_empty() || u.movement.target_position.is_some());
}

#[test]
fn group_ownership_filter_and_center() {
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.add_player(Player::new(1, Team::GLA, "GLA", false));
    for (name, team) in [("OF_U", Team::USA), ("OF_E", Team::GLA)] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
        let _ = team;
    }
    let u = logic
        .create_object("OF_U", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let e = logic
        .create_object("OF_E", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let exec = CommandExecutor::new(&mut logic, 0);
    assert!(exec.is_member(&[u, e], u));
    assert!(!exec.is_member(&[u], e));
    assert!(exec.contains_any_objects_not_owned_by_player(&[u, e], 0));
    let (kept, empty) = exec.remove_any_objects_not_owned_by_player(&[u, e], 0);
    assert_eq!(kept, vec![u]);
    assert!(!empty);
    let c = exec.group_center(&[u, e]).unwrap();
    assert!((c.x - 20.0).abs() < 0.1, "center x={}", c.x);
}

#[test]
fn special_power_at_location_wrapper() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, SpecialPowerType};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SP_L");
    tpl.add_kind_of(KindOf::Structure);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(500.0);
    logic.templates.insert("SP_L".to_string(), tpl);
    let id = logic.create_object("SP_L", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        // May succeed or invalid depending on power readiness residual — must not panic.
        let _ = exec.execute_special_power_at_location(
            &[id],
            &SpecialPowerType::SpySatellite,
            Vec3::new(100.0, 0.0, 50.0),
        );
        let _ = exec.execute_special_power_at_object(&[id], &SpecialPowerType::SpySatellite, id);
    }
}

#[test]
fn guard_area_stamps_radius() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, GuardMode, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GA_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("GA_V".to_string(), tpl);
    let id = logic.create_object("GA_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_guard_area(&[id], Vec3::new(30.0, 0.0, 0.0), 150.0, GuardMode::Normal),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!((u.guard_radius - 150.0).abs() < 0.1, "r={}", u.guard_radius);
}

#[test]
fn group_idle_busy_dead_queries() {
    use super::CommandExecutor;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GQ_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("GQ_V".to_string(), tpl);
    let a = logic.create_object("GQ_V", Team::USA, Vec3::ZERO).unwrap();
    let b = logic
        .create_object("GQ_V", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert!(exec.group_is_idle(&[a, b]));
        assert!(!exec.group_is_busy(&[a, b]));
        assert!(!exec.group_is_ai_dead(&[a, b]));
    }
    logic
        .host_object_mut(a)
        .unwrap()
        .set_ai_state(AIState::Moving);
    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert!(!exec.group_is_idle(&[a, b]));
        // busy requires ALL living busy
        assert!(!exec.group_is_busy(&[a, b]));
    }
    logic
        .host_object_mut(b)
        .unwrap()
        .set_ai_state(AIState::Moving);
    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert!(exec.group_is_busy(&[a, b]));
    }
    logic.host_object_mut(a).unwrap().health.current = 0.0;
    logic.host_object_mut(b).unwrap().health.current = 0.0;
    // mark dead properly if needed
    for id in [a, b] {
        if let Some(o) = logic.host_object_mut(id) {
            o.status.destroyed = true;
        }
    }
    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert!(exec.group_is_ai_dead(&[a, b]));
    }
}

#[test]
fn attack_follow_waypoint_sets_attack_path() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AF_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.add_kind_of(KindOf::Attackable);
    tpl.set_health(200.0);
    logic.templates.insert("AF_V".to_string(), tpl);
    let id = logic.create_object("AF_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.weapon = Some(Weapon {
            damage: 10.0,
            range: 150.0,
            ..Weapon::default()
});
    }
    let wps = vec![Vec3::new(20.0, 0.0, 0.0), Vec3::new(60.0, 0.0, 0.0)];
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_follow_waypoint_path(&[id], &wps, true, false),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(u.is_attack_path, "attack-follow should mark attack path");
    assert!(
        matches!(u.ai_state, AIState::AttackMoving | AIState::Moving),
        "state={:?}",
        u.ai_state
    );
}

#[test]
fn attack_move_sets_is_attack_path_flag() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AM_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.add_kind_of(KindOf::Attackable);
    tpl.set_health(200.0);
    logic.templates.insert("AM_V".to_string(), tpl);
    let id = logic.create_object("AM_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.weapon = Some(Weapon {
            damage: 10.0,
            range: 150.0,
            ..Weapon::default()
});
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_move(&[id], Vec3::new(90.0, 0.0, 0.0), -1),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(u.is_attack_path);
    assert_eq!(u.ai_state, AIState::AttackMoving);
}

#[test]
fn follow_waypoint_as_team_preserves_offsets() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("FT_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("FT_V".to_string(), tpl);
    let a = logic
        .create_object("FT_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("FT_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    // Stamp formation.
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_create_formation(&[a, b]),
            CommandResult::Success
        );
    }
    let wps = vec![Vec3::new(100.0, 0.0, 0.0), Vec3::new(200.0, 0.0, 0.0)];
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_follow_waypoint_path(&[a, b], &wps, true, true),
            CommandResult::Success
        );
    }
    let ga = logic
        .host_object(a)
        .unwrap()
        .movement
        .path
        .last()
        .copied()
        .or(logic.host_object(a).unwrap().movement.target_position)
        .unwrap();
    let gb = logic
        .host_object(b)
        .unwrap()
        .movement
        .path
        .last()
        .copied()
        .or(logic.host_object(b).unwrap().movement.target_position)
        .unwrap();
    // Offsets should keep ~40 world units separation on X (formation).
    let sep = (ga.x - gb.x).abs();
    assert!(
        sep > 20.0,
        "as-team should preserve formation separation, ga={ga:?} gb={gb:?} sep={sep}"
    );
    // Formation id preserved.
    assert_eq!(
        logic.host_object(a).unwrap().formation_id,
        logic.host_object(b).unwrap().formation_id
    );
    assert_ne!(logic.host_object(a).unwrap().formation_id, 0);
}

#[test]
fn do_command_button_using_waypoints_attack_moves() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("BW_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("BW_V".to_string(), tpl);
    let id = logic.create_object("BW_V", Team::USA, Vec3::ZERO).unwrap();
    let wps = vec![Vec3::new(10.0, 0.0, 0.0), Vec3::new(80.0, 0.0, 0.0)];
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_do_command_button_using_waypoints(&[id], "Command_AttackMove", &wps),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(
        !u.movement.path.is_empty() || u.movement.target_position.is_some(),
        "should path along waypoints"
    );
}

#[test]
fn do_command_button_dispatches_stop() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("DC_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("DC_V".to_string(), tpl);
    let id = logic.create_object("DC_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.set_ai_state(AIState::Moving);
        u.set_target(Some(id));
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_do_command_button(&[id], "Command_Stop", None, None),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(
        matches!(u.ai_state, AIState::Idle) || u.target.is_none() || !u.status.moving,
        "stop should clear action state={:?} target={:?}",
        u.ai_state,
        u.target
    );
}

#[test]
fn do_command_button_at_position_moves() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("DC_M");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("DC_M".to_string(), tpl);
    let id = logic.create_object("DC_M", Team::USA, Vec3::ZERO).unwrap();
    let dest = Vec3::new(55.0, 0.0, 10.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_do_command_button(&[id], "Command_AttackMove", Some(dest), None),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert!(
        !u.movement.path.is_empty() || u.movement.target_position.is_some(),
        "attack-move should path"
    );
}

#[test]
fn surrender_stops_and_flags_unit() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SR_I");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("SR_I".to_string(), tpl);
    let id = logic.create_object("SR_I", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_surrender(&[id], true), CommandResult::Success);
    }
    let o = logic.host_object(id).unwrap();
    assert!(o.is_surrendered);
    assert!(o.target.is_none());
}

#[test]
fn attack_team_engages_member_of_team() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for (name, _t) in [("AT_U", Team::USA), ("AT_E", Team::GLA)] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let u = logic.create_object("AT_U", Team::USA, Vec3::ZERO).unwrap();
    let e = logic
        .create_object("AT_E", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    {
        use crate::game_logic::Weapon;
        let uo = logic.host_object_mut(u).unwrap();
        uo.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
});
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        // team_code 0 = GLA
        assert_eq!(
            exec.execute_attack_team(&[u], 0, -1),
            CommandResult::Success
        );
    }
    let unit = logic.host_object(u).unwrap();
    assert!(
        unit.target == Some(e)
            || matches!(
                unit.ai_state,
                AIState::Attacking | AIState::AttackMoving | AIState::Moving
            ),
        "target={:?} state={:?}",
        unit.target,
        unit.ai_state
    );
    assert_eq!(unit.max_shots_to_fire, -1);
    assert_eq!(
        unit.attack_priority_set.as_deref(),
        Some("AIGroup.AttackTeam.GLA")
    );
}

#[test]
fn weapon_lock_forces_slot_and_release() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon, WeaponLockType};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("WL_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("WL_V".to_string(), tpl);
    let id = logic.create_object("WL_V", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
});
        u.secondary_weapon = Some(Weapon {
            damage: 5.0,
            range: 80.0,
            ..Weapon::default()
});
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_weapon_lock(&[id], 1, 2),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert_eq!(u.weapon_lock_type, WeaponLockType::LockedPermanently);
    assert_eq!(u.weapon_lock_slot, 1);
    assert_eq!(u.active_weapon_slot, 1);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_release_weapon_lock(&[id], 2),
            CommandResult::Success
        );
    }
    assert_eq!(
        logic.host_object(id).unwrap().weapon_lock_type,
        WeaponLockType::NotLocked
    );
}

#[test]
fn do_weapon_locks_the_requested_secondary_slot_before_targeting() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, WeaponSlot, WeaponTarget};
    use crate::game_logic::{
        AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon, WeaponLockType,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("DW_SECONDARY");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("DW_SECONDARY".to_string(), tpl);
    let unit_id = logic
        .create_object("DW_SECONDARY", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit_id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
});
        unit.secondary_weapon = Some(Weapon {
            damage: 50.0,
            range: 200.0,
            ..Weapon::default()
});
    }

    let target = Vec3::new(75.0, 0.0, 25.0);
    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[unit_id],
        &WeaponSlot::Secondary,
        1,
        &WeaponTarget::Location(target),
    );
    assert_eq!(result, CommandResult::Success);

    let unit = logic.host_object(unit_id).expect("unit");
    assert_eq!(unit.active_weapon_slot, 1);
    assert_eq!(unit.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(unit.weapon_lock_slot, 1);
    assert_eq!(unit.target_location, Some(target));
    assert_eq!(unit.ai_state, AIState::AttackingGround);
    assert_eq!(unit.max_shots_to_fire, 1);
}

#[test]
fn do_weapon_uses_the_real_tertiary_slot_without_secondary_aliasing() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, WeaponSlot, WeaponTarget};
    use crate::game_logic::{
        GameLogic, KindOf, ObjectId, Team, ThingTemplate, Weapon, WeaponLockType,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("DW_TERTIARY");
    template.add_kind_of(KindOf::Vehicle);
    template.add_kind_of(KindOf::Selectable);
    template.set_health(200.0);
    logic.templates.insert("DW_TERTIARY".to_string(), template);
    let unit_id = logic
        .create_object("DW_TERTIARY", Team::USA, Vec3::ZERO)
        .expect("unit");
    {
        let unit = logic.host_object_mut(unit_id).expect("unit");
        unit.weapon = Some(Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
});
        unit.secondary_weapon = Some(Weapon {
            damage: 20.0,
            range: 100.0,
            ..Weapon::default()
});
        unit.tertiary_weapon = Some(Weapon {
            damage: 73.0,
            range: 300.0,
            last_fire_time: -10.0,
            ..Weapon::default()
});
    }

    let target = ObjectId(999);
    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[unit_id],
        &WeaponSlot::Tertiary,
        -1,
        &WeaponTarget::Object(target),
    );
    assert_eq!(result, CommandResult::Success);

    let unit = logic.host_object_mut(unit_id).expect("unit");
    assert_eq!(unit.active_weapon_slot, 2);
    assert_eq!(unit.weapon_lock_type, WeaponLockType::LockedTemporarily);
    assert_eq!(unit.weapon_lock_slot, 2);
    assert!(unit.fire_at(target, 1.0));
    assert_eq!(unit.last_fire_slot, 2);
    assert!((unit.last_fire_damage - 73.0).abs() < f32::EPSILON);
    assert_eq!(
        unit.secondary_weapon
            .as_ref()
            .map(|weapon| weapon.last_fire_time),
        Some(0.0),
        "the secondary weapon must remain untouched"
    );
}

#[test]
fn do_weapon_rejects_unrepresented_slots_without_primary_fallback() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, WeaponSlot, WeaponTarget};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon, WeaponLockType};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("DW_PRIMARY_ONLY");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("DW_PRIMARY_ONLY".to_string(), tpl);
    let unit_id = logic
        .create_object("DW_PRIMARY_ONLY", Team::USA, Vec3::ZERO)
        .expect("unit");
    logic.host_object_mut(unit_id).expect("unit").weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        ..Weapon::default()
});

    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[unit_id],
        &WeaponSlot::Tertiary,
        -1,
        &WeaponTarget::Location(Vec3::new(75.0, 0.0, 25.0)),
    );
    assert_eq!(result, CommandResult::InvalidCommand);

    let unit = logic.host_object(unit_id).expect("unit");
    assert_eq!(unit.active_weapon_slot, 0);
    assert_eq!(unit.weapon_lock_type, WeaponLockType::NotLocked);
    assert_eq!(unit.target_location, None);

    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[unit_id],
        &WeaponSlot::Slot(u32::MAX),
        -1,
        &WeaponTarget::Location(Vec3::new(80.0, 0.0, 30.0)),
    );
    assert_eq!(result, CommandResult::InvalidCommand);
    assert_eq!(
        logic.host_object(unit_id).expect("unit").target_location,
        None
    );
}

#[test]
fn set_emoticon_stores_name_and_duration() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("EM_U");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("EM_U".to_string(), tpl);
    let id = logic.create_object("EM_U", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_emoticon(&[id], "Emoticon_Alert", 60),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert_eq!(u.emoticon_name, "Emoticon_Alert");
    assert_eq!(u.emoticon_frames_left, 60);
}

#[test]
fn mine_clearing_detail_toggles_weapon_set_flag() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("MC_D");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(300.0);
    logic.templates.insert("MC_D".to_string(), tpl);
    let d = logic.create_object("MC_D", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_mine_clearing_detail(&[d], true),
            CommandResult::Success
        );
    }
    assert!(
        logic
            .host_object(d)
            .unwrap()
            .weapon_set_mine_clearing_detail
    );
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_mine_clearing_detail(&[d], false),
            CommandResult::Success
        );
    }
    assert!(
        !logic
            .host_object(d)
            .unwrap()
            .weapon_set_mine_clearing_detail
    );
}

#[test]
fn ordinary_do_weapon_uses_authored_mine_detail_and_disarms_only_when_armed() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, WeaponSlot, WeaponTarget};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut clearer = ThingTemplate::new("AuthoredMineDetailClearer");
    clearer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0)
        .set_primary_weapon_none()
        .set_mine_clearing_primary_weapon_name("DozerMineDisarmingWeapon");
    logic
        .templates
        .insert("AuthoredMineDetailClearer".to_string(), clearer);

    let clearer_id = logic
        .create_object("AuthoredMineDetailClearer", Team::USA, Vec3::ZERO)
        .expect("typed clearer");
    let mine_id = logic
        .place_land_mine(Team::GLA, Vec3::new(2.0, 0.0, 0.0), None)
        .expect("enemy mine");
    // The normal creation path starts a weapon's reload clock at frame zero.
    // Start this focused combat assertion with the authored detail weapon
    // ready, just as the other direct `update_combat` tests do.
    logic
        .host_object_mut(clearer_id)
        .and_then(|clearer| clearer.mine_clearing_primary_weapon.as_mut())
        .expect("authored mine detail weapon")
        .last_fire_time = -10.0;

    // The actual retail FIRE_WEAPON does not become usable merely because the
    // object owns a mine row.  The parsed option arms the persistent detail
    // bit first; an untyped/no-option command must fail closed here.
    let result = CommandExecutor::new(&mut logic, 0).execute_weapon(
        &[clearer_id],
        &WeaponSlot::Primary,
        -1,
        &WeaponTarget::Location(Vec3::new(2.0, 0.0, 0.0)),
    );
    assert_eq!(result, CommandResult::InvalidCommand);
    assert!(
        logic.host_object(mine_id)
            .and_then(|mine| mine.mine_data.as_ref())
            .is_some_and(|data| data.is_active()),
        "an unarmed or untyped primary must not clear the mine"
    );

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_set_mine_clearing_detail(&[clearer_id], true),
            CommandResult::Success
        );
        assert_eq!(
            exec.execute_weapon(
                &[clearer_id],
                &WeaponSlot::Primary,
                -1,
                &WeaponTarget::Location(Vec3::new(2.0, 0.0, 0.0)),
            ),
            CommandResult::Success
        );
    }
    let clearer = logic.host_object(clearer_id).expect("typed clearer");
    assert!(clearer.weapon_set_mine_clearing_detail);
    assert_eq!(
        clearer.weapon_name_for_slot(0),
        Some("DozerMineDisarmingWeapon"),
        "ordinary DoWeapon must select the authored conditional source name"
    );
    assert_eq!(clearer.target_location, Some(Vec3::new(2.0, 0.0, 0.0)));

    // The normal combat loop owns ground impact and DAMAGE_DISARM; no
    // synthetic ClearMines executor path participates in this assertion.
    logic.update_combat(&[clearer_id, mine_id], 1.0 / 30.0);
    assert!(
        logic.host_object(mine_id)
            .map(|mine| mine.status.destroyed)
            .unwrap_or(true),
        "the ordinary location weapon path must apply DAMAGE_DISARM to the mine"
    );
}

#[test]
fn go_prone_sets_prone_timer_and_bit() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{
        host_enum_table_residual::model_condition_bit_name_index, GameLogic, KindOf, Team,
        ThingTemplate,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GP_I");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("GP_I".to_string(), tpl);
    let i = logic.create_object("GP_I", Team::USA, Vec3::ZERO).unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_go_prone(&[i]), CommandResult::Success);
    }
    let u = logic.host_object(i).unwrap();
    assert!(u.prone_timer > 0.0);
    if let Some(bit) = model_condition_bit_name_index("PRONE") {
        assert_ne!(u.model_condition_bits & (1u128 << bit), 0);
    }
}

#[test]
fn attack_area_engages_enemy_inside_radius() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for (name, team) in [("AA_U", Team::USA), ("AA_E", Team::GLA)] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
        let _ = team;
    }
    let u = logic.create_object("AA_U", Team::USA, Vec3::ZERO).unwrap();
    let e = logic
        .create_object("AA_E", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_area(&[u], Vec3::new(40.0, 0.0, 0.0), 80.0, None),
            CommandResult::Success
        );
    }
    let unit = logic.host_object(u).unwrap();
    assert!(
        unit.attack_priority_set
            .as_deref()
            .is_some_and(|t| t.starts_with("AIGroup.AttackArea.")),
        "AttackArea must persist AIAttackAreaState tag, got {:?}",
        unit.attack_priority_set
    );
    assert!(
        unit.target == Some(e)
            || matches!(
                unit.ai_state,
                AIState::Attacking | AIState::AttackMoving | AIState::Moving
            ),
        "attack area should engage or path, target={:?} state={:?}",
        unit.target,
        unit.ai_state
    );
}

#[test]
fn move_to_and_evacuate_sets_pending_flag() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("EV_T");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(500.0);
    // transport capacity residual
    logic.templates.insert("EV_T".to_string(), tpl);
    let mut pax_tpl = ThingTemplate::new("EV_P");
    pax_tpl.add_kind_of(KindOf::Infantry);
    pax_tpl.add_kind_of(KindOf::Selectable);
    pax_tpl.set_health(100.0);
    logic.templates.insert("EV_P".to_string(), pax_tpl);

    let transport = logic.create_object("EV_T", Team::USA, Vec3::ZERO).unwrap();
    let pax = logic
        .create_object("EV_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        // Force containable capacity
        let _ = t.add_occupant(pax);
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
        p.set_ai_state(crate::game_logic::AIState::Docked);
    }
    assert!(!logic
        .host_object(transport)
        .unwrap()
        .contained_units()
        .is_empty());

    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_move_to_and_evacuate(&[transport], Vec3::new(80.0, 0.0, 0.0), false),
            CommandResult::Success
        );
    }
    let t = logic.host_object(transport).unwrap();
    assert!(
        t.pending_evacuate_on_stop,
        "should pending evacuate after move command"
    );
    assert!(!t.pending_exit_after_evacuate);
}

#[test]
fn move_to_and_evacuate_unloads_when_path_completes() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for (name, kind) in [("EV2_T", KindOf::Vehicle), ("EV2_P", KindOf::Infantry)] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(kind);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(400.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let transport = logic.create_object("EV2_T", Team::USA, Vec3::ZERO).unwrap();
    let pax = logic
        .create_object("EV2_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        assert!(t.add_occupant(pax));
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
        p.set_ai_state(crate::game_logic::AIState::Docked);
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_move_to_and_evacuate(&[transport], Vec3::new(10.0, 0.0, 0.0), false),
            CommandResult::Success
        );
    }
    // Simulate arrival: complete path + movement tick.
    if let Some(t) = logic.host_object_mut(transport) {
        // Snap to end of path and finish
        if let Some(last) = t.movement.path.last().copied() {
            t.set_position(last);
        } else {
            t.set_position(Vec3::new(10.0, 0.0, 0.0));
        }
        t.movement.current_path_index = t.movement.path.len().saturating_sub(0);
        // Force index past end
        t.movement.current_path_index = t.movement.path.len();
        t.pending_evacuate_on_stop = true;
    }
    // Direct evacuate_now residual (arrival hook)
    assert!(logic.evacuate_container_now(transport, false));
    assert!(
        logic
            .host_object(transport)
            .map(|t| t.contained_units().is_empty())
            .unwrap_or(false),
        "passengers should unload"
    );
    let p = logic.host_object(pax).unwrap();
    assert!(p.contained_by.is_none());
    assert_ne!(p.ai_state, crate::game_logic::AIState::Docked);
}

#[test]
fn evacuate_requires_container_not_passenger_only() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("EV_INF");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("EV_INF".to_string(), tpl);
    let a = logic
        .create_object("EV_INF", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    // C++ groupEvacuate no-ops on non-containers without AI contain.
    assert_eq!(exec.execute_evacuate(&[a]), CommandResult::InvalidCommand);
}

#[test]
fn attack_move_uses_assign_unit_path() {
    // C++ AIGroup::groupAttackMoveToPosition (AIGroup.cpp:2260-2273): one pos.
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    let i = prod.find("fn execute_attack_move").expect("attack_move");
    let rest = &prod[i..];
    let end = rest.find("fn execute_force_move").unwrap_or(1500.min(rest.len()));
    let w = &rest[..end];
    assert!(
        w.contains("unit_command_attack_move_to_ex")
            && !w.contains("group_move_destinations")
            && !w.contains("set_destination(goal)"),
        "attack-move must send every member to the identical pos"
    );
    let j = prod.find("fn execute_force_move").expect("force_move");
    let w2 = &prod[j..prod.len().min(j + 1200)];
    assert!(
        w2.contains("group_move_destinations")
            && w2.contains("unit_command_force_move_to")
            && !w2.contains("set_destination(goal)"),
        "force-move must pathfind like Move via unit_command_force_move_to"
    );
}

#[test]
fn path_to_goal_with_state_used_by_guard_scatter_gather() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    assert!(prod.contains("fn path_to_goal_with_state"));
    for name in [
        "fn execute_guard",
        "fn execute_scatter",
        "fn execute_gather",
        "fn execute_build",
    ] {
        let i = prod.find(name).unwrap_or_else(|| panic!("missing {name}"));
        let w = &prod[i..prod.len().min(i + 6000)];
        assert!(
            w.contains("path_to_goal_with_state") || w.contains("assign_unit_path"),
            "{name} must pathfind, not bare set_destination"
        );
        // Guard/scatter/gather should not use bare set_destination(goal)
        if name != "fn execute_build" {
            assert!(
                !w.contains("set_destination(*pos)")
                    && !w.contains("set_destination(pos)")
                    && !w.contains("set_destination(dest)")
                    && !w.contains("set_destination(target_pos)"),
                "{name} still has bare set_destination"
            );
        }
    }
}

/// C++ BuildAssistant.cpp:333-334 clearRemovableForConstruction before create.
#[test]
fn execute_build_source_clears_trees_and_props() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = src.find("fn execute_build").expect("execute_build");
    let w = &src[i..src.len().min(i + 6000)];
    assert!(
        w.contains("clear_removable_for_construction")
            && src.contains("remove_trees_and_props_for_construction"),
        "hq-wtzcx: execute_build must clear removable objects and map trees/props"
    );
}

#[test]
fn interaction_commands_pathfind_surface() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    // Production locomotion commands should prefer path_to_goal_with_state.
    assert!(prod.matches("path_to_goal_with_state").count() >= 10);
    // Bare set_destination should not remain in execute_* interaction paths.
    let exec = prod;
    let bare = exec.matches("unit.set_destination(").count()
        + exec.matches("unit_mut.set_destination(").count();
    assert_eq!(
        bare, 0,
        "production execute paths still call unit.set_destination ({bare})"
    );
}

#[test]
fn deploy_style_toggle_residual() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let start = src.find("fn execute_deploy").expect("execute_deploy");
    let body = &src[start..start + 2500];
    assert!(
        body.contains("deploy_style_metadata") && body.contains("unit_command_toggle_deploy_style"),
        "DeployStyle authorization must use exact authored module metadata"
    );
    assert!(
        !body.contains("looks_deployable")
            && !body.contains("tomahawk")
            && !body.contains("nukecannon"),
        "DeployStyle authorization must not fall back to vehicle basenames"
    );
}

#[test]
fn deploy_command_uses_authored_metadata_and_pack_unpack_timing() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{DeployStyleMetadata, GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;
    use std::time::UNIX_EPOCH;

    fn deploy_command(id: u32, selected_units: Vec<crate::game_logic::ObjectId>) -> GameCommand {
        GameCommand {
            command_type: CommandType::Deploy,
            player_id: 0,
            command_id: id,
            timestamp: UNIX_EPOCH,
            selected_units,
            modifier_keys: ModifierKeys::default(),
        }
    }

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "source-backed deploy", true));

    let mut authored = ThingTemplate::new("ArbitraryDeployStyleVehicle");
    authored
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(300.0);
    authored.deploy_style_metadata = Some(DeployStyleMetadata {
        pack_time_frames: 3,
        unpack_time_frames: 3,
        ..Default::default()
    });
    logic
        .templates
        .insert("ArbitraryDeployStyleVehicle".to_string(), authored);

    let mut ordinary = ThingTemplate::new("PlainVehicleWithoutBehavior");
    ordinary
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(300.0);
    logic
        .templates
        .insert("PlainVehicleWithoutBehavior".to_string(), ordinary);

    let deployable = logic
        .create_object_for_player("ArbitraryDeployStyleVehicle", 0, Vec3::ZERO)
        .expect("source-backed DeployStyle unit");
    let no_behavior = logic
        .create_object_for_player("PlainVehicleWithoutBehavior", 0, Vec3::new(50.0, 0.0, 0.0))
        .expect("ordinary vehicle");

    // Full player command -> executor -> authoritative GameLogic transition.
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor
                .execute_command(deploy_command(1, vec![deployable]))
                .expect("deploy command result"),
            CommandResult::Success
        );
    }
    let first = logic.host_object(deployable).expect("deploying unit");
    assert!(
        first
            .deploy_style
            .as_ref()
            .is_some_and(|style| style.is_busy()),
        "the command starts an authored unpack timer rather than immediately setting deployed"
    );
    assert!(!first.is_deployed());

    logic.frame = 2;
    logic.tick_deploy_style_updates();
    assert!(
        !logic.host_object(deployable).unwrap().is_deployed(),
        "unpack must still be pending before its authored frame boundary"
    );
    logic.frame = 3;
    logic.tick_deploy_style_updates();
    let unpacked = logic.host_object(deployable).expect("unpacked unit");
    assert!(unpacked.is_deployed());
    assert!(unpacked
        .deploy_style
        .as_ref()
        .is_some_and(|style| style.is_ready_to_attack()));

    // The next explicit Deploy reverses direction: deployed status clears at
    // pack start and movement becomes available only on the authored boundary.
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor
                .execute_command(deploy_command(2, vec![deployable]))
                .expect("undeploy command result"),
            CommandResult::Success
        );
    }
    assert!(!logic.host_object(deployable).unwrap().is_deployed());
    logic.frame = 5;
    logic.tick_deploy_style_updates();
    assert!(
        !logic
            .host_object(deployable)
            .unwrap()
            .deploy_style_allows_move(),
        "pack remains active through frame 5"
    );
    logic.frame = 6;
    logic.tick_deploy_style_updates();
    assert!(logic
        .host_object(deployable)
        .unwrap()
        .deploy_style_allows_move());

    // A plain vehicle must not inherit DeployStyle behavior because a name or
    // VEHICLE KindOf happened to resemble an older residual list.
    let mut executor = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        executor
            .execute_command(deploy_command(3, vec![no_behavior]))
            .expect("ordinary deploy command result"),
        CommandResult::InvalidCommand
    );
    drop(executor);
    assert!(logic
        .host_object(no_behavior)
        .unwrap()
        .deploy_style
        .is_none());
}

#[test]
fn execute_stop_clears_guard_residual() {
    // Wave 955: Stop delegates guard clear to GameLogic::unit_command_stop.
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let gl = crate::game_logic::game_logic::GAME_LOGIC_FACADE_SRC;
    let start = src.find("fn execute_stop").expect("execute_stop");
    let body = &src[start..start + 800];
    assert!(
        body.contains("unit_command_stop") && body.contains("apply_player_stealth_mood_delay"),
        "Stop must call unit_command_stop and apply stealth mood delay"
    );
    let gs = gl.find("fn unit_command_stop").expect("unit_command_stop");
    let gbody = &gl[gs..gs + 900];
    assert!(
        gbody.contains("set_guard_position(None)")
            && gbody.contains("end_guard_retaliate")
            && gbody.contains("set_target(None)"),
        "unit_command_stop must clear guard anchors/targets"
    );
    assert!(
        src.contains("fn apply_player_stealth_mood_delay") && src.contains("next_mood_check_time"),
        "shared stealth mood delay helper must schedule next_mood_check_time"
    );
}

#[test]
fn stop_delays_mood_for_unstealthed_stealth_unit() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.set_current_frame(100);
    let mut tpl = ThingTemplate::new("ST_A");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("ST_A".to_string(), tpl);
    let a = logic.create_object("ST_A", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(a).unwrap();
        u.innate_stealth = true;
        u.stealth_delay_frames = 45;
        u.auto_acquire_when_idle = true;
        u.status.stealthed = false;
        u.status.detected = false;
        u.next_mood_check_time = 0;
        u.set_ai_state(AIState::Moving);
    }
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(exec.execute_stop(&[a]), CommandResult::Success);
    let u = logic.host_object(a).unwrap();
    assert_eq!(u.ai_state, AIState::Idle);
    // now=100 + delay 45 + skew 0
    assert_eq!(
        u.next_mood_check_time, 145,
        "player stop should delay mood until stealth window"
    );
}
#[test]
fn add_waypoint_uses_group_destinations() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["WP_A", "WP_B"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("WP_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("WP_B", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    for id in [a, b] {
        logic.host_object_mut(id).unwrap().selection_radius = 10.0;
    }
    let click = Vec3::new(100.0, 0.0, 50.0);
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a, b], click),
        CommandResult::Success
    );
    // Paths should not be identical stacked goals for multi-select.
    let pa = logic.host_object(a).unwrap().movement.path.clone();
    let pb = logic.host_object(b).unwrap().movement.path.clone();
    assert!(!pa.is_empty() && !pb.is_empty());
    let ga = *pa.last().unwrap();
    let gb = *pb.last().unwrap();
    assert!(
        (ga - gb).length() > 5.0,
        "waypoint goals must spread like group move, ga={ga:?} gb={gb:?}"
    );
}

#[test]
fn add_waypoint_skips_immobile_and_still_succeeds() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut mobile = ThingTemplate::new("WP_MOB");
    mobile
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("WP_MOB".into(), mobile);
    let mut stuck = ThingTemplate::new("WP_STUCK");
    stuck
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Immobile)
        .set_health(100.0);
    logic.templates.insert("WP_STUCK".into(), stuck);
    let a = logic
        .create_object("WP_MOB", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("WP_STUCK", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let click = Vec3::new(80.0, 0.0, 0.0);
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a, b], click),
        CommandResult::Success
    );
    assert!(
        !logic.host_object(a).unwrap().movement.path.is_empty(),
        "mobile member must still receive the waypoint"
    );
    assert!(
        logic.host_object(b).unwrap().movement.path.is_empty(),
        "immobile member is skipped, not a group abort"
    );
}

#[test]
fn add_waypoint_clamps_to_map_extent() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("WP_CLAMP");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("WP_CLAMP".into(), tpl);
    let a = logic
        .create_object("WP_CLAMP", Team::USA, Vec3::ZERO)
        .unwrap();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a], Vec3::new(400.0, 0.0, 0.0)),
        CommandResult::Success
    );
    let goal = *logic
        .host_object(a)
        .unwrap()
        .movement
        .path
        .last()
        .expect("clamped waypoint");
    let (_min, max) = logic.world_bounds();
    let margin = 4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
    assert!(
        goal.x <= max.x - margin + 0.1,
        "AddWaypoint must clampWaypointPosition, goal={goal:?} max={max:?}"
    );
}

#[test]
fn add_waypoint_plays_voice_move_when_idle() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::audio_dispatch_impl::{
        clear_test_template_voices, set_test_template_voice, UnitVoiceSlot,
    };
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    clear_test_template_voices();
    set_test_template_voice("WP_VOICE", UnitVoiceSlot::Move, "TestWaypointVoiceMove");
    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("WP_VOICE");
    tpl.add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("WP_VOICE".into(), tpl);
    let a = logic
        .create_object("WP_VOICE", Team::USA, Vec3::ZERO)
        .unwrap();
    logic.queued_audio_events.clear();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a], Vec3::new(40.0, 0.0, 0.0)),
        CommandResult::Success
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .any(|e| e.event_type == "TestWaypointVoiceMove"),
        "idle AddWaypoint must play VoiceMove: {:?}",
        logic
            .queued_audio_events
            .iter()
            .map(|e| e.event_type.clone())
            .collect::<Vec<_>>()
    );
    logic.queued_audio_events.clear();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a], Vec3::new(80.0, 0.0, 0.0)),
        CommandResult::Success
    );
    assert!(
        logic
            .queued_audio_events
            .iter()
            .all(|e| e.event_type != "TestWaypointVoiceMove"),
        "already-moving AddWaypoint must not replay VoiceMove"
    );
    clear_test_template_voices();
}

#[test]
fn add_waypoint_breaks_off_attack() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["WP_ATK", "WP_VIC"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("WP_ATK", Team::USA, Vec3::ZERO)
        .unwrap();
    let v = logic
        .create_object("WP_VIC", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .unwrap();
    {
        let u = logic.host_object_mut(a).unwrap();
        u.set_target(Some(v));
        u.set_guard_target(Some(v));
    }
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_add_waypoint(&[a], Vec3::new(90.0, 0.0, 0.0)),
        CommandResult::Success
    );
    let u = logic.host_object(a).unwrap();
    assert!(u.target.is_none(), "queued waypoint must drop attack target");
    assert!(u.guard_target.is_none(), "queued waypoint must drop guard");
    assert_eq!(u.ai_state, AIState::Moving);
    assert!(!u.status.attacking);
}


#[test]
fn move_delays_mood_for_unstealthed_stealth_unit() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.set_current_frame(50);
    let mut tpl = ThingTemplate::new("MV_ST");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("MV_ST".to_string(), tpl);
    let a = logic.create_object("MV_ST", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(a).unwrap();
        u.innate_stealth = true;
        u.stealth_delay_frames = 30;
        u.auto_acquire_when_idle = true;
        u.status.stealthed = false;
        u.status.detected = false;
        u.next_mood_check_time = 0;
    }
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_move(&[a], Vec3::new(50.0, 0.0, 0.0)),
        CommandResult::Success
    );
    let u = logic.host_object(a).unwrap();
    assert_eq!(u.next_mood_check_time, 80); // 50+30+0
}

#[test]
fn dock_uses_controlling_player_for_centers_and_relationship_for_warehouses() {
    use super::CommandExecutor;
    use crate::game_logic::{DockKind, GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA slot 0", true));
    logic.add_player(Player::new(1, Team::USA, "USA slot 1", false));

    let mut collector = ThingTemplate::new("DockOwnerCollector");
    collector
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Harvester)
        .set_health(100.0);
    logic
        .templates
        .insert("DockOwnerCollector".to_string(), collector);

    let mut center = ThingTemplate::new("DockOwnerCenter");
    center.add_kind_of(KindOf::Structure).set_health(1_000.0);
    center.dock_kind = DockKind::SupplyCenter;
    logic
        .templates
        .insert("DockOwnerCenter".to_string(), center);

    let mut warehouse = ThingTemplate::new("DockOwnerWarehouse");
    warehouse.add_kind_of(KindOf::Structure).set_health(1_000.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    logic
        .templates
        .insert("DockOwnerWarehouse".to_string(), warehouse);

    let collector_id = logic
        .create_object("DockOwnerCollector", Team::USA, Vec3::ZERO)
        .expect("collector");
    let center_id = logic
        .create_object("DockOwnerCenter", Team::USA, Vec3::new(30.0, 0.0, 0.0))
        .expect("center");
    let warehouse_id = logic
        .create_object("DockOwnerWarehouse", Team::USA, Vec3::new(60.0, 0.0, 0.0))
        .expect("warehouse");
    {
        let collector = logic
            .host_object_mut(collector_id)
            .expect("collector object");
        collector.owner_player_id = Some(0);
        collector.stored_resources.supplies = 1;
    }
    for id in [center_id, warehouse_id] {
        let target = logic.host_object_mut(id).expect("dock target");
        target.owner_player_id = Some(1);
        target.stored_resources.supplies = 1;
    }

    // C++ ActionManager::canTransferSuppliesAt: SupplyCenter checks pointer
    // equality of controlling players, not faction/alliance equality.
    {
        let exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.can_issue_dock(collector_id, center_id), None);
        // The two player slots have no alliance, so the same-faction warehouse
        // is an enemy target and must be refused by its relationship gate.
        assert_eq!(exec.can_issue_dock(collector_id, warehouse_id), None);
    }

    logic.get_player_mut(0).expect("slot 0").alliance_team = 7;
    logic.get_player_mut(1).expect("slot 1").alliance_team = 7;
    let exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.can_issue_dock(collector_id, warehouse_id),
        Some(DockKind::SupplyWarehouse),
        "allied owners may collect from a warehouse"
    );
    assert_eq!(
        exec.can_issue_dock(collector_id, center_id),
        None,
        "allied is still not the same controller for a SupplyCenter"
    );
}

#[test]
fn return_to_base_prefers_exact_owner_producer_and_only_allows_explicitly_allied_fallback() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, ParkingPlaceMetadata, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    // Slots 0 and 1 deliberately share a faction but begin as enemies.  A
    // faction/name shortcut would incorrectly send the first unbound jet to
    // the nearer slot-1 airfield.
    logic.add_player(Player::new(0, Team::USA, "USA slot 0", true));
    logic.add_player(Player::new(1, Team::USA, "USA slot 1", false));
    logic.add_player(Player::new(2, Team::China, "China slot 2", false));

    let parking = ParkingPlaceMetadata {
        num_rows: 1,
        num_cols: 1,
        approach_height: 37.0,
        landing_deck_height_offset: 4.0,
        has_runways: false,
        park_in_hangars: true,
        heal_amount_per_second: 10.0,
    };
    for name in ["RTBExactOwnerAirfield", "RTBAlternateAirfield"] {
        let mut airfield = ThingTemplate::new(name);
        airfield
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::FSAirfield)
            .set_health(1_000.0);
        airfield.parking_place = Some(parking.clone());
        logic.templates.insert(name.to_string(), airfield);
    }
    let mut jet = ThingTemplate::new("RTBOwnerJet");
    jet.add_kind_of(KindOf::Aircraft)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("RTBOwnerJet".to_string(), jet);

    let owner_airfield = logic
        .create_object_for_player("RTBExactOwnerAirfield", 0, Vec3::new(80.0, 0.0, 0.0))
        .expect("slot-0 airfield");
    let same_faction_other_player_airfield = logic
        .create_object_for_player("RTBAlternateAirfield", 1, Vec3::new(2.0, 0.0, 0.0))
        .expect("slot-1 airfield");
    let enemy_airfield = logic
        .create_object_for_player("RTBAlternateAirfield", 2, Vec3::new(4.0, 0.0, 0.0))
        .expect("enemy airfield");

    // C++ JetAIUpdate first asks getPP(producerID).  Its exact controller is
    // the authority here, even though two invalid alternates are closer.
    let producer_jet = logic
        .create_object_for_player("RTBOwnerJet", 0, Vec3::ZERO)
        .expect("producer jet");
    logic
        .host_object_mut(producer_jet)
        .expect("producer jet object")
        .producer_id = Some(owner_airfield);
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor.execute_return_to_base(&[producer_jet]),
            CommandResult::Success
        );
    }
    let producer_jet_object = logic
        .host_object(producer_jet)
        .expect("parked producer jet");
    assert_eq!(producer_jet_object.contained_by, Some(owner_airfield));
    assert_eq!(producer_jet_object.producer_id, Some(owner_airfield));
    assert_eq!(producer_jet_object.airfield_parking_space_index, Some(0));
    assert_ne!(
        producer_jet_object.contained_by,
        Some(same_faction_other_player_airfield),
        "nearer same-faction other-player airfield must not override producer"
    );

    // Remove the exact owner field.  The only remaining candidates are a
    // same-faction other player and a genuine enemy; neither is an ally.
    logic
        .host_object_mut(owner_airfield)
        .expect("owner airfield")
        .status
        .sold = true;
    let rejected_jet = logic
        .create_object_for_player("RTBOwnerJet", 0, Vec3::ZERO)
        .expect("rejected jet");
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor.execute_return_to_base(&[rejected_jet]),
            CommandResult::InvalidCommand
        );
    }
    let rejected = logic
        .host_object(rejected_jet)
        .expect("rejected jet object");
    assert_eq!(rejected.contained_by, None);
    assert_eq!(rejected.producer_id, None);
    assert_eq!(rejected.airfield_parking_space_index, None);
    assert_ne!(rejected.contained_by, Some(enemy_airfield));

    // Once the two separate player slots are explicitly allied, C++
    // ALLOW_ALLIES fallback may choose the actual ParkingPlaceBehavior.
    logic.get_player_mut(0).expect("slot 0").alliance_team = 17;
    logic.get_player_mut(1).expect("slot 1").alliance_team = 17;
    let allied_fallback_jet = logic
        .create_object_for_player("RTBOwnerJet", 0, Vec3::ZERO)
        .expect("allied fallback jet");
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor.execute_return_to_base(&[allied_fallback_jet]),
            CommandResult::Success
        );
    }
    let allied = logic
        .host_object(allied_fallback_jet)
        .expect("allied fallback jet object");
    assert_eq!(
        allied.contained_by,
        Some(same_faction_other_player_airfield),
        "only an explicit player alliance may unlock alternate-airfield RTB"
    );
    assert_eq!(allied.airfield_parking_space_index, Some(0));

    // The fallback remains relationship-authorized because it owns an actual
    // reserved slot; it must not be reclassified as a same-owner producer or
    // release/reacquire capacity on each RTB update.
    {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            executor.execute_return_to_base(&[allied_fallback_jet]),
            CommandResult::Success
        );
    }
    let allied_again = logic
        .host_object(allied_fallback_jet)
        .expect("persisted allied fallback reservation");
    assert_eq!(
        allied_again.producer_id,
        Some(same_faction_other_player_airfield)
    );
    assert_eq!(allied_again.airfield_parking_space_index, Some(0));
}

#[test]
fn railed_transport_command_fails_closed_without_pathprefix_runtime() {
    use super::CommandExecutor;
    use crate::assets::IniParser;
    use crate::command_system::CommandResult;
    use crate::game_logic::{
        AIState, ContainModuleKind, DockKind, GameLogic, KindOf, Team, ThingTemplate,
    };
    use glam::Vec3;

    // This is deliberately an arbitrary identity rather than a train/ferry
    // name.  Its only authority comes from the same retail behavior modules
    // as AutoFerry: contain, railed AI with PathPrefixName, and railed dock.
    let mut parser = IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object ArbitraryWaterCarrier
  Type = Vehicle
  Model = ArbitraryWaterCarrierModel
  KindOf = SELECTABLE TRANSPORT
  Behavior = RailedTransportContain ModuleTag_03
    Slots = 2
    AllowInsideKindOf = INFANTRY VEHICLE BOAT
  End
  Behavior = RailedTransportAIUpdate ModuleTag_05
    PathPrefixName = Ferry
  End
  Behavior = RailedTransportDockUpdate ModuleTag_06
    NumberApproachPositions = 9
    PullInsideDuration = 4500
    PushOutsideDuration = 4500
    ToleranceDistance = 400.0
  End
End
"#,
            "retail_railed_transport_executor_probe.ini",
        )
        .expect("parse retail-shaped railed transport");
    let definition = parser
        .get_definition("ArbitraryWaterCarrier")
        .expect("railed transport definition");
    assert!(definition.behavior_modules.iter().any(|module| {
        module
            .class_name
            .eq_ignore_ascii_case("RailedTransportAIUpdate")
            && module.attribute("PathPrefixName") == Some("Ferry")
    }));

    let carrier_template =
        GameLogic::build_template_from_object_definition("ArbitraryWaterCarrier", definition, None);
    assert_eq!(carrier_template.dock_kind, DockKind::RailedTransport);
    assert_eq!(
        carrier_template.contain_module.kind,
        ContainModuleKind::RailedTransport
    );
    assert_eq!(carrier_template.contain_module.slots, Some(2));

    let mut logic = GameLogic::new();
    logic
        .templates
        .insert(carrier_template.name.clone(), carrier_template);
    let mut rider_template = ThingTemplate::new("RetailRailedPassenger");
    rider_template
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic
        .templates
        .insert(rider_template.name.clone(), rider_template);

    let carrier = logic
        .create_object("ArbitraryWaterCarrier", Team::USA, Vec3::ZERO)
        .expect("railed transport object");
    let rider = logic
        .create_object("RetailRailedPassenger", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("passenger");
    let original_path = vec![Vec3::new(40.0, 0.0, 20.0), Vec3::new(80.0, 0.0, 25.0)];
    let original_target = Some(Vec3::new(80.0, 0.0, 25.0));
    {
        let ferry = logic.host_object_mut(carrier).expect("carrier");
        assert!(ferry.add_occupant(rider));
        ferry.movement.path = original_path.clone();
        ferry.movement.target_position = original_target;
        ferry.movement.current_path_index = 1;
        ferry.set_ai_state(AIState::Idle);
    }
    {
        let passenger = logic.host_object_mut(rider).expect("passenger");
        passenger.set_contained_by(Some(carrier));
        passenger.set_ai_state(AIState::Docked);
    }

    let result = {
        let mut executor = CommandExecutor::new(&mut logic, 0);
        executor.execute_railed_transport(&[carrier])
    };
    assert_eq!(result, CommandResult::InvalidCommand);

    // Until Main retains the parsed PathPrefixName, map waypoint pairs, and
    // transit/dock runtime, ExecuteRailedTransport must not masquerade as
    // either Evacuate or a generic Move command.
    let ferry = logic.host_object(carrier).expect("carrier after rejection");
    assert_eq!(ferry.contained_units(), vec![rider]);
    assert_eq!(ferry.movement.path, original_path);
    assert_eq!(ferry.movement.target_position, original_target);
    assert_eq!(ferry.movement.current_path_index, 1);
    assert_eq!(ferry.ai_state, AIState::Idle);
    let passenger = logic.host_object(rider).expect("passenger after rejection");
    assert_eq!(passenger.contained_by, Some(carrier));
    assert_eq!(passenger.ai_state, AIState::Docked);
}

#[test]
fn attack_team_reacquires_after_victim_dies() {
    // C++ AIGroup::groupAttackTeam → aiAttackTeam (AIGroup.cpp:2179-2193).
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["AT2_U", "AT2_E"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(200.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let u = logic.create_object("AT2_U", Team::USA, Vec3::ZERO).unwrap();
    let near = logic
        .create_object("AT2_E", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let far = logic
        .create_object("AT2_E", Team::GLA, Vec3::new(80.0, 0.0, 0.0))
        .unwrap();
    {
        let uo = logic.host_object_mut(u).unwrap();
        uo.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
});
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_team(&[u], 0, 7),
            CommandResult::Success
        );
    }
    assert_eq!(logic.host_object(u).unwrap().target, Some(near));
    assert_eq!(logic.host_object(u).unwrap().max_shots_to_fire, 7);
    logic.host_object_mut(near).unwrap().health.current = 0.0;
    logic.tick_attack_team_persist(&[u]);
    assert_eq!(
        logic.host_object(u).unwrap().target,
        Some(far),
        "attack-team must re-acquire the next living team member"
    );
}

#[test]
fn group_min_max_skips_buildings_without_ai() {
    // C++ AIGroup::getMinMaxAndCenter (AIGroup.cpp:331-362) counts AI only.
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut veh = ThingTemplate::new("MMC_V");
    veh.add_kind_of(KindOf::Vehicle);
    veh.add_kind_of(KindOf::Selectable);
    veh.set_health(200.0);
    let mut bld = ThingTemplate::new("MMC_B");
    bld.add_kind_of(KindOf::Structure);
    bld.add_kind_of(KindOf::Immobile);
    bld.add_kind_of(KindOf::Selectable);
    bld.set_health(1000.0);
    logic.templates.insert("MMC_V".to_string(), veh);
    logic.templates.insert("MMC_B".to_string(), bld);
    let a = logic
        .create_object("MMC_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("MMC_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let building = logic
        .create_object("MMC_B", Team::USA, Vec3::new(400.0, 0.0, 0.0))
        .unwrap();
    let exec = CommandExecutor::new(&mut logic, 0);
    let (min, max, center) = exec
        .group_min_max_and_center(&[a, b, building])
        .expect("AI members");
    assert!((center.x - 20.0).abs() < 0.1, "center={center:?}");
    assert!((max.x - min.x - 40.0).abs() < 0.1);
}

#[test]
fn attack_move_uses_identical_destination() {
    // C++ groupAttackMoveToPosition (AIGroup.cpp:2260-2273).
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AM2_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("AM2_V".to_string(), tpl);
    let a = logic
        .create_object("AM2_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("AM2_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    for id in [a, b] {
        let o = logic.host_object_mut(id).unwrap();
        o.weapon = Some(Weapon {
            damage: 10.0,
            range: 150.0,
            ..Weapon::default()
});
        o.selection_radius = 15.0;
    }
    let dest = Vec3::new(200.0, 0.0, 0.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_move(&[a, b], dest, 3),
            CommandResult::Success
        );
    }
    let ga = logic
        .host_object(a)
        .unwrap()
        .movement
        .target_position
        .or_else(|| logic.host_object(a).unwrap().movement.path.last().copied());
    let gb = logic
        .host_object(b)
        .unwrap()
        .movement
        .target_position
        .or_else(|| logic.host_object(b).unwrap().movement.path.last().copied());
    let ga = ga.expect("a dest");
    let gb = gb.expect("b dest");
    assert!(
        (ga.x - gb.x).abs() < 1.0 && (ga.z - gb.z).abs() < 1.0,
        "attack-move must share one pos ga={ga:?} gb={gb:?}"
    );
}

#[test]
fn scatter_uses_bounding_circle_not_selection_radius() {
    // C++ AIGroup::groupScatter (AIGroup.cpp:1790-1791).
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SC_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("SC_V".to_string(), tpl);
    let a = logic
        .create_object("SC_V", Team::USA, Vec3::new(-10.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("SC_V", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    for id in [a, b] {
        let o = logic.host_object_mut(id).unwrap();
        o.selection_radius = 50.0;
        o.thing.geometry.radius = 5.0;
        o.thing.geometry.bounds_min = Vec3::new(-5.0, 0.0, -5.0);
        o.thing.geometry.bounds_max = Vec3::new(5.0, 0.0, 5.0);
    }
    let before_a = logic.host_object(a).unwrap().get_position();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_scatter(&[a, b]), CommandResult::Success);
    }
    let dest = logic
        .host_object(a)
        .unwrap()
        .movement
        .target_position
        .or_else(|| logic.host_object(a).unwrap().movement.path.last().copied())
        .expect("scatter dest");
    let push = before_a.distance(Vec3::new(dest.x, before_a.y, dest.z));
    // 4 * bounding circle 5 = 20, not 4 * selection 50 = 200.
    assert!(
        (push - 20.0).abs() < 2.0,
        "scatter push={push} expected ~20 from bounding circle"
    );
}

#[test]
fn tighten_helicopters_use_offset_ring() {
    // C++ getHelicopterOffset (AIGroup.cpp:1799-1826, :1884-1898).
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AmericaHelicopterComanche");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Aircraft);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic
        .templates
        .insert("AmericaHelicopterComanche".to_string(), tpl);
    let a = logic
        .create_object(
            "AmericaHelicopterComanche",
            Team::USA,
            Vec3::new(0.0, 10.0, 0.0),
        )
        .unwrap();
    let b = logic
        .create_object(
            "AmericaHelicopterComanche",
            Team::USA,
            Vec3::new(5.0, 10.0, 0.0),
        )
        .unwrap();
    let click = Vec3::new(100.0, 10.0, 50.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_tighten_to_position(&[a, b], click),
            CommandResult::Success
        );
    }
    let ga = logic
        .host_object(a)
        .unwrap()
        .movement
        .target_position
        .or_else(|| logic.host_object(a).unwrap().movement.path.last().copied())
        .expect("a");
    let gb = logic
        .host_object(b)
        .unwrap()
        .movement
        .target_position
        .or_else(|| logic.host_object(b).unwrap().movement.path.last().copied())
        .expect("b");
    let spread = (ga.x - gb.x).hypot(ga.z - gb.z);
    assert!(
        spread > 50.0,
        "heli tighten must use getHelicopterOffset ring spread={spread} ga={ga:?} gb={gb:?}"
    );
}

#[test]
fn group_move_clamps_waypoint_to_map_extent() {
    // C++ clampWaypointPosition (AIGroup.cpp:1497-1521, :1592-1593).
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("CL_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("CL_V".to_string(), tpl);
    let id = logic.create_object("CL_V", Team::USA, Vec3::ZERO).unwrap();
    let (min, max) = logic.world_bounds();
    let outside = Vec3::new(max.x + 500.0, 0.0, max.z + 500.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_move(&[id], outside),
            CommandResult::Success
        );
    }
    let dest = logic
        .host_object(id)
        .unwrap()
        .movement
        .target_position
        .or_else(|| logic.host_object(id).unwrap().movement.path.last().copied())
        .expect("clamped dest");
    let margin = 4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
    assert!(
        dest.x <= max.x - margin + 1.0 && dest.z <= max.z - margin + 1.0,
        "waypoint must clamp inside extent dest={dest:?} max={max:?} min={min:?}"
    );
}

#[test]
fn compute_ground_path_infantry_line_passable_fallback() {
    // C++ friend_computeGroundPath infantry isLinePassable (AIGroup.cpp:590-611).
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
    let i = prod
        .find("fn compute_ground_path_should_group")
        .expect("compute_ground_path_should_group");
    let w = &prod[i..prod.len().min(i + 4500)];
    assert!(
        w.contains("infantry_line_passable_to_center") && w.contains("is_passable"),
        "group-path gate must keep the infantry line-passable fallback"
    );
}

#[test]
fn stamped_formation_move_does_not_tighten() {
    // C++ AIGroup::groupMoveToPosition (AIGroup.cpp:1559-1615): click-to-gather
    // only when !isFormation. Stamped formations keep offsets.
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("FM_V");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("FM_V".to_string(), tpl);
    let a = logic
        .create_object("FM_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("FM_V", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let dest = Vec3::new(20.0, 0.0, 0.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_create_formation(&[a, b]),
            CommandResult::Success
        );
        assert!(
            exec.should_tighten_group_move(&[a, b], dest),
            "click is inside the bbox; tighten would fire without the formation gate"
        );
        assert_eq!(exec.execute_move(&[a, b], dest), CommandResult::Success);
    }
    let fa = logic.host_object(a).unwrap();
    let fb = logic.host_object(b).unwrap();
    assert_ne!(fa.formation_id, 0, "tighten must not dissolve formation");
    assert_eq!(fa.formation_id, fb.formation_id);
    let ga = fa
        .movement
        .path
        .last()
        .copied()
        .or(fa.movement.target_position)
        .unwrap();
    let gb = fb
        .movement
        .path
        .last()
        .copied()
        .or(fb.movement.target_position)
        .unwrap();
    assert!(
        (ga.x - gb.x).abs() > 20.0,
        "formation move keeps stamped offset ga={ga:?} gb={gb:?}"
    );
}

#[test]
fn ground_path_distance_ignores_aircraft() {
    // C++ friend_computeGroundPath (AIGroup.cpp:534-549): aircraft continue;
    // closest_sqr is infantry/vehicle-with-AI only.
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tank = ThingTemplate::new("GP_T");
    tank.add_kind_of(KindOf::Vehicle);
    tank.add_kind_of(KindOf::Selectable);
    tank.set_health(200.0);
    logic.templates.insert("GP_T".to_string(), tank);
    let mut jet = ThingTemplate::new("GP_J");
    jet.add_kind_of(KindOf::Vehicle);
    jet.add_kind_of(KindOf::Aircraft);
    jet.add_kind_of(KindOf::Selectable);
    jet.set_health(100.0);
    logic.templates.insert("GP_J".to_string(), jet);
    let t0 = logic
        .create_object("GP_T", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let t1 = logic
        .create_object("GP_T", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    let j = logic
        .create_object("GP_J", Team::USA, Vec3::new(250.0, 0.0, 0.0))
        .unwrap();
    let dest = Vec3::new(250.0, 0.0, 0.0);
    let exec = CommandExecutor::new(&mut logic, 0);
    assert!(
        exec.compute_ground_path_should_group(&[t0, t1, j], dest),
        "aircraft sitting on the click must not suppress tank group-path"
    );
}

#[test]
fn mixed_infantry_vehicle_column_packs_both_kinds() {
    // C++ groupMoveToPosition (AIGroup.cpp:1550-1553) packs infantry then vehicles.
    use super::CommandExecutor;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut inf = ThingTemplate::new("MX_I");
    inf.add_kind_of(KindOf::Infantry);
    inf.add_kind_of(KindOf::Selectable);
    inf.set_health(100.0);
    logic.templates.insert("MX_I".to_string(), inf);
    let mut veh = ThingTemplate::new("MX_V");
    veh.add_kind_of(KindOf::Vehicle);
    veh.add_kind_of(KindOf::Selectable);
    veh.set_health(200.0);
    logic.templates.insert("MX_V".to_string(), veh);
    let mut ids = Vec::new();
    for i in 0..3 {
        ids.push(
            logic
                .create_object("MX_I", Team::USA, Vec3::new(i as f32 * 8.0, 0.0, 0.0))
                .unwrap(),
        );
    }
    for i in 0..3 {
        ids.push(
            logic
                .create_object("MX_V", Team::USA, Vec3::new(i as f32 * 8.0, 0.0, 20.0))
                .unwrap(),
        );
    }
    let dest = Vec3::new(400.0, 0.0, 0.0);
    let exec = CommandExecutor::new(&mut logic, 0);
    let goals = exec.group_move_destinations(&ids, dest);
    assert_eq!(goals.len(), 6, "mixed group must destination-pack every member");
    let unique_xz: std::collections::HashSet<(i32, i32)> = goals
        .iter()
        .map(|(_, p)| ((p.x * 10.0) as i32, (p.z * 10.0) as i32))
        .collect();
    assert!(
        unique_xz.len() >= 4,
        "infantry 3-col + vehicle 2-col must not collapse to one spine: {goals:?}"
    );
}

#[test]
fn group_special_power_fires_every_capable_caster() {
    // C++ AIGroup::groupDoSpecialPower* (AIGroup.cpp:2614-2735) loops every member.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, PowerTarget, SpecialPowerType};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SP_ALL");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("SP_ALL".to_string(), tpl);
    let a = logic
        .create_object("SP_ALL", Team::USA, Vec3::ZERO)
        .unwrap();
    let b = logic
        .create_object("SP_ALL", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .unwrap();
    for id in [a, b] {
        let o = logic.host_object_mut(id).unwrap();
        o.special_power_cooldowns
            .insert(SpecialPowerType::SpySatellite, 0.0);
        o.special_power_ready = true;
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        let res = exec.execute_special_power(
            &[a, b],
            &SpecialPowerType::SpySatellite,
            &PowerTarget::Location(Vec3::new(80.0, 0.0, 80.0)),
        );
        assert_eq!(res, CommandResult::Success);
    }
    let sa = logic.host_object(a).unwrap().ai_state;
    let sb = logic.host_object(b).unwrap().ai_state;
    assert!(
        sa == crate::game_logic::AIState::SpecialAbility
            || sb == crate::game_logic::AIState::SpecialAbility,
        "at least one caster must enter SpecialAbility; a={sa:?} b={sb:?}"
    );
    // Both members that track the power must be considered (not first-only).
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = src
        .find("fn execute_special_power(")
        .expect("execute_special_power");
    let body = &src[i..src.len().min(i + 2500)];
    assert!(
        !body.contains("vec![src]"),
        "groupDoSpecialPower must not collapse to getSpecialPowerSourceObject"
    );
}

#[test]
fn combat_drop_sets_pending_evacuate() {
    // C++ AIGroup::groupCombatDrop (AIGroup.cpp:2867-2889) aiCombatDrop unloads.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, DropTarget};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("CD_T");
    t.add_kind_of(KindOf::Vehicle);
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
        .create_object("CD_T", Team::USA, Vec3::ZERO)
        .unwrap();
    let pax = logic
        .create_object("CD_P", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.is_combat_chinook_transport = true;
        t.max_transport = 8;
        let _ = t.add_occupant(pax);
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
        p.set_ai_state(crate::game_logic::AIState::Docked);
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
    assert!(
        t.pending_evacuate_on_stop,
        "combat drop must pending-evacuate so passengers rappel on arrival"
    );
    assert!(
        logic
            .host_object(pax)
            .unwrap()
            .contained_by
            .is_some(),
        "passengers stay aboard until the transport arrives"
    );
}

#[test]
fn evacuate_airborne_uses_terrain_height_not_sea_level() {
    // C++ AIGroup::groupEvacuate (AIGroup.cpp:2416-2422): dest Z = terrain height,
    // then aiMoveToAndEvacuate — do not unload in the air.
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("EV_CH");
    t.add_kind_of(KindOf::Vehicle);
    t.add_kind_of(KindOf::Aircraft);
    t.add_kind_of(KindOf::Selectable);
    t.set_health(200.0);
    logic.templates.insert("EV_CH".to_string(), t);
    let mut p = ThingTemplate::new("EV_PX");
    p.add_kind_of(KindOf::Infantry);
    p.add_kind_of(KindOf::Selectable);
    p.set_health(100.0);
    logic.templates.insert("EV_PX".to_string(), p);
    let transport = logic
        .create_object("EV_CH", Team::USA, Vec3::new(10.0, 40.0, 10.0))
        .unwrap();
    let pax = logic
        .create_object("EV_PX", Team::USA, Vec3::new(10.0, 40.0, 10.0))
        .unwrap();
    {
        let t = logic.host_object_mut(transport).unwrap();
        t.is_combat_chinook_transport = true;
        t.max_transport = 8;
        t.status.airborne_target = true;
        t.ground_height = 15.0;
        t.ground_height_from_terrain = true;
        let _ = t.add_occupant(pax);
    }
    {
        let p = logic.host_object_mut(pax).unwrap();
        p.set_contained_by(Some(transport));
        p.set_ai_state(crate::game_logic::AIState::Docked);
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_evacuate(&[transport]),
            CommandResult::Success
        );
    }
    let t = logic.host_object(transport).unwrap();
    assert!(
        t.pending_evacuate_on_stop,
        "airborne evacuate must path-then-unload, not execute_exit in air"
    );
    let goal = t
        .movement
        .path
        .last()
        .copied()
        .or(t.movement.target_position)
        .expect("airborne evacuate must path to ground");
    assert!(
        (goal.y - 15.0).abs() < 0.1,
        "dest Y must be terrain/ground_height not sea-level 0; goal={goal:?}"
    );
    assert!(
        logic
            .host_object(pax)
            .unwrap()
            .contained_by
            .is_some(),
        "passengers must not dump at air position"
    );
}

#[test]
fn object_attack_orders_passenger_fire() {
    // C++ groupAttackObjectPrivate (AIGroup.cpp:2131-2151) orders fire-capable passengers.
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut hum = ThingTemplate::new("AT_H");
    hum.add_kind_of(KindOf::Vehicle);
    hum.add_kind_of(KindOf::Selectable);
    hum.set_health(200.0);
    logic.templates.insert("AT_H".to_string(), hum);
    let mut inf = ThingTemplate::new("AT_I");
    inf.add_kind_of(KindOf::Infantry);
    inf.add_kind_of(KindOf::Selectable);
    inf.set_health(100.0);
    logic.templates.insert("AT_I".to_string(), inf);
    let mut tgt = ThingTemplate::new("AT_E");
    tgt.add_kind_of(KindOf::Vehicle);
    tgt.add_kind_of(KindOf::Selectable);
    tgt.set_health(100.0);
    logic.templates.insert("AT_E".to_string(), tgt);
    let humvee = logic
        .create_object("AT_H", Team::USA, Vec3::ZERO)
        .unwrap();
    let rider = logic
        .create_object("AT_I", Team::USA, Vec3::ZERO)
        .unwrap();
    let enemy = logic
        .create_object("AT_E", Team::China, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let h = logic.host_object_mut(humvee).unwrap();
        h.passengers_allowed_to_fire = true;
        h.is_humvee_transport = true;
        h.max_transport = 5;
        let _ = h.add_occupant(rider);
        h.weapon = Some(Weapon {
            damage: 5.0,
            range: 80.0,
            ..Weapon::default()
});
    }
    {
        let r = logic.host_object_mut(rider).unwrap();
        r.set_contained_by(Some(humvee));
        r.set_ai_state(crate::game_logic::AIState::Garrisoned);
        r.weapon = Some(Weapon {
            damage: 10.0,
            range: 80.0,
            ..Weapon::default()
});
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack(&[humvee], enemy),
            CommandResult::Success
        );
    }
    let rider = logic.host_object(rider).unwrap();
    assert_eq!(
        rider.target,
        Some(enemy),
        "passenger allowed to fire must receive the object-attack order"
    );
}

#[test]
fn stop_idles_garrison_occupants_and_hive_slaves() {
    // C++ AIGroup::groupIdle (AIGroup.cpp:2066-2081): no-AI contain iterate + slaves idle.
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::host_base_defense::{
        init_stinger_hive_slave_roster, order_hive_slaves_to_attack_target,
    };
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut bunker = ThingTemplate::new("ST_B");
    bunker.add_kind_of(KindOf::Structure);
    bunker.add_kind_of(KindOf::Immobile);
    bunker.add_kind_of(KindOf::Selectable);
    bunker.set_health(500.0);
    logic.templates.insert("ST_B".to_string(), bunker);
    let mut inf = ThingTemplate::new("ST_I");
    inf.add_kind_of(KindOf::Infantry);
    inf.add_kind_of(KindOf::Selectable);
    inf.set_health(100.0);
    logic.templates.insert("ST_I".to_string(), inf);
    let site = logic
        .create_object("ST_B", Team::USA, Vec3::ZERO)
        .unwrap();
    let occ = logic
        .create_object("ST_I", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let s = logic.host_object_mut(site).unwrap();
        if let Some(b) = s.building_data.as_mut() {
            b.max_garrison = 5;
        }
        let _ = s.add_occupant(occ);
        s.hive_slaves = init_stinger_hive_slave_roster();
        s.hive_slave_count = 3;
        let _ = order_hive_slaves_to_attack_target(&mut s.hive_slaves, 99);
    }
    {
        let o = logic.host_object_mut(occ).unwrap();
        o.set_contained_by(Some(site));
        o.set_ai_state(crate::game_logic::AIState::Attacking);
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_stop(&[site]), CommandResult::Success);
    }
    let occ = logic.host_object(occ).unwrap();
    assert_eq!(
        occ.ai_state,
        crate::game_logic::AIState::Idle,
        "S on a garrisoned structure must stop occupants"
    );
    let site = logic.host_object(site).unwrap();
    assert!(
        site.hive_slaves
            .iter()
            .filter(|s| s.alive)
            .all(|s| !s.ai_attacking),
        "S must orderSlavesToGoIdle"
    );
}

#[test]
fn stop_idles_ai_transport_occupants() {
    // C++ privateIdle walks contain riders (AIUpdate.cpp:3076-3088).
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut humvee = ThingTemplate::new("ST_HV");
    humvee
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    logic.templates.insert("ST_HV".into(), humvee);
    let mut inf = ThingTemplate::new("ST_RI");
    inf.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert("ST_RI".into(), inf);
    let truck = logic
        .create_object("ST_HV", Team::USA, Vec3::ZERO)
        .unwrap();
    let rider = logic
        .create_object("ST_RI", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let t = logic.host_object_mut(truck).unwrap();
        t.is_humvee_transport = true;
        t.max_transport = 5;
        t.passengers_allowed_to_fire = true;
        assert!(t.add_occupant(rider), "load rider");
    }
    {
        let r = logic.host_object_mut(rider).unwrap();
        r.set_contained_by(Some(truck));
        r.set_ai_state(crate::game_logic::AIState::Attacking);
        r.set_target(Some(ObjectId(99)));
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_stop(&[truck]), CommandResult::Success);
    }
    let r = logic.host_object(rider).unwrap();
    assert_eq!(r.ai_state, crate::game_logic::AIState::Idle);
    assert!(r.target.is_none(), "Stop on transport must idle firing riders");
}


#[test]
fn stealth_mood_delay_skips_while_stealthed_auto_acquire() {
    // C++ AIGroup::groupIdle (AIGroup.cpp:2051): !canAutoAcquireWhileStealthed.
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("ST_P");
    tpl.add_kind_of(KindOf::Infantry);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("ST_P".to_string(), tpl);
    let id = logic.create_object("ST_P", Team::USA, Vec3::ZERO).unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.innate_stealth = true;
        u.stealth_delay_frames = 30;
        u.auto_acquire_when_idle = true;
        u.stealth_breaks_on_attack = false;
        u.status.stealthed = false;
        u.status.detected = false;
        u.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 80.0,
            ..crate::game_logic::Weapon::default()
});
        u.next_mood_check_time = 0;
    }
    assert!(
        !logic.unit_command_apply_stealth_mood_delay(id, 100, 5),
        "units that auto-acquire while stealthed must not get a stop mood delay"
    );
    assert_eq!(logic.host_object(id).unwrap().next_mood_check_time, 0);
}

fn dispatch_test_command(
    command_type: crate::command_system::CommandType,
    player_id: u32,
    selected: Vec<crate::game_logic::ObjectId>,
) -> crate::command_system::GameCommand {
    crate::command_system::GameCommand {
        command_type,
        player_id,
        command_id: 0,
        timestamp: std::time::SystemTime::now(),
        selected_units: selected,
        modifier_keys: crate::command_system::ModifierKeys::default(),
    }
}

#[test]
fn switch_weapons_locks_button_slot_not_cycle() {
    // C++ GameLogicDispatch.cpp:583-590 MSG_SWITCH_WEAPONS locks the
    // ControlBar button slot permanently instead of cycling 0→1→2.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::{GameLogic, Player, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    logic
        .templates
        .insert("HumveeSlot".to_string(), ThingTemplate::new("HumveeSlot"));
    let id = logic
        .create_object("HumveeSlot", Team::USA, Vec3::ZERO)
        .expect("humvee");
    {
        let obj = logic.host_object_mut(id).expect("obj");
        obj.owner_player_id = Some(0);
        obj.weapon = Some(Weapon {
            damage: 1.0,
            range: 100.0,
            ..Weapon::default()
});
        obj.secondary_weapon = Some(Weapon {
            damage: 2.0,
            range: 100.0,
            ..Weapon::default()
});
        obj.tertiary_weapon = Some(Weapon {
            damage: 3.0,
            range: 100.0,
            ..Weapon::default()
});
        obj.active_weapon_slot = 0;
    }

    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_command(dispatch_test_command(
            CommandType::SwitchWeapons { slot: 1 },
            0,
            vec![id],
        ))
        .expect("execute")
    };
    assert_eq!(result, CommandResult::Success);
    let obj = logic.host_object(id).expect("obj");
    assert_eq!(obj.weapon_lock_slot, 1, "must lock the button slot");
    assert_eq!(obj.active_weapon_slot, 1);
    assert_ne!(obj.weapon_lock_slot, 2, "must not cycle to the next slot");
}

#[test]
fn enable_retaliation_mode_sets_logical_flag() {
    // C++ GameLogicDispatch.cpp:603-614 MSG_ENABLE_RETALIATION_MODE.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::{GameLogic, Player, Team};

    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));
    assert!(!logic
        .get_player(0)
        .unwrap()
        .logical_retaliation_mode_enabled);

    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_command(dispatch_test_command(
            CommandType::EnableRetaliationMode {
                player_index: 0,
                enabled: true,
            },
            0,
            vec![],
        ))
        .expect("execute")
    };
    assert_eq!(result, CommandResult::Success);
    assert!(
        logic
            .get_player(0)
            .unwrap()
            .logical_retaliation_mode_enabled
    );
}

#[test]
fn self_destruct_transfers_to_living_ally_then_kills() {
    // C++ GameLogicDispatch.cpp:1762-1797 MSG_SELF_DESTRUCT arg0=true.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut p0 = Player::new(0, Team::USA, "P0", true);
    p0.alliance_team = 1;
    logic.get_players_mut().insert(0, p0);
    let mut p1 = Player::new(1, Team::USA, "P1", false);
    p1.alliance_team = 1;
    logic.get_players_mut().insert(1, p1);

    let mut unit_tpl = ThingTemplate::new("Ranger");
    unit_tpl.add_kind_of(KindOf::Infantry).set_health(100.0);
    logic.templates.insert("Ranger".to_string(), unit_tpl);
    let unit = logic
        .create_object_for_player("Ranger", 0, Vec3::ZERO)
        .expect("unit");
    if let Some(obj) = logic.host_object_mut(unit) {
        obj.owner_player_id = Some(0);
    }

    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_command(dispatch_test_command(
            CommandType::SelfDestruct {
                transfer_to_ally: true,
            },
            0,
            vec![],
        ))
        .expect("execute")
    };
    assert_eq!(result, CommandResult::Success);
    assert!(!logic.get_player(0).unwrap().is_alive);
    assert_eq!(
        logic.host_object(unit).map(|o| o.owner_player_id),
        Some(Some(1)),
        "unit must transfer to the living mutual ally"
    );
}

#[test]
fn place_beacon_spawns_world_object_and_honors_cap() {
    // C++ GameLogicDispatch.cpp:1582-1671 MSG_PLACE_BEACON.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::{GameLogic, Player, Team};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "P0", true));

    let place = |logic: &mut GameLogic| {
        let mut exec = CommandExecutor::new(logic, 0);
        exec.execute_command(dispatch_test_command(
            CommandType::PlaceBeacon {
                location: Vec3::new(10.0, 0.0, 12.0),
                text: "here".into(),
            },
            0,
            vec![],
        ))
        .expect("execute")
    };

    assert_eq!(place(&mut logic), CommandResult::Success);
    let beacons: Vec<_> = logic
        .host_objects()
        .iter()
        .filter(|(_, o)| o.template_name.to_ascii_lowercase().contains("beacon"))
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(beacons.len(), 1);
    let id = beacons[0];
    assert_eq!(
        crate::command_executor::host_beacon_caption(id).as_deref(),
        Some("here")
    );

    let max = game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|s| {
        s.max_beacons_per_player
    })
    .max(1);
    for _ in 1..max {
        assert_eq!(place(&mut logic), CommandResult::Success);
    }
    assert_eq!(
        place(&mut logic),
        CommandResult::InvalidCommand,
        "cap must refuse extra beacons"
    );
}

/// C++ BuildAssistant.cpp:333-334 / :1365-1383 — placement clears trees/props.
#[test]
fn execute_build_clears_removable_and_map_trees() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;

    let _ = game_client::terrain::terrain_visual::init_terrain_visual();
    {
        let mut guard = game_client::terrain::terrain_visual::get_terrain_visual()
            .expect("terrain visual lock");
        let visual = guard.as_mut().expect("terrain visual");
        visual.tree_buffer_mut().clear_all_trees();
        visual.tree_buffer_mut().set_bounds(
            game_client::terrain::TreeRegion2D::new(
                glam::Vec2::new(-200.0, -200.0),
                glam::Vec2::new(200.0, 200.0),
            ),
        );
        let mut data = game_client::terrain::TreeModuleData::default();
        data.model_name = "Oak".into();
        visual
            .tree_buffer_mut()
            .add_tree(
                88,
                glam::Vec3::new(80.0, 80.0, 0.0),
                1.0,
                0.0,
                1.0,
                data,
                game_client::terrain::TreeSphere {
                    center: glam::Vec3::ZERO,
                    radius: 4.0,
                },
            )
            .expect("add tree");
        assert!(visual.add_prop([80.0, 80.0, 0.0], 0.0, 1.0, "TreeProp"));
    }

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 100_000;
    logic.add_player(player);

    let mut barracks = ThingTemplate::new("TestClearFootprintBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .set_cost(50, 0)
        .set_health(1_000.0);
    logic
        .templates
        .insert("TestClearFootprintBarracks".into(), barracks);

    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t.clone());
    let mut dozer = Object::new(dozer_t, ObjectId(9101), Team::USA);
    dozer.set_position(Vec3::new(40.0, 0.0, 80.0));
    dozer.owner_player_id = Some(0);
    logic.objects.insert(ObjectId(9101), dozer);

    let mut shrub_t = ThingTemplate::new("TreeOakShrub");
    shrub_t.set_health(10.0);
    let mut shrub = Object::new(shrub_t, ObjectId(9102), Team::Neutral);
    shrub.set_position(Vec3::new(80.0, 0.0, 80.0));
    shrub.status.effectively_dead = true;
    logic.objects.insert(ObjectId(9102), shrub);

    let site = Vec3::new(80.0, 0.0, 80.0);
    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_build(&[ObjectId(9101)], "TestClearFootprintBarracks", site, 0.0)
    };
    assert_eq!(result, CommandResult::Success, "placement must succeed");

    let shrub = logic.host_object(ObjectId(9102)).expect("shrub still rostered");
    assert!(
        shrub.status.destroyed || !shrub.is_alive(),
        "hq-wtzcx: removable shrub under footprint must be destroyed"
    );

    let mut guard = game_client::terrain::terrain_visual::get_terrain_visual()
        .expect("terrain visual lock");
    let visual = guard.as_mut().expect("terrain visual");
    assert!(
        !visual.construction_removals().is_empty(),
        "hq-wtzcx: execute_build must call removeTreesAndPropsForConstruction"
    );
    assert!(
        visual.terrain_props().is_empty(),
        "map prop under footprint must be removed"
    );
    assert!(
        visual
            .tree_buffer_mut()
            .trees()
            .iter()
            .all(|tree| tree.tree_type < 0),
        "map tree under footprint must be removed"
    );
}

#[test]
fn execute_build_source_records_build_slot_and_docks() {
    let src = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = src.find("fn execute_build").expect("execute_build");
    let w = &src[i..src.len().min(i + 8000)];
    assert!(
        w.contains("dozer_new_task_build")
            && w.contains("dozer_repair_approach_position")
            && w.contains("path_to_goal_with_state_ignoring"),
        "hq-gkpuk/hq-6gy32: execute_build must newTask BUILD, dock half-radius, ignoreObstacle"
    );
    let snap = include_str!("../game_logic/world_scripts/ui_production.rs");
    let j = snap
        .find("fn flatten_and_snap_construction")
        .expect("flatten_and_snap");
    let f = &snap[j..snap.len().min(j + 2500)];
    assert!(
        f.contains("flatten_terrain_box_at")
            && f.contains("HostGeometryType::Box"),
        "hq-6smw3: flatten_and_snap must use GEOMETRY_BOX flatten, not cylinder-only"
    );
}

#[test]
fn dozer_place_and_cancel_use_calc_cost_to_build_handicap() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1_000;
    player.map_side.handicap_build_cost_buildings = 0.75;
    logic.add_player(player);

    let mut barracks = ThingTemplate::new("TestHandicapBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Selectable)
        .set_health(1_000.0)
        .set_cost(1_000, 0);
    logic
        .templates
        .insert("TestHandicapBarracks".into(), barracks);

    let mut dozer_t = ThingTemplate::new("AmericaVehicleDozer");
    dozer_t
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .set_health(200.0);
    logic
        .templates
        .insert("AmericaVehicleDozer".into(), dozer_t.clone());
    let mut dozer = Object::new(dozer_t, ObjectId(9201), Team::USA);
    dozer.set_position(Vec3::new(0.0, 0.0, 0.0));
    dozer.owner_player_id = Some(0);
    logic.objects.insert(ObjectId(9201), dozer);

    assert_eq!(
        logic.modified_build_cost_supplies(0, "TestHandicapBarracks", 1_000),
        750
    );

    let result = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_build(
            &[ObjectId(9201)],
            "TestHandicapBarracks",
            Vec3::new(40.0, 0.0, 0.0),
            0.0,
        )
    };
    assert_eq!(result, CommandResult::Success, "handicap place must succeed");
    assert_eq!(
        logic.get_player(0).unwrap().effective_supplies(),
        250,
        "hq-iherw: place charges calcCostToBuild (1000 * 0.75)"
    );

    let building_id = logic
        .objects
        .values()
        .find(|o| o.template_name == "TestHandicapBarracks")
        .map(|o| o.id)
        .expect("placed barracks");

    let cancel = {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        exec.execute_cancel_construction(building_id, 0)
    };
    assert_eq!(cancel, CommandResult::Success);
    assert_eq!(
        logic.get_player(0).unwrap().effective_supplies(),
        1_000,
        "hq-iherw: cancel refunds calcCostToBuild, not raw INI BuildCost"
    );
}

