//! Ordinary observation copies never acquire a live resource-retirement right.
use super::*;
use crate::game_logic::{GameLogic, ObjectId, Player, Team, ThingTemplate};

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "RadiusCloneOwner", true);
    player.color_rgb = (13, 27, 59);
    world.add_player(player);
    let template = ThingTemplate::new("GLAScudStormCloneResourceProbe");
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object("GLAScudStormCloneResourceProbe", Team::USA, Vec3::ZERO)
        .unwrap();
    world.host_object_mut(id).unwrap().owner_player_id = Some(0);
    (world, id)
}

fn rendered_at(position: Vec3) -> bool {
    game_client::radius_decal::get_projected_shadow_manager()
        .read()
        .collect_render_items()
        .iter()
        .any(|item| item.position == position && item.texture_name == SCUD_STORM_DECAL_TEXTURE)
}

#[test]
fn ordinary_object_clone_drop_keeps_live_radius_handle_and_same_id_peer() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "ordinary_object_clone_drop_keeps_live_radius_handle_and_same_id_peer",
        || {
            let (mut source, id) = world();
            let (mut peer, peer_id) = world();
            assert_eq!(id, peer_id);
            let source_pos = Vec3::new(71., 0., 89.);
            let peer_pos = Vec3::new(-117., 0., 33.);
            assert!(source.create_delivery_radius_decal(id, source_pos));
            assert!(peer.create_delivery_radius_decal(peer_id, peer_pos));
            assert!(rendered_at(source_pos));
            assert!(rendered_at(peer_pos));
            let observation = source.host_object(id).unwrap().clone();
            drop(observation);
            assert!(
                rendered_at(source_pos),
                "dropping an Object observation must not release its source radius handle"
            );
            assert!(rendered_at(peer_pos));
            source
                .host_object_mut(id)
                .unwrap()
                .radius_decal_update
                .as_mut()
                .unwrap()
                .delivery_decal
                .clear();
            assert!(
                !rendered_at(source_pos),
                "actual owner clear must still retire its handle"
            );
            assert!(rendered_at(peer_pos), "same-ID peer handle is independent");
        },
    );
}

#[test]
fn actual_encoded_world_radius_restore_reacquires_resource_without_stealing_source() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "actual_encoded_world_radius_restore_reacquires_resource_without_stealing_source",
        || {
            let (mut source, id) = world();
            let source_pos = Vec3::new(173., 0., 219.);
            let restored_pos = Vec3::new(-181., 0., 227.);
            assert!(source.create_delivery_radius_decal(id, source_pos));
            let builder = crate::save_load::SnapshotBuilder::new();
            let snapshot = builder.create_world_snapshot(&source).unwrap();
            let bytes = bincode_legacy::serialize(&snapshot).unwrap();
            let decoded = crate::save_load::decode_bincode_world_snapshot(&bytes).unwrap();
            assert!(
                rendered_at(source_pos),
                "snapshot observation must not release source resources"
            );
            let (mut restored, restored_id) = world();
            assert_eq!(id, restored_id);
            restored.templates = source.templates.clone();
            builder
                .restore_from_snapshot(&decoded, &mut restored)
                .unwrap();
            let restored_decal = &restored
                .host_object(id)
                .unwrap()
                .radius_decal_update
                .as_ref()
                .unwrap()
                .delivery_decal;
            assert!(
                !restored_decal.has_projected_shadow(),
                "resource pointers are absent after decode, as RadiusDecal.cpp144"
            );
            assert_eq!(restored_decal.position, source_pos);
            assert!(rendered_at(source_pos));
            assert!(restored.create_delivery_radius_decal(id, restored_pos));
            assert!(rendered_at(restored_pos));
            restored.reset();
            assert!(!rendered_at(restored_pos));
            assert!(
                rendered_at(source_pos),
                "restored owner reset cannot retire the source handle"
            );
        },
    );
}
