//! Original component records: View/Display/ControlBar/W3DConvert, source and
//! compiler pins in viewport_cpp_provenance.json. The replay exercises the
//! canonical integer rectangle, not the UI callback event counts in the file.
use super::super::{SessionCamera, continuation::SessionCameraState};
use super::*;
use crate::save_load::camera_codec::CameraCodec;
use serde::Deserialize;

#[derive(Deserialize)]
struct OriginalViewport {
    case: String,
    display: [u32; 2],
    view: [i32; 4],
    aspect_bits: u32,
    viewport_max_bits: [u32; 2],
    viewport_min_bits: [u32; 2],
    center: Option<[i32; 2]>,
    bottom: Option<[i32; 2]>,
    ndc_bits: Option<[u32; 4]>,
}

#[test]
fn integer_viewport_replays_26_executed_original_cpp_records() {
    let records: Vec<OriginalViewport> =
        serde_json::from_str(include_str!("viewport_cpp_records.json")).unwrap();
    assert_eq!(records.len(), 26);
    let mut viewport = TacticalViewport::default();
    for record in records {
        // These are the authored source scenario inputs, not values copied
        // back out of each expected rectangle.
        match record.case.as_str() {
            "default_even" => {
                viewport = TacticalViewport::for_display(800, 600).unwrap();
                viewport.set_height_px(480).unwrap();
            }
            "default_odd" | "resize_preserves_integer_default_ratio" => {
                viewport = TacticalViewport::for_display(801, 603).unwrap();
                viewport.set_height_px(482).unwrap();
                if record.case == "resize_preserves_integer_default_ratio" {
                    viewport.resize_display(1601, 1000).unwrap();
                }
            }
            "hidden_stage_retains_default_height"
            | "hidden_stage_retains_low_height"
            | "hide_missing_parent_nonimmediate_retains_height"
            | "switch_squished_unsupported_source_assert" => {}
            "explicit_hide_expands_after_hidden_stage" | "low_stage_full" => {
                viewport.set_height_px(603).unwrap();
            }
            "show_restores_default_twice"
            | "explicit_show_recomputes_80_percent_after_resize"
            | "replay_show_explicit_height_still_applies" => {
                viewport.set_height_px(482).unwrap();
            }
            "resize_roundtrip_truncates" => viewport.resize_display(801, 603).unwrap(),
            "resize_custom_rect_origin" => {
                viewport = TacticalViewport::for_display(1001, 777).unwrap();
                viewport.set_rect(11, 19, 801, 603).unwrap();
                viewport.resize_display(1603, 1003).unwrap();
            }
            "resize_custom_roundtrip" => viewport.resize_display(1001, 777).unwrap(),
            "negative_origin_truncation" => {
                viewport = TacticalViewport::for_display(801, 603).unwrap();
                viewport.set_rect(-3, -7, 801, 482).unwrap();
            }
            "small_odd_rect" => {
                viewport = TacticalViewport::for_display(5, 3).unwrap();
                viewport.set_rect(2, 3, 5, 3).unwrap();
            }
            "height_two_same_pick_row" => {
                viewport = TacticalViewport::for_display(2, 2).unwrap();
            }
            "height_one_same_pick_row" => {
                viewport = TacticalViewport::for_display(1, 1).unwrap();
            }
            "negative_origin_crosses_zero_whole_expression_truncates" => {
                viewport = TacticalViewport::for_display(1, 1).unwrap();
                viewport.set_rect(-1, -1, 1, 1).unwrap();
            }
            "default_at_one_height_source_zero" => {
                // The source produces a zero-height view and infinite aspect.
                // Safe admission rejects that degenerate result transactionally.
                viewport = TacticalViewport::for_display(1, 1).unwrap();
                let before = viewport;
                assert!(viewport.set_height_px(0).is_err());
                assert_eq!(viewport, before);
                assert_eq!(record.view[1], 0);
                assert_eq!(record.aspect_bits, f32::INFINITY.to_bits());
                assert!(record.center.is_none() && record.bottom.is_none());
                continue;
            }
            "show_missing_parent_retains_height" | "hide_no_window_manager_retains_height" => {
                viewport = TacticalViewport::for_display(801, 603).unwrap();
                viewport.set_height_px(333).unwrap();
            }
            "replay_stage_noop" => {
                viewport = TacticalViewport::for_display(801, 603).unwrap();
            }
            "direct_squished_helper_full_not_switch_admitted" => {
                viewport = TacticalViewport::for_display(803, 607).unwrap();
            }
            "large_finite_resize" => {
                viewport = TacticalViewport::for_display(65535, 65533).unwrap();
                viewport.set_rect(19, 29, 60001, 50003).unwrap();
                viewport.resize_display(32767, 32749).unwrap();
            }
            unknown => panic!("unhandled original scenario: {unknown}"),
        }
        assert_eq!(
            viewport.display_size(),
            (record.display[0], record.display[1]),
            "{}",
            record.case
        );
        assert_eq!(
            viewport.size(),
            (record.view[0], record.view[1]),
            "{}",
            record.case
        );
        assert_eq!(
            viewport.origin(),
            (record.view[2], record.view[3]),
            "{}",
            record.case
        );
        let (min, max) = viewport.normalized_bounds();
        assert_eq!(
            [min.x.to_bits(), min.y.to_bits()],
            record.viewport_min_bits,
            "{}",
            record.case
        );
        assert_eq!(
            [max.x.to_bits(), max.y.to_bits()],
            record.viewport_max_bits,
            "{}",
            record.case
        );
        assert_eq!(
            (viewport.size().0 as f32 / viewport.size().1 as f32).to_bits(),
            record.aspect_bits,
            "{}",
            record.case
        );
        let [center, bottom] = viewport.constraint_pixels();
        assert_eq!(
            [center.0, center.1],
            record.center.unwrap(),
            "{}",
            record.case
        );
        assert_eq!(
            [bottom.0, bottom.1],
            record.bottom.unwrap(),
            "{}",
            record.case
        );
        let center_ndc = viewport.ndc(center.0 as f32, center.1 as f32);
        let bottom_ndc = viewport.ndc(bottom.0 as f32, bottom.1 as f32);
        assert_eq!(
            [
                center_ndc.x.to_bits(),
                center_ndc.y.to_bits(),
                bottom_ndc.x.to_bits(),
                bottom_ndc.y.to_bits()
            ],
            record.ndc_bits.unwrap(),
            "{}",
            record.case,
        );
    }
}

#[test]
fn constraint_length_retains_original_ground_component_for_identical_pick_rows() {
    // W3DView.cpp:490-502 subtracts only center.x/y. The source z remains
    // maxEdgeZ; BaseType.h Coord3D::length includes that third component.
    let viewport = TacticalViewport::for_display(2, 2).unwrap();
    assert_eq!(viewport.constraint_pixels(), [(1, 1), (1, 1)]);
    let view = Mat4::look_at_rh(Vec3::new(0., 300., 300.), Vec3::ZERO, Vec3::Y);
    let projection = Mat4::perspective_rh(50_f32.to_radians(), 1., 1., 2000.);
    assert_eq!(viewport.constraint_inset(view, projection, 80.), 80.);
    assert_eq!(viewport.constraint_inset(view, projection, 0.), 0.);
    assert_eq!(viewport.constraint_inset(view, projection, -20.), 20.);
}

#[test]
fn receiving_viewport_survives_actual_camera_codec_without_pose_or_hag_advance() {
    let mut source = SessionCamera::default();
    source.resize_display(801, 603).unwrap();
    source.set_view_height_px(482).unwrap();
    source.native_camera.initialize_map(80., 300., 37.5, 0.);
    source.camera_yaw_frame = 17;
    source.camera_zoom_frame = 23;
    source.camera_pitch_frame = 31;
    let codec = CameraCodec::<65536>;
    let bytes = codec
        .serialize(&SessionCameraState::capture(&source))
        .unwrap();
    let detached: SessionCameraState = codec.deserialize(&bytes).unwrap();
    detached.validate().unwrap();
    let mut receiving = SessionCamera::default();
    receiving.resize_display(1001, 777).unwrap();
    receiving.viewport.set_rect(11, 19, 801, 603).unwrap();
    receiving.rebuild_projection();
    let viewport = receiving.viewport;
    let receiving_projection = receiving.projection_matrix;
    let peer = SessionCamera::default();
    let peer_bytes = codec
        .serialize(&SessionCameraState::capture(&peer))
        .unwrap();
    detached.admit(&mut receiving);
    assert_eq!(receiving.viewport, viewport);
    assert_eq!(receiving.view_matrix, source.view_matrix);
    assert_eq!(receiving.projection_matrix, source.projection_matrix);
    let hag = codec
        .serialize(&receiving.native_camera.capture_continuation())
        .unwrap();
    // Native host admission rebuilds only the receiving display projection.
    // It does not advance the saved completed pose, axes or HAG.
    receiving.rebuild_projection();
    assert_eq!(receiving.projection_matrix, receiving_projection);
    assert_ne!(receiving.projection_matrix, source.projection_matrix);
    assert_eq!(receiving.view_matrix, source.view_matrix);
    assert_eq!(
        (
            receiving.camera_yaw_frame,
            receiving.camera_zoom_frame,
            receiving.camera_pitch_frame
        ),
        (17, 23, 31)
    );
    assert_eq!(
        codec
            .serialize(&receiving.native_camera.capture_continuation())
            .unwrap(),
        hag
    );
    assert_eq!(
        codec
            .serialize(&SessionCameraState::capture(&peer))
            .unwrap(),
        peer_bytes
    );
    assert_eq!(peer.viewport, TacticalViewport::default());
}

#[test]
fn invalid_receiving_viewport_operations_preserve_owner_pose_projection_and_hag() {
    let mut camera = SessionCamera::default();
    camera.resize_display(801, 603).unwrap();
    camera.set_view_height_px(482).unwrap();
    camera.native_camera.initialize_map(80., 300., 37.5, 0.);
    camera.camera_yaw_frame = 17;
    let codec = CameraCodec::<65536>;
    let before = codec
        .serialize(&SessionCameraState::capture(&camera))
        .unwrap();
    let viewport = camera.viewport;
    assert!(camera.resize_display(801, 0).is_err());
    assert!(camera.set_view_height_px(0).is_err());
    assert!(camera.viewport.set_rect(i32::MAX, 0, 2, 2).is_err());
    assert_eq!(camera.viewport, viewport);
    assert_eq!(
        codec
            .serialize(&SessionCameraState::capture(&camera))
            .unwrap(),
        before
    );
    // Retained integer crop can truncate to zero while shrinking. That failed
    // candidate must not publish a new display extent or projection either.
    camera.set_view_height_px(1).unwrap();
    let before = codec
        .serialize(&SessionCameraState::capture(&camera))
        .unwrap();
    let viewport = camera.viewport;
    assert!(camera.resize_display(1, 1).is_err());
    assert_eq!(camera.viewport, viewport);
    assert_eq!(
        codec
            .serialize(&SessionCameraState::capture(&camera))
            .unwrap(),
        before
    );
}

#[cfg(feature = "game_client")]
#[test]
fn outbound_viewport_observation_cannot_mutate_camera_or_hag_owner() {
    let mut camera = SessionCamera::default();
    camera.resize_display(801, 603).unwrap();
    camera.set_view_height_px(482).unwrap();
    let codec = CameraCodec::<65536>;
    let before = codec
        .serialize(&SessionCameraState::capture(&camera))
        .unwrap();
    let mut observation = game_client::display::view::View::new();
    camera.viewport.observe_in_core_view(&mut observation);
    assert_eq!(
        (
            observation.width(),
            observation.height(),
            observation.origin()
        ),
        (801, 482, (0, 0))
    );
    observation.set_height(777);
    observation.set_origin(19, 29);
    assert_eq!(camera.viewport.size(), (801, 482));
    assert_eq!(camera.viewport.origin(), (0, 0));
    assert_eq!(
        codec
            .serialize(&SessionCameraState::capture(&camera))
            .unwrap(),
        before
    );
    camera.viewport.observe_in_core_view(&mut observation);
    assert_eq!((observation.height(), observation.origin()), (482, (0, 0)));
}
