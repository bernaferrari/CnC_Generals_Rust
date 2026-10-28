use super::*;

#[test]
fn canonical_default_identity_precedes_display_and_preserves_explicit_authored_names() {
    let mut first = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Visible Label", true);
    player.map_side.map_player_name = "player3".into();
    first.add_player(player);
    assert_eq!(
        first.default_host_team_instance_name(Some(1), Team::USA),
        "teamplayer3"
    );
    let mut second = GameLogic::new();
    let mut peer = Player::new(1, Team::USA, "Other Label", true);
    peer.map_side.map_player_name = "AuthoredOwner".into();
    second.add_player(peer);
    assert_eq!(
        second.default_host_team_instance_name(Some(1), Team::USA),
        "teamAuthoredOwner"
    );
    assert_eq!(
        first.default_host_team_instance_name(Some(1), Team::USA),
        "teamplayer3"
    );
    first.add_player(
        Player::new(2, Team::China, "  Pre Map  ", false).with_script_identity("Pre Map"),
    );
    assert_eq!(
        first.default_host_team_instance_name(Some(2), Team::China),
        "teamPre Map"
    );
    first.add_player(Player::new(4, Team::GLA, "", false).with_script_identity("GLA"));
    assert_eq!(
        first.default_host_team_instance_name(Some(4), Team::GLA),
        "teamGLA"
    );
    assert_eq!(
        first.default_host_team_instance_name(None, Team::Neutral),
        ""
    );
    let mut neutral = Player::new(9, Team::Neutral, "Neutral Display", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    first.add_player(neutral);
    assert_eq!(
        first.default_host_team_instance_name(Some(9), Team::Neutral),
        "team"
    );
}

#[test]
fn admitted_default_definition_keeps_canonical_owner_and_existing_authored_hooks() {
    let mut world = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Display Label", true);
    player.map_side.map_player_name = "player0".into();
    world.add_player(player);
    let exact = world
        .host_default_team_id(1)
        .expect("admitted default identity");
    {
        let mut factory = world.team_factory.lock().unwrap();
        let prototype = factory.find_team_prototype("teamplayer0").unwrap();
        assert_eq!(prototype.get_owner_name().as_str(), "player0");
        let mut authored = (*prototype).clone();
        authored.set_script_on_idle("KeepAuthoredIdle".into());
        factory.replace_team_prototype(authored);
    }
    // Repeated player/default admission preserves an existing authored table;
    // a default name is never permission to replace its configuration.
    world.ensure_host_default_team(1);
    assert_eq!(world.host_default_team_id(1), Some(exact));
    let factory = world.team_factory.lock().unwrap();
    let prototype = factory.find_team_prototype("teamplayer0").unwrap();
    assert_eq!(prototype.get_script_on_idle().as_str(), "KeepAuthoredIdle");
    assert_eq!(factory.prototype_names(), ["teamplayer0"]);
}

#[test]
fn ambiguous_default_team_name_never_becomes_an_arbitrary_player_binding() {
    let mut world = GameLogic::new();
    let mut player = Player::new(1, Team::USA, "Display Label", true);
    player.map_side.map_player_name = "player0".into();
    world.add_player(player);
    {
        let mut factory = world.team_factory.lock().unwrap();
        let mut prototype = (*factory.find_team_prototype("teamplayer0").unwrap()).clone();
        prototype.set_singleton(false);
        factory.replace_team_prototype(prototype);
        factory.create_inactive_team("teamplayer0").unwrap();
        factory.create_inactive_team("teamplayer0").unwrap();
    }
    world.get_player_mut(1).unwrap().default_team_id = None;

    world.ensure_host_default_team(1);

    assert_eq!(world.host_default_team_id(1), None);
    assert_eq!(world.get_player(1).unwrap().default_team_id, None);
}

#[test]
fn unknown_or_display_only_team_owner_uses_cpp_neutral_fallback() {
    use game_engine::common::well_known_keys::*;
    let mut world = GameLogic::new();
    let mut participant = Player::new(1, Team::USA, "Visible Label", true);
    participant.map_side.map_player_name = "player3".into();
    world.add_player(participant);
    let mut neutral = Player::new(9, Team::Neutral, "Neutral Display", false);
    neutral.map_side.role = PlayerSideRole::Neutral;
    world.add_player(neutral);
    world.install_owned_script_engine(gamelogic::scripting::engine::ScriptEngine::new().unwrap());
    for owner in ["MissingOwner", "Visible Label", ""] {
        let mut definition = Dict::new();
        definition.set_ascii_string(key_team_name(), "ReceivingTeam");
        definition.set_ascii_string(key_team_owner(), owner);
        definition.set_bool(key_team_is_singleton(), true);
        assert!(world.sync_legacy_team_factory_from_team_dicts(&[definition]));
        let mut factory = world.team_factory.lock().unwrap();
        assert_eq!(
            factory
                .find_team("ReceivingTeam")
                .unwrap()
                .read()
                .unwrap()
                .get_controlling_player_id(),
            Some(9),
            "CPP TeamFactory::initTeam falls back to neutral for unresolved script owner {owner:?}"
        );
    }
}
