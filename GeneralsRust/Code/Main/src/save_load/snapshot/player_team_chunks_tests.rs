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
            players: vec![PlayerRuntimePersist {
                player_id: 1,
                sciences: vec!["TMAI".into(), "FSGM".into()],
                ..Default::default()
            }],
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
