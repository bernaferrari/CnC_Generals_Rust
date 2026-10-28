use generals_main::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use generals_main::save_load::SnapshotBuilder;

fn world(name: &str, object_count: usize) -> (GameLogic, Vec<ObjectId>) {
    let mut logic = GameLogic::new();
    let mut template = ThingTemplate::new("HotkeySquadSessionProbe");
    template
        .set_health(100.0)
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable);
    logic.templates.insert(template.name.clone(), template);
    logic.add_player(Player::new(0, Team::USA, name, true));
    let ids = (0..object_count)
        .map(|index| {
            logic
                .create_object_for_player(
                    "HotkeySquadSessionProbe",
                    0,
                    glam::Vec3::new(index as f32 * 10.0, 0.0, 0.0),
                )
                .expect("receiving player object")
        })
        .collect();
    (logic, ids)
}

#[test]
fn same_player_id_replay_squads_add_empty_selection_and_restore_are_session_owned() {
    let (mut owner, owner_ids) = world("Owner", 1);
    let (mut peer, peer_ids) = world("Peer", 2);
    assert_eq!(owner_ids[0], peer_ids[0], "ObjectIDs overlap across worlds");

    owner.process_player_create_team(0, 3, &owner_ids);
    peer.process_player_create_team(0, 3, &peer_ids);
    assert!(owner.process_player_select_team(0, 3));
    assert!(peer.process_player_select_team(0, 3));
    assert_eq!(owner.player_selected_objects(0), owner_ids);
    assert_eq!(peer.player_selected_objects(0), peer_ids);

    // Create evicts object 2 from squad 3 into squad 4; Add then appends it
    // after object 1 in the current selection, preserving the C++ message order.
    peer.process_player_create_team(0, 4, &[peer_ids[1]]);
    assert_eq!(
        peer.player_hotkey_squad_ids(0, 3),
        [peer_ids[0]],
        "CreateTeam evicts a member from another squad"
    );
    assert!(peer.process_player_select_team(0, 3));
    assert!(peer.process_player_add_team(0, 4));
    assert_eq!(peer.player_selected_objects(0), peer_ids);

    // An empty slot is still a valid C++ Squad; selecting it clears only the
    // owner's current selection and does not touch the peer with player ID 0.
    assert!(owner.process_player_select_team(0, 9));
    assert!(owner.player_selected_objects(0).is_empty());
    assert_eq!(peer.player_selected_objects(0), peer_ids);

    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    assert!(
        generals_main::save_load::snapshot::read_control_groups(&snapshot.lifecycle_tail)
            .unwrap()
            .is_none(),
        "new v9 Player snapshots use the canonical per-player chunk without a duplicate HSQD"
    );
    let mut restored = world("Owner restored", 0).0;
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(restored.player_selected_objects(0).is_empty());
    assert_eq!(restored.player_hotkey_squad_ids(0, 3), owner_ids);
    assert!(restored.process_player_select_team(0, 3));
    assert_eq!(restored.player_selected_objects(0), owner_ids);
    assert_eq!(peer.player_hotkey_squad_ids(0, 3), [peer_ids[0]]);
    assert_eq!(peer.player_hotkey_squad_ids(0, 4), [peer_ids[1]]);
    assert_eq!(peer.player_selected_objects(0), peer_ids);

    // Selection prunes only objects absent from this receiver's live roster;
    // the other world still retains its colliding ID and group membership.
    peer.destroy_object(peer_ids[1]).unwrap();
    assert!(peer.process_player_select_team(0, 4));
    assert_eq!(peer.player_hotkey_squad_ids(0, 4), [peer_ids[1]]);
    peer.process_destroy_list();
    assert!(peer.process_player_select_team(0, 4));
    assert!(peer.player_hotkey_squad_ids(0, 4).is_empty());
    assert_eq!(owner.player_hotkey_squad_ids(0, 3), owner_ids);

    // C++ Squad::addObject does not deduplicate. The Player's canonical current
    // selection preserves duplicate IDs even though native UI mirrors are unique.
    assert!(peer.process_player_select_team(0, 3));
    assert!(peer.process_player_add_team(0, 3));
    assert_eq!(
        peer.player_selected_objects(0),
        [peer_ids[0], peer_ids[0]],
        "AddTeam appends squad IDs in order, including duplicates"
    );

    // The simulation's Player selection includes selectable squad members
    // even after their controlling player differs; only the UI projection
    // filters a normal SelectTeam to locally controlled drawables.
    peer.add_player(Player::new(1, Team::China, "Other", false));
    peer.process_player_create_team(1, 2, &[peer_ids[0]]);
    assert!(peer.process_player_select_team(1, 2));
    assert_eq!(peer.player_selected_objects(1), [peer_ids[0]]);

    peer.reset();
    assert_eq!(owner.player_hotkey_squad_ids(0, 3), owner_ids);
    assert_eq!(restored.player_selected_objects(0), owner_ids);
}
