//! Actual residual hit producers preserve the live source-player score phase.
use super::*;
use crate::game_logic::kill_credit::KillScorePhase;
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use glam::Vec3;
fn setup() -> (GameLogic, ObjectId, ObjectId) {
    let source = "Object AttributionSource\nKindOf = VEHICLE SCORE\nIsTrainable = Yes\nBody = ActiveBody SourceBody\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = DestroyDie SourceDie\nEnd\nEnd\nObject AttributionVictim\nKindOf = VEHICLE SCORE\nBuildCost = 500\nExperienceValue = 25 25 25 25\nSkillPointValue = 25 25 25 25\nBody = ActiveBody VictimBody\nMaxHealth = 2\nInitialHealth = 2\nEnd\nBehavior = DestroyDie VictimDie\nEnd\nEnd\n";
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "residual-attribution.ini".into(),
                source.into(),
            )])
            .unwrap(),
        )
        .unwrap();
    for (id, team, alliance) in [
        (0, Team::USA, 1),
        (1, Team::USA, 1),
        (2, Team::GLA, 2),
        (3, Team::China, 1),
    ] {
        let mut player = Player::new(id, team, &format!("Owner{id}"), id == 0);
        player.alliance_team = alliance;
        player.cash_bounty_percent = 0.2;
        player.resources.supplies = 1000;
        world.add_player(player);
    }
    world.frame = 10;
    let source = world
        .create_object_for_player("AttributionSource", 0, Vec3::new(-1000., 0., 0.))
        .unwrap();
    let victim = world
        .create_object_for_player("AttributionVictim", 2, Vec3::new(1000., 0., 0.))
        .unwrap();
    (world, source, victim)
}
fn transfer(world: &mut GameLogic, id: ObjectId, owner: u32) {
    let old = world.objects[&id].team;
    let team = world.players[&owner].team;
    world.objects.get_mut(&id).unwrap().set_team_and_owner(
        team,
        Some(owner),
        &mut world.ownership_events,
    );
    world.on_capture_object_owned(id, old, team, true);
}
fn plan(world: &mut GameLogic, source: Option<ObjectId>, victim: ObjectId) {
    assert_eq!(
        world.apply_scud_area_at(Vec3::ZERO, source, Team::USA, true),
        (0, false)
    );
    assert_eq!(world.scud_poison_zones.active_count(), 1);
    assert_eq!(
        world.scud_poison_zones.active_zones()[0].source_team,
        Team::USA
    );
    world
        .objects
        .get_mut(&victim)
        .unwrap()
        .set_position(Vec3::ZERO);
}
fn tick(world: &mut GameLogic, victim: ObjectId) {
    world.update_scud_poison_zones();
    assert!(world.objects[&victim].status.on_die_started);
    assert!(world.objects[&victim].status.destroyed);
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == victim)
            .count(),
        1
    );
}
fn credited(world: &GameLogic, source: ObjectId, owner: u32) {
    for id in [0, 1, 3] {
        let expected = u32::from(id == owner);
        assert_eq!(
            world.players[&id].statistics.units_destroyed, expected,
            "owner {id}"
        );
        assert_eq!(
            world.players[&id].statistics.money_earned,
            expected * 100,
            "owner {id}"
        );
        assert_eq!(
            world.players[&id].resources.supplies,
            1000 + expected * 100,
            "owner {id}"
        );
        assert_eq!(
            world.players[&id].skill_points,
            (expected * 25) as i32,
            "owner {id}"
        );
    }
    assert_eq!(world.players[&2].statistics.units_lost, 1);
    if let Some(source) = world.host_object(source) {
        assert_eq!(source.experience.current, 25.);
    }
}
fn uncredited(world: &GameLogic) {
    for id in [0, 1, 3] {
        assert_eq!(world.players[&id].statistics.units_destroyed, 0);
        assert_eq!(world.players[&id].resources.supplies, 1000);
        assert_eq!(world.players[&id].skill_points, 0);
    }
    assert_eq!(
        world.players[&2].statistics.units_lost, 0,
        "no live damager means no scoreTheKill, including victim loss"
    );
}
#[test]
fn planned_scud_poison_uses_current_owner_not_captured_team() {
    for owner in [0, 1, 3] {
        let (mut world, source, victim) = setup();
        let (peer, peer_source, peer_victim) = setup();
        assert_eq!((source, victim), (peer_source, peer_victim));
        plan(&mut world, Some(source), victim);
        if owner != 0 {
            transfer(&mut world, source, owner);
        }
        tick(&mut world, victim);
        credited(&world, source, owner); // before physical removal, as C++.
        assert_eq!(
            world.objects[&victim].kill_score_phase,
            KillScorePhase::Complete
        );
        assert_eq!(
            world
                .objects_to_destroy
                .iter()
                .find(|event| event.id == victim)
                .unwrap()
                .killer,
            Some(world.players[&owner].team)
        );
        assert_eq!(world.cash_bounty.floating_texts.len(), 1);
        assert_eq!(world.cash_bounty.floating_texts[0].killer_id, source);
        assert_eq!(world.cash_bounty.floating_texts[0].victim_id, victim);
        world.mark_object_for_destruction(victim, Some(Team::USA));
        world.update_scud_poison_zones();
        credited(&world, source, owner);
        world.process_destroy_list();
        assert!(!world.objects.contains_key(&victim));
        credited(&world, source, owner);
        world.process_destroy_list();
        credited(&world, source, owner);
        assert_eq!(peer.objects[&peer_victim].health.current, 2.);
        assert_eq!(peer.objects[&peer_source].experience.current, 0.);
        uncredited(&peer);
    }
}
#[test]
fn missing_or_physically_removed_planned_source_never_uses_stale_team_credit() {
    for missing in [false, true] {
        let (mut world, source, victim) = setup();
        let source_id = if missing { ObjectId(999_999) } else { source };
        plan(&mut world, Some(source_id), victim);
        if !missing {
            world.destroy_object(source);
            world.process_destroy_list();
            assert!(!world.objects.contains_key(&source));
        }
        tick(&mut world, victim);
        assert_eq!(
            world
                .objects_to_destroy
                .iter()
                .find(|event| event.id == victim)
                .unwrap()
                .killer,
            None
        );
        uncredited(&world);
        world.process_destroy_list();
        uncredited(&world);
    }
}
#[test]
fn dead_but_queued_source_scores_before_its_earlier_physical_removal() {
    let (mut world, source, victim) = setup();
    plan(&mut world, Some(source), victim);
    world
        .apply_owned_damage(
            source,
            1000.,
            None,
            crate::game_logic::combat::DamageType::Unresistable,
            crate::game_logic::host_usa_pilot::HostDeathType::Normal,
            None,
            &crate::game_logic::object::DamageHitContext::default(),
        )
        .unwrap();
    assert!(!world.objects[&source].is_alive());
    assert_eq!(world.objects_to_destroy.front().unwrap().id, source);
    tick(&mut world, victim);
    credited(&world, source, 0);
    world.process_destroy_list();
    assert!(!world.objects.contains_key(&source));
    assert!(!world.objects.contains_key(&victim));
    credited(&world, source, 0);
}
#[test]
fn transferring_source_after_lethal_return_cannot_move_admitted_score() {
    let (mut world, source, victim) = setup();
    plan(&mut world, Some(source), victim);
    tick(&mut world, victim);
    credited(&world, source, 0);
    transfer(&mut world, source, 1);
    world.process_destroy_list();
    credited(&world, source, 0);
    assert_eq!(world.cash_bounty.floating_texts.len(), 1);
}
#[test]
fn immediate_comanche_and_scud_blasts_share_source_aware_score_order() {
    for comanche in [false, true] {
        let (mut world, source, victim) = setup();
        world
            .objects
            .get_mut(&victim)
            .unwrap()
            .set_position(Vec3::ZERO);
        transfer(&mut world, source, 1);
        let hit = if comanche {
            world.apply_comanche_rocket_pod_area_at(Vec3::ZERO, Some(source))
        } else {
            world.apply_scud_area_at(Vec3::ZERO, Some(source), Team::USA, false)
        };
        assert_eq!(hit, (1, true));
        credited(&world, source, 1);
        world.process_destroy_list();
        credited(&world, source, 1);
    }
}
#[test]
fn direct_deletion_and_a_late_manual_mark_never_invent_score_or_death() {
    let (mut world, _, victim) = setup();
    world.destroy_object(victim);
    world.mark_object_for_destruction(victim, Some(Team::USA));
    assert!(!world.objects[&victim].status.on_die_started);
    assert_eq!(world.objects_to_destroy.len(), 1);
    assert!(
        !world
            .objects_to_destroy
            .front()
            .unwrap()
            .needs_death_effects()
    );
    world.process_destroy_list();
    uncredited(&world);
}
#[test]
fn encoded_pre_drain_save_retains_admitted_credit_and_ordered_removals() {
    let (mut source_world, source, victim) = setup();
    plan(&mut source_world, Some(source), victim);
    tick(&mut source_world, victim);
    credited(&source_world, source, 0);
    source_world.destroy_object(source); // ordered [victim death, source direct].
    let expected = source_world.pending_object_removal_snapshot();
    assert_eq!(
        expected
            .iter()
            .map(|event| (event.id, event.after_death))
            .collect::<Vec<_>>(),
        [(victim, true), (source, false)]
    );
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source_world).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    assert_eq!(decoded.objects[&victim].health.current, 0.);
    let directory = tempfile::tempdir().unwrap();
    let mut manager = crate::save_load::SaveFileManager::with_save_directory(directory.path());
    manager.init().unwrap();
    let info = crate::save_load::SaveGameInfo {
        pristine_map_name: None,
        filename: "pending-score".into(),
        display_name: "pending-score".into(),
        description: String::new(),
        map_name: "AttributionFixture".into(),
        campaign_side: None,
        mission_number: None,
        save_date: std::time::UNIX_EPOCH,
        game_version: env!("CARGO_PKG_VERSION").into(),
        play_time: std::time::Duration::ZERO,
        difficulty: crate::save_load::GameDifficulty::Medium,
        save_type: crate::save_load::SaveFileType::Normal,
    };
    manager
        .save_game("pending-score", &source_world, &info)
        .unwrap();
    let (mut restored, _, _) = setup();
    manager.load_game("pending-score", &mut restored).unwrap();
    assert_eq!(
        restored
            .pending_object_removal_snapshot()
            .iter()
            .map(|event| (event.id, event.after_death))
            .collect::<Vec<_>>(),
        [(victim, true), (source, false)]
    );
    assert_eq!(
        restored.objects[&victim].kill_score_phase,
        KillScorePhase::Complete
    );
    credited(&restored, source, 0);
    // Both real saved and uninterrupted owners complete the identical drain.
    for world in [&mut source_world, &mut restored] {
        world.process_destroy_list();
        world.process_destroy_list();
        assert!(!world.objects.contains_key(&source));
        assert!(!world.objects.contains_key(&victim));
        credited(world, source, 0);
    }
}
