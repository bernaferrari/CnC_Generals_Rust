//! Actual Object geometry observations for C++ AIStates.cpp:6344-6351.
use super::*;
use crate::ai::states::AIEnterState;
use game_engine::common::system::GeometryType;

fn object(id: ObjectID, position: Coord3D) -> Object {
    // The geometry-only operation needs no body, modules, or admitted world.
    let mut object = Object::new_raw(
        Arc::new(DefaultThingTemplate::new("EnterBorder".into())),
        id,
        ObjectStatusMaskType::none(),
        None,
    );
    object.ambient_lifecycle_admitted = false;
    object.set_prepared_position(&position);
    object
}

fn goal(shape: GeometryType) -> Object {
    let mut goal = object(2, Coord3D::new(100.0, 100.0, 0.0));
    goal.geometry_info.shape.geometry_type = shape;
    goal.geometry_info.set_bounds(crate::common::AABox {
        min: Coord3D::new(-10.0, -2.0, 0.0),
        max: Coord3D::new(10.0, 2.0, 3.0),
    });
    goal
}

#[test]
fn enter_border_box_uses_live_minor_radius_without_template_shape() {
    let goal = goal(GeometryType::Box);
    assert_eq!(goal.get_template_geometry_type(), None);
    let outside_minor = object(1, Coord3D::new(106.0, 100.0, 0.0));
    assert!(!AIEnterState::is_within_goal_border(&outside_minor, &goal));
    let inside_minor = object(1, Coord3D::new(101.0, 100.0, 0.0));
    assert!(AIEnterState::is_within_goal_border(&inside_minor, &goal));
}

#[test]
fn enter_border_round_live_shapes_use_major_radius() {
    let owner = object(1, Coord3D::new(106.0, 100.0, 0.0));
    for shape in [GeometryType::Sphere, GeometryType::Cylinder] {
        assert!(AIEnterState::is_within_goal_border(&owner, &goal(shape)));
    }
}

#[test]
fn enter_border_preserves_strict_squared_xy_comparison() {
    for (shape, radius) in [(GeometryType::Box, 2.0), (GeometryType::Sphere, 10.0)] {
        let goal = goal(shape);
        let exactly_on_border = object(1, Coord3D::new(100.0 + radius, 100.0, 0.0));
        assert!(!AIEnterState::is_within_goal_border(
            &exactly_on_border,
            &goal
        ));
        // The original comparison ignores height; airborne waiting is separate.
        let same_xy = object(1, Coord3D::new(100.0, 100.0, 1000.0));
        assert!(AIEnterState::is_within_goal_border(&same_xy, &goal));
    }
}
