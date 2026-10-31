use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use glam::Vec3;

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("WaypointInfantry");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert("WaypointInfantry".into(), template);
    let unit = world
        .create_object_for_player("WaypointInfantry", 0, Vec3::ZERO)
        .expect("unit");
    (world, unit)
}

#[test]
fn appending_waypoints_keeps_the_current_movement_target_in_each_world() {
    let (mut first, first_id) = world();
    let (mut second, second_id) = world();
    assert_eq!(first_id, second_id);
    let first_goal = Vec3::new(80.0, 0.0, 0.0);
    let second_goal = Vec3::new(0.0, 0.0, 90.0);
    assert!(first.append_unit_waypoint(first_id, first_goal));
    assert!(second.append_unit_waypoint(second_id, second_goal));
    let first_target = first
        .host_object(first_id)
        .unwrap()
        .movement
        .target_position;
    let second_target = second
        .host_object(second_id)
        .unwrap()
        .movement
        .target_position;
    assert_eq!(first_target, Some(first_goal));
    assert_eq!(second_target, Some(second_goal));
    let first_path = first.host_object(first_id).unwrap().movement.path.clone();
    let appended_goal = Vec3::new(160.0, 0.0, 0.0);
    assert!(first.append_unit_waypoint(first_id, appended_goal));
    let updated_path = &first.host_object(first_id).unwrap().movement.path;
    assert!(updated_path.starts_with(&first_path));
    assert_eq!(updated_path.last(), Some(&appended_goal));
    assert_eq!(
        first
            .host_object(first_id)
            .unwrap()
            .movement
            .target_position,
        first_target
    );
    assert_eq!(
        second
            .host_object(second_id)
            .unwrap()
            .movement
            .target_position,
        second_target
    );
}
