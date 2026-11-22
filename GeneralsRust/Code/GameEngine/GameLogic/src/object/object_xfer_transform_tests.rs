//! Original Object.cpp transform phase. No registry or shared clock fixture.
use super::*;
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

#[test]
fn obsolete_object_versions_are_rejected_before_identity_changes() {
    for version in 0..OBJECT_XFER_VERSION {
        let mut bytes = vec![version];
        bytes.extend_from_slice(&0xB333_u32.to_le_bytes());
        let mut object = Object::new_test(0xA722, 100.0);
        let error = object
            .xfer_checked(&mut XferLoad::new(Cursor::new(&bytes), 1))
            .unwrap_err();
        assert_eq!(error.operation, "version");
        assert_eq!(object.get_id(), 0xA722);
        assert!(native::ObjectSnapshotRecord {
            template_name: "Obsolete".into(),
            bytes,
        }
        .saved_id()
        .is_err());
    }
}

#[test]
fn current_matrix_record_keeps_full_rows_and_one_observation() {
    let matrix = Matrix3D::from_translation(Coord3D::new(10.0, -20.0, 30.0))
        * Matrix3D::from_rotation_z(0.75);
    let mut bytes = Vec::new();
    xfer_transform_phase(
        &mut XferSave::new(Cursor::new(&mut bytes), 1),
        matrix,
        &mut |value| {
            assert_eq!(value, matrix);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(bytes.len(), 49);
    assert_eq!(bytes[0], 1);
    let mut calls = 0;
    xfer_transform_phase(
        &mut XferLoad::new(Cursor::new(&bytes), 1),
        Matrix3D::IDENTITY,
        &mut |value| {
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
    let mut identity = Matrix3D::IDENTITY;
    xfer_matrix3d(
        &mut XferSave::new(Cursor::new(&mut bytes), 1),
        &mut identity,
    )
    .unwrap();
    bytes.truncate(20);
    let mut observed = 0;
    assert!(
        xfer_transform_phase(
            &mut XferLoad::new(Cursor::new(&bytes), 1),
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
    let mut identity_owner = crate::system::game_logic::GameLogic::new();
    let id = factory
        .prepare_object(
            &mut identity_owner,
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
fn nonfinite_saved_transform_is_rejected_before_its_setter() {
    for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut matrix = Matrix3D::IDENTITY;
        matrix.x_axis.x = bad;
        let mut bytes = Vec::new();
        xfer_matrix3d(&mut XferSave::new(Cursor::new(&mut bytes), 1), &mut matrix).unwrap();
        let error = xfer_transform_phase(
            &mut XferLoad::new(Cursor::new(bytes), 1),
            Matrix3D::IDENTITY,
            &mut |_| panic!("invalid saved matrix must not reach a simulation setter"),
        )
        .unwrap_err();
        assert!(error.contains("matrix"));
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
