//! Behavior suite extracted from `combat_particles_and_economy`.
use super::*;

#[test]
fn capture_building_upgrade_queue_complete_unlocks_capture_ability() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_INFANTRY_CAPTURE};

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    ensure_test_barracks_template(&mut game_logic);
    // C++ retail ranger-style capture module: `StartsPaused = Yes` +
    // `UnpauseSpecialPowerUpgrade` (parsed as capture_starts_paused /
    // capture_upgrade_trigger).  Without it the capture power is ready
    // before the research completes, and ActionManager::canCaptureBuilding
    // (ActionManager.cpp:971-1069, getPercentReady < 1.0) would accept it.
    let infantry_template = game_logic
        .templates
        .get_mut("TestInfantry")
        .expect("TestInfantry template");
    infantry_template.capture_starts_paused = true;
    infantry_template.capture_upgrade_trigger =
        Some(UPGRADE_INFANTRY_CAPTURE.to_string());
    // Retail infantry authors SightRange/ShroudClearingRange; without a
    // non-zero look radius the captor registers no look and the building
    // stays shrouded for the FOW gate.
    infantry_template.sight_range = 200.0;
    let mut barracks = ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .set_health(1000.0)
        .set_cost(600, -1);
    barracks.capturable = true;
    game_logic
        .templates
        .insert("AmericaBarracks".to_string(), barracks);

    let barracks_id = game_logic
        .create_object("AmericaBarracks", Team::USA, Vec3::new(-50.0, 0.0, 0.0))
        .expect("barracks");
    assert!(
        game_logic
            .host_object(barracks_id)
            .map(|b| b.building_data.is_some() && b.is_constructed())
            .unwrap_or(false),
        "barracks must be a constructed producer for QueueUpgrade"
    );

    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(12.0, 0.0, 0.0))
        .expect("captor");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");

    // C++ ActionManager::canCaptureBuilding runs isObjectShroudedForAction
    // (ActionManager.cpp:76-102) — the local player must see the target. The
    // host consults the shared shroud manager, so this fixture registers the
    // captor's look and refreshes vision before issuing CaptureBuilding (a
    // live game would have run Object::look every frame since spawn).
    game_logic.host_object_mut(captor_id).expect("captor").vision_range = 200.0;
    // The shroud look radius (Object::getShroudClearingRange) is what
    // registers the look; vision_range alone does not.
    game_logic.host_object_mut(captor_id).expect("captor").shroud_range = 200.0;
    game_logic.update_main_crate_vision();
    // A C++ match initializes the shroud grid at map load; without it every
    // look computes Hidden and the FOW gate would refuse the capture.
    gamelogic::system::shroud_manager::get_shroud_manager()
        .lock()
        .expect("shroud")
        .init_shroud_grid(512.0, 512.0);
    game_logic.update_main_crate_vision();
    game_logic.queue_command(GameCommand {
        command_type: CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    let captor = game_logic.host_object(captor_id).expect("captor");
    assert_ne!(captor.ai_state, AIState::Capturing);
    assert_ne!(captor.target, Some(building_id));

    // Queue capture research from barracks.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_INFANTRY_CAPTURE.to_string(),
        },
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let player = game_logic.get_player(0).expect("player");
    assert!(
        player.has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "capture upgrade must be queued after QueueUpgrade"
    );
    assert!(
        !player.has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "must not unlock before research completes"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CaptureBuilding),
        "host residual must record pending Capture research"
    );

    // C++ research advances on the producer's Upgrade.ini BuildTime; one
    // 1/30s frame must not instant-complete 30 seconds of research.
    game_logic.update();
    game_logic.update_with_dt(30.0);

    let player = game_logic.get_player(0).expect("player after complete");
    assert!(
        !player.has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "queue must clear on complete"
    );
    assert!(
        player.has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
        "player unlock flag must be set"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::CaptureBuilding),
        "registry must record Capture complete"
    );
    assert!(
        game_logic.host_upgrades().honesty_capture_unlock_ok(),
        "capture unlock honesty"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::CaptureBuilding),
        "host path honesty for Capture"
    );

    // Infantry should carry upgrade tag after complete.
    let captor = game_logic.host_object(captor_id).expect("captor tagged");
    assert!(
        captor.has_upgrade_tag(UPGRADE_INFANTRY_CAPTURE),
        "captor must receive capture upgrade tag"
    );

    // The unpaused capture special keeps its authored ctor ReloadTime
    // (retail RangerCaptureBuilding reload = 15000ms); the research window
    // above lets it elapse — model that with the C++ setReadyFrame(0)
    // residual so the power is ready for the final command.
    game_logic
        .host_object_mut(captor_id)
        .expect("captor ready")
        .set_special_power_ready_seconds(
            &crate::command_system::SpecialPowerType::RangerCaptureBuilding,
            0.0,
        );

    // The research window's vision pass does not retain the captor's
    // Register the captor's maintained sight of the target the way the live
    // vision pass would (fixture style of scripts_and_capture.rs:2392).
    {
        let mut shroud = gamelogic::system::shroud_manager::get_shroud_manager()
            .lock()
            .expect("shroud");
        shroud.set_host_object_shroud_status(
            0,
            building_id.0,
            gamelogic::common::ObjectShroudStatus::Clear,
        );
        shroud.mark_host_object_seen(0, building_id.0);
    }

    // Ability now available.
    game_logic.queue_command(GameCommand {
        command_type: CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after unlock");
    assert_eq!(
        captor.ai_state,
        AIState::Capturing,
        "CaptureBuilding must work after research complete"
    );
    assert_eq!(captor.target, Some(building_id));

    // C++ SpecialAbilityUpdate performs the authored unpack and preparation
    // phases before it defects the target; click acceptance is not a capture.
    game_logic.update_ai(&[captor_id, building_id], 1.0 / 30.0);
    game_logic.update_ai(&[captor_id, building_id], 3.0);
    game_logic.update_ai(&[captor_id, building_id], 20.0);

    let building = game_logic
        .host_object(building_id)
        .expect("building after capture");
    assert_eq!(
        building.team,
        Team::USA,
        "CaptureBuilding must transfer ownership after its authored channel"
    );
    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after capture complete");
    assert_eq!(captor.ai_state, AIState::Capturing);
    game_logic.update_ai(&[captor_id, building_id], 2.0);
    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after capture packing");
    assert_eq!(captor.ai_state, AIState::Idle);
    assert!(captor.target.is_none());
}

#[test]
fn capture_building_walk_into_range_transfers_ownership_after_upgrade() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::UPGRADE_INFANTRY_CAPTURE;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_structure_template(&mut game_logic);
    // C++ retail capture module: `StartsPaused = Yes` +
    // `UnpauseSpecialPowerUpgrade` — the upgrade completion (applied directly
    // below so the walk path is the unit under test) unpauses the power.
    let infantry_template = game_logic
        .templates
        .get_mut("TestInfantry")
        .expect("TestInfantry template");
    infantry_template.capture_starts_paused = true;
    infantry_template.capture_upgrade_trigger =
        Some(UPGRADE_INFANTRY_CAPTURE.to_string());
    infantry_template.sight_range = 200.0;

    // Outside capture range (≈ 8+25+4 = 37) so Capturing must walk in.
    let captor_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(55.0, 0.0, 0.0))
        .expect("captor");
    let building_id = game_logic
        .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("building");

    // Unlock the capture ability without research frames: the C++
    // UnpauseSpecialPowerUpgrade residual (grant upgrade + unpause).
    game_logic.apply_upgrade_to_object(captor_id, UPGRADE_INFANTRY_CAPTURE);
    // The unpaused special keeps its authored 15s ctor ReloadTime (retail
    // RangerCaptureBuilding reload, 15000ms) — model it elapsing with the
    // C++ setReadyFrame(0) residual so the power is ready at click time.
    game_logic
        .host_object_mut(captor_id)
        .expect("captor ready")
        .set_special_power_ready_seconds(
            &crate::command_system::SpecialPowerType::RangerCaptureBuilding,
            0.0,
        );

    // C++ canCaptureBuilding FOW gate (ActionManager.cpp:76-102): register
    // the captor's look + refresh the shared shroud before issuing capture.
    game_logic.host_object_mut(captor_id).expect("captor").vision_range = 200.0;
    game_logic.host_object_mut(captor_id).expect("captor").shroud_range = 200.0;
    gamelogic::system::shroud_manager::get_shroud_manager()
        .lock()
        .expect("shroud")
        .init_shroud_grid(512.0, 512.0);
    game_logic.update_main_crate_vision();

    game_logic.queue_command(GameCommand {
        command_type: CommandType::CaptureBuilding {
            target_id: building_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![captor_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    {
        let captor = game_logic.host_object(captor_id).expect("captor after cmd");
        assert_eq!(captor.ai_state, AIState::Capturing);
        assert_eq!(captor.target, Some(building_id));
    }
    {
        let building = game_logic.host_object(building_id).expect("building");
        assert_eq!(
            building.team,
            Team::GLA,
            "must not transfer ownership until in range"
        );
    }

    // Simulate walk + exact capture channel.
    let mut transferred = false;
    for _ in 0..900 {
        game_logic.update();
        if game_logic
            .host_object(building_id)
            .map(|b| b.team == Team::USA)
            .unwrap_or(false)
        {
            transferred = true;
            break;
        }
    }

    assert!(
        transferred,
        "upgraded infantry must walk into range and complete the capture channel"
    );
    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after transfer");
    assert_eq!(captor.ai_state, AIState::Capturing);
    game_logic.update_ai(&[captor_id, building_id], 2.0);
    let captor = game_logic
        .host_object(captor_id)
        .expect("captor after capture packing");
    assert_eq!(captor.ai_state, AIState::Idle);
    assert!(captor.target.is_none());
}

#[test]
fn flashbang_upgrade_queue_complete_equips_ranger_secondary() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_AMERICA_FLASHBANG};
    use crate::game_logic::weapon_bootstrap::{RANGER_PRIMARY_WEAPON, ensure_host_weapon_store};

    ensure_host_weapon_store();

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);
    ensure_test_barracks_template(&mut game_logic);

    // Ranger without secondary — unlock must equip it.
    let mut ranger = ThingTemplate::new("USA_Ranger");
    ranger
        .add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Selectable)
        .set_health(120.0)
        .set_primary_weapon_name(RANGER_PRIMARY_WEAPON);
    // Intentionally no secondary_weapon_name — research unlocks it.
    game_logic
        .templates
        .insert("USA_Ranger".to_string(), ranger);

    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(-40.0, 0.0, 0.0))
        .expect("barracks");

    let ranger_id = game_logic
        .create_object("USA_Ranger", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("ranger");
    {
        let r = game_logic.host_object(ranger_id).expect("ranger");
        assert!(
            r.secondary_weapon.is_none(),
            "pre-upgrade ranger must lack FlashBang secondary"
        );
        assert!(!r.has_upgrade_tag(UPGRADE_AMERICA_FLASHBANG));
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_FLASHBANG.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![barracks_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .get_player(0)
            .unwrap()
            .has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG)
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::FlashBangGrenade)
    );

    game_logic.update();
    game_logic.update_with_dt(30.0);

    let player = game_logic.get_player(0).expect("player");
    assert!(player.has_unlocked_upgrade(UPGRADE_AMERICA_FLASHBANG));
    assert!(!player.has_queued_upgrade(UPGRADE_AMERICA_FLASHBANG));
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::FlashBangGrenade)
    );
    assert!(
        game_logic.host_upgrades().honesty_flashbang_equipped_ok(),
        "FlashBang complete must equip at least one unit"
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::FlashBangGrenade)
    );

    let ranger = game_logic.host_object(ranger_id).expect("ranger after");
    assert!(
        ranger.has_upgrade_tag(UPGRADE_AMERICA_FLASHBANG),
        "ranger must receive FlashBang upgrade tag"
    );
    let secondary = ranger
        .secondary_weapon
        .as_ref()
        .expect("FlashBang secondary must be equipped on complete");
    assert!(
        (secondary.damage - 35.0).abs() < 0.1,
        "expected RangerFlashBangGrenadeWeapon damage 35, got {}",
        secondary.damage
    );
    assert!((secondary.range - 172.5).abs() < 0.1);
}

#[test]
fn supply_lines_upgrade_queue_complete_tags_supply_center() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_AMERICA_SUPPLY_LINES};

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 5000;
    game_logic.add_player(player);

    let mut supply = ThingTemplate::new("AmericaSupplyCenter");
    supply
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    game_logic
        .templates
        .insert("AmericaSupplyCenter".to_string(), supply);

    let producer_id = game_logic
        .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply center");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_SUPPLY_LINES.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![producer_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    assert!(
        game_logic
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::SupplyLines)
    );
    game_logic.update();
    game_logic.update_with_dt(30.0);

    assert!(
        game_logic
            .get_player(0)
            .unwrap()
            .has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES)
    );
    assert!(
        game_logic
            .host_upgrades()
            .honesty_host_path_ok(HostUpgradeKind::SupplyLines)
    );
    let sc = game_logic.host_object(producer_id).expect("sc after");
    assert!(
        sc.has_upgrade_tag(UPGRADE_AMERICA_SUPPLY_LINES),
        "Supply Lines must tag the supply center"
    );
}

#[test]
fn black_market_residual_cash_increases_over_frames() {
    use crate::game_logic::host_black_market::{
        BLACK_MARKET_DEPOSIT_AMOUNT, BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES,
    };

    fn run_with_market(with_market: bool) -> (u32, u32, u32, bool) {
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::GLA, "GLA", true);
        player.resources.supplies = 1000;
        game_logic.add_player(player);

        if with_market {
            let mut market = ThingTemplate::new("GLABlackMarket");
            market
                .add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::FSBlackMarket)
                .add_kind_of(KindOf::Selectable)
                .set_health(500.0)
                .set_cost(2500, 0);
            game_logic
                .templates
                .insert("GLABlackMarket".to_string(), market);

            let market_id = game_logic
                .create_object("GLABlackMarket", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
                .expect("black market");
            assert!(
                game_logic
                    .host_object(market_id)
                    .map(|o| o.is_constructed() && o.is_alive())
                    .unwrap_or(false),
                "market must be alive and constructed"
            );
        }

        let cash_before = game_logic.get_player(0).unwrap().resources.supplies;
        // First deposit schedules at frame 0 → due at frame 60.
        // update_simulation sees frame N then frame becomes N+1, so need 61 steps
        // to observe frame==60. Run two full intervals for multi-deposit honesty.
        let steps = (BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES as usize) * 2 + 5;
        for _ in 0..steps {
            game_logic.update();
        }
        let cash_after = game_logic.get_player(0).unwrap().resources.supplies;
        let gained = cash_after.saturating_sub(cash_before);
        let residual_cash = game_logic.black_market_residual_cash_total();
        let residual_deposits = game_logic.black_market_residual_deposits();
        let honesty = game_logic.honesty_black_market_ok();
        (gained, residual_cash, residual_deposits, honesty)
    }

    let (without_gained, without_residual, without_deposits, without_honesty) =
        run_with_market(false);
    let (with_gained, with_residual, with_deposits, with_honesty) = run_with_market(true);

    // Fail-closed: no market → no residual Black Market credits.
    assert_eq!(without_residual, 0);
    assert_eq!(without_deposits, 0);
    assert!(
        !without_honesty,
        "black market honesty must fail-closed without a market"
    );

    // With market: at least two residual deposits over ~2 intervals.
    assert!(
        with_deposits >= 2,
        "expected ≥2 residual deposits over 2 intervals (got {with_deposits})"
    );
    assert_eq!(
        with_residual,
        with_deposits.saturating_mul(BLACK_MARKET_DEPOSIT_AMOUNT),
        "residual cash must equal deposits × deposit amount"
    );
    assert!(
        with_honesty,
        "black market residual honesty after AutoDeposit credits"
    );
    assert!(
        with_gained > without_gained,
        "cash gain with market ({with_gained}) must exceed without ({without_gained})"
    );
    // Residual-only delta must match market credits (base $5/s passive is shared).
    let residual_delta = with_gained.saturating_sub(without_gained);
    assert_eq!(
        residual_delta, with_residual,
        "extra cash with market must equal residual AutoDeposit total \
         (with={with_gained}, without={without_gained}, residual={with_residual})"
    );
    assert!(
        with_residual >= BLACK_MARKET_DEPOSIT_AMOUNT,
        "must credit at least one deposit amount"
    );
}

#[test]
fn black_market_residual_skips_under_construction() {
    use crate::game_logic::host_black_market::BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);

    let mut market = ThingTemplate::new("GLABlackMarket");
    market
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBlackMarket)
        .set_health(500.0);
    game_logic
        .templates
        .insert("GLABlackMarket".to_string(), market);

    let market_id = game_logic
        .create_object_under_construction("GLABlackMarket", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("under-construction market");
    assert!(
        game_logic
            .host_object(market_id)
            .map(|o| !o.is_constructed())
            .unwrap_or(false),
        "market must start under construction"
    );

    for _ in 0..(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES as usize + 10) {
        game_logic.update();
    }

    assert_eq!(
        game_logic.black_market_residual_cash_total(),
        0,
        "under-construction market must not residual-deposit"
    );
    assert!(!game_logic.honesty_black_market_ok());
}

#[test]
fn black_market_residual_skips_fake_market() {
    use crate::game_logic::host_black_market::BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);

    let mut fake = ThingTemplate::new("FakeGLABlackMarket");
    fake.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBlackMarket)
        .add_kind_of(KindOf::FSFake)
        .set_health(500.0);
    game_logic
        .templates
        .insert("FakeGLABlackMarket".to_string(), fake);

    let _id = game_logic
        .create_object("FakeGLABlackMarket", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
        .expect("fake market");

    for _ in 0..(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES as usize + 10) {
        game_logic.update();
    }

    assert_eq!(
        game_logic.black_market_residual_cash_total(),
        0,
        "Fake market ActualMoney=No must not residual-deposit cash"
    );
    assert!(!game_logic.honesty_black_market_ok());
}

#[test]
fn accepted_gather_event_contains_only_executor_accepted_carriers() {
    use crate::command_system::{CommandType, GameCommand, ModifierKeys};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_dozer_template(&mut game_logic);

    let mut harvester = ThingTemplate::new("TestHarvester");
    harvester
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Harvester)
        .set_health(300.0);
    game_logic
        .templates
        .insert("TestHarvester".to_string(), harvester);

    let mut resource = ThingTemplate::new("TestGatherSupply");
    resource
        .add_kind_of(KindOf::Harvestable)
        .add_kind_of(KindOf::Resource)
        .set_health(100.0);
    game_logic
        .templates
        .insert("TestGatherSupply".to_string(), resource);

    let harvester_id = game_logic
        .create_object("TestHarvester", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("harvester");
    let dozer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("dozer");
    let target_id = game_logic
        .create_object("TestGatherSupply", Team::Neutral, Vec3::new(20.0, 0.0, 0.0))
        .expect("resource target");
    let issued_at = std::time::SystemTime::now();

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Gather { target_id },
        player_id: 0,
        command_id: 97,
        timestamp: issued_at.clone(),
        selected_units: vec![harvester_id, dozer_id],
        modifier_keys: ModifierKeys::default(),
    });
    game_logic.process_commands();

    let accepted = game_logic.take_accepted_gather_commands();
    assert_eq!(accepted.len(), 1, "Gather must be accepted exactly once");
    let event = &accepted[0];
    assert_eq!(event.command_id, 97);
    assert_eq!(event.issued_at, issued_at);
    assert_eq!(event.player_id, 0);
    assert_eq!(event.target_id, target_id);
    assert_eq!(
        event.carrier_ids,
        vec![harvester_id],
        "only the local selected Harvester with an accepted Gather path may be tracked"
    );
}

#[test]
fn retail_harvesters_parse_and_accept_gather_through_live_command_authority() {
    use crate::assets::ini_parser::IniParser;
    use crate::command_system::{
        CommandSystem, CommandType, ModifierKeys, MouseButton, MouseCommandContext,
        PresentationSelectedUnitHint, PresentationTargetHint,
    };
    use crate::presentation_frame::PresentationFrame;

    fn retail_definition(
        relative_path: &str,
        object_name: &str,
    ) -> crate::assets::ObjectDefinition {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .join(relative_path);
        let contents = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read retail Object INI {}: {error}", path.display()));
        let mut parser = IniParser::new();
        parser
            .parse_ini_content(&contents, &path.display().to_string())
            .unwrap_or_else(|error| panic!("parse retail Object INI {}: {error}", path.display()));
        parser
            .get_definition(object_name)
            .cloned()
            .unwrap_or_else(|| panic!("{object_name} missing from {}", path.display()))
    }

    let cases = [
        (
            "AmericaVehicleChinook",
            "windows_game/extracted_big_files_v2/INI/Object/AmericaAir.ini",
            Team::USA,
            0,
        ),
        (
            "ChinaVehicleSupplyTruck",
            "windows_game/extracted_big_files_v2/INI/Object/ChinaVehicle.ini",
            Team::China,
            1,
        ),
    ];

    for (object_name, source_path, team, player_id) in cases {
        let definition = retail_definition(source_path, object_name);
        let template =
            GameLogic::build_template_from_object_definition(object_name, &definition, None);
        assert!(
            template.is_kind_of(KindOf::Harvester),
            "{object_name} must preserve its retail HARVESTER capability"
        );
        assert!(
            !template.is_kind_of(KindOf::Resource) && !template.is_kind_of(KindOf::Harvestable),
            "{object_name} is a collector, not a harvestable supply source"
        );
        assert!(
            !template.is_kind_of(KindOf::Structure),
            "{object_name} must retain its authored movable classification rather than a name-based fallback structure"
        );

        let mut game_logic = GameLogic::new();
        ensure_test_player_for_team(&mut game_logic, team);
        game_logic
            .templates
            .insert(object_name.to_string(), template);

        let mut resource = ThingTemplate::new("RetailGatherSupply");
        resource
            .add_kind_of(KindOf::Resource)
            .add_kind_of(KindOf::Harvestable)
            .set_health(100.0);
        game_logic
            .templates
            .insert("RetailGatherSupply".to_string(), resource);

        let collector_id = game_logic
            .create_object(object_name, team, Vec3::ZERO)
            .unwrap_or_else(|| panic!("spawn retail collector {object_name}"));
        let collector_object = game_logic
            .host_object(collector_id)
            .expect("spawned retail collector must be live");
        assert!(
            collector_object.is_resource_collector()
                && collector_object.can_move()
                && collector_object.team == team,
            "{object_name} must be a live, movable local HARVESTER before Gather authority"
        );
        assert_eq!(
            game_logic.player_team(player_id),
            Some(team),
            "{object_name} Gather must use its real local player/team"
        );
        let target_id = game_logic
            .create_object(
                "RetailGatherSupply",
                Team::Neutral,
                Vec3::new(20.0, 0.0, 0.0),
            )
            .expect("spawn gather supply");

        // A ground supply truck must use the same path authority as a normal
        // Gather order.  Keep this explicit: aircraft bypass the ground grid,
        // so China would otherwise leave a hidden path regression untested.
        let start_cell = game_logic.pathfinding_system.grid.world_to_grid(Vec3::ZERO);
        let supply_cell = game_logic
            .pathfinding_system
            .grid
            .world_to_grid(Vec3::new(20.0, 0.0, 0.0));
        let direct_ground_path = game_logic.pathfinding_system.find_path_ex(
            Vec3::ZERO,
            Vec3::new(20.0, 0.0, 0.0),
            &game_logic.objects,
            false,
            Some(collector_id),
        );
        assert!(
            direct_ground_path.is_some(),
            "{object_name} ground grid path absent: origin={:?} size={}x{} start={start_cell:?} blocked={} static={} goal={supply_cell:?} blocked={} static={}",
            game_logic.pathfinding_system.grid.origin(),
            game_logic.pathfinding_system.grid.width(),
            game_logic.pathfinding_system.grid.height(),
            game_logic.pathfinding_system.grid.is_blocked(start_cell),
            game_logic
                .pathfinding_system
                .grid
                .is_static_blocked(start_cell),
            game_logic.pathfinding_system.grid.is_blocked(supply_cell),
            game_logic
                .pathfinding_system
                .grid
                .is_static_blocked(supply_cell),
        );
        assert!(
            game_logic.assign_unit_path_for_test(collector_id, Vec3::new(20.0, 0.0, 0.0), &[]),
            "{object_name} must have a valid authoritative path to the supply source"
        );
        assert!(
            game_logic.unit_command_stop(collector_id),
            "path probe must leave {object_name} ready for the real Gather order"
        );

        // The visible game path freezes capability into the presentation frame
        // before classifying the right click.  Prove the parsed capability
        // survives that boundary rather than injecting a Gather command.
        let frame = PresentationFrame::build_from_logic(&game_logic, player_id);
        let collector = frame
            .objects
            .iter()
            .find(|object| object.id == collector_id)
            .expect("local collector in presentation frame");
        assert!(
            PresentationFrame::object_has_kind(collector, KindOf::Harvester),
            "{object_name} HARVESTER must reach the presentation freeze"
        );

        let context = MouseCommandContext {
            world_position: Vec3::new(20.0, 0.0, 0.0),
            target_object: Some(target_id),
            target_presentation: Some(PresentationTargetHint {
                id: target_id,
                is_alive: true,
                is_structure: false,
                is_resource: true,
                under_construction: false,
                sold: false,
                team: Team::Neutral,
                is_enemy_of_local: false,
                is_neutral: true,
                template_name: "RetailGatherSupply".to_string(),
                can_be_entered: false,
                enter_available_capacity: 0,
                enter_uses_transport_slots: false,
                enter_requires_infantry: false,
                enter_forbids_aircraft: false,
                enter_disabled_subdued: false,
                enter_is_rider_change: false,
                rider_change_allowed_templates: Vec::new(),
                is_damaged: false,
                is_friendly_of_local: false,
                provides_vehicle_repair: false,
                provides_aircraft_repair: false,
                provides_heal: false,
                can_provide_service: true,
                dock_kind: crate::game_logic::DockKind::None,
                dock_controller_is_local: false,
                stored_supplies: 0,
                capturable: false,
                immune_to_capture: false,
                capture_garrisonable: false,
                capture_nonstealthed_garrison_count: 0,
                capture_friendly_garrison_count: 0,
                capture_target_effectively_stealthed: false,
                is_crate: false,
                is_salvage_crate: false,
                is_vehicle: false,
                is_aircraft: false,
                is_drone: false,
                is_carbomb: false,
                is_unmanned: false,
                is_mine: false,
            }),
            selected_presentation: vec![PresentationSelectedUnitHint {
                id: collector_id,
                is_alive: true,
                // Keep legacy Worker false: Gather must depend only on the
                // authored Harvester capability frozen above.
                is_resource_collector: true,
                is_worker: false,
                can_attack: false,
                can_move: collector.is_mobile,
                can_request_service: true,
                can_capture: false,
                template_name: object_name.to_string(),
                can_repair: false,
                is_damaged: false,
                is_vehicle: true,
                is_aircraft: collector.object_type
                    == crate::presentation_frame::PresentationObjectType::Aircraft,
                is_above_terrain: false,
                is_infantry: false,
                transport_slot_count: 0,
                stored_supplies: 0,
                is_controlled_by_local: true,
                capture_power: crate::game_logic::CapturePowerKind::None,
                capture_power_ready: false,
                is_salvager: false,
                can_override_special_power_destination: false,
            }],
            presentation_box_select_units: Vec::new(),
            presentation_select_similar_units: Vec::new(),
            screen_position: glam::Vec2::ZERO,
            viewport_size: None,
            world_min: None,
            world_max: None,
            mouse_button: MouseButton::Right,
            modifier_keys: ModifierKeys::default(),
            is_drag: false,
            drag_start: None,
            drag_end: None,
            drag_start_world: None,
            drag_end_world: None,
        };

        let mut commands = CommandSystem::new();
        let command = commands
            .process_mouse_input(&context, &[collector_id], player_id, Some(&game_logic))
            .expect("presentation-frozen RMB must produce a command");
        assert!(
            matches!(&command.command_type, CommandType::Gather { target_id: id } if *id == target_id),
            "{object_name} right click must classify as Gather"
        );

        // This is the real GameLogic command authority, not a fabricated
        // successful result.  Its accepted carrier event feeds the physical
        // input provenance latch in Main.
        game_logic.queue_command(command);
        game_logic.process_commands();
        let accepted = game_logic.take_accepted_gather_commands();
        let collector_after = game_logic
            .host_object(collector_id)
            .expect("collector remains live after Gather command");
        assert_eq!(
            accepted.len(),
            1,
            "{object_name} Gather acceptance (state={:?}, path={:?}, target={:?})",
            collector_after.ai_state,
            collector_after.movement.path,
            collector_after.target,
        );
        assert_eq!(accepted[0].carrier_ids, vec![collector_id]);
        assert_eq!(accepted[0].player_id, player_id);
        assert_eq!(accepted[0].target_id, target_id);
    }
}

#[test]
fn supply_lines_drop_off_yields_more_cash_than_without() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_supply_gather::REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST;
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_AMERICA_SUPPLY_LINES};
    use crate::game_logic::object::AIState;

    fn run_one_drop_off(with_supply_lines: bool) -> (u32, u32, u32, bool) {
        let mut game_logic = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 1000;
        game_logic.add_player(player);
        // Live dock-approach queues are a process-global keyed by ObjectId;
        // clear stale reservations so this instance docks from a clean slate.
        crate::game_logic::host_supply_gather::reset_live_dock_queues();
        let mut chinook = ThingTemplate::new("AmericaVehicleChinook");
        chinook
            .add_kind_of(KindOf::Harvester)
            .add_kind_of(KindOf::Aircraft)
            .set_health(100.0);
        game_logic
            .templates
            .insert("AmericaVehicleChinook".to_string(), chinook);

        let mut supply = ThingTemplate::new("AmericaSupplyCenter");
        supply
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::SupplyCenter)
            .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
        // C++ AmericaSupplyCenter authors a SupplyCenterDockUpdate module
        // (SupplyCenterDockUpdate.cpp); the host keys dock-approach capacity
        // on the parsed dock kind, so the fixture must author it too.
        supply.dock_kind = crate::game_logic::DockKind::SupplyCenter;
        game_logic
            .templates
            .insert("AmericaSupplyCenter".to_string(), supply);

        let sc_id = game_logic
            .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("supply center");

        if with_supply_lines {
            game_logic.queue_command(GameCommand {
                command_type: CommandType::QueueUpgrade {
                    upgrade_name: UPGRADE_AMERICA_SUPPLY_LINES.to_string(),
                },
                player_id: 0,
                command_id: 1,
                timestamp: std::time::SystemTime::now(),
                selected_units: vec![sc_id],
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
            game_logic.process_commands();
            game_logic.update();
            // C++ research advances on the producer's Upgrade.ini BuildTime
            // (Upgrade_AmericaSupplyLines BuildTime = 30s; retail residual
            // table 900 frames) — one 1/30s frame must not instant-complete.
            game_logic.update_with_dt(30.0);
            assert!(
                game_logic
                    .get_player(0)
                    .unwrap()
                    .has_unlocked_upgrade(UPGRADE_AMERICA_SUPPLY_LINES),
                "Supply Lines must unlock before boosted deposit"
            );
            assert!(
                game_logic
                    .host_upgrades()
                    .honesty_complete_ok(HostUpgradeKind::SupplyLines)
            );
        }

        const CARGO: u32 = 400;
        let chinook_id = game_logic
            .create_object("AmericaVehicleChinook", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("chinook");
        {
            let chinook = game_logic.host_object_mut(chinook_id).expect("chinook mut");
            chinook.set_stored_supplies(CARGO);
            chinook.set_ai_state(AIState::ReturningResources);
        }

        let cash_before = game_logic.get_player(0).unwrap().resources.supplies;
        game_logic.update();
        let dropoffs = game_logic.take_supply_dropoff_events();
        assert_eq!(
            dropoffs.len(),
            1,
            "only the real ReturningResources carrier deposit emits an event"
        );
        let dropoff = dropoffs[0];
        assert_eq!(dropoff.carrier_id, chinook_id);
        assert_eq!(dropoff.player_id, 0);
        assert_eq!(
            dropoff.carried_amount, CARGO,
            "event reports carried cargo, not passive income or upgrade bonus"
        );
        let cash_after = game_logic.get_player(0).unwrap().resources.supplies;
        let gained = cash_after.saturating_sub(cash_before);
        let bonus = game_logic.supply_lines_bonus_cash_total();
        let honesty = game_logic.honesty_supply_lines_economy_ok();

        let remaining = game_logic
            .host_object(chinook_id)
            .map(|o| o.stored_resources.supplies)
            .unwrap_or(u32::MAX);
        assert_eq!(remaining, 0, "cargo must clear on drop-off");

        let expected_boost = if with_supply_lines {
            REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST
        } else {
            0
        };
        assert!(
            gained >= CARGO.saturating_add(expected_boost),
            "drop-off cash too low (gained={gained}, cargo={CARGO}, boost={expected_boost}, with_supply_lines={with_supply_lines})"
        );
        assert_eq!(bonus, expected_boost);

        let pure_deposit = CARGO.saturating_add(bonus);
        (gained, pure_deposit, bonus, honesty)
    }

    let (without_gained, without_pure, without_bonus, without_honesty) = run_one_drop_off(false);
    let (with_gained, with_pure, with_bonus, with_honesty) = run_one_drop_off(true);

    assert_eq!(without_bonus, 0, "no economy boost without Supply Lines");
    assert!(
        !without_honesty,
        "economy honesty fail-closed without upgrade"
    );
    assert_eq!(with_bonus, REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST);
    assert!(
        with_honesty,
        "Supply Lines economy residual honesty after boosted drop-off"
    );
    assert!(
        with_pure > without_pure,
        "with SupplyLines pure deposit ({with_pure}) must exceed without ({without_pure})"
    );
    assert_eq!(
        with_pure - without_pure,
        REGULAR_CHINOOK_UPGRADED_SUPPLY_BOOST,
        "delta must equal Chinook INI UpgradedSupplyBoost"
    );
    assert!(
        with_gained > without_gained,
        "with SupplyLines frame gain ({with_gained}) must exceed without ({without_gained})"
    );
}

#[test]
fn supply_lines_does_not_boost_non_chinook_collector() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_AMERICA_SUPPLY_LINES};
    use crate::game_logic::object::AIState;

    let mut game_logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 1000;
    game_logic.add_player(player);
    ensure_test_dozer_template(&mut game_logic);

    let mut supply = ThingTemplate::new("AmericaSupplyCenter");
    supply
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::SupplyCenter)
        .add_kind_of(KindOf::Selectable)
            .set_health(100.0);
    // C++ AmericaSupplyCenter authors SupplyCenterDockUpdate (dock-approach
    // capacity keys on the parsed dock kind) — author it so the dozer's
    // drop-off below actually runs instead of failing closed.
    supply.dock_kind = crate::game_logic::DockKind::SupplyCenter;
    game_logic
        .templates
        .insert("AmericaSupplyCenter".to_string(), supply);

    let sc_id = game_logic
        .create_object("AmericaSupplyCenter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("supply center");
    game_logic.queue_command(GameCommand {
        command_type: CommandType::QueueUpgrade {
            upgrade_name: UPGRADE_AMERICA_SUPPLY_LINES.to_string(),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![sc_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    crate::game_logic::host_supply_gather::reset_live_dock_queues();
    game_logic.update();
    // C++ research advances on the producer's Upgrade.ini BuildTime
    // (Upgrade_AmericaSupplyLines BuildTime = 30s → 900 frames) — one
    // 1/30s frame must not instant-complete it.
    game_logic.update_with_dt(30.0);
    assert!(
        game_logic
            .host_upgrades()
            .honesty_complete_ok(HostUpgradeKind::SupplyLines)
    );

    const CARGO: u32 = 400;
    let dozer_id = game_logic
        .create_object("TestDozer", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("dozer");
    {
        let dozer = game_logic.host_object_mut(dozer_id).expect("dozer mut");
        dozer.set_stored_supplies(CARGO);
        dozer.set_ai_state(AIState::ReturningResources);
    }
    game_logic.update();
    assert_eq!(
        game_logic.supply_lines_bonus_cash_total(),
        0,
        "Supply Lines must not boost a non-Chinook collector"
    );
}

#[test]
fn loaded_combat_chinook_does_not_auto_gather() {
    use crate::game_logic::{DockKind, SupplyTruckMetadata, SupplyTruckState};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::USA, "USA", true);
    player.resources.supplies = 0;
    logic.add_player(player);
    ensure_test_infantry_template(&mut logic);

    let mut warehouse = ThingTemplate::new("FiniteWarehouse");
    warehouse
        .add_kind_of(KindOf::SupplySource)
        .set_health(100.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    warehouse.dock_starting_boxes = Some(10);
    logic.templates.insert(warehouse.name.clone(), warehouse);
    let box_value = crate::game_logic::host_structure_economy_residual::VALUE_PER_SUPPLY_BOX as u32;
    let source = logic
        .create_object("FiniteWarehouse", Team::Neutral, Vec3::new(2.0, 0.0, 0.0))
        .expect("warehouse");
    {
        let warehouse = logic.host_object_mut(source).expect("warehouse mut");
        warehouse.set_stored_supplies(10 * box_value);
    }

    let chinook_id = create_test_combat_chinook(&mut logic, Vec3::ZERO);
    {
        let chinook = logic.host_object_mut(chinook_id).expect("chinook mut");
        chinook.thing.template.supply_truck_metadata = Some(SupplyTruckMetadata {
            max_boxes: 8,
            warehouse_scan_distance: 700.0,
            warehouse_delay_frames: 0,
            center_delay_frames: 0,
            upgraded_supply_boost: 60,
        });
        chinook.set_target(Some(source));
        chinook.set_ai_state(AIState::Gathering);
        chinook.supply_truck_state = SupplyTruckState::DockingWarehouse;
        chinook.set_stored_supplies(0);
    }
    let infantry = logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("infantry");
    {
        let chinook = logic.host_object_mut(chinook_id).expect("load");
        assert!(chinook.add_occupant(infantry), "load troop");
    }

    logic.update_support_states(&[chinook_id, source, infantry], 1.0 / 30.0);

    let cargo = logic
        .host_object(chinook_id)
        .expect("chinook after")
        .stored_resources
        .supplies;
    assert_eq!(cargo, 0, "loaded Combat Chinook must not gather");
    assert_eq!(
        logic.host_object(chinook_id).expect("state").ai_state,
        AIState::Idle,
        "loaded Chinook must leave Gathering"
    );
    assert_eq!(
        logic
            .host_object(source)
            .expect("warehouse after")
            .stored_resources
            .supplies,
        10 * box_value,
        "warehouse must stay full"
    );
}

#[test]
fn supply_truck_force_wanting_reenters_gathering() {
    use crate::game_logic::{DockKind, SupplyTruckMetadata, SupplyTruckState};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::China, "China", true);
    player.resources.supplies = 0;
    logic.add_player(player);

    let mut truck = ThingTemplate::new("ChinaVehicleSupplyTruck");
    truck.add_kind_of(KindOf::Harvester).set_health(100.0);
    truck.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 4,
        warehouse_scan_distance: 700.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 0,
    });
    logic.templates.insert(truck.name.clone(), truck);

    let mut warehouse = ThingTemplate::new("FiniteWarehouse");
    warehouse
        .add_kind_of(KindOf::SupplySource)
        .set_health(100.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    warehouse.dock_starting_boxes = Some(10);
    logic.templates.insert(warehouse.name.clone(), warehouse);
    let source = logic
        .create_object("FiniteWarehouse", Team::Neutral, Vec3::new(20.0, 0.0, 0.0))
        .expect("warehouse");
    {
        let warehouse = logic.host_object_mut(source).expect("warehouse mut");
        warehouse.set_stored_supplies(10 * 75);
    }

    let collector_id = logic
        .create_object_for_player("ChinaVehicleSupplyTruck", 0, Vec3::ZERO)
        .expect("truck");
    {
        let collector = logic.host_object_mut(collector_id).expect("truck mut");
        collector.set_ai_state(AIState::Idle);
        collector.supply_truck_state = SupplyTruckState::Regrouping;
        collector.supply_truck_force_pending = true;
        collector.set_stored_supplies(0);
        collector.stop_moving();
    }

    // Sync the process-global state earlier tests in this binary leak: stale
    // (player, ObjectId) shroud rows and dock-approach queue reservations
    // keyed by reused ObjectIds. A live C++ game tears these down per match;
    // the host keeps them process-global, so each fixture re-syncs them.
    crate::game_logic::host_supply_gather::reset_live_dock_queues();
    logic.update_main_crate_vision();
    logic.update_support_states(&[collector_id, source], 1.0 / 30.0);

    let after = logic.host_object(collector_id).expect("truck after");
    assert_eq!(
        after.ai_state,
        AIState::Gathering,
        "force-wanting Idle must re-enter Wanting/Gathering"
    );
    assert_eq!(after.target, Some(source));
    assert!(
        !after.supply_truck_force_pending,
        "Wanting onEnter clears force"
    );
    assert_eq!(after.supply_truck_state, SupplyTruckState::Wanting);
}

#[test]
fn worker_gather_arms_mine_clearing_detail() {
    use crate::game_logic::{DockKind, SupplyTruckMetadata, SupplyTruckState};

    let mut logic = GameLogic::new();
    let mut player = Player::new(0, Team::GLA, "GLA", true);
    player.resources.supplies = 0;
    logic.add_player(player);

    let mut worker = ThingTemplate::new("GLAInfantryWorker");
    worker
        .add_kind_of(KindOf::Harvester)
        .add_kind_of(KindOf::Worker)
        .set_health(100.0);
    worker.supply_truck_metadata = Some(SupplyTruckMetadata {
        max_boxes: 1,
        warehouse_scan_distance: 700.0,
        warehouse_delay_frames: 0,
        center_delay_frames: 0,
        upgraded_supply_boost: 8,
    });
    logic.templates.insert(worker.name.clone(), worker);

    let mut warehouse = ThingTemplate::new("FiniteWarehouse");
    warehouse
        .add_kind_of(KindOf::SupplySource)
        .set_health(100.0);
    warehouse.dock_kind = DockKind::SupplyWarehouse;
    warehouse.dock_starting_boxes = Some(4);
    logic.templates.insert(warehouse.name.clone(), warehouse);
    let source = logic
        .create_object("FiniteWarehouse", Team::Neutral, Vec3::new(2.0, 0.0, 0.0))
        .expect("warehouse");

    let worker_id = logic
        .create_object_for_player("GLAInfantryWorker", 0, Vec3::ZERO)
        .expect("worker");
    {
        let worker = logic.host_object_mut(worker_id).expect("worker mut");
        worker.set_target(Some(source));
        worker.set_ai_state(AIState::Gathering);
        worker.supply_truck_state = SupplyTruckState::DockingWarehouse;
        assert!(!worker.weapon_set_mine_clearing_detail);
    }

    logic.update_support_states(&[worker_id, source], 1.0 / 30.0);

    let worker = logic.host_object(worker_id).expect("worker after");
    assert!(
        worker.weapon_set_mine_clearing_detail,
        "gathering GLA worker must arm WEAPONSET_MINE_CLEARING_DETAIL"
    );
}

#[test]
fn garrison_residual_enter_sets_garrisoned_state_and_capacity() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    let bunker = game_logic.host_object(bunker_id).expect("bunker");
    assert!(
        bunker.can_contain() && bunker.garrison_capacity() > 0,
        "TestBunker must be residual-garrisonable"
    );
    assert_eq!(bunker.garrison_count(), 0);
    let capacity = bunker.garrison_capacity();

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: bunker_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("infantry cmd");
    assert_eq!(
        infantry.ai_state,
        AIState::Entering,
        "Enter command must start residual enter"
    );
    assert_eq!(infantry.target, Some(bunker_id));

    // Walk-into-range residual: close enough that Entering completes this frame.
    game_logic.update_ai(&[infantry_id, bunker_id], 1.0 / 30.0);

    let bunker = game_logic.host_object(bunker_id).expect("bunker after");
    assert!(
        bunker.contained_units().contains(&infantry_id),
        "bunker must list garrisoned infantry"
    );
    assert_eq!(bunker.garrison_count(), 1);
    assert!(
        bunker.garrison_count() <= capacity,
        "must respect residual capacity"
    );

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(
        infantry.ai_state,
        AIState::Garrisoned,
        "infantry must be Garrisoned after enter residual"
    );
    assert_eq!(infantry.contained_by, Some(bunker_id));
    assert!(!infantry.can_move(), "garrisoned units cannot move freely");
    assert_eq!(game_logic.garrison_residual_enters(), 1);
}

#[test]
fn garrison_residual_exit_frees_unit_and_capacity() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    // Force enter residual via Entering support-state.
    {
        let unit = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry mut");
        unit.target = Some(bunker_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[infantry_id, bunker_id], 1.0 / 30.0);
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state,
        AIState::Garrisoned
    );
    assert_eq!(game_logic.garrison_residual_enters(), 1);

    // Evacuate from selected bunker (structure inventory residual).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![bunker_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let bunker = game_logic
        .host_object(bunker_id)
        .expect("bunker after exit");
    assert!(
        !bunker.contained_units().contains(&infantry_id),
        "evacuate must free garrison capacity"
    );
    assert_eq!(bunker.garrison_count(), 0);

    let infantry = game_logic.host_object(infantry_id).expect("infantry free");
    // C++ GarrisonContain::exitObjectViaDoor walks the occupant out
    // (burst/left/right); the unit is free but still completing its walk.
    assert_eq!(infantry.ai_state, AIState::Moving);
    assert!(infantry.contained_by.is_none());
    assert!(infantry.target.is_none());
    assert!(infantry.can_move(), "exited unit must be free to move");
    assert_eq!(game_logic.garrison_residual_exits(), 1);
    assert!(
        game_logic.honesty_garrison_enter_exit_ok(),
        "enter+exit residual honesty"
    );
}

#[test]
fn garrison_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    // Shrink residual capacity to 1 for a fast full test.
    {
        let bunker = game_logic.host_object_mut(bunker_id).expect("bunker mut");
        if let Some(data) = bunker.building_data.as_mut() {
            data.max_garrison = 1;
        }
    }

    let first_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("first");
    {
        let unit = game_logic.host_object_mut(first_id).unwrap();
        unit.target = Some(bunker_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[first_id, bunker_id], 1.0 / 30.0);
    assert!(
        game_logic
            .host_object(bunker_id)
            .unwrap()
            .contained_units()
            .contains(&first_id)
    );

    let second_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("second");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: bunker_id,
        },
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![second_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let second = game_logic.host_object(second_id).expect("second after");
    assert_ne!(
        second.ai_state,
        AIState::Entering,
        "full garrison must reject Enter"
    );
    assert_ne!(second.target, Some(bunker_id));
    assert_eq!(
        game_logic.host_object(bunker_id).unwrap().garrison_count(),
        1,
        "capacity stays full at residual max"
    );
}

#[test]
fn garrison_residual_rejects_vehicle_enter_structure() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let tank_id = game_logic
        .create_object("TestTank", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: bunker_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_ne!(tank.ai_state, AIState::Entering);
    assert_ne!(tank.target, Some(bunker_id));
}

#[test]
fn garrison_really_damaged_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    {
        let bunker = game_logic.host_object_mut(bunker_id).expect("bunker mut");
        bunker.health.current = 200.0;
        bunker.refresh_model_condition_bits();
        assert_eq!(bunker.body_damage_state, HostBodyDamageType::ReallyDamaged);
    }

    assert!(
        !game_logic.can_unit_enter_normal_target(infantry_id, bunker_id),
        "ReallyDamaged garrison must reject new occupants"
    );

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: bunker_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("infantry");
    assert_ne!(
        infantry.ai_state,
        AIState::Entering,
        "Enter command must not start against a ReallyDamaged wreck"
    );
    assert_ne!(infantry.target, Some(bunker_id));
}

#[test]
fn garrison_really_damaged_edge_ejects_occupants() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    {
        let unit = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry mut");
        unit.target = Some(bunker_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[infantry_id, bunker_id], 1.0 / 30.0);
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state,
        AIState::Garrisoned
    );
    assert!(
        game_logic
            .host_object(bunker_id)
            .unwrap()
            .contained_units()
            .contains(&infantry_id)
    );

    let (destroyed, _) = game_logic.apply_host_damage(bunker_id, 800.0);
    assert!(!destroyed, "bunker must survive as a ReallyDamaged wreck");
    assert_eq!(
        game_logic.host_object(bunker_id).unwrap().body_damage_state,
        HostBodyDamageType::ReallyDamaged
    );

    let bunker = game_logic.host_object(bunker_id).expect("bunker after");
    assert!(
        !bunker.contained_units().contains(&infantry_id),
        "crossing ReallyDamaged must eject garrison occupants"
    );
    assert_eq!(bunker.garrison_count(), 0);

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert!(
        infantry.contained_by.is_none(),
        "ejected occupant must leave the container"
    );
    assert_ne!(
        infantry.ai_state,
        AIState::Garrisoned,
        "ejected occupant must not stay Garrisoned"
    );
}

#[test]
fn garrisonable_until_destroyed_still_allows_occupy_when_really_damaged() {
    use crate::game_logic::host_enum_table_residual::HostBodyDamageType;

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");

    {
        let bunker = game_logic.host_object_mut(bunker_id).expect("bunker mut");
        bunker
            .thing
            .template
            .add_kind_of(KindOf::GarrisonableUntilDestroyed);
        bunker.health.current = 200.0;
        bunker.refresh_model_condition_bits();
        assert_eq!(bunker.body_damage_state, HostBodyDamageType::ReallyDamaged);
    }

    assert!(
        game_logic.can_unit_enter_normal_target(infantry_id, bunker_id),
        "GARRISONABLE_UNTIL_DESTROYED must still accept occupants when ReallyDamaged"
    );

    {
        let unit = game_logic
            .host_object_mut(infantry_id)
            .expect("infantry mut");
        unit.target = Some(bunker_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[infantry_id, bunker_id], 1.0 / 30.0);
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state,
        AIState::Garrisoned,
        "Firebase-style bunker must still complete occupy"
    );
    assert!(
        game_logic
            .host_object(bunker_id)
            .unwrap()
            .contained_units()
            .contains(&infantry_id)
    );

    let (destroyed, _) = game_logic.apply_host_damage(bunker_id, 50.0);
    assert!(!destroyed);
    assert_eq!(
        game_logic.host_object(bunker_id).unwrap().body_damage_state,
        HostBodyDamageType::ReallyDamaged
    );
    assert!(
        game_logic
            .host_object(bunker_id)
            .unwrap()
            .contained_units()
            .contains(&infantry_id),
        "GARRISONABLE_UNTIL_DESTROYED must not eject on ReallyDamaged"
    );
}

#[test]
fn garrison_residual_fire_from_garrison_damages_nearby_enemy() {
    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    ensure_test_tank_template(&mut game_logic);
    ensure_test_garrison_template(&mut game_logic);

    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("infantry");
    let bunker_id = game_logic
        .create_object("TestBunker", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("bunker");
    let enemy_id = game_logic
        .create_object("TestTank", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
        .expect("enemy");

    // Equip residual weapon + force garrisoned state.
    {
        let unit = game_logic.host_object_mut(infantry_id).unwrap();
        unit.weapon = Some(Weapon {
            damage: 40.0,
            range: 100.0,
            reload_time: 0.1,
            last_fire_time: -10.0,
            ..Weapon::default()
        });
        unit.target = Some(bunker_id);
        unit.set_contained_by(Some(bunker_id));
        unit.set_ai_state(AIState::Garrisoned);
        unit.set_position(Vec3::new(0.0, 0.0, 0.0));
    }
    {
        let bunker = game_logic.host_object_mut(bunker_id).unwrap();
        assert!(bunker.add_occupant(infantry_id));
    }

    let enemy_hp_before = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);

    game_logic.update_combat(&[infantry_id, bunker_id, enemy_id], 1.0 / 30.0);

    let enemy_hp_after = game_logic
        .host_object(enemy_id)
        .map(|e| e.health.current)
        .unwrap_or(0.0);
    assert!(
        enemy_hp_after < enemy_hp_before,
        "garrison residual fire must damage nearby enemy (before={enemy_hp_before}, after={enemy_hp_after})"
    );
    assert!(
        game_logic.honesty_garrison_fire_ok(),
        "fire-from-garrison residual honesty"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().ai_state,
        AIState::Garrisoned,
        "firing must not eject garrisoned unit"
    );
    assert_eq!(
        game_logic.host_object(infantry_id).unwrap().contained_by,
        Some(bunker_id)
    );
}

#[test]
fn garrison_residual_barracks_not_garrisonable() {
    let mut game_logic = GameLogic::new();
    ensure_test_barracks_template(&mut game_logic);
    let barracks_id = game_logic
        .create_object("TestBarracks", Team::USA, Vec3::new(0.0, 0.0, 0.0))
        .expect("barracks");
    let barracks = game_logic.host_object(barracks_id).unwrap();
    assert!(
        !barracks.can_contain(),
        "faction barracks must not accept residual garrison"
    );
    assert_eq!(barracks.garrison_capacity(), 0);
}

#[test]
fn transport_residual_enter_sets_docked_state_and_capacity() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_infantry_template(&mut game_logic);
    let transport_id = create_test_transport(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), 5);
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    // The shroud manager is a process-global singleton while ObjectIds are
    // reused per GameLogic instance, so earlier tests in this binary leave
    // (player, ObjectId) rows behind. A C++ match rebuilds shroud state
    // (ShroudManager::clear_all "multiplayer match initialization"), so this
    // fixture resets it to the fresh-process state before gating paths run.
    gamelogic::system::shroud_manager::get_shroud_manager()
        .lock()
        .expect("shroud")
        .clear_all();
    let transport = game_logic.host_object(transport_id).expect("transport");
    assert!(
        transport.can_contain() && transport.transport_capacity() == 5,
        "TestTransport must expose residual capacity"
    );
    assert_eq!(transport.transport_count(), 0);

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: transport_id,
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("infantry cmd");
    assert_eq!(
        infantry.ai_state,
        AIState::Entering,
        "Enter command must start residual transport enter"
    );
    assert_eq!(infantry.target, Some(transport_id));

    game_logic.update_ai(&[infantry_id, transport_id], 1.0 / 30.0);

    let transport = game_logic
        .host_object(transport_id)
        .expect("transport after");
    assert!(
        transport.contained_units().contains(&infantry_id),
        "transport must list loaded infantry"
    );
    assert_eq!(transport.transport_count(), 1);
    assert!(transport.transport_count() <= transport.transport_capacity());

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(
        infantry.ai_state,
        AIState::Docked,
        "infantry must be Docked after transport residual load"
    );
    assert_eq!(infantry.contained_by, Some(transport_id));
    assert!(!infantry.can_move(), "loaded units cannot move freely");
    assert_eq!(game_logic.transport_residual_loads(), 1);
    assert_eq!(
        game_logic.garrison_residual_enters(),
        0,
        "vehicle load must not count as structure garrison"
    );
}

#[test]
fn transport_residual_load_two_unload_both_free() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_infantry_template(&mut game_logic);
    let transport_id = create_test_transport(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), 5);

    let unit_a = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit b");

    // The shroud manager is a process-global singleton while ObjectIds are
    // reused per GameLogic instance, so earlier tests in this binary leave
    // (player, ObjectId) rows behind. A C++ match rebuilds shroud state
    // (ShroudManager::clear_all "multiplayer match initialization"), so this
    // fixture resets it to the fresh-process state before gating paths run.
    gamelogic::system::shroud_manager::get_shroud_manager()
        .lock()
        .expect("shroud")
        .clear_all();
    // Load both via Enter residual (walk-into-range completes same frame).
    for unit_id in [unit_a, unit_b] {
        {
            let unit = game_logic.host_object_mut(unit_id).expect("unit mut");
            unit.target = Some(transport_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, transport_id], 1.0 / 30.0);
    }

    let transport = game_logic
        .host_object(transport_id)
        .expect("transport loaded");
    assert!(
        transport.contained_units().contains(&unit_a)
            && transport.contained_units().contains(&unit_b),
        "both infantry must be loaded"
    );
    assert_eq!(transport.transport_count(), 2);
    assert_eq!(game_logic.transport_residual_loads(), 2);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("loaded unit");
        assert_eq!(unit.ai_state, AIState::Docked);
        assert_eq!(unit.contained_by, Some(transport_id));
        assert!(!unit.can_move());
    }

    // Unload all (selected transport Evacuate / Exit residual).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 0,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![transport_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let transport = game_logic
        .host_object(transport_id)
        .expect("transport empty");
    assert!(
        transport.contained_units().is_empty(),
        "evacuate must clear all transport occupants"
    );
    assert_eq!(transport.transport_count(), 0);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("freed unit");
        // C++ OpenContain::exitObjectViaDoor places the rider at ExitStart
        // and issues aiFollowPath to ExitEnd (OpenContain.cpp:915-1020);
        // the freed rider walks out instead of Idling in place.
        assert_eq!(
            unit.ai_state,
            AIState::Moving,
            "unloaded unit must walk its exit path (C++ aiFollowPath)"
        );
        assert!(unit.contained_by.is_none(), "contained_by must clear");
        assert!(unit.target.is_none());
        assert!(unit.can_move(), "unloaded unit must be free to move");
    }

    assert_eq!(game_logic.transport_residual_unloads(), 2);
    assert!(
        game_logic.honesty_transport_load_unload_ok(),
        "load+unload residual honesty"
    );
    assert_eq!(
        game_logic.garrison_residual_exits(),
        0,
        "transport unload must not count as garrison exit"
    );
}

#[test]
fn transport_residual_exit_command_unloads_all() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_infantry_template(&mut game_logic);
    let transport_id = create_test_transport(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), 4);
    let unit_a = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(2.0, 0.0, 0.0))
        .expect("b");

    // The shroud manager is a process-global singleton while ObjectIds are
    // reused per GameLogic instance, so earlier tests in this binary leave
    // (player, ObjectId) rows behind. A C++ match rebuilds shroud state
    // (ShroudManager::clear_all "multiplayer match initialization"), so this
    // fixture resets it to the fresh-process state before gating paths run.
    gamelogic::system::shroud_manager::get_shroud_manager()
        .lock()
        .expect("shroud")
        .clear_all();
    for unit_id in [unit_a, unit_b] {
        {
            let unit = game_logic.host_object_mut(unit_id).unwrap();
            unit.target = Some(transport_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, transport_id], 1.0 / 30.0);
    }
    assert_eq!(
        game_logic
            .host_object(transport_id)
            .unwrap()
            .transport_count(),
        2
    );

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Exit,
        player_id: 0,
        command_id: 3,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![transport_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    assert!(
        game_logic
            .host_object(transport_id)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    assert!(game_logic.host_object(unit_a).unwrap().can_move());
    assert!(game_logic.host_object(unit_b).unwrap().can_move());
    assert_eq!(game_logic.transport_residual_unloads(), 2);
}

#[test]
fn transport_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::USA);
    ensure_test_infantry_template(&mut game_logic);
    let transport_id = create_test_transport(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), 1);
    // The shroud manager is a process-global singleton while ObjectIds are
    // reused per GameLogic instance, so earlier tests in this binary leave
    // (player, ObjectId) rows behind. A C++ match rebuilds shroud state
    // (ShroudManager::clear_all "multiplayer match initialization"), so this
    // fixture resets it to the fresh-process state before gating paths run.
    gamelogic::system::shroud_manager::get_shroud_manager()
        .lock()
        .expect("shroud")
        .clear_all();

    let first_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(1.0, 0.0, 0.0))
        .expect("first");
    {
        let unit = game_logic.host_object_mut(first_id).unwrap();
        unit.target = Some(transport_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[first_id, transport_id], 1.0 / 30.0);
    assert!(
        game_logic
            .host_object(transport_id)
            .unwrap()
            .contained_units()
            .contains(&first_id)
    );

    let second_id = game_logic
        .create_object("TestInfantry", Team::USA, Vec3::new(3.0, 0.0, 0.0))
        .expect("second");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: transport_id,
        },
        player_id: 0,
        command_id: 4,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![second_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let second = game_logic.host_object(second_id).expect("second after");
    assert_ne!(
        second.ai_state,
        AIState::Entering,
        "full transport must reject Enter"
    );
    assert_ne!(second.target, Some(transport_id));
    assert_eq!(
        game_logic
            .host_object(transport_id)
            .unwrap()
            .transport_count(),
        1
    );
}

#[test]
fn overlord_bunker_residual_without_bunker_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_infantry_template(&mut game_logic);
    // Some(0): overlord-style, no BattleBunker residual installed.
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), None);
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    let overlord = game_logic.host_object(overlord_id).expect("overlord");
    assert!(
        overlord.is_overlord_style_container(),
        "TestOverlord must mark overlord-style residual"
    );
    assert_eq!(overlord.overlord_bunker_slot_capacity(), 0);
    assert!(
        !overlord.can_contain(),
        "Overlord without BattleBunker residual must not accept enter"
    );
    assert_eq!(overlord.transport_capacity(), 0);

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: overlord_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("infantry");
    assert_ne!(
        infantry.ai_state,
        AIState::Entering,
        "enter must be rejected without bunker residual"
    );
    assert_ne!(infantry.target, Some(overlord_id));
    assert_eq!(game_logic.overlord_bunker_residual_enters(), 0);
}

#[test]
fn overlord_bunker_residual_enter_sets_docked_state_and_capacity() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    // C++ TransportContain requires the rider's controlling player to equal
    // the transport's; fixtures model that with a live China player.
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_infantry_template(&mut game_logic);
    // C++ ChinaTankOverlordBattleBunker TransportContain Slots = 5.
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), Some(5));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");

    let overlord = game_logic.host_object(overlord_id).expect("overlord");
    assert!(
        overlord.can_contain() && overlord.overlord_bunker_slot_capacity() == 5,
        "BattleBunker residual must expose 5 infantry slots"
    );
    assert_eq!(overlord.transport_capacity(), 5);
    assert_eq!(overlord.transport_count(), 0);

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: overlord_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let infantry = game_logic.host_object(infantry_id).expect("infantry cmd");
    assert_eq!(
        infantry.ai_state,
        AIState::Entering,
        "Enter command must start residual Overlord bunker enter"
    );
    assert_eq!(infantry.target, Some(overlord_id));

    game_logic.update_ai(&[infantry_id, overlord_id], 1.0 / 30.0);

    let overlord = game_logic.host_object(overlord_id).expect("overlord after");
    assert!(
        overlord.contained_units().contains(&infantry_id),
        "Overlord bunker residual must list loaded infantry"
    );
    assert_eq!(overlord.transport_count(), 1);
    assert!(overlord.transport_count() <= overlord.transport_capacity());

    let infantry = game_logic.host_object(infantry_id).expect("infantry after");
    assert_eq!(
        infantry.ai_state,
        AIState::Docked,
        "infantry must be Docked after Overlord bunker residual load"
    );
    assert_eq!(infantry.contained_by, Some(overlord_id));
    assert!(!infantry.can_move(), "loaded units cannot move freely");
    assert_eq!(game_logic.overlord_bunker_residual_enters(), 1);
    assert_eq!(
        game_logic.transport_residual_loads(),
        0,
        "Overlord bunker enter must not count as generic transport load"
    );
    assert_eq!(
        game_logic.garrison_residual_enters(),
        0,
        "Overlord bunker enter must not count as structure garrison"
    );
}

#[test]
fn overlord_bunker_enter_sets_rider_experience_sink_to_tank() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_infantry_template(&mut game_logic);
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), Some(5));
    let infantry_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("infantry");
    {
        let overlord = game_logic.host_object_mut(overlord_id).expect("overlord");
        overlord.thing.template.is_trainable = true;
    }

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: overlord_id,
        },
        player_id: 1,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![infantry_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();
    game_logic.update_ai(&[infantry_id, overlord_id], 1.0 / 30.0);

    let infantry = game_logic.host_object(infantry_id).expect("rider");
    assert_eq!(
        infantry.experience_sink,
        Some(overlord_id),
        "Overlord onContaining must setExperienceSink to the tank"
    );

    game_logic.award_experience(infantry_id, 40.0);
    let infantry = game_logic.host_object(infantry_id).expect("rider after xp");
    let overlord = game_logic.host_object(overlord_id).expect("tank after xp");
    assert_eq!(
        infantry.experience.current, 0.0,
        "bunker occupant must forward kill XP"
    );
    assert_eq!(
        overlord.experience.current, 40.0,
        "tank must receive the rider sink forward"
    );
}

#[test]
fn overlord_bunker_residual_load_two_unload_both_free() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_infantry_template(&mut game_logic);
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), Some(5));

    let unit_a = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("unit a");
    let unit_b = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("unit b");

    // The shroud manager is a process-global singleton while ObjectIds are
    // reused per GameLogic instance, so earlier tests in this binary leave
    // (player, ObjectId) rows behind. A C++ match rebuilds shroud state
    // (ShroudManager::clear_all "multiplayer match initialization"), so this
    // fixture resets it to the fresh-process state before gating paths run.
    gamelogic::system::shroud_manager::get_shroud_manager()
        .lock()
        .expect("shroud")
        .clear_all();
    for unit_id in [unit_a, unit_b] {
        {
            let unit = game_logic.host_object_mut(unit_id).expect("unit mut");
            unit.target = Some(overlord_id);
            unit.set_ai_state(AIState::Entering);
        }
        game_logic.update_ai(&[unit_id, overlord_id], 1.0 / 30.0);
    }

    let overlord = game_logic
        .host_object(overlord_id)
        .expect("overlord loaded");
    assert!(
        overlord.contained_units().contains(&unit_a)
            && overlord.contained_units().contains(&unit_b),
        "both infantry must be loaded into Overlord bunker residual"
    );
    assert_eq!(overlord.transport_count(), 2);
    assert_eq!(game_logic.overlord_bunker_residual_enters(), 2);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("loaded unit");
        assert_eq!(unit.ai_state, AIState::Docked);
        assert_eq!(unit.contained_by, Some(overlord_id));
        assert!(!unit.can_move());
    }

    // Unload all (selected Overlord Evacuate residual).
    game_logic.queue_command(GameCommand {
        command_type: CommandType::Evacuate,
        player_id: 1,
        command_id: 2,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![overlord_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let overlord = game_logic.host_object(overlord_id).expect("overlord empty");
    assert!(
        overlord.contained_units().is_empty(),
        "evacuate must clear all Overlord bunker residual occupants"
    );
    assert_eq!(overlord.transport_count(), 0);

    for unit_id in [unit_a, unit_b] {
        let unit = game_logic.host_object(unit_id).expect("freed unit");
        // C++ OpenContain::exitObjectViaDoor places the rider at ExitStart
        // and issues aiFollowPath to ExitEnd (OpenContain.cpp:915-1020);
        // the freed rider walks out instead of Idling in place.
        assert_eq!(
            unit.ai_state,
            AIState::Moving,
            "unloaded unit must walk its exit path (C++ aiFollowPath)"
        );
        assert!(unit.contained_by.is_none(), "contained_by must clear");
        assert!(unit.target.is_none());
        assert!(unit.can_move(), "unloaded unit must be free to move");
    }

    assert_eq!(game_logic.overlord_bunker_residual_exits(), 2);
    assert!(
        game_logic.honesty_overlord_bunker_enter_exit_ok(),
        "enter+exit residual honesty"
    );
    assert_eq!(
        game_logic.transport_residual_unloads(),
        0,
        "Overlord bunker unload must not count as generic transport unload"
    );
    assert_eq!(
        game_logic.garrison_residual_exits(),
        0,
        "Overlord bunker unload must not count as garrison exit"
    );
}

#[test]
fn overlord_bunker_residual_capacity_full_rejects_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_player_for_team(&mut game_logic, Team::China);
    ensure_test_infantry_template(&mut game_logic);
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), Some(1));

    let first_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(1.0, 0.0, 0.0))
        .expect("first");
    {
        let unit = game_logic.host_object_mut(first_id).unwrap();
        unit.target = Some(overlord_id);
        unit.set_ai_state(AIState::Entering);
    }
    game_logic.update_ai(&[first_id, overlord_id], 1.0 / 30.0);
    assert!(
        game_logic
            .host_object(overlord_id)
            .unwrap()
            .contained_units()
            .contains(&first_id)
    );

    let second_id = game_logic
        .create_object("TestInfantry", Team::China, Vec3::new(3.0, 0.0, 0.0))
        .expect("second");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: overlord_id,
        },
        player_id: 1,
        command_id: 4,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![second_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let second = game_logic.host_object(second_id).expect("second after");
    assert_ne!(
        second.ai_state,
        AIState::Entering,
        "full Overlord bunker residual must reject Enter"
    );
    assert_ne!(second.target, Some(overlord_id));
    assert_eq!(
        game_logic
            .host_object(overlord_id)
            .unwrap()
            .transport_count(),
        1
    );
}

#[test]
fn overlord_bunker_residual_rejects_vehicle_enter() {
    use crate::command_system::{CommandType, GameCommand};

    let mut game_logic = GameLogic::new();
    ensure_test_tank_template(&mut game_logic);
    let overlord_id = create_test_overlord(&mut game_logic, Vec3::new(0.0, 0.0, 0.0), Some(5));
    let tank_id = game_logic
        .create_object("TestTank", Team::China, Vec3::new(2.0, 0.0, 0.0))
        .expect("tank");

    game_logic.queue_command(GameCommand {
        command_type: CommandType::Enter {
            target_id: overlord_id,
        },
        player_id: 1,
        command_id: 5,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![tank_id],
        modifier_keys: crate::command_system::ModifierKeys::default(),
    });
    game_logic.process_commands();

    let tank = game_logic.host_object(tank_id).expect("tank");
    assert_ne!(
        tank.ai_state,
        AIState::Entering,
        "vehicles must not enter Overlord BattleBunker residual"
    );
    assert_eq!(game_logic.overlord_bunker_residual_enters(), 0);
    assert!(
        game_logic
            .host_object(overlord_id)
            .unwrap()
            .contained_units()
            .is_empty()
    );
}
