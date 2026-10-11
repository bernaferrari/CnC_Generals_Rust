//! Pure capsule checks: no fixture-wide mutex, registry reset or publication.
use super::*;
use crate::save_load::SnapshotBuilder;

fn snapshot() -> WorldSnapshot {
    SnapshotBuilder::new()
        .create_world_snapshot(&GameLogic::new())
        .unwrap()
}

fn raw_footer(world: &mut WorldSnapshot, bytes: &[u8]) {
    world.lifecycle_tail.clear();
    for b in bytes {
        world
            .lifecycle_tail
            .extend_from_slice(format!("{b:02x}").as_bytes());
    }
    let length = world.lifecycle_tail.len() as u32;
    world
        .lifecycle_tail
        .extend_from_slice(&length.to_le_bytes());
    world.lifecycle_tail.extend_from_slice(MAGIC);
}

#[test]
fn replace_capsule_preserves_prior_domains_and_hides_authored_tags() {
    let mut world = snapshot();
    world.lifecycle_tail = b"prior FSGM TMAI bytes".to_vec();
    let prefix = world.lifecycle_tail.clone();
    let chunks = PlayerTeamChunks {
        players: Some(PlayersChunkPersist {
            host_starting_cash_source: Some(HostStartingCashSource::GameInfo),
            players: vec![PlayerRuntimePersist {
                player_id: 1,
                sciences: vec!["TMAI".into(), "FSGM".into()],
                ..Default::default()
            }],
            ..Default::default()
        }),
        teams: None,
    };
    bind_chunks_to_world(&mut world, &chunks).unwrap();
    assert_eq!(chunks_from_world(&world).unwrap(), chunks);
    assert!(world.lifecycle_tail.starts_with(&prefix));
    let encoded = &world.lifecycle_tail[prefix.len()..world.lifecycle_tail.len() - 8];
    assert!(!encoded.windows(4).any(|w| w == b"TMAI" || w == b"FSGM"));
    bind_chunks_to_world(&mut world, &PlayerTeamChunks::default()).unwrap();
    assert_eq!(
        chunks_from_world(&world).unwrap(),
        PlayerTeamChunks::default()
    );
    assert_eq!(&world.lifecycle_tail[..prefix.len()], prefix);
}

#[test]
fn capsule_decoder_rejects_trailing_binary_bytes() {
    let mut world = snapshot();
    let mut encoded = bincode_legacy::serialize(&(None::<Vec<u8>>, None::<Vec<u8>>)).unwrap();
    encoded.push(7);
    raw_footer(&mut world, &encoded);
    assert!(chunks_from_world(&world).is_err());
}

#[test]
fn capsule_decoder_rejects_unbounded_length_and_invalid_option_without_allocating() {
    let mut world = snapshot();
    let mut bytes = vec![1];
    bytes.extend_from_slice(&u64::MAX.to_le_bytes());
    raw_footer(&mut world, &bytes);
    assert!(chunks_from_world(&world).is_err());
    raw_footer(&mut world, &[2, 0]);
    assert!(chunks_from_world(&world).is_err());
}

#[test]
fn capsule_decoder_rejects_truncated_footer_and_invalid_hex() {
    let mut world = snapshot();
    for bytes in [
        b"PTSC".as_slice(),
        b"00\xff\xff\xff\xffPTSC",
        b"z0\x02\0\0\0PTSC",
        b"0\x01\0\0\0PTSC",
    ] {
        world.lifecycle_tail = bytes.to_vec();
        assert!(chunks_from_world(&world).is_err(), "{bytes:?}");
    }
}

#[test]
fn rejected_footer_replacement_leaves_snapshot_bytes_unchanged() {
    let mut world = snapshot();
    world.lifecycle_tail = b"PTSC".to_vec();
    let before = world.lifecycle_tail.clone();
    assert!(bind_chunks_to_world(&mut world, &PlayerTeamChunks::default()).is_err());
    assert_eq!(world.lifecycle_tail, before);
}

#[test]
fn snapshot_without_capsule_has_no_implicit_players_or_teams() {
    let mut world = snapshot();
    world.lifecycle_tail = b"unrelated prior metadata".to_vec();
    assert_eq!(
        chunks_from_world(&world).unwrap(),
        PlayerTeamChunks::default()
    );
}

#[test]
fn complete_roster_requires_exact_unique_map_definitions() {
    let world = GameLogic::new();
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team("Definition".into(), "".into(), false, None)
        .unwrap();
    let chunks = stamp_from_live(&world).unwrap();
    validate_definitions(&world, &chunks).unwrap();
    let mut missing = chunks.clone();
    missing.teams.as_mut().unwrap().prototypes.clear();
    assert!(validate_definitions(&world, &missing).is_err());
    let mut no_id = chunks.clone();
    no_id.teams.as_mut().unwrap().prototypes[0].prototype_id = None;
    assert!(validate_definitions(&world, &no_id).is_err());
    let mut duplicate = chunks.clone();
    let p = duplicate.teams.as_ref().unwrap().prototypes[0].clone();
    duplicate.teams.as_mut().unwrap().prototypes.push(p);
    assert!(validate_definitions(&world, &duplicate).is_err());
    let mut historical = missing;
    historical.teams.as_mut().unwrap().persist_roster = false;
    validate_definitions(&world, &historical).unwrap();
}

#[test]
fn historical_named_relationship_resolves_before_destination_write_borrow() {
    let mut world = GameLogic::new();
    let (self_id, other_id) = {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team("Self".into(), "".into(), false, None)
            .unwrap();
        factory
            .init_team("Other".into(), "".into(), false, None)
            .unwrap();
        let a = factory
            .create_inactive_team("Self")
            .unwrap()
            .read()
            .unwrap()
            .get_id();
        let b = factory
            .create_inactive_team("Other")
            .unwrap()
            .read()
            .unwrap()
            .get_id();
        (a, b)
    };
    let mut chunks = stamp_from_live(&world).unwrap();
    let saved = chunks.teams.as_mut().unwrap();
    saved.persist_roster = false;
    for team in &mut saved.teams {
        team.members = None;
    }
    let team = saved
        .teams
        .iter_mut()
        .find(|t| t.team_id == self_id)
        .unwrap();
    team.team_relations = vec![
        TeamRelPersist {
            team_name: "Self".into(),
            team_id: 0,
            relationship: 2,
        },
        TeamRelPersist {
            team_name: "Other".into(),
            team_id: 0,
            relationship: 0,
        },
    ];
    apply_pending(&mut world, &chunks).unwrap();
    let factory = world.team_factory.lock().unwrap();
    let team = factory.find_team_by_id(self_id).unwrap();
    let relations = team.read().unwrap().team_relation_override_pairs();
    assert!(relations.contains(&(self_id, gamelogic::common::Relationship::Allies)));
    assert!(relations.contains(&(other_id, gamelogic::common::Relationship::Enemies)));
}

#[test]
fn side_identity_capsule_retains_roles_names_and_unresolved_start() {
    use super::super::player_team_persist::HostSideIdentityPersist;
    let mut world = snapshot();
    let roles = [
        (PlayerSideRole::Participant, "player3"),
        (PlayerSideRole::Neutral, ""),
        (PlayerSideRole::ReplayObserver, "ReplayObserver"),
        (PlayerSideRole::Authored, "PlyrCivilian"),
    ];
    let chunks = PlayerTeamChunks {
        players: Some(PlayersChunkPersist {
            host_starting_cash_source: Some(HostStartingCashSource::GameInfo),
            players: roles
                .into_iter()
                .enumerate()
                .map(|(id, (role, name))| PlayerRuntimePersist {
                    player_id: id as u32,
                    host_alliance_team: Some(-1),
                    host_side_identity: Some(HostSideIdentityPersist {
                        role,
                        authored_name: name.into(),
                        start_position: -1,
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        teams: None,
    };
    bind_chunks_to_world(&mut world, &chunks).unwrap();
    assert_eq!(chunks_from_world(&world).unwrap(), chunks);
}

#[test]
fn admission_cash_wire_distinguishes_zero_and_old_absence() {
    assert_eq!(
        parse_players_block(&[5, 0, 0]).unwrap().host_starting_cash,
        None
    );
    assert_eq!(
        parse_players_block(&[6, 0, 0, 0])
            .unwrap()
            .host_starting_cash,
        None
    );
    for cash in [0u32, 12_500, u32::MAX] {
        let mut payload = vec![6, 0, 0, 1];
        payload.extend_from_slice(&cash.to_le_bytes());
        assert_eq!(
            parse_players_block(&payload).unwrap().host_starting_cash,
            Some(cash)
        );
    }
}

#[test]
fn default_current_world_has_valid_owned_cash_metadata() {
    let saved = WorldSnapshot::default();
    let chunks = chunks_from_world(&saved).unwrap();
    assert_eq!(
        chunks.players.as_ref().unwrap().host_starting_cash,
        Some(crate::game_logic::Player::DEFAULT_STARTING_MONEY)
    );
    validate_roster(&saved, &chunks).unwrap();
}

#[test]
fn admission_cash_wire_rejects_invalid_presence_truncation_and_trailing_bytes() {
    for payload in [
        &[6, 0, 0, 2][..],
        &[6, 0, 0, 1],
        &[6, 0, 0, 1, 0, 0, 0],
        &[6, 0, 0, 0, 9],
    ] {
        assert!(parse_players_block(payload).is_err(), "{payload:?}");
    }
}

#[test]
fn missing_admission_cash_rejects_empty_world_before_restore_mutation() {
    let builder = SnapshotBuilder::new();
    let mut live = GameLogic::new();
    live.set_session_starting_cash(12_500);
    let mut saved = builder.create_world_snapshot(&GameLogic::new()).unwrap();
    let mut chunks = chunks_from_world(&saved).unwrap();
    chunks.players.as_mut().unwrap().host_starting_cash = None;
    bind_chunks_to_world(&mut saved, &chunks).unwrap();
    assert!(builder.restore_from_snapshot(&saved, &mut live).is_err());
    assert_eq!(live.skirmish_rules().starting_cash, 12_500);
    saved.version = 25;
    builder.restore_from_snapshot(&saved, &mut live).unwrap();
    assert_eq!(
        live.skirmish_rules().starting_cash,
        crate::game_logic::Player::DEFAULT_STARTING_MONEY
    );
}

#[test]
fn admission_capability_rejects_missing_malformed_and_duplicate_identities() {
    let mut source = GameLogic::new();
    source.add_player(crate::game_logic::Player::new(
        0,
        crate::game_logic::Team::USA,
        "Human",
        true,
    ));
    source.add_player(crate::game_logic::Player::new(
        3,
        crate::game_logic::Team::GLA,
        "AI",
        false,
    ));
    let world = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let original = chunks_from_world(&world).unwrap();
    assert!(validate_host_alliances(&world, &original).is_ok());
    for broken in 0..4 {
        let mut chunks = original.clone();
        let rows = &mut chunks.players.as_mut().unwrap().players;
        match broken {
            0 => rows[0].host_side_identity = None,
            1 => {
                let identity = rows[0].host_side_identity.as_mut().unwrap();
                identity.role = PlayerSideRole::ReplayObserver;
                identity.authored_name = "player0".into();
            }
            2 => {
                for row in rows {
                    row.host_side_identity.as_mut().unwrap().authored_name = "same-key".into();
                }
            }
            3 => {
                let identity = rows[0].host_side_identity.as_mut().unwrap();
                identity.role = PlayerSideRole::Neutral;
                identity.authored_name.clear();
            }
            _ => unreachable!(),
        }
        assert!(
            validate_host_alliances(&world, &chunks).is_err(),
            "broken identity case {broken}"
        );
    }
}

#[test]
fn admission_cash_wire_keeps_definition_base_distinct_from_fixed_game_info() {
    for base in [None, Some(0), Some(9_000), Some(u32::MAX)] {
        let mut payload = vec![7, 0, 0, 1];
        payload.extend_from_slice(&17_321u32.to_le_bytes());
        payload.push(u8::from(base.is_some()));
        if let Some(base) = base {
            payload.extend_from_slice(&base.to_le_bytes());
        }
        let parsed = parse_players_block(&payload).unwrap();
        assert_eq!(parsed.host_starting_cash, Some(17_321));
        assert_eq!(
            parsed.host_starting_cash_source,
            Some(base.map_or(
                HostStartingCashSource::GameInfo,
                HostStartingCashSource::Definitions
            ))
        );
    }
    assert_eq!(
        parse_players_block(&[6, 0, 0, 0])
            .unwrap()
            .host_starting_cash_source,
        None
    );
}

#[test]
fn admission_cash_wire_rejects_missing_or_invalid_definition_source() {
    for suffix in [vec![], vec![2], vec![1], vec![1, 0, 0, 0], vec![0, 9]] {
        let mut payload = vec![7, 0, 0, 1];
        payload.extend_from_slice(&17_321u32.to_le_bytes());
        payload.extend_from_slice(&suffix);
        assert!(parse_players_block(&payload).is_err(), "{payload:?}");
    }
}

#[test]
fn current_envelope_rejects_historical_players_without_cash_source() {
    let mut payload = vec![6, 0, 0, 1];
    payload.extend_from_slice(&17_321u32.to_le_bytes());
    let chunks = PlayerTeamChunks {
        players: Some(parse_players_block(&payload).unwrap()),
        teams: None,
    };
    let mut saved = WorldSnapshot::default();
    assert!(validate_host_alliances(&saved, &chunks).is_err());
    saved.version = 26;
    validate_host_alliances(&saved, &chunks).unwrap();
}
