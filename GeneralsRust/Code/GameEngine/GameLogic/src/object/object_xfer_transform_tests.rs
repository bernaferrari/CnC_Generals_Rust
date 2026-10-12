//! Original Object.cpp transform phase. No registry or shared clock fixture.
use super::*;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

#[test]
fn versions_one_through_six_consume_position_then_orientation_without_matrix_header() {
    let position = Coord3D::new(10.0, -20.0, 30.0);
    let angle = 0.75_f32;
    let sentinel = 0x7654_3210_u32;
    let mut bytes = Vec::new();
    for value in [position.x, position.y, position.z, angle] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&sentinel.to_le_bytes());
    for version in 1..=6 {
        let mut xfer = XferLoad::new(Cursor::new(&bytes), 1);
        let mut observed = Vec::new();
        xfer_transform_phase(&mut xfer, version, Matrix3D::IDENTITY, &mut |change| {
            match change {
                SavedTransformChange::Position(value) => {
                    assert_eq!(value, position);
                    assert!(observed.is_empty());
                    observed.push("position");
                }
                SavedTransformChange::Orientation(value) => {
                    assert_eq!(value, angle);
                    assert_eq!(observed, ["position"]);
                    observed.push("orientation");
                }
                SavedTransformChange::Matrix(_) => panic!("pre-v7 has no matrix record"),
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(observed, ["position", "orientation"]);
        assert_eq!(xfer.bytes_read(), 16);
        let mut following = 0;
        xfer.xfer_unsigned_int(&mut following).unwrap();
        assert_eq!(following, sentinel);
    }
}

#[test]
fn current_matrix_record_keeps_full_rows_and_one_observation() {
    let matrix = Matrix3D::from_translation(Coord3D::new(10.0, -20.0, 30.0))
        * Matrix3D::from_rotation_z(0.75);
    let mut bytes = Vec::new();
    xfer_transform_phase(
        &mut XferSave::new(Cursor::new(&mut bytes), 1),
        9,
        matrix,
        &mut |change| {
            assert!(matches!(change, SavedTransformChange::Matrix(_)));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(bytes.len(), 49);
    assert_eq!(bytes[0], 1);
    let mut calls = 0;
    xfer_transform_phase(
        &mut XferLoad::new(Cursor::new(&bytes), 1),
        9,
        Matrix3D::IDENTITY,
        &mut |change| {
            let SavedTransformChange::Matrix(value) = change else {
                panic!("current matrix")
            };
            assert_eq!(value, matrix);
            calls += 1;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(calls, 1);
}

#[test]
fn truncated_transform_stops_before_unread_phase() {
    let mut bytes = Vec::new();
    for value in [10.0_f32, 20.0, 30.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let mut observed = 0;
    assert!(
        xfer_transform_phase(
            &mut XferLoad::new(Cursor::new(&bytes), 1),
            6,
            Matrix3D::IDENTITY,
            &mut |change| {
                assert!(matches!(change, SavedTransformChange::Position(_)));
                observed += 1;
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(
        observed, 1,
        "position effect precedes failed orientation read"
    );
    observed = 0;
    assert!(
        xfer_transform_phase(
            &mut XferLoad::new(Cursor::new(&bytes), 1),
            9,
            Matrix3D::IDENTITY,
            &mut |_| {
                observed += 1;
                Ok(())
            }
        )
        .is_err()
    );
    assert_eq!(observed, 0, "incomplete matrix has no setter effect");
}

#[test]
fn checked_object_rejects_incomplete_header_before_changing_identity() {
    let mut factory = crate::object::object_factory::ObjectFactory::new();
    let id = factory
        .prepare_object(
            Arc::new(DefaultThingTemplate::new("DetachedHeader".into())),
            Coord3D::ZERO,
            None,
            crate::object::object_factory::ObjectCreationFlags::NO_DRAWABLE,
        )
        .unwrap();
    let owner = factory.get_object(id).unwrap().get_base_object().unwrap();
    for bytes in [vec![], vec![9], vec![9, 1, 2, 3]] {
        let mut object = owner.write().unwrap();
        let expected_phase = if bytes.is_empty() { "version" } else { "id" };
        let error = object
            .xfer_checked(&mut XferLoad::new(Cursor::new(bytes), 1))
            .unwrap_err();
        assert_eq!(error.object_id, id);
        assert_eq!(error.operation, expected_phase);
        assert_eq!(object.get_id(), id);
    }
}

#[test]
fn named_flags_reject_invalid_names_and_negative_counts_without_panicking() {
    let mut bytes = vec![1];
    bytes.extend_from_slice(&(-1_i32).to_le_bytes());
    let mut bits = 1;
    assert!(
        xfer_named_bits(
            &mut XferLoad::new(Cursor::new(bytes), 1),
            &mut bits,
            &["NONE"]
        )
        .is_err()
    );
    let mut bytes = Vec::new();
    let mut save = XferSave::new(Cursor::new(&mut bytes), 1);
    let mut version = 1;
    save.xfer_version(&mut version, 1).unwrap();
    let mut count = 1;
    save.xfer_int(&mut count).unwrap();
    save.xfer_ascii_string(&mut "NOT_A_STATUS".to_owned())
        .unwrap();
    let error = xfer_named_bits(
        &mut XferLoad::new(Cursor::new(bytes), 1),
        &mut bits,
        &["NONE"],
    )
    .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("NOT_A_STATUS"));
}

#[test]
fn pre_v8_status_mapping_omits_none_and_preserves_all_old_positions() {
    assert_eq!(restore_legacy_status(0), ObjectStatusMaskType::empty());
    assert_eq!(restore_legacy_status(1), ObjectStatusMaskType::DESTROYED);
    assert_eq!(
        restore_legacy_status(1 << 2),
        ObjectStatusMaskType::UNDER_CONSTRUCTION
    );
    assert_eq!(restore_legacy_status(1 << 31).bits(), 1_u64 << 32);
    let old = 1 | (1 << 2) | (1 << 31);
    let restored = restore_legacy_status(old);
    assert!(restored.contains(ObjectStatusMaskType::DESTROYED));
    assert!(restored.contains(ObjectStatusMaskType::UNDER_CONSTRUCTION));
    assert_eq!(restored.bits(), (1_u64 << 1) | (1 << 3) | (1 << 32));
    assert_eq!(restored.bits() & 1, 0, "old records never introduce NONE");
    assert_eq!(restore_legacy_status(u32::MAX).bits(), 0x1_ffff_fffe);
}

#[test]
fn nonfinite_saved_transform_is_rejected_before_its_setter() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut old = Vec::new();
        for value in [0.0_f32, 0.0, 0.0, bad] {
            old.extend_from_slice(&value.to_le_bytes());
        }
        let mut observed = 0;
        let error = xfer_transform_phase(
            &mut XferLoad::new(Cursor::new(old), 1),
            6,
            Matrix3D::IDENTITY,
            &mut |change| {
                assert!(matches!(change, SavedTransformChange::Position(_)));
                observed += 1;
                Ok(())
            },
        )
        .unwrap_err();
        assert!(error.contains("orientation"));
        assert_eq!(observed, 1);
        let mut matrix = Matrix3D::IDENTITY;
        matrix.x_axis.x = bad;
        let mut bytes = Vec::new();
        xfer_matrix3d(&mut XferSave::new(Cursor::new(&mut bytes), 1), &mut matrix).unwrap();
        let error = xfer_transform_phase(
            &mut XferLoad::new(Cursor::new(bytes), 1),
            9,
            Matrix3D::IDENTITY,
            &mut |_| panic!("invalid saved matrix must not reach a simulation setter"),
        )
        .unwrap_err();
        assert!(error.contains("matrix"));
    }
}

#[test]
fn finite_noncanonical_legacy_angles_never_reach_the_normalizing_setter() {
    for angle in [f32::MAX, -f32::MAX, 1.0e8, -1.0e8] {
        let mut bytes = Vec::new();
        for value in [0.0_f32, 0.0, 0.0, angle] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let mut observed = 0;
        let error = xfer_transform_phase(
            &mut XferLoad::new(Cursor::new(bytes), 1),
            6,
            Matrix3D::IDENTITY,
            &mut |change| {
                assert!(matches!(change, SavedTransformChange::Position(_)));
                observed += 1;
                Ok(())
            },
        )
        .unwrap_err();
        assert!(error.contains("non-canonical"));
        assert_eq!(
            observed, 1,
            "original position phase still precedes angle validation"
        );
    }
}

#[test]
fn unresolved_saved_team_is_an_error_in_the_exact_receiving_factory() {
    let receiver = crate::team::TeamFactory::new();
    let peer = crate::team::TeamFactory::new();
    assert_eq!(
        resolve_saved_team(&receiver, 71).unwrap_err(),
        "missing saved team 71"
    );
    assert!(peer.find_team_by_id(71).is_none());
}

#[test]
fn legacy_negative_pi_orientation_uses_original_positive_pi_boundary() {
    let pi = std::f32::consts::PI;
    assert_eq!(
        super::super::object_triggers::normalize_orientation_angle(-pi),
        pi
    );
    assert_eq!(
        super::super::object_triggers::normalize_orientation_angle(pi),
        pi
    );
    assert_eq!(
        super::super::object_triggers::normalize_orientation_angle(0.0),
        0.0
    );
}
