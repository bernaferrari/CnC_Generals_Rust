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

/// C++ FloatUpdate.cpp:106-114 writes the sway into the Drawable's instance
/// matrix (keep its Z rotation, then Rotate_Y(yaw), Rotate_X(pitch)); the
/// Drawable transform is Object * instance (Drawable.cpp:2639-2647).
fn sway_instance(input: &UnitRenderInput) -> glam::Mat4 {
    crate::game_logic::host_float_update::rebuild_receiving_instance(
        glam::Mat4::IDENTITY,
        input.float_yaw,
        input.float_pitch,
    )
}

#[test]
fn frozen_sway_survives_same_id_other_world_freeze_and_reset() {
    let mut first = boat_world(0.3, -0.1);
    let frame = PresentationFrame::build_from_logic(&mut first, 1);
    let input = boat_input(&frame);
    assert_eq!((input.float_yaw, input.float_pitch), (0.3, -0.1));
    let expected = input.world_matrix_with_drawable_instance(Some(sway_instance(&input)));
    let mut second = boat_world(-0.8, 0.2);
    let other_frame = PresentationFrame::build_from_logic(&mut second, 1);
    let other_input = boat_input(&other_frame);
    assert_eq!((other_input.float_yaw, other_input.float_pitch), (-0.8, 0.2));
    assert_ne!(
        other_input.world_matrix_with_drawable_instance(Some(sway_instance(&other_input))),
        expected
    );
    assert_eq!(
        (input.float_yaw, input.float_pitch),
        (0.3, -0.1),
        "same-ID other-world freeze must not replace a completed input sway"
    );
    second.reset();
    assert_eq!(
        input.world_matrix_with_drawable_instance(Some(sway_instance(&input))),
        expected,
        "resetting another world must not clear a completed input sway"
    );
}

#[test]
fn frozen_sway_uses_cpp_heading_yaw_pitch_order() {
    let mut world = boat_world(0.3, -0.1);
    let frame = PresentationFrame::build_from_logic(&mut world, 1);
    let input = boat_input(&frame);
    let instance = sway_instance(&input);
    // In C++ Z-up space the instance is Ry(yaw) * Rx(pitch) after the kept
    // Z rotation (identity here). Analytic basis vectors distinguish the two
    // noncommuting rotations without reading globals.
    let basis = glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W);
    let cpp = basis * instance * basis;
    let (sy, cy) = 0.3f32.sin_cos();
    let (sp, cp) = (-0.1f32).sin_cos();
    assert!(cpp.x_axis.truncate().abs_diff_eq(glam::Vec3::new(cy, 0.0, -sy), 1e-6));
    assert!(cpp.y_axis.truncate().abs_diff_eq(glam::Vec3::new(sy * sp, cp, cy * sp), 1e-6));
    // The heading is the Object transform, applied before the instance.
    let matrix = input.world_matrix_with_drawable_instance(Some(instance));
    let object = glam::Mat4::from_translation(input.position)
        * glam::Mat4::from_rotation_y(input.orientation);
    assert!(matrix.abs_diff_eq(object * instance, 1e-6));
    assert_eq!(matrix.w_axis.truncate(), input.position);
}

#[test]
fn zero_sway_and_serialized_frozen_pose_keep_defaults() {
    let mut world = boat_world(0.0, 0.0);
    let frame = PresentationFrame::build_from_logic(&mut world, 1);
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
    let mut nonzero = boat_world(0.25, -0.15);
    let frame = PresentationFrame::build_from_logic(&mut nonzero, 1);
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
