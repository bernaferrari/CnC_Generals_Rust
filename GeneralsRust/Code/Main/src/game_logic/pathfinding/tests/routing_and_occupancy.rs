use super::super::*;

fn open_grid(w: i32, h: i32) -> PathfindingGrid {
    PathfindingGrid::new(w as f32 * 10.0, h as f32 * 10.0, 10.0)
}

/// hq-9jz4r: layer change only looks ALLOWED_STEPS=3 past the ramp.
#[test]
fn optimize_caps_bridge_layer_steps() {
    let mut g = open_grid(16, 12);
    for y in 4..=6 {
        for x in 4..12 {
            g.set_cell_type(GridPos::new(x, y), PathfindCellType::Water);
        }
    }
    g.stamp_bridge_deck(
        Vec3::new(30.0, 20.0, 40.0),
        Vec3::new(30.0, 20.0, 60.0),
        Vec3::new(130.0, 20.0, 40.0),
        Vec3::new(130.0, 20.0, 60.0),
        false,
    );
    let mut raw = Vec::new();
    // Five ground cells so count at the first deck node is > ALLOWED_STEPS.
    for x in 0..=4 {
        raw.push(g.grid_to_world(GridPos::new(x, 5)));
    }
    for x in 5..=12 {
        raw.push(g.grid_to_world_on_layer(
            GridPos::new(x, 5),
            PathfindLayerEnum::from_u32(g.first_bridge_layer_id().unwrap_or(2) as u32),
        ));
    }
    let opt = g.optimize_ground_path_ex(&raw, SURFACE_GROUND, false, None, 0);
    assert!(
        opt.len() >= 3,
        "ALLOWED_STEPS=3 must keep ramp approach, got {opt:?}"
    );
    let start_layer = g.layer_for_destination(opt[0]);
    assert_eq!(
        start_layer,
        PathfindLayerEnum::Ground,
        "must keep a ground approach node"
    );
}

/// hq-5iup4: optimize must not cut through parked occupants; un-pinched cliff can.
#[test]
fn optimize_respects_occupancy_and_unpinched_cliff() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut g = open_grid(16, 16);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut ally = Object::new(tmpl, ObjectId(4), Team::USA);
    ally.set_position(g.grid_to_world(GridPos::new(8, 4)));
    ally.owner_player_id = Some(0);
    objects.insert(ally.id, ally);
    g.update_dynamic_obstacles(&objects);
    let start = g.grid_to_world(GridPos::new(2, 4));
    let mid = g.grid_to_world(GridPos::new(8, 2));
    let end = g.grid_to_world(GridPos::new(14, 4));
    let raw = vec![start, mid, end];
    let opt = g.optimize_ground_path_ex(&raw, SURFACE_GROUND, false, Some(0), 0);
    assert!(
        opt.len() >= 3,
        "must keep the detour around idle ally, got {opt:?}"
    );

    let mut cliff = open_grid(8, 8);
    cliff.set_cell_type(GridPos::new(4, 4), PathfindCellType::Cliff);
    let a = cliff.grid_to_world(GridPos::new(1, 4));
    let b = cliff.grid_to_world(GridPos::new(4, 4));
    let c = cliff.grid_to_world(GridPos::new(7, 4));
    let collapsed = cliff.optimize_ground_path_ex(&[a, b, c], SURFACE_GROUND, false, None, 0);
    assert!(
        collapsed.len() <= 2,
        "un-pinched cliff ramp must collapse, got {collapsed:?}"
    );
}

/// hq-vovla: FenceWidth>0 rasters a fence; name-only decorative props do not.
#[test]
fn fence_width_not_name_classifies_fence() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut named = ThingTemplate::new("DecorativeFenceProp");
    named.add_kind_of(KindOf::Structure);
    named.fence_width = 0.0;
    let mut prop = Object::new(named, ObjectId(1), Team::Neutral);
    prop.set_position(Vec3::new(40.0, 0.0, 40.0));
    prop.selection_radius = 20.0;
    objects.insert(prop.id, prop);
    sys.apply_structure_static_blocks(&objects);
    let named_cell = sys.grid.world_to_grid(Vec3::new(40.0, 0.0, 40.0));
    assert!(
        !sys.grid.is_obstacle_fence(named_cell),
        "name-only fence must not become a crush corridor"
    );

    let mut real = ThingTemplate::new("ChinaChainlink");
    real.fence_width = 40.0;
    real.fence_x_offset = 0.0;
    let mut fence = Object::new(real, ObjectId(2), Team::China);
    fence.set_position(Vec3::new(120.0, 0.0, 40.0));
    fence.set_orientation(0.0);
    objects.insert(fence.id, fence);
    sys.apply_structure_static_blocks(&objects);
    let fence_cell = sys.grid.world_to_grid(Vec3::new(120.0, 0.0, 40.0));
    assert!(
        sys.grid.is_obstacle_fence(fence_cell),
        "INI FenceWidth must classify a crushable fence strip"
    );
    assert!(sys.grid.cell_passable_for(fence_cell, SURFACE_GROUND, true));
}

/// hq-qbvcc: scaffolds classify as path obstacles at placement.
#[test]
fn under_construction_structure_blocks_path() {
    use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("AmericaWarFactory");
    tmpl.add_kind_of(KindOf::Structure);
    let mut factory = Object::new_under_construction(tmpl, ObjectId(4), Team::USA);
    factory.set_position(Vec3::new(80.0, 0.0, 80.0));
    factory.selection_radius = 20.0;
    assert!(factory.status.under_construction);
    objects.insert(factory.id, factory);
    sys.apply_structure_static_blocks(&objects);
    let cell = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 80.0));
    assert!(
        sys.grid.is_static_blocked(cell),
        "C++ addObjectToPathfindMap at construct() must block unfinished buildings"
    );

    let mut logic = GameLogic::new();
    let mut place = ThingTemplate::new("TestScaffoldBarracks");
    place.add_kind_of(KindOf::Structure);
    logic.templates.insert("TestScaffoldBarracks".into(), place);
    let id = logic
        .create_object_under_construction(
            "TestScaffoldBarracks",
            Team::USA,
            Vec3::new(80.0, 0.0, 80.0),
        )
        .expect("scaffold");
    let obj = logic.host_object(id).expect("placed");
    assert!(obj.status.under_construction);
    let placed = obj.get_position();
    let placed_cell = logic.pathfinding_system.grid.world_to_grid(placed);
    assert!(
        logic.pathfinding_system.grid.is_static_blocked(placed_cell),
        "placement must stamp the scaffold footprint immediately"
    );
}

/// hq-ah4jh: queued move must not install dest velocity before A*.
#[test]
fn queued_move_does_not_charge_before_path() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
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
    assert!(unit.waiting_for_path);
    assert!(
        unit.movement.velocity.length_squared() < 1.0e-6,
        "must not charge the raw click, vel={:?}",
        unit.movement.velocity
    );
    assert!(
        unit.movement.target_position.is_none(),
        "locomotor must not integrate toward dest while waiting"
    );
}

/// hq-985ts: leftover/C++ clearCellForDiameter on the live grid.
#[test]
fn clear_cell_for_diameter_open_and_blocked() {
    let mut g = open_grid(16, 16);
    assert_eq!(g.clear_cell_for_diameter(false, GridPos::new(8, 8), 2), 2);
    assert_eq!(g.clear_cell_for_diameter(false, GridPos::new(8, 8), 1), 1);
    g.set_blocked(GridPos::new(7, 8), true);
    assert_eq!(
        g.clear_cell_for_diameter(false, GridPos::new(8, 8), 2),
        0,
        "diameter 2 must fail when an adjacent cell is blocked"
    );
    let (r, _) = PathfindingGrid::radius_and_center(15.0, 10.0);
    assert_eq!(r, 1);
    assert_eq!(PathfindingGrid::path_diameter_for_unit(15.0, 10.0, true), 2);
    assert_eq!(PathfindingGrid::path_diameter_for_unit(8.0, 10.0, false), 1);
}

/// hq-985ts: tanks cannot thread a one-cell infantry slot.
#[test]
fn vehicle_astar_rejects_infantry_width_gap() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    for y in 0..20 {
        sys.grid.set_blocked(GridPos::new(8, y), true);
        sys.grid.set_blocked(GridPos::new(10, y), true);
    }
    let start = Vec3::new(20.0, 0.0, 50.0);
    let goal = Vec3::new(150.0, 0.0, 50.0);

    let mut inf_t = ThingTemplate::new("Ranger");
    inf_t.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(inf_t, ObjectId(1), Team::USA);
    inf.set_position(start);
    inf.selection_radius = 8.0;
    let mut objects = HashMap::new();
    objects.insert(inf.id, inf);
    let infantry_path = sys
        .find_path_ex_surfaces(
            start,
            goal,
            &objects,
            false,
            SURFACE_GROUND,
            false,
            Some(ObjectId(1)),
        )
        .expect("infantry can thread a one-cell corridor");
    assert!(infantry_path.len() >= 2);

    objects.clear();
    let mut tank_t = ThingTemplate::new("Crusader");
    tank_t.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tank_t, ObjectId(2), Team::USA);
    tank.set_position(start);
    tank.selection_radius = 15.0;
    objects.insert(tank.id, tank);
    sys.note_logic_frame(1);
    let tank_path = sys.find_path_ex_surfaces(
        start,
        goal,
        &objects,
        false,
        SURFACE_GROUND,
        false,
        Some(ObjectId(2)),
    );
    let crossed = tank_path.as_ref().is_some_and(|path| {
        path.iter().any(|p| {
            let c = sys.grid.world_to_grid(*p);
            c.x == 9
        }) && path
            .last()
            .is_some_and(|p| sys.grid.world_to_grid(*p).x >= 12)
    });
    assert!(
        !crossed,
        "vehicle A* must not thread a one-cell infantry gap, got {tank_path:?}"
    );
}

/// hq-asov5: structure-aware rally zones split on water.
#[test]
fn path_zones_reject_water_dest_and_split_river() {
    let mut g = open_grid(8, 8);
    for y in 0..8 {
        g.set_cell_type(GridPos::new(4, y), PathfindCellType::Water);
    }
    g.rebuild_path_zones();
    let from = g.grid_to_world(GridPos::new(1, 4));
    let to = g.grid_to_world(GridPos::new(6, 4));
    let wet = g.grid_to_world(GridPos::new(4, 4));
    assert!(
        !g.quick_path_exists(from, to),
        "water must split path_zones"
    );
    assert!(
        !g.quick_path_exists(from, wet),
        "ground rally must reject a water dest"
    );
    assert_ne!(g.path_zone(GridPos::new(1, 4)), 0);
    assert_ne!(
        g.path_zone(GridPos::new(1, 4)),
        g.path_zone(GridPos::new(6, 4))
    );
}

/// hq-998ki: GROUND+WATER / GROUND+CLIFF combiners join banks; dest gates stay C++.
#[test]
fn path_zones_surface_combiners_join_water_and_cliff() {
    let mut g = open_grid(8, 8);
    for y in 0..8 {
        g.set_cell_type(GridPos::new(4, y), PathfindCellType::Water);
    }
    g.rebuild_path_zones();
    let from = g.grid_to_world(GridPos::new(1, 4));
    let to = g.grid_to_world(GridPos::new(6, 4));
    let wet = g.grid_to_world(GridPos::new(4, 4));
    assert!(
        !g.quick_path_exists(from, to),
        "GROUND-only must not share a zone across CELL_WATER"
    );
    assert!(
        g.quick_path_exists_for(from, to, SURFACE_GROUND | SURFACE_WATER),
        "GROUND+WATER combiner must join opposite banks"
    );
    assert!(
        g.quick_path_exists_for(from, wet, SURFACE_GROUND | SURFACE_WATER),
        "amphibious dest on water is validMovementPosition"
    );

    let mut c = open_grid(8, 8);
    for y in 0..8 {
        c.set_cell_type(GridPos::new(4, y), PathfindCellType::Cliff);
    }
    c.rebuild_path_zones();
    let c_from = c.grid_to_world(GridPos::new(1, 4));
    let c_to = c.grid_to_world(GridPos::new(6, 4));
    let cliff = c.grid_to_world(GridPos::new(4, 4));
    assert!(
        !c.quick_path_exists(c_from, c_to),
        "GROUND-only must not share a zone across CELL_CLIFF"
    );
    assert!(
        c.quick_path_exists_for(c_from, c_to, SURFACE_GROUND | SURFACE_CLIFF),
        "GROUND+CLIFF combiner must join opposite banks"
    );
    assert!(
        !c.quick_path_exists_for(c_from, cliff, SURFACE_GROUND | SURFACE_CLIFF),
        "C++ rejects cliff goals even for cliff locos"
    );
}

/// hq-0r5xs: rubble is its own zone; Impassable is not uninitialized 0.
#[test]
fn path_zones_rubble_and_impassable_are_not_ground() {
    let mut g = open_grid(8, 8);
    for y in 0..8 {
        g.set_cell_type(GridPos::new(4, y), PathfindCellType::Rubble);
    }
    g.rebuild_path_zones();
    let from = g.grid_to_world(GridPos::new(1, 4));
    let to = g.grid_to_world(GridPos::new(6, 4));
    let rubble = g.grid_to_world(GridPos::new(4, 4));
    assert_ne!(
        g.path_zone(GridPos::new(4, 4)),
        0,
        "CELL_RUBBLE must get a real zone"
    );
    assert_ne!(
        g.path_zone(GridPos::new(1, 4)),
        g.path_zone(GridPos::new(4, 4)),
        "rubble must not merge into the Clear flood"
    );
    assert!(
        !g.quick_path_exists(from, to),
        "GROUND-only must not share a zone across CELL_RUBBLE"
    );
    assert!(
        !g.quick_path_exists(from, rubble),
        "GROUND-only must not treat rubble dest as pathable"
    );
    assert!(
        g.quick_path_exists_for(from, to, SURFACE_GROUND | SURFACE_RUBBLE),
        "GROUND+RUBBLE combiner must join opposite banks"
    );
    assert!(
        g.quick_path_exists_for(from, rubble, SURFACE_GROUND | SURFACE_RUBBLE),
        "rubble dest is valid for GROUND+RUBBLE locos"
    );

    let mut imp = open_grid(8, 8);
    for y in 0..8 {
        imp.set_cell_type(GridPos::new(4, y), PathfindCellType::Impassable);
    }
    imp.rebuild_path_zones();
    let i_from = imp.grid_to_world(GridPos::new(1, 4));
    let i_to = imp.grid_to_world(GridPos::new(6, 4));
    let i_cell = imp.grid_to_world(GridPos::new(4, 4));
    assert_ne!(
        imp.path_zone(GridPos::new(4, 4)),
        0,
        "Impassable must get a real zone, not uninitialized 0"
    );
    assert!(
        !imp.quick_path_exists(i_from, i_to),
        "GROUND-only must not share a zone across Impassable"
    );
    assert!(
        !imp.quick_path_exists(i_from, i_cell),
        "Impassable dest must not false-positive via zone 0"
    );

    for ty in [
        PathfindCellType::Obstacle,
        PathfindCellType::BridgeImpassable,
    ] {
        let mut g = open_grid(8, 8);
        for y in 0..8 {
            g.set_cell_type(GridPos::new(4, y), ty);
        }
        g.rebuild_path_zones();
        let from = g.grid_to_world(GridPos::new(1, 4));
        let to = g.grid_to_world(GridPos::new(6, 4));
        let cell = g.grid_to_world(GridPos::new(4, 4));
        assert_ne!(
            g.path_zone(GridPos::new(4, 4)),
            0,
            "{ty:?} must get a real zone"
        );
        assert!(
            !g.quick_path_exists(from, to),
            "GROUND-only must not share a zone across {ty:?}"
        );
        assert!(
            !g.quick_path_exists(from, cell),
            "{ty:?} dest must not false-positive via zone 0"
        );
    }

    let mut sys = PathfindingSystem::new(80.0, 80.0);
    sys.grid.rebuild_path_zones();
    let s_from = sys.grid.grid_to_world(GridPos::new(1, 4));
    let s_to = sys.grid.grid_to_world(GridPos::new(6, 4));
    assert!(
        sys.grid.quick_path_exists(s_from, s_to),
        "open map must path before rubble stamp"
    );
    for y in 0..8 {
        let p = sys.grid.grid_to_world(GridPos::new(4, y));
        sys.stamp_rubble_at_world(p, 0);
    }
    assert_eq!(
        sys.grid.cell_type(GridPos::new(4, 4)),
        PathfindCellType::Rubble
    );
    assert!(
        !sys.grid.quick_path_exists(s_from, s_to),
        "stamp_rubble_at_world must rebuild zones so rubble splits GROUND"
    );
}

/// hq-ykcav: crusher combiners merge fence Obstacle with adjacent Clear.
#[test]
fn path_zones_crusher_combiner_joins_fence_banks() {
    let mut g = open_grid(8, 8);
    for y in 0..8 {
        g.set_cell_obstacle(GridPos::new(4, y), true, false);
    }
    g.rebuild_path_zones();
    let from = g.grid_to_world(GridPos::new(1, 4));
    let to = g.grid_to_world(GridPos::new(6, 4));
    let fence = g.grid_to_world(GridPos::new(4, 4));
    assert!(g.is_obstacle_fence(GridPos::new(4, 4)));
    assert_ne!(
        g.path_zone(GridPos::new(1, 4)),
        g.path_zone(GridPos::new(6, 4)),
        "fence Obstacle must split raw path_zones"
    );
    assert!(
        !g.quick_path_exists(from, to),
        "non-crusher must not share a zone across a fence line"
    );
    assert!(
        g.quick_path_exists_for_crusher(from, to, SURFACE_GROUND, true),
        "crusher combiner must join opposite sides of a fence"
    );
    // Fence dest is validMovementPosition for crushers (cell-level crush is wired).
    assert!(
        g.quick_path_exists_for_crusher(from, fence, SURFACE_GROUND, true),
        "crusher dest on a fence cell stays same-zone"
    );
}

/// hq-dypfr: adjustToPossibleDestination walks near a blocked seed dest.
#[test]
fn adjust_to_possible_destination_walks_near_blocked_goal() {
    let mut g = open_grid(8, 8);
    g.set_cell_type(GridPos::new(6, 4), PathfindCellType::Impassable);
    g.rebuild_path_zones();
    let start = g.grid_to_world(GridPos::new(1, 4));
    let mut dest = g.grid_to_world(GridPos::new(6, 4));
    assert!(
        g.adjust_to_possible_destination(start, &mut dest, SURFACE_GROUND, false, 0.0),
        "must find a same-zone cell near the impassable seed"
    );
    assert_ne!(g.world_to_grid(dest), GridPos::new(6, 4));
    assert!(g.quick_path_exists(start, dest));
}

/// hq-dypfr: C++ worldToCell outside bounds fails adjustToPossibleDestination.
#[test]
fn adjust_to_possible_destination_out_of_bounds_fails() {
    let g = open_grid(8, 8);
    let start = g.grid_to_world(GridPos::new(1, 1));
    let mut dest = Vec3::new(50_000.0, 0.0, 50_000.0);
    assert!(!g.adjust_to_possible_destination(start, &mut dest, SURFACE_GROUND, false, 0.0));
}

/// hq-su78f: full queue refuses the newest, keeps the oldest.
#[test]
fn queue_overflow_refuses_newest() {
    let mut sys = PathfindingSystem::new(100.0, 100.0);
    let mk = |id: u32| PendingHostPath {
        unit_id: ObjectId(id),
        start: Vec3::ZERO,
        destination: Vec3::new(id as f32, 0.0, 0.0),
        waypoints: Vec::new(),
        aircraft: false,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        ignore_obstacle: None,
    };
    for i in 1..=PATHFIND_QUEUE_LEN as u32 {
        assert!(sys.queue_path(mk(i)), "slot {i} must enqueue");
    }
    assert_eq!(sys.pending_path_count(), PATHFIND_QUEUE_LEN);
    assert!(
        !sys.queue_path(mk(9000)),
        "C++ queueForPath refuses when nextSlot==head"
    );
    assert_eq!(sys.pending_path_count(), PATHFIND_QUEUE_LEN);
    assert!(
        sys.queue_path(mk(1)),
        "duplicate ObjectID is a no-op success"
    );
    let drained = sys.take_pending_paths();
    assert_eq!(drained.len(), PATHFIND_QUEUE_LEN);
    assert_eq!(drained[0].unit_id, ObjectId(1), "oldest waiter stays");
    assert!(
        drained.iter().all(|p| p.unit_id != ObjectId(9000)),
        "newest must not evict oldest"
    );
}

/// hq-p1pwi: moveAllies leaves packing/unpacking units planted.
#[test]
fn move_allies_skips_deploy_style_busy() {
    use crate::game_logic::host_deploy_style::{HostDeployStyleData, HostDeployStyleState};
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut mover_t = ThingTemplate::new("Ranger");
    mover_t.add_kind_of(KindOf::Infantry);
    let mut mover = Object::new(mover_t, ObjectId(1), Team::USA);
    mover.set_position(Vec3::new(10.0, 0.0, 10.0));
    objects.insert(mover.id, mover);

    let mut idle_t = ThingTemplate::new("Humvee");
    idle_t.add_kind_of(KindOf::Vehicle);
    let mut idle = Object::new(idle_t, ObjectId(2), Team::USA);
    idle.set_position(Vec3::new(50.0, 0.0, 10.0));
    objects.insert(idle.id, idle);

    let mut busy_t = ThingTemplate::new("NukeCannon");
    busy_t.add_kind_of(KindOf::Vehicle);
    let mut busy = Object::new(busy_t, ObjectId(3), Team::USA);
    busy.set_position(Vec3::new(60.0, 0.0, 10.0));
    let mut style = HostDeployStyleData::default();
    style.state = HostDeployStyleState::Deploying;
    busy.deploy_style = Some(style);
    objects.insert(busy.id, busy);

    let path = vec![
        Vec3::new(10.0, 0.0, 10.0),
        Vec3::new(50.0, 0.0, 10.0),
        Vec3::new(60.0, 0.0, 10.0),
    ];
    let nudged = sys.allies_to_nudge_off_path(ObjectId(1), &path, &objects);
    assert!(
        nudged.contains(&ObjectId(2)),
        "idle ally on the path must scoot"
    );
    assert!(
        !nudged.contains(&ObjectId(3)),
        "packing unit must stay planted"
    );
}

/// hq-8cfgs: 4-corner CLEAR / pinch CLIFF / dest layer / walk-on-wall A*.
#[test]
fn layer_wall_classify_and_search() {
    let mut g = open_grid(16, 16);
    g.set_wall_height(12.0);
    // Fat enough that cell (8,8) and its 3x3 are 4-corner CLEAR (no pinch).
    g.add_wall_piece(1, Vec3::new(85.0, 0.0, 85.0), 0.0, 40.0, 40.0);
    assert!(g.wall_piece_count() == 1);
    let center = GridPos::new(8, 8);
    assert_eq!(
        g.layer_cell_type(LAYER_WALL_ID, center),
        Some(PathfindCellType::Clear),
        "interior 4-corner cell must be CLEAR on LAYER_WALL"
    );
    // Cell (4,8) is [40,50]×[80,90]: two corners on the 40-radius deck.
    assert_eq!(
        g.layer_cell_type(LAYER_WALL_ID, GridPos::new(4, 8)),
        Some(PathfindCellType::BridgeImpassable),
        "1–3 corner cells are BRIDGE_IMPASSABLE (AIPathfind.cpp:3794-3796)"
    );
    g.clear_static_blocks();
    assert_eq!(
        g.layer_cell_type(LAYER_WALL_ID, center),
        Some(PathfindCellType::Clear),
        "terrain rebuild must reclassify remaining wall pieces"
    );
    let on_wall = Vec3::new(85.0, 12.0, 85.0);
    assert!(g.is_point_on_wall(on_wall));
    assert_eq!(
        g.layer_for_destination(on_wall),
        PathfindLayerEnum::Wall,
        "dest at wall height on a CLEAR cell is LAYER_WALL"
    );
    let on_ground = Vec3::new(85.0, 0.0, 85.0);
    assert_ne!(
        g.layer_for_destination(on_ground),
        PathfindLayerEnum::Wall,
        "ground-height click on the footprint stays off LAYER_WALL"
    );
    g.remove_wall_piece(1);
    assert_eq!(g.wall_piece_count(), 0);
    assert!(
        g.layer_cell_type(LAYER_WALL_ID, center).is_none(),
        "removing the last piece drops LAYER_WALL"
    );
    assert!(!g.is_point_on_wall(on_wall));
}

#[test]
fn infantry_paths_along_classified_wall() {
    let mut sys = PathfindingSystem::new(160.0, 160.0);
    sys.set_wall_height(12.0);
    for x in 4..13 {
        sys.grid
            .set_cell_type(GridPos::new(x, 8), PathfindCellType::Obstacle);
    }
    sys.grid
        .add_wall_piece(7, Vec3::new(85.0, 0.0, 85.0), 0.0, 50.0, 25.0);
    let objects = HashMap::new();
    let from = Vec3::new(50.0, 12.0, 85.0);
    let to = Vec3::new(120.0, 12.0, 85.0);
    assert_eq!(
        sys.grid.layer_for_destination(from),
        PathfindLayerEnum::Wall
    );
    assert_eq!(sys.grid.layer_for_destination(to), PathfindLayerEnum::Wall);
    let path = sys.find_path(from, to, &objects);
    assert!(
        path.as_ref().map(|p| p.len() >= 2).unwrap_or(false),
        "infantry A* must walk LAYER_WALL over the blocked ground"
    );
}

#[test]
fn destroy_wall_piece_splats_units_on_deck() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(160.0, 160.0);
    sys.set_wall_height(12.0);
    sys.grid
        .add_wall_piece(1, Vec3::new(80.0, 0.0, 50.0), 0.0, 20.0, 8.0);
    let mut objects = HashMap::new();
    let mut inf_t = ThingTemplate::new("RedGuard");
    inf_t.add_kind_of(KindOf::Infantry);
    let mut on_deck = Object::new(inf_t, ObjectId(10), Team::China);
    on_deck.set_position(Vec3::new(80.0, 12.0, 50.0));
    objects.insert(on_deck.id, on_deck);
    let mut ground_t = ThingTemplate::new("Battlemaster");
    ground_t.add_kind_of(KindOf::Vehicle);
    let mut on_ground = Object::new(ground_t, ObjectId(11), Team::China);
    on_ground.set_position(Vec3::new(80.0, 0.0, 50.0));
    objects.insert(on_ground.id, on_ground);
    let splat = sys.splat_units_on_wall_piece(ObjectId(1), &objects);
    assert!(splat.contains(&ObjectId(10)), "deck unit must splat");
    assert!(
        !splat.contains(&ObjectId(11)),
        "ground unit on the footprint must live"
    );
}

/// hq-1bkmw: infantry occupancy is getRadiusAndCenter (1 cell), not 3×3.
#[test]
fn infantry_occupancy_is_single_cell() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut g = open_grid(16, 16);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(tmpl, ObjectId(1), Team::USA);
    inf.set_position(Vec3::new(55.0, 0.0, 55.0));
    inf.owner_player_id = Some(0);
    inf.selection_radius = 5.0;
    g.update_dynamic_obstacles(&objects);
    let center = g.world_to_grid(Vec3::new(55.0, 0.0, 55.0));
    assert!(g.is_blocked(center), "infantry center cell must occupy");
    assert!(
        !g.is_blocked(GridPos::new(center.x + 1, center.y)),
        "r=5 infantry must not stamp a 3x3 Chebyshev"
    );
    assert!(
        !g.is_blocked(GridPos::new(center.x, center.y + 1)),
        "r=5 infantry must not stamp a 3x3 Chebyshev"
    );
}

/// hq-7q7d9: landing dest rejects Obstacle and scans aircraft radius.
#[test]
fn landing_dest_rejects_obstacle_and_uses_radius() {
    let mut g = open_grid(16, 16);
    let obstacle = GridPos::new(5, 5);
    g.set_cell_type(obstacle, PathfindCellType::Obstacle);
    let land = g
        .adjust_to_landing_destination(obstacle, 400, PathfindLayerEnum::Ground)
        .expect("spiral off obstacle");
    assert_ne!(land, obstacle);
    assert_ne!(g.cell_type(land), PathfindCellType::Obstacle);

    let rail = GridPos::new(8, 8);
    g.set_cell_type(rail, PathfindCellType::BridgeImpassable);
    let land_rail = g
        .adjust_to_landing_destination(rail, 400, PathfindLayerEnum::Ground)
        .expect("spiral off bridge rail");
    assert_ne!(g.cell_type(land_rail), PathfindCellType::BridgeImpassable);

    let mut g2 = open_grid(16, 16);
    g2.set_cell_type(GridPos::new(6, 5), PathfindCellType::Obstacle);
    let dest = GridPos::new(5, 5);
    assert_eq!(
        g2.adjust_to_landing_destination_for(dest, 400, PathfindLayerEnum::Ground, 0, true),
        Some(dest),
        "radius 0 may land on a clear center"
    );
    let wide = g2
        .adjust_to_landing_destination_for(dest, 400, PathfindLayerEnum::Ground, 1, true)
        .expect("spiral off radius-1 obstacle");
    assert_ne!(
        wide, dest,
        "radius 1 footprint must refuse neighbor Obstacle"
    );
}

/// hq-lhwsl: yield A* walks off the avoided path, not a 20wu shove.
#[test]
fn get_move_away_from_path_leaves_avoided_route() {
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let from = Vec3::new(50.0, 0.0, 50.0);
    let avoided = vec![
        Vec3::new(10.0, 0.0, 50.0),
        Vec3::new(50.0, 0.0, 50.0),
        Vec3::new(150.0, 0.0, 50.0),
    ];
    let path = sys
        .get_move_away_from_path(
            from,
            &avoided,
            None,
            SURFACE_GROUND,
            false,
            6.0,
            15.0,
            Some(0),
            0,
            false,
        )
        .expect("yield path");
    assert!(path.len() >= 2);
    let dest = *path.last().unwrap();
    assert!(
        (dest.x - from.x).abs() > 1.0 || (dest.z - from.z).abs() > 1.0,
        "must leave the start cell, dest={dest:?}"
    );
    assert!(
        (dest.z - 50.0).abs() > 5.0 || dest.x < 20.0 || dest.x > 140.0,
        "dest must not sit on the avoided corridor, dest={dest:?}"
    );
}

/// hq-fyn6i: moveAllies uses ALLIES / ignoreObstacle / mover radius.
#[test]
fn move_allies_uses_allies_radius_and_ignore() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut masks = [0u16; 16];
    masks[0] = 1u16 << 0 | 1u16 << 1;
    masks[1] = 1u16 << 0 | 1u16 << 1;
    sys.set_player_ally_masks(masks);

    let mut objects = HashMap::new();
    let mut mover_t = ThingTemplate::new("Overlord");
    mover_t.add_kind_of(KindOf::Vehicle);
    let mut mover = Object::new(mover_t, ObjectId(1), Team::USA);
    mover.set_position(Vec3::new(10.0, 0.0, 10.0));
    mover.selection_radius = 25.0;
    mover.owner_player_id = Some(0);
    objects.insert(mover.id, mover);

    let mut ally_t = ThingTemplate::new("Battlemaster");
    ally_t.add_kind_of(KindOf::Vehicle);
    let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
    ally.set_position(Vec3::new(70.0, 0.0, 10.0));
    ally.owner_player_id = Some(1);
    objects.insert(ally.id, ally);

    let mut ignored_t = ThingTemplate::new("Dozer");
    ignored_t.add_kind_of(KindOf::Vehicle);
    ignored_t.add_kind_of(KindOf::Dozer);
    let mut ignored = Object::new(ignored_t, ObjectId(3), Team::USA);
    ignored.set_position(Vec3::new(80.0, 0.0, 10.0));
    ignored.owner_player_id = Some(0);
    objects.insert(ignored.id, ignored);

    let mut off_t = ThingTemplate::new("Humvee");
    off_t.add_kind_of(KindOf::Vehicle);
    let mut off = Object::new(off_t, ObjectId(4), Team::USA);
    off.set_position(Vec3::new(70.0, 0.0, 40.0));
    off.owner_player_id = Some(0);
    objects.insert(off.id, off);

    let path = vec![Vec3::new(10.0, 0.0, 10.0), Vec3::new(80.0, 0.0, 10.0)];
    sys.set_ignore_obstacle(Some(ObjectId(3)));
    let nudged = sys.allies_to_nudge_off_path(ObjectId(1), &path, &objects);
    assert!(
        nudged.contains(&ObjectId(2)),
        "allied other-player unit on the Overlord radius must scoot"
    );
    assert!(
        !nudged.contains(&ObjectId(3)),
        "ignoreObstacle must stay planted"
    );
    assert!(
        !nudged.contains(&ObjectId(4)),
        "unit off the mover footprint must not fidget"
    );
}

/// hq-8lb00: oriented box, not a clamped selection-radius square.
#[test]
fn structure_box_footprint_is_not_clamped_square() {
    use crate::game_logic::{
        HostGeometryInfo, HostGeometryType, KindOf, Object, ObjectId, Team, ThingTemplate,
    };
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("LongThinFactory");
    tmpl.add_kind_of(KindOf::Structure);
    tmpl.geometry_info = HostGeometryInfo {
        geom_type: HostGeometryType::Box,
        is_small: false,
        height: 20.0,
        major_radius: 40.0,
        minor_radius: 8.0,
        authored: true,
    };
    let mut factory = Object::new(tmpl, ObjectId(1), Team::USA);
    factory.set_position(Vec3::new(80.0, 0.0, 80.0));
    factory.set_orientation(0.0);
    objects.insert(factory.id, factory);
    sys.apply_structure_static_blocks(&objects);
    let center = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 80.0));
    assert!(sys.grid.is_static_blocked(center), "box center must stamp");
    let along_major = sys.grid.world_to_grid(Vec3::new(110.0, 0.0, 80.0));
    assert!(
        sys.grid.is_static_blocked(along_major),
        "long major axis must stamp past a 4-cell clamp"
    );
    let off_minor = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 110.0));
    assert!(
        !sys.grid.is_static_blocked(off_minor),
        "thin minor axis must not become a square"
    );
}

/// hq-hjx23: C++ pinch pass after structure stamp.
#[test]
fn structure_placement_runs_pinch_pass() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("AmericaWarFactory");
    tmpl.add_kind_of(KindOf::Structure);
    let mut factory = Object::new(tmpl, ObjectId(2), Team::USA);
    factory.set_position(Vec3::new(80.0, 0.0, 80.0));
    factory.selection_radius = 15.0;
    objects.insert(factory.id, factory);
    sys.apply_structure_static_blocks(&objects);
    let center = sys.grid.world_to_grid(Vec3::new(80.0, 0.0, 80.0));
    assert!(sys.grid.is_static_blocked(center));
    let mut found_pinched = false;
    for dy in -4..=4 {
        for dx in -4..=4 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let n = GridPos::new(center.x + dx, center.y + dy);
            if sys.grid.is_valid_pos(n)
                && sys.grid.cell_type(n) == PathfindCellType::Clear
                && sys.grid.is_pinched(n)
            {
                found_pinched = true;
            }
        }
    }
    assert!(
        found_pinched,
        "C++ MARK_BORDER_PINCHED must mark obstacle-adjacent Clear cells"
    );
}

/// hq-nkgtf: adjustDestination must not snap into a disconnected pocket.
#[test]
fn adjust_destination_rejects_disconnected_pocket() {
    let mut g = open_grid(16, 16);
    for y in 0..16 {
        g.set_cell_type(GridPos::new(8, y), PathfindCellType::Water);
    }
    g.rebuild_path_zones();
    let from = GridPos::new(2, 8);
    let dest = GridPos::new(12, 8);
    assert_ne!(g.path_zone(from), g.path_zone(dest));
    g.query_from = Some(from);
    g.query_orig_dest = Some(from);
    let snapped = g.adjust_destination_ex(dest, SURFACE_GROUND, false, 64, None, 0);
    if let Some(cell) = snapped {
        assert_eq!(
            g.path_zone(cell),
            g.path_zone(from),
            "must not accept the far-bank pocket cell {cell:?}"
        );
    }
    assert_ne!(snapped, Some(dest), "disconnected dest must not be kept");
}

/// hq-zjbin: parked/taxiing aircraft do not stamp UNIT_PRESENT.
#[test]
fn grounded_aircraft_do_not_stamp_unit_present() {
    use crate::game_logic::{KindOf, Object, ObjectId, ObjectType, Team, ThingTemplate};
    let mut g = open_grid(16, 16);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("AmericaJetRaptor");
    tmpl.add_kind_of(KindOf::Aircraft);
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut jet = Object::new(tmpl, ObjectId(7), Team::USA);
    jet.object_type = ObjectType::Aircraft;
    jet.loco_appearance = LocomotorAppearance::Wings;
    jet.status.airborne_target = false;
    jet.set_position(Vec3::new(55.0, 0.0, 55.0));
    jet.owner_player_id = Some(0);
    objects.insert(jet.id, jet);
    g.update_dynamic_obstacles(&objects);
    let center = g.world_to_grid(Vec3::new(55.0, 0.0, 55.0));
    assert!(
        !g.is_blocked(center),
        "C++ updatePos never stamps air-movement UNIT_PRESENT"
    );
}

/// hq-2pdvs: invented Structure+radius>=20 set must not detour aircraft.
#[test]
fn tall_building_detour_requires_aircraft_path_around() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("WideBarracks");
    tmpl.add_kind_of(KindOf::Structure);
    let mut bldg = Object::new(tmpl, ObjectId(3), Team::USA);
    bldg.set_position(Vec3::new(50.0, 0.0, 0.0));
    bldg.selection_radius = 30.0;
    objects.insert(bldg.id, bldg);
    let from = Vec3::new(0.0, 40.0, 0.0);
    let to = Vec3::new(100.0, 40.0, 0.0);
    let path = PathfindingSystem::detour_path_around_tall_buildings(&[from, to], &objects);
    assert_eq!(
        path.len(),
        2,
        "without AIRCRAFT_PATH_AROUND, no invented tall detour"
    );
    let adj = PathfindingSystem::circle_clips_tall_building(from, to, 80.0, &objects, None);
    assert!(adj.is_none(), "circleClips is AIRCRAFT_PATH_AROUND only");
}

/// hq-0i8ht: human checkForAdjust / A* stay inside m_logicalExtent.
#[test]
fn human_units_stay_in_logical_extent() {
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    sys.grid
        .set_logical_extent(GridPos::new(0, 0), GridPos::new(10, 10));
    sys.set_human_player_mask(1u16 << 0);
    sys.grid.set_query_is_human(true);

    assert!(sys.grid.in_logical_extent(GridPos::new(5, 5)));
    assert!(!sys.grid.in_logical_extent(GridPos::new(15, 5)));

    let inside = sys.grid.adjust_destination_on_layer(
        GridPos::new(5, 5),
        SURFACE_GROUND,
        false,
        8,
        Some(0),
        0,
        PathfindLayerEnum::Ground,
    );
    assert!(inside.is_some(), "human dest inside logical extent is ok");

    let outside = sys.grid.adjust_destination_on_layer(
        GridPos::new(15, 5),
        SURFACE_GROUND,
        false,
        4,
        Some(0),
        0,
        PathfindLayerEnum::Ground,
    );
    if let Some(adj) = outside {
        assert!(
            sys.grid.in_logical_extent(adj),
            "human adjust must not land outside m_logicalExtent"
        );
    }

    sys.grid.set_query_is_human(false);
    let ai = sys.grid.adjust_destination_on_layer(
        GridPos::new(15, 5),
        SURFACE_GROUND,
        false,
        4,
        Some(1),
        0,
        PathfindLayerEnum::Ground,
    );
    assert!(ai.is_some(), "computer players may leave the logical map");

    let mut objects = HashMap::new();
    let tmpl = ThingTemplate::new("Ranger");
    let mut unit = Object::new(tmpl, ObjectId(1), Team::USA);
    unit.set_position(Vec3::new(20.0, 0.0, 20.0));
    unit.owner_player_id = Some(0);
    objects.insert(unit.id, unit);
    sys.grid.set_query_is_human(true);
    let path = sys.find_path_ex(
        Vec3::new(20.0, 0.0, 20.0),
        Vec3::new(180.0, 0.0, 20.0),
        &objects,
        false,
        Some(ObjectId(1)),
    );
    if let Some(path) = path {
        for p in &path {
            let cell = sys.grid.world_to_grid(*p);
            assert!(
                sys.grid.in_logical_extent(cell),
                "human A* waypoint {:?} outside logical extent",
                cell
            );
        }
    }
}

/// hq-8kkhs: dozerHack reaches live crate A*.
#[test]
fn dozer_hack_reaches_live_astar() {
    let mut sys = PathfindingSystem::new(120.0, 80.0);
    for y in 0..8 {
        sys.grid
            .set_cell_type(GridPos::new(5, y), PathfindCellType::Impassable);
    }
    sys.grid.set_cell_obstacle_owned(
        GridPos::new(5, 3),
        false,
        false,
        20,
        Some(0),
        Some(Team::USA),
    );

    let mut objects = HashMap::new();
    let mut dozer_t = ThingTemplate::new("AmericaDozer");
    dozer_t.add_kind_of(KindOf::Dozer);
    dozer_t.add_kind_of(KindOf::Vehicle);
    let mut dozer = Object::new(dozer_t, ObjectId(1), Team::USA);
    dozer.set_position(Vec3::new(20.0, 0.0, 35.0));
    dozer.owner_player_id = Some(0);
    objects.insert(dozer.id, dozer);

    let start = Vec3::new(20.0, 0.0, 35.0);
    let goal = Vec3::new(90.0, 0.0, 35.0);
    let path = sys
        .find_path_ex(start, goal, &objects, false, Some(ObjectId(1)))
        .expect("dozer must step through non-enemy CELL_OBSTACLE");
    assert!(path.len() >= 2);

    let mut inf_t = ThingTemplate::new("Ranger");
    inf_t.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(inf_t, ObjectId(2), Team::USA);
    inf.set_position(start);
    inf.owner_player_id = Some(0);
    let mut inf_objects = HashMap::new();
    inf_objects.insert(inf.id, inf);
    let blocked = sys.find_path_ex(start, goal, &inf_objects, false, Some(ObjectId(2)));
    assert!(
        blocked.is_none(),
        "non-dozer cannot dozerHack a CELL_OBSTACLE gap"
    );

    sys.grid.set_cell_obstacle_owned(
        GridPos::new(5, 3),
        false,
        false,
        21,
        Some(2),
        Some(Team::GLA),
    );
    let enemy = sys.find_path_ex(start, goal, &objects, false, Some(ObjectId(1)));
    assert!(
        enemy.is_none(),
        "dozer must not dozerHack an ENEMIES obstacle"
    );
}

/// hq-ugt0x: downhill-only is not hardcoded false on the live A* call.
#[test]
fn downhill_only_reaches_live_astar() {
    let src = include_str!("../system_requests.rs");
    let i = src
        .find("fn find_path_via_crate")
        .expect("find_path_via_crate");
    let w = &src[i..src.len().min(i + 9000)];
    assert!(
        w.contains("downhill_only") && w.contains("seeker_downhill_only"),
        "live A* must pass seeker downhill_only, not a hardcoded false"
    );
    assert!(
        w.contains("seed_line") && w.contains("!downhill_only"),
        "seed line must be suppressed when downhill-only"
    );
    assert!(
        w.contains("dozer_ok_ref") && w.contains("cell_allowed"),
        "dozerHack and human cell_allowed must reach crate A*"
    );
    assert!(
        !w.contains("downhill_only: false"),
        "must not hardcode downhill_only: false"
    );
    assert!(
        w.contains("ignored_obstacle_cells") || w.contains("let ignore_cells"),
        "ignoreObstacle footprint must reach crate A* ignore_cells"
    );
    assert!(
        !w.contains("nearest_static_open"),
        "find_path_via_crate must not fall back to nearest_static_open"
    );
}

/// hq-9za5p: findClosestPath walks to a valid neighbor when the goal is blocked.
#[test]
fn find_closest_path_walks_to_valid_cell_when_goal_impassable() {
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let goal = GridPos::new(12, 5);
    sys.grid.set_cell_type(goal, PathfindCellType::Impassable);
    let from = Vec3::new(20.0, 0.0, 50.0);
    let to = sys.grid.grid_to_world(goal);
    let path = sys
        .find_closest_path(from, to, SURFACE_GROUND, false, true)
        .expect("closest path");
    let end = *path.last().expect("end");
    let end_cell = sys.grid.world_to_grid(end);
    assert_ne!(end_cell, goal, "must not stand on the impassable click");
    assert!(
        sys.grid.cell_passable_for(end_cell, SURFACE_GROUND, false),
        "closest dest {end_cell:?} must be a valid destination"
    );
    let d = (end_cell.x - goal.x).abs() + (end_cell.y - goal.y).abs();
    assert!(d <= 2, "should hug the blocked cell, end={end_cell:?}");
}

/// hq-1g4ym: flee cell stays outside both repulsor radii.
#[test]
fn find_safe_path_stays_outside_both_repulsors() {
    let mut sys = PathfindingSystem::new(400.0, 400.0);
    let from = Vec3::new(100.0, 0.0, 100.0);
    let r1 = Vec3::new(100.0, 0.0, 100.0);
    let r2 = Vec3::new(160.0, 0.0, 100.0);
    let radius = 40.0;
    let path = sys
        .find_safe_path_from(from, r1, r2, radius, SURFACE_GROUND, false, true)
        .expect("safe path");
    let end = *path.last().expect("end");
    let d1 = (end.x - r1.x).hypot(end.z - r1.z);
    let d2 = (end.x - r2.x).hypot(end.z - r2.z);
    assert!(
        d1.min(d2) > radius * 0.9,
        "end {end:?} must leave both radii d1={d1} d2={d2}"
    );
}

/// hq-2ugs5: checkPathCost straight line passes the 1.4*(dx+dy) gate.
#[test]
fn check_path_cost_straight_line_passes_group_gate() {
    let g = open_grid(32, 32);
    let from = Vec3::new(20.0, 0.0, 20.0);
    let to = Vec3::new(80.0, 0.0, 20.0);
    let cost = g.check_path_cost(SURFACE_GROUND, false, from, to);
    let dx = (to.x - from.x).abs();
    let dz = (to.z - from.z).abs();
    assert!(
        cost <= 1.4 * (dx + dz) + 1.0,
        "straight cost {cost} should pass 1.4*(dx+dz)={}",
        1.4 * (dx + dz)
    );
    let bad = g.check_path_cost(SURFACE_GROUND, false, Vec3::new(-100.0, 0.0, -100.0), to);
    assert!(bad >= 0x7fff_0000u32 as f32 * 0.5);
}

/// hq-2ugs5: groupDest cost gate rejects a long detour around a wall.
#[test]
fn adjust_destination_for_group_rejects_long_detour() {
    let mut g = open_grid(24, 24);
    // Wall between group click (4,10) and offset dest (16,10).
    for y in 0..24 {
        g.set_cell_type(GridPos::new(10, y), PathfindCellType::Impassable);
    }
    // Leave a far gap at the top so a path exists but is expensive.
    g.set_cell_type(GridPos::new(10, 22), PathfindCellType::Clear);
    let dest = GridPos::new(16, 10);
    let group = GridPos::new(4, 10);
    let snapped = g.adjust_destination_for_group(
        dest,
        group,
        SURFACE_GROUND,
        false,
        None,
        0,
        PathfindLayerEnum::Ground,
    );
    if let Some(cell) = snapped {
        let cost = g.check_path_cost(
            SURFACE_GROUND,
            false,
            g.grid_to_world(group),
            g.grid_to_world(cell),
        );
        let w = g.grid_to_world(cell);
        let gw = g.grid_to_world(group);
        let dx = (gw.x - w.x).abs();
        let dz = (gw.z - w.z).abs();
        assert!(
            cost <= 1.4 * (dx + dz) + 1.0 || cell != dest,
            "group adjust must reject the far-side dest or pick a cheap cell, got {cell:?} cost={cost}"
        );
    }
}

/// hq-f4q2o: attack A* walks around a partial wall instead of through it.
#[test]
fn find_attack_path_goes_around_partial_wall() {
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    for y in 0..12 {
        sys.grid
            .set_cell_type(GridPos::new(10, y), PathfindCellType::Impassable);
    }
    let objects = HashMap::new();
    let from = Vec3::new(20.0, 0.0, 50.0);
    let victim = Vec3::new(150.0, 0.0, 50.0);
    if let Some(p) = sys.find_attack_firing_position(from, victim, 80.0, &objects, false, None) {
        let crosses = p.windows(2).any(|w| {
            let a = sys.grid.world_to_grid(w[0]);
            let b = sys.grid.world_to_grid(w[1]);
            sys.grid.cell_type(a) == PathfindCellType::Impassable
                || sys.grid.cell_type(b) == PathfindCellType::Impassable
        });
        assert!(
            !crosses,
            "attack A* must not step on Impassable, path={p:?}"
        );
        let end = *p.last().unwrap();
        let dist = (end.x - victim.x).hypot(end.z - victim.z);
        assert!(
            dist <= 80.0 + 15.0,
            "firing cell should be in/near range, end={end:?} dist={dist}"
        );
    }
}

/// hq-4wg8j: seed line aborts on allied-fixed and crushable enemies.
#[test]
fn seed_line_aborts_allied_fixed_and_crushable_enemies() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut g = open_grid(24, 24);
    let mut masks = [0u16; 16];
    masks[0] = 1u16 << 0 | 1u16 << 1;
    masks[1] = 1u16 << 0 | 1u16 << 1;
    g.set_player_ally_masks(masks);

    let mut objects = HashMap::new();
    let mut ally_t = ThingTemplate::new("AllyTank");
    ally_t.add_kind_of(KindOf::Vehicle);
    let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
    ally.set_position(g.grid_to_world(GridPos::new(8, 8)));
    ally.owner_player_id = Some(1);
    ally.crushable_level = 1;
    objects.insert(ally.id, ally);

    let mut car_t = ThingTemplate::new("CivilianCar");
    car_t.add_kind_of(KindOf::Vehicle);
    let mut car = Object::new(car_t, ObjectId(3), Team::GLA);
    car.set_position(g.grid_to_world(GridPos::new(10, 8)));
    car.owner_player_id = Some(2);
    car.crushable_level = 1;
    objects.insert(car.id, car);

    g.update_dynamic_obstacles(&objects);
    let layer = PathfindLayerEnum::Ground;
    assert!(
        !g.seed_line_occupancy_ok(GridPos::new(8, 8), Some(0), masks[0], false, layer),
        "allied-fixed must abort the seed line"
    );
    assert!(
        !g.seed_line_occupancy_ok(GridPos::new(10, 8), Some(0), masks[0], false, layer),
        "crushable enemy-fixed must abort the seed line"
    );
    assert!(
        g.seed_line_occupancy_ok(GridPos::new(4, 4), Some(0), masks[0], false, layer),
        "empty cell must seed"
    );
}
