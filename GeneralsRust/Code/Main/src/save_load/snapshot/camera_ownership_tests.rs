//! Camera values belong to detached save candidates, never a process-wide pending slot.
use super::*;

fn camera(offset: f32) -> CameraPersist {
    CameraPersist {
        angle: 0.5 + offset,
        position: [1000.0 + offset, 2000.0, 3000.0],
        target: [10.0 + offset, 30.0, 20.0],
        zoom: 0.75 + offset,
    }
}

fn view_bytes(persist: &WorldPersistV18) -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    write_tactical_view_block(&mut CommonXferSave::new(&mut bytes, 1), persist).unwrap();
    bytes.into_inner()
}

#[test]
fn camera_owned_view_wire_is_cpp_angle_and_logical_position_not_native_eye() {
    // View.cpp::View::xfer version 1 transfers raw angle, then m_pos XYZ.
    let mut persist = WorldPersistV18::default();
    camera(0.0).write_to(&mut persist);
    let mut expected = vec![1];
    for value in [0.5_f32, 10.0, 20.0, 30.0] {
        expected.extend_from_slice(&value.to_le_bytes());
    }
    assert_eq!(view_bytes(&persist), expected);
    assert_eq!(
        parse_tactical_view_block(&expected).unwrap(),
        TacticalViewPersist {
            angle: 0.5,
            position: [10.0, 20.0, 30.0],
        }
    );
    assert_eq!(persist.camera(), Some(camera(0.0)));
}

#[test]
fn camera_owned_source_view_sidecar_never_changes_native_bincode_record() {
    let mut persist = WorldPersistV18::default();
    camera(0.0).write_to(&mut persist);
    let without = bincode_legacy::serialize(&persist).unwrap();
    persist.tactical_view_camera = Some(TacticalViewPersist {
        angle: 2.0,
        position: [99.0, 88.0, 77.0],
    });
    assert_eq!(bincode_legacy::serialize(&persist).unwrap(), without);
    let decoded: WorldPersistV18 = bincode_legacy::deserialize(&without).unwrap();
    assert_eq!(decoded.camera(), Some(camera(0.0)));
    assert_eq!(decoded.tactical_view_camera, None);
}

#[test]
fn camera_owned_parse_capture_restore_interleaving_never_publishes_to_peer() {
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    let mut saved_a = capture_persist_v18(&a);
    let mut saved_b = capture_persist_v18(&b);
    camera(0.0).write_to(&mut saved_a);
    camera(1.0).write_to(&mut saved_b);
    let source_a = parse_tactical_view_block(&view_bytes(&saved_a)).unwrap();
    let source_b = parse_tactical_view_block(&view_bytes(&saved_b)).unwrap();
    restore_persist_v18(&saved_b, &mut b).unwrap();
    // Logic-only capture cannot consume either another world's camera or its own
    // detached native record: the engine explicitly supplies that view at save.
    assert_eq!(capture_persist_v18(&a).camera(), None);
    assert_eq!(capture_persist_v18(&b).camera(), None);
    restore_persist_v18(&saved_a, &mut a).unwrap();
    assert_eq!(capture_persist_v18(&b).camera(), None);
    assert_eq!(saved_a.camera(), Some(camera(0.0)));
    assert_eq!(saved_b.camera(), Some(camera(1.0)));
    assert_ne!(source_a, source_b);
    assert_eq!(
        parse_tactical_view_block(&view_bytes(&saved_a)).unwrap(),
        source_a
    );
}
