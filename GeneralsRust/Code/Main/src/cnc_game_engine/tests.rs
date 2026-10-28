// Executable engine behavior regressions.
use super::{
    CnCGameEngine, GameMode, GameState, StartupNewGameDispatch, should_exit_for_smoke_test,
    should_keep_logic_running_while_iconic,
};
use crate::command_line::CommandLineArgs;
use game_engine::common::global_data::{
    test_isolation_lock, with_global_data_restored as with_global_data_snapshot_restored,
};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

fn with_global_and_startup_state_snapshot_restored<F: FnOnce()>(f: F) {
    let _guard = test_isolation_lock()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let global_snapshot = game_engine::common::global_data::read().clone();
    let previous_difficulty = gamelogic::helpers::TheScriptEngine::get_global_difficulty();
    let previous_rank_points =
        gamelogic::helpers::TheGameLogic::get_rank_points_to_add_at_game_start();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    *game_engine::common::global_data::write() = global_snapshot;
    gamelogic::helpers::TheScriptEngine::set_global_difficulty(previous_difficulty);
    gamelogic::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(previous_rank_points);
    if let Err(payload) = result {
        std::panic::resume_unwind(payload);
    }
}

fn create_temp_test_dir(prefix: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "generals_main_{prefix}_{}_{}",
        std::process::id(),
        nonce
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn startup_deferred_budget_is_disabled() {
    // Re-pinned 2026-09-07: only non-Menu play disables the deferred budget.
    // C++ AssetManager is async; a Menu budget of 0 flips allow_sync_model_loads
    // and 271 missing W3Ds beachball the render thread (hq-fq1h), so Menu
    // (even with no startup frame yet) always budgets >= 4.
    let budget = CnCGameEngine::startup_deferred_model_load_budget(GameState::InGame, None, 0);
    assert_eq!(budget, 0);
    let menu_no_frame = CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, None, 0);
    assert_eq!(menu_no_frame, 4);
}

#[test]
fn startup_deferred_budget_is_enabled_for_visible_menu_frames() {
    let budget = CnCGameEngine::startup_deferred_model_load_budget(GameState::Menu, Some(12), 12);
    assert_eq!(budget, 4);
}

#[test]
fn smoke_test_exit_only_after_menu_startup_complete() {
    assert!(should_exit_for_smoke_test(
        true,
        GameState::Menu,
        1.0,
        false
    ));
    assert!(!should_exit_for_smoke_test(
        false,
        GameState::Menu,
        1.0,
        false
    ));
    assert!(!should_exit_for_smoke_test(
        true,
        GameState::Loading,
        1.0,
        false
    ));
    assert!(!should_exit_for_smoke_test(
        true,
        GameState::Menu,
        0.995,
        false
    ));
    assert!(!should_exit_for_smoke_test(
        true,
        GameState::Menu,
        1.0,
        true
    ));
}

#[test]
fn configured_startup_shell_map_disables_missing_shell_map() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.shell_map_on = true;
            global.writable.shell_map_name = "__definitely_missing_shell_map__".to_string();
        }

        let shell_map = CnCGameEngine::configured_startup_shell_map();
        assert!(shell_map.is_none());

        let global = game_engine::common::global_data::read();
        assert!(!global.writable.shell_map_on);
    });
}

#[test]
fn windowed_menu_attempts_shellmapmd_when_named() {
    // C++ Shell::showShellMap(TRUE) (Shell.cpp:448-466) starts GAME_SHELL
    // with m_shellMapName on the main menu. Pre-fix windowed boot skipped
    // that decode whenever start_in_menu was true (hq-4yc1).
    assert!(
        CnCGameEngine::windowed_shell_map_load_decision(
            true,
            Some(CnCGameEngine::DEFAULT_WINDOWED_SHELL_MAP),
        ),
        "windowed main menu must attempt ShellMapMD when a name is configured"
    );
    assert!(
        !CnCGameEngine::windowed_shell_map_load_decision(true, None),
        "missing shell-map name must fail-soft so Menu can still open"
    );
    assert!(
        !CnCGameEngine::windowed_shell_map_load_decision(true, Some("   ")),
        "blank shell-map name must fail-soft"
    );
}

#[test]
fn configured_startup_shell_map_uses_disk_shellmapmd_when_cache_empty() {
    // C++ GameEngine.cpp:633-642 only disables ShellMapOn when MapCache
    // lacks m_shellMapName. An empty boot cache must still accept a
    // disk-resident ShellMapMD extract (hq-4yc1).
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.shell_map_on = true;
            global.writable.shell_map_name.clear();
        }

        let resolved_asset = CnCGameEngine::resolve_windowed_shell_map_asset(
            CnCGameEngine::DEFAULT_WINDOWED_SHELL_MAP,
        );
        let shell_map = CnCGameEngine::configured_startup_shell_map();
        if let Some(resolved) = resolved_asset {
            let selected = shell_map.expect("disk-resident ShellMapMD must be selected");
            assert!(
                selected.eq_ignore_ascii_case(&resolved)
                    || selected.to_ascii_lowercase().contains("shellmapmd"),
                "windowed boot must select ShellMapMD (got {selected})"
            );
            assert!(
                game_engine::common::global_data::read()
                    .writable
                    .shell_map_on
            );
        } else {
            assert!(
                shell_map.is_none(),
                "missing ShellMapMD asset must fail-soft (documented asset gate)"
            );
            assert!(
                !game_engine::common::global_data::read()
                    .writable
                    .shell_map_on
            );
        }
    });
}

#[test]
fn configured_startup_shell_map_remaps_stale_shellmap1_to_shellmapmd() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.shell_map_on = true;
            global.writable.shell_map_name = r"Maps\ShellMap1\ShellMap1.map".to_string();
        }
        let resolved_md = CnCGameEngine::resolve_windowed_shell_map_asset(
            CnCGameEngine::DEFAULT_WINDOWED_SHELL_MAP,
        )
        .or_else(|| CnCGameEngine::resolve_windowed_shell_map_asset("ShellMapMD"));
        let selected = CnCGameEngine::configured_startup_shell_map();
        if resolved_md.is_some() {
            let selected = selected.expect("stale ShellMap1 must remap to ShellMapMD");
            assert!(
                selected.to_ascii_lowercase().contains("shellmapmd"),
                "got {selected}"
            );
            assert!(
                game_engine::common::global_data::read()
                    .writable
                    .shell_map_on
            );
        }
    });
}

#[test]
fn effective_fps_limit_prefers_script_override() {
    let limit = CnCGameEngine::effective_fps_limit_for_frame(Some(45), false, 30, 2.0, true, true);
    assert_eq!(limit, Some(45));
}

#[test]
fn effective_fps_limit_honors_cpp_tivo_replay_rule_for_global_limit() {
    let limit = CnCGameEngine::effective_fps_limit_for_frame(None, true, 30, 1.0, true, true);
    assert_eq!(limit, None);
}

#[test]
fn effective_fps_limit_disables_global_limit_for_fast_visual_multiplier() {
    let limit = CnCGameEngine::effective_fps_limit_for_frame(None, true, 30, 1.5, false, false);
    assert_eq!(limit, None);
}

#[test]
fn startup_new_game_dispatch_prefers_last_queued_message() {
    use game_engine::common::message_stream::{GameMessage, GameMessageType};

    let mut first = GameMessage::new(GameMessageType::NewGame);
    first.append_integer_argument(0);
    first.append_integer_argument(0);
    first.append_integer_argument(0);

    let mut replay = GameMessage::new(GameMessageType::NewGame);
    replay.append_integer_argument(3);
    replay.append_integer_argument(1);
    replay.append_integer_argument(42);
    replay.append_integer_argument(90);

    let dispatch = CnCGameEngine::startup_new_game_dispatch_from_messages(&[
        first,
        GameMessage::new(GameMessageType::ClearGameData),
        replay,
    ])
    .expect("expected startup dispatch");

    assert_eq!(dispatch.game_mode, GameMode::Replay);
    assert_eq!(dispatch.difficulty, super::GameDifficulty::Medium);
    assert_eq!(dispatch.rank_points, 42);
    assert_eq!(dispatch.max_fps, Some(90));
}

#[test]
fn startup_dispatch_keeps_difficulty_and_rank_under_the_receiving_owner() {
    with_global_and_startup_state_snapshot_restored(|| {
        // A foreign standalone Core runtime is not this receiving session.
        gamelogic::helpers::TheScriptEngine::set_global_difficulty(0);
        gamelogic::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(13);
        let dispatch = StartupNewGameDispatch {
            game_mode_code: 0,
            game_mode: GameMode::SinglePlayer,
            difficulty_code: 2,
            difficulty: super::GameDifficulty::Hard,
            rank_points: 77,
            max_fps: None,
        };

        let prepared_map = CnCGameEngine::apply_application_new_game_dispatch(dispatch);
        assert!(prepared_map.is_none());
        assert_eq!(
            gamelogic::helpers::TheScriptEngine::get_global_difficulty(),
            0
        );
        let mut owner = crate::game_logic::GameLogic::new();
        let peer = crate::game_logic::GameLogic::new();
        CnCGameEngine::admit_startup_simulation_inputs(&mut owner, Some(dispatch));
        assert_eq!(owner.rank_points_to_add_at_game_start(), 77);
        assert_eq!(peer.rank_points_to_add_at_game_start(), 0);
        let snapshot = crate::save_load::SnapshotBuilder::new()
            .create_world_snapshot(&owner)
            .unwrap();
        assert_eq!(snapshot.persist_v18.rank_points_to_add_at_game_start, 77);
        assert_eq!(owner.get_difficulty(), crate::ai::AIDifficulty::Hard);
        assert_eq!(peer.get_difficulty(), crate::ai::AIDifficulty::Medium);
        assert_eq!(
            owner
                .owned_script_engine
                .as_ref()
                .unwrap()
                .get_global_difficulty(),
            gamelogic::player::GameDifficulty::Hard
        );
        assert_eq!(
            peer.owned_script_engine
                .as_ref()
                .unwrap()
                .get_global_difficulty(),
            gamelogic::player::GameDifficulty::Normal
        );
        assert_eq!(
            gamelogic::helpers::TheGameLogic::get_rank_points_to_add_at_game_start(),
            13
        );
    });
}

#[test]
fn startup_new_game_dispatch_requires_pending_file_for_startup_map_preparation() {
    with_global_and_startup_state_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.map_name = "Maps\\Unexpected\\Unexpected.map".to_string();
            global.pending_file.clear();
        }

        let dispatch = StartupNewGameDispatch {
            game_mode_code: 0,
            game_mode: GameMode::SinglePlayer,
            difficulty_code: 1,
            difficulty: super::GameDifficulty::Medium,
            rank_points: 0,
            max_fps: None,
        };

        let prepared_map = CnCGameEngine::apply_application_new_game_dispatch(dispatch);
        assert!(prepared_map.is_none());

        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.map_name, "Maps\\Unexpected\\Unexpected.map");
        assert!(global.pending_file.is_empty());
    });
}

#[test]
fn owned_new_game_map_preserves_peer_pending_file_and_precedes_fallback() {
    with_global_and_startup_state_snapshot_restored(|| {
        let peer_pending = "Maps/Peer/StillPending.map";
        game_engine::common::global_data::write().pending_file = peer_pending.into();
        let dispatch = StartupNewGameDispatch {
            game_mode_code: 2,
            game_mode: GameMode::Skirmish,
            difficulty_code: 1,
            difficulty: super::GameDifficulty::Medium,
            rank_points: 0,
            max_fps: Some(73),
        };

        let prepared = CnCGameEngine::prepare_application_new_game_map(
            Some("Maps/Receiver/Owned.map"),
            false,
            dispatch,
        );
        assert!(prepared.is_none());
        assert_eq!(
            game_engine::common::global_data::read().pending_file,
            peer_pending,
            "an owned request must not consume a peer pending map"
        );
        {
            let global = game_engine::common::global_data::read();
            assert!(global.writable.use_fps_limit);
            assert_eq!(global.writable.frames_per_second_limit, 73);
        }
        let selected = CnCGameEngine::resolve_application_new_game_map(
            GameMode::Skirmish,
            None,
            Some("Maps/Receiver/Owned.map".into()),
            prepared,
            Some("Maps/Receiver/Previous.map".into()),
        );
        assert_eq!(selected, "Maps/Receiver/Owned.map");
        assert_eq!(
            game_engine::common::global_data::read().pending_file,
            peer_pending
        );

        let peer_selected = CnCGameEngine::resolve_application_new_game_map(
            GameMode::Skirmish,
            None,
            Some("Maps/Peer/Owned.map".into()),
            None,
            Some("Maps/Peer/Previous.map".into()),
        );
        assert_eq!(peer_selected, "Maps/Peer/Owned.map");
        assert_eq!(
            CnCGameEngine::resolve_application_new_game_map(
                GameMode::Skirmish,
                None,
                None,
                None,
                Some("Maps/Receiver/Previous.map".into()),
            ),
            "Maps/Receiver/Previous.map",
            "a receiver keeps its configured fallback when a request has no map"
        );
        let campaign = CnCGameEngine::select_new_game_request_map(
            GameMode::SinglePlayer,
            Some("Maps/Campaign/Owned.map".into()),
            None,
            true,
            dispatch,
            Some("Maps/Receiver/Previous.map".into()),
        );
        assert_eq!(campaign, "Maps/Campaign/Owned.map");
        assert_eq!(
            CnCGameEngine::resolve_application_new_game_map(
                GameMode::Skirmish,
                Some(" ".into()),
                Some("Maps/Receiver/Owned.map".into()),
                None,
                None,
            ),
            "Maps/Receiver/Owned.map"
        );
        assert_eq!(
            game_engine::common::global_data::read().pending_file,
            peer_pending,
            "an owned campaign map must not consume another request's pending map"
        );
    });
}

#[test]
fn deferred_runtime_new_game_keeps_its_map_after_shell_command_precedes_it() {
    with_global_and_startup_state_snapshot_restored(
        deferred_runtime_new_game_keeps_its_map_after_shell_command_precedes_it_body,
    );
}

fn deferred_runtime_new_game_keeps_its_map_after_shell_command_precedes_it_body() {
    use game_engine::common::message_stream::{
        ClientShellMapLaunch, GameMessage, GameMessageType, MessageStream,
    };

    let peer_pending = "Maps/Peer/Pending.map";
    game_engine::common::global_data::write().pending_file = peer_pending.into();
    let mut ordered = MessageStream::new();
    let shell = ordered.append_message(GameMessageType::NewGame);
    shell.append_integer_argument(4); // GAME_SHELL
    shell.append_integer_argument(1);
    shell.append_integer_argument(0);
    shell.append_integer_argument(30);
    shell.set_shell_map_launch(ClientShellMapLaunch {
        map: "Maps/Shell/Shell.map".into(),
        seed: 1,
    });

    let mut host_new_game = GameMessage::new(GameMessageType::NewGame);
    host_new_game.append_integer_argument(2); // GAME_SKIRMISH
    host_new_game.append_integer_argument(1);
    host_new_game.append_integer_argument(0);
    host_new_game.append_integer_argument(30);
    host_new_game.set_owned_new_game_map("Maps/Host/Owned.map".into());
    CnCGameEngine::append_common_message_to_stream(&mut ordered, &host_new_game);

    assert!(CnCGameEngine::new_game_drain_waits_for_owned_launch(
        ordered.get_messages()
    ));

    // The shell handler consumes its message while the host NewGame remains
    // ordered. Rebuild through Main's production forwarding helper, which
    // must retain the exact request-local map descriptor.
    let deferred_host_message = ordered
        .get_messages()
        .iter()
        .last()
        .expect("host NewGame remains after Shell")
        .clone();
    let mut deferred = MessageStream::new();
    CnCGameEngine::append_common_message_to_stream(&mut deferred, &deferred_host_message);
    assert!(!CnCGameEngine::new_game_drain_waits_for_owned_launch(
        deferred.get_messages()
    ));
    let forwarded = deferred.get_messages().iter().next().unwrap();
    assert_eq!(forwarded.owned_new_game_map(), Some("Maps/Host/Owned.map"));
    let dispatch = CnCGameEngine::startup_new_game_dispatch_from_message(forwarded)
        .expect("deferred NewGame remains a valid command");
    assert_eq!(dispatch.game_mode, GameMode::Skirmish);
    let request_map = CnCGameEngine::select_new_game_request_map(
        dispatch.game_mode,
        None,
        forwarded.owned_new_game_map().map(str::to_owned),
        true,
        dispatch,
        Some("Maps/Receiver/Fallback.map".into()),
    );
    assert_eq!(request_map, "Maps/Host/Owned.map");
    assert_eq!(
        game_engine::common::global_data::read().pending_file,
        peer_pending,
        "ordered NewGame map must not consume a peer pending map"
    );

    let mut replay = GameMessage::new(GameMessageType::NewGame);
    replay.append_integer_argument(3); // GAME_REPLAY
    replay.append_integer_argument(1);
    replay.append_integer_argument(0);
    replay.append_integer_argument(30);
    replay.set_owned_new_game_map("Maps/Host/ReplayPlaceholder.map".into());
    CnCGameEngine::consume_legacy_pending_map_for_recorded_new_game(&replay);
    assert_eq!(
        replay.owned_new_game_map(),
        Some("Maps/Host/ReplayPlaceholder.map"),
        "an owned request map is not substituted for Recorder::GameInfo"
    );
    assert_eq!(
        game_engine::common::global_data::read().pending_file,
        peer_pending,
        "an owned Replay request must not consume a peer pending map"
    );
}

#[test]
fn map_select_new_game_dispatch_consumes_the_exact_runtime_pending_map() {
    use game_engine::common::message_stream::{GameMessage, GameMessageType};

    with_global_and_startup_state_snapshot_restored(|| {
        let expected_map = "Maps\\Official\\MapSelectExact.map".to_string();
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.map_name = "Maps\\Stale\\Stale.map".to_string();
            global.pending_file = expected_map.clone();
        }

        let mut message = GameMessage::new(GameMessageType::NewGame);
        message.append_integer_argument(0); // GAME_SINGLE_PLAYER
        message.append_integer_argument(1); // DIFFICULTY_NORMAL
        message.append_integer_argument(0);
        let dispatch = CnCGameEngine::startup_new_game_dispatch_from_message(&message)
            .expect("MapSelect NewGame must decode");

        assert_eq!(dispatch.game_mode, GameMode::SinglePlayer);
        assert_eq!(
            CnCGameEngine::apply_application_new_game_dispatch(dispatch),
            Some(expected_map.clone())
        );

        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.map_name, expected_map);
        assert!(global.pending_file.is_empty());
    });
}

#[cfg(feature = "game_client")]
#[test]
fn campaign_launch_descriptor_precedes_stale_map_and_hud_faction() {
    use game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDescriptor;

    let descriptor = HostCampaignLaunchDescriptor {
        generation: 7,
        map_name: "Maps\\Campaign\\ExactLaunch.map".to_string(),
        campaign_name: "USA".to_string(),
        // Deliberately differs from the campaign name: C++'s campaign player
        // faction is more specific than a stale HUD/default fallback.
        campaign_player_faction: "FactionChina".to_string(),
        is_challenge: false,
        game_info_starting_cash: None,
        player_template_name: None,
        player_template_index: None,
        game_mode_code: 0,
        difficulty_code: 1,
        rank_points: 0,
        max_fps: None,
    };

    let overrides =
        CnCGameEngine::campaign_launch_start_overrides(GameMode::SinglePlayer, Some(&descriptor))
            .expect("ordinary campaign descriptor must resolve");
    assert_eq!(
        overrides.map.as_deref(),
        Some("Maps\\Campaign\\ExactLaunch.map")
    );
    assert_eq!(overrides.faction.as_deref(), Some("China"));
    assert_eq!(
        overrides
            .player_template
            .as_ref()
            .map(|identity| identity.template_name.as_str()),
        Some("FactionChina"),
        "a normal campaign PlayerFaction that exactly resolves must retain its template"
    );

    let overrides =
        CnCGameEngine::campaign_launch_start_overrides(GameMode::Skirmish, Some(&descriptor))
            .expect("non-single-player dispatch ignores the campaign descriptor");
    assert!(overrides.map.is_none());
    assert!(overrides.faction.is_none());
    assert!(overrides.player_template.is_none());
}

#[cfg(feature = "game_client")]
#[test]
fn challenge_launch_rejects_a_missing_or_unpaired_selected_general() {
    use game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDescriptor;

    let descriptor = HostCampaignLaunchDescriptor {
        generation: 9,
        map_name: "Maps\\Challenge\\Exact.map".to_string(),
        campaign_name: "CHALLENGE_0".to_string(),
        campaign_player_faction: "FactionAmericaAirForceGeneral".to_string(),
        is_challenge: true,
        game_info_starting_cash: Some(0),
        player_template_name: None,
        player_template_index: None,
        game_mode_code: 0,
        difficulty_code: 1,
        rank_points: 0,
        max_fps: Some(30),
    };

    assert!(
        CnCGameEngine::campaign_launch_start_overrides(GameMode::SinglePlayer, Some(&descriptor),)
            .is_err()
    );
}

#[cfg(feature = "game_client")]
#[test]
fn challenge_launch_retains_the_exact_selected_template_name_and_index() {
    use game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDescriptor;

    game_engine::common::ini::ensure_player_templates_loaded();
    let (tank_index, air_force_index) = {
        let store = game_engine::common::rts::player_template::get_player_template_store();
        (
            store
                .find_template_index("FactionChinaTankGeneral")
                .expect("retail Tank General template") as i32,
            store
                .find_template_index("FactionAmericaAirForceGeneral")
                .expect("retail Air Force General template") as i32,
        )
    };

    let descriptor = HostCampaignLaunchDescriptor {
        generation: 10,
        map_name: "Maps\\Challenge\\ExactTank.map".to_string(),
        campaign_name: "CHALLENGE_0".to_string(),
        campaign_player_faction: "FactionChinaTankGeneral".to_string(),
        is_challenge: true,
        game_info_starting_cash: Some(0),
        player_template_name: Some("FactionChinaTankGeneral".to_string()),
        player_template_index: Some(tank_index),
        game_mode_code: 0,
        difficulty_code: 1,
        rank_points: 0,
        max_fps: Some(30),
    };

    let overrides =
        CnCGameEngine::campaign_launch_start_overrides(GameMode::SinglePlayer, Some(&descriptor))
            .expect("matched Challenge template must resolve");
    let identity = overrides
        .player_template
        .expect("Challenge must carry exact PlayerTemplate identity");
    assert_eq!(identity.template_name, "FactionChinaTankGeneral");
    assert_eq!(identity.template_index, Some(tank_index));
    assert_eq!(overrides.faction.as_deref(), Some("China"));
    assert_eq!(
        overrides.game_info_starting_cash,
        Some(0),
        "present Challenge GameInfo cash survives delivery"
    );
    let missing_cash = HostCampaignLaunchDescriptor {
        game_info_starting_cash: None,
        ..descriptor.clone()
    };
    assert!(
        CnCGameEngine::campaign_launch_start_overrides(GameMode::SinglePlayer, Some(&missing_cash))
            .is_err()
    );

    let stale = HostCampaignLaunchDescriptor {
        player_template_index: Some(air_force_index),
        ..descriptor
    };
    assert!(
        CnCGameEngine::campaign_launch_start_overrides(GameMode::SinglePlayer, Some(&stale),)
            .is_err()
    );
}

#[test]
fn campaign_faction_identity_maps_only_exact_cpp_base_names() {
    assert_eq!(
        CnCGameEngine::base_faction_from_campaign_faction("FactionAmerica").as_deref(),
        Some("USA")
    );
    assert_eq!(
        CnCGameEngine::base_faction_from_campaign_faction("china").as_deref(),
        Some("China")
    );
    assert_eq!(
        CnCGameEngine::base_faction_from_campaign_faction("FactionGLA").as_deref(),
        Some("GLA")
    );
    assert!(CnCGameEngine::base_faction_from_campaign_faction("TankGeneral").is_none());
}

#[test]
fn startup_new_game_dispatch_ignores_unrelated_messages() {
    use game_engine::common::message_stream::{GameMessage, GameMessageType};

    let dispatch = CnCGameEngine::startup_new_game_dispatch_from_messages(&[
        GameMessage::new(GameMessageType::Invalid),
        GameMessage::new(GameMessageType::ClearGameData),
    ]);

    assert!(dispatch.is_none());
}

#[test]
fn take_new_game_dispatch_drains_stream_and_keeps_other_messages() {
    use game_engine::common::message_stream::{GameMessage, GameMessageType, MessageStream};

    let mut stream = MessageStream::new();
    stream.append_message(GameMessageType::ClearGameData);
    let ng = stream.append_message(GameMessageType::NewGame);
    ng.append_integer_argument(2); // GAME_SKIRMISH
    ng.append_integer_argument(1);
    ng.append_integer_argument(0);
    ng.append_integer_argument(30);
    stream.append_message(GameMessageType::Invalid);

    let dispatch = CnCGameEngine::take_new_game_dispatch_from_common_stream(&mut stream)
        .expect("NewGame must be drained");
    assert_eq!(dispatch.game_mode, GameMode::Skirmish);
    assert_eq!(dispatch.max_fps, Some(30));
    assert_eq!(
        stream.message_count(),
        2,
        "non-NewGame messages must remain"
    );
    let types: Vec<_> = stream
        .get_messages()
        .iter()
        .map(|m| m.get_type().clone())
        .collect();
    assert!(
        types
            .iter()
            .any(|t| matches!(t, GameMessageType::ClearGameData))
    );
    assert!(types.iter().any(|t| matches!(t, GameMessageType::Invalid)));
    assert!(!types.iter().any(|t| matches!(t, GameMessageType::NewGame)));
    let _ = GameMessage::new(GameMessageType::Invalid);
}

#[test]
fn take_clear_game_data_drains_stream_and_keeps_other_messages() {
    use game_engine::common::message_stream::{GameMessageType, MessageStream};

    let mut stream = MessageStream::new();
    stream.append_message(GameMessageType::Invalid);
    stream.append_message(GameMessageType::ClearGameData);

    assert!(CnCGameEngine::take_clear_game_data_from_common_stream(
        &mut stream
    ));
    let types: Vec<_> = stream
        .get_messages()
        .iter()
        .map(|m| m.get_type().clone())
        .collect();
    assert!(
        !types
            .iter()
            .any(|t| matches!(t, GameMessageType::ClearGameData))
    );
    assert!(types.iter().any(|t| matches!(t, GameMessageType::Invalid)));
    assert!(!CnCGameEngine::take_clear_game_data_from_common_stream(
        &mut stream
    ));
}

#[test]
fn clear_game_data_pushes_score_screen_not_main_menu() {
    // C++ GameLogicDispatch.cpp:223-253 / :439 default showScoreScreen.
    assert!(CnCGameEngine::clear_game_data_should_push_score_screen(
        false, true, true
    ));
    assert!(
        !CnCGameEngine::clear_game_data_should_push_score_screen(true, true, true),
        "in-shell + in-game must not push ScoreScreen"
    );
    assert!(!CnCGameEngine::clear_game_data_should_push_score_screen(
        false, true, false
    ));
}

#[test]
fn peek_new_game_leaves_message_for_propagate_messages() {
    // C++ GameLogic::logicMessageDispatcher MSG_NEW_GAME
    // (GameLogicDispatch.cpp:396-423) consumes the streamed message after
    // MessageStream::propagateMessages. Host start peeks so pump can still
    // deliver NewGame to crate GameLogic.
    use game_engine::common::message_stream::{GameMessageType, MessageStream};

    let mut stream = MessageStream::new();
    stream.append_message(GameMessageType::ClearGameData);
    let ng = stream.append_message(GameMessageType::NewGame);
    ng.append_integer_argument(2); // GAME_SKIRMISH
    ng.append_integer_argument(1);
    ng.append_integer_argument(0);
    ng.append_integer_argument(30);

    let peeked = CnCGameEngine::peek_new_game_dispatch_from_common_stream(&stream)
        .expect("NewGame must be visible without removing it");
    assert_eq!(peeked.game_mode, GameMode::Skirmish);
    assert_eq!(peeked.max_fps, Some(30));

    {
        assert!(
            stream
                .get_messages()
                .iter()
                .any(|m| matches!(m.get_type(), GameMessageType::NewGame)),
            "peek must leave MSG_NEW_GAME on the stream for pump_message_stream"
        );
    }

    let _ = CnCGameEngine::take_new_game_dispatch_from_common_stream(&mut stream);
}

#[test]
fn startup_camera_metadata_converts_cpp_ground_axes() {
    // ShellMapMD's InitialCameraPosition is serialized as C++ X/Y ground,
    // Z height. The Rust renderer uses X/Z ground and Y height.
    let focus = CnCGameEngine::cpp_initial_camera_ground_focus([827.8885, 234.29129, 0.0]);
    assert_eq!(focus, glam::Vec2::new(827.8885, 234.29129));
}

#[test]
fn startup_camera_focus_prefers_shell_metadata_before_default_seed() {
    let focus = CnCGameEngine::select_startup_camera_focus(
        true,
        Some(glam::Vec2::new(12.0, 34.0)),
        Some(glam::Vec2::new(56.0, 78.0)),
        glam::Vec2::new(90.0, 91.0),
    );

    assert_eq!(focus, glam::Vec2::new(12.0, 34.0));
}

#[test]
fn startup_camera_focus_falls_back_to_shell_seed_without_metadata() {
    let focus = CnCGameEngine::select_startup_camera_focus(
        true,
        None,
        Some(glam::Vec2::new(56.0, 78.0)),
        glam::Vec2::new(90.0, 91.0),
    );

    assert_eq!(
        focus,
        glam::Vec2::new(
            87.0 * gamelogic::common::MAP_XY_FACTOR,
            77.0 * gamelogic::common::MAP_XY_FACTOR,
        )
    );
}

#[test]
fn startup_camera_focus_keeps_non_shell_fallback_order() {
    let focus = CnCGameEngine::select_startup_camera_focus(
        false,
        None,
        Some(glam::Vec2::new(56.0, 78.0)),
        glam::Vec2::new(90.0, 91.0),
    );

    assert_eq!(focus, glam::Vec2::new(56.0, 78.0));
}

#[test]
fn startup_mode_requires_new_game_dispatch_for_non_menu_startup() {
    let mut start_in_menu = false;
    let mut map_to_load = Some("Maps\\ShellMapMD\\ShellMapMD.map".to_string());

    let mode = CnCGameEngine::resolve_startup_mode_from_dispatch(
        &mut start_in_menu,
        &mut map_to_load,
        None,
        false,
    );

    assert_eq!(mode, GameMode::Shell);
    assert!(start_in_menu);
    assert!(map_to_load.is_none());
}

#[test]
fn startup_initial_file_uses_candidate_map_without_consuming_peer_pending_file() {
    with_global_and_startup_state_snapshot_restored(|| {
        let args = CommandLineArgs::parse_from_args(vec![
            "generals".to_string(),
            "-file".to_string(),
            "Maps\\Candidate\\OwnedStart.map".to_string(),
        ])
        .expect("parse initial-file command line");
        let selected = CnCGameEngine::startup_initial_file_from_command_line(&args, true)
            .expect("initial file is enabled");
        let peer_pending = "Maps\\Peer\\StillPending.map";
        game_engine::common::global_data::write().pending_file = peer_pending.to_string();

        let dispatch = StartupNewGameDispatch {
            game_mode_code: 0,
            game_mode: GameMode::SinglePlayer,
            difficulty_code: 1,
            difficulty: super::GameDifficulty::Medium,
            rank_points: 0,
            max_fps: None,
        };
        let mut start_in_menu = false;
        let mut map_to_load = Some(selected.clone());
        let mode = CnCGameEngine::resolve_startup_mode_from_dispatch(
            &mut start_in_menu,
            &mut map_to_load,
            Some(dispatch),
            false,
        );

        assert_eq!(mode, GameMode::SinglePlayer);
        assert!(!start_in_menu);
        assert_eq!(map_to_load.as_deref(), Some(selected.as_str()));
        assert_eq!(
            game_engine::common::global_data::read().pending_file,
            peer_pending,
            "candidate startup must not consume another owner's pending map"
        );

        let mut start_in_menu = false;
        let mut map_to_load = Some(selected);
        let fallback = CnCGameEngine::resolve_startup_mode_from_dispatch(
            &mut start_in_menu,
            &mut map_to_load,
            None,
            true,
        );
        assert_eq!(fallback, GameMode::Shell);
        assert!(start_in_menu);
        assert!(map_to_load.is_none());
        assert_eq!(
            game_engine::common::global_data::read().pending_file,
            peer_pending,
            "failed candidate fallback must not clear another owner's pending map"
        );
    });
}

#[test]
fn startup_initial_file_helper_matches_cpp_table_and_gating() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.initial_file.clear();
        }

        let replay_args = vec![
            "generals".to_string(),
            "-file".to_string(),
            "Replays\\demo.rep".to_string(),
        ];
        let replay_parsed = CommandLineArgs::parse_from_args(replay_args).unwrap();
        assert_eq!(
            CnCGameEngine::startup_initial_file_from_command_line(&replay_parsed, true),
            Some("Replays\\demo.rep".to_string())
        );
        assert_eq!(
            CnCGameEngine::startup_initial_file_from_command_line(&replay_parsed, false),
            None
        );

        let replay_alias_args = vec![
            "generals".to_string(),
            "-replay".to_string(),
            "Replays\\demo.rep".to_string(),
        ];
        let replay_alias_parsed = CommandLineArgs::parse_from_args(replay_alias_args).unwrap();
        assert_eq!(
            CnCGameEngine::startup_initial_file_from_command_line(&replay_alias_parsed, true),
            None
        );
    });
}

#[test]
fn startup_initial_file_helper_prefers_runtime_initial_file_state() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.initial_file = "Replays\\runtime.rep".to_string();
        }

        let cli_args = vec![
            "generals".to_string(),
            "-file".to_string(),
            "Maps\\cli\\cli.map".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(cli_args).unwrap();

        assert_eq!(
            CnCGameEngine::startup_initial_file_from_command_line(&parsed, true),
            Some("Replays\\runtime.rep".to_string())
        );
    });
}

#[test]
fn startup_initial_file_split_matches_cpp_suffix_rules() {
    let (map_file, replay_file) =
        CnCGameEngine::split_startup_initial_file(Some("Maps\\Test\\Test.map".to_string()));
    assert_eq!(map_file, Some("Maps\\Test\\Test.map".to_string()));
    assert!(replay_file.is_none());

    let (map_file, replay_file) =
        CnCGameEngine::split_startup_initial_file(Some("Replays\\demo.rep".to_string()));
    assert!(map_file.is_none());
    assert_eq!(replay_file, Some("Replays\\demo.rep".to_string()));
}

#[test]
fn apply_command_line_overrides_keeps_initial_map_side_effects_until_startup_handling() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-file".to_string(),
            "Maps\\Test\\Test.map".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.initial_file, "Maps\\Test\\Test.map");
        assert!(global.pending_file.is_empty());
        assert!(global.writable.shell_map_on);
        assert!(global.writable.play_intro);
        assert!(!global.writable.after_intro);
    });
}

#[test]
fn sync_after_intro_when_intro_disabled_marks_after_intro() {
    with_global_data_snapshot_restored(|| {
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.play_intro = false;
            global.writable.after_intro = false;
        }

        CnCGameEngine::sync_after_intro_when_intro_disabled();

        let global = game_engine::common::global_data::read();
        assert!(!global.writable.play_intro);
        assert!(global.writable.after_intro);
    });
}

#[test]
fn game_logic_gate_without_network_matches_cpp_pause_behavior() {
    assert!(CnCGameEngine::should_update_game_logic_frame(false, None));
    assert!(!CnCGameEngine::should_update_game_logic_frame(true, None));
}

#[test]
fn game_logic_gate_with_network_uses_frame_ready_only() {
    assert!(CnCGameEngine::should_update_game_logic_frame(
        false,
        Some(true)
    ));
    assert!(CnCGameEngine::should_update_game_logic_frame(
        true,
        Some(true)
    ));
    assert!(!CnCGameEngine::should_update_game_logic_frame(
        false,
        Some(false)
    ));
    assert!(!CnCGameEngine::should_update_game_logic_frame(
        true,
        Some(false)
    ));
}

#[test]
fn network_gate_skips_runtime_network_lookup_until_multiplayer_exists() {
    assert_eq!(CnCGameEngine::network_frame_data_ready_gate(false), None);
}

#[test]
fn iconic_minimized_mode_keeps_network_sessions_running() {
    assert!(should_keep_logic_running_while_iconic(
        GameMode::Multiplayer
    ));
    assert!(should_keep_logic_running_while_iconic(GameMode::Lan));
    assert!(should_keep_logic_running_while_iconic(GameMode::Internet));
    assert!(!should_keep_logic_running_while_iconic(
        GameMode::SinglePlayer
    ));
    assert!(!should_keep_logic_running_while_iconic(GameMode::Skirmish));
    assert!(!should_keep_logic_running_while_iconic(GameMode::Shell));
}

#[test]
fn command_line_no_show_client_physics_is_projected_to_receiving_rules() {
    let args = vec!["generals".to_string(), "-noShowClientPhysics".to_string()];
    let mut writable = game_engine::common::command_line::WritableGlobalData::default();
    CnCGameEngine::apply_ordered_startup_overrides_from_raw_args(&args, &mut writable, false);
    assert!(!writable.show_client_physics);

    let args = vec!["generals".to_string()];
    let mut writable = game_engine::common::command_line::WritableGlobalData::default();
    CnCGameEngine::apply_ordered_startup_overrides_from_raw_args(&args, &mut writable, false);
    assert!(writable.show_client_physics);
}

#[test]
fn command_line_fps_order_matches_cpp_fps_then_nofpslimit() {
    let args = vec![
        "generals".to_string(),
        "-fps".to_string(),
        "60".to_string(),
        "-nofpslimit".to_string(),
    ];
    let mut writable = game_engine::common::command_line::WritableGlobalData::default();
    CnCGameEngine::apply_fps_limit_overrides_from_raw_args(&args, &mut writable);
    assert!(!writable.use_fps_limit);
    assert_eq!(writable.frames_per_second_limit, 30000);
}

#[test]
fn command_line_fps_order_matches_cpp_nofpslimit_then_fps() {
    let args = vec![
        "generals".to_string(),
        "-nofpslimit".to_string(),
        "-fps".to_string(),
        "60".to_string(),
    ];
    let mut writable = game_engine::common::command_line::WritableGlobalData::default();
    CnCGameEngine::apply_fps_limit_overrides_from_raw_args(&args, &mut writable);
    assert!(!writable.use_fps_limit);
    assert_eq!(writable.frames_per_second_limit, 60);
}

#[test]
fn command_line_window_resolution_overrides_sync_to_writable_globals() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-win".to_string(),
            "-xres".to_string(),
            "1280".to_string(),
            "-yres".to_string(),
            "720".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(global.writable.windowed);
        assert_eq!(global.writable.x_resolution, 1280);
        assert_eq!(global.writable.y_resolution, 720);
    });
}

#[test]
fn command_line_noaudio_overrides_sync_to_writable_globals() {
    with_global_data_snapshot_restored(|| {
        let args = vec!["generals".to_string(), "-noaudio".to_string()];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(!global.writable.audio_on);
        assert!(!global.writable.speech_on);
        assert!(!global.writable.sounds_on);
        assert!(!global.writable.music_on);
    });
}

#[test]
fn command_line_startup_parity_flags_apply_in_argv_order() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-particleEdit".to_string(),
            "-fullscreen".to_string(),
            "-benchmark".to_string(),
            "9".to_string(),
            "-playStats".to_string(),
            "4".to_string(),
            "-seed".to_string(),
            "-1".to_string(),
            "-netMinPlayers".to_string(),
            "3".to_string(),
            "-forceBenchmark".to_string(),
            "-nomusic".to_string(),
            "-noshaders".to_string(),
            "-scriptDebug".to_string(),
            "-winCursors".to_string(),
            "-constantDebug".to_string(),
            "-showTeamDot".to_string(),
            "-nomovecamera".to_string(),
            "-NoShellAnim".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(!global.writable.windowed);
        assert!(global.writable.particle_edit);
        assert!(global.writable.script_debug);
        assert!(global.writable.win_cursors);
        assert!(!global.writable.animate_windows);
        assert!(!global.writable.music_on);
        assert!(global.writable.play_sizzle);
        assert_eq!(global.writable.chip_set_type, 1);
        assert!(global.writable.force_benchmark);
        assert!(global.writable.constant_debug_update);
        assert!(global.writable.show_team_dot);
        assert!(global.writable.disable_camera_movement);
        assert_eq!(global.writable.fixed_seed, -1);
        assert_eq!(global.writable.net_min_players, 3);
        assert_eq!(global.writable.benchmark_timer, 9);
        assert_eq!(global.writable.play_stats, 4);
    });
}

#[test]
fn command_line_standalone_nosizzle_is_ignored_during_startup_overrides() {
    with_global_data_snapshot_restored(|| {
        let args = vec!["generals".to_string(), "-nosizzle".to_string()];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(global.writable.play_sizzle);
    });
}

#[test]
fn command_line_jump_to_frame_matches_cpp_no_draw_behavior() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-jumpToFrame".to_string(),
            "240".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        let debug_gated = CnCGameEngine::allow_debug_startup_flags();
        assert_eq!(global.writable.no_draw, debug_gated);
        if debug_gated {
            assert!(!global.writable.use_fps_limit);
            assert_eq!(global.writable.frames_per_second_limit, 30000);
        }
    });
}

#[test]
fn startup_water_weather_preload_paths_match_cpp_order() {
    assert_eq!(
        CnCGameEngine::startup_water_weather_ini_paths(),
        [
            "Data/INI/Default/Water.ini",
            "Data/INI/Water.ini",
            "Data/INI/Default/Weather.ini",
            "Data/INI/Weather.ini",
        ]
    );
}

#[test]
fn preload_startup_weather_inis_returns() {
    // Shipped boot path: must return (fail-open on missing files). A hang here
    // is the observed Loading stall at "Preloading water and weather settings".
    let start = std::time::Instant::now();
    CnCGameEngine::preload_startup_weather_inis();
    assert!(
        start.elapsed() < std::time::Duration::from_secs(15),
        "preload_startup_weather_inis must return quickly, elapsed {:?}",
        start.elapsed()
    );
}

#[test]
fn startup_ai_data_preload_paths_match_cpp_order() {
    assert_eq!(
        CnCGameEngine::startup_ai_data_ini_paths(),
        ["Data/INI/Default/AIData.ini", "Data/INI/AIData.ini",]
    );
}

#[test]
fn startup_audio_failure_quits_only_when_audio_is_enabled() {
    assert!(CnCGameEngine::startup_audio_should_quit(false, false));
    assert!(!CnCGameEngine::startup_audio_should_quit(true, false));
    assert!(!CnCGameEngine::startup_audio_should_quit(false, true));
}

#[test]
fn debug_startup_flag_gating_matches_build_mode() {
    assert_eq!(
        CnCGameEngine::allow_debug_startup_flags(),
        cfg!(any(debug_assertions, feature = "internal"))
    );
}

#[test]
fn command_line_map_override_syncs_to_writable_globals() {
    with_global_data_snapshot_restored(|| {
        let args = vec![
            "generals".to_string(),
            "-map".to_string(),
            "Maps\\ShellMap1.map".to_string(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.map_name, "Maps\\ShellMap1\\ShellMap1.map");
    });
}

#[test]
fn command_line_mod_override_updates_active_mod_and_loads_best_effort() {
    with_global_data_snapshot_restored(|| {
        let temp_root = create_temp_test_dir("mod_override");
        let user_data_dir = temp_root.join("UserData");
        let mod_dir = user_data_dir.join("Mods").join("TestMod");
        std::fs::create_dir_all(&mod_dir).unwrap();

        {
            let mut global = game_engine::common::global_data::write();
            global.set_user_data_dir(user_data_dir.to_string_lossy().into_owned());
        }

        let args = vec![
            "generals".to_string(),
            "-mod".to_string(),
            std::path::Path::new("Mods")
                .join("TestMod")
                .to_string_lossy()
                .into_owned(),
        ];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let expected = format!("{}{}", mod_dir.to_string_lossy(), std::path::MAIN_SEPARATOR);
        let global = game_engine::common::global_data::read();
        assert_eq!(global.writable.mod_dir, expected);
        assert!(global.writable.mod_big.is_empty());
        assert_eq!(
            global
                .get_override("active_mod")
                .and_then(|value| value.as_str()),
            Some(expected.as_str())
        );

        let _ = fs::remove_dir_all(temp_root);
    });
}

#[test]
fn command_line_update_images_sets_writable_flag() {
    with_global_data_snapshot_restored(|| {
        let args = vec!["generals".to_string(), "-updateimages".to_string()];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(global.writable.should_update_tga_to_dds);
    });
}

#[test]
fn command_line_update_images_alias_is_case_insensitive() {
    with_global_data_snapshot_restored(|| {
        let args = vec!["generals".to_string(), "-UpDaTeDdS".to_string()];
        let parsed = CommandLineArgs::parse_from_args(args).unwrap();
        CnCGameEngine::apply_command_line_overrides(&parsed);

        let global = game_engine::common::global_data::read();
        assert!(global.writable.should_update_tga_to_dds);
    });
}

#[test]
#[cfg(all(feature = "game_client", not(target_arch = "wasm32")))]
fn menu_client_survivors_use_actual_receiving_command_owner_without_core_queue() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "menu_client_survivors_use_actual_receiving_command_owner_without_core_queue",
        || {
            use crate::command_system::{
                flush_recorder_and_replay_authority, stamp_host_logic_frame,
            };
            use game_engine::common::message_stream::{
                Coord3D, GameMessage, GameMessageType, get_message_stream,
            };
            use std::sync::{
                Arc,
                atomic::{AtomicUsize, Ordering},
            };
            let calls = Arc::new(AtomicUsize::new(0));
            let foreign = Arc::clone(&calls);
            game_client::message_stream::command_router::set_host_command_authority(Some(
                Arc::new(move |_| {
                    foreign.fetch_add(1, Ordering::SeqCst);
                }),
            ));
            let client = game_client::core::GameClient::new().unwrap();
            let mut owner = crate::game_logic::GameLogic::new();
            let mut peer = crate::game_logic::GameLogic::new();
            game_client::message_stream::command_list::get_command_list()
                .write()
                .unwrap()
                .retain_messages(|_| false);
            {
                let stream = get_message_stream();
                let mut stream = stream.write().unwrap();
                stream.clear_messages();
                let first = stream.append_message(GameMessageType::DoStop);
                first.set_player_index(0);
                let second =
                    stream.append_message(GameMessageType::DoMoveTo(Coord3D::new(12., 0., 4.)));
                second.set_player_index(3);
            }
            // This is the same production helper used by both Menu branches
            // and runtime-host start after NewGame interception. These ordinary
            // survivors must not depend on Core's absent player0 queue.
            CnCGameEngine::pump_receiving_client_commands(&client, &mut owner.replay_pending)
                .unwrap();
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            stamp_host_logic_frame(&mut owner.replay_pending, 7);
            stamp_host_logic_frame(&mut peer.replay_pending, 7);
            flush_recorder_and_replay_authority(
                &mut owner.replay_pending,
                &mut owner.command_queue,
                0,
            );
            flush_recorder_and_replay_authority(
                &mut peer.replay_pending,
                &mut peer.command_queue,
                0,
            );
            assert_eq!(owner.command_queue.len(), 2);
            assert_eq!(owner.command_queue[0].player_id, 0);
            assert!(matches!(
                owner.command_queue[0].command_type,
                crate::command_system::CommandType::Stop
            ));
            assert_eq!(owner.command_queue[1].player_id, 3);
            assert!(matches!(
                owner.command_queue[1].command_type,
                crate::command_system::CommandType::MoveTo { .. }
            ));
            assert!(peer.command_queue.is_empty());
            CnCGameEngine::pump_receiving_client_commands(&client, &mut owner.replay_pending)
                .unwrap();
            flush_recorder_and_replay_authority(
                &mut owner.replay_pending,
                &mut owner.command_queue,
                0,
            );
            assert_eq!(
                owner.command_queue.len(),
                2,
                "menu survivors queue once, not once per pump"
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
            let error = game_client::message_stream::command_router::route_commands_to_gamelogic(
                vec![GameMessage::with_player(GameMessageType::DoStop, 0)],
                7,
            )
            .unwrap_err();
            assert!(error.to_string().contains("Player 0 not initialized"));
            assert_eq!(
                calls.load(Ordering::SeqCst),
                1,
                "foreign standalone callback remains owned by Core"
            );
        },
    );
}
