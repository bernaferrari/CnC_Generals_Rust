//! Terrain ownership checks for TEAM_WANDER and TEAM_PANIC waypoint lookup.
use super::GameLogic;
use gamelogic::system::map_loader::{Coord3D, MapData, MapWaypoint};
use glam::Vec3;
use std::sync::Arc;

fn map_with_path(
    first_id: u32,
    first_xy: (f32, f32),
    terminal_xy: (f32, f32),
    path_label: &str,
    terminal_label: &str,
) -> MapData {
    let mut data = MapData::new();
    data.width = 64;
    data.height = 64;
    data.heightmap = vec![0; 64 * 64];
    data.waypoints.extend([
        MapWaypoint {
            id: first_id,
            name: format!("{path_label}Start"),
            location: Coord3D::new(first_xy.0, first_xy.1, 0.0),
            path_label1: path_label.into(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        },
        MapWaypoint {
            id: first_id + 1,
            name: format!("{terminal_label}End"),
            location: Coord3D::new(terminal_xy.0, terminal_xy.1, 0.0),
            path_label1: terminal_label.into(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        },
    ]);
    data.waypoint_links.push((first_id, first_id + 1));
    data
}

fn load_owned_map(logic: &GameLogic, data: MapData) {
    logic
        .world_services
        .terrain()
        .write()
        .expect("owned terrain")
        .load_map_data(data);
}

#[test]
fn wander_and_panic_waypoint_lookup_use_the_driving_world_terrain() {
    let owner = GameLogic::new();
    load_owned_map(
        &owner,
        map_with_path(
            71_001,
            (10.0, 20.0),
            (40.0, 20.0),
            "SharedPath",
            "OwnerTail",
        ),
    );

    let mut foreign = GameLogic::new();
    load_owned_map(
        &foreign,
        map_with_path(
            72_001,
            (300.0, 400.0),
            (900.0, 400.0),
            "SharedPath",
            "ForeignTail",
        ),
    );

    let owner_services = Arc::clone(&owner.world_services);
    let foreign_services = Arc::clone(&foreign.world_services);
    assert!(!Arc::ptr_eq(&owner_services, &foreign_services));

    // Reproduce the dangerous ambient selection explicitly: while the foreign
    // world is scoped, the legacy terrain adapter resolves its same-label path.
    gamelogic::system::engine_stores::with_world_services(&foreign_services, || {
        let ambient = gamelogic::terrain::get_terrain_logic();
        assert!(Arc::ptr_eq(&ambient, foreign.world_services.terrain()));
        let ambient_id = ambient
            .read()
            .expect("foreign terrain")
            .get_closest_waypoint_on_path(
                &gamelogic::common::Coord3D::new(0.0, 0.0, 0.0),
                "SharedPath",
            )
            .map(|waypoint| waypoint.get_id())
            .expect("foreign same-label path");
        assert_eq!(ambient_id, 72_001);

        let wander = owner
            .host_wander_waypoint_path_from("SharedPath", Vec3::ZERO)
            .expect("owner wander path");
        assert_eq!(wander.len(), 2);
        assert_eq!(wander[0].x, 10.0);
        assert_eq!(wander[0].z, 20.0);
        assert_eq!(wander[1].x, 40.0);
        assert_eq!(wander[1].z, 20.0);
        // The continuation is followed through link[0], even though the
        // terminal waypoint has a distinct path label.
        assert_eq!(
            owner
                .world_services
                .terrain()
                .read()
                .expect("owner terrain")
                .get_waypoint_by_id(71_002)
                .expect("owner terminal")
                .get_path_label1()
                .as_str(),
            "OwnerTail"
        );

        let panic = owner
            .host_panic_waypoint_from("SharedPath", Vec3::ZERO)
            .expect("owner panic waypoint");
        assert_eq!(panic.0, 71_001);
        assert_eq!(panic.1.x, 10.0);
        assert_eq!(panic.1.z, 20.0);
        assert!(panic.2, "link[0] continuation exists");
        assert_eq!(panic.3, 1.0 + 30.0);
    });

    // The scoped foreign owner has been restored; the owner's separate terrain
    // still contains its own path and terminal extent.
    let after_scope = owner
        .host_wander_waypoint_path_from("SharedPath", Vec3::ZERO)
        .expect("owner path after foreign scope");
    assert_eq!(after_scope[0].x, 10.0);
    assert_eq!(after_scope[1].x, 40.0);
    foreign.reset();
    drop(foreign);
    let after_foreign_reset = owner
        .host_wander_waypoint_path_from("SharedPath", Vec3::ZERO)
        .expect("owner path after foreign reset/drop");
    assert_eq!(after_foreign_reset, after_scope);
}
