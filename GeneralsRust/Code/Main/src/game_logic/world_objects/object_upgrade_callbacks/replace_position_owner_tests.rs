use super::*;
use crate::fixture_content::{FixtureObject, admit_fixture_objects};
use crate::game_logic::pathfinding::GridPos;
use glam::{Mat4, Quat, Vec3};

#[test]
fn replace_object_mux_requires_both_authored_triggers_even_for_become_real_names() {
    const SOURCE: &str = "FakeGLAReplacementMuxSource";
    const REPLACEMENT: &str = "ReplacementMuxTarget";
    const FIRST: &str = "Upgrade_BecomeRealReplacementMuxSource";
    const SECOND: &str = "Upgrade_UnlockReplacementMuxSource";

    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "Replacement owner", false));
    crate::game_logic::host_upgrade_rules::register_test_upgrade(&mut world, FIRST, "OBJECT", 0, 1);
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &mut world, SECOND, "OBJECT", 0, 2,
    );
    admit_fixture_objects(
        &mut world,
        &[
            FixtureObject::new(SOURCE)
                .kind_of("STRUCTURE")
                .health(120.0)
                .module(format!(
                    "Behavior = ReplaceObjectUpgrade ReplaceMux\n TriggeredBy = {FIRST} {SECOND}\n RequiresAllTriggers = Yes\n ReplaceObject = {REPLACEMENT}\nEnd"
                )),
            FixtureObject::new(REPLACEMENT)
                .kind_of("STRUCTURE")
                .health(250.0),
        ],
    );
    let source = world
        .create_object_for_player(SOURCE, 1, Vec3::ZERO)
        .unwrap();
    world.apply_upgrade_to_object(source, FIRST);
    assert!(world.host_object(source).unwrap().is_alive());
    assert_eq!(world.host_objects().registered_len(), 1);

    world.apply_upgrade_to_object(source, SECOND);
    assert!(world.host_object(source).unwrap().status.destroyed);
    assert!(
        world
            .host_objects()
            .registered_values()
            .any(|actor| actor.template_name == REPLACEMENT && actor.is_alive())
    );
}

#[test]
fn saved_replace_object_latch_does_not_replay_a_previously_missing_target() {
    const SOURCE: &str = "ReplacementLatchSource";
    const REPLACEMENT: &str = "ReplacementLatchTarget";
    const UPGRADE: &str = "Upgrade_ActivateReplacementLatch";

    let world = |has_target| {
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::GLA, "Replacement owner", false));
        crate::game_logic::host_upgrade_rules::register_test_upgrade(
            &mut world, UPGRADE, "OBJECT", 0, 1,
        );
        let mut objects = vec![
            FixtureObject::new(SOURCE)
                .kind_of("STRUCTURE")
                .health(120.0)
                .module(format!(
                    "Behavior = ReplaceObjectUpgrade ReplaceLatch\n TriggeredBy = {UPGRADE}\n ReplaceObject = {REPLACEMENT}\nEnd"
                )),
        ];
        if has_target {
            objects.push(
                FixtureObject::new(REPLACEMENT)
                    .kind_of("STRUCTURE")
                    .health(250.0),
            );
        }
        admit_fixture_objects(&mut world, &objects);
        world
    };
    let mut source = world(false);
    let id = source
        .create_object_for_player(SOURCE, 1, Vec3::ZERO)
        .unwrap();
    source.apply_upgrade_to_object(id, UPGRADE);
    assert!(source.host_object(id).unwrap().is_alive());

    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut receiving = world(true);
    builder
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();

    receiving.apply_upgrade_to_object(id, UPGRADE);
    assert!(receiving.host_object(id).unwrap().is_alive());
    assert_eq!(receiving.host_objects().registered_len(), 1);
    assert!(receiving.remove_object_upgrade_with_receiving_rules(id, UPGRADE));
    receiving.apply_upgrade_to_object(id, UPGRADE);
    assert!(receiving.host_object(id).unwrap().status.destroyed);
    assert!(
        receiving
            .host_objects()
            .registered_values()
            .any(|actor| actor.template_name == REPLACEMENT && actor.is_alive())
    );
}

#[test]
fn replacement_transfers_rotated_static_footprint_before_build_complete_and_player_callback() {
    const SOURCE: &str = "StaticReplacementSource";
    const TARGET: &str = "StaticReplacementTarget";
    const CHILD: &str = "StaticReplacementCompletionWitness";
    const REPLACE: &str = "Upgrade_ReplaceStaticFootprint";
    const COMPLETE: &str = "Upgrade_ObserveStaticReplacementComplete";

    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "Replacement owner", false));
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &mut world, REPLACE, "OBJECT", 0, 1,
    );
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &mut world, COMPLETE, "OBJECT", 0, 1,
    );
    admit_fixture_objects(
        &mut world,
        &[
            FixtureObject::new(SOURCE)
                .kind_of("STRUCTURE IMMOBILE")
                .health(120.0)
                .field("Geometry = CYLINDER")
                .field("GeometryIsSmall = No")
                .field("GeometryMajorRadius = 38")
                .field("GeometryHeight = 12")
                .module(format!(
                    "Behavior = ReplaceObjectUpgrade ReplaceStatic\n TriggeredBy = {REPLACE}\n ReplaceObject = {TARGET}\nEnd"
                )),
            FixtureObject::new(TARGET)
                .kind_of("STRUCTURE IMMOBILE")
                .health(250.0)
                .build_cost(317)
                .field("Geometry = BOX")
                .field("GeometryIsSmall = No")
                .field("GeometryMajorRadius = 26")
                .field("GeometryMinorRadius = 3")
                .field("GeometryHeight = 12")
                .module(format!(
                    "Behavior = GrantUpgradeCreate CompletionGrant\n UpgradeToGrant = {COMPLETE}\nEnd"
                ))
                .module(format!(
                    "Behavior = ObjectCreationUpgrade CompletionWitness\n TriggeredBy = {COMPLETE}\n UpgradeObject = OCL_StaticReplacementWitness\nEnd"
                )),
            FixtureObject::new(CHILD).kind_of("INFANTRY").health(50.0),
        ],
    );
    world.admit_object_creation_list_sources([format!(
        "ObjectCreationList OCL_StaticReplacementWitness\n CreateObject\n ObjectNames = {CHILD}\n Count = 1\n Offset = 40 0 0\n End\nEnd\n"
    )]).unwrap();

    let position = Vec3::new(105.0, 0.0, 105.0);
    let transform = Mat4::from_rotation_translation(
        Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        position,
    );
    let source = world.create_object_for_player(SOURCE, 1, position).unwrap();
    assert!(world.set_registered_object_transform(source, transform));
    let side = world
        .pathfinding_system
        .grid
        .world_to_grid(position + Vec3::X * 20.0);
    let end = world
        .pathfinding_system
        .grid
        .world_to_grid(position - Vec3::Z * 20.0);
    assert!(world.is_pathfinding_static_blocked(side.x, side.y));
    assert!(world.is_pathfinding_static_blocked(end.x, end.y));
    let before_spent = world.get_player(1).unwrap().statistics.resources_spent();

    world.apply_upgrade_to_object(source, REPLACE);
    let child = world
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == CHILD)
        .expect("BuildComplete OCL witness");
    let expected = transform.transform_point3(Vec3::X * 40.0);
    assert!(child.get_position().abs_diff_eq(expected, 0.001));
    assert!(!world.is_pathfinding_static_blocked(side.x, side.y));
    assert!(world.is_pathfinding_static_blocked(end.x, end.y));
    assert!(world.host_object(source).unwrap().status.destroyed);
    assert_eq!(
        world.get_player(1).unwrap().statistics.resources_spent() - before_spent,
        317,
    );

    world.process_destroy_list();
    assert!(world.host_object(source).is_none());
    assert!(!world.is_pathfinding_static_blocked(side.x, side.y));
    assert!(world.is_pathfinding_static_blocked(end.x, end.y));
}

#[test]
fn authored_replacement_takes_the_exact_target_pose_and_team_before_retirement() {
    const SOURCE: &str = "ReplacementPositionSource";
    const REPLACEMENT: &str = "ReplacementPositionTarget";
    const UPGRADE: &str = "Upgrade_ActivatePositionReplacement";

    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::GLA, "Replacement owner", false));
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &mut world, UPGRADE, "OBJECT", 0, 1,
    );
    admit_fixture_objects(
        &mut world,
        &[
            FixtureObject::new(SOURCE)
                .kind_of("VEHICLE SELECTABLE")
                .health(100.0)
                .walker()
                .module(format!(
                    "Behavior = ReplaceObjectUpgrade FixtureReplace\n TriggeredBy = {UPGRADE}\n ReplaceObject = {REPLACEMENT}\nEnd"
                )),
            FixtureObject::new(REPLACEMENT)
                .kind_of("VEHICLE SELECTABLE")
                .health(175.0)
                .walker(),
        ],
    );
    world.pathfinding_system.map_ready = true;
    let position = Vec3::new(100.0, 0.0, 100.0);
    let source = world.create_object_for_player(SOURCE, 1, position).unwrap();
    assert!(world.set_registered_object_transform(
        source,
        Mat4::from_rotation_translation(Quat::from_rotation_y(0.6), position),
    ));
    let original = world.host_object(source).unwrap();
    let transform = original.get_transform_matrix();
    let team = original.team_instance_id;
    let (x, y) = original.pathfind_cur_cell;
    let cell = GridPos::new(x, y);
    assert_eq!(world.pathfinding_ground_position(cell), Some(source));
    world.register_path_goal(source, position);
    assert_eq!(world.pathfinding_ground_goal(cell), Some(source));

    world
        .apply_upgrade_to_object_with_effects_checked(source, UPGRADE, &mut Default::default())
        .unwrap();
    let replacement = world
        .host_objects()
        .registered_values()
        .find(|object| object.template_name == REPLACEMENT)
        .unwrap()
        .id;
    assert!(world.host_object(source).unwrap().status.destroyed);
    let replacement_actor = world.host_object(replacement).unwrap();
    assert_eq!(replacement_actor.get_transform_matrix(), transform);
    assert_eq!(replacement_actor.owner_player_id, Some(1));
    assert_eq!(replacement_actor.team_instance_id, team);
    assert_eq!(replacement_actor.health.maximum, 175.0);
    // ReplaceObjectUpgrade.cpp:83 invokes Player's completion callback
    // after the constructor's transform has already registered a pose.
    // Its remove footprint step calls removePos/removeGoal; adding the
    // footprint back only classifies structures (AIPathfind.cpp:4125–4172).
    assert_eq!(replacement_actor.pathfind_cur_cell, (-1, -1));
    assert_eq!(world.pathfinding_ground_position(cell), None);
    assert_ne!(world.pathfinding_ground_goal(cell), Some(source));

    world.process_destroy_list();
    assert!(world.host_object(source).is_none());
    assert_eq!(world.pathfinding_ground_position(cell), None);
    let new_position = position + Vec3::new(20.0, 0.0, 0.0);
    assert!(world.set_registered_object_transform(
        replacement,
        Mat4::from_rotation_translation(Quat::from_rotation_y(0.6), new_position),
    ));
    let (x, y) = world.host_object(replacement).unwrap().pathfind_cur_cell;
    let new_cell = GridPos::new(x, y);
    assert_ne!(new_cell, cell);
    assert_eq!(
        world.pathfinding_ground_position(new_cell),
        Some(replacement)
    );
}

#[test]
fn authored_replace_removes_live_wall_footprint_before_source_retirement() {
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use gamelogic::ai::pathfind_astar::PathfindLayerEnum;

    const SOURCE: &str = "ReplacementWallSource";
    const TARGET: &str = "ReplacementWallTarget";
    const UPGRADE: &str = "Upgrade_ReplaceWallFootprint";

    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Occupant owner", true));
    world.add_player(Player::new(1, Team::GLA, "Replacement owner", false));
    crate::game_logic::host_upgrade_rules::register_test_upgrade(
        &mut world, UPGRADE, "OBJECT", 0, 1,
    );
    admit_fixture_objects(
        &mut world,
        &[
            FixtureObject::new(SOURCE)
                .kind_of("STRUCTURE IMMOBILE WALK_ON_TOP_OF_WALL")
                .health(250.0)
                .field("Geometry = BOX")
                .field("GeometryIsSmall = No")
                .field("GeometryMajorRadius = 30")
                .field("GeometryMinorRadius = 30")
                .module(format!(
                    "Behavior = ReplaceObjectUpgrade ReplaceWall\n TriggeredBy = {UPGRADE}\n ReplaceObject = {TARGET}\nEnd"
                )),
            FixtureObject::new(TARGET).kind_of("STRUCTURE IMMOBILE").health(300.0),
            FixtureObject::new("ReplacementWallOccupant")
                .kind_of("INFANTRY").health(100.0)
                .module("Behavior = InstantDeathBehavior RetireWallOccupant\nEnd"),
        ],
    );
    let position = Vec3::new(105.0, 0.0, 105.0);
    let source = world.create_object_for_player(SOURCE, 1, position).unwrap();
    let victim = world
        .create_object_for_player("ReplacementWallOccupant", 0, position)
        .unwrap();
    let ground = world
        .create_object_for_player("ReplacementWallOccupant", 0, position + Vec3::Y * 8.0)
        .unwrap();
    world.host_object_mut(victim).unwrap().pathfind_layer = PathfindLayerEnum::Wall as u8;
    world.host_object_mut(ground).unwrap().pathfind_layer = PathfindLayerEnum::Ground as u8;
    world
        .pathfinding_system
        .sync_wall_pieces_from_objects(world.objects.read(), 8.0);
    assert!(world.pathfinding_system.is_point_on_wall(position));

    world
        .apply_upgrade_to_object_with_effects_checked(source, UPGRADE, &mut Default::default())
        .unwrap();

    let splatted = world.host_object(victim).unwrap();
    assert_eq!(splatted.health.current, 0.0);
    assert_eq!(splatted.status.death_type, HostDeathType::Splatted);
    assert_eq!(splatted.last_damage_source, Some(victim));
    assert_eq!(world.host_object(ground).unwrap().health.current, 100.0);
    assert!(world.host_object(source).unwrap().status.destroyed);
    assert_eq!(
        world
            .pending_object_removal_snapshot()
            .into_iter()
            .map(|event| event.id)
            .collect::<Vec<_>>(),
        vec![victim, source],
    );
    assert!(
        world
            .host_objects()
            .registered_values()
            .any(|object| object.template_name == TARGET && object.is_alive())
    );
}
