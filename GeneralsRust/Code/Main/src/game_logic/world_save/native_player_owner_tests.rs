//! Receiving observer / linked Side list transfer before Team create callbacks.
use super::*;
fn build_world(first: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world.add_player(
        Player::new(7, Team::USA, "BuildOwner", false).with_script_identity("BuildOwner"),
    );
    world.ai_manager.ai_players.insert(
        7,
        crate::ai::AIPlayer::new(7, Team::USA, crate::ai::AIDifficulty::Medium),
    );
    let mut dict = Dict::new();
    dict.set_ascii_string(key_player_name(), "BuildOwner");
    world.prepared_map_sides.add_side(&dict);
    for name in ["FirstBuilding", "SecondBuilding"] {
        let mut template = ThingTemplate::new(name);
        template.add_kind_of(KindOf::Structure).set_health(100.0);
        world.templates.insert(name.into(), template);
    }
    for (index, name) in [first, "SecondBuilding"].into_iter().enumerate() {
        let mut entry = gamelogic::build_list_info::BuildListInfo::new();
        entry.set_template_name(name.into());
        entry.set_building_name(format!("Authored{index}").into());
        entry.set_script(format!("Complete{index}").into());
        entry.set_num_rebuilds(if index == 0 { u32::MAX } else { 2 });
        entry.set_health(73 + index as i32);
        entry.set_unsellable(index == 0);
        world
            .prepared_map_sides
            .get_side_info_mut(0)
            .unwrap()
            .add_to_build_list(entry, index as i32);
    }
    world.transfer_side_build_lists_to_players();
    world
}
#[test]
fn receiving_side_lists_keep_order_owner_and_ai_save_continuation() {
    let (mut owner, mut peer) = (build_world("FirstBuilding"), build_world("SecondBuilding"));
    assert!(
        owner
            .prepared_map_sides
            .get_side_info(0)
            .unwrap()
            .get_build_list()
            .is_none()
    );
    let rows = &owner.get_player(7).unwrap().map_side.build_list;
    assert_eq!(
        rows.iter()
            .map(|entry| entry.template.as_str())
            .collect::<Vec<_>>(),
        ["FirstBuilding", "SecondBuilding"]
    );
    assert_eq!(rows[0].num_rebuilds, u32::MAX);
    assert_eq!(rows[0].completion.script, "Complete0");
    assert_eq!(rows[1].completion.health_percent, 74);
    assert_eq!(
        peer.get_player(7).unwrap().map_side.build_list[0].template,
        "SecondBuilding"
    );
    assert_eq!(
        owner.ai_manager.ai_players[&7].building_queue[0].template_name,
        "FirstBuilding"
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let mut restored = build_world("SecondBuilding");
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        restored.ai_manager.ai_players[&7].building_queue[0].template_name,
        "FirstBuilding"
    );
    peer.reset();
    assert_eq!(
        owner.get_player(7).unwrap().map_side.build_list[0].template,
        "FirstBuilding"
    );
}
#[test]
fn receiving_replay_observer_keeps_local_identity_through_peer_reset_and_restore() {
    let mut owner = GameLogic::new();
    let mut peer = GameLogic::new();
    owner.game_mode = GameMode::Replay;
    peer.game_mode = GameMode::Replay;
    owner.install_replay_observer_side();
    peer.install_replay_observer_side();
    let id = owner.replay_observer_player_id().unwrap();
    assert_eq!(peer.replay_observer_player_id(), Some(id));
    owner.apply_replay_observer_as_local_player();
    assert_eq!(owner.local_player_id(), Some(id));
    assert!(owner.get_player(id).unwrap().is_observer);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let encoded = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&encoded).unwrap();
    let mut restored = GameLogic::new();
    restored.install_replay_observer_side();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(restored.replay_observer_player_id(), Some(id));
    assert_eq!(restored.local_player_id(), Some(id));
    assert_eq!(
        restored.get_player(id).unwrap().map_side.map_player_name,
        "ReplayObserver"
    );
    peer.reset();
    assert_eq!(owner.local_player_id(), Some(id));
}
