#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
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
                dispatch.game_mode_code, dispatch.difficulty_code, dispatch.rank_points, dispatch.max_fps
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

    /// Pull MSG_NEW_GAME out of the common message stream without discarding
    /// unrelated messages. Returns a fully resolved start_game_from_ui tuple.
    pub(super) fn take_pending_new_game_start_request(
        &self,
    ) -> Option<(
        GameMode,
        String,
        String,
        Option<crate::skirmish_config::SkirmishMatchConfig>,
    )> {
        let dispatch = Self::take_new_game_dispatch_from_common_stream()?;
        self.build_start_request_from_pending_globals(Some(dispatch))
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

        // Rebuild stream without NewGame messages so pump doesn't double-handle.
        stream_guard.clear_messages();
        for message in &kept {
            Self::append_common_message_to_stream(&mut stream_guard, message);
        }
        Some(dispatch)
    }

    /// Resolve map/faction/skirmish config after a NewGame dispatch (or helper flag).
    pub(super) fn build_start_request_from_pending_globals(
        &self,
        dispatch: Option<StartupNewGameDispatch>,
    ) -> Option<(
        GameMode,
        String,
        String,
        Option<crate::skirmish_config::SkirmishMatchConfig>,
    )> {
        let dispatch = dispatch.unwrap_or(StartupNewGameDispatch {
            game_mode_code: 2, // GAME_SKIRMISH default when only the helper flag is set
            game_mode: GameMode::Skirmish,
            difficulty_code: 1,
            difficulty: GameDifficulty::Medium,
            rank_points: 0,
            max_fps: None,
        });

        let prepared_map = Self::apply_startup_new_game_dispatch(dispatch);

        let mode = dispatch.game_mode;
        let map = prepared_map
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

        let faction = skirmish
            .as_ref()
            .map(crate::skirmish_config::local_faction_from_config)
            .unwrap_or_else(|| {
                self.ui_local_player_team_name()
                    .unwrap_or_else(|| "USA".to_string())
            });

        Some((mode, faction, map, skirmish))
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
                    game_client::gui::get_shell().get_screen_count() as u32
                }
                #[cfg(not(feature = "game_client"))]
                {
                    0u32
                }
            },
            shell_top_wnd: {
                #[cfg(feature = "game_client")]
                {
                    let mut shell = game_client::gui::get_shell();
                    // Honest residual: report actual Shell stack top only.
                    // Do not invent MainMenu.wnd when the stack is empty.
                    shell
                        .top()
                        .map(|layout| layout.get_filename().to_string())
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
                    game_client::gui::get_shell().is_shell_active() || self.shell_menu_active
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
