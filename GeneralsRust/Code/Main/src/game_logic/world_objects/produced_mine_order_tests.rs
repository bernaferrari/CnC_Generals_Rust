use super::super::*;

fn world() -> (GameLogic, ObjectId, [ObjectId; 3]) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Original", true));
    world.add_player(Player::new(1, Team::USA, "Capturing", false));
    let mut producer = ThingTemplate::new("OrderedMineProducer");
    producer.add_kind_of(KindOf::Structure).set_health(100.0);
    producer.set_authored_ai_update_interface(Some(false));
    world.templates.insert(producer.name.clone(), producer);
    let mut mine = ThingTemplate::new("OrderedProducedMine");
    mine.add_kind_of(KindOf::Mine).set_health(100.0);
    mine.set_authored_ai_update_interface(Some(false));
    world.templates.insert(mine.name.clone(), mine);
    let producer = world
        .create_object_for_player("OrderedMineProducer", 0, Vec3::ZERO)
        .unwrap();
    let mut mines = [ObjectId(0); 3];
    for (slot, id) in mines.iter_mut().zip([700, 9, 300]) {
        world.set_next_object_id_for_restore(ObjectId(id));
        *slot = world
            .create_object_for_player("OrderedProducedMine", 0, Vec3::ZERO)
            .unwrap();
        world.objects.get_mut(slot).unwrap().producer_id = Some(producer);
    }
    world.ownership_events.clear();
    (world, producer, mines)
}

fn capture_order(world: &mut GameLogic, producer: ObjectId) -> Vec<ObjectId> {
    let destination = world.host_default_team_id(1).unwrap();
    assert!(world.defect_host_object(producer, destination, 0));
    world.ownership_events.complete_boundary();
    world
        .ownership_events
        .take_completed()
        .into_iter()
        .map(|event| event.object)
        .collect()
}

#[test]
fn produced_mines_capture_in_registration_order_and_prepend_roster_synchronously() {
    // Repetition varies the independent HashMap seeds; list order is fixed by
    // actual registration, not by numeric IDs or allocation/hash iteration.
    for _ in 0..16 {
        let (mut world, producer, [first, second, third]) = world();
        let (peer, peer_producer, _) = self::world();
        assert_eq!(producer, peer_producer);
        assert_eq!(
            capture_order(&mut world, producer),
            [producer, third, second, first]
        );
        let destination = world.host_default_team_id(1).unwrap();
        let factory = world.team_factory.lock().unwrap();
        let team = factory.find_team_by_id(destination).unwrap();
        assert_eq!(
            team.read().unwrap().get_members(),
            &[first.0, second.0, third.0, producer.0]
        );
        assert_eq!(peer.objects[&third].owner_player_id, Some(0));
        assert_eq!(
            peer.objects.ordered_ids(),
            &[third, second, first, producer]
        );
    }
}

#[test]
fn recreated_mine_is_list_head_and_unrelated_or_removed_mines_do_not_transfer() {
    let (mut world, producer, [first, second, third]) = world();
    assert!(!world.finish_direct_object_removal(first));
    world.set_next_object_id_for_restore(first);
    let recreated = world
        .create_object_for_player("OrderedProducedMine", 0, Vec3::ZERO)
        .unwrap();
    assert_eq!(recreated, first);
    world.host_object_mut(first).unwrap().producer_id = Some(producer);
    world.objects.get_mut(&second).unwrap().producer_id = None;
    // The return value reports a removed structure, not removal success.
    assert!(!world.finish_direct_object_removal(third));
    assert!(!world.objects.contains_key(&third));
    world.ownership_events.clear();
    assert_eq!(capture_order(&mut world, producer), [producer, first]);
    assert_eq!(world.objects[&second].owner_player_id, Some(0));
    assert!(!world.objects.contains_key(&third));
}

#[test]
fn loaded_produced_mines_follow_original_recreated_list_before_capture() {
    let (source, producer, [first, second, third]) = world();
    let builder = crate::save_load::SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let mut loaded = GameLogic::new();
    loaded.add_player(Player::new(0, Team::USA, "Original", true));
    loaded.add_player(Player::new(1, Team::USA, "Capturing", false));
    loaded.templates = source.templates.clone();
    builder.restore_from_snapshot(&saved, &mut loaded).unwrap();
    // GameLogic::xfer saves traversal order, then newObject prepends records.
    assert_eq!(
        loaded.objects.ordered_ids(),
        &[producer, first, second, third]
    );
    assert_eq!(
        capture_order(&mut loaded, producer),
        [producer, first, second, third]
    );
    assert_eq!(source.objects[&first].owner_player_id, Some(0));
}
