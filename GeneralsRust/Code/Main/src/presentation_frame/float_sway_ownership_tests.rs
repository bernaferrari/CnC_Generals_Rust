//! Production frame/input regression for FloatUpdate instance ownership.
use super::*;
use crate::game_logic::host_float_update::HostFloatUpdateData;
use crate::game_logic::{GameLogic, Object, ObjectId, Team, ThingTemplate};

fn boat_world(yaw: f32, pitch: f32) -> GameLogic {
    let mut world = GameLogic::new();
    let mut object = Object::new(ThingTemplate::new("FrozenBoat"), ObjectId(77), Team::USA);
    object.float_update = Some(HostFloatUpdateData {
        yaw,
        pitch,
        ..Default::default()
    });
    object.set_position(glam::Vec3::new(10.0, 3.0, 20.0));
    object.set_orientation(0.6);
    world.add_object(object);
    world
}

fn boat_input(frame: &PresentationFrame) -> UnitRenderInput {
    UnitRenderInput::from_renderable(
        frame
            .objects
            .iter()
            .find(|object| object.id == ObjectId(77))
            .unwrap(),
    )
}

#[test]
fn frozen_sway_survives_same_id_other_world_freeze_and_reset() {
    let first = boat_world(0.3, -0.1);
    let frame = PresentationFrame::build_from_logic(&first, 1);
    let input = boat_input(&frame);
    let expected = input.world_matrix();
    let mut second = boat_world(-0.8, 0.2);
    let other_frame = PresentationFrame::build_from_logic(&second, 1);
    let other_input = boat_input(&other_frame);
    assert_ne!(other_input.world_matrix(), expected);
    assert_eq!(
        input.world_matrix(),
        expected,
        "same-ID other-world freeze must not replace a completed input sway"
    );
    second.reset();
    assert_eq!(
        input.world_matrix(),
        expected,
        "resetting another world must not clear a completed input sway"
    );
}

#[test]
fn frozen_sway_uses_cpp_heading_yaw_pitch_order() {
    let world = boat_world(0.3, -0.1);
    let frame = PresentationFrame::build_from_logic(&world, 1);
    let input = boat_input(&frame);
    let matrix = input.world_matrix();
    let (sh, ch) = 0.6f32.sin_cos();
    let (sy, cy) = 0.3f32.sin_cos();
    let (sp, cp) = (-0.1f32).sin_cos();
    // C++ FloatUpdate.cpp:109-112: heading -> yaw -> pitch. In host Y-up
    // that is Ry(heading) * Rz(yaw) * Rx(pitch). These analytic basis vectors
    // distinguish all three noncommuting rotations without reading globals.
    let expected_x = glam::Vec3::new(ch * cy, sy, -sh * cy);
    let expected_y = glam::Vec3::new(-ch * sy * cp + sh * sp, cy * cp, sh * sy * cp + ch * sp);
    assert!(matrix.x_axis.truncate().abs_diff_eq(expected_x, 1e-6));
    assert!(matrix.y_axis.truncate().abs_diff_eq(expected_y, 1e-6));
    assert_eq!(matrix.w_axis.truncate(), input.position);
}

#[test]
fn zero_sway_and_serialized_frozen_pose_keep_defaults() {
    let world = boat_world(0.0, 0.0);
    let frame = PresentationFrame::build_from_logic(&world, 1);
    let input = boat_input(&frame);
    let expected = glam::Mat4::from_translation(input.position)
        * glam::Mat4::from_rotation_y(input.orientation);
    assert_eq!(input.world_matrix(), expected);
    let default_data = HostFloatUpdateData::default();
    assert!(
        !default_data.enabled,
        "C++ FloatUpdateModuleData ctor defaults disabled"
    );
    assert_eq!((default_data.yaw, default_data.pitch), (0.0, 0.0));
    let nonzero = boat_world(0.25, -0.15);
    let frame = PresentationFrame::build_from_logic(&nonzero, 1);
    let input = boat_input(&frame);
    let object = frame
        .objects
        .iter()
        .find(|object| object.id == input.id)
        .unwrap();
    let bytes = serde_json::to_vec(object).unwrap();
    let restored: RenderableObject = serde_json::from_slice(&bytes).unwrap();
    let restored_input = UnitRenderInput::from_renderable(&restored);
    assert_eq!(restored_input.world_matrix(), input.world_matrix());
    assert_eq!((restored.float_yaw, restored.float_pitch), (0.25, -0.15));
}
