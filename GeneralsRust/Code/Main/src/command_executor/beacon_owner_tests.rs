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
    *logic.host_objects().keys().next().unwrap()
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
    assert!(owner.host_objects().is_empty());
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
