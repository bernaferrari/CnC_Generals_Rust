use gamelogic::common::Relationship;
use generals_main::game_logic::host_booby_trap::HostBoobyTrapRegistry;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use generals_main::save_load::snapshot::{apply_pending_player_team_chunks, stamp_player_team_chunks};
use glam::Vec3;

const TRAP_SOURCE: &str = "Object BoobyTrap\n KindOf = IMMOBILE BOOBY_TRAP\n Body = ActiveBody Body\n MaxHealth = 37\n InitialHealth = 19\n End\n Behavior = StickyBombUpdate Sticky\n  OffsetZ = 17\n End\n UnitSpecificSounds\n  StickyBombCreated = StickyCreated\n End\nEnd\nObject BoobyTrapWithoutUpdate\n KindOf = IMMOBILE BOOBY_TRAP\n Body = ActiveBody Body\n MaxHealth = 1\n InitialHealth = 1\nEnd\nEnd\nObject TrapPlanter\n KindOf = INFANTRY SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\nEnd\nEnd\nObject TrapTarget\n KindOf = STRUCTURE IMMOBILE SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\nEnd\nEnd\nObject TrapMobileTarget\n KindOf = STRUCTURE SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\nEnd\nEnd\nObject TrapVictim\n KindOf = INFANTRY SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\nEnd\nEnd\n";

fn world() -> (GameLogic, ObjectId, ObjectId) {
    world_with_positions(Vec3::ZERO, Vec3::new(8.0, 0.0, 0.0))
}

fn world_with_positions(
    planter_position: Vec3,
    target_position: Vec3,
) -> (GameLogic, ObjectId, ObjectId) {
    let mut game = GameLogic::new();
    game.admit_object_ini_sources([("BoobyTrapChargeFidelity.ini".into(), TRAP_SOURCE.into())])
        .expect("admit typed charge and host templates");
    game.start_new_game(GameMode::SinglePlayer);

    let mut planter = Player::new(0, Team::GLA, "GLA", false);
    planter.set_map_relationship(1, Relationship::Enemies);
    planter.set_map_relationship(2, Relationship::Allies);
    let mut target_owner = Player::new(1, Team::USA, "USA", false);
    target_owner.set_map_relationship(0, Relationship::Enemies);
    let mut same_player_alliance_other_faction = Player::new(2, Team::USA, "USA2", false);
    same_player_alliance_other_faction.set_map_relationship(0, Relationship::Allies);
    game.add_player(planter);
    game.add_player(target_owner);
    game.add_player(same_player_alliance_other_faction);

    let planter_id = game
        .create_object_for_player("TrapPlanter", 0, planter_position)
        .expect("planter");
    let target_id = game
        .create_object_for_player("TrapTarget", 1, target_position)
        .expect("target");
    (game, planter_id, target_id)
}

fn move_into_same_named_team(game: &mut GameLogic, member: ObjectId) -> u32 {
    let mut chunks = stamp_player_team_chunks(game).expect("capture Team continuation");
    let teams = chunks.teams.as_mut().unwrap();
    let source_index = teams
        .teams
        .iter()
        .position(|team| team.members.as_ref().is_some_and(|members| members.contains(&member.0)))
        .expect("member's current Team");
    let mut sibling = teams.teams[source_index].clone();
    let sibling_id = teams.unique_team_id;
    teams.unique_team_id += 1;
    sibling.team_id = sibling_id;
    sibling.members = Some(vec![member.0]);
    teams.teams[source_index]
        .members
        .as_mut()
        .unwrap()
        .retain(|&id| id != member.0);
    teams.teams.push(sibling);
    apply_pending_player_team_chunks(game, &chunks).expect("install same-name Team sibling");
    sibling_id
}

#[test]
fn public_sticky_bomb_init_uses_each_worlds_pose_owner_and_authored_sound() {
    let (mut first, first_planter, first_target) =
        world_with_positions(Vec3::new(31.0, 12.0, 9.0), Vec3::new(8.0, 0.0, 0.0));
    let (mut peer, peer_planter, peer_target) =
        world_with_positions(Vec3::new(77.0, 20.0, 14.0), Vec3::new(8.0, 0.0, 0.0));
    assert_eq!((first_planter, first_target), (peer_planter, peer_target));
    let first_exact_team = move_into_same_named_team(&mut first, first_planter);
    let peer_exact_team = move_into_same_named_team(&mut peer, peer_planter);
    first.queued_audio_events.drain_requests();
    peer.queued_audio_events.drain_requests();

    let first_charge = first
        .spawn_booby_trap_special_object(first_planter, Team::GLA, first_target)
        .expect("typed StickyBombUpdate charge");
    let peer_charge = peer
        .spawn_booby_trap_special_object(peer_planter, Team::GLA, peer_target)
        .expect("typed StickyBombUpdate charge in peer world");
    assert_eq!(first_charge, peer_charge);

    let first_position = first.host_object(first_charge).unwrap().get_position();
    let peer_position = peer.host_object(peer_charge).unwrap().get_position();
    assert_eq!((first_position.x, first_position.z), (31.0, 9.0));
    assert_eq!((peer_position.x, peer_position.z), (77.0, 14.0));
    assert_eq!(
        first.host_object(first_charge).unwrap().owner_player_id,
        Some(0)
    );
    assert_eq!(
        first.host_object(first_charge).unwrap().team_instance_name,
        first.host_object(first_planter).unwrap().team_instance_name,
        "charge joins the planter's exact Team instance"
    );
    let first_teams = stamp_player_team_chunks(&first).unwrap();
    assert!(first_teams.teams.unwrap().teams.iter().any(|team| {
        team.team_id == first_exact_team
            && team.members.as_ref().is_some_and(|members| {
                members.contains(&first_planter.0) && members.contains(&first_charge.0)
            })
    }), "charge joins the exact same-name Team instance as its planter");
    assert_eq!(
        first.host_object(first_charge).unwrap().experience_sink,
        Some(first_planter),
        "SpecialAbilityUpdate sinks charge XP to its planter"
    );
    assert_eq!(
        peer.host_object(peer_charge).unwrap().owner_player_id,
        Some(0)
    );
    assert_eq!(
        peer.host_object(peer_charge).unwrap().team_instance_name,
        peer.host_object(peer_planter).unwrap().team_instance_name,
        "peer charge uses the peer world's planter Team"
    );
    let peer_teams = stamp_player_team_chunks(&peer).unwrap();
    assert!(peer_teams.teams.unwrap().teams.iter().any(|team| {
        team.team_id == peer_exact_team
            && team.members.as_ref().is_some_and(|members| {
                members.contains(&peer_planter.0) && members.contains(&peer_charge.0)
            })
    }), "peer charge joins its receiving world's exact Team instance");
    assert_eq!(
        peer.host_object(peer_charge).unwrap().experience_sink,
        Some(peer_planter)
    );
    assert_eq!(
        first.host_object(first_charge).unwrap().health.maximum,
        37.0
    );
    assert_eq!(
        first.host_object(first_charge).unwrap().health.current,
        19.0
    );
    assert_eq!(peer.host_object(peer_charge).unwrap().health.maximum, 37.0);
    assert_eq!(peer.host_object(peer_charge).unwrap().health.current, 19.0);
    assert!(
        first
            .host_object(first_target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(
        peer.host_object(peer_target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );

    let first_audio = first.queued_audio_events.drain_requests();
    let peer_audio = peer.queued_audio_events.drain_requests();
    assert_eq!(first_audio.len(), 1);
    assert_eq!(first_audio[0].event_type, "StickyCreated");
    assert_eq!(first_audio[0].object_id, None);
    assert_eq!(first_audio[0].position, Some(first_position));
    assert_eq!(peer_audio.len(), 1);
    assert_eq!(peer_audio[0].event_type, "StickyCreated");
    assert_eq!(peer_audio[0].object_id, None);
    assert_eq!(peer_audio[0].position, Some(peer_position));

    let first_mobile_target = first
        .create_object_for_player("TrapMobileTarget", 1, Vec3::new(120.0, 5.0, 40.0))
        .expect("mobile target");
    let peer_mobile_target = peer
        .create_object_for_player("TrapMobileTarget", 1, Vec3::new(120.0, 5.0, 40.0))
        .expect("peer mobile target");
    assert_eq!(first_mobile_target, peer_mobile_target);
    let first_mobile_charge = first
        .spawn_booby_trap_special_object(first_planter, Team::GLA, first_mobile_target)
        .expect("mobile target charge");
    let peer_mobile_charge = peer
        .spawn_booby_trap_special_object(peer_planter, Team::GLA, peer_mobile_target)
        .expect("peer mobile target charge");
    assert_eq!(first_mobile_charge, peer_mobile_charge);
    assert_eq!(
        first
            .host_object(first_mobile_charge)
            .unwrap()
            .get_position(),
        Vec3::new(120.0, 22.0, 40.0),
        "mobile target placement uses receiving StickyBombUpdate OffsetZ"
    );
    assert_eq!(
        peer.host_object(peer_mobile_charge).unwrap().get_position(),
        Vec3::new(120.0, 22.0, 40.0)
    );
}

fn install_public_registry(
    game: &mut GameLogic,
    planter: ObjectId,
    target: ObjectId,
    charge: Option<ObjectId>,
) {
    let mut registry = HostBoobyTrapRegistry::new();
    registry.install(target, planter, Team::GLA, 0, 8.0, charge);
    game.restore_booby_traps(registry, charge.is_some() as u32);
    game.host_object_mut(target)
        .expect("target")
        .set_status_booby_trapped(true);
}

#[test]
fn charge_detonation_is_isolated_between_worlds_with_matching_ids() {
    let (mut first, first_planter, first_target) = world();
    let (mut peer, peer_planter, peer_target) = world();
    assert_eq!((first_planter, first_target), (peer_planter, peer_target));
    let first_charge = first
        .spawn_booby_trap_special_object(first_planter, Team::GLA, first_target)
        .unwrap();
    let peer_charge = peer
        .spawn_booby_trap_special_object(peer_planter, Team::GLA, peer_target)
        .unwrap();
    assert_eq!(first_charge, peer_charge);
    install_public_registry(&mut first, first_planter, first_target, Some(first_charge));
    install_public_registry(&mut peer, peer_planter, peer_target, Some(peer_charge));
    let peer_health = peer.host_object(peer_target).unwrap().health.current;

    assert!(
        first.detonate_booby_trap_at(first_target, Vec3::new(8.0, 0.0, 0.0), None, false, true) > 0
    );
    assert_eq!(
        peer.host_object(peer_target).unwrap().health.current,
        peer_health
    );
    assert!(peer.host_object(peer_target).unwrap().status.booby_trapped);
    assert!(peer.host_object(peer_charge).unwrap().is_alive());
}

#[test]
fn saved_registry_metadata_does_not_repair_a_clear_status() {
    use generals_main::save_load::snapshot::{SnapshotBuilder, WorldSnapshot};

    let (mut source, planter, target) = world();
    install_public_registry(&mut source, planter, target, None);
    source
        .host_object_mut(target)
        .unwrap()
        .set_status_booby_trapped(false);
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded: WorldSnapshot = bincode_legacy::deserialize(&bytes).unwrap();
    let (mut restored, _, restored_target) = world();
    assert_eq!(target, restored_target);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(restored.booby_trap_residual().is_booby_trapped(target));
    assert!(!restored.host_object(target).unwrap().status.booby_trapped);
}

#[test]
fn public_booby_blast_damages_player_allied_splash_victim_across_factions() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED"),
        "StickyBombUpdate initialization owns the target status transition"
    );
    install_public_registry(&mut game, planter, target, Some(charge));
    assert_eq!(
        game.host_object(charge).unwrap().producer_id,
        Some(target),
        "StickyBombUpdate producer is the trapped target"
    );

    let allied_other_faction = game
        .create_object_for_player("TrapVictim", 2, Vec3::new(18.0, 0.0, 0.0))
        .expect("same player-alliance, different faction victim");
    let allied_health = game
        .host_object(allied_other_faction)
        .unwrap()
        .health
        .current;
    let target_health = game.host_object(target).unwrap().health.current;
    let hits = game.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true);

    assert!(
        hits > 0,
        "the admitted charge detonates through the public path"
    );
    assert!(game.host_object(target).unwrap().health.current < target_health);
    assert!(
        game.host_object(allied_other_faction)
            .unwrap()
            .health
            .current
            < allied_health,
        "C++ StickyBombUpdate applies damage to every in-range object"
    );
    assert!(
        !game
            .host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn public_booby_spawn_rejects_missing_target_without_creating_charge() {
    let (mut game, planter, target) = world();
    let objects_before = game.host_objects().registered_len();
    let spawned_before = game.booby_trap_objects_spawned();
    assert_eq!(
        game.spawn_booby_trap_special_object(planter, Team::GLA, ObjectId(u32::MAX)),
        None
    );
    assert_eq!(game.host_objects().registered_len(), objects_before);
    assert_eq!(game.booby_trap_objects_spawned(), spawned_before);
    assert!(!game.host_object(target).unwrap().status.booby_trapped);
}

#[test]
fn public_booby_trigger_gate_uses_charge_owner_player_relationship() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    install_public_registry(&mut game, planter, target, Some(charge));
    let allied_trigger = game
        .create_object_for_player("TrapVictim", 2, Vec3::new(9.0, 0.0, 0.0))
        .expect("player-allied trigger with a different faction");
    let target_health = game.host_object(target).unwrap().health.current;

    assert_eq!(
        game.detonate_booby_trap_at(
            target,
            Vec3::new(8.0, 0.0, 0.0),
            Some(allied_trigger),
            false,
            false,
        ),
        0,
        "C++ rejects an allied trigger using the selected charge owner"
    );
    assert_eq!(
        game.host_object(target).unwrap().health.current,
        target_health
    );
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(game.host_object(charge).unwrap().is_alive());
}

#[test]
fn public_booby_trigger_gate_rejects_missing_trigger_object() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    install_public_registry(&mut game, planter, target, Some(charge));
    let target_health = game.host_object(target).unwrap().health.current;

    assert_eq!(
        game.detonate_booby_trap_at(
            target,
            Vec3::new(8.0, 0.0, 0.0),
            Some(ObjectId(u32::MAX)),
            false,
            false,
        ),
        0,
        "a supplied but missing trigger object fails closed"
    );
    assert_eq!(
        game.host_object(target).unwrap().health.current,
        target_health
    );
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
    assert!(game.host_object(charge).unwrap().is_alive());
}

#[test]
fn public_booby_trigger_uses_charge_player_to_team_even_for_defector() {
    let (mut game, planter, target) = world();
    let charge = game
        .spawn_booby_trap_special_object(planter, Team::GLA, target)
        .expect("authored StickyBombUpdate charge");
    install_public_registry(&mut game, planter, target, Some(charge));
    let enemy_trigger = game
        .create_object_for_player("TrapVictim", 1, Vec3::new(9.0, 0.0, 0.0))
        .expect("enemy trigger");
    game.host_object_mut(enemy_trigger)
        .unwrap()
        .begin_undetected_defection(0, 100, false);
    assert!(
        game.host_object(enemy_trigger)
            .unwrap()
            .is_undetected_defector()
    );
    let target_health = game.host_object(target).unwrap().health.current;

    assert!(
        game.detonate_booby_trap_at(
            target,
            Vec3::new(8.0, 0.0, 0.0),
            Some(enemy_trigger),
            false,
            false,
        ) > 0,
        "C++ uses the charge owner's relationship to the trigger's Team; the Object-level defector override does not make it allied"
    );
    assert!(game.host_object(target).unwrap().health.current < target_health);
    assert!(
        !game
            .host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}

#[test]
fn public_booby_charge_scan_keeps_nearest_invalid_and_rejects_missing_charge() {
    let (mut game, planter, target) = world();
    let valid_charge = game
        .create_object_for_player("BoobyTrap", 0, Vec3::new(12.0, 8.0, 0.0))
        .expect("authored StickyBombUpdate charge");
    game.host_object_mut(valid_charge).unwrap().producer_id = Some(target);
    let nearest_invalid = game
        .create_object_for_player("BoobyTrapWithoutUpdate", 0, Vec3::new(8.0, 8.0, 0.0))
        .expect("nearest charge without StickyBombUpdate");
    game.host_object_mut(nearest_invalid).unwrap().producer_id = Some(target);
    install_public_registry(&mut game, planter, target, Some(valid_charge));

    let health_before = game.host_object(target).unwrap().health.current;
    assert_eq!(
        game.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true),
        0,
        "nearest producer-matched charge lacks StickyBombUpdate"
    );
    assert_eq!(
        game.host_object(target).unwrap().health.current,
        health_before
    );
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );

    // A stale registry id cannot authorize damage when no eligible charge is
    // present in the object scan.
    game.host_object_mut(nearest_invalid)
        .unwrap()
        .status
        .destroyed = true;
    game.host_object_mut(valid_charge).unwrap().status.destroyed = true;
    let stale_target = game.host_object(target).unwrap().health.current;
    install_public_registry(&mut game, planter, target, Some(ObjectId(u32::MAX)));
    assert_eq!(
        game.detonate_booby_trap_at(target, Vec3::new(8.0, 0.0, 0.0), None, false, true),
        0,
        "missing actual charge is rejected despite stale registry metadata"
    );
    assert_eq!(
        game.host_object(target).unwrap().health.current,
        stale_target
    );
    assert!(
        game.host_object(target)
            .unwrap()
            .has_object_status_bit("BOOBY_TRAPPED")
    );
}
