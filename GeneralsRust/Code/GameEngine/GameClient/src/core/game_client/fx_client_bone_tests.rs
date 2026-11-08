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
    crate::test_support::ensure_ambient_host();
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

#[test]
fn host_bone_fx_uses_borrowed_receiving_client_after_move_and_peer_drop() {
    use crate::fx_list::{FXList, FXListStore, FXNugget, FxViewServices};
    use game_engine::common::ini::ini_fx_list::{FXList as AuthoredList, FXNugget as AuthoredNugget};
    use game_engine::common::ascii_string::AsciiString;
    use std::sync::mpsc;

    struct Observe(mpsc::Sender<String>);
    impl FXNugget for Observe {
        fn do_fx_pos(
            &self,
            primary: Option<&gamelogic::common::Coord3D>,
            _: Option<&gamelogic::common::Matrix3D>,
            _: f32,
            _: Option<&gamelogic::common::Coord3D>,
            _: f32,
        ) {
            self.0.send(format!("effect:{}", primary.unwrap().x)).unwrap();
        }
    }

    fn dispatch(client: &GameClient, nested_client: &GameClient) -> Vec<String> {
        let (send, receive) = mpsc::channel();
        let mut child = FXList::new();
        child.add_fx_nugget(Box::new(Observe(send.clone())));
        let mut catalog = FXListStore::new();
        catalog.add_fx_list("child".into(), child);
        let mut authored = AuthoredList::new(AsciiString::from("outer"));
        authored.add_nugget(AuthoredNugget::FXListAtBonePos {
            fx_name: AsciiString::from("child"),
            bone_name: AsciiString::from("WeaponFireFXBone"),
            orient_to_bone: true,
        });
        let outer = FXList::from_definition(&authored);
        let pose = gamelogic::helpers::HostFxObjectPose {
            id: 73,
            position: gamelogic::common::Coord3D::ZERO,
            transform: gamelogic::common::Matrix3D::IDENTITY,
            player_index: 0,
            bounding_circle_radius: 1.0,
            is_shrouded: false,
        };
        let mut find = |name: &str| {
            assert_eq!(name, "child");
            // A nested client borrow must not change the receiving client.
            assert_eq!(
                nested_client.query_current_client_bone_positions(73, "WeaponFireFXBone", 1, 4)[0].0.x,
                22.0
            );
            catalog.find_fx_list(name)
        };
        let mut bones = |id, name: &str, start, limit| {
            send.send(format!("bones:{start}")).unwrap();
            client.query_current_client_bone_positions(id, name, start, limit)
                .into_iter()
                .map(|(_, matrix)| matrix)
                .collect()
        };
        outer.do_fx_obj_host_with_view(&pose, None, &mut FxViewServices {
            add_scorch: &mut |_, _, _| {},
            shake: &mut |_, _| {},
            find_list: &mut find,
            bone_transforms: &mut bones,
            position_visible: &mut |_| true,
            create_light_pulse: &mut |_| false,
            play_fx_audio: &mut |_, _| false,
        });
        receive.try_iter().collect()
    }

    crate::test_support::ensure_ambient_host();
    let first = client_with_bone(11.0);
    let peer = client_with_bone(22.0);
    let first = Box::new(first);
    assert_eq!(dispatch(&first, &peer), ["bones:0", "bones:1", "effect:11"]);
    assert_eq!(dispatch(&peer, &peer), ["bones:0", "bones:1", "effect:22"]);
    drop(first);
    assert_eq!(dispatch(&peer, &peer), ["bones:0", "bones:1", "effect:22"]);
}
