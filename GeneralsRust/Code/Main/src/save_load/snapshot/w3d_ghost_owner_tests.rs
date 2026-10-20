//! C++ GhostObject named block and manager Xfer against actual owned managers.
//! Save decode/restore interleaving must not select a process-wide pending slot.
use super::*;
use crate::game_logic::GameLogic;
use crate::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};
use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
use gamelogic::object::w3d_ghost_object::{
    Matrix3x4, OBJECTSHROUD_FOGGED, ParentGeometrySnapshot, RenderObjectClass, RenderObjectState,
    RenderSubObjectSnapshot, W3DDrawableInfo,
};
use std::time::{Duration, SystemTime};

fn ghost_owner(name: &str, parent: Option<u32>, local: usize) -> W3DGhostObjectManager {
    let mut manager = W3DGhostObjectManager::new();
    manager.set_local_player_index(local);
    manager.add_ghost_object(parent, parent.is_some()).unwrap();
    let ghost = &mut manager.used_mut()[0];
    ghost.set_drawable_info(W3DDrawableInfo {
        drawable_id: 8,
        flags: 1,
        shroud_status_object_id: parent.unwrap_or(0),
    });
    ghost.snapshot(
        local,
        local,
        false,
        &[RenderObjectState {
            name: name.into(),
            scale: 2.0,
            geometry_scale: 2.0,
            color: 0x1122_3344,
            transform: Matrix3x4::IDENTITY,
            sub_objects: vec![RenderSubObjectSnapshot {
                name: "TURRET".into(),
                visible: true,
                transform: Matrix3x4::IDENTITY,
            }],
            class_id: RenderObjectClass::HLod,
        }],
        ParentGeometrySnapshot {
            geometry_type: 2,
            is_small: false,
            major_radius: 9.0,
            minor_radius: 3.0,
            position: [1.0, 2.0, 3.0],
            angle: 0.25,
        },
    );
    ghost.set_previous_shroudedness(local, OBJECTSHROUD_FOGGED);
    manager
}
fn info(slot: &str) -> SaveGameInfo {
    SaveGameInfo {
        pristine_map_name: None,
        filename: slot.into(),
        display_name: slot.into(),
        description: "owned ghost companion".into(),
        map_name: "GhostMap".into(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: Duration::ZERO,
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    }
}
fn assert_ghost(manager: &W3DGhostObjectManager, name: &str, parent: Option<u32>, local: usize) {
    assert_eq!(manager.used_count(), 1);
    assert_eq!(manager.local_player_index(), local);
    assert!(!manager.is_save_locked());
    let ghost = &manager.used()[0];
    assert_eq!(ghost.parent_object_id(), parent);
    assert_eq!(ghost.drawable_info().drawable_id, 8);
    assert_eq!(ghost.parent_geometry().unwrap().position, [1.0, 2.0, 3.0]);
    assert_eq!(
        ghost.previous_shroudedness(local),
        Some(OBJECTSHROUD_FOGGED)
    );
    let snapshot = &ghost.snapshots(local)[0];
    assert_eq!(snapshot.render_object.name, name);
    assert_eq!(snapshot.render_object.scale, 1.0);
    assert_eq!(snapshot.render_object.color, 0x1122_3344);
    assert_eq!(snapshot.render_object.sub_objects[0].name, "TURRET");
    assert!(snapshot.render_object.sub_objects[0].visible);
    assert!(snapshot.uv_animations_disabled);
}

#[test]
fn two_decoded_saves_keep_their_own_ghost_companion_through_interleaved_restore() {
    let directory = tempfile::TempDir::new().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    saves.init().unwrap();
    let mut a = GameLogic::new();
    a.objects.insert(
        crate::game_logic::ObjectId(42),
        crate::game_logic::Object::new(
            crate::game_logic::ThingTemplate::new("GhostParentA"),
            crate::game_logic::ObjectId(42),
            crate::game_logic::Team::USA,
        ),
    );
    a.w3d_ghosts = ghost_owner("GhostA", Some(42), 0);
    let mut b = GameLogic::new();
    b.objects.insert(
        crate::game_logic::ObjectId(42),
        crate::game_logic::Object::new(
            crate::game_logic::ThingTemplate::new("GhostParentB"),
            crate::game_logic::ObjectId(42),
            crate::game_logic::Team::China,
        ),
    );
    b.w3d_ghosts = ghost_owner("GhostB", Some(42), 1);
    saves.save_game("a", &a, &info("a")).unwrap();
    saves.save_game("b", &b, &info("b")).unwrap();
    let crate::save_load::DecodedGameSave {
        snapshot: snapshot_a,
        ..
    } = saves.load_game_snapshot("a").unwrap();
    let bytes_a = snapshot_a.w3d_ghost_companion.as_ref().unwrap().clone();
    let crate::save_load::DecodedGameSave {
        snapshot: snapshot_b,
        ..
    } = saves.load_game_snapshot("b").unwrap();
    assert_ne!(bytes_a, *snapshot_b.w3d_ghost_companion.as_ref().unwrap());
    assert_eq!(snapshot_a.w3d_ghost_companion.as_ref().unwrap(), &bytes_a);
    a.w3d_ghosts = Default::default();
    b.w3d_ghosts = Default::default();
    let builder = SnapshotBuilder::new();
    builder.restore_from_snapshot(&snapshot_b, &mut b).unwrap();
    builder.restore_from_snapshot(&snapshot_a, &mut a).unwrap();
    assert_ghost(&a.w3d_ghosts, "GhostA", Some(42), 0);
    assert_ghost(&b.w3d_ghosts, "GhostB", Some(42), 1);
    // A snapshot is repeatable owned data, not a one-use queue entry.
    builder.restore_from_snapshot(&snapshot_a, &mut a).unwrap();
    assert_ghost(&a.w3d_ghosts, "GhostA", Some(42), 0);
    assert_ghost(&b.w3d_ghosts, "GhostB", Some(42), 1);
    drop(b);
    assert_ghost(&a.w3d_ghosts, "GhostA", Some(42), 0);
}

#[test]
fn unrelated_restore_does_not_consume_a_decoded_ghost_companion() {
    let directory = tempfile::TempDir::new().unwrap();
    let mut saves = SaveFileManager::with_save_directory(directory.path());
    saves.init().unwrap();
    let mut source = GameLogic::new();
    source.w3d_ghosts = ghost_owner("WaitingGhost", None, 0);
    saves
        .save_game("waiting", &source, &info("waiting"))
        .unwrap();
    let crate::save_load::DecodedGameSave { snapshot, .. } =
        saves.load_game_snapshot("waiting").unwrap();
    let builder = SnapshotBuilder::new();
    let mut unrelated = GameLogic::new();
    unrelated.w3d_ghosts = ghost_owner("Unrelated", Some(42), 0);
    builder
        .restore_from_snapshot(&WorldSnapshot::default(), &mut unrelated)
        .unwrap();
    assert_eq!(unrelated.w3d_ghosts.used_count(), 0);
    let mut restored = GameLogic::new();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_ghost(&restored.w3d_ghosts, "WaitingGhost", None, 0);
    assert_ghost(&source.w3d_ghosts, "WaitingGhost", None, 0);
}

#[test]
fn malformed_ghost_companion_rejects_before_mutating_the_receiving_world() {
    let mut live = GameLogic::new();
    live.set_current_frame(91);
    live.w3d_ghosts = ghost_owner("StillLive", Some(42), 0);
    let before = live.w3d_ghosts.clone();
    let bytes = capture_w3d_ghost_xfer_bytes(&before).unwrap();
    for invalid in [
        bytes[..bytes.len() - 1].to_vec(),
        [bytes.clone(), vec![0x71]].concat(),
    ] {
        let mut snapshot = WorldSnapshot::default();
        snapshot.frame_number = 7;
        snapshot.w3d_ghost_companion = Some(invalid);
        assert!(
            SnapshotBuilder::new()
                .restore_from_snapshot(&snapshot, &mut live)
                .is_err()
        );
        assert_eq!(live.get_current_frame(), 91);
        assert_eq!(live.w3d_ghosts, before);
    }
}

#[test]
fn saved_ghost_geometry_is_validated_before_the_receiving_world_changes() {
    let owner = ghost_owner("FiniteFootprint", None, 0);
    let bytes = capture_w3d_ghost_xfer_bytes(&owner).unwrap();
    let valid = decode_w3d_ghost_xfer_bytes(&bytes).unwrap();
    assert_eq!(
        capture_w3d_ghost_xfer_bytes(&valid).unwrap(),
        bytes,
        "valid original ghost wire remains byte-identical"
    );
    let original = owner.used()[0].parent_geometry().unwrap();
    let mut bad = vec![
        ParentGeometrySnapshot {
            geometry_type: 3,
            ..original
        },
        ParentGeometrySnapshot {
            major_radius: f32::NAN,
            ..original
        },
        ParentGeometrySnapshot {
            minor_radius: -1.0,
            ..original
        },
        ParentGeometrySnapshot {
            angle: f32::INFINITY,
            ..original
        },
        ParentGeometrySnapshot {
            position: [1.0, f32::NAN, 3.0],
            ..original
        },
        ParentGeometrySnapshot {
            geometry_type: 0,
            major_radius: f32::MAX,
            ..original
        },
        ParentGeometrySnapshot {
            major_radius: 12_000.0,
            minor_radius: 12_000.0,
            ..original
        },
    ];
    let mut live = GameLogic::new();
    live.set_current_frame(91);
    live.w3d_ghosts = ghost_owner("LiveFinite", Some(42), 0);
    let before = live.w3d_ghosts.clone();
    for geometry in bad.drain(..) {
        let mut candidate = owner.clone();
        candidate.used_mut()[0].set_parent_geometry(geometry);
        let mut snapshot = WorldSnapshot::default();
        snapshot.frame_number = 7;
        snapshot.w3d_ghost_companion = Some(capture_w3d_ghost_xfer_bytes(&candidate).unwrap());
        assert!(
            SnapshotBuilder::new()
                .restore_from_snapshot(&snapshot, &mut live)
                .is_err()
        );
        assert_eq!(live.get_current_frame(), 91);
        assert_eq!(
            live.w3d_ghosts, before,
            "failed geometry cannot alter live snapshots or scene events"
        );
    }
}

#[test]
fn saved_manager_bounds_the_combined_original_footprint_work() {
    let mut manager = W3DGhostObjectManager::new();
    for _ in 0..17 {
        manager.add_ghost_object(None, false).unwrap();
        manager.used_mut()[0].set_parent_geometry(ParentGeometrySnapshot {
            geometry_type: 2,
            is_small: false,
            major_radius: 10_240.0,
            minor_radius: 10_240.0,
            position: [0.0; 3],
            angle: 0.0,
        });
    }
    let before = manager.clone();
    let bytes = capture_w3d_ghost_xfer_bytes(&manager).unwrap();
    assert!(decode_w3d_ghost_xfer_bytes(&bytes).is_err());
    assert_eq!(manager, before);
}

#[test]
fn manager_decode_keeps_other_owners_creation_locks_and_scene_events_unchanged() {
    let mut live = ghost_owner("LiveLocked", Some(42), 0);
    live.set_save_lock_ghost_objects(true);
    let before = live.clone();
    let candidate = ghost_owner("Candidate", None, 1);
    let bytes = capture_w3d_ghost_xfer_bytes(&candidate).unwrap();
    let loaded = decode_w3d_ghost_xfer_bytes(&bytes).unwrap();
    assert_ghost(&loaded, "Candidate", None, 1);
    assert_eq!(live, before);
    assert!(live.add_ghost_object(Some(99), true).is_none());
    assert_eq!(live, before);
    assert!(decode_w3d_ghost_xfer_bytes(&bytes[..bytes.len() - 1]).is_err());
    assert_eq!(live, before);
}

#[test]
fn named_ghost_companion_does_not_change_the_gamelogic_positional_body() {
    let mut snapshot = WorldSnapshot::default();
    let body = bincode_legacy::serialize(&snapshot).unwrap();
    snapshot.w3d_ghost_companion =
        Some(capture_w3d_ghost_xfer_bytes(&ghost_owner("SeparateBlock", Some(42), 0)).unwrap());
    assert_eq!(bincode_legacy::serialize(&snapshot).unwrap(), body);
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&body).unwrap();
    assert!(decoded.w3d_ghost_companion.is_none());
}

#[cfg(feature = "game_client")]
#[test]
fn presentation_freezes_the_driving_ghost_owner_and_retains_its_snapshot() {
    let mut a = GameLogic::new();
    let mut b = GameLogic::new();
    a.w3d_ghosts = ghost_owner("FrameA", Some(42), 0);
    b.w3d_ghosts = ghost_owner("FrameB", Some(42), 0);
    let frame_a = crate::presentation_frame::PresentationFrame::build_from_logic(&mut a, 0);
    let frame_b = crate::presentation_frame::PresentationFrame::build_from_logic(&mut b, 0);
    let scene_a = frame_a.w3d_ghost_scene.as_ref().unwrap();
    let scene_b = frame_b.w3d_ghost_scene.as_ref().unwrap();
    assert_eq!(scene_a.snapshots.len(), 1);
    assert_eq!(scene_b.snapshots.len(), 1);
    assert_eq!(
        scene_a.snapshots[0].render_object.render_object.name,
        "FrameA"
    );
    assert_eq!(
        scene_b.snapshots[0].render_object.render_object.name,
        "FrameB"
    );
    assert_eq!(scene_a.hidden_parent_object_ids, vec![42]);
    assert_eq!(scene_b.hidden_parent_object_ids, vec![42]);
    // C++ snapShot hides the normal render objects without toggling the
    // Drawable's separate FullyObscured flag. freeSnapShot restores them.
    assert!(!a.w3d_ghosts.used()[0].parent_fully_obscured());
    a.w3d_ghosts.used_mut()[0].free_snapshot(0, 0);
    let freed = crate::presentation_frame::PresentationFrame::build_from_logic(&mut a, 0);
    assert!(freed.w3d_ghost_scene.as_ref().unwrap().snapshots.is_empty());
    assert!(
        freed
            .w3d_ghost_scene
            .as_ref()
            .unwrap()
            .hidden_parent_object_ids
            .is_empty()
    );
    a.w3d_ghosts = Default::default();
    let cleared = crate::presentation_frame::PresentationFrame::build_from_logic(&mut a, 0);
    assert!(
        cleared
            .w3d_ghost_scene
            .as_ref()
            .unwrap()
            .snapshots
            .is_empty()
    );
    assert_eq!(
        scene_a.snapshots[0].render_object.render_object.name,
        "FrameA"
    );
    assert_ghost(&b.w3d_ghosts, "FrameB", Some(42), 0);
}

#[test]
fn truncated_large_child_count_fails_without_reserving_the_declared_count() {
    let owner = ghost_owner("BoundedChildRead", Some(42), 0);
    let mut bytes = capture_w3d_ghost_xfer_bytes(&owner).unwrap();
    // W3DRenderObjectSnapshot::xfer v1 writes the actual affine transform,
    // then its i32 child count (C++ W3DGhostObject.cpp:174-177). The first
    // identity transform in this authored manager is that root transform.
    let matrix: Vec<u8> = Matrix3x4::IDENTITY
        .rows
        .iter()
        .flat_map(|row| row.iter())
        .flat_map(|value| value.to_le_bytes())
        .collect();
    let transform = bytes
        .windows(matrix.len())
        .position(|window| window == matrix)
        .unwrap();
    let count = transform + matrix.len();
    assert_eq!(&bytes[count..count + 4], &1i32.to_le_bytes());
    bytes[count..count + 4].copy_from_slice(&i32::MAX.to_le_bytes());
    assert!(decode_w3d_ghost_xfer_bytes(&bytes).is_err());
    assert_ghost(&owner, "BoundedChildRead", Some(42), 0);
}

#[test]
fn direct_gamelogic_xfer_clears_any_prior_named_ghost_companion() {
    use crate::save_load::{Snapshot, XferLoad, XferSave};
    let mut source = WorldSnapshot::default();
    let mut bytes = Vec::new();
    source
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes)))
        .unwrap();
    let mut destination = WorldSnapshot::default();
    destination.w3d_ghost_companion =
        Some(capture_w3d_ghost_xfer_bytes(&ghost_owner("PreviousNamedLoad", Some(42), 0)).unwrap());
    destination
        .xfer(&mut XferLoad::new(Cursor::new(bytes)))
        .unwrap();
    assert!(destination.w3d_ghost_companion.is_none());
}
