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
    let ga = logic.host_object(a).unwrap().requested_destination;
    let gb = logic.host_object(b).unwrap().requested_destination;
    assert_eq!(ga, Some(dest), "a goal {ga:?}");
    assert_eq!(gb, Some(dest), "b goal {gb:?}");
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
        o.set_geometry_radius(5.0);
        o.set_geometry_bounds_min(Vec3::new(-5.0, 0.0, -5.0));
        o.set_geometry_bounds_max(Vec3::new(5.0, 0.0, 5.0));
    }
    let before_a = logic.host_object(a).unwrap().get_position();
    {
        let mut exec = CommandExecutor::new(&mut logic, 0);
        assert_eq!(exec.execute_scatter(&[a, b]), CommandResult::Success);
    }
    let unit = logic.host_object(a).unwrap();
    let dest = unit
        .requested_destination
        .expect("scatter command destination");
    assert!(
        !unit.movement.path.is_empty(),
        "scatter installs a real path"
    );
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
    let unit = logic.host_object(id).unwrap();
    let dest = unit
        .requested_destination
        .expect("clamped command destination");
    let path_goal = unit.movement.path.last().copied().expect("clamped path");
    // AIPathfind::adjustCoordToCell quantizes the route, not the command.
    assert_eq!(
        logic.pathfinding_system.grid.world_to_grid(path_goal),
        logic.pathfinding_system.grid.world_to_grid(dest)
    );
    let margin = 4.0 * crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
    assert!(
        dest.x <= max.x - margin + 1.0 && dest.z <= max.z - margin + 1.0,
        "waypoint must clamp inside extent dest={dest:?} max={max:?} min={min:?}"
    );
}

#[test]
fn compute_ground_path_infantry_line_passable_fallback() {
    // AIGroup.cpp:590-611: below the distance/count threshold, every
    // infantry must have a passable line to the closest member at the center.
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use glam::Vec3;
    let mut logic = GameLogic::new();
    logic.set_ai_definition_base(game_engine::common::ini::AIData {
        min_distance_for_group: 100.0,
        distance_requires_group: 500.0,
        ..Default::default()
    });
    for (name, kind) in [
        ("LineVehicle", KindOf::Vehicle),
        ("LineInfantry", KindOf::Infantry),
    ] {
        let mut template = ThingTemplate::new(name);
        template.add_kind_of(kind).set_health(100.0);
        logic.templates.insert(name.into(), template);
    }
    let vehicle = logic
        .create_object("LineVehicle", Team::USA, Vec3::ZERO)
        .unwrap();
    let left = logic
        .create_object("LineInfantry", Team::USA, Vec3::new(-40.0, 0.0, 0.0))
        .unwrap();
    let right = logic
        .create_object("LineInfantry", Team::USA, Vec3::new(40.0, 0.0, 0.0))
        .unwrap();
    let members = [vehicle, left, right];
    let goal = Vec3::new(200.0, 0.0, 0.0);
    assert!(
        super::CommandExecutor::new(&mut logic, 0).compute_ground_path_should_group(&members, goal)
    );
    let obstacle = logic
        .pathfinding_system
        .grid
        .world_to_grid(Vec3::new(20.0, 0.0, 0.0));
    logic.pathfinding_system.grid.set_blocked(obstacle, true);
    assert!(
        !super::CommandExecutor::new(&mut logic, 0)
            .compute_ground_path_should_group(&members, goal),
        "one blocked infantry-to-center line rejects the shared path"
    );
    logic.pathfinding_system.grid.set_blocked(obstacle, false);
    let mut executor = super::CommandExecutor::new(&mut logic, 0);
    assert!(executor.compute_ground_path_should_group(&members, goal));
    assert_eq!(
        executor.execute_move(&members, goal),
        crate::command_system::CommandResult::Success
    );
    for id in members {
        assert!(
            !logic.host_object(id).unwrap().movement.path.is_empty(),
            "accepted clear route {id:?}"
        );
    }
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

fn evacuation_world(
    raw: f32,
    deck: Option<f32>,
) -> (
    crate::game_logic::GameLogic,
    crate::game_logic::ObjectId,
    crate::game_logic::ObjectId,
) {
    use crate::game_logic::{GameLogic, Team};
    use glam::Vec3;
    game_engine::common::ini::ini_locomotor::load_locomotors_from_str(
        r#"
Locomotor HqEvacChinookLocomotor
  Surfaces = AIR
  Speed = 100
  Acceleration = 100
  Appearance = HOVER
  ZAxisBehavior = RELATIVE_TO_HIGHEST_LAYER
  PreferredHeight = 10
End
"#,
    )
    .unwrap();
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object AmericaVehicleChinook
  KindOf = VEHICLE AIRCRAFT PRODUCED_AT_HELIPAD
  Geometry = CYLINDER
  GeometryMajorRadius = 8
  GeometryHeight = 12
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 200
  End
  Behavior = ChinookAIUpdate ModuleTag_AI
  End
  Behavior = TransportContain ModuleTag_Contain
    Slots = 8
    AllowInsideKindOf = INFANTRY VEHICLE
  End
  Locomotor = SET_NORMAL HqEvacChinookLocomotor
End
Object EvacPassenger
  KindOf = INFANTRY SELECTABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
  End
  Behavior = AIUpdateInterface ModuleTag_AI
  End
End
"#,
            "hq_evacuation.ini",
        )
        .unwrap();
    let mut world = GameLogic::new();
    for name in ["AmericaVehicleChinook", "EvacPassenger"] {
        world.templates.insert(
            name.into(),
            GameLogic::build_template_from_object_definition(
                name,
                parser.get_definition(name).unwrap(),
                None,
            ),
        );
    }
    let width = world.pathfinding_system.grid.width() as u32;
    let height = world.pathfinding_system.grid.height() as u32;
    assert!(world.restore_terrain_heights_from_grid(
        width,
        height,
        &vec![raw; (width * height) as usize]
    ));
    if let Some(deck) = deck {
        world.pathfinding_system.grid.stamp_bridge_deck(
            Vec3::new(-40.0, deck, -40.0),
            Vec3::new(40.0, deck, -40.0),
            Vec3::new(-40.0, deck, 40.0),
            Vec3::new(40.0, deck, 40.0),
            false,
        );
    }
    let transport = world
        .create_object(
            "AmericaVehicleChinook",
            Team::USA,
            Vec3::new(10.0, 80.0, 10.0),
        )
        .unwrap();
    let passenger = world
        .create_object("EvacPassenger", Team::USA, Vec3::new(10.0, 80.0, 10.0))
        .unwrap();
    {
        let unit = world.host_object_mut(transport).unwrap();
        assert!(
            unit.chinook_ai.is_some(),
            "real Chinook spawn binds its controller"
        );
        unit.status.airborne_target = true;
        unit.ground_height = raw;
        unit.ground_height_from_terrain = true;
        assert!(unit.add_occupant(passenger));
    }
    world
        .host_object_mut(passenger)
        .unwrap()
        .set_contained_by(Some(transport));
    (world, transport, passenger)
}

fn assert_airborne_evacuation(
    world: &mut crate::game_logic::GameLogic,
    id: crate::game_logic::ObjectId,
    passenger: crate::game_logic::ObjectId,
    expected_height: f32,
) {
    assert_eq!(
        super::CommandExecutor::new(world, 0).execute_evacuate(&[id]),
        crate::command_system::CommandResult::Success
    );
    let unit = world.host_object(id).unwrap();
    assert!(unit.pending_evacuate_on_stop, "unload waits for arrival");
    let ai = unit.chinook_ai.as_ref().unwrap();
    assert_eq!(
        ai.dest,
        [10.0, 10.0, expected_height],
        "CPP layer-height evacuation command"
    );
    assert_eq!(
        unit.requested_destination,
        Some(glam::Vec3::new(10.0, expected_height, 10.0))
    );
    assert!(
        !unit.movement.path.is_empty(),
        "aircraft receives a real descent route"
    );
    assert_eq!(world.host_object(passenger).unwrap().contained_by, Some(id));
}

#[test]
fn evacuate_airborne_uses_terrain_height_not_sea_level() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "evacuate_airborne_uses_terrain_height_not_sea_level",
        || {
            let (mut world, transport, passenger) = evacuation_world(15.0, None);
            assert_airborne_evacuation(&mut world, transport, passenger, 15.0);
        },
    );
}

#[test]
fn airborne_evacuation_uses_driving_highest_layer_and_valid_zero_ground() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "airborne_evacuation_uses_driving_highest_layer_and_valid_zero_ground",
        || {
            let (mut first, id, passenger) = evacuation_world(3.0, Some(21.0));
            let (mut other, other_id, other_passenger) = evacuation_world(7.0, Some(55.0));
            let (mut zero, zero_id, zero_passenger) = evacuation_world(0.0, None);
            zero.host_object_mut(zero_id).unwrap().ground_height = 99.0;
            assert_eq!(id, other_id);
            let foreign = gamelogic::system::engine_stores::new_for_world();
            let held = foreign.ai().write().unwrap();
            gamelogic::system::engine_stores::with_active_stores(&foreign, || {
                assert_airborne_evacuation(&mut first, id, passenger, 21.0);
                assert_airborne_evacuation(&mut other, other_id, other_passenger, 55.0);
                other.reset();
                assert_airborne_evacuation(&mut first, id, passenger, 21.0);
                // Zero is authored terrain, not a missing query. The poisoned
                // object observation must not substitute for the owned cache.
                assert_airborne_evacuation(&mut zero, zero_id, zero_passenger, 0.0);
            });
            drop(held);
        },
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

#[test]
fn movement_extent_uses_controlling_human_not_local_presentation() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "movement_extent_uses_controlling_human_not_local_presentation",
        || {
            use crate::game_logic::{GameLogic, GridPos, KindOf, Player, Team, ThingTemplate};
            use glam::Vec3;
            let mut world = GameLogic::new();
            world.override_world_size(200.0, 200.0);
            let mut human = Player::new(0, Team::USA, "RemoteHuman", true);
            human.is_local = false;
            let mut computer = Player::new(1, Team::China, "ViewedComputer", false);
            computer.is_local = true;
            world.add_player(human);
            world.add_player(computer);
            let mut template = ThingTemplate::new("ControllerExtentVehicle");
            template.add_kind_of(KindOf::Vehicle).set_health(100.0);
            world
                .templates
                .insert("ControllerExtentVehicle".into(), template);
            let from = world
                .pathfinding_system
                .grid
                .grid_to_world(GridPos::new(6, 6));
            let goal = world
                .pathfinding_system
                .grid
                .grid_to_world(GridPos::new(15, 6));
            let human_id = world
                .create_object_for_player("ControllerExtentVehicle", 0, from)
                .unwrap();
            let computer_id = world
                .create_object_for_player(
                    "ControllerExtentVehicle",
                    1,
                    world
                        .pathfinding_system
                        .grid
                        .grid_to_world(GridPos::new(6, 9)),
                )
                .unwrap();
            for id in [human_id, computer_id] {
                world.host_object_mut(id).unwrap().selection_radius = 1.0;
            }
            world.refresh_pathfind_ally_masks();
            world
                .pathfinding_system
                .grid
                .set_logical_extent(GridPos::new(5, 5), GridPos::new(10, 10));
            let human_path = world
                .pathfinding_system
                .find_path_ex(from, goal, &world.objects, false, Some(human_id))
                .expect("human request adjusts to reachable logical extent");
            let human_end = world
                .pathfinding_system
                .grid
                .world_to_grid(*human_path.last().unwrap());
            assert!(
                world.pathfinding_system.grid.in_logical_extent(human_end),
                "remote human must obey logical extent: {human_end:?}"
            );
            let computer_start = world.host_object(computer_id).unwrap().get_position();
            let computer_path = world
                .pathfinding_system
                .find_path_ex(
                    computer_start,
                    goal,
                    &world.objects,
                    false,
                    Some(computer_id),
                )
                .expect("computer request may use the border");
            let computer_end = world
                .pathfinding_system
                .grid
                .world_to_grid(*computer_path.last().unwrap());
            assert!(
                !world
                    .pathfinding_system
                    .grid
                    .in_logical_extent(computer_end),
                "locally viewed computer retains CPP border access: {computer_end:?}"
            );
            assert_eq!(computer_end, GridPos::new(15, 6));
            world.get_player_mut(0).unwrap().is_human = false;
            world.refresh_pathfind_ally_masks();
            let no_humans = world
                .pathfinding_system
                .find_path_ex(from, goal, &world.objects, false, Some(human_id))
                .expect("an explicitly admitted zero-human mask permits computer border access");
            let no_human_end = world
                .pathfinding_system
                .grid
                .world_to_grid(*no_humans.last().unwrap());
            assert_eq!(no_human_end, GridPos::new(15, 6));
            assert!(
                !world
                    .pathfinding_system
                    .grid
                    .in_logical_extent(no_human_end)
            );
            assert!(!world.get_player(0).unwrap().is_local);
            assert!(world.get_player(1).unwrap().is_local);
        },
    );
}
