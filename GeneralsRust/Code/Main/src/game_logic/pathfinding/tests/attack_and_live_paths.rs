use super::super::*;

fn open_grid(w: i32, h: i32) -> PathfindingGrid {
    PathfindingGrid::new(w as f32 * 10.0, h as f32 * 10.0, 10.0)
}

/// hq-6a3ks: attack LOS leftover airborne + KINDOF gates.
#[test]
fn attack_los_airborne_and_kindof_gates() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    for y in 0..20 {
        sys.grid.set_cell_obstacle_owned(
            GridPos::new(10, y),
            false,
            false,
            99,
            Some(2),
            Some(Team::GLA),
        );
    }
    let from = Vec3::new(20.0, 0.0, 50.0);
    let to = Vec3::new(150.0, 0.0, 50.0);

    let mut atk_t = ThingTemplate::new("Ranger");
    atk_t.add_kind_of(KindOf::Infantry);
    let no_los = Object::new(atk_t.clone(), ObjectId(1), Team::USA);
    assert!(
        !sys.is_attack_view_blocked_for(from, to, Some(&no_los), None),
        "units without AttackNeedsLineOfSight must not refuse cells behind a building"
    );

    atk_t.add_kind_of(KindOf::AttackNeedsLineOfSight);
    let atk = Object::new(atk_t, ObjectId(1), Team::USA);
    assert!(
        sys.is_attack_view_blocked_for(from, to, Some(&atk), None),
        "LOS kind must still see the opaque wall"
    );

    let mut air_t = ThingTemplate::new("Raptor");
    air_t.add_kind_of(KindOf::Aircraft);
    let mut air = Object::new(air_t, ObjectId(7), Team::GLA);
    air.set_position(Vec3::new(150.0, 20.0, 50.0));
    air.ground_height = 0.0;
    assert!(
        air.is_significantly_above_terrain(),
        "control: victim is airborne"
    );
    assert!(
        !sys.is_attack_view_blocked_for(from, to, Some(&atk), Some(&air)),
        "airborne victim must not count deck/structure cells as blocked LOS"
    );
}

/// hq-nxr47: findAttackPath A* applies leftover LOS_TERRAIN.
#[test]
fn attack_los_terrain_blocks_ridge_for_mobile_weapon() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let w = sys.grid.width();
    let h = sys.grid.height();
    let mut heights = vec![0.0f32; (w * h) as usize];
    let mid = w / 2;
    for y in (h / 2 - 1).max(0)..=(h / 2 + 1).min(h - 1) {
        for x in (mid - 1).max(0)..=(mid + 1).min(w - 1) {
            heights[(y * w + x) as usize] = 80.0;
        }
    }
    sys.set_terrain_height_samples(w, h, heights);

    let from = Vec3::new(20.0, 0.0, 50.0);
    let to = Vec3::new(150.0, 0.0, 50.0);
    let mut atk_t = ThingTemplate::new("Ranger");
    atk_t.add_kind_of(KindOf::Infantry);
    atk_t.add_kind_of(KindOf::AttackNeedsLineOfSight);
    let mut atk = Object::new(atk_t.clone(), ObjectId(1), Team::USA);
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        ..Weapon::default()
    });
    atk.selection_radius = 5.0;
    let mut victim = Object::new(
        {
            let mut t = ThingTemplate::new("Rebel");
            t.add_kind_of(KindOf::Infantry);
            t
        },
        ObjectId(2),
        Team::GLA,
    );
    victim.selection_radius = 5.0;

    assert!(
        sys.is_attack_view_blocked_for(from, to, Some(&atk), Some(&victim)),
        "mobile weapon must treat a ridge as blocked LOS_TERRAIN"
    );

    let unarmed = Object::new(atk_t, ObjectId(4), Team::USA);
    assert!(
        sys.is_attack_view_blocked_for(from, to, Some(&unarmed), Some(&victim)),
        "leftover no-weapon mobile still applies terrain LOS"
    );

    let mut turret_t = ThingTemplate::new("Patriot");
    turret_t.add_kind_of(KindOf::AttackNeedsLineOfSight);
    turret_t.add_kind_of(KindOf::Immobile);
    let mut turret = Object::new(turret_t, ObjectId(3), Team::USA);
    turret.weapon = Some(Weapon {
        damage: 10.0,
        range: 100.0,
        ..Weapon::default()
    });
    assert!(
        !sys.is_attack_view_blocked_for(from, to, Some(&turret), Some(&victim)),
        "immobile attackers skip terrain LOS (cannot path around)"
    );

    let src = include_str!("../system_requests.rs");
    let i = src
        .find("pub fn is_attack_view_blocked_for(")
        .expect("is_attack_view_blocked_for");
    let body = &src[i..src.len().min(i + 3500)];
    assert!(
        body.contains("LOS_TERRAIN")
            && body.contains("KindOf::Immobile")
            && body.contains("is_clear_line_of_sight_terrain"),
        "isAttackViewBlockedByObstacle must apply leftover LOS_TERRAIN"
    );
}

/// hq-nxr47: A* keeps searching when the first in-range cell is ridge-blocked.
#[test]
fn find_attack_path_circles_ridge_to_clear_shot() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate, Weapon};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let w = sys.grid.width();
    let h = sys.grid.height();
    let mut heights = vec![0.0f32; (w * h) as usize];
    let mid = w / 2;
    // Hill only on the east-west midline so a northern firing cell stays clear.
    for x in (mid - 1).max(0)..=(mid + 1).min(w - 1) {
        heights[(5 * w + x) as usize] = 80.0;
    }
    sys.set_terrain_height_samples(w, h, heights);

    let mut objects = HashMap::new();
    let mut atk_t = ThingTemplate::new("Ranger");
    atk_t.add_kind_of(KindOf::Infantry);
    atk_t.add_kind_of(KindOf::AttackNeedsLineOfSight);
    let mut atk = Object::new(atk_t, ObjectId(1), Team::USA);
    atk.set_position(Vec3::new(20.0, 0.0, 50.0));
    atk.owner_player_id = Some(0);
    atk.selection_radius = 5.0;
    atk.locomotor_surfaces = SURFACE_GROUND;
    atk.weapon = Some(Weapon {
        damage: 10.0,
        range: 90.0,
        ..Weapon::default()
    });
    objects.insert(atk.id, atk);

    let mut tgt_t = ThingTemplate::new("Rebel");
    tgt_t.add_kind_of(KindOf::Infantry);
    let mut tgt = Object::new(tgt_t, ObjectId(2), Team::GLA);
    tgt.set_position(Vec3::new(150.0, 0.0, 50.0));
    tgt.selection_radius = 5.0;
    objects.insert(tgt.id, tgt);

    let path = sys
        .find_attack_firing_position(
            Vec3::new(20.0, 0.0, 50.0),
            Vec3::new(150.0, 0.0, 50.0),
            90.0,
            &objects,
            false,
            Some(ObjectId(1)),
        )
        .expect("must find a terrain-clear firing cell");
    let goal = *path.last().unwrap();
    let atk = objects.get(&ObjectId(1)).unwrap();
    let tgt = objects.get(&ObjectId(2)).unwrap();
    assert!(
        !sys.is_attack_view_blocked_for(goal, Vec3::new(150.0, 0.0, 50.0), Some(atk), Some(tgt)),
        "chosen firing cell must have leftover LOS_TERRAIN, goal={goal:?}"
    );
}

/// hq-p8eko: findAttackPath A* occupancy costs match leftover.
#[test]
fn attack_step_occupancy_matches_leftover() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut g = open_grid(24, 24);
    let mut masks = [0u16; 16];
    masks[0] = 1u16 << 0 | 1u16 << 1;
    masks[1] = 1u16 << 0 | 1u16 << 1;
    g.set_player_ally_masks(masks);

    let mut objects = HashMap::new();
    let mut enemy_t = ThingTemplate::new("Bunker");
    enemy_t.add_kind_of(KindOf::Vehicle);
    let mut enemy = Object::new(enemy_t, ObjectId(1), Team::GLA);
    enemy.set_position(g.grid_to_world(GridPos::new(5, 5)));
    enemy.owner_player_id = Some(2);
    enemy.crushable_level = 0;
    objects.insert(enemy.id, enemy);

    let mut ally_t = ThingTemplate::new("Humvee");
    ally_t.add_kind_of(KindOf::Vehicle);
    let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
    ally.set_position(g.grid_to_world(GridPos::new(6, 6)));
    ally.owner_player_id = Some(1);
    objects.insert(ally.id, ally);

    let mut goal_t = ThingTemplate::new("Ranger");
    goal_t.add_kind_of(KindOf::Infantry);
    let mut goaled = Object::new(goal_t, ObjectId(3), Team::USA);
    goaled.set_position(g.grid_to_world(GridPos::new(1, 1)));
    goaled.owner_player_id = Some(0);
    goaled.movement.target_position = Some(g.grid_to_world(GridPos::new(8, 8)));
    objects.insert(goaled.id, goaled);

    g.update_dynamic_obstacles(&objects);
    let start = GridPos::new(4, 4);
    let layer = PathfindLayerEnum::Ground;
    assert!(
        g.attack_step_occupancy(
            GridPos::new(5, 5),
            start,
            Some(0),
            masks[0],
            false,
            true,
            0,
            layer
        )
        .is_none(),
        "uncrushable enemy-fixed must skip"
    );
    assert_eq!(
        g.attack_step_occupancy(
            GridPos::new(6, 6),
            start,
            Some(0),
            masks[0],
            false,
            true,
            0,
            layer
        ),
        Some(3 * 14),
        "ally-fixed +3*DIAG"
    );
    assert_eq!(
        g.attack_step_occupancy(
            GridPos::new(8, 8),
            start,
            Some(0),
            masks[0],
            false,
            true,
            0,
            layer
        ),
        Some(3 * 10),
        "ally-goal vehicle +3*ORTHO"
    );
    assert_eq!(
        g.attack_step_occupancy(
            GridPos::new(8, 8),
            start,
            Some(0),
            masks[0],
            false,
            false,
            0,
            layer
        ),
        Some(10),
        "ally-goal infantry +ORTHO"
    );
}

/// hq-d38ae: vehicle strip-back walks parent chain and retreats off congestion.
#[test]
fn attack_vehicle_strip_back_retreats_off_busy_ally() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut masks = [0u16; 16];
    masks[0] = 1u16 << 0 | 1u16 << 1;
    masks[1] = 1u16 << 0 | 1u16 << 1;
    sys.grid.set_player_ally_masks(masks);

    let mut objects = HashMap::new();
    let mut tank_t = ThingTemplate::new("Crusader");
    tank_t.add_kind_of(KindOf::Vehicle);
    tank_t.add_kind_of(KindOf::AttackNeedsLineOfSight);
    let mut tank = Object::new(tank_t, ObjectId(1), Team::USA);
    tank.set_position(Vec3::new(20.0, 0.0, 50.0));
    tank.owner_player_id = Some(0);
    tank.selection_radius = 8.0;
    tank.locomotor_surfaces = SURFACE_GROUND;
    objects.insert(tank.id, tank);

    let mut ally_t = ThingTemplate::new("Humvee");
    ally_t.add_kind_of(KindOf::Vehicle);
    let mut ally = Object::new(ally_t, ObjectId(2), Team::China);
    // Parked, not idle: ally-fixed on the approach parent chain.
    ally.set_position(Vec3::new(70.0, 0.0, 50.0));
    ally.owner_player_id = Some(1);
    ally.ai_state = crate::game_logic::AIState::Attacking;
    objects.insert(ally.id, ally);

    let path = sys.find_attack_firing_position(
        Vec3::new(20.0, 0.0, 50.0),
        Vec3::new(150.0, 0.0, 50.0),
        80.0,
        &objects,
        false,
        Some(ObjectId(1)),
    );
    if let Some(p) = path {
        let ally_cell = sys.grid.world_to_grid(Vec3::new(70.0, 0.0, 50.0));
        let end = sys.grid.world_to_grid(*p.last().unwrap());
        assert_ne!(
            end, ally_cell,
            "vehicle strip-back must not park on the busy ally, path={p:?}"
        );
    }
}

/// hq-gd0jd: live grid_to_world samples deck / terrain Y, not 0.
#[test]
fn grid_to_world_uses_deck_height() {
    let mut g = open_grid(16, 16);
    g.stamp_bridge_deck(
        Vec3::new(20.0, 20.0, 40.0),
        Vec3::new(20.0, 20.0, 60.0),
        Vec3::new(90.0, 20.0, 40.0),
        Vec3::new(90.0, 20.0, 60.0),
        false,
    );
    let deck = GridPos::new(5, 5);
    let world = g.grid_to_world(deck);
    assert!(
        world.y > 10.0,
        "deck cell must emit layer height, got Y={}",
        world.y
    );
    let layered = g.grid_to_world_on_layer(
        deck,
        g.layer_for_destination(Vec3::new(world.x, 20.0, world.z)),
    );
    assert!(
        (layered.y - 20.0).abs() < 2.0 || layered.y > 10.0,
        "on-layer deck Y should track the span, got {}",
        layered.y
    );
}

/// hq-7tj9x: findClosest rebuilds via crate A*, not thin host A* / fail-open line.
#[test]
fn find_closest_path_rebuilds_via_crate_not_thin_line() {
    let src = include_str!("../system_routes.rs");
    let i = src
        .find("pub fn find_closest_path(")
        .expect("find_closest_path");
    let w = &src[i..src.len().min(i + 7000)];
    assert!(
        w.contains("find_path_via_crate"),
        "closest must rebuild with leftover crate A*"
    );
    assert!(
        !w.contains("self.grid.find_path("),
        "closest must not reconstruct with thin host A*"
    );
    assert!(
        w.contains("no fail-open line"),
        "closest must not fail-open a straight from→to line"
    );
    assert!(
        w.contains("enqueue_connect_layer"),
        "closest search must hop checkChangeLayers"
    );
}

/// hq-ajphf: findClosestPath accepts an occupied goal when canPathThroughUnits.
#[test]
fn find_closest_path_accepts_occupied_goal_when_can_path_through_units() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let goal = GridPos::new(12, 5);
    let from = Vec3::new(20.0, 0.0, 50.0);
    let to = sys.grid.grid_to_world(goal);

    let mut objects = HashMap::new();
    let mut inf_t = ThingTemplate::new("Ranger");
    inf_t.add_kind_of(KindOf::Infantry);
    let mut seeker = Object::new(inf_t.clone(), ObjectId(1), Team::USA);
    seeker.set_position(from);
    seeker.owner_player_id = Some(0);
    seeker.selection_radius = 1.0;
    seeker.locomotor_surfaces = SURFACE_GROUND;
    seeker.can_path_through_units = false;
    objects.insert(seeker.id, seeker);

    let mut parked = Object::new(inf_t, ObjectId(2), Team::USA);
    parked.set_position(to);
    parked.owner_player_id = Some(0);
    parked.selection_radius = 1.0;
    objects.insert(parked.id, parked);

    sys.grid.update_dynamic_obstacles(&objects);
    sys.bind_seeker_from_mover(&objects, Some(ObjectId(1)));
    sys.apply_seeker_human_flag();

    let without = sys
        .find_closest_path(from, to, SURFACE_GROUND, false, true)
        .expect("closest without tunnel");
    let without_cell = sys.grid.world_to_grid(*without.last().unwrap());
    assert_ne!(
        without_cell, goal,
        "without canPathThroughUnits must walk to a nearby empty cell, end={without_cell:?}"
    );

    objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .can_path_through_units = true;
    sys.bind_seeker_from_mover(&objects, Some(ObjectId(1)));
    assert!(
        sys.seeker_can_path_through_units,
        "bind must copy leftover canPathThroughUnits"
    );
    let with = sys
        .find_closest_path(from, to, SURFACE_GROUND, false, true)
        .expect("closest with tunnel");
    let with_cell = sys.grid.world_to_grid(*with.last().unwrap());
    assert_eq!(
        with_cell, goal,
        "canPathThroughUnits must accept the occupied goal, end={with_cell:?}"
    );

    let src = include_str!("../system_routes.rs");
    let i = src
        .find("pub fn find_closest_path(")
        .expect("find_closest_path");
    let body = &src[i..src.len().min(i + 2500)];
    assert!(
        body.contains("seeker_can_path_through_units"),
        "findClosestPath must honor leftover canPathThroughUnits goal accept"
    );
}

/// hq-7tj9x: closest path around a blocked factory does not walk through it.
#[test]
fn find_closest_path_does_not_cut_through_building() {
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    for y in 0..12 {
        sys.grid
            .set_cell_type(GridPos::new(8, y), PathfindCellType::Impassable);
    }
    let goal = GridPos::new(12, 5);
    sys.grid.set_cell_type(goal, PathfindCellType::Impassable);
    let from = Vec3::new(20.0, 0.0, 50.0);
    let to = sys.grid.grid_to_world(goal);
    let path = sys
        .find_closest_path(from, to, SURFACE_GROUND, false, true)
        .expect("closest path");
    let crosses = path
        .iter()
        .any(|p| sys.grid.cell_type(sys.grid.world_to_grid(*p)) == PathfindCellType::Impassable);
    assert!(
        !crosses,
        "closest crate rebuild must not stand on Impassable, path={path:?}"
    );
}

/// hq-hy7cu: closest search hops onto a deck cell via checkChangeLayers.
#[test]
fn find_closest_path_hops_connect_layer_onto_deck() {
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    sys.grid.stamp_bridge_deck(
        Vec3::new(20.0, 20.0, 40.0),
        Vec3::new(20.0, 20.0, 60.0),
        Vec3::new(90.0, 20.0, 40.0),
        Vec3::new(90.0, 20.0, 60.0),
        false,
    );
    let deck = GridPos::new(5, 5);
    let world = sys.grid.grid_to_world(deck);
    let on_deck = Vec3::new(world.x, 20.0, world.z);
    let layer = sys.grid.layer_for_destination(on_deck);
    assert_ne!(
        layer,
        PathfindLayerEnum::Ground,
        "deck click must pick the span"
    );
    let from = sys.grid.grid_to_world(GridPos::new(1, 5));
    let path = sys
        .find_closest_path(from, on_deck, SURFACE_GROUND, false, true)
        .expect("closest onto deck");
    let end = *path.last().expect("end");
    let end_layer = sys
        .grid
        .layer_for_destination(Vec3::new(end.x, end.y.max(20.0), end.z));
    assert!(
        end_layer != PathfindLayerEnum::Ground || end.y > 5.0,
        "closest dest should be the deck cell, end={end:?} layer={end_layer:?}"
    );
}

/// hq-hy7cu: flee search also hops connect-layer.
#[test]
fn find_safe_path_enqueues_change_layers() {
    let src = include_str!("../system_routes.rs");
    let i = src
        .find("pub fn find_safe_path_from(")
        .expect("find_safe_path_from");
    let w = &src[i..src.len().min(i + 5000)];
    assert!(
        w.contains("enqueue_connect_layer"),
        "safe search must hop checkChangeLayers"
    );
    assert!(
        w.contains("cell_passable_for_layer"),
        "safe neighbors expand on the current layer"
    );
}

/// hq-cg5on: snapClosestGoalPosition offsets off an occupied pad cell.
#[test]
fn snap_closest_goal_avoids_fixed_occupant() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut parked = Object::new(tmpl, ObjectId(7), Team::USA);
    let pad = sys.grid.grid_to_world(GridPos::new(8, 8));
    parked.set_position(pad);
    parked.owner_player_id = Some(0);
    parked.selection_radius = 1.0;
    objects.insert(parked.id, parked);
    sys.grid.update_dynamic_obstacles(&objects);
    sys.seeker_id = Some(ObjectId(8));
    sys.seeker_player = Some(0);
    let snapped = sys.snap_closest_goal_position(pad, SURFACE_GROUND, false, 0.0);
    let snapped_cell = sys.grid.world_to_grid(snapped);
    assert_ne!(
        snapped_cell,
        GridPos::new(8, 8),
        "snap must leave the occupied pad, got {snapped:?}"
    );
    let d = (snapped_cell.x - 8).abs() + (snapped_cell.y - 8).abs();
    assert!(d <= 2, "snap should stay in the 3×3, cell={snapped_cell:?}");
}

/// hq-hlj28: processPathfindQueue stops at PATHFIND_CELLS_PER_FRAME.
#[test]
fn process_pathfind_queue_stops_at_cell_budget() {
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    sys.set_pathfind_cells_per_frame(1);
    let mk = |id: u32, z: f32| PendingHostPath {
        unit_id: ObjectId(id),
        start: Vec3::new(10.0, 0.0, z),
        destination: Vec3::new(80.0, 0.0, z),
        waypoints: Vec::new(),
        aircraft: false,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        ignore_obstacle: None,
    };
    assert!(sys.queue_path(mk(1, 10.0)));
    assert!(sys.queue_path(mk(2, 30.0)));
    assert!(sys.queue_path(mk(3, 50.0)));
    assert_eq!(sys.pending_path_count(), 3);
    let objects = HashMap::new();
    sys.begin_pathfind_queue_frame();
    let mut processed = 0usize;
    while sys.pathfind_budget_remaining() {
        let Some(req) = sys.pop_pending_path() else {
            break;
        };
        let _ = sys.find_path_ex(
            req.start,
            req.destination,
            &objects,
            false,
            Some(req.unit_id),
        );
        processed += 1;
        if processed > 8 {
            break;
        }
    }
    assert_eq!(
        processed, 1,
        "budget 1 cell must stop after the first search"
    );
    assert_eq!(
        sys.pending_path_count(),
        2,
        "later waiters stay queued for a later frame"
    );
}

/// hq-xa155: ignoreObstacle footprint cells reach crate A*.
#[test]
fn ignore_obstacle_walks_through_owned_footprint() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(120.0, 80.0);
    for y in 0..8 {
        sys.grid.set_cell_obstacle_owned(
            GridPos::new(5, y),
            false,
            false,
            9,
            Some(0),
            Some(Team::USA),
        );
    }
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    let mut unit = Object::new(tmpl, ObjectId(1), Team::USA);
    let start = Vec3::new(20.0, 0.0, 35.0);
    let goal = Vec3::new(90.0, 0.0, 35.0);
    unit.set_position(start);
    unit.owner_player_id = Some(0);
    let mut objects = HashMap::new();
    objects.insert(unit.id, unit);

    let blocked = sys.find_path_ex_surfaces(
        start,
        goal,
        &objects,
        false,
        SURFACE_GROUND,
        false,
        Some(ObjectId(1)),
    );
    assert!(
        blocked.is_none()
            || blocked
                .as_ref()
                .is_some_and(|p| { !p.iter().any(|wp| sys.grid.world_to_grid(*wp).x == 5) }),
        "without ignore, factory footprint stays a wall"
    );

    sys.set_ignore_obstacle(Some(ObjectId(9)));
    let through = sys
        .find_path_ex_surfaces(
            start,
            goal,
            &objects,
            false,
            SURFACE_GROUND,
            false,
            Some(ObjectId(1)),
        )
        .expect("ignoreObstacle must walk the ignored footprint");
    assert!(
        through.iter().any(|wp| sys.grid.world_to_grid(*wp).x == 5),
        "path must step through ignored CELL_OBSTACLE, path={through:?}"
    );
    sys.set_ignore_obstacle(None);
}

/// hq-gsdys: seeker flags come from the mover, not nearest living object.
#[test]
fn find_path_ex_surfaces_uses_mover_not_nearest() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut dozer_t = ThingTemplate::new("AmericaDozer");
    dozer_t.add_kind_of(KindOf::Dozer);
    dozer_t.add_kind_of(KindOf::Vehicle);
    let mut dozer = Object::new(dozer_t, ObjectId(1), Team::USA);
    dozer.set_position(Vec3::new(20.0, 0.0, 20.0));
    dozer.owner_player_id = Some(0);
    objects.insert(dozer.id, dozer);
    let mut inf_t = ThingTemplate::new("Ranger");
    inf_t.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(inf_t, ObjectId(2), Team::USA);
    inf.set_position(Vec3::new(80.0, 0.0, 20.0));
    inf.owner_player_id = Some(0);
    objects.insert(inf.id, inf);

    let hop = Vec3::new(90.0, 0.0, 20.0);
    let goal = Vec3::new(150.0, 0.0, 20.0);
    let _ = sys.find_path_ex_surfaces(
        hop,
        goal,
        &objects,
        false,
        SURFACE_GROUND,
        false,
        Some(ObjectId(1)),
    );
    assert_eq!(sys.seeker_id, Some(ObjectId(1)));
    assert!(sys.seeker_is_dozer, "mover dozer must keep dozerHack");
    assert!(
        !sys.seeker_is_infantry,
        "nearest infantry must not steal seeker"
    );

    let _ = sys.find_path_ex_surfaces(hop, goal, &objects, false, SURFACE_GROUND, false, None);
    assert_eq!(sys.seeker_id, None, "no mover → no nearest-object steal");
    assert!(!sys.seeker_is_dozer);
    assert!(!sys.seeker_is_infantry);
}

/// hq-0h1vk / hq-hlj28: crate search accounts cells for the queue budget.
#[test]
fn find_path_via_crate_notes_cells_allocated() {
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let objects = HashMap::new();
    let _ = sys.find_path_ex(
        Vec3::new(10.0, 0.0, 10.0),
        Vec3::new(80.0, 0.0, 10.0),
        &objects,
        false,
        None,
    );
    assert!(
        sys.cumulative_cells_allocated() > 0,
        "crate A* must add m_cumulativeCellsAllocated"
    );
}

/// hq-xl5vj: BLAST_CRATER skips the airborne height gate and never unstamps.
#[test]
fn blast_crater_stamps_above_terrain_and_survives_remove() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut objects = HashMap::new();
    let mut crater_t = ThingTemplate::new("ScriptedBlastCrater");
    crater_t.add_kind_of(KindOf::Structure);
    crater_t.add_kind_of(KindOf::BlastCrater);
    let mut crater = Object::new(crater_t, ObjectId(7), Team::Neutral);
    crater.set_position(Vec3::new(80.0, 50.0, 80.0));
    crater.selection_radius = 20.0;
    objects.insert(crater.id, crater);
    sys.apply_structure_static_blocks(&objects);
    let cell = sys.grid.world_to_grid(Vec3::new(80.0, 50.0, 80.0));
    assert!(
        sys.grid.is_static_blocked(cell),
        "C++ height gate excepts KINDOF_BLAST_CRATER"
    );

    let mut floating_t = ThingTemplate::new("HoverPad");
    floating_t.add_kind_of(KindOf::Structure);
    let mut floating = Object::new(floating_t, ObjectId(8), Team::USA);
    floating.set_position(Vec3::new(40.0, 50.0, 40.0));
    floating.selection_radius = 20.0;
    objects.insert(floating.id, floating);
    sys.apply_structure_static_blocks(&objects);
    let air_cell = sys.grid.world_to_grid(Vec3::new(40.0, 50.0, 40.0));
    assert!(
        !sys.grid.is_static_blocked(air_cell),
        "non-crater airborne structure still skips classify"
    );

    objects.clear();
    sys.grid.clear_static_blocks();
    sys.apply_structure_static_blocks(&objects);
    assert!(
        sys.grid.is_static_blocked(cell),
        "C++ never removes BLAST_CRATER footprints"
    );
}

/// hq-zqfpa: getAircraftPath first node is dest altitude; detours keep offset Y.
#[test]
fn aircraft_path_first_node_uses_dest_altitude() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let objects = HashMap::new();
    let start = Vec3::new(10.0, 20.0, 10.0);
    let goal = Vec3::new(80.0, 50.0, 90.0);
    let path = sys
        .find_path_ex(start, goal, &objects, true, None)
        .expect("aircraft two-node path");
    assert_eq!(path.len(), 2);
    assert!(
        (path[0].x - start.x).abs() < 0.01 && (path[0].z - start.z).abs() < 0.01,
        "first node keeps unit XY"
    );
    assert!(
        (path[0].y - goal.y).abs() < 0.01,
        "C++ pos.z = to->z: first node is dest altitude, got {}",
        path[0].y
    );
    assert!((path[1].y - goal.y).abs() < 0.01);

    let mut objects = HashMap::new();
    let mut jet_t = ThingTemplate::new("Raptor");
    jet_t.add_kind_of(KindOf::Aircraft);
    let mut jet = Object::new(jet_t, ObjectId(1), Team::USA);
    jet.set_position(start);
    jet.loco_appearance = LocomotorAppearance::Wings;
    objects.insert(jet.id, jet);
    let mut bldg_t = ThingTemplate::new("CommandCenter");
    bldg_t.add_kind_of(KindOf::Structure);
    bldg_t.add_kind_of(KindOf::AircraftPathAround);
    let mut bldg = Object::new(bldg_t, ObjectId(2), Team::USA);
    bldg.set_position(Vec3::new(45.0, 0.0, 50.0));
    bldg.selection_radius = 30.0;
    objects.insert(bldg.id, bldg);
    let detoured = sys
        .find_path_ex(start, goal, &objects, true, Some(ObjectId(1)))
        .expect("wings path");
    assert!(
        (detoured[0].y - goal.y).abs() < 0.01,
        "first node stays dest altitude after detour, got {}",
        detoured[0].y
    );
    if detoured.len() > 2 {
        let mid_ys: Vec<f32> = detoured[1..detoured.len() - 1]
            .iter()
            .map(|p| p.y)
            .collect();
        assert!(
            mid_ys.iter().any(|y| (*y - start.y).abs() > 0.01),
            "detour nodes must not be flattened to start Y, mids={mid_ys:?}"
        );
    }
}

/// hq-wuufk: C++ adjustCoordToCell — infantry cell center, vehicle +0.05 inset.
#[test]
fn adjust_coord_to_cell_centers_infantry_insets_vehicles() {
    let g = open_grid(16, 16);
    let cell = GridPos::new(3, 4);
    let origin = g.grid_to_world(cell);
    let size = g.grid_size();
    let center = g.adjust_coord_to_cell(cell, true);
    let inset = g.adjust_coord_to_cell(cell, false);
    assert!(
        (center.x - (origin.x + size * 0.5)).abs() < 0.001
            && (center.z - (origin.z + size * 0.5)).abs() < 0.001,
        "infantry must plant on cell center, got {center:?} origin={origin:?}"
    );
    assert!(
        (inset.x - (origin.x + size * 0.05)).abs() < 0.001
            && (inset.z - (origin.z + size * 0.05)).abs() < 0.001,
        "vehicle must use C++ (cell+0.05)*size, got {inset:?} origin={origin:?}"
    );
    assert!(
        (inset.x - origin.x).abs() > 0.2,
        "vehicle inset must not be the cell-origin corner"
    );
}

/// hq-wuufk: even-diameter dest near a cell boundary seeds the next cell.
#[test]
fn vehicle_adjust_destination_seeds_half_cell() {
    let g = open_grid(20, 20);
    let (_, center) = PathfindingGrid::radius_and_center(8.0, 10.0);
    assert!(
        !center,
        "selection_radius 8 must be even-diameter / !center"
    );
    let dest = Vec3::new(19.9, 0.0, 5.0);
    assert_eq!(
        g.world_to_grid(dest),
        GridPos::new(1, 0),
        "truncate-toward-zero stays in cell 1"
    );
    assert_eq!(
        g.cell_for_unit_position(dest, false),
        GridPos::new(2, 0),
        "C++ half-cell seed must advance a vehicle near the far cell edge"
    );
    assert_eq!(
        g.cell_for_unit_position(dest, true),
        GridPos::new(1, 0),
        "infantry / centerInCell still truncates"
    );
}

/// hq-wuufk: live crate path emits adjustCoordToCell, not cell origins.
#[test]
fn crate_path_uses_adjust_coord_to_cell_not_corners() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let size = sys.grid.grid_size();
    let start = Vec3::new(20.0, 0.0, 20.0);
    let goal = Vec3::new(80.0, 0.0, 20.0);

    let mut inf_objects = HashMap::new();
    let mut inf_t = ThingTemplate::new("Ranger");
    inf_t.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(inf_t, ObjectId(1), Team::USA);
    inf.set_position(start);
    inf.selection_radius = 1.0;
    inf_objects.insert(inf.id, inf);
    let inf_path = sys
        .find_path_ex(start, goal, &inf_objects, false, Some(ObjectId(1)))
        .expect("infantry path");
    assert!(inf_path.len() >= 2);
    assert!(
        (inf_path[0].x - start.x).abs() < 0.01 && (inf_path[0].z - start.z).abs() < 0.01,
        "first node stays unit feet"
    );
    for p in &inf_path[1..] {
        let cell = sys.grid.world_to_grid(*p);
        let origin = sys.grid.grid_to_world(cell);
        assert!(
            (p.x - origin.x - size * 0.5).abs() < 0.25
                && (p.z - origin.z - size * 0.5).abs() < 0.25,
            "infantry waypoint {p:?} must be cell center, origin={origin:?}"
        );
    }

    let mut veh_objects = HashMap::new();
    let mut veh_t = ThingTemplate::new("CrusaderTank");
    veh_t.add_kind_of(KindOf::Vehicle);
    let mut veh = Object::new(veh_t, ObjectId(2), Team::USA);
    veh.set_position(start);
    veh.selection_radius = 8.0;
    veh_objects.insert(veh.id, veh);
    let veh_path = sys
        .find_path_ex(start, goal, &veh_objects, false, Some(ObjectId(2)))
        .expect("vehicle path");
    assert!(veh_path.len() >= 2);
    for p in &veh_path[1..] {
        let cell = sys.grid.world_to_grid(*p);
        let origin = sys.grid.grid_to_world(cell);
        assert!(
            (p.x - origin.x - size * 0.05).abs() < 0.25
                && (p.z - origin.z - size * 0.05).abs() < 0.25,
            "vehicle waypoint {p:?} must be C++ +0.05 inset, origin={origin:?}"
        );
        assert!(
            (p.x - origin.x).abs() > 0.2 || (p.z - origin.z).abs() > 0.2,
            "vehicle must not walk the cell-origin corner, p={p:?} origin={origin:?}"
        );
    }
}

/// hq-9hwau: patchPath skips occupied suffix nodes (parked allies).
#[test]
fn patch_path_skips_occupied_ally_waypoints() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let from = sys.grid.grid_to_world(GridPos::new(1, 4));
    let mid = sys.grid.grid_to_world(GridPos::new(8, 4));
    let dest = sys.grid.grid_to_world(GridPos::new(16, 4));
    let mut objects = HashMap::new();
    let mut seeker_t = ThingTemplate::new("Ranger");
    seeker_t.add_kind_of(KindOf::Infantry);
    let mut seeker = Object::new(seeker_t, ObjectId(1), Team::USA);
    seeker.set_position(from);
    seeker.owner_player_id = Some(0);
    objects.insert(seeker.id, seeker);
    let mut parked_t = ThingTemplate::new("Humvee");
    parked_t.add_kind_of(KindOf::Vehicle);
    let mut parked = Object::new(parked_t, ObjectId(9), Team::USA);
    parked.set_position(dest);
    parked.owner_player_id = Some(0);
    parked.crushable_level = 1;
    parked.selection_radius = 4.0;
    objects.insert(parked.id, parked);
    assert!(
        sys.patch_path(
            from,
            &[from, mid, dest],
            SURFACE_GROUND,
            false,
            &objects,
            Some(ObjectId(1)),
        )
        .is_none(),
        "C++ returns null when the last node itself is occupied"
    );

    objects.get_mut(&ObjectId(9)).unwrap().set_position(mid);
    let patched = sys
        .patch_path(
            from,
            &[from, mid, dest],
            SURFACE_GROUND,
            false,
            &objects,
            Some(ObjectId(1)),
        )
        .expect("must splice onto a still-clear dest suffix");
    let last_cell = sys.grid.world_to_grid(*patched.last().unwrap());
    let dest_cell = sys.grid.world_to_grid(dest);
    assert_eq!(last_cell, dest_cell, "suffix must still reach dest");
}

/// hq-vp531: findAttackPath binds leftover seeker from the mover.
#[test]
fn find_attack_path_binds_leftover_seeker() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    sys.set_human_player_mask(1u16 << 0);

    let mut objects = HashMap::new();
    let mut inf_t = ThingTemplate::new("Ranger");
    inf_t.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(inf_t, ObjectId(1), Team::USA);
    inf.set_position(Vec3::new(20.0, 0.0, 20.0));
    inf.owner_player_id = Some(0);
    inf.selection_radius = 1.0;
    inf.locomotor_surfaces = SURFACE_GROUND;
    objects.insert(inf.id, inf);

    // Stale infantry seeker from a prior pathfind.
    sys.bind_seeker_from_mover(&objects, Some(ObjectId(1)));
    sys.apply_seeker_human_flag();
    assert!(sys.seeker_is_infantry);
    assert!(sys.seeker_is_human);
    assert!(sys.seeker_center_in_cell);

    let mut tank_t = ThingTemplate::new("Crusader");
    tank_t.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(tank_t, ObjectId(2), Team::GLA);
    tank.set_position(Vec3::new(20.0, 0.0, 50.0));
    tank.owner_player_id = Some(2);
    tank.selection_radius = 8.0;
    tank.locomotor_surfaces = SURFACE_GROUND | SURFACE_WATER;
    objects.insert(tank.id, tank);

    let path = sys.find_attack_firing_position(
        Vec3::new(20.0, 0.0, 50.0),
        Vec3::new(150.0, 0.0, 50.0),
        80.0,
        &objects,
        false,
        Some(ObjectId(2)),
    );
    assert_eq!(
        sys.seeker_id,
        Some(ObjectId(2)),
        "must bind the attacking tank"
    );
    assert!(
        !sys.seeker_is_infantry,
        "tank must not keep infantry occupancy"
    );
    assert_eq!(sys.seeker_player, Some(2));
    assert!(
        !sys.seeker_is_human,
        "AI tank (player 2) must not clamp like a human"
    );
    assert!(
        !sys.seeker_center_in_cell,
        "even-diameter vehicle must use leftover +0.05 inset"
    );
    if let Some(p) = path {
        let goal = *p.last().unwrap();
        let cell = sys.grid.world_to_grid(goal);
        let expected = sys.grid.adjust_coord_to_cell(cell, false);
        assert!(
            (goal.x - expected.x).abs() < 0.25 && (goal.z - expected.z).abs() < 0.25,
            "bound vehicle goal must be leftover adjustCoordToCell, goal={goal:?} expected={expected:?}"
        );
    }
}

/// hq-wwbt9: findAttackPath range-tests leftover adjustCoordToCell, not cell corners.
#[test]
fn find_attack_path_range_tests_adjust_coord_to_cell() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let size = sys.grid.grid_size();

    let mut objects = HashMap::new();
    let mut inf_t = ThingTemplate::new("Ranger");
    inf_t.add_kind_of(KindOf::Infantry);
    let mut inf = Object::new(inf_t, ObjectId(1), Team::USA);
    inf.set_position(Vec3::new(20.0, 0.0, 50.0));
    inf.owner_player_id = Some(0);
    inf.selection_radius = 1.0;
    inf.locomotor_surfaces = SURFACE_GROUND;
    objects.insert(inf.id, inf);

    let path = sys
        .find_attack_firing_position(
            Vec3::new(20.0, 0.0, 50.0),
            Vec3::new(150.0, 0.0, 50.0),
            80.0,
            &objects,
            false,
            Some(ObjectId(1)),
        )
        .expect("infantry attack path");
    let goal = *path.last().unwrap();
    let cell = sys.grid.world_to_grid(goal);
    let origin = sys.grid.grid_to_world(cell);
    let expected = sys.grid.adjust_coord_to_cell(cell, true);
    assert!(
        (goal.x - expected.x).abs() < 0.25 && (goal.z - expected.z).abs() < 0.25,
        "infantry firing cell must be leftover cell center, goal={goal:?} expected={expected:?}"
    );
    assert!(
        (goal.x - origin.x - size * 0.5).abs() < 0.25
            && (goal.z - origin.z - size * 0.5).abs() < 0.25,
        "infantry must not range-test the cell-origin corner, goal={goal:?} origin={origin:?}"
    );

    let plant =
        sys.snap_closest_goal_position(Vec3::new(35.0, 0.0, 45.0), SURFACE_GROUND, false, 8.0);
    let plant_cell = sys.grid.world_to_grid(plant);
    let plant_origin = sys.grid.grid_to_world(plant_cell);
    assert!(
        (plant.x - plant_origin.x - size * 0.05).abs() < 0.25
            && (plant.z - plant_origin.z - size * 0.05).abs() < 0.25,
        "vehicle snapClosestGoalPosition must use leftover +0.05, plant={plant:?} origin={plant_origin:?}"
    );
    assert!(
        (plant.x - plant_origin.x).abs() > 0.2 || (plant.z - plant_origin.z).abs() > 0.2,
        "vehicle snap must not stay on the cell-origin corner"
    );
}

/// hq-ykcav: crusher combiners merge fence↔clear so crushers cross fence-divided pockets.
#[test]
fn crusher_zones_join_fence_divided_banks() {
    let mut g = open_grid(8, 8);
    for y in 0..8 {
        g.set_cell_obstacle(GridPos::new(4, y), true, false);
    }
    g.rebuild_path_zones();
    let from = g.grid_to_world(GridPos::new(1, 4));
    let to = g.grid_to_world(GridPos::new(6, 4));
    assert!(
        !g.quick_path_exists_for(from, to, SURFACE_GROUND),
        "non-crusher must see two zones across a fence line"
    );
    assert!(
        g.quick_path_exists_for_crusher(from, to, SURFACE_GROUND, true),
        "crusher combiners must join fence↔clear banks"
    );
    assert_ne!(
        g.path_zone(GridPos::new(1, 4)),
        g.path_zone(GridPos::new(6, 4)),
        "raw path_zones stay split; only crusher effective zone merges"
    );
}

/// hq-dypfr: adjustToPossibleDestination walks off an impassable seed.
#[test]
fn adjust_to_possible_destination_spirals_off_obstacle() {
    let mut g = open_grid(16, 16);
    g.set_cell_type(GridPos::new(8, 8), PathfindCellType::Obstacle);
    g.rebuild_path_zones();
    let start = g.grid_to_world(GridPos::new(2, 8));
    let mut dest = g.grid_to_world(GridPos::new(8, 8));
    assert!(
        g.adjust_to_possible_destination(start, &mut dest, SURFACE_GROUND, false, 0.0),
        "must find a same-zone possible cell near the blocked seed"
    );
    let snapped = g.world_to_grid(dest);
    assert_ne!(snapped, GridPos::new(8, 8), "must leave the obstacle cell");
    assert_eq!(
        g.cell_type(snapped),
        PathfindCellType::Clear,
        "snapped dest must be Clear"
    );
}

/// hq-ykcav: crusher findPath crosses a fence line the zone pre-gate used to refuse.
#[test]
fn find_path_crusher_crosses_fence_line() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(160.0, 160.0);
    for y in 0..16 {
        sys.grid.set_cell_obstacle(GridPos::new(8, y), true, false);
    }
    sys.grid.rebuild_path_zones();
    let mut objects = HashMap::new();
    let mut t = ThingTemplate::new("Overlord");
    t.add_kind_of(KindOf::Vehicle);
    let mut tank = Object::new(t, ObjectId(1), Team::China);
    tank.set_position(Vec3::new(20.0, 0.0, 80.0));
    tank.crusher_level = 3;
    tank.selection_radius = 8.0;
    tank.locomotor_surfaces = SURFACE_GROUND;
    objects.insert(tank.id, tank);
    let path = sys.find_path_ex_surfaces(
        Vec3::new(20.0, 0.0, 80.0),
        Vec3::new(140.0, 0.0, 80.0),
        &objects,
        false,
        SURFACE_GROUND,
        true,
        Some(ObjectId(1)),
    );
    assert!(
        path.is_some(),
        "crusher must path across a fence-divided pocket"
    );
}

/// hq-w1rig: createAWall rasters mobile vehicles that classify_object_footprint skips.
#[test]
fn create_wall_from_object_stamps_mobile_vehicle() {
    use crate::game_logic::{KindOf, Object, ObjectId, Team, ThingTemplate};
    let mut sys = PathfindingSystem::new(200.0, 200.0);
    let mut tmpl = ThingTemplate::new("CivilianTrainEngine");
    tmpl.add_kind_of(KindOf::Vehicle);
    let mut train = Object::new(tmpl, ObjectId(7), Team::USA);
    train.set_position(Vec3::new(80.0, 0.0, 80.0));
    train.selection_radius = 15.0;
    assert!(
        sys.grid.classify_object_footprint(&train, false).is_none(),
        "regular classify must still skip mobile vehicles"
    );
    sys.create_wall_from_object(&train);
    let cell = sys.grid.world_to_grid(train.get_position());
    assert!(
        sys.grid.is_static_blocked(cell),
        "createAWall must stamp a locomotive footprint"
    );
    sys.remove_wall_from_object(&train);
    assert!(
        !sys.grid.is_static_blocked(cell),
        "removeWall must undo the locomotive stamp"
    );
}

/// hq-00tq6 / hq-m8gg7: leftover computeQuickPath two-node leftover-installed.
#[test]
fn leftover_compute_quick_path_nodes_are_start_and_dest() {
    let start = Vec3::new(10.0, 3.0, 20.0);
    let dest = Vec3::new(50.0, 8.0, 40.0);
    let path = PathfindingSystem::leftover_compute_quick_path_nodes(start, dest);
    assert_eq!(path.len(), 2);
    assert_eq!(path[0], Vec3::new(10.0, 8.0, 20.0));
    assert_eq!(path[1], dest);
}

#[test]
fn leftover_off_map_start_gate_matches_leftover_ai_path() {
    use gamelogic::common::Coord3D;
    use gamelogic::object::unit::leftover_should_force_direct_path_for_off_map_start;
    if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
        terrain.reset();
    }
    assert!(leftover_should_force_direct_path_for_off_map_start(
        &Coord3D::new(-100.0, -100.0, 5.0),
        &Coord3D::new(-50.0, -25.0, 9.0),
    ));
    assert!(!leftover_should_force_direct_path_for_off_map_start(
        &Coord3D::new(0.0, 0.0, 5.0),
        &Coord3D::new(-50.0, -25.0, 9.0),
    ));
    let sys = PathfindingSystem::new(200.0, 200.0);
    assert!(
        sys.leftover_should_force_direct_path_for_off_map_start(
            Vec3::new(-100.0, 5.0, -100.0),
            Vec3::new(-50.0, 9.0, -25.0),
        ),
        "off-map start+dest leftover-installs computeQuickPath on live world bounds"
    );
    assert!(
        !sys.leftover_should_force_direct_path_for_off_map_start(
            Vec3::new(10.0, 5.0, 10.0),
            Vec3::new(-50.0, 9.0, -25.0),
        ),
        "on-map start does not leftover-install off-map computeQuickPath"
    );
}

#[test]
fn leftover_line_passable_non_final_gate_skips_final_goal() {
    let sys = PathfindingSystem::new(200.0, 200.0);
    let start = Vec3::new(10.0, 0.0, 10.0);
    let dest = Vec3::new(80.0, 0.0, 10.0);
    assert!(
        !sys.leftover_should_use_direct_path_for_line_passable_non_final_goal(
            true,
            start,
            dest,
            SURFACE_GROUND,
            None,
        )
    );
    assert!(
        sys.leftover_should_use_direct_path_for_line_passable_non_final_goal(
            false,
            start,
            dest,
            SURFACE_GROUND,
            None,
        )
    );
    // Live host world is often origin-centered; leftover THE_AI grid is 0..N.
    let centered = PathfindingSystem::new_with_origin(Vec3::new(-256.0, 0.0, -256.0), 512.0, 512.0);
    assert!(
        centered.leftover_should_use_direct_path_for_line_passable_non_final_goal(
            false,
            Vec3::new(-100.0, 0.0, -100.0),
            Vec3::new(-50.0, 0.0, -50.0),
            SURFACE_GROUND,
            None,
        ),
        "non-final line-passable must leftover-install onto live cells, not leftover THE_AI"
    );
    let mut walled = PathfindingSystem::new(200.0, 200.0);
    for y in 0..20 {
        walled
            .grid
            .set_cell_type(GridPos::new(10, y), PathfindCellType::Impassable);
    }
    assert!(
        !walled.leftover_should_use_direct_path_for_line_passable_non_final_goal(
            false,
            Vec3::new(10.0, 0.0, 10.0),
            Vec3::new(180.0, 0.0, 10.0),
            SURFACE_GROUND,
            None,
        ),
        "blocked leftover isLinePassable must not leftover-install computeQuickPath"
    );
}

#[test]
fn compute_assigned_unit_path_leftover_installs_compute_quick_path() {
    let src = concat!(
        include_str!("../../world_save.rs"),
        include_str!("../../world_save/world_subsystems.rs"),
        include_str!("../../world_save/world_paths.rs"),
        include_str!("../../world_save/world_runtime.rs"),
        include_str!("../../world_save/world_players.rs"),
        include_str!("../../world_save/world_load.rs"),
    );
    let i = src
        .find("fn compute_assigned_unit_path")
        .expect("compute_assigned_unit_path");
    let w = &src[i..src.len().min(i + 3500)];
    assert!(
        w.contains("leftover_should_force_direct_path_for_off_map_start")
            && w.contains("leftover_should_use_direct_path_for_line_passable_non_final_goal")
            && w.contains("leftover_compute_quick_path_nodes"),
        "live compute_assigned_unit_path must leftover-install computeQuickPath"
    );
    assert!(
        w.contains("is_safe_path") && w.contains("ChaseTarget"),
        "non-final hops must include requestSafePath and attack-pursue"
    );
    let pf = include_str!("../system_routes.rs");
    assert!(
        pf.contains("leftover_should_force_direct_path_for_off_map_start(start, goal)")
            && pf.contains("leftover_compute_quick_path_nodes(start, goal)"),
        "find_path_ex_surfaces must leftover-install off-map computeQuickPath"
    );
}

#[test]
fn assign_unit_path_non_final_line_passable_is_two_node() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    logic.templates.insert("Ranger".into(), tmpl);
    let start = Vec3::new(10.0, 0.0, 10.0);
    let dest = Vec3::new(80.0, 0.0, 10.0);
    let id = logic
        .create_object("Ranger", Team::USA, start)
        .expect("ranger");
    if let Some(u) = logic.host_object_mut(id) {
        u.movement.max_speed = 20.0;
        u.is_safe_path = true;
        u.locomotor_surfaces = SURFACE_GROUND;
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(logic.assign_unit_path_for_test(id, dest, &[]));
    let unit = logic.host_object(id).expect("unit");
    assert_eq!(
        unit.movement.path.len(),
        2,
        "leftover computeQuickPath is two-node start+dest"
    );
    assert!((unit.movement.path[0].x - start.x).abs() < 0.01);
    assert!((unit.movement.path[1].x - dest.x).abs() < 0.01);
}

#[test]
fn assign_unit_path_off_map_start_is_two_node() {
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
        terrain.reset();
    }
    let mut logic = GameLogic::new();
    let mut tmpl = ThingTemplate::new("Ranger");
    tmpl.add_kind_of(KindOf::Infantry);
    logic.templates.insert("Ranger".into(), tmpl);
    // GameLogic::new world is -256..256. Start and dest are off that region.
    let start = Vec3::new(-300.0, 0.0, -300.0);
    let dest = Vec3::new(-280.0, 0.0, -280.0);
    let id = logic
        .create_object("Ranger", Team::USA, start)
        .expect("ranger");
    if let Some(u) = logic.host_object_mut(id) {
        u.movement.max_speed = 20.0;
        u.locomotor_surfaces = SURFACE_GROUND;
    }
    logic.force_map_loaded_for_path_test(true);
    assert!(
        logic.assign_unit_path_for_test(id, dest, &[]),
        "off-map start+dest must leftover-install computeQuickPath, not fail A*"
    );
    let unit = logic.host_object(id).expect("unit");
    assert_eq!(
        unit.movement.path.len(),
        2,
        "off-map computeQuickPath is two-node start+dest"
    );
    assert!((unit.movement.path[0].x - start.x).abs() < 0.01);
    assert!((unit.movement.path[1].x - dest.x).abs() < 0.01);
}
