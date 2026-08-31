use super::super::*;

fn open_grid(w: i32, h: i32) -> PathfindingGrid {
    PathfindingGrid::new(w as f32 * 10.0, h as f32 * 10.0, 10.0)
}

#[test]
fn host_astar_rejects_diagonal_corner_cut() {
    let mut g = open_grid(8, 8);
    // Block both ortho legs between (2,2) and (3,3)
    g.set_blocked(GridPos::new(3, 2), true);
    g.set_blocked(GridPos::new(2, 3), true);
    // Path from (2,2) to (3,3) cannot go diagonal through blocked legs.
    let path = g.find_path(GridPos::new(2, 2), GridPos::new(4, 4));
    assert!(path.is_some());
    // Ensure path does not step from (2,2) directly to (3,3)
    let cells: Vec<_> = path
        .unwrap()
        .into_iter()
        .map(|p| g.world_to_grid(p))
        .collect();
    for w in cells.windows(2) {
        let dx = (w[1].x - w[0].x).abs();
        let dy = (w[1].y - w[0].y).abs();
        if dx == 1 && dy == 1 {
            let ortho_a = GridPos::new(w[0].x + (w[1].x - w[0].x), w[0].y);
            let ortho_b = GridPos::new(w[0].x, w[0].y + (w[1].y - w[0].y));
            assert!(!g.is_static_blocked(ortho_a) && !g.is_static_blocked(ortho_b));
        }
    }
}

#[test]
fn host_march_closes_range_without_teleport() {
    use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(400.0, 400.0);
    let mut objects = HashMap::new();
    let tmpl = ThingTemplate::new("Ranger");
    let mut unit = Object::new(tmpl, ObjectId(1), Team::USA);
    let start = Vec3::new(20.0, 0.0, 20.0);
    let goal = Vec3::new(220.0, 0.0, 20.0);
    unit.set_position(start);
    unit.movement.max_speed = 20.0;
    objects.insert(unit.id, unit);

    let path = sys
        .find_path_ex(start, goal, &objects, false, Some(ObjectId(1)))
        .expect("open-field path");
    assert!(path.len() >= 2);
    {
        let u = objects.get_mut(&ObjectId(1)).unwrap();
        u.movement.path = path;
        u.movement.current_path_index = 0;
    }
    for _ in 0..400 {
        let _ = sys.move_unit_along_path(ObjectId(1), &mut objects, 1.0 / 30.0);
    }
    let end = objects[&ObjectId(1)].get_position();
    let dx = end.x - goal.x;
    let dz = end.z - goal.z;
    assert!(
        (dx * dx + dz * dz).sqrt() < 30.0,
        "unit must walk into range without set_position pull, end={end:?}"
    );
}

#[test]
fn host_astar_snaps_blocked_start_to_nearest_open() {
    let mut g = open_grid(12, 12);
    g.set_blocked(GridPos::new(2, 2), true);
    g.set_blocked(GridPos::new(1, 2), true);
    g.set_blocked(GridPos::new(2, 1), true);
    let path = g.find_path(GridPos::new(2, 2), GridPos::new(10, 10));
    assert!(
        path.is_some(),
        "blocked start must snap to a walkable cell like a blocked goal"
    );
    let first = g.world_to_grid(path.unwrap()[0]);
    assert!(!g.is_static_blocked(first));
}

#[test]
fn host_astar_soft_cost_dynamic_occupancy() {
    let mut g = open_grid(12, 12);
    // Wall of dynamic occupancy across middle — still pathable with surcharge.
    for y in 0..12 {
        g.set_dynamic_blocked(GridPos::new(5, y), true);
    }
    let path = g.find_path(GridPos::new(1, 5), GridPos::new(10, 5));
    assert!(path.is_some(), "dynamic occupancy must not hard-block path");
    assert!(path.unwrap().len() >= 2);
}

#[test]
fn host_astar_static_block_still_hard() {
    let mut g = open_grid(12, 12);
    for y in 0..12 {
        g.set_blocked(GridPos::new(5, y), true);
    }
    // Completely sealed — no path.
    let path = g.find_path(GridPos::new(1, 5), GridPos::new(10, 5));
    assert!(path.is_none());
}

/// C++ `Pathfinder::worldToGrid` / `REAL_TO_INT` (AIPathfind.h:856-858).
#[test]
fn world_to_grid_truncates_toward_zero_like_real_to_int() {
    let g = open_grid(20, 20);
    assert_eq!(
        g.world_to_grid(Vec3::new(19.9, 0.0, 5.0)),
        GridPos::new(1, 0),
        "19.9/10=1.99 → 1, 5/10=0.5 → 0 (round would be 2,1)"
    );
    assert_eq!(
        g.world_to_grid(Vec3::new(20.0, 0.0, 0.0)),
        GridPos::new(2, 0)
    );
    assert_eq!(
        g.world_to_grid(Vec3::new(-19.9, 0.0, -5.1)),
        GridPos::new(-1, 0)
    );
}

#[test]
fn compute_normal_radial_offset_xz_perpendicular() {
    let from = Vec3::new(0.0, 0.0, 0.0);
    let to = Vec3::new(100.0, 0.0, 0.0);
    let obj = Vec3::new(50.0, 0.0, 0.0);
    let p = PathfindingSystem::compute_normal_radial_offset_xz(from, to, obj, 10.0);
    // cross=0 uses fallback normal (1,0) or perpendicular — distance from obj ~ radius
    let d = ((p.x - obj.x).powi(2) + (p.z - obj.z).powi(2)).sqrt();
    assert!((d - 10.0).abs() < 0.01, "offset radius {d}");
}

#[test]
fn tall_building_aircraft_detour_inserts_waypoints() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("TallTower");
    tmpl.add_kind_of(KindOf::Structure);
    tmpl.add_kind_of(KindOf::AircraftPathAround);
    tmpl.add_kind_of(KindOf::Attackable);
    let mut bldg = Object::new(tmpl, ObjectId(1), Team::USA);
    bldg.set_position(Vec3::new(50.0, 0.0, 0.0));
    bldg.selection_radius = 25.0;
    objects.insert(bldg.id, bldg);

    let from = Vec3::new(0.0, 40.0, 0.0);
    let to = Vec3::new(100.0, 40.0, 0.0);
    let path = PathfindingSystem::detour_path_around_tall_buildings(&[from, to], &objects);
    assert!(
        path.len() > 2,
        "expected inserted tall-building waypoints, got {}",
        path.len()
    );
    // Path should not go through building center (within radius).
    for p in &path[1..path.len() - 1] {
        let d = ((p.x - 50.0).powi(2) + (p.z - 0.0).powi(2)).sqrt();
        // inserts are on the radius circle (~45)
        assert!(d + 1e-3 >= 20.0, "waypoint inside building d={d} at {p:?}");
    }
}

/// hq-1dbmo: hover/thrust still run leftover segmentIntersectsTallBuilding.
#[test]
fn hover_aircraft_detour_around_tall_building() {
    use crate::game_logic::{
        KindOf, LocomotorAppearance, Object, ObjectId, Team, ThingTemplate,
        host_combat_chinook::{HostChinookAI, HostChinookAIState},
    };
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("CommandCenter");
    tmpl.add_kind_of(KindOf::Structure);
    tmpl.add_kind_of(KindOf::AircraftPathAround);
    let mut bldg = Object::new(tmpl, ObjectId(1), Team::USA);
    bldg.set_position(Vec3::new(50.0, 0.0, 0.0));
    bldg.selection_radius = 25.0;
    objects.insert(bldg.id, bldg);

    let from = Vec3::new(0.0, 40.0, 0.0);
    let to = Vec3::new(100.0, 40.0, 0.0);
    let mut hover_t = ThingTemplate::new("AmericaChinook");
    hover_t.add_kind_of(KindOf::Aircraft);
    let mut hover = Object::new(hover_t, ObjectId(2), Team::USA);
    hover.loco_appearance = LocomotorAppearance::Hover;
    hover.set_position(from);
    objects.insert(hover.id, hover);
    let path = sys
        .find_path_ex_surfaces(
            from,
            to,
            &objects,
            true,
            SURFACE_AIR,
            false,
            Some(ObjectId(2)),
        )
        .expect("hover aircraft path");
    assert!(
        path.len() > 2,
        "hover/thrust must still insert tall-building detour, got {}",
        path.len()
    );

    let mut drop_t = ThingTemplate::new("CombatChinook");
    drop_t.add_kind_of(KindOf::Aircraft);
    let mut drop = Object::new(drop_t, ObjectId(3), Team::USA);
    drop.loco_appearance = LocomotorAppearance::Hover;
    drop.set_position(from);
    let mut ai = HostChinookAI::new_combat([0.0, 40.0, 0.0]);
    ai.state = HostChinookAIState::MoveToCombatDrop;
    ai.combat_drop_target = Some(1);
    drop.chinook_ai = Some(ai);
    objects.insert(drop.id, drop);
    let ignored = sys
        .find_path_ex_surfaces(
            from,
            to,
            &objects,
            true,
            SURFACE_AIR,
            false,
            Some(ObjectId(3)),
        )
        .expect("combat-drop path");
    assert_eq!(
        ignored.len(),
        2,
        "getBuildingToNotPathAround must skip the combat-drop goal"
    );
}

#[test]
fn tall_building_segment_intersect_cpp_surface() {
    let src = include_str!("../system_attack.rs");
    assert!(src.contains("segmentIntersectsTallBuilding"));
    assert!(src.contains("AIRCRAFT_PATH_AROUND"));
    assert!(src.contains("compute_normal_radial_offset_xz"));
    assert!(src.contains("find_path_ex"));
}

#[test]
fn circle_clips_tall_building_nudges_goal() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("TallCC");
    tmpl.add_kind_of(KindOf::Structure);
    tmpl.add_kind_of(KindOf::AircraftPathAround);
    let mut bldg = Object::new(tmpl, ObjectId(9), Team::USA);
    bldg.set_position(Vec3::new(0.0, 0.0, 0.0));
    bldg.selection_radius = 30.0;
    objects.insert(bldg.id, bldg);

    let from = Vec3::new(-100.0, 50.0, 0.0);
    let to = Vec3::new(5.0, 50.0, 0.0); // inside building footprint
    let adj = PathfindingSystem::circle_clips_tall_building(from, to, 80.0, &objects, None)
        .expect("must clip");
    let d = (adj.x * adj.x + adj.z * adj.z).sqrt();
    // leftover/C++: bounding circle + 2 cells, then perpendicular offset
    assert!(
        d >= 45.0,
        "adjusted goal still inside building d={d} adj={adj:?}"
    );
}

#[test]
fn circle_clips_cpp_surface() {
    let src = include_str!("../system_attack.rs");
    assert!(src.contains("circleClipsTallBuilding"));
    assert!(src.contains("circle_clips_tall_building"));
}

/// C++ PathfindCell::CellType (AIPathfind.h:233-242) on the live host grid.
#[test]
fn host_grid_classifies_water_cliff_impassable() {
    let mut g = open_grid(8, 8);
    g.set_cell_type(GridPos::new(2, 2), PathfindCellType::Water);
    g.set_cell_type(GridPos::new(3, 3), PathfindCellType::Cliff);
    g.set_cell_type(GridPos::new(4, 4), PathfindCellType::Impassable);
    assert_eq!(g.cell_type(GridPos::new(2, 2)), PathfindCellType::Water);
    assert_eq!(g.cell_type(GridPos::new(3, 3)), PathfindCellType::Cliff);
    assert_eq!(
        g.cell_type(GridPos::new(4, 4)),
        PathfindCellType::Impassable
    );
    assert!(
        !g.is_static_blocked(GridPos::new(2, 2)),
        "water is not hard-blocked"
    );
    assert!(
        !g.is_static_blocked(GridPos::new(3, 3)),
        "cliff is not hard-blocked"
    );
    assert!(g.is_static_blocked(GridPos::new(4, 4)));
    let path = g.find_path(GridPos::new(0, 0), GridPos::new(7, 7));
    assert!(
        path.is_some(),
        "water/cliff must stay walkable for ground A*"
    );
}

/// C++ Pathfinder::classifyMapCell (AIPathfind.cpp:4491-4521):
/// cliff at top-left, water if any of 4 corners — water wins. No slope gate.
#[test]
fn classify_map_cell_water_wins_over_cliff_no_slope_gate() {
    use PathfindCellType::*;
    assert_eq!(PathfindingGrid::classify_map_cell(false, false), Clear);
    assert_eq!(PathfindingGrid::classify_map_cell(true, false), Cliff);
    assert_eq!(PathfindingGrid::classify_map_cell(false, true), Water);
    assert_eq!(
        PathfindingGrid::classify_map_cell(true, true),
        Water,
        "C++ assigns water after cliff so wet cliff-base stays SURFACE_WATER"
    );
    let src = concat!(
        include_str!("../../world_save.rs"),
        include_str!("../../world_save/world_subsystems.rs"),
        include_str!("../../world_save/world_paths.rs"),
        include_str!("../../world_save/world_runtime.rs"),
        include_str!("../../world_save/world_players.rs"),
        include_str!("../../world_save/world_load.rs"),
    );
    assert!(
        !src.contains("MAX_SLOPE"),
        "live seed_pathfinding_from_terrain must not slope-gate Impassable"
    );
    assert!(
        src.contains("is_underwater_at_world(tl)")
            && src.contains("classify_map_cell(cliff, water)"),
        "live seed must sample four corners then classify_map_cell"
    );
}

/// Live find_path_ex must call crate AStarPathfinder (AIPathfind.cpp:6438).
#[test]
fn live_find_path_ex_uses_crate_astar() {
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let objects = HashMap::new();
    let path = sys
        .find_path_ex(
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(80.0, 0.0, 10.0),
            &objects,
            false,
            None,
        )
        .expect("crate A* open-field path");
    assert!(path.len() >= 2);
    assert!(
        sys.crate_astar.is_some(),
        "crate A* must be wired after first search"
    );
    sys.grid
        .set_cell_type(GridPos::new(5, 1), PathfindCellType::Water);
    let wet = sys
        .find_path_ex(
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(80.0, 0.0, 10.0),
            &objects,
            false,
            None,
        )
        .expect("water-costed crate path");
    assert!(wet.len() >= 2);
    assert_eq!(
        sys.grid.cell_type(GridPos::new(5, 1)),
        PathfindCellType::Water
    );
}

/// C++ Pathfinder::queueForPath / processPathfindQueue (AI.cpp:332-339).
#[test]
fn host_path_queue_defers_until_taken() {
    let mut sys = PathfindingSystem::new(100.0, 100.0);
    sys.queue_path(PendingHostPath {
        unit_id: ObjectId(1),
        start: Vec3::ZERO,
        destination: Vec3::new(50.0, 0.0, 0.0),
        waypoints: Vec::new(),
        aircraft: false,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        ignore_obstacle: None,
    });
    assert_eq!(sys.pending_path_count(), 1);
    let drained = sys.take_pending_paths();
    assert_eq!(drained.len(), 1);
    assert_eq!(sys.pending_path_count(), 0);
}

/// Live assign_unit_path queues when map_loaded (AI.cpp:332-339).
#[test]
fn assign_unit_path_queues_until_next_update() {
    use crate::game_logic::{GameLogic, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    tmpl.add_kind_of(KindOf::Attackable);
    logic.templates.insert("Ranger".into(), tmpl);
    let id = logic
        .create_object("Ranger", Team::USA, Vec3::new(10.0, 0.0, 10.0))
        .expect("ranger");
    if let Some(u) = logic.host_object_mut(id) {
        u.movement.max_speed = 20.0;
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.assign_unit_path(id, Vec3::new(80.0, 0.0, 10.0), &[]));
    let unit = logic.host_object(id).expect("unit");
    assert!(
        unit.waiting_for_path,
        "C++ m_waitingForPath until next frame"
    );
    assert!(
        unit.movement.path.is_empty(),
        "waypoints must not land same frame"
    );
    logic.update();
    let unit = logic.host_object(id).expect("unit after queue");
    assert!(!unit.waiting_for_path);
    assert!(
        !unit.movement.path.is_empty(),
        "processPathfindQueue must install crate A* path"
    );
}

#[test]
fn assign_shared_group_paths_uses_one_spine() {
    use crate::game_logic::{GameLogic, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.force_map_loaded_for_path_test(false);
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    logic.templates.insert("Ranger".into(), tmpl);
    let a = logic
        .create_object("Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("a");
    let b = logic
        .create_object("Ranger", Team::USA, Vec3::new(10.0, 0.0, 0.0))
        .expect("b");
    for id in [a, b] {
        if let Some(u) = logic.host_object_mut(id) {
            u.movement.max_speed = 20.0;
        }
    }
    let dest = Vec3::new(80.0, 0.0, 0.0);
    let goals = vec![(a, dest), (b, dest + Vec3::new(0.0, 0.0, 10.0))];
    assert!(logic.assign_shared_group_paths(&goals, dest));
    let pa = logic.host_object(a).unwrap().movement.path.clone();
    let pb = logic.host_object(b).unwrap().movement.path.clone();
    assert!(!pa.is_empty() && !pb.is_empty());
    assert_eq!(
        pa.last().copied().unwrap(),
        dest,
        "leader last waypoint is destination"
    );
    assert_eq!(
        pb.last().copied().unwrap().z,
        10.0,
        "follower last waypoint is slot"
    );
}

#[test]
fn cliff_pinch_converts_clear_neighbors() {
    let mut g = open_grid(8, 8);
    g.set_cell_type(GridPos::new(4, 4), PathfindCellType::Cliff);
    g.pinch_tighten_cliffs();
    assert_eq!(g.cell_type(GridPos::new(4, 5)), PathfindCellType::Cliff);
    assert!(
        g.is_pinched(GridPos::new(4, 6))
            || g.cell_type(GridPos::new(4, 5)) == PathfindCellType::Cliff
    );
}

#[test]
fn terrain_zones_split_on_water() {
    let mut g = open_grid(8, 8);
    for y in 0..8 {
        g.set_cell_type(GridPos::new(4, y), PathfindCellType::Water);
    }
    g.rebuild_terrain_zones();
    let a = g.terrain_zone(GridPos::new(1, 4));
    let b = g.terrain_zone(GridPos::new(6, 4));
    assert_ne!(a, 0);
    assert_ne!(b, 0);
    assert_ne!(a, b, "water must split terrain zones");
    let from = g.grid_to_world(GridPos::new(1, 4));
    let to = g.grid_to_world(GridPos::new(6, 4));
    assert!(!g.quick_path_exists_for_ui(from, to));
}

#[test]
fn terrain_zones_ignore_structure_obstacles() {
    let mut g = open_grid(8, 8);
    for y in 0..8 {
        g.set_cell_type(GridPos::new(4, y), PathfindCellType::Obstacle);
    }
    g.rebuild_terrain_zones();
    let from = g.grid_to_world(GridPos::new(1, 4));
    let to = g.grid_to_world(GridPos::new(6, 4));
    assert!(g.quick_path_exists_for_ui(from, to));
}

#[test]
fn bridge_deck_classifies_layer_not_flatten() {
    // C++ PathfindLayer: deck CLEAR on its layer, river under stays Water,
    // sides BRIDGE_IMPASSABLE, destroy closes the layer only.
    let mut g = open_grid(12, 12);
    for y in 4..=6 {
        for x in 3..8 {
            g.set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
        }
    }
    let from_l = Vec3::new(20.0, 20.0, 40.0);
    let from_r = Vec3::new(20.0, 20.0, 60.0);
    let to_l = Vec3::new(90.0, 20.0, 40.0);
    let to_r = Vec3::new(90.0, 20.0, 60.0);
    g.stamp_bridge_deck(from_l, from_r, to_l, to_r, false);
    let layer = g.first_bridge_layer_id().expect("bridge layer");
    let deck = GridPos::new(5, 5);
    assert_eq!(
        g.cell_type(deck),
        PathfindCellType::Water,
        "must not flatten deck onto ground"
    );
    assert_eq!(
        g.layer_cell_type(layer, deck),
        Some(PathfindCellType::Clear)
    );
    let side = GridPos::new(5, 3);
    assert_eq!(
        g.layer_cell_type(layer, side),
        Some(PathfindCellType::BridgeImpassable)
    );
    g.stamp_bridge_deck(from_l, from_r, to_l, to_r, true);
    assert_eq!(
        g.cell_type(deck),
        PathfindCellType::Water,
        "destroyed deck must not slab the river"
    );
    assert_eq!(
        g.layer_cell_type(layer, deck),
        Some(PathfindCellType::BridgeImpassable)
    );
    assert_eq!(g.ground_connect_layer(deck), 0);
}

#[test]
fn low_overpass_stamps_ground_bridge_impassable() {
    let mut g = open_grid(12, 12);
    g.stamp_bridge_deck(
        Vec3::new(20.0, 0.0, 40.0),
        Vec3::new(20.0, 0.0, 60.0),
        Vec3::new(90.0, 0.0, 40.0),
        Vec3::new(90.0, 0.0, 60.0),
        false,
    );
    assert_eq!(
        g.cell_type(GridPos::new(5, 5)),
        PathfindCellType::BridgeImpassable
    );
    assert!(g.is_static_blocked(GridPos::new(5, 5)));
}

#[test]
fn classified_bridge_layer_hops_across_river() {
    let mut sys = PathfindingSystem::new(120.0, 120.0);
    for y in 0..12 {
        for x in 3..8 {
            sys.grid
                .set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
        }
    }
    sys.grid.stamp_bridge_deck(
        Vec3::new(20.0, 20.0, 40.0),
        Vec3::new(20.0, 20.0, 60.0),
        Vec3::new(90.0, 20.0, 40.0),
        Vec3::new(90.0, 20.0, 60.0),
        false,
    );
    let objects = HashMap::new();
    let from = sys.grid.grid_to_world(GridPos::new(1, 5));
    let to = sys.grid.grid_to_world(GridPos::new(10, 5));
    let path = sys.find_path(from, to, &objects);
    assert!(
        path.as_ref().map(|p| p.len() >= 2).unwrap_or(false),
        "crate A* must hop the classified deck across water"
    );
}

/// hq-z66hi: rally/dozer zone gate must honor bridge connectLayer.
#[test]
fn connect_layer_merges_zones_across_river() {
    let mut g = open_grid(12, 12);
    for y in 0..12 {
        for x in 3..8 {
            g.set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
        }
    }
    g.stamp_bridge_deck(
        Vec3::new(20.0, 20.0, 40.0),
        Vec3::new(20.0, 20.0, 60.0),
        Vec3::new(90.0, 20.0, 40.0),
        Vec3::new(90.0, 20.0, 60.0),
        false,
    );
    g.rebuild_terrain_zones();
    g.rebuild_path_zones();
    let from = g.grid_to_world(GridPos::new(1, 5));
    let to = g.grid_to_world(GridPos::new(10, 5));
    assert!(
        g.quick_path_exists(from, to),
        "clientSafeQuickDoesPathExist must join banks via connectLayer"
    );
    assert!(
        g.quick_path_exists_for_ui(from, to),
        "ForUI effectiveTerrainZone still applies hierarchical bridge merge"
    );
    g.stamp_bridge_deck(
        Vec3::new(20.0, 20.0, 40.0),
        Vec3::new(20.0, 20.0, 60.0),
        Vec3::new(90.0, 20.0, 40.0),
        Vec3::new(90.0, 20.0, 60.0),
        true,
    );
    g.rebuild_terrain_zones();
    g.rebuild_path_zones();
    assert!(
        !g.quick_path_exists(from, to),
        "destroyed deck must drop ground connect and split banks"
    );
}

/// hq-nya50: findBrokenBridge returns the destroyed span that joins the banks.
#[test]
fn find_broken_bridge_returns_destroyed_connecting_span() {
    let mut g = open_grid(12, 12);
    for y in 0..12 {
        for x in 3..8 {
            g.set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
        }
    }
    let from_l = Vec3::new(20.0, 20.0, 40.0);
    let from_r = Vec3::new(20.0, 20.0, 60.0);
    let to_l = Vec3::new(90.0, 20.0, 40.0);
    let to_r = Vec3::new(90.0, 20.0, 60.0);
    g.stamp_bridge_deck(from_l, from_r, to_l, to_r, false);
    g.bind_bridge_layer_object_id(from_l, from_r, to_l, to_r, 42);
    g.rebuild_terrain_zones();
    g.rebuild_path_zones();
    let from = g.grid_to_world(GridPos::new(1, 5));
    let to = g.grid_to_world(GridPos::new(10, 5));
    assert!(g.find_broken_bridge(from, to).is_none());

    g.stamp_bridge_deck(from_l, from_r, to_l, to_r, true);
    g.bind_bridge_layer_object_id(from_l, from_r, to_l, to_r, 42);
    g.rebuild_terrain_zones();
    g.rebuild_path_zones();
    assert_eq!(
        g.find_broken_bridge(from, to),
        Some(ObjectId(42)),
        "destroyed layer that connects the two banks must return the span id"
    );
    assert!(
        g.find_broken_bridge(from, from).is_none(),
        "same-zone hop must not pick a bridge"
    );
}

/// hq-54w0z: dest on a deck stays on the deck (not snapped to the riverbank).
#[test]
fn adjust_destination_keeps_bridge_deck() {
    let mut g = open_grid(12, 12);
    for y in 4..=6 {
        for x in 3..8 {
            g.set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
        }
    }
    g.stamp_bridge_deck(
        Vec3::new(20.0, 20.0, 40.0),
        Vec3::new(20.0, 20.0, 60.0),
        Vec3::new(90.0, 20.0, 40.0),
        Vec3::new(90.0, 20.0, 60.0),
        false,
    );
    let dest = GridPos::new(5, 5);
    let world = g.grid_to_world(dest);
    let on_deck = Vec3::new(world.x, 20.0, world.z);
    let layer = g.layer_for_destination(on_deck);
    assert_ne!(
        layer,
        PathfindLayerEnum::Ground,
        "deck click must pick the span"
    );
    let snapped = g
        .adjust_destination_on_layer(dest, SURFACE_GROUND, false, 400, None, 0, layer)
        .expect("deck dest");
    assert_eq!(snapped, dest, "must not spiral off the Clear deck cell");
    let bank = g.adjust_destination_ex(dest, SURFACE_GROUND, false, 400, None, 0);
    assert_ne!(
        bank,
        Some(dest),
        "ground-only adjust still refuses Water under the span"
    );
}

/// hq-tg3cs: deck traffic must not stamp the roadbed under the span.
#[test]
fn deck_occupancy_does_not_block_roadbed() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut g = open_grid(12, 12);
    g.stamp_bridge_deck(
        Vec3::new(20.0, 20.0, 40.0),
        Vec3::new(20.0, 20.0, 60.0),
        Vec3::new(90.0, 20.0, 40.0),
        Vec3::new(90.0, 20.0, 60.0),
        false,
    );
    let deck = GridPos::new(5, 5);
    let world = g.grid_to_world(deck);
    let layer = g.layer_for_destination(Vec3::new(world.x, 20.0, world.z));
    assert_ne!(layer, PathfindLayerEnum::Ground);

    let mut objects = HashMap::new();
    let mut deck_t = ThingTemplate::new("Humvee");
    deck_t.add_kind_of(KindOf::Vehicle);
    let mut on_deck = Object::new(deck_t, ObjectId(10), Team::USA);
    on_deck.set_position(Vec3::new(world.x, 20.0, world.z));
    on_deck.selection_radius = 1.0;
    on_deck.owner_player_id = Some(1);
    objects.insert(on_deck.id, on_deck);
    g.update_dynamic_obstacles(&objects);

    assert!(
        !g.is_blocked(deck),
        "mid-span deck unit must not set ground dynamic_bits"
    );
    g.query_layer = PathfindLayerEnum::Ground as u8;
    assert_eq!(
        g.occupancy_extra_cost(deck, Some(0), false, 0),
        0,
        "roadbed under the span must ignore deck occupancy"
    );
    g.query_layer = layer as u8;
    assert_eq!(
        g.occupancy_extra_cost(deck, Some(0), false, 0),
        u32::MAX / 8,
        "same XZ on the deck still occupies the layer"
    );

    let mut ground_t = ThingTemplate::new("Battlemaster");
    ground_t.add_kind_of(KindOf::Vehicle);
    let mut on_ground = Object::new(ground_t, ObjectId(11), Team::China);
    on_ground.set_position(Vec3::new(world.x, 0.0, world.z));
    on_ground.selection_radius = 1.0;
    on_ground.owner_player_id = Some(2);
    objects.insert(on_ground.id, on_ground);
    g.update_dynamic_obstacles(&objects);

    g.query_layer = PathfindLayerEnum::Ground as u8;
    assert_eq!(
        g.occupancy_extra_cost(deck, Some(0), false, 0),
        u32::MAX / 8,
        "ground unit still occupies the roadbed"
    );
    g.query_layer = layer as u8;
    assert_eq!(
        g.occupancy_extra_cost(deck, Some(0), false, 0),
        u32::MAX / 8,
        "deck unit still occupies the layer after ground stamp"
    );
}

/// hq-a79xb: own reservation accepted; allied other player refused; enemy accepted.
#[test]
fn has_allied_goal_own_vs_ally_player() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut g = open_grid(12, 12);
    let mut masks = [0u16; 16];
    masks[0] = 1u16 << 0 | 1u16 << 1;
    masks[1] = 1u16 << 0 | 1u16 << 1;
    g.set_player_ally_masks(masks);

    // Default infantry selection_radius 8.0 -> radius_and_center gives a 3x3
    // goal footprint (AIPathfind.cpp:9766-9796), and C++ setGoalUnit
    // (:1302-1337) keeps ONE m_goalUnitID per cell (last writer wins).
    // Keep the three goals' footprints disjoint or the later stamp evicts the
    // earlier goal identity — authentic C++ behavior, not a port bug.
    let mut objects = HashMap::new();
    let mut own_t = ThingTemplate::new("Ranger");
    own_t.add_kind_of(KindOf::Infantry);
    let mut own = Object::new(own_t, ObjectId(10), Team::USA);
    own.set_position(g.grid_to_world(GridPos::new(2, 2)));
    own.owner_player_id = Some(0);
    own.movement.target_position = Some(g.grid_to_world(GridPos::new(4, 4)));
    objects.insert(own.id, own);

    let mut ally_t = ThingTemplate::new("RedGuard");
    ally_t.add_kind_of(KindOf::Infantry);
    let mut ally = Object::new(ally_t, ObjectId(20), Team::China);
    ally.set_position(g.grid_to_world(GridPos::new(10, 1)));
    ally.owner_player_id = Some(1);
    ally.movement.target_position = Some(g.grid_to_world(GridPos::new(8, 2)));
    objects.insert(ally.id, ally);

    let mut enemy_t = ThingTemplate::new("Rebel");
    enemy_t.add_kind_of(KindOf::Infantry);
    let mut enemy = Object::new(enemy_t, ObjectId(30), Team::GLA);
    enemy.set_position(g.grid_to_world(GridPos::new(10, 10)));
    enemy.owner_player_id = Some(2);
    enemy.movement.target_position = Some(g.grid_to_world(GridPos::new(8, 9)));
    objects.insert(enemy.id, enemy);

    g.update_dynamic_obstacles(&objects);
    g.query_seeker_id = 10;
    assert!(
        !g.has_allied_goal(GridPos::new(4, 4), Some(0)),
        "own UNIT_GOAL must be accepted"
    );
    assert!(
        g.has_allied_goal(GridPos::new(8, 2), Some(0)),
        "allied other-player goal must be refused"
    );
    assert!(
        !g.has_allied_goal(GridPos::new(8, 9), Some(0)),
        "enemy UNIT_GOAL is not allied"
    );
}

/// hq-7qvsn: no enemy-moving cost; ally moving only near start; no goal cost.
#[test]
fn occupancy_cost_matches_examine_neighboring_cells() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut g = open_grid(24, 24);
    let mut masks = [0u16; 16];
    masks[0] = 1u16 << 0 | 1u16 << 1;
    masks[1] = 1u16 << 0 | 1u16 << 1;
    g.set_player_ally_masks(masks);

    let mut objects = HashMap::new();
    let mut enemy_t = ThingTemplate::new("Technical");
    enemy_t.add_kind_of(KindOf::Vehicle);
    let mut enemy = Object::new(enemy_t, ObjectId(1), Team::GLA);
    enemy.set_position(g.grid_to_world(GridPos::new(5, 5)));
    enemy.owner_player_id = Some(2);
    enemy.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    objects.insert(enemy.id, enemy);

    let mut ally_t = ThingTemplate::new("Humvee");
    ally_t.add_kind_of(KindOf::Vehicle);
    let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
    ally.set_position(g.grid_to_world(GridPos::new(6, 6)));
    ally.owner_player_id = Some(1);
    ally.movement.velocity = Vec3::new(5.0, 0.0, 0.0);
    objects.insert(ally.id, ally);

    let mut goal_t = ThingTemplate::new("Ranger");
    goal_t.add_kind_of(KindOf::Infantry);
    let mut goaled = Object::new(goal_t, ObjectId(3), Team::USA);
    goaled.set_position(g.grid_to_world(GridPos::new(1, 1)));
    goaled.owner_player_id = Some(0);
    goaled.movement.target_position = Some(g.grid_to_world(GridPos::new(8, 8)));
    objects.insert(goaled.id, goaled);

    g.update_dynamic_obstacles(&objects);
    let start = GridPos::new(5, 5);
    let enemy_cell = GridPos::new(5, 5);
    let ally_near = GridPos::new(6, 6);
    let ally_far = GridPos::new(6, 6);
    let far_start = GridPos::new(20, 20);
    let goal_cell = GridPos::new(8, 8);

    assert_eq!(
        g.occupancy_extra_cost(enemy_cell, Some(0), false, 0),
        0,
        "moving enemy adds no examineNeighboringCells cost"
    );
    let near = g.occupancy_cost(ally_near, Some(0), false, 0, masks[0], Some(start));
    assert!(
        near.unwrap_or(0.0) > 0.0,
        "allied mover within 10 cells of start is charged"
    );
    let far = g.occupancy_cost(ally_far, Some(0), false, 0, masks[0], Some(far_start));
    assert_eq!(
        far,
        Some(0.0),
        "allied mover more than 10 cells from start is free"
    );
    let goal_cost = g.occupancy_cost(goal_cell, Some(0), false, 0, masks[0], Some(start));
    assert_eq!(
        goal_cost,
        Some(0.0),
        "UNIT_GOAL is not a movement-path surcharge"
    );
}

#[test]
fn occupancy_radius_marks_overlord_footprint() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut g = open_grid(16, 16);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("Overlord");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tmpl, ObjectId(1), Team::China);
    tank.set_position(Vec3::new(50.0, 0.0, 50.0));
    tank.selection_radius = 25.0;
    tank.owner_player_id = Some(1);
    objects.insert(tank.id, tank);
    g.update_dynamic_obstacles(&objects);
    let center = g.world_to_grid(Vec3::new(50.0, 0.0, 50.0));
    assert!(g.is_blocked(center));
    assert!(g.is_blocked(GridPos::new(center.x + 2, center.y)));
}

/// hq-0xpfm: C++ setGoalAircraft + adjustToLandingDestination.
#[test]
fn aircraft_goals_and_landing_dest_unstack() {
    use crate::game_logic::{KindOf, Object, ObjectId, ObjectType, Team, ThingTemplate};
    let mut g = open_grid(16, 16);
    let dest = Vec3::new(85.0, 40.0, 85.0);
    let dest_cell = g.world_to_grid(dest);
    let mut objects = HashMap::new();
    let mut tmpl_a = ThingTemplate::new("AmericaVehicleChinook");
    tmpl_a.add_kind_of(KindOf::Aircraft);
    tmpl_a.add_kind_of(KindOf::Vehicle);
    let mut a = Object::new(tmpl_a, ObjectId(11), Team::USA);
    a.object_type = ObjectType::Aircraft;
    a.loco_appearance = LocomotorAppearance::Hover;
    a.status.airborne_target = true;
    a.set_position(Vec3::new(20.0, 40.0, 20.0));
    a.movement.target_position = Some(dest);
    a.owner_player_id = Some(0);
    objects.insert(a.id, a);
    g.update_dynamic_obstacles(&objects);
    assert_eq!(
        g.goal_aircraft(dest_cell),
        11,
        "first chinook stamps goalAircraft"
    );

    let mut tmpl_b = ThingTemplate::new("AmericaVehicleChinook");
    tmpl_b.add_kind_of(KindOf::Aircraft);
    tmpl_b.add_kind_of(KindOf::Vehicle);
    let mut b = Object::new(tmpl_b, ObjectId(12), Team::USA);
    b.object_type = ObjectType::Aircraft;
    b.loco_appearance = LocomotorAppearance::Hover;
    b.status.airborne_target = true;
    b.set_position(Vec3::new(30.0, 40.0, 20.0));
    b.movement.target_position = Some(dest);
    b.owner_player_id = Some(0);
    objects.insert(b.id, b);
    g.update_dynamic_obstacles(&objects);
    g.query_seeker_id = 12;
    g.query_check_for_aircraft = true;
    let snapped = g
        .adjust_destination_on_layer(
            dest_cell,
            SURFACE_AIR,
            false,
            400,
            Some(0),
            0,
            PathfindLayerEnum::Ground,
        )
        .expect("second LZ");
    g.query_check_for_aircraft = false;
    assert_ne!(
        snapped, dest_cell,
        "second aircraft dest must leave first LZ"
    );
    assert!(!g.has_other_aircraft_goal(snapped));

    let water = GridPos::new(4, 4);
    g.set_cell_type(water, PathfindCellType::Water);
    let land = g
        .adjust_to_landing_destination(water, 400, PathfindLayerEnum::Ground)
        .expect("landing dest");
    assert_ne!(g.cell_type(land), PathfindCellType::Water);
    assert_ne!(
        land, dest_cell,
        "landing dest refuses occupied aircraft goal"
    );
}

/// hq-sa4qi: HOVER attack dests spiral so two Comanches do not share a cell.
#[test]
fn aircraft_attack_dests_spiral_off_occupied_hover_cell() {
    use crate::game_logic::{KindOf, Object, ObjectId, ObjectType, Team, ThingTemplate};
    let sys = PathfindingSystem::new(400.0, 400.0);
    let target = Vec3::new(200.0, 0.0, 200.0);
    let dest = Vec3::new(110.0, 40.0, 200.0);
    let mut objects = HashMap::new();
    let mut tmpl_a = ThingTemplate::new("AmericaVehicleComanche");
    tmpl_a.add_kind_of(KindOf::Aircraft);
    tmpl_a.add_kind_of(KindOf::Vehicle);
    let mut a = Object::new(tmpl_a, ObjectId(21), Team::USA);
    a.object_type = ObjectType::Aircraft;
    a.loco_appearance = LocomotorAppearance::Hover;
    a.status.airborne_target = true;
    a.set_position(Vec3::new(20.0, 40.0, 200.0));
    a.movement.target_position = Some(dest);
    a.selection_radius = 8.0;
    objects.insert(a.id, a);

    let mut tmpl_b = ThingTemplate::new("AmericaVehicleComanche");
    tmpl_b.add_kind_of(KindOf::Aircraft);
    tmpl_b.add_kind_of(KindOf::Vehicle);
    let mut b = Object::new(tmpl_b, ObjectId(22), Team::USA);
    b.object_type = ObjectType::Aircraft;
    b.loco_appearance = LocomotorAppearance::Hover;
    b.status.airborne_target = true;
    b.set_position(Vec3::new(30.0, 40.0, 190.0));
    b.selection_radius = 8.0;
    objects.insert(b.id, b);

    let adj = sys.adjust_target_destination(
        22,
        &objects,
        dest,
        target,
        8.0,
        SURFACE_AIR,
        false,
        8.0,
        5.0,
        150.0,
        0.0,
    );
    let dest_cell = sys.grid.world_to_grid(dest);
    let adj_cell = sys.grid.world_to_grid(adj);
    assert_ne!(
        adj_cell, dest_cell,
        "second Comanche must leave first hover cell"
    );
    assert!(
        crate::game_logic::weapon_bootstrap::is_goal_pos_within_attack_range(
            adj, target, 150.0, 0.0, 8.0, 5.0
        ),
        "spiral dest must stay in attack range"
    );
}

/// hq-6p032: crushers plan through idle crushable cars (AIPathfind.cpp:5063).
#[test]
fn crusher_plans_through_idle_crushable_cars() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    for (i, z) in (20..90).step_by(10).enumerate() {
        let mut tmpl = ThingTemplate::new("CivilianCar");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut car = Object::new(tmpl, ObjectId(100 + i as u32), Team::GLA);
        car.set_position(Vec3::new(80.0, 0.0, z as f32));
        car.crushable_level = 1;
        car.crusher_level = 0;
        car.owner_player_id = Some(1);
        car.selection_radius = 4.0;
        objects.insert(car.id, car);
    }
    let mut tank_t = ThingTemplate::new("Crusader");
    tank_t.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tank_t, ObjectId(1), Team::USA);
    tank.set_position(Vec3::new(10.0, 0.0, 50.0));
    tank.crusher_level = 2;
    tank.crushable_level = 2;
    tank.owner_player_id = Some(0);
    objects.insert(tank.id, tank);

    let start = Vec3::new(10.0, 0.0, 50.0);
    let goal = Vec3::new(150.0, 0.0, 50.0);
    let crushed = sys
        .find_path_ex_surfaces(
            start,
            goal,
            &objects,
            false,
            SURFACE_GROUND,
            true,
            Some(ObjectId(1)),
        )
        .expect("crusher must path through cars");
    assert!(crushed.len() >= 2);
    // C++ getRadiusAndCenter (AIPathfind.cpp:9670-9696): radius-4 cars stamp
    // ONE pathfind cell each (updatePos loops i in [x-0, x+1)). A crusher's
    // A* treats crushable enemy cells as passable (checkForMovement,
    // AIPathfind.cpp:5063 canCrushOrSquish), so its route must cross the
    // stamped car column; Path::optimizeGroundPath (AIPathfind.cpp:567-680)
    // may then collapse it to one straight segment, so sample along each
    // window rather than testing waypoints alone.
    let car_cells: Vec<GridPos> = (20..90)
        .step_by(10)
        .map(|z| sys.grid.world_to_grid(Vec3::new(80.0, 0.0, z as f32)))
        .collect();
    let enters_car_cell = crushed.windows(2).any(|w| {
        (0..=32).any(|s| {
            let t = s as f32 / 32.0;
            let p = w[0] * (1.0 - t) + w[1] * t;
            car_cells.contains(&sys.grid.world_to_grid(p))
        })
    });
    assert!(
        enters_car_cell,
        "crusher path must drive through the stamped car cells, path={crushed:?} cells={car_cells:?}"
    );

    let mut inf_t = ThingTemplate::new("Ranger");
    inf_t.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(inf_t, ObjectId(2), Team::USA);
    inf.set_position(start);
    inf.crusher_level = 0;
    inf.owner_player_id = Some(0);
    objects.insert(inf.id, inf);
    objects.remove(&ObjectId(1));
    sys.note_logic_frame(1);
    let walked = sys
        .find_path_ex_surfaces(
            start,
            goal,
            &objects,
            false,
            SURFACE_GROUND,
            false,
            Some(ObjectId(2)),
        )
        .expect("non-crusher can detour");
    let walk_len: f32 = walked.windows(2).map(|w| (w[0] - w[1]).length()).sum();
    let crush_len: f32 = crushed.windows(2).map(|w| (w[0] - w[1]).length()).sum();
    // C++ allyFixedCount is a COST, not a wall (AIPathfind.cpp:5050-5060,
    // 6281-6291): the non-crusher may legally thread the 10-unit gaps between
    // 1-cell car stamps; Path::optimizeGroundPath (AIPathfind.cpp:567-680)
    // then straightens to one corner waypoint. The observable contract:
    // no walked waypoint lands in a stamped car cell, and the detour still
    // costs strictly more than the crush-through line.
    let avoids_car_cells = walked
        .iter()
        .all(|p| !car_cells.contains(&sys.grid.world_to_grid(*p)));
    assert!(
        avoids_car_cells,
        "non-crusher must not route through car cells, walk={walked:?} cells={car_cells:?}"
    );
    assert!(
        walk_len > crush_len,
        "non-crusher must pay extra over crush-through crush={crush_len} walk={walk_len}"
    );
}

/// hq-t7pyo: ALLIES occupancy is allyFixed, never crush-through.
#[test]
fn occupancy_allies_are_not_crush_through() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut masks = [0u16; 16];
    masks[0] = 1u16 << 0 | 1u16 << 1;
    masks[1] = 1u16 << 0 | 1u16 << 1;
    sys.set_player_ally_masks(masks);

    let mut objects = HashMap::new();
    for (i, z) in (20..90).step_by(10).enumerate() {
        let mut tmpl = ThingTemplate::new("AlliedCar");
        tmpl.add_kind_of(KindOf::Vehicle);
        let mut car = Object::new(tmpl, ObjectId(100 + i as u32), Team::China);
        car.set_position(Vec3::new(80.0, 0.0, z as f32));
        car.crushable_level = 1;
        car.crusher_level = 0;
        car.owner_player_id = Some(1);
        car.selection_radius = 4.0;
        objects.insert(car.id, car);
    }
    let mut tank_t = ThingTemplate::new("AllyCrusader");
    tank_t.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tank_t, ObjectId(1), Team::USA);
    tank.set_position(Vec3::new(10.0, 0.0, 50.0));
    tank.crusher_level = 2;
    tank.crushable_level = 2;
    tank.owner_player_id = Some(0);
    objects.insert(tank.id, tank);

    let start = Vec3::new(10.0, 0.0, 50.0);
    let goal = Vec3::new(150.0, 0.0, 50.0);
    let path = sys
        .find_path_ex_surfaces(
            start,
            goal,
            &objects,
            false,
            SURFACE_GROUND,
            true,
            Some(ObjectId(1)),
        )
        .expect("must detour around allied cars");
    // C++ getRadiusAndCenter (AIPathfind.cpp:9670-9696): radius-4 allied cars
    // stamp ONE pathfind cell each. allyFixedCount is a cost, not a wall
    // (AIPathfind.cpp:5050-5060, 6281-6291) — C++ may thread the gaps between
    // stamps — but no waypoint may land inside a stamped car cell, and the
    // route must not be a straight crush-through line.
    let car_cells: Vec<GridPos> = (20..90)
        .step_by(10)
        .map(|z| sys.grid.world_to_grid(Vec3::new(80.0, 0.0, z as f32)))
        .collect();
    let avoids_car_cells = path
        .iter()
        .all(|p| !car_cells.contains(&sys.grid.world_to_grid(*p)));
    assert!(
        avoids_car_cells,
        "ALLIES cars must not be routed through, path={path:?} cells={car_cells:?}"
    );
}

/// hq-bw35t: attack / requestPath must not fail-open through sealed walls.
#[test]
fn attack_and_request_path_fail_closed_through_walls() {
    use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    for y in 0..20 {
        sys.grid
            .set_cell_type(GridPos::new(10, y), PathfindCellType::Impassable);
    }
    let objects = HashMap::new();
    let from = Vec3::new(20.0, 0.0, 50.0);
    let to = Vec3::new(150.0, 0.0, 50.0);
    let crosses_wall = |system: &PathfindingSystem, path: &[Vec3]| {
        path.windows(2).any(|w| {
            let a = system.grid.world_to_grid(w[0]);
            let b = system.grid.world_to_grid(w[1]);
            (a.x - 10) * (b.x - 10) <= 0 && (a.x != 10 || b.x != 10)
                || system.grid.cell_type(a) == PathfindCellType::Impassable
                || system.grid.cell_type(b) == PathfindCellType::Impassable
        })
    };
    if let Some(p) =
        sys.find_path_ex_surfaces(from, to, &objects, false, SURFACE_GROUND, false, None)
    {
        assert!(
            !crosses_wall(&sys, &p),
            "A* must not walk Impassable, path={p:?}"
        );
    }
    if let Some(p) = sys.find_attack_firing_position(from, to, 50.0, &objects, false, None) {
        assert!(
            !crosses_wall(&sys, &p),
            "findAttackPath must not install a through-wall march, path={p:?}"
        );
        assert!(
            !(p.len() == 2 && (p[1] - to).length() < 1.0),
            "must not fail-open start→goal through the wall"
        );
    }

    let mut logic = GameLogic::new();
    // GameLogic grids are centered at (-w/2,-h/2): derive the wall column from
    // the LIVE grid so it truly separates start and goal cells.
    let wall_x = {
        let grid = &logic.pathfinding_system.grid;
        let start_cell = grid.world_to_grid(from);
        let goal_cell = grid.world_to_grid(to);
        let wx = (start_cell.x + goal_cell.x) / 2;
        assert!(
            start_cell.x < wx && wx < goal_cell.x,
            "wall column {wx} must sit between start cell {start_cell:?} and goal cell {goal_cell:?}"
        );
        wx
    };
    for y in 0..logic.pathfinding_system.grid.height() {
        logic
            .pathfinding_system
            .grid
            .set_cell_type(GridPos::new(wall_x, y), PathfindCellType::Impassable);
    }
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let id = ObjectId(7);
    let mut unit = Object::new(tmpl, id, Team::USA);
    unit.set_position(from);
    logic.objects.insert(id, unit);
    let ok = logic.request_object_path(id, to);
    let unit = logic.objects.get(&id).expect("unit");
    if ok {
        let path = &unit.movement.path;
        let through = path.windows(2).any(|w| {
            let a = logic.pathfinding_system.grid.world_to_grid(w[0]);
            let b = logic.pathfinding_system.grid.world_to_grid(w[1]);
            (a.x - wall_x) * (b.x - wall_x) <= 0 && (a.x != wall_x || b.x != wall_x)
                || logic.pathfinding_system.grid.cell_type(a) == PathfindCellType::Impassable
                || logic.pathfinding_system.grid.cell_type(b) == PathfindCellType::Impassable
        });
        assert!(
            !through,
            "requestPath must not march through the wall: {path:?}"
        );
    }
}

/// hq-3biqe: assign_unit_path must pass real is_crusher into live A*.
#[test]
fn assign_unit_path_crusher_walks_rubble() {
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    logic.force_map_loaded_for_path_test(false);
    let start = Vec3::new(10.0, 0.0, 10.0);
    let goal = Vec3::new(80.0, 0.0, 10.0);
    let start_cell = logic.pathfinding_system.grid.world_to_grid(start);
    let goal_cell = logic.pathfinding_system.grid.world_to_grid(goal);
    let wall_x = (start_cell.x + goal_cell.x) / 2;
    for y in 0..logic.pathfinding_system.grid.height() {
        logic
            .pathfinding_system
            .grid
            .set_cell_type(GridPos::new(wall_x, y), PathfindCellType::Rubble);
    }
    let mut tmpl = ThingTemplate::new("Overlord");
    tmpl.add_kind_of(KindOf::Vehicle);
    logic.templates.insert("Overlord".into(), tmpl);
    let id = logic
        .create_object("Overlord", Team::USA, start)
        .expect("overlord");
    if let Some(u) = logic.host_object_mut(id) {
        u.crusher_level = 2;
        u.locomotor_surfaces = SURFACE_GROUND;
        u.movement.max_speed = 20.0;
    }
    assert!(
        logic.assign_unit_path_for_test(id, goal, &[]),
        "crusher assign_unit_path must path CELL_RUBBLE"
    );
    let unit = logic.host_object(id).expect("unit");
    assert!(
        unit.movement.path.len() >= 2,
        "crusher must receive a live A* path, got {:?}",
        unit.movement.path
    );
}

/// hq-8q7gp: adjustDestination refuses uncrushable parked occupants.
#[test]
fn adjust_destination_refuses_enemy_fixed_occupant() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut g = open_grid(16, 16);
    let dest = GridPos::new(8, 8);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("CivilianCar");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut car = Object::new(tmpl, ObjectId(9), Team::GLA);
    car.set_position(g.grid_to_world(dest));
    car.crushable_level = 1;
    car.owner_player_id = Some(1);
    objects.insert(car.id, car);
    g.update_dynamic_obstacles(&objects);
    let snapped = g
        .adjust_destination_ex(dest, SURFACE_GROUND, false, 64, Some(0), 0)
        .expect("neighbor");
    assert_ne!(snapped, dest, "must not accept the occupied cell");
    assert!(
        !g.has_blocking_fixed_occupant(snapped, 0),
        "spiral must land off the parked car"
    );
    let crushed = g
        .adjust_destination_ex(dest, SURFACE_GROUND, true, 64, Some(0), 2)
        .expect("crusher");
    assert_eq!(crushed, dest, "crusher may occupy the crushable car cell");
}

/// hq-9erk0: rally uses structure-aware zones, not ForUI terrain zones.
#[test]
fn rally_gate_rejects_structure_enclosed_courtyard() {
    let mut g = open_grid(12, 12);
    for y in 0..12 {
        g.set_cell_type(GridPos::new(6, y), PathfindCellType::Obstacle);
    }
    g.rebuild_terrain_zones();
    g.rebuild_path_zones();
    let from = g.grid_to_world(GridPos::new(2, 6));
    let to = g.grid_to_world(GridPos::new(10, 6));
    assert!(
        g.quick_path_exists_for_ui(from, to),
        "ForUI ignores structure obstacles"
    );
    assert!(
        !g.quick_path_exists(from, to),
        "clientSafeQuickDoesPathExist must split on Obstacle"
    );
}

/// hq-c88bl: rubble husks stamp CELL_RUBBLE, not Clear / Obstacle.
#[test]
fn destroyed_building_stamps_rubble_not_clear() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("AmericaWarFactory");
    tmpl.add_kind_of(KindOf::Structure);
    let mut factory = Object::new(tmpl, ObjectId(3), Team::USA);
    factory.set_position(Vec3::new(80.0, 0.0, 80.0));
    factory.selection_radius = 20.0;
    factory.body_damage_state = HostBodyDamageType::Rubble;
    factory.status.keep_as_rubble = true;
    factory.status.effectively_dead = true;
    factory.health.current = 0.0;
    objects.insert(factory.id, factory);
    sys.apply_structure_static_blocks(&objects);
    let cell = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 80.0));
    assert_eq!(sys.grid.cell_type(cell), PathfindCellType::Rubble);
    assert_ne!(
        sys.grid.path_zone(cell),
        0,
        "rubble husk must get a real zone"
    );
    let nearby = sys.grid.world_to_grid(Vec3::new(20.0, 0.0, 20.0));
    assert_ne!(
        sys.grid.path_zone(cell),
        sys.grid.path_zone(nearby),
        "rubble husk must not merge into Clear ground"
    );
    assert!(
        !sys.grid.cell_passable_for(cell, SURFACE_GROUND, false),
        "infantry cannot walk rubble at full ground surfaces"
    );
    assert!(
        sys.grid.cell_passable_for(cell, SURFACE_GROUND, true),
        "crushers walk rubble"
    );
}

/// hq-2f8q0: CPOP must not lead through an obstacle wall.
#[test]
fn cpop_lead_aborts_through_building() {
    let mut g = open_grid(16, 16);
    for y in 0..16 {
        g.set_cell_type(GridPos::new(8, y), PathfindCellType::Obstacle);
    }
    let a = g.grid_to_world(GridPos::new(2, 8));
    let b = g.grid_to_world(GridPos::new(14, 8));
    let pos = g.grid_to_world(GridPos::new(4, 8));
    let lead =
        PathfindingSystem::compute_point_on_path_ex(pos, &[a, b], Some(&g), SURFACE_GROUND, false);
    let lead_cell = g.world_to_grid(lead);
    assert!(
        lead_cell.x < 8,
        "must not aim through the building, lead={lead:?} cell={lead_cell:?}"
    );
    let blind = PathfindingSystem::compute_point_on_path(pos, &[a, b]);
    let blind_cell = g.world_to_grid(blind);
    assert!(
        blind_cell.x >= 8,
        "ungated geometric lead still crosses the wall (control)"
    );
}

/// hq-5g9m3: C++ always tries lead; offset of 2 cells must still cut to next.
#[test]
fn cpop_leads_when_two_cells_off_clear_path() {
    let g = open_grid(16, 16);
    let a = g.grid_to_world(GridPos::new(2, 8));
    let b = g.grid_to_world(GridPos::new(14, 8));
    // Two cells off the polyline (C++ k = 2/3, still tries next node).
    let pos = g.grid_to_world(GridPos::new(4, 6));
    let lead =
        PathfindingSystem::compute_point_on_path_ex(pos, &[a, b], Some(&g), SURFACE_GROUND, false);
    let lead_cell = g.world_to_grid(lead);
    assert!(
        lead_cell.x >= 12,
        "clear line must lead to next node, lead={lead:?} cell={lead_cell:?}"
    );
}

/// hq-csg1b: pinched cells on a straight A* jog still collapse.
#[test]
fn optimize_collapses_pinched_collinear_jog() {
    let mut g = open_grid(12, 8);
    for x in 3..9 {
        g.set_pinched(GridPos::new(x, 4), true);
    }
    let raw: Vec<Vec3> = (2..=10)
        .map(|x| g.grid_to_world(GridPos::new(x, 4)))
        .collect();
    let opt = g.optimize_ground_path_ex(&raw, SURFACE_GROUND, false, None, 0);
    assert!(
        opt.len() <= 2,
        "straight pinched jog must collapse, got {opt:?}"
    );
}
