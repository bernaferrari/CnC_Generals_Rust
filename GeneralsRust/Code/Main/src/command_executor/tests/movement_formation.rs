use super::dispatch_test_command;

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
        oa.thing.geometry.radius = 10.0;
        oa.thing.geometry.bounds_min = Vec3::new(-10.0, 0.0, -10.0);
        oa.thing.geometry.bounds_max = Vec3::new(10.0, 0.0, 10.0);
    }
    {
        let ob = logic.host_object_mut(b).unwrap();
        ob.selection_radius = 10.0;
        ob.thing.geometry.radius = 10.0;
        ob.thing.geometry.bounds_min = Vec3::new(-10.0, 0.0, -10.0);
        ob.thing.geometry.bounds_max = Vec3::new(10.0, 0.0, 10.0);
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
fn formation_move_stamps_group_get_speed_factor() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for name in ["FM_FAST", "FM_SLOW"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(100.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let fast = logic
        .create_object("FM_FAST", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let slow = logic
        .create_object("FM_SLOW", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let f = logic.host_object_mut(fast).unwrap();
        f.movement.max_speed = 40.0;
        f.health.current = 100.0;
        f.health.maximum = 100.0;
        f.refresh_model_condition_bits();
    }
    {
        let s = logic.host_object_mut(slow).unwrap();
        s.movement.max_speed = 20.0;
        s.health.current = 100.0;
        s.health.maximum = 100.0;
        s.refresh_model_condition_bits();
    }
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_create_formation(&[fast, slow]),
        CommandResult::Success
    );
    assert_eq!(
        exec.execute_move_formation_to_position(&[fast, slow], Vec3::new(200.0, 0.0, 0.0)),
        CommandResult::Success
    );
    let ff = logic.host_object(fast).unwrap().group_speed_factor;
    let sf = logic.host_object(slow).unwrap().group_speed_factor;
    assert!(
        (ff - 0.5).abs() < 0.02,
        "fast unit must cap to group getSpeed, factor={ff}"
    );
    assert!(
        (sf - 1.0).abs() < 0.02,
        "slow unit stays at its max, factor={sf}"
    );
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
fn guard_uses_vision_radius_and_skips_inert_structures() {
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, GuardTarget};
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    for (name, kinds) in [
        ("GD_V", &[KindOf::Vehicle, KindOf::Selectable][..]),
        ("GD_S", &[KindOf::Structure, KindOf::Selectable][..]),
        ("GD_T", &[KindOf::Structure, KindOf::Selectable][..]),
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
    let t = logic
        .create_object("GD_T", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    {
        let u = logic.host_object_mut(v).unwrap();
        u.vision_range = 120.0;
        u.selection_radius = 10.0;
    }
    {
        let turret = logic.host_object_mut(t).unwrap();
        turret.vision_range = 200.0;
        turret.weapon = Some(Weapon {
            damage: 10.0,
            range: 200.0,
            ..Weapon::default()
        });
    }
    let pos = Vec3::new(50.0, 0.0, 0.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_guard(
                &[v, s, t],
                &GuardTarget::Position(pos),
                crate::game_logic::GuardMode::Normal
            ),
            CommandResult::Success
        );
    }
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
    // Inert structure (no leftover AI / weapon) must not enter guard.
    assert_ne!(
        logic.host_object(s).unwrap().ai_state,
        AIState::GuardingArea
    );
    // Turret / stinger-style structure still scans from the post.
    assert_eq!(
        logic.host_object(t).unwrap().ai_state,
        AIState::GuardingArea
    );
}

#[test]
fn group_guard_includes_stunned_member_that_cannot_move() {
    // C++ AIGroup::groupGuardPosition: AI interface only — stun/EMP does not skip.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, GuardTarget};
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("GD_STUN");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(200.0);
    logic.templates.insert("GD_STUN".to_string(), tpl);
    let id = logic
        .create_object("GD_STUN", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.shock_stun_frames = 40;
        u.vision_range = 100.0;
        assert!(
            !u.can_move(),
            "flailing stun must block can_move so the test is live"
        );
    }
    assert!(
        logic.host_unit_can_guard(id),
        "stunned vehicle still has AIUpdate analog"
    );
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_guard(
                &[id],
                &GuardTarget::Position(Vec3::new(80.0, 0.0, 0.0)),
                crate::game_logic::GuardMode::Normal
            ),
            CommandResult::Success
        );
    }
    let u = logic.host_object(id).unwrap();
    assert_eq!(u.ai_state, AIState::GuardingArea);
    assert!(
        !u.can_move(),
        "guard must not clear stun just to walk to the post"
    );
}

#[test]
fn script_team_guard_includes_turret_and_stunned() {
    // C++ ScriptActions::doTeamGuard: AIUpdateInterface only.
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let _ = gamelogic::scripting::take_host_script_hunt_guard_requests();
    let mut logic = GameLogic::new();
    logic.scripts_loaded = true;
    for (name, kinds) in [
        ("SG_V", &[KindOf::Vehicle, KindOf::Selectable][..]),
        ("SG_S", &[KindOf::Structure, KindOf::Selectable][..]),
        ("SG_T", &[KindOf::Structure, KindOf::Selectable][..]),
    ] {
        let mut tpl = ThingTemplate::new(name);
        for k in kinds {
            tpl.add_kind_of(*k);
        }
        tpl.set_health(400.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let v = logic
        .create_object("SG_V", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let stun = logic
        .create_object("SG_V", Team::USA, Vec3::new(8.0, 0.0, 0.0))
        .unwrap();
    let inert = logic
        .create_object("SG_S", Team::USA, Vec3::new(16.0, 0.0, 0.0))
        .unwrap();
    let turret = logic
        .create_object("SG_T", Team::USA, Vec3::new(24.0, 0.0, 0.0))
        .unwrap();
    for id in [v, stun, inert, turret] {
        if let Some(o) = logic.host_object_mut(id) {
            o.team_instance_name = "W29GuardTeam".into();
        }
    }
    {
        let u = logic.host_object_mut(stun).unwrap();
        u.shock_stun_frames = 40;
        assert!(!u.can_move());
    }
    {
        let t = logic.host_object_mut(turret).unwrap();
        t.weapon = Some(crate::game_logic::Weapon {
            damage: 10.0,
            range: 150.0,
            ..crate::game_logic::Weapon::default()
        });
    }
    gamelogic::scripting::request_host_script_hunt_guard(
        gamelogic::scripting::HostScriptHuntGuardRequest::TeamGuard {
            team: "W29GuardTeam".into(),
        },
    );
    crate::game_logic::evaluate_and_execute_scripts_for_test(&mut logic, 0.0);
    assert_eq!(
        logic.host_object(v).unwrap().ai_state,
        AIState::GuardingArea
    );
    assert_eq!(
        logic.host_object(stun).unwrap().ai_state,
        AIState::GuardingArea
    );
    assert_eq!(
        logic.host_object(turret).unwrap().ai_state,
        AIState::GuardingArea
    );
    assert_ne!(
        logic.host_object(inert).unwrap().ai_state,
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
fn sleep_mood_ai_rejects_hunt_guard_attack_move() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::host_strategy_center::HostAiAttitude;
    use crate::game_logic::{AIState, GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(1, Team::USA, "USA AI", false));
    let mut tpl = ThingTemplate::new("SleepScout");
    tpl.add_kind_of(KindOf::Vehicle);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("SleepScout".into(), tpl);
    let id = logic
        .create_object("SleepScout", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let u = logic.host_object_mut(id).unwrap();
        u.owner_player_id = Some(1);
        u.set_ai_attitude(HostAiAttitude::Sleep);
    }
    assert!(!logic.unit_command_patrol(id));
    assert!(!logic.unit_command_attack_move_to(id, Vec3::new(10.0, 0.0, 0.0)));
    assert!(!logic.unit_command_guard_full(
        id,
        Some(Vec3::ZERO),
        None,
        80.0,
        crate::game_logic::GuardMode::Normal
    ));
    let mut exec = CommandExecutor::new(&mut logic, 1);
    assert_eq!(exec.execute_patrol(&[id]), CommandResult::InvalidCommand);
    let u = logic.host_object(id).unwrap();
    assert_eq!(u.ai_state, AIState::Idle);
}

#[test]
fn forbid_player_commands_blocks_player_hunt_click() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SpectreLike");
    tpl.add_kind_of(KindOf::Aircraft);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(100.0);
    logic.templates.insert("SpectreLike".into(), tpl);
    let id = logic
        .create_object("SpectreLike", Team::USA, Vec3::ZERO)
        .unwrap();
    logic.host_object_mut(id).unwrap().forbid_player_commands = true;
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(exec.execute_patrol(&[id]), CommandResult::InvalidCommand);
    assert_eq!(logic.host_object(id).unwrap().ai_state, AIState::Idle);
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
fn sell_command_sells_every_selected_structure() {
    // C++ GameLogicDispatch.cpp:1450-1459 MSG_SELL → groupSell.
    use super::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType};
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    for name in ["GS_A", "GS_B"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.add_kind_of(KindOf::Structure);
        tpl.add_kind_of(KindOf::Selectable);
        tpl.set_health(500.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("GS_A", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .unwrap();
    let b = logic
        .create_object("GS_B", Team::USA, Vec3::new(20.0, 0.0, 0.0))
        .unwrap();
    let mut exec = CommandExecutor::new(&mut logic, 0);
    assert_eq!(
        exec.execute_command(dispatch_test_command(
            CommandType::Sell { object_id: a },
            0,
            vec![a, b],
        ))
        .expect("sell"),
        CommandResult::Success
    );
    for id in [a, b] {
        assert!(
            logic.is_object_being_sold(id)
                || logic
                    .host_object(id)
                    .map(|o| o.status.sold)
                    .unwrap_or(false),
            "hq-5mdst: every selected structure must enter sell"
        );
    }
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
fn post_fire_override_steers_live_beam_and_spectre_orbit() {
    // Given: a live Particle Uplink beam and Spectre orbit after fire.
    // When: a post-fire override click is issued through the live command.
    // Then: the next strike tick aims the beam and gunship at that click.
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("SP_Steer");
    tpl.add_kind_of(KindOf::Structure);
    tpl.add_kind_of(KindOf::Selectable);
    tpl.set_health(500.0);
    logic.templates.insert("SP_Steer".to_string(), tpl);
    let id = logic
        .create_object("SP_Steer", Team::USA, Vec3::ZERO)
        .unwrap();
    let fire_pos = Vec3::new(40.0, 0.0, 10.0);
    let click = Vec3::new(220.0, 0.0, 180.0);
    logic
        .special_power_strikes
        .spawn_beam_field(id, Team::USA, fire_pos, logic.frame, 1);
    logic
        .special_power_strikes
        .spawn_orbit_field(id, Team::USA, fire_pos, logic.frame, 2);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_override_special_power_destination(&[id], click),
            CommandResult::Success
        );
    }
    {
        let beam = logic
            .special_power_strikes
            .beam_fields()
            .iter()
            .find(|f| f.source_object == id)
            .expect("live beam");
        assert!(
            beam.manual_target_mode,
            "override command must arm PUC manual drive immediately"
        );
        assert!(
            (beam.override_destination.x - click.x).abs() < 0.01
                && (beam.override_destination.z - click.z).abs() < 0.01,
            "beam override {:?} != click {:?}",
            beam.override_destination,
            click
        );
        let orbit = logic
            .special_power_strikes
            .orbit_fields()
            .iter()
            .find(|f| f.source_object == id)
            .expect("live spectre orbit");
        assert!(
            (orbit.position.x - click.x).abs() < 0.01 && (orbit.position.z - click.z).abs() < 0.01,
            "spectre orbit {:?} != click {:?}",
            orbit.position,
            click
        );
    }
    logic.update_special_power_strikes();

    let beam = logic
        .special_power_strikes
        .beam_fields()
        .iter()
        .find(|f| f.source_object == id)
        .expect("live beam");
    assert!(
        beam.manual_target_mode,
        "post-fire click must arm PUC manual drive"
    );
    assert!(
        (beam.override_destination.x - click.x).abs() < 0.01
            && (beam.override_destination.z - click.z).abs() < 0.01,
        "beam override {:?} != click {:?}",
        beam.override_destination,
        click
    );

    let orbit = logic
        .special_power_strikes
        .orbit_fields()
        .iter()
        .find(|f| f.source_object == id)
        .expect("live spectre orbit");
    assert!(
        (orbit.position.x - click.x).abs() < 0.01 && (orbit.position.z - click.z).abs() < 0.01,
        "spectre orbit {:?} != click {:?}",
        orbit.position,
        click
    );
}

#[test]
fn spectre_gunship_click_steers_producer_orbit_and_override_target() {
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::host_spectre_gunship_update::{
        HostGunshipStatus, HostSpectreGunshipUpdateData,
    };
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut cc = ThingTemplate::new("AmericaCommandCenter");
    cc.add_kind_of(KindOf::Structure);
    cc.add_kind_of(KindOf::Selectable);
    cc.set_health(1000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), cc);
    let mut ship = ThingTemplate::new("AmericaSpectreGunship");
    ship.add_kind_of(KindOf::Aircraft);
    ship.add_kind_of(KindOf::Selectable);
    ship.set_health(500.0);
    logic
        .templates
        .insert("AmericaSpectreGunship".to_string(), ship);

    let caster = logic
        .create_object("AmericaCommandCenter", Team::USA, Vec3::ZERO)
        .unwrap();
    let gunship = logic
        .create_object(
            "AmericaSpectreGunship",
            Team::USA,
            Vec3::new(10.0, 80.0, 10.0),
        )
        .unwrap();
    let fire_pos = Vec3::new(80.0, 0.0, 20.0);
    let click = Vec3::new(300.0, 0.0, 250.0);
    {
        let g = logic.host_object_mut(gunship).unwrap();
        g.producer_id = Some(caster);
        g.spectre_gunship_update = Some(HostSpectreGunshipUpdateData::initiate_at(fire_pos));
    }
    logic
        .special_power_strikes
        .spawn_orbit_field(caster, Team::USA, fire_pos, logic.frame, 3);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_override_special_power_destination(&[gunship], click),
            CommandResult::Success
        );
    }
    let orbit = logic
        .special_power_strikes
        .orbit_fields()
        .iter()
        .find(|f| f.source_object == caster)
        .expect("caster orbit");
    assert!(
        (orbit.position.x - click.x).abs() < 0.01 && (orbit.position.z - click.z).abs() < 0.01,
        "producer orbit {:?} != click {:?}",
        orbit.position,
        click
    );
    let flight = logic
        .host_object(gunship)
        .and_then(|o| o.spectre_gunship_update.clone())
        .expect("gunship flight");
    assert_eq!(flight.status, HostGunshipStatus::Inserting);
    assert!(
        (flight.override_target.x - click.x).abs() < 0.01
            && (flight.override_target.z - click.z).abs() < 0.01,
        "gunship override_target {:?} != click {:?}",
        flight.override_target,
        click
    );
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
fn attack_ground_orders_locked_hive_slaves() {
    // C++ AIGroup::groupAttackPosition: !doSlavesHaveFreedom →
    // orderSlavesToAttackPosition before aiAttackPosition.
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::host_base_defense::init_stinger_hive_slave_roster;
    use crate::game_logic::{AIState, GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut site_t = ThingTemplate::new("HiveSite");
    site_t.add_kind_of(KindOf::Structure);
    site_t.add_kind_of(KindOf::Immobile);
    site_t.add_kind_of(KindOf::Selectable);
    site_t.add_kind_of(KindOf::Attackable);
    site_t.set_health(500.0);
    logic.templates.insert("HiveSite".to_string(), site_t);
    let site = logic
        .create_object("HiveSite", Team::GLA, Vec3::ZERO)
        .unwrap();
    {
        let s = logic.host_object_mut(site).unwrap();
        s.hive_slaves = init_stinger_hive_slave_roster();
        s.hive_slave_count = 3;
        s.weapon = Some(Weapon {
            damage: 20.0,
            range: 225.0,
            can_target_ground: true,
            ..Weapon::default()
        });
    }
    let dest = Vec3::new(40.0, 0.0, 12.0);
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(
            exec.execute_attack_ground(&[site], Some(dest), -1),
            CommandResult::Success
        );
    }
    let s = logic.host_object(site).unwrap();
    assert_eq!(s.ai_state, AIState::AttackingGround);
    assert!(
        s.hive_slaves.iter().filter(|sl| sl.alive).all(|sl| {
            sl.ai_attacking
                && sl.attack_target_id == 0
                && sl
                    .attack_ground
                    .is_some_and(|p| (p[0] - dest.x).abs() < 0.01 && (p[2] - dest.z).abs() < 0.01)
        }),
        "locked hive slaves must receive orderSlavesToAttackPosition"
    );
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
            exec.execute_guard_area(
                &[id],
                Vec3::new(30.0, 0.0, 0.0),
                150.0,
                GuardMode::Normal,
                None
            ),
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
