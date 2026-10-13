use super::*;
use crate::message_stream::player_state::get_local_player_id;

fn register(client: &mut GameClient) -> DrawableId {
    client
        .register_drawable(Box::new(BasicDrawable::new(DrawableId::INVALID)))
        .unwrap()
}

#[test]
fn actual_registration_uses_only_the_receiving_clients_counter() {
    let mut first = GameClient::new().unwrap();
    let mut peer = GameClient::new().unwrap();
    assert_eq!(register(&mut first), DrawableId(1));
    assert_eq!(register(&mut peer), DrawableId(1));
    for id in 2..=17 {
        assert_eq!(register(&mut peer), DrawableId(id));
    }
    peer.invalidate_presentation_drawable_world();
    assert_eq!(register(&mut first), DrawableId(2));
    assert_eq!(register(&mut peer), DrawableId(18));
    assert!(first.find_drawable_by_id(DrawableId(1)).is_some());
    assert!(peer.find_drawable_by_id(DrawableId(1)).is_none());
    drop(peer);
    assert_eq!(register(&mut first), DrawableId(3));
}

#[test]
fn detached_empty_save_and_failed_load_cannot_change_running_allocation() {
    let mut running = GameClient::new().unwrap();
    assert_eq!(register(&mut running), DrawableId(1));
    // GameClient.cpp1353 deliberately does not save m_nextDrawableID.
    // A detached empty archive starts at one; neither its creation nor its
    // loadPostProcess reserves IDs in the running client.
    let mut empty = GameClient::new().unwrap();
    let bytes = serialize_client(&mut empty);
    let mut candidate = deserialize_client(&bytes);
    assert_eq!(register(&mut candidate), DrawableId(1));
    assert_eq!(register(&mut candidate), DrawableId(2));

    let mut malformed = GameClient::new().unwrap();
    let mut load = XferLoad::new(Cursor::new(vec![3]), 3);
    assert!(malformed.xfer(&mut load).is_err());
    drop(malformed);
    drop(candidate);
    assert_eq!(register(&mut running), DrawableId(2));
}

#[test]
fn allocation_preserves_cpp_unsigned_increment() {
    let mut client = GameClient::new().unwrap();
    client.set_drawable_id_counter(u32::MAX);
    assert_eq!(client.alloc_drawable_id(), DrawableId(u32::MAX));
    assert_eq!(client.get_drawable_id_counter(), 0);
    assert_eq!(client.alloc_drawable_id(), DrawableId(0));
    assert_eq!(client.alloc_drawable_id(), DrawableId(1));
}

#[test]
fn detached_client_constructor_and_player_changes_do_not_publish_input_identity() {
    let mut running = Box::new(GameClient::new().unwrap());
    running.set_local_player_id(7);
    register_live_game_client(&mut running);
    assert_eq!(get_local_player_id(), 7);
    let mut candidate = GameClient::new().unwrap();
    assert_eq!(candidate.local_player_id, 0);
    candidate.set_local_player_id(3);
    assert_eq!(candidate.local_player_id, 3);
    assert_eq!(get_local_player_id(), 7);
    drop(candidate);
    assert_eq!(get_local_player_id(), 7);
    running.set_local_player_id(11);
    assert_eq!(get_local_player_id(), 11);
}
