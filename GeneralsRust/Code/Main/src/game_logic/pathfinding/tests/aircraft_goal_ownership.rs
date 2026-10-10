//! Aircraft reservations are derived from the driving match's object goals.
use super::super::*;
use crate::game_logic::{GameLogic, KindOf, Object, ObjectId, ObjectType, Team, ThingTemplate};

fn aircraft(id: u32, appearance: LocomotorAppearance, goal: Vec3) -> Object {
    let mut template = ThingTemplate::new("GoalAircraft");
    template.add_kind_of(KindOf::Aircraft);
    template.add_kind_of(KindOf::Vehicle);
    let mut object = Object::new(template, ObjectId(id), Team::USA);
    object.object_type = ObjectType::Aircraft;
    object.loco_appearance = appearance;
    object.cur_locomotor_name = Some("GoalAirLocomotor".into());
    object.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
    object.selection_radius = 8.0;
    object.movement.target_position = Some(goal);
    object
}

fn adjusted(world: &GameLogic, seeker: ObjectId, dest: Vec3) -> Vec3 {
    world.pathfinding_system.adjust_target_destination(
        seeker.0,
        &world.objects,
        dest,
        dest + Vec3::new(60.0, 0.0, 0.0),
        8.0,
        SURFACE_AIR,
        false,
        8.0,
        0.0,
        200.0,
        0.0,
    )
}

#[test]
fn aircraft_goal_projection_uses_half_cell_footprints_on_the_driving_grid() {
    let mut grid =
        PathfindingGrid::new_with_origin(Vec3::new(-40.0, 0.0, -30.0), 200.0, 200.0, 10.0);
    let goal = Vec3::new(36.0, 40.0, 46.0);
    let object = aircraft(7, LocomotorAppearance::Hover, goal);
    assert_eq!(PathfindingGrid::radius_and_center(8.0, 10.0), (1, false));
    let mut objects = HashMap::from([(object.id, object)]);
    let claims = grid.aircraft_goal_claims(&objects, 8);
    let expected = HashSet::from([
        GridPos::new(7, 7),
        GridPos::new(7, 8),
        GridPos::new(8, 7),
        GridPos::new(8, 8),
    ]);
    assert_eq!(claims, expected);
    grid.update_dynamic_obstacles(&objects);
    for cell in expected {
        assert_eq!(grid.goal_aircraft(cell), 7);
    }
    assert_eq!(grid.goal_aircraft(GridPos::new(6, 6)), 0);
    objects.get_mut(&ObjectId(7)).unwrap().selection_radius = 2.0;
    assert_eq!(
        grid.aircraft_goal_claims(&objects, 8),
        HashSet::from([GridPos::new(7, 7)])
    );
    assert!(
        grid.aircraft_goal_claims(&objects, 7).is_empty(),
        "own reservation is allowed"
    );
}

#[test]
fn aircraft_goal_projection_excludes_non_reserving_runtime_states() {
    let grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let goal = Vec3::new(80.0, 40.0, 80.0);
    let mut object = aircraft(1, LocomotorAppearance::Hover, goal);
    let claims = |object: &Object| {
        grid.aircraft_goal_claims(&HashMap::from([(object.id, object.clone())]), 2)
    };
    assert!(!claims(&object).is_empty());
    object.cur_locomotor_name = None;
    assert!(claims(&object).is_empty());
    object.cur_locomotor_name = Some("GoalAirLocomotor".into());
    object.loco_appearance = LocomotorAppearance::Thrust;
    assert!(claims(&object).is_empty());
    object.loco_appearance = LocomotorAppearance::Wings;
    object.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_GROUND;
    assert!(
        claims(&object).is_empty(),
        "ground locomotor reserves a ground goal"
    );
    object.locomotor_surfaces = crate::game_logic::object::LOCO_SURFACE_AIR;
    object.health.current = 0.0;
    assert!(
        claims(&object).is_empty(),
        "dead objects do not reserve goals"
    );
    object.health.current = object.health.maximum;
    object.template_mut().add_kind_of(KindOf::Immobile);
    assert!(claims(&object).is_empty());
}

#[test]
fn aircraft_attack_query_uses_current_goals_instead_of_old_occupancy() {
    let mut world = GameLogic::new();
    world.pathfinding_system = PathfindingSystem::new(300.0, 300.0);
    let dest = Vec3::new(75.0, 40.0, 75.0);
    world.objects.insert(
        ObjectId(1),
        aircraft(1, LocomotorAppearance::Hover, Vec3::new(180.0, 40.0, 180.0)),
    );
    world
        .objects
        .insert(ObjectId(2), aircraft(2, LocomotorAppearance::Wings, dest));
    world
        .pathfinding_system
        .grid
        .update_dynamic_obstacles(&world.objects);
    let blocked = adjusted(&world, ObjectId(1), dest);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .movement
        .target_position = None;
    let clear = adjusted(&world, ObjectId(1), dest);
    assert_ne!(
        blocked, clear,
        "clearing a live goal takes effect before another occupancy refresh"
    );
    let mut fresh = PathfindingSystem::new(300.0, 300.0);
    fresh.grid.update_dynamic_obstacles(&world.objects);
    assert_eq!(
        clear,
        fresh.adjust_target_destination(
            1,
            &world.objects,
            dest,
            dest + Vec3::new(60.0, 0.0, 0.0),
            8.0,
            SURFACE_AIR,
            false,
            8.0,
            0.0,
            200.0,
            0.0
        )
    );
}

#[test]
fn aircraft_attack_spiral_preserves_original_candidate_order_and_failure() {
    let mut grid = PathfindingGrid::new(200.0, 200.0, 10.0);
    let dest = Vec3::new(75.0, 40.0, 75.0);
    let claims = HashSet::from([GridPos::new(7, 7)]);
    grid.set_cell_type(GridPos::new(8, 7), PathfindCellType::Water);
    let mut out = dest;
    assert!(grid.adjust_target_destination(&mut out, 2.0, 1, &claims, |_| true));
    assert_eq!(
        out,
        Vec3::new(85.0, 40.0, 75.0),
        "right first, aircraft may occupy water"
    );
    let mut impossible = dest;
    assert!(!grid.adjust_target_destination(&mut impossible, 2.0, 1, &claims, |_| false));
    assert_eq!(
        impossible, dest,
        "failed range search does not mutate the goal"
    );
    let mut off_map = Vec3::new(205.0, 40.0, 75.0);
    let before = off_map;
    assert!(!grid.adjust_target_destination(&mut off_map, 8.0, 1, &HashSet::new(), |_| true));
    assert_eq!(off_map, before);
}

// Only the foreign Core sentinel and authored INI catalogs need isolation.
// The ordinary projection/spiral tests above have no global synchronization.
fn isolated(name: &str) -> bool {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    const MARKER: &str = "GENERALS_AIRCRAFT_GOAL_OWNER_TEST";
    let module = module_path!().split_once("::").unwrap().1;
    let name = format!("{module}::{name}");
    if std::env::var(MARKER).as_deref() == Ok(name.as_str()) {
        return false;
    }
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([&name, "--exact", "--test-threads=1", "--nocapture"])
        .env(MARKER, &name)
        .env("GENERALS_GAMEWORLD_SHADOW", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let pipes: [Box<dyn Read + Send>; 2] = [
        Box::new(child.stdout.take().unwrap()),
        Box::new(child.stderr.take().unwrap()),
    ];
    let readers = pipes.map(|mut pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            pipe.read_to_end(&mut bytes).unwrap();
            bytes
        })
    });
    let deadline = Instant::now() + Duration::from_secs(25);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("{name} exceeded deadline");
        }
        std::thread::sleep(Duration::from_millis(25));
    };
    let output =
        readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
    assert!(status.success(), "{name}: {output:?}");
    assert!(
        output[0].contains("1 passed; 0 failed"),
        "exact child ran no regression: {output:?}"
    );
    true
}

fn authored_world(goal: Vec3) -> (GameLogic, ObjectId, ObjectId) {
    use game_engine::common::ini::ini_locomotor::load_locomotors_from_str;
    load_locomotors_from_str(
        r#"
Locomotor HqAwymHover
  Surfaces = AIR
  Speed = 30
  Appearance = HOVER
End
Locomotor HqAwymWings
  Surfaces = AIR
  Speed = 30
  Appearance = WINGS
End
"#,
    )
    .expect("authored flight locomotors");
    let mut parser = crate::assets::IniParser::new();
    parser
        .parse_ini_content(
            r#"
Object AmericaVehicleComanche
  KindOf = VEHICLE AIRCRAFT PRODUCED_AT_HELIPAD
  Geometry = CYLINDER
  GeometryMajorRadius = 8
  GeometryHeight = 12
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 220
  End
  Behavior = AIUpdateInterface ModuleTag_AI
  End
  Locomotor = SET_NORMAL HqAwymHover
End
Object HqAwymWingAircraft
  KindOf = VEHICLE AIRCRAFT
  Geometry = CYLINDER
  GeometryMajorRadius = 8
  GeometryHeight = 12
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 220
  End
  Behavior = JetAIUpdate ModuleTag_AI
    NeedsRunway = Yes
  End
  Locomotor = SET_NORMAL HqAwymWings
End
"#,
            "hq_awym_aircraft.ini",
        )
        .expect("authored aircraft definitions");
    let mut world = GameLogic::new();
    for name in ["AmericaVehicleComanche", "HqAwymWingAircraft"] {
        world.templates.insert(
            name.to_owned(),
            GameLogic::build_template_from_object_definition(
                name,
                parser.get_definition(name).unwrap(),
                None,
            ),
        );
    }
    let hover = world
        .create_object("AmericaVehicleComanche", Team::USA, goal)
        .unwrap();
    let wings = world
        .create_object(
            "HqAwymWingAircraft",
            Team::USA,
            goal + Vec3::new(60.0, 0.0, 0.0),
        )
        .unwrap();
    world.override_world_size(600.0, 600.0);
    for id in [hover, wings] {
        let object = world.objects.get_mut(&id).unwrap();
        assert_eq!(
            object.selection_radius, 8.0,
            "authored geometry owns the footprint radius"
        );
        object.movement.target_position = Some(goal);
        assert!(PathfindingGrid::is_aircraft_that_adjusts_destination(
            object
        ));
        assert!(!PathfindingGrid::is_doing_ground_movement(object));
    }
    assert_eq!(
        world.objects[&hover].loco_appearance,
        LocomotorAppearance::Hover
    );
    assert_eq!(
        world.objects[&wings].loco_appearance,
        LocomotorAppearance::Wings
    );
    (world, hover, wings)
}

#[test]
fn authored_same_id_matches_keep_aircraft_goals_and_foreign_core_independent() {
    if isolated("authored_same_id_matches_keep_aircraft_goals_and_foreign_core_independent") {
        return;
    }
    use gamelogic::common::Coord3D;
    let ai = gamelogic::ai::the_ai();
    let foreign = ai.read().unwrap().pathfinder().unwrap();
    let sentinel = Coord3D::new(45.0, 45.0, 0.0);
    let poisoned = Coord3D::new(75.0, 75.0, 0.0);
    {
        let mut pathfinder = foreign.write().unwrap();
        pathfinder.reset_with_size(30, 30);
        pathfinder.update_aircraft_goal(&sentinel, 1, 0, true);
        pathfinder.update_aircraft_goal(&poisoned, 999, 1, false);
    }
    let foreign_goal = || {
        let mut out = Coord3D::new(0.0, 0.0, 0.0);
        assert!(
            foreign
                .read()
                .unwrap()
                .goal_position_for_unit(1, 2.0, &mut out)
        );
        (out.x, out.y)
    };
    let before = foreign_goal();
    let dest = Vec3::new(75.0, 40.0, 75.0);
    let (mut first, hover, wings) = authored_world(dest);
    let reference = adjusted(&first, hover, dest);
    let (mut second, hover_b, wings_b) = authored_world(Vec3::new(175.0, 40.0, 175.0));
    assert_eq!((hover, wings), (hover_b, wings_b));
    assert_eq!(foreign_goal(), before, "constructors are inert");
    first
        .pathfinding_system
        .grid
        .update_dynamic_obstacles(&first.objects);
    assert_eq!(
        foreign_goal(),
        before,
        "owned occupancy cannot publish to Core"
    );
    second
        .pathfinding_system
        .grid
        .update_dynamic_obstacles(&second.objects);
    let _ = second.pathfinding_system.adjust_landing_destination_for(
        hover_b.0,
        &second.objects,
        Vec3::ZERO,
        dest,
    );
    assert_eq!(adjusted(&first, hover, dest), reference);
    assert_eq!(foreign_goal(), before);
    second
        .objects
        .get_mut(&wings_b)
        .unwrap()
        .movement
        .target_position = Some(dest);
    second
        .pathfinding_system
        .grid
        .update_dynamic_obstacles(&second.objects);
    assert_eq!(adjusted(&first, hover, dest), reference);
    second.reset();
    drop(second);
    assert_eq!(adjusted(&first, hover, dest), reference);
    assert_eq!(
        foreign_goal(),
        before,
        "reset/drop cannot overwrite foreign Core goals"
    );
    foreign.write().unwrap().reset_with_size(30, 30);
    assert_eq!(
        adjusted(&first, hover, dest),
        reference,
        "poisoned Core goals do not influence own attack placement"
    );
    let object = first.objects.get_mut(&hover).unwrap();
    object.status.disabled_unmanned = true;
    assert!(PathfindingGrid::is_doing_ground_movement(object));
    assert!(
        first
            .pathfinding_system
            .grid
            .aircraft_goal_claims(&first.objects, wings.0)
            .is_empty()
    );
}

#[test]
fn authored_aircraft_save_load_continues_owned_goal_changes() {
    if isolated("authored_aircraft_save_load_continues_owned_goal_changes") {
        return;
    }
    let dest = Vec3::new(75.0, 40.0, 75.0);
    let (mut source, hover, wings) = authored_world(dest);
    source
        .pathfinding_system
        .grid
        .update_dynamic_obstacles(&source.objects);
    // These are mutable locomotor values, different from the authored defaults.
    // Reconstructing immutable appearance must not reapply an entire binding.
    {
        let object = source.objects.get_mut(&wings).unwrap();
        object.movement.max_speed = 0.5;
        object.braking = 3.0;
        object.wander_angle_offset = 0.37;
        object.is_braking = true;
    }
    let dir = tempfile::TempDir::new().unwrap();
    let mut manager = crate::save_load::SaveFileManager::with_save_directory(dir.path());
    manager.init().unwrap();
    manager
        .quick_save(&source)
        .expect("actual Common named-chunk writer");
    let (decoded, _) = manager
        .load_game_snapshot("quicksave")
        .expect("actual save decoder");
    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    // Common saves transfer runtime into an already prepared map candidate.
    // This synthetic open map has the same known extent on both instances.
    restored.override_world_size(600.0, 600.0);
    manager
        .restore_game_snapshot(&decoded, &mut restored)
        .expect("actual detached restore route");
    assert_eq!(restored.world_bounds(), source.world_bounds());
    for id in [hover, wings] {
        let loaded = &restored.objects[&id];
        let original = &source.objects[&id];
        assert_eq!(loaded.selection_radius, original.selection_radius);
        assert_eq!(loaded.cur_locomotor_name, original.cur_locomotor_name);
        assert_eq!(
            loaded.loco_appearance, original.loco_appearance,
            "current immutable locomotor definition must be resolved after load"
        );
    }
    let loaded = &restored.objects[&wings];
    assert_eq!(loaded.movement.max_speed, 0.5);
    assert_eq!(loaded.braking, 3.0);
    assert_eq!(loaded.wander_angle_offset, 0.37);
    assert!(
        loaded.is_braking,
        "saved runtime flags are not definition defaults"
    );
    assert_eq!(
        restored.objects[&wings].movement.target_position,
        source.objects[&wings].movement.target_position
    );
    assert_eq!(
        adjusted(&restored, hover, dest),
        adjusted(&source, hover, dest)
    );
    for next in [Some(dest + Vec3::new(60.0, 0.0, 0.0)), None, Some(dest)] {
        source
            .objects
            .get_mut(&wings)
            .unwrap()
            .movement
            .target_position = next;
        restored
            .objects
            .get_mut(&wings)
            .unwrap()
            .movement
            .target_position = next;
        source
            .pathfinding_system
            .grid
            .update_dynamic_obstacles(&source.objects);
        restored
            .pathfinding_system
            .grid
            .update_dynamic_obstacles(&restored.objects);
        assert_eq!(
            adjusted(&restored, hover, dest),
            adjusted(&source, hover, dest),
            "continuation observes the same current goal"
        );
    }
    let reference = adjusted(&source, hover, dest);
    restored.reset();
    restored
        .pathfinding_system
        .grid
        .update_dynamic_obstacles(&restored.objects);
    assert!(
        restored
            .pathfinding_system
            .grid
            .aircraft_goal_claims(&restored.objects, hover.0)
            .is_empty()
    );
    assert_eq!(
        adjusted(&source, hover, dest),
        reference,
        "reset cannot clear the other world's goals"
    );
}

#[test]
fn authored_aircraft_liveness_ignores_same_id_foreign_shadow_health() {
    if isolated("authored_aircraft_liveness_ignores_same_id_foreign_shadow_health") {
        return;
    }
    use crate::gameworld_shadow::{
        CoupledTickGuard, GameWorldShadow, coupled_entity_health, with_coupled_shadow,
    };
    let dest = Vec3::new(75.0, 40.0, 75.0);
    let (mut driving, hover, wings) = authored_world(dest);
    // Only the hover reserves this goal; the wings is the attack seeker.
    driving
        .objects
        .get_mut(&wings)
        .unwrap()
        .movement
        .target_position = None;
    let (foreign, foreign_hover, foreign_wings) = authored_world(Vec3::new(175.0, 40.0, 175.0));
    assert_eq!((hover, wings), (foreign_hover, foreign_wings));
    let mut shadow = GameWorldShadow::new(16);
    shadow.sync_from_host(&foreign);
    let entity = shadow
        .entity_for_host(foreign_hover)
        .expect("actual foreign mapping");
    assert_eq!(
        shadow.world().entity(entity).unwrap().health,
        foreign.objects[&foreign_hover].health.current
    );
    let baseline_claims = driving
        .pathfinding_system
        .grid
        .aircraft_goal_claims(&driving.objects, wings.0);
    let baseline_attack = adjusted(&driving, wings, dest);
    assert!(!baseline_claims.is_empty());
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .health = 0.0;
    {
        let _couple = CoupledTickGuard::enter();
        with_coupled_shadow(&mut shadow, || {
            assert_eq!(
                coupled_entity_health(hover),
                Some(0.0),
                "positive control reads the dead foreign shadow"
            );
            assert!(
                driving.objects[&hover].is_alive(),
                "ordinary liveness uses the owned body"
            );
            assert_eq!(
                driving
                    .pathfinding_system
                    .grid
                    .aircraft_goal_claims(&driving.objects, wings.0),
                baseline_claims
            );
            assert_eq!(adjusted(&driving, wings, dest), baseline_attack);
            driving
                .pathfinding_system
                .grid
                .update_dynamic_obstacles(&driving.objects);
            for cell in &baseline_claims {
                assert_eq!(
                    driving.pathfinding_system.grid.goal_aircraft(*cell),
                    hover.0,
                    "alive host reserves despite dead foreign HP"
                );
            }
        });
    }
    // Reverse the disagreement. A dead host cannot acquire a reservation from
    // a healthy foreign same-ID entity, nor retain its earlier cached footprint.
    driving.objects.get_mut(&hover).unwrap().health.current = 0.0;
    shadow
        .world_mut()
        .world_mut()
        .entity_mut(entity)
        .unwrap()
        .health = 220.0;
    let dead_attack = adjusted(&driving, wings, dest);
    assert_ne!(
        dead_attack, baseline_attack,
        "the local HP change has a real effect"
    );
    {
        let _couple = CoupledTickGuard::enter();
        with_coupled_shadow(&mut shadow, || {
            assert_eq!(
                coupled_entity_health(hover),
                Some(220.0),
                "positive control reads the live foreign shadow"
            );
            assert!(
                !driving.objects[&hover].is_alive(),
                "a healthy foreign body cannot revive this owner"
            );
            assert!(
                driving
                    .pathfinding_system
                    .grid
                    .aircraft_goal_claims(&driving.objects, wings.0)
                    .is_empty()
            );
            assert_eq!(adjusted(&driving, wings, dest), dead_attack);
            driving
                .pathfinding_system
                .grid
                .update_dynamic_obstacles(&driving.objects);
            for cell in &baseline_claims {
                assert_eq!(
                    driving.pathfinding_system.grid.goal_aircraft(*cell),
                    0,
                    "dead host clears the old footprint despite healthy foreign HP"
                );
            }
        });
    }
}
