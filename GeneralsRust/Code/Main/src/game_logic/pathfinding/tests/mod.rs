mod aircraft_goal_ownership;
mod attack_and_live_paths;
mod foundational_grid;
mod group_destination_owner;
mod path_order;
mod routing_and_occupancy;

mod path_follow_receiver;

fn adjust_fixture(
    grid: &super::PathfindingGrid,
    start: glam::Vec3,
    dest: &mut glam::Vec3,
    surfaces: u32,
    crusher: bool,
    radius: f32,
    ground: &super::PathfindingGround<'_>,
) -> bool {
    use crate::game_logic::{GameLogic, HostGeometryType, KindOf, Player, Team, ThingTemplate};
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Owner", true));
    let mut template = ThingTemplate::new("DestinationFixture");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    template.geometry_info.geom_type = HostGeometryType::Sphere;
    template.geometry_info.major_radius = radius;
    template.geometry_info.authored = true;
    world.templates.insert("DestinationFixture".into(), template);
    let id = world.create_object_for_player("DestinationFixture", 0, start).unwrap();
    let actor = world.host_object_mut(id).unwrap();
    actor.pathfind_layer = grid.layer_for_destination(start, ground) as u8;
    actor.crusher_level = u8::from(crusher);
    grid.adjust_to_possible_destination_for_object(
        world.host_object(id).unwrap(), world.host_objects(), start, dest, surfaces, ground,
    )
}
