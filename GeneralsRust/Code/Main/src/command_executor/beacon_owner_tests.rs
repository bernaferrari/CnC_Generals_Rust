//! Actual command admission with reused ObjectIds in independent matches.
use super::*;
use crate::command_system::ModifierKeys;
use crate::game_logic::Player;
use std::time::SystemTime;

fn world() -> GameLogic {
    let mut logic = GameLogic::new();
    let mut local = Player::new(0, Team::USA, "Local", true);
    local.alliance_team = 1;
    let mut enemy = Player::new(1, Team::GLA, "Enemy", false);
    enemy.alliance_team = 2;
    logic.get_players_mut().insert(0, local);
    logic.get_players_mut().insert(1, enemy);
    logic
}
fn place(logic: &mut GameLogic, player: u32, text: &str) -> ObjectId {
    let command = GameCommand {
        command_type: CommandType::PlaceBeacon {
            location: Vec3::new(11., 0., 12.),
            text: text.into(),
        },
        player_id: player,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        CommandExecutor::new(logic, player)
            .execute_command(command)
            .unwrap(),
        CommandResult::Success
    );
    *logic.host_objects().registered_keys().next().unwrap()
}

fn authored_beacon_world(beacon_name: &str, maximum: i32) -> GameLogic {
    use crate::game_logic::PlayerTemplateIdentity;
    use game_engine::common::ini::INILoadType;
    use game_engine::common::rts::player_template::PlayerTemplateStore;

    let mut logic = GameLogic::new();
    logic
        .get_players_mut()
        .insert(0, Player::new(0, Team::USA, "Same ID", true));
    let mut definitions = PlayerTemplateStore::new_owned();
    definitions
        .apply_source(
            &format!(
                "PlayerTemplate BeaconOwner\n Side = America\n BeaconName = {beacon_name}\nEnd\n"
            ),
            INILoadType::Overwrite,
        )
        .unwrap();
    logic
        .admit_player_template_definitions(definitions)
        .unwrap();
    assert!(logic.bind_player_template_identity(
        0,
        PlayerTemplateIdentity {
            template_name: "BeaconOwner".into(),
            template_index: None,
        },
    ));
    logic.admit_max_beacons_per_player(maximum);
    logic
}

fn place_result(logic: &mut GameLogic, player: u32, x: f32, text: &str) -> CommandResult {
    CommandExecutor::new(logic, player)
        .execute_command(GameCommand {
            command_type: CommandType::PlaceBeacon {
                location: Vec3::new(x, 0., 12.),
                text: text.into(),
            },
            player_id: player,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: vec![],
            modifier_keys: ModifierKeys::default(),
        })
        .unwrap()
}

#[test]
fn authored_beacon_settings_and_templates_are_owned_by_each_world() {
    let mut one = authored_beacon_world("SignalGlyph", 1);
    let mut two = authored_beacon_world("RallyMarker", 2);

    let one_id = match place_result(&mut one, 0, 11., "one") {
        CommandResult::Success => *one.host_objects().registered_keys().next().unwrap(),
        result => panic!("first world rejected its beacon: {result:?}"),
    };
    let two_id = match place_result(&mut two, 0, 11., "two") {
        CommandResult::Success => *two.host_objects().registered_keys().next().unwrap(),
        result => panic!("second world rejected its beacon: {result:?}"),
    };
    assert_eq!(
        one_id, two_id,
        "sessions deliberately reuse the same object ID"
    );
    assert_eq!(
        one.host_object(one_id).unwrap().template_name,
        "SignalGlyph"
    );
    assert_eq!(
        two.host_object(two_id).unwrap().template_name,
        "RallyMarker"
    );
    assert!(host_object_is_beacon(&one, one_id));
    assert!(host_object_is_beacon(&two, two_id));

    assert_eq!(
        place_result(&mut one, 0, 21., "overflow"),
        CommandResult::InvalidCommand
    );
    assert_eq!(
        place_result(&mut two, 0, 21., "second"),
        CommandResult::Success
    );
    assert!(!host_object_is_beacon(&one, two_id));
    assert_eq!(host_beacon_caption(&one, one_id).as_deref(), Some("one"));
    assert_eq!(host_beacon_caption(&two, two_id).as_deref(), Some("two"));

    // Snapshot restoration preserves the receiving session's admitted content
    // rule, rather than copying a mutable cap from another world's save.
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&two).unwrap();
    let mut restored = authored_beacon_world("RallyMarker", 1);
    assert_eq!(
        place_result(&mut restored, 0, 31., "prototype"),
        CommandResult::Success
    );
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(restored.max_beacons_per_player, 1);

    one.reset();
    two.reset();
    assert_eq!(one.max_beacons_per_player, 1);
    assert_eq!(two.max_beacons_per_player, 2);
    assert!(!host_object_is_beacon(&one, one_id));
    assert!(!host_object_is_beacon(&two, two_id));
}

#[test]
fn beacon_commands_keep_same_id_caption_and_hidden_state_in_the_receiving_world() {
    let mut owner = world();
    let id = place(&mut owner, 1, "owner enemy");
    assert!(host_beacon_is_hidden(&owner, id));
    let mut peer = world();
    let peer_id = place(&mut peer, 0, "peer local");
    assert_eq!(id, peer_id);
    assert_eq!(
        host_beacon_caption(&owner, id).as_deref(),
        Some("owner enemy")
    );
    assert_eq!(
        host_beacon_caption(&peer, id).as_deref(),
        Some("peer local")
    );
    assert!(!host_beacon_is_hidden(&peer, id));
    assert!(host_beacon_position_is_hidden(
        &owner,
        Vec3::new(11., 0., 12.)
    ));
    assert!(!host_beacon_position_is_hidden(
        &peer,
        Vec3::new(11., 0., 12.)
    ));
    owner.reset();
    assert_eq!(host_beacon_caption(&owner, id), None);
    assert!(!host_beacon_is_hidden(&owner, id));
    assert_eq!(
        host_beacon_caption(&peer, id).as_deref(),
        Some("peer local")
    );
    drop(owner);
    assert!(!host_beacon_is_hidden(&peer, id));
}

#[test]
fn beacon_removal_discards_only_its_session_entry_and_reused_ids_start_clean() {
    let mut owner = world();
    let id = place(&mut owner, 0, "remove me");
    let mut peer = world();
    assert_eq!(place(&mut peer, 0, "keep me"), id);
    owner.destroy_object(id);
    owner.process_destroy_list();
    assert_eq!(host_beacon_caption(&owner, id), None);
    assert_eq!(host_beacon_caption(&peer, id).as_deref(), Some("keep me"));
    owner.reset();
    // Reset clears the roster and its client bookkeeping, without a global clear.
    assert_eq!(host_beacon_caption(&owner, id), None);
    assert_eq!(host_beacon_caption(&peer, id).as_deref(), Some("keep me"));
}

#[test]
fn beacon_pulse_deadline_is_strict_and_wraps_like_cpp_unsigned_int() {
    let mut owner = world();
    owner.frame = u32::MAX - 10;
    let id = place(&mut owner, 0, "pulse");
    assert!(!owner.beacon_client.pulse_due(id, 19, 30));
    assert!(owner.beacon_client.pulse_due(id, 20, 30));
    assert!(!owner.beacon_client.pulse_due(id, 50, 30));
    assert!(owner.beacon_client.pulse_due(id, 51, 30));
    let mut peer = world();
    assert_eq!(place(&mut peer, 0, "peer"), id);
    assert_eq!(host_beacon_caption(&owner, id).as_deref(), Some("pulse"));
    assert!(!peer.beacon_client.pulse_due(id, 30, 30));
    assert!(peer.beacon_client.pulse_due(id, 31, 30));
}

#[test]
fn encoded_roster_replacement_cannot_reuse_the_previous_beacon_entry() {
    use crate::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};
    let mut owner = world();
    let id = place(&mut owner, 1, "old hidden caption");
    let mut peer = world();
    assert_eq!(place(&mut peer, 0, "peer caption"), id);
    let builder = SnapshotBuilder::new();
    // Restore validates loaded team prototypes before clearing any roster.
    // Admit the same authored Enemy prototype, then save its empty roster.
    let mut source = world();
    let source_id = place(&mut source, 1, "source prototype");
    source.destroy_object(source_id);
    source.process_destroy_list();
    // Preserve the admitted prototype but exercise a valid empty roster whose
    // saved allocator restarts at one, as the replacement boundary permits.
    source.set_next_object_id_for_restore(id);
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    builder.restore_from_snapshot(&decoded, &mut owner).unwrap();
    assert!(owner.host_objects().registered_is_empty());
    assert_eq!(host_beacon_caption(&owner, id), None);
    assert!(!host_beacon_is_hidden(&owner, id));
    assert_eq!(place(&mut owner, 0, "new visible caption"), id);
    assert!(!host_beacon_is_hidden(&owner, id));
    assert_eq!(
        host_beacon_caption(&owner, id).as_deref(),
        Some("new visible caption")
    );
    assert_eq!(
        host_beacon_caption(&peer, id).as_deref(),
        Some("peer caption")
    );
    assert!(!host_beacon_is_hidden(&peer, id));
}

#[test]
fn actual_host_beacon_update_observes_strict_pulse_boundary_in_its_own_radar() {
    use game_engine::common::system::radar::RadarEventType;
    let mut owner = world();
    let id = place(&mut owner, 0, "owner");
    let mut peer = world();
    assert_eq!(place(&mut peer, 0, "peer"), id);
    let pulses = |world: &GameLogic| {
        world
            .radar()
            .get_active_events()
            .iter()
            .filter(|event| event.event_type == RadarEventType::BeaconPulse)
            .count()
    };
    let created = owner.frame;
    tick_live_beacon_client_updates(&mut owner);
    assert_eq!(pulses(&owner), 0);
    owner.frame = created + 30;
    tick_live_beacon_client_updates(&mut owner);
    assert_eq!(pulses(&owner), 0);
    owner.frame = created + 31;
    tick_live_beacon_client_updates(&mut owner);
    assert_eq!(pulses(&owner), 1);
    assert_eq!(pulses(&peer), 0);
    peer.frame = created + 31;
    tick_live_beacon_client_updates(&mut peer);
    assert_eq!(pulses(&peer), 1);
    assert_eq!(pulses(&owner), 1);
}
