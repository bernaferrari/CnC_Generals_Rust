#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

/// Last campaign/Challenge NewGame identity so Restart Mission can re-post
/// PlayerTemplate + difficulty + rank (QuitMenu.cpp:197-216).
#[derive(Clone, Debug)]
pub(super) struct LastNewGameIdentity {
    pub(super) dispatch: StartupNewGameDispatch,
    pub(super) player_template: Option<crate::game_logic::PlayerTemplateIdentity>,
    pub(super) faction: String,
    pub(super) map: String,
}

static LAST_NEW_GAME_IDENTITY: std::sync::Mutex<Option<LastNewGameIdentity>> =
    std::sync::Mutex::new(None);

pub(super) fn record_last_new_game_identity(identity: LastNewGameIdentity) {
    if let Ok(mut guard) = LAST_NEW_GAME_IDENTITY.lock() {
        *guard = Some(identity);
    }
}

pub(super) fn last_new_game_identity() -> Option<LastNewGameIdentity> {
    LAST_NEW_GAME_IDENTITY
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

pub(super) fn update_last_new_game_dispatch(dispatch: StartupNewGameDispatch) {
    if let Ok(mut guard) = LAST_NEW_GAME_IDENTITY.lock() {
        if let Some(identity) = guard.as_mut() {
            identity.dispatch = dispatch;
        }
    }
}

impl CnCGameEngine {
    pub(super) fn append_message_argument_to_common_stream(
        target: &mut game_engine::common::message_stream::GameMessage,
        arg: &game_engine::common::message_stream::GameMessageArgumentType,
    ) {
        use game_engine::common::message_stream::GameMessageArgumentType;
        match arg {
            GameMessageArgumentType::Integer(v) => target.append_integer_argument(*v),
            GameMessageArgumentType::Real(v) => target.append_real_argument(*v),
            GameMessageArgumentType::Boolean(v) => target.append_boolean_argument(*v),
            GameMessageArgumentType::ObjectID(v) => target.append_object_id_argument(*v),
            GameMessageArgumentType::DrawableID(v) => target.append_drawable_id_argument(*v),
            GameMessageArgumentType::TeamID(v) => target.append_team_id_argument(*v),
            GameMessageArgumentType::SquadID(v) => target.append_team_id_argument(*v),
            GameMessageArgumentType::Location(v) => target.append_location_argument(v.clone()),
            GameMessageArgumentType::Pixel(v) => target.append_pixel_argument(v.clone()),
            GameMessageArgumentType::PixelRegion(v) => {
                target.append_pixel_region_argument(v.clone())
            }
            GameMessageArgumentType::Timestamp(v) => target.append_timestamp_argument(*v),
            GameMessageArgumentType::WideChar(v) => target.append_wide_char_argument(*v),
            GameMessageArgumentType::String(v) => target.append_string_argument(v.clone()),
        }
    }

    pub(super) fn append_common_message_to_stream(
        stream: &mut game_engine::common::message_stream::MessageStream,
        message: &game_engine::common::message_stream::GameMessage,
    ) {
        let forwarded = stream.append_message(message.get_type().clone());
        for arg in message.get_arguments() {
            Self::append_message_argument_to_common_stream(forwarded, &arg.data);
        }
    }

    pub(super) fn legacy_game_mode_from_new_game_code(mode: i32) -> Option<GameMode> {
        match mode {
            0 => Some(GameMode::SinglePlayer), // GAME_SINGLE_PLAYER
            1 => Some(GameMode::Multiplayer),  // GAME_LAN
            2 => Some(GameMode::Skirmish),     // GAME_SKIRMISH
            3 => Some(GameMode::Replay),       // GAME_REPLAY
            4 => Some(GameMode::Shell),        // GAME_SHELL
            _ => None,
        }
    }

    pub(super) fn legacy_game_difficulty_from_new_game_code(difficulty: i32) -> GameDifficulty {
        match difficulty {
            0 => GameDifficulty::Easy,
            1 => GameDifficulty::Medium,
            2 => GameDifficulty::Hard,
            _ => GameDifficulty::Medium,
        }
    }

    pub(super) fn startup_new_game_dispatch_from_message(
        message: &game_engine::common::message_stream::GameMessage,
    ) -> Option<StartupNewGameDispatch> {
        use game_engine::common::message_stream::GameMessageArgumentType;

        if !matches!(
            message.get_type(),
            game_engine::common::message_stream::GameMessageType::NewGame
        ) {
            return None;
        }

        let mode_code = match message.get_argument(0) {
            Some(GameMessageArgumentType::Integer(mode_code)) => *mode_code,
            _ => return None,
        };
        let game_mode = Self::legacy_game_mode_from_new_game_code(mode_code)?;

        let difficulty_code = match message.get_argument(1) {
            Some(GameMessageArgumentType::Integer(value)) => *value,
            _ => 1,
        };
        let difficulty = Self::legacy_game_difficulty_from_new_game_code(difficulty_code);

        let rank_points = match message.get_argument(2) {
            Some(GameMessageArgumentType::Integer(value)) => *value,
            _ => 0,
        };

        let max_fps = match message.get_argument(3) {
            Some(GameMessageArgumentType::Integer(value)) => {
                let resolved = if (1..=1000).contains(value) {
                    *value
                } else {
                    game_engine::common::global_data::read()
                        .writable
                        .frames_per_second_limit
                };
                Some(resolved)
            }
            _ => None,
        };

        Some(StartupNewGameDispatch {
            game_mode_code: mode_code,
            game_mode,
            difficulty_code,
            difficulty,
            rank_points,
            max_fps,
        })
    }

    pub(super) fn startup_new_game_dispatch_from_messages(
        messages: &[game_engine::common::message_stream::GameMessage],
    ) -> Option<StartupNewGameDispatch> {
        let mut resolved = None;
        for message in messages {
            if let Some(dispatch) = Self::startup_new_game_dispatch_from_message(message) {
                resolved = Some(dispatch);
            }
        }
        resolved
    }

    pub(super) fn take_startup_messages_from_stream(
        owner_gen: u64,
    ) -> Result<Vec<game_engine::common::message_stream::GameMessage>, String> {
        if !startup_worker_owns(owner_gen) {
            return Err("startup worker abandoned; host owns session (skip stream clear)".into());
        }
        let stream = game_engine::common::message_stream::get_message_stream();
        let mut stream_guard = stream
            .write()
            .map_err(|_| "failed to acquire startup message stream lock".to_string())?;
        if !startup_worker_owns(owner_gen) {
            return Err("startup worker abandoned; host owns session (skip stream clear)".into());
        }
        let messages = stream_guard
            .get_messages()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        stream_guard.clear_messages();
        Ok(messages)
    }

    pub(super) fn apply_startup_new_game_dispatch(
        dispatch: StartupNewGameDispatch,
    ) -> Option<String> {
        let mut prepared_map_name = None;
        let mut global = game_engine::common::global_data::write();

        gamelogic::helpers::TheScriptEngine::set_global_difficulty(dispatch.difficulty_code);
        crate::game_logic::host_faction_skirmish_residual::set_live_host_session_difficulty(
            dispatch.difficulty_code,
        );
        gamelogic::helpers::TheGameLogic::set_rank_points_to_add_at_game_start(
            dispatch.rank_points,
        );

        if let Some(max_fps) = dispatch.max_fps {
            global.writable.use_fps_limit = true;
            global.writable.frames_per_second_limit = max_fps;
        }

        if !global.pending_file.trim().is_empty() {
            let pending_file = global.pending_file.clone();
            global.writable.map_name = pending_file.clone();
            global.pending_file.clear();
            prepared_map_name = Some(pending_file);
        }

        prepared_map_name
    }

    pub(super) fn resolve_startup_mode_from_dispatch(
        start_in_menu: &mut bool,
        map_to_load: &mut Option<String>,
        startup_new_game: Option<StartupNewGameDispatch>,
        replay_startup_requested: bool,
    ) -> GameMode {
        if *start_in_menu {
            return GameMode::Shell;
        }

        if let Some(dispatch) = startup_new_game {
            debug!(
                "Startup NewGame dispatch: mode_code={} difficulty_code={} rank_points={} max_fps={:?}",
                dispatch.game_mode_code,
                dispatch.difficulty_code,
                dispatch.rank_points,
                dispatch.max_fps
            );
            let prepared_map = Self::apply_startup_new_game_dispatch(dispatch);
            if map_to_load.is_none() {
                *map_to_load = prepared_map;
            }
            return dispatch.game_mode;
        }

        warn!(
            "Startup map/replay launch requested without a queued NewGame message; falling back to menu startup"
        );
        *start_in_menu = true;
        *map_to_load = None;
        game_engine::common::global_data::write()
            .pending_file
            .clear();

        if replay_startup_requested {
            warn!("Startup replay launch is deferred because recorder did not queue NewGame");
        }

        GameMode::Shell
    }

    /// Decode MSG_NEW_GAME from the common stream without discarding it.
    /// Host start still consumes the payload; `propagate_messages` can deliver
    /// the same message to crate GameLogic (C++ `logicMessageDispatcher`).
    pub(super) fn take_pending_new_game_start_request(&self) -> Option<HostStartRequest> {
        let dispatch = Self::peek_new_game_dispatch_from_common_stream()?;
        self.build_start_request_from_pending_globals(Some(dispatch))
    }

    /// Inspect every `NewGame` on the common stream without removing it.
    /// Prefers the last non-Shell NewGame so leftover GAME_SHELL from
    /// showShellMap cannot hide a later GAME_SKIRMISH Start.
    pub(super) fn peek_new_game_dispatch_from_common_stream() -> Option<StartupNewGameDispatch> {
        let stream = game_engine::common::message_stream::get_message_stream();
        let stream_guard = match stream.read() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        let mut last_any = None;
        let mut last_match = None;
        let mut n = 0u32;
        for message in stream_guard.get_messages().iter() {
            if let Some(d) = Self::startup_new_game_dispatch_from_message(message) {
                n += 1;
                last_any = Some(d);
                if !matches!(d.game_mode, GameMode::Shell) {
                    last_match = Some(d);
                }
            }
        }
        let dispatch = last_match.or(last_any);
        if n > 0 {
            log::info!(
                "peek NewGame count={n} last_mode={:?}",
                dispatch.as_ref().map(|d| d.game_mode)
            );
        }
        dispatch
    }

    /// Remove every `NewGame` message from the common stream, keeping others.
    /// Returns the last NewGame dispatch (C++ prefers the latest enqueue).
    pub(super) fn take_new_game_dispatch_from_common_stream() -> Option<StartupNewGameDispatch> {
        let stream = game_engine::common::message_stream::get_message_stream();
        let mut stream_guard = match stream.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };

        let messages: Vec<_> = stream_guard.get_messages().iter().cloned().collect();
        if messages.is_empty() {
            return None;
        }

        let mut dispatch = None;
        let mut kept = Vec::with_capacity(messages.len());
        for message in messages {
            if let Some(d) = Self::startup_new_game_dispatch_from_message(&message) {
                // Prefer the last NewGame (matches startup_new_game_dispatch_from_messages).
                dispatch = Some(d);
            } else {
                kept.push(message);
            }
        }

        let Some(dispatch) = dispatch else {
            return None;
        };

        // Drop leftover NewGame after host start if pump did not consume it.
        stream_guard.clear_messages();
        for message in &kept {
            Self::append_common_message_to_stream(&mut stream_guard, message);
        }
        Some(dispatch)
    }

    /// Drop leftover GAME_SHELL NewGame only. Keep GAME_SKIRMISH / campaign.
    pub(super) fn take_shell_new_game_messages_from_common_stream() {
        let stream = game_engine::common::message_stream::get_message_stream();
        let mut stream_guard = match stream.write() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        let messages: Vec<_> = stream_guard.get_messages().iter().cloned().collect();
        if messages.is_empty() {
            return;
        }
        let mut kept = Vec::with_capacity(messages.len());
        for message in messages {
            match Self::startup_new_game_dispatch_from_message(&message) {
                Some(d) if matches!(d.game_mode, GameMode::Shell) => {}
                _ => kept.push(message),
            }
        }
        stream_guard.clear_messages();
        for message in &kept {
            Self::append_common_message_to_stream(&mut stream_guard, message);
        }
    }

    /// Consume MSG_CLEAR_GAME_DATA from TheMessageStream.
    ///
    /// C++ ScriptEngine.cpp:5514-5518 appends the message when the end-game
    /// timer expires. QuitMenu Exit uses the same message. Main is the only
    /// offline InGame consumer, so scripted VICTORY/DEFEAT and QuitMenu both
    /// end the live match here.
    pub(super) fn take_clear_game_data_from_common_stream() -> bool {
        let stream = game_engine::common::message_stream::get_message_stream();
        let mut stream = stream.write().unwrap_or_else(|e| e.into_inner());
        let messages: Vec<_> = stream.get_messages().iter().cloned().collect();

        let mut found = false;
        let mut kept = Vec::with_capacity(messages.len());
        for message in messages {
            if matches!(
                message.get_type(),
                game_engine::common::message_stream::GameMessageType::ClearGameData
            ) {
                found = true;
            } else {
                kept.push(message);
            }
        }

        if !found {
            return false;
        }

        stream.clear_messages();
        for message in &kept {
            Self::append_common_message_to_stream(&mut stream, message);
        }
        true
    }

    /// C++ GameLogicDispatch leftover MSG_* from TheMessageStream:
    /// SelfDestruct, EnableRetaliationMode, SwitchWeapons, beacon place/remove/text.
    pub(super) fn host_consume_leftover_dispatch_messages(&mut self) {
        if !matches!(self.current_state, GameState::InGame) {
            return;
        }
        let player_id = self.local_player_id_for_ui();
        let selected = self.ui_selected_ids(player_id);
        let commands =
            crate::command_executor::take_leftover_dispatch_commands_from_common_stream(&selected);
        for command in commands {
            self.host_queue_and_process_command_silent(command);
        }
    }

    /// C++ `GameLogic::clearGameData` (`GameLogicDispatch.cpp:244`):
    /// `(!isInShellGame() || !isInGame()) && showScoreScreen`.
    pub(super) fn clear_game_data_should_push_score_screen(
        in_shell_game: bool,
        in_game: bool,
        show_score_screen: bool,
    ) -> bool {
        (!in_shell_game || !in_game) && show_score_screen
    }

    /// C++ `TheShell->push("Menus/ScoreScreen.wnd"); TheShell->showShell(FALSE)`
    /// (`GameLogicDispatch.cpp:248-249`).
    pub(super) fn host_push_score_screen_like_cpp(&mut self) {
        #[cfg(feature = "game_client")]
        {
            let result = game_client::gui::with_shell_mut(|shell| {
                if let Err(e) = game_client::system::SubsystemInterface::init(shell) {
                    return Err(e);
                }
                shell.push("Menus/ScoreScreen.wnd", false)?;
                shell.show_shell(false)?;
                Ok(shell.get_screen_count())
            });
            match result {
                Some(Ok(_)) => {
                    self.shell_menu_active = true;
                }
                Some(Err(e)) => {
                    warn!("clearGameData failed to push ScoreScreen.wnd: {e:?}");
                }
                None => {
                    game_client::gui::queue_shell_operation(|shell| {
                        if let Err(e) = shell.push("Menus/ScoreScreen.wnd", false) {
                            warn!("deferred ScoreScreen push failed: {e:?}");
                            return;
                        }
                        if let Err(e) = shell.show_shell(false) {
                            warn!("deferred showShell(FALSE) failed: {e:?}");
                        }
                    });
                    self.shell_menu_active = true;
                }
            }
        }
        self.set_runtime_host_ui_screen_override(Some("ScoreScreen"));
    }

    /// End the live offline match when ScriptEngine or QuitMenu posts
    /// MSG_CLEAR_GAME_DATA. C++ `GameLogic::clearGameData`
    /// (`GameLogicDispatch.cpp:223-253`) pushes ScoreScreen then resets.
    pub(super) fn host_consume_clear_game_data(&mut self) -> bool {
        if !matches!(
            self.current_state,
            GameState::InGame | GameState::Victory | GameState::Defeat
        ) {
            return false;
        }
        // Network/replay stay out of this consumer (GameNetwork deferred).
        if matches!(
            self.host_match_game_mode,
            Some(
                crate::game_logic::GameMode::Multiplayer
                    | crate::game_logic::GameMode::Internet
                    | crate::game_logic::GameMode::Lan
                    | crate::game_logic::GameMode::Replay
            )
        ) {
            return false;
        }
        if !Self::take_clear_game_data_from_common_stream() {
            return false;
        }
        self.host_set_paused(false);
        if let Ok(mut guard) = gamelogic::scripting::engine::get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                // C++ GameLogicDispatch.cpp:241 TheScriptActions->closeWindows(FALSE).
                engine.close_windows(false);
            }
        }

        let in_shell_game = matches!(
            self.host_match_game_mode,
            Some(crate::game_logic::GameMode::Shell)
        );
        let in_game = matches!(
            self.current_state,
            GameState::InGame | GameState::Victory | GameState::Defeat
        );
        // MSG_CLEAR_GAME_DATA uses the default showScoreScreen=TRUE
        // (GameLogicDispatch.cpp:439).
        let show_score =
            Self::clear_game_data_should_push_score_screen(in_shell_game, in_game, true);

        // C++ pushes ScoreScreen then TheGameEngine->reset(). Rust Shell::reset
        // (if it runs via resetAll) pops the stack, so reset first then push
        // so the player still sees ScoreScreen. CampaignManager state survives.
        // Snapshot the live end frame first: leftover get_frame and Eva reset to 0.
        #[cfg(feature = "game_client")]
        {
            let frame = self.presentation_or_boot_logic_frame();
            let frame = if frame != 0 {
                frame
            } else {
                game_client::eva::eva_logic_frame()
            };
            game_client::gui::callbacks::publish_leftover_score_end_frame(frame);
        }
        self.reset_match_state();
        if show_score {
            self.host_push_score_screen_like_cpp();
            self.transition_to_state(GameState::Menu);
            // show_shell_menu with a non-empty stack reveals ScoreScreen
            // instead of pushing MainMenu.wnd.
            self.set_runtime_host_ui_screen_override(Some("ScoreScreen"));
        } else {
            self.transition_to_state(GameState::Menu);
        }
        true
    }

    /// C++ `restartMissionMenu` (QuitMenu.cpp:175-226): re-apply MSG_NEW_GAME
    /// mode/difficulty/rank and keep the last Challenge PlayerTemplate.
    pub(super) fn host_restart_mission_from_dispatch(&mut self, dispatch: StartupNewGameDispatch) {
        update_last_new_game_dispatch(dispatch);
        let prepared_map = Self::apply_startup_new_game_dispatch(dispatch);
        let identity = last_new_game_identity();
        let map = prepared_map
            .filter(|name| !name.trim().is_empty())
            .or_else(|| identity.as_ref().map(|id| id.map.clone()))
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| self.presentation_or_boot_map_name());
        let faction = identity
            .as_ref()
            .map(|id| id.faction.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| {
                self.ui_local_player_team_name()
                    .unwrap_or_else(|| "USA".to_string())
            });
        let player_template = identity.and_then(|id| id.player_template);
        let request = match player_template {
            Some(player_template) => HostStartRequest::with_player_template(
                dispatch.game_mode,
                faction,
                map,
                None,
                player_template,
            ),
            None => {
                HostStartRequest::without_player_template(dispatch.game_mode, faction, map, None)
            }
        };
        record_last_new_game_identity(LastNewGameIdentity {
            dispatch,
            player_template: request.player_template.clone(),
            faction: request.faction.clone(),
            map: request.map.clone(),
        });
        self.start_game_from_ui(request);
    }

    /// Resolve map/faction/skirmish config after a NewGame dispatch (or helper flag).
    pub(super) fn build_start_request_from_pending_globals(
        &self,
        dispatch: Option<StartupNewGameDispatch>,
    ) -> Option<HostStartRequest> {
        let dispatch = dispatch.unwrap_or(StartupNewGameDispatch {
            game_mode_code: 2, // GAME_SKIRMISH default when only the helper flag is set
            game_mode: GameMode::Skirmish,
            difficulty_code: 1,
            difficulty: GameDifficulty::Medium,
            rank_points: 0,
            max_fps: None,
        });

        // C++ has one GameInfo/GlobalData path for a campaign launch. Main's
        // host consumes the typed shell descriptor only when it exactly
        // corresponds to the NewGame payload being dispatched.  The bridge
        // drops stale/mismatched descriptors, so ordinary commands and a
        // later Skirmish launch cannot inherit a selected campaign faction.
        #[cfg(feature = "game_client")]
        let campaign_launch = match game_client::gui::campaign_launch_host_bridge::take_host_campaign_launch_for_new_game(
            dispatch.game_mode_code,
            dispatch.difficulty_code,
            dispatch.rank_points,
            dispatch.max_fps,
        ) {
            game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDelivery::None => None,
            game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDelivery::Matched(descriptor) => Some(descriptor),
            game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDelivery::Mismatched => {
                warn!("Rejecting MSG_NEW_GAME whose payload does not match its pending campaign launch descriptor");
                Self::clear_pending_campaign_start_map();
                return None;
            }
        };
        #[cfg(feature = "game_client")]
        let campaign_launch_overrides = match Self::campaign_launch_start_overrides(
            dispatch.game_mode,
            campaign_launch.as_ref(),
        ) {
            Ok(overrides) => overrides,
            Err(reason) => {
                // A Challenge descriptor carries a selected General slot,
                // not an arbitrary team label.  Starting from a stale HUD
                // faction here would silently launch the wrong General.
                warn!(
                    "Rejecting Challenge MSG_NEW_GAME without a validated selected General: {reason}"
                );
                Self::clear_pending_campaign_start_map();
                return None;
            }
        };
        #[cfg(not(feature = "game_client"))]
        let campaign_launch_overrides = CampaignLaunchStartOverrides::default();

        let CampaignLaunchStartOverrides {
            map: campaign_launch_map,
            faction: campaign_launch_faction,
            player_template,
        } = campaign_launch_overrides;

        // Do not consume pending globals until a typed Challenge selection is
        // known to be valid.  That keeps a rejected/stale descriptor from
        // mutating the next shell launch's map state.
        let prepared_map = Self::apply_startup_new_game_dispatch(dispatch);

        let mode = dispatch.game_mode;
        let map = campaign_launch_map
            .or(prepared_map)
            .filter(|m| !m.trim().is_empty())
            .or_else(|| {
                let g = game_engine::common::global_data::read();
                let m = g.writable.map_name.trim();
                if !m.is_empty() {
                    Some(m.to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| DEFAULT_SKIRMISH_MAP.to_string());

        let skirmish = if matches!(mode, GameMode::Skirmish) {
            #[cfg(feature = "game_client")]
            {
                crate::skirmish_config::config_from_client_skirmish_setup(Some(map.as_str()))
            }
            #[cfg(not(feature = "game_client"))]
            {
                None
            }
        } else {
            None
        };

        let faction = campaign_launch_faction
            .or_else(|| {
                skirmish
                    .as_ref()
                    .map(crate::skirmish_config::local_faction_from_config)
            })
            .or_else(|| self.ui_local_player_team_name())
            .unwrap_or_else(|| "USA".to_string());

        let request = match player_template {
            Some(player_template) => HostStartRequest::with_player_template(
                mode,
                faction,
                map,
                skirmish,
                player_template,
            ),
            None => HostStartRequest::without_player_template(mode, faction, map, skirmish),
        };
        record_last_new_game_identity(LastNewGameIdentity {
            dispatch,
            player_template: request.player_template.clone(),
            faction: request.faction.clone(),
            map: request.map.clone(),
        });
        Some(request)
    }

    /// Extract the Main-owned start fields from a matching typed campaign
    /// descriptor.  Keep the exact PlayerTemplate identity alongside its
    /// compatibility base Team so the session can construct the same General
    /// C++ puts into `SidesList::playerFaction`.
    #[cfg(feature = "game_client")]
    pub(super) fn campaign_launch_start_overrides(
        mode: GameMode,
        descriptor: Option<
            &game_client::gui::campaign_launch_host_bridge::HostCampaignLaunchDescriptor,
        >,
    ) -> std::result::Result<CampaignLaunchStartOverrides, &'static str> {
        let Some(descriptor) = descriptor.filter(|_| matches!(mode, GameMode::SinglePlayer)) else {
            return Ok(CampaignLaunchStartOverrides::default());
        };

        let map = (!descriptor.map_name.trim().is_empty()).then(|| descriptor.map_name.clone());

        // Challenge C++ selection is an indexed PlayerTemplate.  Pairing the
        // index with its exact selected name protects the host from a stale
        // selection after the shell reorders or replaces its template list.
        // Never fall through to the current HUD/default faction for this
        // branch: that would create a plausible-looking but wrong match.
        if descriptor.is_challenge {
            let player_template = descriptor
                .player_template_name
                .as_deref()
                .zip(descriptor.player_template_index)
                .and_then(|(name, index)| {
                    PlayerTemplateIdentity::from_exact_indexed_name(name, index)
                })
                .ok_or("the selected Challenge PlayerTemplate slot is missing or stale")?;
            let faction = Self::base_faction_from_player_template_identity(&player_template)
                .ok_or("the selected Challenge PlayerTemplate has no supported Main base side")?;
            let map = map.ok_or("the selected Challenge map is empty")?;
            return Ok(CampaignLaunchStartOverrides {
                map: Some(map),
                faction: Some(faction),
                player_template: Some(player_template),
            });
        }

        // A normal campaign has no GameSlot index.  Preserve an exact
        // PlayerFaction only when it resolves in the Common store; plain base
        // labels retain the existing no-template map/default behavior.
        let player_template = descriptor
            .player_template_name
            .as_deref()
            .and_then(PlayerTemplateIdentity::from_exact_name)
            .or_else(|| {
                PlayerTemplateIdentity::from_exact_name(descriptor.campaign_player_faction.as_str())
            });
        let faction = player_template
            .as_ref()
            .and_then(Self::base_faction_from_player_template_identity)
            .or_else(|| {
                Self::base_faction_from_campaign_faction(
                    descriptor.campaign_player_faction.as_str(),
                )
            })
            .or_else(|| {
                Self::base_faction_from_campaign_faction(descriptor.campaign_name.as_str())
            });
        Ok(CampaignLaunchStartOverrides {
            map,
            faction,
            player_template,
        })
    }

    /// Resolve a Challenge PlayerTemplate only when the index retained by the
    /// C++ shell still names precisely the same template.  Name-only lookup is
    /// insufficient because challenge selection is position-sensitive.
    #[cfg(feature = "game_client")]
    pub(super) fn base_faction_from_indexed_player_template(
        template_name: &str,
        template_index: i32,
    ) -> Option<String> {
        PlayerTemplateIdentity::from_exact_indexed_name(template_name, template_index)
            .as_ref()
            .and_then(Self::base_faction_from_player_template_identity)
    }

    /// Drop the map writes paired with a rejected typed campaign descriptor.
    /// Challenge publishers intentionally mirror `pending_file` into both
    /// GlobalData residences before queuing NewGame. Leaving either copy after
    /// an invalid selected-General slot would let a later unrelated launch
    /// inherit the rejected Challenge map.
    #[cfg(feature = "game_client")]
    fn clear_pending_campaign_start_map() {
        game_engine::common::global_data::write()
            .pending_file
            .clear();
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().pending_file.clear();
        }
    }

    /// Convert a C++ PlayerTemplate `Side`/`BaseSide` identity to Main's
    /// currently-supported base Team string.  This is intentionally exact and
    /// fail-closed: a General template is not guessed from a display label.
    pub(super) fn base_faction_from_player_template(template_name: &str) -> Option<String> {
        PlayerTemplateIdentity::from_exact_name(template_name)
            .as_ref()
            .and_then(Self::base_faction_from_player_template_identity)
    }

    fn base_faction_from_player_template_identity(
        player_template: &PlayerTemplateIdentity,
    ) -> Option<String> {
        player_template
            .base_team()
            .map(|team| team.get_name().to_string())
    }

    /// C++ Campaign `PlayerFaction` and PlayerTemplate base-side values use
    /// both plain faction names and `Faction*` identifiers.
    pub(super) fn base_faction_from_campaign_faction(value: &str) -> Option<String> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "usa" | "us" | "america" | "factionamerica" => Some("USA".to_string()),
            "china" | "factionchina" => Some("China".to_string()),
            "gla" | "factiongla" => Some("GLA".to_string()),
            _ => None,
        }
    }

    pub(super) const MENU_CAUSTIC_WARMUP_DELAY_FRAMES: u64 = 120;
    pub(super) const CAUSTIC_WARMUP_RETRY_INTERVAL: Duration = Duration::from_secs(10);

    pub(super) fn runtime_host_enabled(&self) -> bool {
        self.runtime_host_active
    }

    /// Honest OS-window residual for status.txt.
    ///
    /// True only when this is not a headless host and winit reports visible
    /// (`Some(true)`), or the platform cannot query visibility (`None` →
    /// `unwrap_or(!headless)`). Headless stays false even if the hidden
    /// window later reports `Some(true)`.
    pub(super) fn runtime_host_window_visible(&self) -> bool {
        crate::executable_smoke::ExecutableSmokeResult::window_visible_from_winit_query(
            self.runtime_host_headless,
            self.window.is_visible(),
        )
    }

    /// Physical OS window outer frame for status.txt clicker aim.
    /// Headless always reports zeros — there is no OS window to click.
    /// Coordinates are **points** (CGEvent / Quartz), not retina backing pixels.
    pub(super) fn runtime_host_window_outer_rect(&self) -> (i32, i32, u32, u32) {
        if self.runtime_host_headless {
            return (0, 0, 0, 0);
        }
        let pos = self.window.outer_position().unwrap_or_default();
        let size = self.window.outer_size();
        let scale = self.window.scale_factor().max(0.0001);
        let x = ((pos.x as f64) / scale).round() as i32;
        let y = ((pos.y as f64) / scale).round() as i32;
        let w = ((size.width as f64) / scale).round().max(0.0) as u32;
        let h = ((size.height as f64) / scale).round().max(0.0) as u32;
        (x, y, w, h)
    }

    /// Hittable menu gadget centers (`Name@x,y` OS screen **points**).
    /// Headless publishes none. Geometry-only / hidden / disabled stay omitted.
    /// Client/WND center is offset by `inner_position` so an OS clicker can aim.
    pub(super) fn runtime_host_hittable_gadget_hits(&self) -> Vec<String> {
        if self.runtime_host_headless {
            return Vec::new();
        }
        let origin = self.window.inner_position().unwrap_or_default();
        let scale = self.window.scale_factor().max(0.0001);
        let ox = ((origin.x as f64) / scale).round() as i32;
        let oy = ((origin.y as f64) / scale).round() as i32;
        let mut hits = Vec::new();
        for name in super::runtime::STATUS_GADGET_HIT_NAMES {
            if let Some((x, y)) = self.named_gadget_center_if_hittable(name) {
                // WND centers are already logical/client points (same space as
                // inner_position after /scale). Do not divide again.
                let sx = ox + x;
                let sy = oy + y;
                hits.push(format!("{name}@{sx},{sy}"));
            }
        }
        hits
    }

    pub(super) fn set_runtime_ui_state_projection(&mut self, state: UISystemState) {
        let projected = match state {
            UISystemState::MainMenu => "MainMenu",
            UISystemState::FactionSelection => "FactionSelection",
            UISystemState::InGame => "GameHUD",
            UISystemState::PauseMenu => "PauseMenu",
            UISystemState::Victory => "Victory",
            UISystemState::Loading => "Loading",
        };
        self.runtime_host_base_ui_screen = Some(projected.to_string());
    }

    pub(super) fn set_runtime_host_ui_screen_override(&mut self, screen: Option<&str>) {
        self.runtime_host_ui_screen_override = screen
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .map(str::to_string);
    }

    pub(super) fn take_runtime_host_pending_capture(&mut self) -> bool {
        let pending = self.runtime_host_pending_capture;
        self.runtime_host_pending_capture = false;
        pending
    }

    pub(super) fn runtime_host_status_snapshot(&mut self) -> RuntimeHostSnapshot {
        // Wave 556: prefer presentation victory residual when installed (no live
        // re-evaluate dual-read). Boot residual only without freeze.
        let (match_over, victory_label) = self.presentation_or_boot_match_over_label();

        // Wave 546/554: presentation freeze owns host status map residual when
        // installed (even if empty — no host dual-read mid-frame). Empty → "-".
        let map_name = {
            let m = self.presentation_or_boot_map_name();
            let t = m.trim();
            if t.is_empty() {
                "-".to_string()
            } else {
                t.to_string()
            }
        };

        let ui_screen = self
            .runtime_host_ui_screen_override
            .as_ref()
            .or(self.runtime_host_base_ui_screen.as_ref())
            .map(|screen| format!("Some({screen})"))
            .unwrap_or_else(|| format!("{:?}", self.ui_manager.current_screen()));
        if ui_screen.to_ascii_lowercase().contains("skirmish") {
            self.runtime_host_saw_skirmish_menu = true;
        }

        let startup_progress = if matches!(self.current_state, GameState::Loading | GameState::Menu)
        {
            self.startup_last_reported_progress.clamp(0.0, 1.0)
        } else {
            1.0
        };

        // Object-roster stats are presentation-owned when freeze installed (no live
        // get_objects dual-read). Wave 547: selection count prefers engine selection
        // residual first (command authority), then presentation freeze fail-closed
        // (no empty-presentation fallthrough that re-reads a second residual mid-frame).
        // Boot residual without freeze: engine selection only.
        let (local_mobile_units, under_construction, selected_count, sample_unit_pos) =
            if let Some(frame) = self.last_presentation_frame.as_ref() {
                let team = frame.local_team();
                let selected = if !self.selected_objects.is_empty() {
                    self.selected_objects.len() as u32
                } else {
                    // Wave 547: presentation freeze owns selection count residual.
                    frame.count_selected_friendlies(team)
                };
                (
                    frame.count_mobile_friendlies(team),
                    frame.count_under_construction_friendlies(team),
                    selected,
                    frame
                        .first_friendly_sample_label(team)
                        .unwrap_or_else(|| "-".to_string()),
                )
            } else {
                (
                    0u32,
                    0u32,
                    self.selected_objects.len() as u32,
                    "-".to_string(),
                )
            };

        {
            let (d, k) = crate::game_logic::host_damage_log::cumulative_totals();
            self.match_damage_applied = d;
            self.match_kills = k;
        }
        // SLUIROSTER (documented diagnostic, env-gated GENERALS_SLUI_PROBE=1):
        // local-player roster line per status publish for the mid-match
        // save/load windowed-drive verification (positions/health/selection
        // compared pre-save vs post-load). Temp probe for the SaveLoadUiHunt;
        // removed after the drive. Never writes evidence keys.
        static SLUI_ROSTER_TICKS: std::sync::atomic::AtomicU32 =
            std::sync::atomic::AtomicU32::new(0);
        if std::env::var("GENERALS_SLUI_PROBE").as_deref() == Ok("1")
            && SLUI_ROSTER_TICKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < u32::MAX
        {
            if let Some(frame) = self.last_presentation_frame.as_ref() {
                let team = frame.local_team();
                let roster: Vec<String> = frame
                    .objects
                    .iter()
                    .filter(|o| o.team == team && !o.destroyed && !o.sold)
                    .take(14)
                    .map(|o| {
                        format!(
                            "id={} {} @({:.1},{:.1},{:.1}) hp={:.0}/{:.0} sel={} dest={}",
                            o.id.0,
                            o.template_name,
                            o.position.x,
                            o.position.y,
                            o.position.z,
                            o.health_current,
                            o.health_max,
                            o.selected,
                            o.move_destination
                                .map(|d| format!("({:.1},{:.1})", d.x, d.y))
                                .unwrap_or_else(|| "-".to_string())
                        )
                    })
                    .collect();
                if !roster.is_empty() {
                    log::warn!(
                        "SLUI f={} lf={} st={:?} paused={} | {}",
                        self.frame_counter,
                        self.presentation_or_boot_logic_frame(),
                        self.current_state,
                        self.game_paused,
                        roster.join(" | ")
                    );
                }
            }
        }


        let (window_outer_x, window_outer_y, window_outer_w, window_outer_h) =
            self.runtime_host_window_outer_rect();
        let gadget_hits = self.runtime_host_hittable_gadget_hits();

        RuntimeHostSnapshot {
            state: format!("{:?}", self.current_state),
            ui_screen,
            skirmish_menu_ok: self.runtime_host_saw_skirmish_menu,
            paused: self.game_paused,
            fps: self.fps.max(0.0),
            startup_progress,
            startup_phase: self.startup_loading_phase.clone(),
            map: map_name,
            frame: self.frame_counter,
            logic_frame: self.presentation_or_boot_logic_frame(),
            logic_steps: self.presentation_or_boot_logic_steps(),
            under_construction,
            match_damage_applied: self.match_damage_applied,
            match_kills: self.match_kills,
            selected_count,
            local_mobile_units,
            last_gameplay_cmd: self.runtime_host_last_gameplay_cmd.clone(),
            match_over,
            victory_label,
            presentation_frame_ok: self.last_presentation_frame.is_some(),
            gameworld_presentation_entities: self.last_gameworld_presentation_entity_count as u32,
            gameworld_overlay_stamped: self
                .last_presentation_frame
                .as_ref()
                .map(|f| f.gameworld_overlay_stamped as u32)
                .unwrap_or(0),
            gameworld_appended: self
                .last_presentation_frame
                .as_ref()
                .map(|f| f.gameworld_appended as u32)
                .unwrap_or(0),
            gameworld_rebuilt: self
                .last_presentation_frame
                .as_ref()
                .map(|f| f.gameworld_rebuilt as u32)
                .unwrap_or(0),
            gameworld_primary_objects: self
                .last_presentation_frame
                .as_ref()
                .map(|f| f.gameworld_primary_objects)
                .unwrap_or(false),
            shell_screen_count: {
                #[cfg(feature = "game_client")]
                {
                    // Honest residual: report actual Shell stack depth only.
                    game_client::gui::with_shell_ref(|shell| shell.get_screen_count()).unwrap_or(0)
                        as u32
                }
                #[cfg(not(feature = "game_client"))]
                {
                    0u32
                }
            },
            shell_top_wnd: {
                #[cfg(feature = "game_client")]
                {
                    // Honest residual: report actual Shell stack top only.
                    // Do not invent MainMenu.wnd when the stack is empty.
                    game_client::gui::with_shell_ref(|shell| {
                        shell.top_filename().map(str::to_owned)
                    })
                    .flatten()
                    .unwrap_or_default()
                }
                #[cfg(not(feature = "game_client"))]
                {
                    String::new()
                }
            },
            shell_active: {
                #[cfg(feature = "game_client")]
                {
                    game_client::gui::with_shell_ref(|shell| shell.is_shell_active())
                        .unwrap_or(false)
                        || self.shell_menu_active
                }
                #[cfg(not(feature = "game_client"))]
                {
                    false
                }
            },
            presentation_live_fallback_reads: self
                .render_pipeline
                .last_presentation_live_fallback_reads()
                as u32,
            waypoint_mode: self.sticky_waypoint_mode,
            // Snapshot stays false; publish_status ORs a real promoted capture.
            live_frame_ok: false,
            window_visible: self.runtime_host_window_visible(),
            // Physical winit MouseInput **or** winit-equivalent inject that
            // re-enters handle_mouse_button_input after a real gadget hit /
            // RMB order. Not drive_os_wnd_* and not direct note_* from host cmds.
            wnd_widget_tree_nav: self.interactive_playability.wnd_menu_to_match_complete(),
            interactive_gameplay: self.interactive_playability.gameplay_complete(),
            physical_build_and_produce: self.interactive_playability.build_and_produce_complete(),
            physical_gather_resources: self.interactive_playability.gather_resources_complete(),
            physical_save_load_continue: self.interactive_playability.save_load_continue_complete(),
            pending_capture: self.runtime_host_pending_capture,
            render_alive_objects: self.render_pipeline.debug_last_alive_objects() as u32,
            render_fow_filtered: self.render_pipeline.debug_last_fow_filtered() as u32,
            render_item_count: self.render_pipeline.debug_render_item_count() as u32,
            render_model_missing: self.render_pipeline.debug_last_model_missing() as u32,
            render_frustum_culled: self.render_pipeline.debug_last_frustum_culled() as u32,
            camera_pos: format!(
                "{:.1},{:.1},{:.1}",
                self.camera_position.x, self.camera_position.y, self.camera_position.z
            ),
            camera_target: format!(
                "{:.1},{:.1},{:.1}",
                self.camera_target.x, self.camera_target.y, self.camera_target.z
            ),
            sample_unit_pos,
            window_outer_x,
            window_outer_y,
            window_outer_w,
            window_outer_h,
            gadget_hits,
        }
    }

    /// Unknown residual-audit host actions fail closed.
    /// Default production builds do not compile residual packs; callers pass `false`.
    #[inline]
    pub(super) fn host_unknown_action_fail_closed(&self, residual_pack_ok: bool) -> bool {
        residual_pack_ok
    }

    pub(super) fn parse_runtime_host_mode(mode: Option<&str>) -> GameMode {
        match mode
            .unwrap_or("skirmish")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "singleplayer" | "single_player" | "single" => GameMode::SinglePlayer,
            "skirmish" => GameMode::Skirmish,
            "multiplayer" | "multi" => GameMode::Multiplayer,
            "internet" | "online" => GameMode::Internet,
            "network" | "lan" => GameMode::Lan,
            "replay" => GameMode::Replay,
            _ => GameMode::Skirmish,
        }
    }
}
