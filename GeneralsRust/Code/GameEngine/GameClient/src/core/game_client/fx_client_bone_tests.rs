//! Actual per-client drawable bone lookup, with no published-client/test lock.
use super::*;
use crate::drawable::{BasicDrawable, BoneData, Drawable, Matrix4, Vector3};

fn client_with_bone(x: f32) -> GameClient {
    let mut client = GameClient::new().unwrap();
    let mut drawable = BasicDrawable::new(DrawableId::INVALID);
    drawable.set_object_id(Some(73));
    let mut bones = BoneData::default();
    bones.add_current_bone(
        "WeaponFireFXBone01",
        Vector3::new(x, 2.0, 3.0),
        Matrix4::translation(Vector3::new(x, 2.0, 3.0)),
    );
    drawable.set_bone_data(bones);
    client
        .register_drawable_with_template(Box::new(drawable), Some("FXOwner".into()))
        .unwrap();
    client
}

#[test]
fn current_bone_query_resolves_only_the_supplied_same_id_client() {
    let mut first = client_with_bone(11.0);
    let mut peer = client_with_bone(22.0);
    let first_bones = first.query_current_client_bone_positions(73, "WeaponFireFXBone", 1, 4);
    let peer_bones = peer.query_current_client_bone_positions(73, "WeaponFireFXBone", 1, 4);
    assert_eq!(first_bones.len(), 1);
    assert_eq!(peer_bones.len(), 1);
    assert_eq!(first_bones[0].0.x, 11.0);
    assert_eq!(first_bones[0].1.w_axis.x, 11.0);
    assert_eq!(peer_bones[0].0.x, 22.0);
    assert_eq!(peer_bones[0].1.w_axis.x, 22.0);
    let mut first_owner_bones = first
        .drawables
        .borrow_native()
        .current_client_bone_transforms(73, "WeaponFireFXBone", 1, 4);
    let peer_owner_bones = peer
        .drawables
        .borrow_native()
        .current_client_bone_transforms(73, "WeaponFireFXBone", 1, 4);
    assert_eq!(first_owner_bones.len(), 1);
    assert_eq!(peer_owner_bones.len(), 1);
    assert_eq!(first_owner_bones.pop().unwrap().w_axis.x, 11.0);
    assert_eq!(peer_owner_bones[0].w_axis.x, 22.0);
    assert!(
        first
            .query_current_client_bone_positions(73, "WeaponFireFXBone", 0, 4)
            .is_empty(),
        "prefix01 storage does not supply an unnumbered prefix bone"
    );
    assert!(
        first
            .query_current_client_bone_positions(74, "WeaponFireFXBone", 1, 4)
            .is_empty()
    );
    assert!(
        first
            .query_current_client_bone_positions(73, "WeaponFireFXBone", 0, 0)
            .is_empty()
    );
}
