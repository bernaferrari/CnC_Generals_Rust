use game_engine::common::game_common::MAX_PLAYER_COUNT;
use gamelogic::object::w3d_ghost_object::{
    GhostSceneEvent, Matrix3x4, ParentGeometrySnapshot, RenderObjectClass, RenderObjectState,
    RenderSubObjectSnapshot, W3DDrawableInfo, W3DGhostObject, W3DGhostObjectManager,
    INVALID_DRAWABLE_ID, INVALID_OBJECT_ID, OBJECTSHROUD_FOGGED,
};

fn geometry() -> ParentGeometrySnapshot {
    ParentGeometrySnapshot {
        geometry_type: 2,
        is_small: false,
        major_radius: 20.0,
        minor_radius: 10.0,
        position: [1.0, 2.0, 3.0],
        angle: 0.5,
    }
}

fn render_object(name: &str) -> RenderObjectState {
    RenderObjectState {
        name: name.to_string(),
        scale: 1.25,
        color: 0xff00_ff00,
        transform: Matrix3x4::IDENTITY,
        sub_objects: vec![
            RenderSubObjectSnapshot {
                name: "BODY".to_string(),
                visible: true,
                transform: Matrix3x4::IDENTITY,
            },
            RenderSubObjectSnapshot {
                name: "MUZZLEFX01".to_string(),
                visible: true,
                transform: Matrix3x4::IDENTITY,
            },
        ],
        class_id: RenderObjectClass::HLod,
    }
}

#[test]
fn new_w3d_ghost_has_cpp_default_drawable_info_and_player_slots() {
    let ghost = W3DGhostObject::new();

    assert_eq!(MAX_PLAYER_COUNT, 16);
    assert_eq!(ghost.drawable_info().drawable_id, INVALID_DRAWABLE_ID);
    assert_eq!(ghost.drawable_info().flags, 0);
    assert_eq!(
        ghost.drawable_info().shroud_status_object_id,
        INVALID_OBJECT_ID
    );
    for player in 0..MAX_PLAYER_COUNT {
        assert!(ghost.snapshots(player).is_empty());
    }
}

#[test]
fn snapshot_only_captures_local_visible_drawables_and_disables_fog_effects() {
    let mut ghost = W3DGhostObject::new();
    ghost.update_parent_object(Some(42), true);

    ghost.snapshot(1, 0, false, &[render_object("Tank")], geometry());
    assert!(ghost.snapshots(1).is_empty());

    ghost.snapshot(0, 0, true, &[render_object("Tank")], geometry());
    assert!(ghost.snapshots(0).is_empty());

    ghost.snapshot(0, 0, false, &[render_object("Tank")], geometry());

    let snapshot = &ghost.snapshots(0)[0];
    assert!(snapshot.uv_animations_disabled);
    assert!(snapshot.muzzle_fx_hidden);
    assert!(!snapshot.render_object.sub_objects[1].visible);
    assert_eq!(ghost.parent_geometry(), Some(geometry()));
    assert_eq!(
        ghost.scene_events(),
        &[
            GhostSceneEvent::RemoveParentObject(42),
            GhostSceneEvent::AddSnapshot {
                player_index: 0,
                snapshot: 0
            }
        ]
    );
}

#[test]
fn free_snapshot_removes_scene_snapshot_and_restores_parent() {
    let mut ghost = W3DGhostObject::new();
    ghost.update_parent_object(Some(7), true);
    ghost.snapshot(0, 0, false, &[render_object("Dozer")], geometry());

    ghost.free_snapshot(0, 0);

    assert!(ghost.snapshots(0).is_empty());
    assert!(ghost
        .scene_events()
        .contains(&GhostSceneEvent::RemoveSnapshot {
            player_index: 0,
            snapshot: 0
        }));
    assert!(ghost
        .scene_events()
        .contains(&GhostSceneEvent::RestoreParentObject(7)));
}

#[test]
fn manager_uses_free_store_and_respects_lock_flags() {
    let mut manager = W3DGhostObjectManager::new();
    assert_eq!(manager.add_ghost_object(Some(1), true), Some(0));
    assert_eq!(manager.used_count(), 1);

    manager.remove_ghost_object(0);
    assert_eq!(manager.used_count(), 0);
    assert_eq!(manager.free_count(), 1);

    manager.set_lock_ghost_objects(true);
    assert_eq!(manager.add_ghost_object(Some(2), true), None);
    manager.set_lock_ghost_objects(false);
    manager.set_save_lock_ghost_objects(true);
    assert_eq!(manager.add_ghost_object(Some(2), true), None);
}

#[test]
fn local_player_switch_replaces_scene_objects_like_cpp() {
    let mut manager = W3DGhostObjectManager::new();
    manager.add_ghost_object(Some(9), true).unwrap();
    manager.used_mut()[0].snapshot(1, 1, false, &[render_object("Tank")], geometry());

    manager.set_local_player_index(1);

    let ghost = &manager.used()[0];
    assert!(ghost
        .scene_events()
        .contains(&GhostSceneEvent::RemoveParentObject(9)));
    assert!(ghost
        .scene_events()
        .contains(&GhostSceneEvent::AddSnapshot {
            player_index: 1,
            snapshot: 0
        }));
    assert_eq!(manager.local_player_index(), 1);
}

#[test]
fn orphan_update_releases_objects_without_any_stored_snapshots() {
    let mut manager = W3DGhostObjectManager::new();
    manager.add_ghost_object(None, true).unwrap();

    manager.update_orphaned_objects(&[]);

    assert_eq!(manager.used_count(), 0);
    assert_eq!(manager.free_count(), 1);
}

#[test]
fn partition_restore_marks_snapshot_players_as_fogged() {
    let mut manager = W3DGhostObjectManager::new();
    manager.add_ghost_object(Some(3), true).unwrap();
    manager.used_mut()[0].snapshot(0, 0, false, &[render_object("Tank")], geometry());

    manager.release_partition_data();
    manager.restore_partition_data();

    assert_eq!(
        manager.used()[0].previous_shroudedness(0),
        Some(OBJECTSHROUD_FOGGED)
    );
}

#[test]
fn drawable_info_round_trips_as_plain_w3d_state() {
    let mut ghost = W3DGhostObject::new();
    let info = W3DDrawableInfo {
        drawable_id: 77,
        flags: 0x20,
        shroud_status_object_id: 55,
    };

    ghost.set_drawable_info(info);

    assert_eq!(ghost.drawable_info(), info);
}
