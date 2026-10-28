use gamelogic::common::Relationship;
use generals_main::game_logic::host_booby_trap::HostBoobyTrapRegistry;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use glam::Vec3;

const TRAP_SOURCE: &str = "Object BoobyTrap\n KindOf = IMMOBILE BOOBY_TRAP\n Body = ActiveBody Body\n MaxHealth = 1\n InitialHealth = 1\n End\n Behavior = StickyBombUpdate Sticky\n End\nEnd\nObject BoobyTrapWithoutUpdate\n KindOf = IMMOBILE BOOBY_TRAP\n Body = ActiveBody Body\n MaxHealth = 1\n InitialHealth = 1\nEnd\nEnd\nObject TrapPlanter\n KindOf = INFANTRY SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\nEnd\nEnd\nObject TrapTarget\n KindOf = STRUCTURE SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 1000\n InitialHealth = 1000\nEnd\nEnd\nObject TrapVictim\n KindOf = INFANTRY SELECTABLE SCORE\n Body = ActiveBody Body\n MaxHealth = 100\n InitialHealth = 100\nEnd\nEnd\n";

fn world() -> (GameLogic, ObjectId, ObjectId) {
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
        .create_object_for_player("TrapPlanter", 0, Vec3::ZERO)
        .expect("planter");
    let target_id = game
        .create_object_for_player("TrapTarget", 1, Vec3::new(8.0, 0.0, 0.0))
        .expect("target");
    (game, planter_id, target_id)
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
