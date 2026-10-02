use super::*;

fn player(index: PlayerIndex) -> Arc<RwLock<Player>> {
    Arc::new(RwLock::new(Player::new(index)))
}

#[test]
fn indexed_lookup_does_not_read_an_already_write_locked_player() {
    let admitted = player(7);
    let mut list = PlayerList::new();
    list.add_player(Arc::clone(&admitted));
    list.set_local_player_index(7);

    let _player_write = admitted.write().unwrap();
    assert!(Arc::ptr_eq(list.get_player(7).unwrap(), &admitted));
    assert!(Arc::ptr_eq(list.get_local_player().unwrap(), &admitted));
    assert!(list.get_player(0).is_none());
    assert!(list.get_player(-1).is_none());
    assert!(list.get_player(i32::MAX).is_none());
}

#[test]
fn sparse_admission_keeps_slot_identity_count_and_cpp_iteration_order() {
    let high = player((MAX_PLAYER_COUNT - 1) as PlayerIndex);
    let low = player(2);
    let mut list = PlayerList::new();
    list.add_player(Arc::clone(&high));
    list.add_player(Arc::clone(&low));

    assert_eq!(list.get_player_count(), 2);
    assert!(Arc::ptr_eq(list.get_player(2).unwrap(), &low));
    assert!(Arc::ptr_eq(
        list.get_player((MAX_PLAYER_COUNT - 1) as PlayerIndex)
            .unwrap(),
        &high,
    ));
    let mut players = list.iter();
    assert!(Arc::ptr_eq(players.next().unwrap(), &low));
    assert!(Arc::ptr_eq(players.next().unwrap(), &high));
    assert!(players.next().is_none());
}

#[test]
fn clear_resets_slots_and_local_index_without_retargeting_old_handles() {
    let original = player(4);
    let mut list = PlayerList::new();
    list.add_player(Arc::clone(&original));
    list.set_local_player_index(4);
    list.clear();

    assert_eq!(list.get_player_count(), 0);
    assert_eq!(list.get_local_player_index(), PLAYER_INDEX_INVALID);
    assert!(list.get_local_player().is_none());
    assert!(list.get_player(4).is_none());
    assert!(list.iter().next().is_none());

    let replacement = player(4);
    list.add_player(Arc::clone(&replacement));
    list.set_local_player_index(4);
    let _original_write = original.write().unwrap();
    assert!(Arc::ptr_eq(list.get_player(4).unwrap(), &replacement));
    assert!(Arc::ptr_eq(list.get_local_player().unwrap(), &replacement));
}

#[test]
fn admission_rejects_invalid_indices_and_keeps_the_first_duplicate_identity() {
    let original = player(3);
    let duplicate = player(3);
    let mut list = PlayerList::new();
    list.add_player(Arc::clone(&original));
    list.add_player(duplicate);
    list.add_player(player(PLAYER_INDEX_INVALID));
    list.add_player(player(MAX_PLAYER_COUNT as PlayerIndex));
    list.add_player(player(i32::MAX));

    assert_eq!(list.get_player_count(), 1);
    assert!(Arc::ptr_eq(list.get_player(3).unwrap(), &original));
    assert!(list.get_player(PLAYER_INDEX_INVALID).is_none());
    assert!(list.get_player(MAX_PLAYER_COUNT as PlayerIndex).is_none());
}
