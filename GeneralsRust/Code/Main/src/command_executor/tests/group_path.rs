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
        assert_eq!(exec.execute_move(&[id], outside), CommandResult::Success);
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
    assert_eq!(
        goals.len(),
        6,
        "mixed group must destination-pack every member"
    );
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

    // SpySatellite residual inits the process-global shroud grid; drop it so
    // later tests see the fail-open uninitialized-grid path.
    let _shroud_isolation = crate::fow_rendering::shroud_test_isolation_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
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
    let sa = logic.host_object(a).unwrap().ai_state.clone();
    let sb = logic.host_object(b).unwrap().ai_state.clone();
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
    let transport = logic.create_object("CD_T", Team::USA, Vec3::ZERO).unwrap();
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
        logic.host_object(pax).unwrap().contained_by.is_some(),
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
        assert_eq!(exec.execute_evacuate(&[transport]), CommandResult::Success);
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
        logic.host_object(pax).unwrap().contained_by.is_some(),
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
    let humvee = logic.create_object("AT_H", Team::USA, Vec3::ZERO).unwrap();
    let rider = logic.create_object("AT_I", Team::USA, Vec3::ZERO).unwrap();
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
fn object_attack_skips_vehicle_transport_riders() {
    // C++ TransportContain::isPassengerAllowedToFire + isAbleToAttack:
    // vehicle riders never receive the group attack order.
    use super::CommandExecutor;
    use crate::command_system::CommandResult;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate, Weapon};
    use glam::Vec3;

    let mut logic = GameLogic::new();
    let mut chin = ThingTemplate::new("AT_CC");
    chin.add_kind_of(KindOf::Vehicle);
    chin.add_kind_of(KindOf::Aircraft);
    chin.add_kind_of(KindOf::Selectable);
    chin.set_health(300.0);
    logic.templates.insert("AT_CC".to_string(), chin);
    let mut veh = ThingTemplate::new("AT_V");
    veh.add_kind_of(KindOf::Vehicle);
    veh.add_kind_of(KindOf::Selectable);
    veh.set_health(200.0);
    logic.templates.insert("AT_V".to_string(), veh);
    let mut tgt = ThingTemplate::new("AT_VE");
    tgt.add_kind_of(KindOf::Vehicle);
    tgt.add_kind_of(KindOf::Selectable);
    tgt.set_health(100.0);
    logic.templates.insert("AT_VE".to_string(), tgt);
    let chinook = logic.create_object("AT_CC", Team::USA, Vec3::ZERO).unwrap();
    let rider = logic.create_object("AT_V", Team::USA, Vec3::ZERO).unwrap();
    let enemy = logic
        .create_object("AT_VE", Team::China, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    {
        let h = logic.host_object_mut(chinook).unwrap();
        h.install_combat_chinook_transport();
        let _ = h.add_occupant(rider);
        h.weapon = Some(Weapon {
            damage: 5.0,
            range: 80.0,
            ..Weapon::default()
        });
    }
    {
        let r = logic.host_object_mut(rider).unwrap();
        r.set_contained_by(Some(chinook));
        r.set_ai_state(crate::game_logic::AIState::Garrisoned);
        r.weapon = Some(Weapon {
            damage: 10.0,
            range: 80.0,
            ..Weapon::default()
        });
    }
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        let _ = exec.execute_attack(&[chinook], enemy);
    }
    let rider = logic.host_object(rider).unwrap();
    assert_ne!(
        rider.target,
        Some(enemy),
        "vehicle Combat Chinook rider must not receive the attack order"
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
    let site = logic.create_object("ST_B", Team::USA, Vec3::ZERO).unwrap();
    let occ = logic.create_object("ST_I", Team::USA, Vec3::ZERO).unwrap();
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
    let truck = logic.create_object("ST_HV", Team::USA, Vec3::ZERO).unwrap();
    let rider = logic.create_object("ST_RI", Team::USA, Vec3::ZERO).unwrap();
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
    assert!(
        r.target.is_none(),
        "Stop on transport must idle firing riders"
    );
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
