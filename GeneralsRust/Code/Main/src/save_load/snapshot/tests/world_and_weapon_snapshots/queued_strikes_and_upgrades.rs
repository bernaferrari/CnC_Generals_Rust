//! Behavior suite extracted from the original test module.
use super::super::*;

#[test]
fn special_power_daisy_cutter_mid_flight_save_load_still_impacts() {
    use crate::command_system::SpecialPowerType;

    let mut source = GameLogic::new();
    ensure_strike_test_tank(&mut source);

    let caster_id = source
        .create_object("StrikeTestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("caster");
    let enemy_id = source
        .create_object("StrikeTestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = source.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 500.0;
        enemy.health.maximum = 500.0;
        enemy.template_mut().armor = 0.0;
    }

    // Activate at frame 0 → DaisyCutter impact at frame 90.
    source.set_current_frame(0);
    let strike_id = source
        .queue_special_power_strike(
            &SpecialPowerType::DaisyCutter,
            caster_id,
            Vec3::new(40.0, 0.0, 0.0),
        )
        .expect("DaisyCutter must queue");

    // Mid-flight: save before impact.
    source.set_current_frame(45);
    source.update_special_power_strikes();
    assert_eq!(
        source.special_power_strikes().pending_count(),
        1,
        "strike must still be queued mid-flight"
    );
    assert!(
        source
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::DaisyCutter)
    );
    let health_mid = source.host_object(enemy_id).unwrap().health.current;
    assert!((health_mid - 500.0).abs() < 0.1, "no damage mid-flight");

    // Combat particle residual from activation should be present for snapshot.
    assert!(
        source.combat_particles().system_count() >= 1,
        "activation should spawn combat particle residual"
    );

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot mid-flight DaisyCutter");
    assert_eq!(snapshot.special_power_strikes.strikes.len(), 1);
    assert_eq!(
        snapshot.special_power_strikes.strikes[0].phase,
        HostStrikePhase::Queued
    );
    assert_eq!(snapshot.special_power_strikes.strikes[0].impact_frame, 90);
    assert!(
        !snapshot.combat_particles.systems.is_empty(),
        "combat particles must be captured in WorldSnapshot"
    );

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore mid-flight DaisyCutter");

    assert_eq!(restored.get_current_frame(), 45);
    assert_eq!(restored.special_power_strikes().pending_count(), 1);
    let restored_strike = restored
        .special_power_strikes()
        .get(strike_id)
        .expect("pending strike must survive load");
    assert_eq!(restored_strike.impact_frame, 90);
    assert_eq!(restored_strike.phase, HostStrikePhase::Queued);
    assert!(
        restored.combat_particles().system_count() >= 1,
        "combat particle registry must restore active systems"
    );

    // Still before impact after load: no damage.
    restored.set_current_frame(89);
    restored.update_special_power_strikes();
    assert!((restored.host_object(enemy_id).unwrap().health.current - 500.0).abs() < 0.1);
    assert!(
        !restored
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::DaisyCutter)
    );

    // Impact after remaining delay: damage applied.
    restored.set_current_frame(90);
    restored.update_special_power_strikes();
    assert!(
        restored
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::DaisyCutter),
        "DaisyCutter must complete after mid-flight load"
    );
    let enemy_after = restored.host_object(enemy_id).map(|o| o.health.current);
    assert!(
        enemy_after.is_none()
            || enemy_after == Some(0.0)
            || restored
                .host_object(enemy_id)
                .map(|o| o.status.destroyed || o.health.current < 500.0)
                .unwrap_or(true),
        "enemy must take DaisyCutter residual damage after load (got {enemy_after:?})"
    );
    let completed = restored
        .special_power_strikes()
        .get(strike_id)
        .expect("completed strike");
    assert_eq!(completed.phase, HostStrikePhase::Completed);
    assert!(completed.total_damage_applied > 0.0);
    assert!(completed.objects_hit >= 1);
}

#[test]
fn special_power_a10_mid_flight_save_load_still_impacts() {
    use crate::command_system::SpecialPowerType;

    let mut source = GameLogic::new();
    ensure_strike_test_tank(&mut source);

    let caster_id = source
        .create_object("StrikeTestTank", Team::USA, Vec3::ZERO)
        .expect("caster");
    let enemy_id = source
        .create_object("StrikeTestTank", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
        .expect("enemy");
    {
        let enemy = source.host_object_mut(enemy_id).expect("enemy");
        enemy.health.current = 200.0;
        enemy.health.maximum = 200.0;
        enemy.template_mut().armor = 0.0;
    }

    // A10 delay is 60 frames.
    source.set_current_frame(100);
    let strike_id = source
        .queue_special_power_strike(
            &SpecialPowerType::Airstrike,
            caster_id,
            Vec3::new(15.0, 0.0, 0.0),
        )
        .expect("A10 must queue");
    assert_eq!(
        source
            .special_power_strikes()
            .get(strike_id)
            .unwrap()
            .impact_frame,
        160
    );

    source.set_current_frame(130);
    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("A10 mid-flight snapshot");

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("A10 restore");

    assert_eq!(restored.get_current_frame(), 130);
    assert!(
        restored
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::A10Strike)
    );

    restored.set_current_frame(159);
    restored.update_special_power_strikes();
    assert!((restored.host_object(enemy_id).unwrap().health.current - 200.0).abs() < 0.1);

    restored.set_current_frame(160);
    restored.update_special_power_strikes();
    assert!(
        restored
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::A10Strike),
        "A10 must complete after mid-flight load"
    );
    let health = restored
        .host_object(enemy_id)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        health < 200.0 || restored.host_object(enemy_id).is_none(),
        "A10 residual damage must apply post-load (health={health})"
    );
}

#[test]
fn save_file_roundtrip_preserves_pending_special_power_strike() {
    use crate::command_system::SpecialPowerType;
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use std::time::{Duration, SystemTime};

    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("save manager init");

    let mut source = GameLogic::new();
    ensure_strike_test_tank(&mut source);
    let caster = source
        .create_object("StrikeTestTank", Team::USA, Vec3::ZERO)
        .expect("caster");
    let enemy = source
        .create_object("StrikeTestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
        .expect("enemy");
    {
        let e = source.host_object_mut(enemy).unwrap();
        e.health.current = 300.0;
        e.health.maximum = 300.0;
        e.template_mut().armor = 0.0;
    }
    source.set_current_frame(0);
    source
        .queue_special_power_strike(
            &SpecialPowerType::DaisyCutter,
            caster,
            Vec3::new(10.0, 0.0, 0.0),
        )
        .expect("queue");
    source.set_current_frame(30);

    let info = SaveGameInfo {
        pristine_map_name: None,
        filename: "special_power_strike_rt".to_string(),
        display_name: "Special Power Strike Roundtrip".to_string(),
        description: "residual pending strike save/load".to_string(),
        map_name: "ResidualMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    manager
        .save_game("special_power_strike_rt", &source, &info)
        .expect("save");

    let mut loaded = GameLogic::new();
    loaded.templates = source.templates.clone();
    manager
        .load_game("special_power_strike_rt", &mut loaded)
        .expect("load");

    assert_eq!(loaded.get_current_frame(), 30);
    assert_eq!(loaded.special_power_strikes().pending_count(), 1);
    loaded.set_current_frame(90);
    loaded.update_special_power_strikes();
    assert!(
        loaded
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::DaisyCutter),
        "file-loaded strike must complete"
    );
    let health = loaded
        .host_object(enemy)
        .map(|o| o.health.current)
        .unwrap_or(0.0);
    assert!(
        health < 300.0 || loaded.host_object(enemy).is_none(),
        "damage after file load (health={health})"
    );
}

#[test]
fn host_upgrade_capture_mid_flight_save_load_completes_unlock() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{
        HostUpgradeKind, HostUpgradePhase, UPGRADE_INFANTRY_CAPTURE,
    };

    let mut source = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    source.add_player(player);
    ensure_upgrade_test_templates(&mut source);

    let barracks_id = source
        .create_object("TestBarracks", Team::USA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("barracks");
    // Residual C++ CommandSet authorship: Object::canProduceUpgrade walks the
    // producer's CommandSet, so the synthetic barracks carries the retail
    // AmericaBarracks producer identity for the Capture button walk.
    source
        .host_object_mut(barracks_id)
        .expect("barracks")
        .set_command_set_override(Some("AmericaBarracks".into()));
    // Stand outside the barracks/building static path footprint so post-load
    // CaptureBuilding can A* (same live gap as 12-unit spawn sitting inside
    // the structure block). Upgrade residual is what this test persists.
    let captor_id = source
        .create_object("TestInfantry", Team::USA, Vec3::new(80.0, 0.0, 0.0))
        .expect("captor");
    let building_id = source
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");

    // Queue capture research; do NOT update yet (mid-flight residual window).
    source.set_current_frame(20);
    source.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_INFANTRY_CAPTURE.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    source.process_commands();

    assert!(
        source
            .get_player(0)
            .unwrap()
            .has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "player research queue must hold Capture mid-flight"
    );
    assert!(
        !source
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "must not unlock before research completes"
    );
    assert_eq!(source.host_upgrades().pending_count(), 1);
    assert!(
        source
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CaptureBuilding),
        "host residual must record pending Capture research"
    );

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&source)
        .expect("snapshot mid-flight Capture upgrade");
    assert_eq!(snapshot.host_upgrades.entries.len(), 1);
    assert_eq!(
        snapshot.host_upgrades.entries[0].phase,
        HostUpgradePhase::Queued
    );
    assert_eq!(
        snapshot.host_upgrades.entries[0].kind,
        HostUpgradeKind::CaptureBuilding
    );
    assert!(
        snapshot.players.iter().any(|p| p
            .research_queue
            .iter()
            .any(|n| n.contains("Capture") || n == UPGRADE_INFANTRY_CAPTURE)),
        "player research_queue must also capture in-flight upgrade"
    );

    let mut restored = GameLogic::new();
    restored.templates = source.templates.clone();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore mid-flight Capture upgrade");

    assert_eq!(restored.get_current_frame(), 20);
    assert_eq!(restored.host_upgrades().pending_count(), 1);
    assert!(
        restored
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CaptureBuilding),
        "host registry pending Capture must survive load"
    );
    assert!(
        restored
            .get_player(0)
            .unwrap()
            .has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "player queued upgrade must survive load"
    );
    assert!(
        !restored
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "must still be mid-research after load"
    );

    // C++ Upgrade.ini BuildTime=30s is 900 logic frames at 30 FPS.  A single
    // update must preserve the pending queue, not unlock it early.
    for _ in 0..899 {
        restored.update();
    }
    assert!(
        !restored
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "capture must remain locked before the authored 30-second duration"
    );
    restored.update();

    assert!(
        restored
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "capture unlock must complete after mid-flight load"
    );
    assert!(
        restored
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CaptureBuilding),
        "registry must record Capture complete after load"
    );
    assert!(
        restored.host_upgrades().honesty_capture_unlock_ok(),
        "capture unlock honesty after load"
    );
    assert!(
        restored
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::CaptureBuilding),
        "host path honesty for Capture after load"
    );
    // The 900-frame research interval intentionally advances the simulation
    // far beyond the original test's one-frame window.  Re-establish the
    // authored in-range capture setup before testing the now-unlocked ability;
    // this keeps the assertion about snapshot/research state independent of
    // autonomous movement during the elapsed research time.
    if let Some(captor) = restored.host_object_mut(captor_id) {
        captor.set_position(Vec3::ZERO);
        captor.target = None;
        captor.set_ai_state(AIState::Idle);
    }
    let captor = restored
        .host_object(captor_id)
        .expect("captor after complete");
    assert!(
        captor.has_upgrade_tag(UPGRADE_INFANTRY_CAPTURE),
        "captor must receive capture upgrade tag after post-load complete"
    );

    // Live-fixture residuals before the command (same shape as
    // capture_and_containment.rs): the authored capture reload elapsed during
    // the research window, and the local-player FOW gate needs the captor's
    // maintained sight of the target — a live game ran Object::look every
    // frame since spawn, a restored world must seed it explicitly.
    restored
        .host_object_mut(captor_id)
        .expect("captor ready")
        .set_special_power_ready_seconds(
            &crate::command_system::SpecialPowerType::RangerCaptureBuilding,
            0.0,
        );
    {
        let shroud_manager = restored.world_services.shroud();
        let mut shroud = shroud_manager.lock().expect("shroud");
        shroud.set_host_object_shroud_status(
            0,
            building_id.0,
            gamelogic::common::ObjectShroudStatus::Clear,
        );
        shroud.mark_host_object_seen(0, building_id.0);
    }

    // Ability now available.
    restored.queue_command(GameCommand {
        command_type: CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    restored.process_commands();
    let captor = restored
        .host_object(captor_id)
        .expect("captor after unlock");
    assert_eq!(
        captor.ai_state,
        AIState::Capturing,
        "CaptureBuilding must work after mid-flight save/load + complete"
    );
}

#[test]
fn save_file_roundtrip_preserves_pending_host_upgrade() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_INFANTRY_CAPTURE};
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
    use std::time::{Duration, SystemTime};

    let save_dir = tempfile::TempDir::new().expect("temp save dir");
    let mut manager = SaveFileManager::with_save_directory(save_dir.path());
    manager.init().expect("save manager init");

    let mut source = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    source.add_player(player);
    ensure_upgrade_test_templates(&mut source);
    let barracks = source
        .create_object("TestBarracks", Team::USA, Vec3::ZERO)
        .expect("barracks");
    // Residual C++ CommandSet authorship for the capture producer walk.
    source
        .host_object_mut(barracks)
        .expect("barracks")
        .set_command_set_override(Some("AmericaBarracks".into()));
    source.set_current_frame(5);
    source.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_INFANTRY_CAPTURE.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![barracks],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    source.process_commands();
    assert_eq!(source.host_upgrades().pending_count(), 1);

    let info = SaveGameInfo {
        pristine_map_name: None,
        filename: "host_upgrade_rt".to_string(),
        display_name: "Host Upgrade Roundtrip".to_string(),
        description: "residual pending upgrade save/load".to_string(),
        map_name: "ResidualMap".to_string(),
        campaign_side: None,
        mission_number: None,
        save_date: SystemTime::now(),
        game_version: env!("CARGO_PKG_VERSION").to_string(),
        play_time: Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type: SaveFileType::Normal,
    };
    manager
        .save_game("host_upgrade_rt", &source, &info)
        .expect("save");

    let mut loaded = GameLogic::new();
    loaded.templates = source.templates.clone();
    manager
        .load_game("host_upgrade_rt", &mut loaded)
        .expect("load");

    assert_eq!(loaded.get_current_frame(), 5);
    assert_eq!(loaded.host_upgrades().pending_count(), 1);
    assert!(
        loaded
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CaptureBuilding)
    );
    for _ in 0..900 {
        loaded.update();
    }
    assert!(
        loaded
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "file-loaded pending upgrade must complete"
    );
    assert!(
        loaded
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CaptureBuilding),
        "file-loaded registry must record complete"
    );
}
