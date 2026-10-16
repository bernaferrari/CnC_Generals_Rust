use super::*;
use crate::presentation_frame::camera::continuation::SessionCameraState;

fn records() -> Vec<serde_json::Value> {
    serde_json::from_str(include_str!("radar_view_cpp_records.json")).unwrap()
}
fn camera(record: &serde_json::Value) -> RadarCamera {
    let mut viewport = TacticalViewport::default();
    viewport
        .set_rect(
            record["origin"][0].as_i64().unwrap() as i32,
            record["origin"][1].as_i64().unwrap() as i32,
            record["size"][0].as_i64().unwrap() as i32,
            record["size"][1].as_i64().unwrap() as i32,
        )
        .unwrap();
    let mut columns = [[0.; 4]; 4];
    for (column, values) in columns.iter_mut().zip(
        record["inverse_view_projection_source_columns"]
            .as_array()
            .unwrap(),
    ) {
        for (component, value) in column.iter_mut().zip(values.as_array().unwrap()) {
            *component = value.as_f64().unwrap() as f32;
        }
    }
    let source_inverse = Mat4::from_cols_array_2d(&columns);
    let axis = Mat4::from_cols(glam::Vec4::X, glam::Vec4::Z, glam::Vec4::Y, glam::Vec4::W);
    let host_inverse = axis * source_inverse;
    RadarCamera {
        viewport,
        view: host_inverse.inverse(),
        projection: Mat4::IDENTITY,
        terrain: record["terrain"].as_f64().unwrap() as f32,
    }
}

#[test]
fn radar_corners_match_original_cpp_corner_and_plane_components() {
    let records = records();
    assert_eq!(records.len(), 4);
    for (case, record) in records.iter().enumerate() {
        let actual = camera(record).corners().unwrap();
        for (index, (actual, expected)) in actual
            .iter()
            .zip(record["corners"].as_array().unwrap())
            .enumerate()
        {
            // Oracle output is original XYZ; the production owner stores XZY.
            let expected = Vec3::new(
                expected[0].as_f64().unwrap() as f32,
                expected[2].as_f64().unwrap() as f32,
                expected[1].as_f64().unwrap() as f32,
            );
            for (actual, expected) in actual.to_array().into_iter().zip(expected.to_array()) {
                // Affine getPickRay scaffolding is replayed through a real matrix
                // inverse/unprojection here, so retain a bounded f32 inverse error.
                assert!(
                    (actual - expected).abs() <= 3e-5 * expected.abs().max(1.),
                    "case{case} corner{index}: {actual} versus{expected}"
                );
            }
        }
    }
}

#[test]
fn invalid_receiving_radar_projection_never_selects_a_fallback() {
    let valid = camera(&records()[0]);
    assert!(
        valid
            .with_matrices(Mat4::ZERO, Mat4::IDENTITY)
            .corners()
            .is_none()
    );
    assert!(
        valid
            .with_matrices(Mat4::IDENTITY, Mat4::ZERO)
            .corners()
            .is_none()
    );
    let parallel = RadarCamera {
        view: Mat4::IDENTITY,
        projection: Mat4::IDENTITY,
        ..valid
    };
    assert!(parallel.corners().is_none());
    let nonfinite = RadarCamera {
        terrain: f32::NAN,
        ..valid
    };
    assert!(nonfinite.corners().is_none());
}

#[test]
fn late_filter_matrices_replace_only_the_draw_observation_without_advancing_owner() {
    let mut owner = SessionView::default();
    owner.target = Vec3::new(20., 37., 40.);
    let first = camera(&records()[0]);
    owner.camera.viewport = first.viewport;
    owner.camera.view_matrix = first.view;
    owner.camera.projection_matrix = first.projection;
    let before = serde_json::to_vec(&SessionCameraState::capture(&owner.camera)).unwrap();
    let target = owner.target;
    let captured = RadarCamera::capture(&owner, first.terrain);
    let expected_before = captured.corners().unwrap();
    let second = camera(&records()[1]);
    let late = captured.with_matrices(second.view, second.projection);
    assert_ne!(late.corners().unwrap(), expected_before);
    assert_eq!(captured.corners().unwrap(), expected_before);
    let late_corners = late.corners().unwrap();
    for _ in 0..120 {
        assert_eq!(late.corners().unwrap(), late_corners);
    }
    assert_eq!(owner.target, target);
    assert_eq!(
        serde_json::to_vec(&SessionCameraState::capture(&owner.camera)).unwrap(),
        before
    );
    assert_eq!(late.viewport, first.viewport);
    assert_eq!(late.terrain, first.terrain);
}
