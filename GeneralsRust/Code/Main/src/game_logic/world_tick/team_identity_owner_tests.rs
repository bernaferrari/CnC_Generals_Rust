//! CPP Player761/784: script playerName is independent of display text.
use super::*;
fn world(label: &str) -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([(
        "team-identity.ini".into(),
        "Object IdentityActor\nKindOf = INFANTRY\nBody = ActiveBody IdentityBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nEnd\n".into(),
    )]).unwrap()).unwrap();
    for id in [1, 2] {
        world.add_player(Player::new(id, Team::USA, label, id == 1));
    }
    world
}
#[test]
fn same_display_players_admit_distinct_actual_default_teams_and_members() {
    let mut owner = world("Same Display");
    let mut peer = world("Same Display");
    let a = owner
        .create_object_for_player("IdentityActor", 1, Vec3::ONE)
        .unwrap();
    let b = owner
        .create_object_for_player("IdentityActor", 2, Vec3::ONE)
        .unwrap();
    let peer_a = peer
        .create_object_for_player("IdentityActor", 1, Vec3::ONE)
        .unwrap();
    assert_eq!(a, peer_a);
    let a_team = owner.host_object(a).unwrap().team_instance_id.unwrap();
    let b_team = owner.host_object(b).unwrap().team_instance_id.unwrap();
    assert_ne!(a_team, b_team);
    let factory = owner.team_factory.lock().unwrap();
    for (player, object, team) in [(1, a, a_team), (2, b, b_team)] {
        let teams = factory.owned_team_instances_for_controller(player);
        assert_eq!(teams.len(), 1);
        let bound = teams[0].read().unwrap();
        assert_eq!(bound.get_id(), team);
        assert_eq!(bound.get_members(), &[object.0]);
    }
    drop(factory);
    let original = owner.default_host_team_instance_name(Some(1), Team::USA);
    owner.get_player_mut(1).unwrap().name = "Renamed Display".into();
    assert_eq!(
        owner.default_host_team_instance_name(Some(1), Team::USA),
        original
    );
    assert_eq!(peer.get_player(1).unwrap().name, "Same Display");
}
#[test]
fn authored_identity_and_default_team_survive_actual_encoded_restore() {
    use crate::save_load::{SnapshotBuilder, snapshot::decode_bincode_world_snapshot};
    let mut source = world("Same Display");
    let a = source
        .create_object_for_player("IdentityActor", 1, Vec3::ONE)
        .unwrap();
    let b = source
        .create_object_for_player("IdentityActor", 2, Vec3::ONE)
        .unwrap();
    let before = [a, b].map(|id| source.host_object(id).unwrap().team_instance_id);
    source.get_player_mut(1).unwrap().name = "Different Label".into();
    let builder = SnapshotBuilder::new();
    let saved = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&saved).unwrap();
    let decoded = decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = world("Receiving Labels");
    restored.clear_all_players();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert_eq!(
        [a, b].map(|id| restored.host_object(id).unwrap().team_instance_id),
        before
    );
    assert_eq!(
        restored.get_player(1).unwrap().script_identity(),
        Some("RustPlayer1")
    );
    assert_eq!(
        restored.get_player(2).unwrap().script_identity(),
        Some("RustPlayer2")
    );
    assert_eq!(restored.get_player(1).unwrap().name, "Different Label");
}
#[test]
fn exact_authored_identity_is_preserved_and_conflicting_controller_rejects_before_admission() {
    let mut world = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Same Display", true);
    let mut side = game_engine::common::dict::Dict::new();
    side.set_ascii_string(
        game_engine::common::well_known_keys::key_player_name(),
        "PlyrAmerica1",
    );
    player.apply_map_side_dict(&side, false, &mut Default::default());
    world.add_player(player);
    assert_eq!(
        world.default_host_team_instance_name(Some(1), Team::USA),
        "teamPlyrAmerica1"
    );
    let error = world
        .try_add_player(
            Player::new(2, Team::USA, "Same Display", false).with_script_identity("PlyrAmerica1"),
        )
        .unwrap_err();
    assert!(error.to_string().contains("another controller"));
    assert!(world.get_player(2).is_none());
    let factory = world.team_factory.lock().unwrap();
    assert_eq!(factory.list_team_prototypes().len(), 1);
    assert_eq!(factory.owned_team_instances_for_controller(1).len(), 1);
    assert!(factory.owned_team_instances_for_controller(2).is_empty());
}

#[test]
fn null_controller_is_not_reserved_neutral_default_team() {
    let mut world = GameLogic::new();
    assert_eq!(world.default_host_team_instance_name(None, Team::USA), "");
    assert_eq!(
        world.default_host_team_instance_name(Some(99), Team::USA),
        ""
    );
    let mut neutral = Player::new(0, Team::Neutral, "Neutral Label", false);
    let mut dict = game_engine::common::dict::Dict::new();
    dict.set_ascii_string(game_engine::common::well_known_keys::key_player_name(), "");
    neutral.apply_map_side_dict(&dict, false, &mut Default::default());
    world.add_player(neutral);
    assert_eq!(
        world.default_host_team_instance_name(Some(0), Team::Neutral),
        "team"
    );
}

#[test]
fn different_script_identity_cannot_replace_live_same_id_player_or_default_team() {
    let mut owner = world("Old Display");
    let peer = world("Old Display");
    let id = owner
        .create_object_for_player("IdentityActor", 1, Vec3::ONE)
        .unwrap();
    let expected = owner.live_draw_playback_identity(owner.host_object(id).unwrap());
    let team_id = owner.host_object(id).unwrap().team_instance_id.unwrap();
    let exact = owner
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(team_id)
        .unwrap();
    owner.get_player_mut(1).unwrap().resources.supplies = 123;
    let error = owner
        .try_add_player(
            Player::new(1, Team::USA, "New Display", true)
                .with_script_identity("DifferentIdentity"),
        )
        .unwrap_err();
    assert!(error.to_string().contains("different script identity"));
    assert_eq!(
        owner.get_player(1).unwrap().script_identity(),
        Some("RustPlayer1")
    );
    assert_eq!(owner.get_player(1).unwrap().resources.supplies, 123);
    assert_eq!(owner.get_player(1).unwrap().name, "Old Display");
    assert_eq!(
        owner.live_draw_playback_identity(owner.host_object(id).unwrap()),
        expected
    );
    let same = owner
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(team_id)
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&same, &exact));
    assert_eq!(same.read().unwrap().get_members(), [id.0]);
    assert!(
        owner
            .team_factory
            .lock()
            .unwrap()
            .find_team_prototype("teamDifferentIdentity")
            .is_none()
    );
    assert_eq!(peer.get_player(1).unwrap().name, "Old Display");
    assert!(peer.host_object(id).is_none());
}
#[test]
fn actual_civilian_dict_identity_precedes_default_team_but_deposit_remains_after_template_init() {
    use game_engine::common::{dict::Dict, well_known_keys::*};
    let mut owner = GameLogic::new();
    owner.set_session_starting_cash(10_000);
    let peer = GameLogic::new();
    let mut dict = Dict::new();
    dict.set_ascii_string(key_player_name(), "AuthoredCivilian");
    dict.set_ascii_string(key_player_faction(), "FactionCivilian");
    dict.set_unicode_string(key_player_display_name(), "Only Display");
    dict.set_int(key_player_start_money(), 41);
    owner.admit_retained_map_civilians(&[dict]);
    let player = owner
        .get_players()
        .values()
        .find(|player| player.script_identity() == Some("AuthoredCivilian"))
        .unwrap();
    let id = player.id;
    assert_eq!(player.resources.supplies, 10_041);
    assert_eq!(player.map_side.role, PlayerSideRole::Authored);
    assert_eq!(
        owner.default_host_team_instance_name(Some(id), Team::Neutral),
        "teamAuthoredCivilian"
    );
    let factory = owner.team_factory.lock().unwrap();
    let prototype = factory.find_team_prototype("teamAuthoredCivilian").unwrap();
    assert_eq!(prototype.get_owner_name().as_str(), "AuthoredCivilian");
    let teams = factory.find_team_instances("teamAuthoredCivilian");
    assert_eq!(teams.len(), 1);
    assert_eq!(
        teams[0].read().unwrap().get_controlling_player_id(),
        Some(id)
    );
    assert!(
        factory
            .find_team_prototype(&format!("teamRustPlayer{id}"))
            .is_none()
    );
    assert!(peer.get_players().is_empty());
}
