//! C++ TerrainLogic closest-path waypoint query regressions using locally owned terrain.

use super::*;

fn add_path_waypoint(
    terrain: &mut TerrainLogic,
    id: WaypointID,
    position: Coord3D,
    labels: [&str; 3],
) {
    terrain.add_waypoint_from_map(&MapWaypoint {
        id,
        name: format!("Waypoint{id}"),
        location: crate::system::map_loader::Coord3D::new(position.x, position.y, position.z),
        path_label1: labels[0].to_string(),
        path_label2: labels[1].to_string(),
        path_label3: labels[2].to_string(),
        bi_directional: false,
    });
}

#[test]
fn closest_waypoint_path_rejects_empty_label() {
    let mut terrain = TerrainLogic::new();
    add_path_waypoint(
        &mut terrain,
        1,
        Coord3D::new(2.0, 0.0, 0.0),
        ["Route", "", ""],
    );
    add_path_waypoint(&mut terrain, 2, Coord3D::new(0.0, 0.0, 0.0), ["", "", ""]);
    let origin = Coord3D::new(0.0, 0.0, 0.0);

    assert!(terrain.get_closest_waypoint_on_path(&origin, "").is_none());
    assert_eq!(
        terrain
            .get_closest_waypoint_on_path(&origin, "rOuTe")
            .map(Waypoint::get_id),
        Some(1)
    );
}

#[test]
fn closest_waypoint_path_matches_all_slots_without_trimming() {
    let mut terrain = TerrainLogic::new();
    add_path_waypoint(
        &mut terrain,
        3,
        Coord3D::new(2.0, 0.0, 0.0),
        ["Primary", "Secondary", "Tertiary"],
    );
    add_path_waypoint(&mut terrain, 4, Coord3D::new(0.0, 0.0, 0.0), [" ", "", ""]);
    let origin = Coord3D::new(0.0, 0.0, 0.0);

    for label in ["pRiMaRy", "sEcOnDaRy", "tErTiArY"] {
        assert_eq!(
            terrain
                .get_closest_waypoint_on_path(&origin, label)
                .map(Waypoint::get_id),
            Some(3)
        );
    }
    assert_eq!(
        terrain
            .get_closest_waypoint_on_path(&origin, " ")
            .map(Waypoint::get_id),
        Some(4)
    );
    assert!(
        terrain
            .get_closest_waypoint_on_path(&origin, " Primary ")
            .is_none()
    );
    assert!(
        terrain
            .get_closest_waypoint_on_path(&origin, "Missing")
            .is_none()
    );
}

#[test]
fn closest_waypoint_path_uses_xy_distance() {
    let mut terrain = TerrainLogic::new();
    add_path_waypoint(
        &mut terrain,
        5,
        Coord3D::new(2.0, 0.0, 0.0),
        ["Route", "", ""],
    );
    add_path_waypoint(
        &mut terrain,
        6,
        Coord3D::new(1.0, 0.0, 0.0),
        ["Route", "", ""],
    );
    // Map admission grounds Z; change it explicitly to distinguish XY from XYZ distance.
    terrain
        .get_waypoint_by_id_mut(6)
        .expect("near waypoint")
        .set_location_z(1000.0);

    assert_eq!(
        terrain
            .get_closest_waypoint_on_path(&Coord3D::new(0.0, 0.0, 0.0), "Route")
            .map(Waypoint::get_id),
        Some(6)
    );
}

#[test]
fn closest_waypoint_path_keeps_head_on_ties_and_replaces_it_when_closer() {
    let mut terrain = TerrainLogic::new();
    add_path_waypoint(
        &mut terrain,
        7,
        Coord3D::new(4.0, 0.0, 0.0),
        ["Route", "", ""],
    );
    add_path_waypoint(
        &mut terrain,
        8,
        Coord3D::new(2.0, 0.0, 0.0),
        ["Route", "", ""],
    );
    add_path_waypoint(
        &mut terrain,
        9,
        Coord3D::new(2.0, 0.0, 0.0),
        ["Route", "", ""],
    );

    assert_eq!(
        terrain
            .get_closest_waypoint_on_path(&Coord3D::new(0.0, 0.0, 0.0), "Route")
            .map(Waypoint::get_id),
        Some(9)
    );
    assert_eq!(
        terrain
            .get_closest_waypoint_on_path(&Coord3D::new(10.0, 0.0, 0.0), "Route")
            .map(Waypoint::get_id),
        Some(7)
    );
}

#[test]
fn closest_waypoint_path_admits_first_match_at_nonfinite_distance() {
    for x in [f32::MAX, f32::INFINITY, f32::NAN] {
        let mut terrain = TerrainLogic::new();
        add_path_waypoint(
            &mut terrain,
            10,
            Coord3D::new(x, 0.0, 0.0),
            ["Route", "", ""],
        );
        // The first list entry does not match; admission must use the first match.
        add_path_waypoint(
            &mut terrain,
            11,
            Coord3D::new(0.0, 0.0, 0.0),
            ["Other", "", ""],
        );

        assert_eq!(
            terrain
                .get_closest_waypoint_on_path(&Coord3D::new(0.0, 0.0, 0.0), "Route")
                .map(Waypoint::get_id),
            Some(10)
        );
    }
}

#[test]
fn closest_waypoint_path_replaces_first_infinity_with_later_finite_match() {
    let mut terrain = TerrainLogic::new();
    add_path_waypoint(
        &mut terrain,
        12,
        Coord3D::new(1.0, 0.0, 0.0),
        ["Route", "", ""],
    );
    add_path_waypoint(
        &mut terrain,
        13,
        Coord3D::new(f32::INFINITY, 0.0, 0.0),
        ["Route", "", ""],
    );

    assert_eq!(
        terrain
            .get_closest_waypoint_on_path(&Coord3D::new(0.0, 0.0, 0.0), "Route")
            .map(Waypoint::get_id),
        Some(12)
    );
}

#[test]
fn closest_waypoint_path_preserves_strict_nan_comparison_order() {
    let origin = Coord3D::new(0.0, 0.0, 0.0);
    let mut first_nan = TerrainLogic::new();
    add_path_waypoint(
        &mut first_nan,
        14,
        Coord3D::new(1.0, 0.0, 0.0),
        ["Route", "", ""],
    );
    add_path_waypoint(
        &mut first_nan,
        15,
        Coord3D::new(f32::NAN, 0.0, 0.0),
        ["Route", "", ""],
    );
    assert_eq!(
        first_nan
            .get_closest_waypoint_on_path(&origin, "Route")
            .map(Waypoint::get_id),
        Some(15)
    );

    let mut later_nan = TerrainLogic::new();
    add_path_waypoint(
        &mut later_nan,
        16,
        Coord3D::new(f32::NAN, 0.0, 0.0),
        ["Route", "", ""],
    );
    add_path_waypoint(
        &mut later_nan,
        17,
        Coord3D::new(1.0, 0.0, 0.0),
        ["Route", "", ""],
    );
    assert_eq!(
        later_nan
            .get_closest_waypoint_on_path(&origin, "Route")
            .map(Waypoint::get_id),
        Some(17)
    );
}

#[test]
fn closest_waypoint_path_keeps_first_match_for_nonfinite_query() {
    let mut terrain = TerrainLogic::new();
    add_path_waypoint(
        &mut terrain,
        18,
        Coord3D::new(1.0, 0.0, 0.0),
        ["Route", "", ""],
    );
    add_path_waypoint(
        &mut terrain,
        19,
        Coord3D::new(2.0, 0.0, 0.0),
        ["Route", "", ""],
    );

    for x in [f32::NAN, f32::INFINITY] {
        assert_eq!(
            terrain
                .get_closest_waypoint_on_path(&Coord3D::new(x, 0.0, 0.0), "Route")
                .map(Waypoint::get_id),
            Some(19)
        );
    }
}

#[test]
fn closest_waypoint_path_admits_exact_max_distance() {
    let x = f32::from_bits(0x5f7f_ffff);
    let y = f32::from_bits(0x5980_0000);
    assert_eq!((x * x + y * y).to_bits(), f32::MAX.to_bits());
    let mut terrain = TerrainLogic::new();
    add_path_waypoint(&mut terrain, 20, Coord3D::new(x, y, 0.0), ["Route", "", ""]);

    assert_eq!(
        terrain
            .get_closest_waypoint_on_path(&Coord3D::new(0.0, 0.0, 0.0), "Route")
            .map(Waypoint::get_id),
        Some(20)
    );
}
