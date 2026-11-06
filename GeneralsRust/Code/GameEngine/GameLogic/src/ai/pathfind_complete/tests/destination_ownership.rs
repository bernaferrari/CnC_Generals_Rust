use super::*;

fn register_test_object(
    id: ObjectID,
    kinds: &[KindOf],
    team: Option<std::sync::Arc<std::sync::RwLock<crate::team::Team>>>,
) -> std::sync::Arc<std::sync::RwLock<crate::object::Object>> {
    let mut template = crate::common::types::DefaultThingTemplate::new(format!("PfTest{id}"));
    for kind in kinds {
        template.add_kind_of(*kind);
    }
    let mut obj = crate::object::Object::new_test(id, 100.0);
    obj.set_template_for_test(std::sync::Arc::new(template));
    if let Some(team) = team {
        obj.set_team(Some(team)).expect("set_team");
    }
    let arc = std::sync::Arc::new(std::sync::RwLock::new(obj));
    OBJECT_REGISTRY.register_object(id, &arc);
    arc
}

#[test]
fn possible_destination_uses_each_borrowed_receiver_on_every_footprint_check() {
    // AIPathfind.cpp:4900-4956 and 5562-5610: each check observes obj's AI.
    let factory = crate::object::object_factory::ObjectFactory::new();
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let observations = PathfindObservations { factory: &factory, players: &players, terrain: &terrain };
    let object = crate::object::Object::new_test(100, 100.0);
    let mut world = PathfindingSystem::new(16, 16);
    world.new_map();
    let goal = GridCoord::new(5, 5).to_world(0.0);
    let start = GridCoord::new(2, 2).to_world(0.0);
    let cell = GridCoord::new(5, 5);
    world.pathfinder.set_cell_type(cell, PathfindCellType::Obstacle);
    world.pathfinder.set_cell_obstacle_id(cell, 99, false, false);
    world.update_aircraft_goal(&goal, 100, 0, true);
    let request = PathRequest {
        object_id: 100, from: start, to: goal, surfaces: SURFACE_GROUND,
        is_crusher: false, unit_radius: 0.0, allow_partial: false,
        move_allies: false, ignore_obstacle_id: Some(99), is_human: false,
    };
    let ignored = AdjustmentOwner { source: &object, request: request.clone(), aircraft_goal_only: false };
    let aircraft = AdjustmentOwner { source: &object, request: request.clone(), aircraft_goal_only: true };
    let ground = AdjustmentOwner {
        source: &object, request: PathRequest { ignore_obstacle_id: None, ..request },
        aircraft_goal_only: false,
    };
    for (owner, keep) in [(&ignored, true), (&aircraft, true), (&ground, false), (&aircraft, true)] {
        let mut destination = goal;
        assert!(world.adjust_to_possible_destination_with_observations(
            &start, &mut destination, SURFACE_GROUND, false, 0.0,
            Some(&observations), Some(owner),
        ));
        assert_eq!(destination == goal, keep);
    }
}

#[test]
fn dozer_hack_steps_non_enemy_obstacle_not_enemy() {
    // C++ AIPathfind.cpp:6207-6226 examineNeighboringCells dozerHack:
    // KINDOF_DOZER + CELL_OBSTACLE + obstacle exists && !ENEMIES.
    let _lock = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    const DOZER_ID: ObjectID = 0x00D0_7E01;
    const ALLY_OBS_ID: ObjectID = 0x00D0_7E02;
    const ENEMY_OBS_ID: ObjectID = 0x00D0_7E03;
    const MISSING_OBS_ID: ObjectID = 0x00D0_7E04;
    const DOZER_TEAM: u32 = 0x00D0_7E11;
    const ENEMY_TEAM: u32 = 0x00D0_7E12;

    let dozer_team = std::sync::Arc::new(std::sync::RwLock::new(crate::team::Team::new(
        "DozerHackTeam".into(),
        DOZER_TEAM,
    )));
    let enemy_team = std::sync::Arc::new(std::sync::RwLock::new(crate::team::Team::new(
        "DozerHackEnemy".into(),
        ENEMY_TEAM,
    )));
    dozer_team
        .write()
        .unwrap()
        .set_override_team_relationship(ENEMY_TEAM, Relationship::Enemies);

    let _dozer = register_test_object(DOZER_ID, &[KindOf::Dozer], Some(dozer_team.clone()));
    let _ally_obs = register_test_object(ALLY_OBS_ID, &[], None);
    let _enemy_obs = register_test_object(ENEMY_OBS_ID, &[], Some(enemy_team.clone()));
    struct Unreg(ObjectID);
    impl Drop for Unreg {
        fn drop(&mut self) {
            OBJECT_REGISTRY.unregister_object(self.0);
        }
    }
    let _u0 = Unreg(DOZER_ID);
    let _u1 = Unreg(ALLY_OBS_ID);
    let _u2 = Unreg(ENEMY_OBS_ID);
    // Keep dozer on its team for the enemy case; ally/neutral obstacle has no team
    // → relationship Neutral (not ENEMIES) → dozerHack allowed.

    // Full-height wall so line-seed cannot skip the obstacle (2x2 diagonal
    // seed would otherwise enqueue the Clear goal). One Obstacle gap for dozerHack.
    let mut system = PathfindingSystem::new(8, 4);
    system.new_map();
    let start = GridCoord::new(0, 1);
    let goal = GridCoord::new(4, 1);
    let obs = GridCoord::new(2, 1);
    {
        let pf = &mut system.pathfinder;
        for x in 0..8 {
            for y in 0..4 {
                pf.set_cell_type(GridCoord::new(x, y), PathfindCellType::Clear);
            }
        }
        for y in 0..4 {
            pf.set_cell_type(GridCoord::new(2, y), PathfindCellType::Impassable);
        }
        pf.set_cell_type(obs, PathfindCellType::Obstacle);
        pf.set_cell_obstacle_id(obs, ALLY_OBS_ID, false, false);
    }

    let from = system.world_pos_for_coord(start, PathfindLayerEnum::Ground);
    let to = system.world_pos_for_coord(goal, PathfindLayerEnum::Ground);
    let mk = |object_id: ObjectID| PathRequest {
        object_id,
        from,
        to,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };

    let dozer_path = system.find_path(mk(DOZER_ID));
    assert!(
        dozer_path.success,
        "dozer must step on non-enemy CELL_OBSTACLE: {:?}",
        dozer_path.waypoints
    );

    let infantry = system.find_path(mk(INVALID_ID));
    assert!(!infantry.success, "non-dozer cannot step on CELL_OBSTACLE");

    {
        let pf = &mut system.pathfinder;
        pf.set_cell_obstacle_id(obs, ENEMY_OBS_ID, false, false);
    }
    let enemy_path = system.find_path(mk(DOZER_ID));
    assert!(
        !enemy_path.success,
        "dozer must not dozerHack an ENEMIES obstacle"
    );

    {
        let pf = &mut system.pathfinder;
        pf.set_cell_obstacle_id(obs, MISSING_OBS_ID, false, false);
    }
    let missing = system.find_path(mk(DOZER_ID));
    assert!(
        !missing.success,
        "missing obstacle object must fail-closed (not dozerHack)"
    );

    OBJECT_REGISTRY.unregister_object(DOZER_ID);
    OBJECT_REGISTRY.unregister_object(ALLY_OBS_ID);
    OBJECT_REGISTRY.unregister_object(ENEMY_OBS_ID);
}

/// C++ processPathfindQueue calls doPathfind once. It must not also A* a
/// PathRequest snapshotted when the object was queued.
#[test]
fn process_queue_skips_snapshot_after_do_pathfind() {
    let _lock = crate::object::registry::test_isolation_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    const ID: ObjectID = 0x00F1_A701;
    const LIVE_SENTINEL_ID: ObjectID = 0x00F1_A702;
    const REPLACED_ID: ObjectID = 0x00F1_A703;
    struct Unreg(ObjectID);
    impl Drop for Unreg {
        fn drop(&mut self) {
            OBJECT_REGISTRY.unregister_object(self.0);
        }
    }
    let _unreg = Unreg(ID);
    // Keep the registry non-empty while ID is removed. The Object destructor
    // calls pathfinder cleanup, which resolves through ObjectRegistry again;
    // with no sentinel the empty-store fast path could hide lock re-entry.
    let sentinel = register_test_object(LIVE_SENTINEL_ID, &[], None);

    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    #[derive(Debug)]
    struct CountingAi {
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    }
    impl crate::modules::AIUpdateInterface for CountingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn is_moving(&self) -> bool {
            false
        }
        fn is_idle(&self) -> bool {
            true
        }
        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }
        fn do_pathfind(&mut self) {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    let obj = register_test_object(ID, &[], None);
    obj.write()
        .unwrap()
        .set_ai_update_interface(Some(std::sync::Arc::new(std::sync::Mutex::new(
            CountingAi {
                calls: calls.clone(),
            },
        ))));

    let mut system = PathfindingSystem::new(40, 40);
    system.new_map();
    system
        .queue_path_request(PathRequest {
            object_id: ID,
            from: Coord3D::new(20.0, 20.0, 0.0),
            to: Coord3D::new(200.0, 200.0, 0.0),
            surfaces: SURFACE_GROUND,
            is_crusher: false,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id: Some(99),
            is_human: false,
        })
        .unwrap();

    let n = system.process_queue(PATHFIND_CELLS_PER_FRAME);
    assert_eq!(n, 1, "one pathfind per queued object");
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "do_pathfind once"
    );
    assert_eq!(
        system.cumulative_cells_allocated(),
        0,
        "snapshotted PathRequest must not run a second search"
    );
    assert!(system.request_queue.is_empty());
    assert!(system.object_path_queue.is_empty());

    // Exercise the last-handle path at a controlled point: unregister drops
    // the registry's Arc and runs Object::drop while another registered object
    // forces the destructor's pathfinder lookup through the registry lock.
    drop(obj);
    OBJECT_REGISTRY.unregister_object(ID);
    assert!(!OBJECT_REGISTRY.store_is_empty());

    // Replacement has the same last-Arc hazard: insertion has already made
    // the new value visible when destruction of the old value calls back in.
    let replaced = register_test_object(REPLACED_ID, &[], None);
    drop(replaced);
    let replacement = register_test_object(REPLACED_ID, &[], None);
    drop(replacement);

    // Clear must also move its old map out from under the write lock before
    // any removed object's destructor runs.
    OBJECT_REGISTRY.clear();
    drop(sentinel);
    assert!(OBJECT_REGISTRY.store_is_empty());
}

#[test]
fn cell_center_height_samples_the_requested_bridge_slot() {
    // TerrainLogic.cpp getLayerHeight selects the requested PathfindLayer, not
    // whichever bridge happens to occupy the first elevated/Top slot.
    let cell = GridCoord::new(9, 9);
    for (top_height, other_height) in [(11.0, 23.0), (101.0, 109.0)] {
        let sample = |x, y, layer| {
            assert_eq!((x, y), (95.0, 95.0));
            match layer {
                CommonPathfindLayerEnum::Top => top_height,
                CommonPathfindLayerEnum::Bridge1 => other_height,
                _ => 0.0,
            }
        };
        assert_eq!(
            PathfindingSystem::cell_center_with_height(cell, PathfindLayerEnum::Top, sample).z,
            top_height
        );
        assert_eq!(
            PathfindingSystem::cell_center_with_height(cell, PathfindLayerEnum::Layer3, sample).z,
            other_height
        );
        assert_eq!(
            PathfindingSystem::cell_center_with_height(cell, PathfindLayerEnum::Ground, sample).z,
            0.0
        );
    }
}

#[test]
fn cpp_destination_uses_cell_identity_not_surface_or_registry() {
    let factory = crate::object::object_factory::ObjectFactory::new();
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let observations = PathfindObservations {
        factory: &factory,
        players: &players,
        terrain: &terrain,
    };
    let mut first = PathfindingSystem::new(8, 8);
    let mut second = PathfindingSystem::new(8, 8);
    first.new_map();
    second.new_map();
    let cell = GridCoord::new(4, 4);
    let position = cell.to_world(0.0);
    let request = PathRequest {
        object_id: 100,
        from: position,
        to: position,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: Some(41),
        is_human: false,
    };
    first.set_cell_type(&position, PathfindCellType::Water);
    assert!(
        first.check_destination_with_observations(
            &request,
            cell,
            PathfindLayerEnum::Ground,
            0,
            true,
            Some(false),
            Some(&observations)
        ),
        "C++ checkDestination assumes locomotor validation and accepts water"
    );
    first
        .pathfinder
        .set_cell_obstacle_id(cell, 41, false, false);
    second
        .pathfinder
        .set_cell_obstacle_id(cell, 42, false, false);
    assert!(
        first.check_destination_with_observations(
            &request,
            cell,
            PathfindLayerEnum::Ground,
            0,
            true,
            Some(false),
            Some(&observations)
        ),
        "the receiving grid owns the ignored obstacle"
    );
    assert!(
        !second.check_destination_with_observations(
            &request,
            cell,
            PathfindLayerEnum::Ground,
            0,
            true,
            Some(false),
            Some(&observations)
        ),
        "a different obstacle remains blocked"
    );
}

#[test]
fn aircraft_destination_respects_resolved_cell_layer() {
    let factory = crate::object::object_factory::ObjectFactory::new();
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let observations = PathfindObservations {
        factory: &factory,
        players: &players,
        terrain: &terrain,
    };
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    let cell = GridCoord::new(4, 4);
    let pos = cell.to_world(0.0);
    world.update_aircraft_goal(&pos, 99, 0, true);
    let request = PathRequest {
        object_id: 100,
        from: pos,
        to: pos,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let accepts = |world: &PathfindingSystem, layer| {
        world.check_destination_with_observations(
            &request,
            cell,
            layer,
            0,
            true,
            Some(true),
            Some(&observations),
        )
    };
    assert!(!accepts(&world, PathfindLayerEnum::Ground));
    assert!(
        !accepts(&world, PathfindLayerEnum::Top),
        "missing bridge cell resolves to reserved ground"
    );
    world
        .pathfinder
        .set_cell_type_on_layer(cell, PathfindLayerEnum::Top, PathfindCellType::Clear);
    assert!(
        accepts(&world, PathfindLayerEnum::Top),
        "a ground reservation does not reserve the bridge cell"
    );
    assert!(!accepts(&world, PathfindLayerEnum::Ground));
}

#[test]
fn destination_respects_allied_and_fixed_enemy_goal_units() {
    const SOURCE: ObjectID = 710101;
    const ALLY: ObjectID = 710102;
    const ENEMY: ObjectID = 710103;
    let team = std::sync::Arc::new(std::sync::RwLock::new(crate::team::Team::new(
        "DestinationSource".into(),
        7101,
    )));
    let enemy_team = std::sync::Arc::new(std::sync::RwLock::new(crate::team::Team::new(
        "DestinationEnemy".into(),
        7102,
    )));
    team.write()
        .unwrap()
        .set_override_team_relationship(7101, Relationship::Allies);
    team.write()
        .unwrap()
        .set_override_team_relationship(7102, Relationship::Enemies);
    let _source = register_test_object(SOURCE, &[], Some(team.clone()));
    let _ally = register_test_object(ALLY, &[], Some(team));
    let _enemy = register_test_object(ENEMY, &[], Some(enemy_team));
    struct Unregister;
    impl Drop for Unregister {
        fn drop(&mut self) {
            for id in [SOURCE, ALLY, ENEMY] {
                OBJECT_REGISTRY.unregister_object(id);
            }
        }
    }
    let _cleanup = Unregister;
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    let cell = GridCoord::new(4, 4);
    let position = cell.to_world(0.0);
    let request = PathRequest {
        object_id: SOURCE,
        from: position,
        to: position,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: true,
        ignore_obstacle_id: None,
        is_human: false,
    };
    world.set_goal_cells(
        ALLY,
        ICoord2D::new(cell.x, cell.y),
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    assert!(
        !world.check_destination(
            &request,
            cell,
            PathfindLayerEnum::Ground,
            0,
            true,
            Some(false)
        ),
        "C++ never usurps an ally goal"
    );
    world.set_goal_cells(
        ENEMY,
        ICoord2D::new(cell.x, cell.y),
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    world.set_pos_cells(
        ENEMY,
        ICoord2D::new(cell.x, cell.y),
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    assert!(
        !world.check_destination(
            &request,
            cell,
            PathfindLayerEnum::Ground,
            0,
            true,
            Some(false)
        ),
        "a fixed enemy that cannot be crushed blocks the destination"
    );
    assert!(
        !world.check_destination(&request, cell, PathfindLayerEnum::Top, 0, true, Some(false)),
        "a bridge fallback must observe the fixed ground unit"
    );
    world.clear_pos_cells(
        ENEMY,
        ICoord2D::new(cell.x, cell.y),
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    assert!(
        world.check_destination(
            &request,
            cell,
            PathfindLayerEnum::Ground,
            0,
            true,
            Some(false)
        ),
        "an enemy goal without a fixed unit is allowed"
    );
}

#[test]
fn destination_occupancy_uses_the_receiving_factory() {
    use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};
    use std::sync::{Arc, RwLock};
    fn instance(relation: Relationship) -> (ObjectFactory, ObjectID, ObjectID) {
        let mut factory = ObjectFactory::new();
        let mut logic = crate::system::game_logic::GameLogic::new();
        let source_team = Arc::new(RwLock::new(crate::team::Team::new("Source".into(), 801)));
        let target_team = Arc::new(RwLock::new(crate::team::Team::new("Target".into(), 802)));
        source_team
            .write()
            .unwrap()
            .set_override_team_relationship(802, relation);
        let mut create = |name: &str, team| {
            factory
                .prepare_object(
                    &mut logic,
                    Arc::new(crate::common::types::DefaultThingTemplate::new(name.into())),
                    Coord3D::ZERO,
                    Some(team),
                    ObjectCreationFlags::NO_AI,
                )
                .unwrap()
        };
        let source = create("DestinationSource", source_team);
        let target = create("DestinationTarget", target_team);
        (factory, source, target)
    }
    let (ally_factory, source, goal) = instance(Relationship::Allies);
    let (enemy_factory, peer_source, peer_goal) = instance(Relationship::Enemies);
    assert_eq!((source, goal), (peer_source, peer_goal));
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let cell = GridCoord::new(4, 4);
    let pos = cell.to_world(0.0);
    let request = PathRequest {
        object_id: source,
        from: pos,
        to: pos,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: true,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    world.set_goal_cells(
        goal,
        ICoord2D::new(cell.x, cell.y),
        0,
        true,
        PathfindLayerEnum::Ground,
        true,
        false,
    );
    let ally = PathfindObservations {
        factory: &ally_factory,
        players: &players,
        terrain: &terrain,
    };
    let enemy = PathfindObservations {
        factory: &enemy_factory,
        players: &players,
        terrain: &terrain,
    };
    let check = |owner| {
        world.check_destination_with_observations(
            &request,
            cell,
            PathfindLayerEnum::Ground,
            0,
            true,
            Some(false),
            Some(owner),
        )
    };
    assert!(!check(&ally));
    assert!(check(&enemy));
    let mut ally_pos = pos;
    world.snap_closest_goal_position_with_observations(
        SURFACE_GROUND,
        false,
        &mut ally_pos,
        0.0,
        source,
        Some(&ally),
    );
    assert_eq!(
        GridCoord::from_world(&ally_pos),
        GridCoord::new(cell.x - 1, cell.y - 1),
        "an allied reservation must trigger the original neighborhood search"
    );
    let source_handle = ally_factory
        .get_object(source)
        .unwrap()
        .get_base_object()
        .unwrap();
    let source_borrow = source_handle.write().unwrap();
    let mut borrowed_pos = pos;
    world.snap_closest_goal_position_with_owner(
        SURFACE_GROUND,
        false,
        &mut borrowed_pos,
        0.0,
        source,
        None,
        Some(false),
        Some(&source_borrow),
        Some(&ally),
    );
    assert_eq!(GridCoord::from_world(&borrowed_pos), GridCoord::new(3, 3));
    world
        .pathfinder
        .set_cell_obstacle_id(cell, 777, false, false);
    let mut ignored_pos = pos;
    world.snap_closest_goal_position_with_owner(
        SURFACE_GROUND,
        false,
        &mut ignored_pos,
        0.0,
        source,
        Some(777),
        Some(false),
        Some(&source_borrow),
        Some(&ally),
    );
    assert_eq!(GridCoord::from_world(&ignored_pos), cell);
}

#[test]
fn destination_goal_occupancy_follows_resolved_bridge_cell() {
    let factory = crate::object::object_factory::ObjectFactory::new();
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let observations = PathfindObservations {
        factory: &factory,
        players: &players,
        terrain: &terrain,
    };
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    let cell = GridCoord::new(4, 4);
    let pos = cell.to_world(0.0);
    let request = PathRequest {
        object_id: INVALID_ID,
        from: pos,
        to: pos,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: false,
    };
    let center = ICoord2D::new(cell.x, cell.y);
    world.set_goal_cells(42, center, 0, true, PathfindLayerEnum::Ground, true, false);
    let check = |world: &PathfindingSystem, layer| {
        world.check_destination_with_observations(
            &request,
            cell,
            layer,
            0,
            true,
            Some(false),
            Some(&observations),
        )
    };
    assert!(!check(&world, PathfindLayerEnum::Ground));
    assert!(
        !check(&world, PathfindLayerEnum::Top),
        "a missing bridge cell selects the occupied ground cell"
    );
    assert!(!world.goal_cell_available(cell, PathfindLayerEnum::Top, 100));
    world
        .pathfinder
        .set_cell_type_on_layer(cell, PathfindLayerEnum::Top, PathfindCellType::Clear);
    assert!(
        check(&world, PathfindLayerEnum::Top),
        "a bridge cell has its own occupancy"
    );
    assert!(world.goal_cell_available(cell, PathfindLayerEnum::Top, 100));
    world.set_goal_cells(43, center, 0, true, PathfindLayerEnum::Top, false, true);
    assert!(!check(&world, PathfindLayerEnum::Top));
    world.pathfinder.set_cell_type_on_layer(
        cell,
        PathfindLayerEnum::Top,
        PathfindCellType::Impassable,
    );
    assert!(
        !check(&world, PathfindLayerEnum::Top),
        "an impassable bridge cell falls back to occupied ground"
    );
}

#[test]
fn owned_adjust_candidate_applies_human_logical_extent() {
    let factory = crate::object::object_factory::ObjectFactory::new();
    let players = crate::player::PlayerList::new();
    let terrain = crate::terrain::TerrainLogic::new();
    let observations = PathfindObservations {
        factory: &factory,
        players: &players,
        terrain: &terrain,
    };
    let object = crate::object::Object::new_test(100, 100.0);
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    world.set_logical_extent(ICoord2D::new(1, 1), ICoord2D::new(2, 2));
    let cell = GridCoord::new(4, 4);
    let pos = cell.to_world(0.0);
    let request = PathRequest {
        object_id: 100,
        from: pos,
        to: pos,
        surfaces: SURFACE_GROUND,
        is_crusher: false,
        unit_radius: 0.0,
        allow_partial: false,
        move_allies: false,
        ignore_obstacle_id: None,
        is_human: true,
    };
    let owner = AdjustmentOwner {
        source: &object,
        request,
        aircraft_goal_only: false,
    };
    let mut goal = pos;
    assert!(!world.try_adjust_cell_with_observations(
        cell.x,
        cell.y,
        PathfindLayerEnum::Ground,
        SURFACE_GROUND,
        false,
        0,
        true,
        None,
        None,
        &mut goal,
        Some(&observations),
        Some(&owner)
    ));
    let computer = AdjustmentOwner {
        request: PathRequest {
            is_human: false,
            ..owner.request.clone()
        },
        ..owner
    };
    assert!(world.try_adjust_cell_with_observations(
        cell.x,
        cell.y,
        PathfindLayerEnum::Ground,
        SURFACE_GROUND,
        false,
        0,
        true,
        None,
        None,
        &mut goal,
        Some(&observations),
        Some(&computer)
    ));
    assert!(world.try_adjust_cell_with_observations(
        cell.x,
        cell.y,
        PathfindLayerEnum::Ground,
        SURFACE_GROUND,
        false,
        1,
        false,
        None,
        None,
        &mut goal,
        Some(&observations),
        Some(&computer)
    ));
    assert_eq!((goal.x, goal.y), (40.5, 40.5));
}

#[test]
fn quick_path_truncates_destination_and_clips_zone_cell() {
    let mut world = PathfindingSystem::new(8, 8);
    world.new_map();
    let from = Coord3D::new(5.0, 5.0, 0.0);
    let just_outside = Coord3D::new(-1.0, 5.0, 0.0);
    assert!(world.client_safe_quick_does_path_exist(SURFACE_GROUND, &from, &just_outside));
    let clearly_outside = Coord3D::new(-11.0, 5.0, 0.0);
    assert!(!world.client_safe_quick_does_path_exist(SURFACE_GROUND, &from, &clearly_outside));
}

#[test]
fn line_obstacle_owner_is_resolved_per_layer() {
    let mut world = PathfindingSystem::new(8, 6);
    let cell = GridCoord::new(3, 2);
    for x in 0..8 {
        world.pathfinder.set_cell_type_on_layer(
            GridCoord::new(x, 2),
            PathfindLayerEnum::Top,
            PathfindCellType::Clear,
        );
    }
    world
        .pathfinder
        .set_cell_obstacle_id(cell, 41, false, false);
    world
        .pathfinder
        .set_cell_obstacle_id_on_layer(cell, PathfindLayerEnum::Top, 42, false, false);
    let from = GridCoord::new(1, 2).to_world(0.0);
    let to = GridCoord::new(6, 2).to_world(0.0);
    let line = |layer, owner| {
        world.is_line_passable_for_surfaces_on_layer(&from, &to, SURFACE_GROUND, layer, owner)
    };
    assert!(line(PathfindLayerEnum::Ground, Some(41)));
    assert!(!line(PathfindLayerEnum::Top, Some(41)));
    assert!(line(PathfindLayerEnum::Top, Some(42)));
    assert!(!line(PathfindLayerEnum::Ground, Some(INVALID_ID)));
    let explicit_cells = HashSet::from([cell]);
    assert!(world.is_line_passable_on_layer_with_cells(
        &from,
        &to,
        SURFACE_GROUND,
        PathfindLayerEnum::Ground,
        Some(&explicit_cells)
    ));
}

#[test]
fn destination_footprint_uses_the_receiving_layer_obstacle_owner() {
    let mut world = PathfindingSystem::new(8, 8);
    let mut peer = PathfindingSystem::new(8, 8);
    let center = GridCoord::new(3, 3);
    let obstacle = GridCoord::new(4, 3);
    for grid in [&mut world, &mut peer] {
        for x in 2..=4 {
            for y in 2..=4 {
                grid.pathfinder.set_cell_type_on_layer(
                    GridCoord::new(x, y),
                    PathfindLayerEnum::Top,
                    PathfindCellType::Clear,
                );
            }
        }
    }
    world
        .pathfinder
        .set_cell_obstacle_id(obstacle, 41, false, false);
    world.pathfinder.set_cell_obstacle_id_on_layer(
        obstacle,
        PathfindLayerEnum::Top,
        42,
        false,
        false,
    );
    peer.pathfinder
        .set_cell_obstacle_id(obstacle, 42, false, false);
    let valid = |grid: &PathfindingSystem, layer, owner| {
        grid.is_destination_valid(center, layer, SURFACE_GROUND, false, 1, true, owner)
    };
    assert!(valid(&world, PathfindLayerEnum::Ground, Some(41)));
    assert!(!valid(&peer, PathfindLayerEnum::Ground, Some(41)));
    assert!(!valid(&world, PathfindLayerEnum::Top, Some(41)));
    assert!(valid(&world, PathfindLayerEnum::Top, Some(42)));
    assert!(!valid(&world, PathfindLayerEnum::Ground, Some(INVALID_ID)));
    assert!(valid(&world, PathfindLayerEnum::Ground, Some(41)));
}

#[test]
fn destination_footprint_checks_obstacles_not_locomotor_surfaces() {
    let mut world = PathfindingSystem::new(8, 8);
    let cell = GridCoord::new(3, 3);
    let valid = |world: &PathfindingSystem, surfaces, crusher, ignore| {
        world.is_destination_valid(
            cell,
            PathfindLayerEnum::Ground,
            surfaces,
            crusher,
            0,
            true,
            ignore,
        )
    };
    world
        .pathfinder
        .set_cell_type(cell, PathfindCellType::Water);
    assert!(valid(&world, SURFACE_GROUND, false, None));
    world
        .pathfinder
        .set_cell_type(cell, PathfindCellType::Impassable);
    assert!(!valid(&world, SURFACE_AIR, false, None));
    world.pathfinder.set_cell_obstacle_id(cell, 41, true, false);
    assert!(!valid(&world, SURFACE_GROUND, true, None));
    assert!(valid(&world, SURFACE_GROUND, true, Some(41)));
}
