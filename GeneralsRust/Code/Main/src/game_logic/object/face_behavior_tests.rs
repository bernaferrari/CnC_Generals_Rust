//! C++ AIStates.cpp:7398-7470 Face behavior on the production Object owner.
//! These checks replace the alternate AI facade's registry-backed captures.

use super::*;

fn face_object(id: u32, min_speed: f32) -> Object {
    let mut template = ThingTemplate::new("FaceBehavior");
    template.add_kind_of(KindOf::Vehicle);
    let mut object = Object::new(template, ObjectId(id), Team::USA);
    object.set_position(glam::Vec3::ZERO);
    object.set_orientation(0.0);
    object.min_speed = min_speed;
    object.face_can_turn_in_place = min_speed == 0.0;
    object.face_active = true;
    object
}

#[test]
fn face_goal_selects_orientation_or_explicit_position_without_integrating() {
    let target = glam::Vec3::new(0.0, 0.0, 12.0);
    for (id, min_speed, expected) in [
        (1, 0.0, LocoGoalType::Angle),
        (2, 1.0, LocoGoalType::PositionExplicit),
    ] {
        let mut object = face_object(id, min_speed);
        let position = object.get_position();
        let orientation = object.get_orientation();
        let relative = object.relative_angle_2d_to(target);
        assert!(object.arm_face_locomotor_goal(target));
        assert_eq!(object.locomotor_goal_type, expected);
        assert_eq!(object.get_position(), position);
        assert_eq!(object.get_orientation(), orientation);
        if expected == LocoGoalType::Angle {
            assert_eq!(object.locomotor_goal_angle, orientation + relative);
            assert_eq!(object.movement.target_position, None);
        } else {
            assert_eq!(object.movement.target_position, Some(target));
            assert_eq!(object.locomotor_goal_angle, 0.0);
        }
    }
}

#[test]
fn face_within_threshold_completes_without_a_turn_or_move_goal() {
    for min_speed in [0.0, 1.0] {
        let mut object = face_object(3, min_speed);
        assert!(!object.arm_face_locomotor_goal(glam::Vec3::new(1.0, 0.0, 0.01)));
        assert!(!object.face_active);
        assert_eq!(object.locomotor_goal_type, LocoGoalType::None);
        assert_eq!(object.movement.target_position, None);
        assert_eq!(object.get_orientation(), 0.0);
        assert_eq!(object.get_position(), glam::Vec3::ZERO);
    }
}

#[test]
fn face_relative_angle_wraps_across_pi_in_both_directions() {
    let mut object = face_object(4, 0.0);
    for (orientation, target_z) in [(3.13, 0.01), (-3.13, -0.01)] {
        object.set_orientation(orientation);
        let target = glam::Vec3::new(-1.0, 0.0, target_z);
        let relative = object.relative_angle_2d_to(target);
        assert!(relative.abs() < 0.1, "wrapped relative angle: {relative}");
        assert!(!object.arm_face_locomotor_goal(target));
        assert_eq!(object.get_orientation(), orientation);
    }
}

#[test]
fn face_relative_angle_respects_cpp_two_degree_boundary() {
    let mut object = face_object(5, 0.0);
    let near = glam::Vec3::new(1.0, 0.0, 0.03);
    let far = glam::Vec3::new(1.0, 0.0, 0.05);
    assert!(object.relative_angle_2d_to(near).abs() < FACE_REL_THRESH_RAD);
    assert!(!object.arm_face_locomotor_goal(near));
    assert_eq!(object.locomotor_goal_type, LocoGoalType::None);
    object.face_active = true;
    assert!(object.relative_angle_2d_to(far).abs() > FACE_REL_THRESH_RAD);
    assert!(object.arm_face_locomotor_goal(far));
    assert_eq!(object.locomotor_goal_type, LocoGoalType::Angle);
    assert!(object.face_active);
    assert_eq!(object.get_orientation(), 0.0);
}
