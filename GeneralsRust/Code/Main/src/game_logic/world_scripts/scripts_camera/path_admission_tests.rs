//! Actual receiving GameLogic/TerrainLogic admission, no global path fixture.
use super::*;
use gamelogic::common::Coord3D;
use gamelogic::system::map_loader::{MapData, MapWaypoint};

fn waypoint(id: u32, name: &str, x: f32) -> MapWaypoint {
    MapWaypoint {
        id,
        name: name.into(),
        location: Coord3D::new(x, 0., -999.),
        path_label1: String::new(),
        path_label2: String::new(),
        path_label3: String::new(),
        bi_directional: false,
    }
}
fn owner(nodes: Vec<MapWaypoint>, links: Vec<(u32, u32)>) -> GameLogic {
    let world = GameLogic::new();
    let mut map = MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = (0..64 * 64).map(|index| (index % 64) as u8).collect();
    map.waypoints = nodes;
    map.waypoint_links = links;
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    world
}
fn admitted(world: &GameLogic, name: &str) -> CameraChain {
    admit_camera_chain(world, name, Vec3::ZERO)
        .unwrap()
        .unwrap()
}
#[test]
fn receiving_duplicate_name_uses_first_raw_roster_match_and_keeps_peer_unchanged() {
    let a = owner(
        vec![waypoint(1, "Same", 100.), waypoint(2, "Same", 180.)],
        vec![],
    );
    let b = owner(
        vec![waypoint(1, "Same", 300.), waypoint(2, "Same", 380.)],
        vec![],
    );
    let before = b.capture_camera_continuation().unwrap();
    // Terrain admission prepends map entries, so ID2 is the raw first match.
    assert_eq!(
        a.world_services
            .terrain()
            .read()
            .unwrap()
            .get_first_waypoint()
            .unwrap()
            .get_id(),
        2
    );
    assert_eq!(admitted(&a, "Same").points[0].x, 180.);
    assert_eq!(admitted(&b, "Same").points[0].x, 380.);
    assert!(
        admit_camera_chain(&a, "same", Vec3::ZERO)
            .unwrap()
            .is_none()
    );
    assert!(
        admit_camera_chain(&a, "Missing", Vec3::ZERO)
            .unwrap()
            .is_none()
    );
    assert_eq!(b.capture_camera_continuation().unwrap(), before);
}
#[test]
fn short_nodes_do_not_spend_the_accepted_waypoint_budget() {
    let world = owner(
        (1..=260)
            .map(|id| waypoint(id, if id == 1 { "Start" } else { "Node" }, id as f32))
            .collect(),
        (1..260).map(|id| (id, id + 1)).collect(),
    );
    let chain = admitted(&world, "Start");
    let step = gamelogic::common::MAP_XY_FACTOR;
    assert!(
        chain.points.len() > 25,
        "raw short hops must not consume accepted slots"
    );
    assert_eq!(chain.points.last().unwrap().x, 24. * step);
    assert_eq!(chain.points.len(), (24. * step) as usize);
}
#[test]
fn progressing_ring_revisits_nodes_until_the_accepted_slot_cap() {
    let world = owner(
        vec![waypoint(1, "Ring", 20.), waypoint(2, "Other", 40.)],
        vec![(1, 2), (2, 1)],
    );
    let chain = admitted(&world, "Ring");
    assert_eq!(chain.points.len(), 24);
    assert_eq!(chain.points.last().unwrap().x, 40.);
    assert_eq!(chain.points[0].x, chain.points[2].x);
}
#[test]
fn nonprogressing_ring_is_an_error_and_resolver_never_cancels_live_state() {
    let mut world = owner(
        vec![waypoint(1, "Ring", 1.), waypoint(2, "Other", 2.)],
        vec![(1, 2), (2, 1)],
    );
    world.session_view.camera.camera_yaw_target = Some(1.);
    world.session_view.camera.rotation =
        crate::presentation_frame::camera::rotation::RotationTiming::historical(
            3.,
            world.session_view.camera.rotation.ease_in(),
            world.session_view.camera.rotation.ease_out(),
            world.session_view.camera.rotation.time,
        );
    world.session_view.camera.camera_zoom_target = Some(0.8);
    world.session_view.camera.camera_zoom_duration = 5.;
    world.session_view.scripted.script_camera_freeze_time = true;
    world.session_view.camera.camera_yaw_radians = 0.7;
    let before = world.capture_camera_continuation().unwrap();
    let result = admit_camera_chain(&world, "Ring", Vec3::ZERO);
    assert!(result.is_err());
    assert!(result.err().unwrap().contains("no admission progress"));
    assert_eq!(world.capture_camera_continuation().unwrap(), before);
    assert_eq!(world.session_view.camera.camera_yaw_radians, 0.7);
}
#[test]
fn final_ground_uses_last_admitted_cap_point_not_next_raw_node() {
    let world = owner(
        (1..=30)
            .map(|id| waypoint(id, if id == 1 { "Start" } else { "Node" }, 20. * id as f32))
            .collect(),
        (1..30).map(|id| (id, id + 1)).collect(),
    );
    let chain = admitted(&world, "Start");
    assert_eq!(chain.points.len(), 24);
    assert_eq!(chain.points.last().unwrap().x, 480.);
    assert_eq!(chain.final_ground, 30.); // grid column48 * 0.625
}
#[test]
fn final_ground_follows_short_terminal_pushback() {
    // Keep this point inside the source interpolation region. At map Y=0,
    // BaseHeightMap deliberately returns the integer edge sample, which would
    // not distinguish the short terminal X=41 from the prior X=40 slot.
    let mut nodes = vec![
        waypoint(1, "Start", 20.),
        waypoint(2, "Middle", 40.),
        waypoint(3, "End", 41.),
    ];
    for node in &mut nodes {
        node.location.y = 20.;
    }
    let world = owner(nodes, vec![(1, 2), (2, 3)]);
    let chain = admitted(&world, "Start");
    assert_eq!(chain.points.len(), 3);
    assert_eq!(chain.points.last().unwrap().x, 41.);
    let expected = world
        .world_services
        .terrain()
        .read()
        .unwrap()
        .get_ground_height(41., 20., None);
    assert_eq!(chain.final_ground, expected);
    assert!((chain.final_ground - 2.5625).abs() < 1e-5);
}
