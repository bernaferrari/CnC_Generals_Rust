//! The command dispatcher releases native field loans only at source lifecycle
//! operations, then resumes the same admitted list on its receiving Engine.
use super::dispatch::LastNewGameIdentity;
use super::*;

// Only metadata changed by the source preparation prefix. A rejected native
// roster/descriptor must not publish a prepared mode or consume its pending map.
struct SourcePreparationBefore {
    mode: GameMode,
    session_difficulty: Option<crate::ai::AIDifficulty>,
    script_difficulty: gamelogic::player::GameDifficulty,
    use_fps_limit: bool,
    fps: i32,
    pending_file: String,
    map_name: String,
    identity: Option<LastNewGameIdentity>,
    #[cfg(feature = "game_client")]
    background_image: Option<bool>,
}

#[cfg(feature = "game_client")]
fn create_source_background_layout(
    manager: &mut game_client::gui::window_manager::WindowManager,
    filename: &str,
) -> game_client::gui::game_window::WindowResult<
    std::rc::Rc<std::cell::RefCell<game_client::gui::window_manager::WindowLayout>>,
> {
    let (layout, _) = manager.create_layout_with_windows(filename)?;
    if layout.borrow().get_first_window().is_none() {
        manager.destroy_layout(&layout);
        return Err(game_client::gui::game_window::WindowError::InvalidWindow);
    }
    layout.borrow_mut().hide(false);
    layout.borrow_mut().bring_forward();
    Ok(layout)
}

enum NativeLogicOperation {
    Begin,
    Resume(
        crate::game_logic::NativeTickContinuation,
        crate::game_logic::NativeCommandCompletion,
    ),
}

impl CnCGameEngine {
    pub(super) fn receiving_recorded_start_request(
        &mut self,
        dispatch: StartupNewGameDispatch,
    ) -> Option<HostStartRequest> {
        let recorder = self.game_logic.replay_pending.recorder_owned();
        if !recorder.is_playback() {
            warn!("Rejecting native Replay start without its receiving admitted header/reader");
            return None;
        }
        let info = recorder.get_game_info().clone();
        let original_mode = recorder.get_game_mode();
        let config =
            match crate::skirmish_config::replay_admission::config_from_recorded_info(&info) {
                Ok(config) => config,
                Err(error) => {
                    warn!("Cannot admit recorded slots: {error}");
                    return None;
                }
            };
        if config.is_none() && original_mode != 0 {
            warn!(
                "Replay has no participant slots; authored-side-only replay admission remains unsupported (original mode {original_mode})"
            );
            return None;
        }
        if info.map.trim().is_empty() {
            warn!("Rejecting replay without recorded map");
            return None;
        }
        let faction = config
            .as_ref()
            .map(crate::skirmish_config::local_faction_from_config)
            .unwrap_or_else(|| "USA".into());
        let mut request =
            HostStartRequest::without_player_template(GameMode::Replay, faction, info.map, config);
        request.game_info_starting_cash = Some(info.starting_cash);
        request.difficulty_code = Some(dispatch.difficulty_code);
        request.recorder_dispatch = Some(dispatch);
        Some(request)
    }

    /// All UI/renderer/clock loans end before this method returns an owned action.
    fn borrow_native_command_phase(
        &mut self,
        dt: f32,
        budget: Option<usize>,
        shell: bool,
        operation: NativeLogicOperation,
        mut owned_observer: Option<&mut crate::gameworld_shadow::GameWorldShadow>,
    ) -> crate::game_logic::NativeTickProgress {
        if self.game_paused {
            assert!(matches!(operation, NativeLogicOperation::Begin));
            return crate::game_logic::NativeTickProgress::Complete(Default::default());
        }
        if self.game_logic.is_paused() {
            self.game_logic.set_paused(false);
        }
        let gather_eligible = self.host_physical_gather_evidence_eligible();
        let local_player_id = self.local_player_id_for_ui();
        let presentation_entity_count = &mut self.last_gameworld_presentation_entity_count;
        let tracked_carriers = &self.physical_gather_carrier_ids;
        let playability = &mut self.interactive_playability;
        #[cfg(feature = "game_client")]
        let cinematic_ui = self.game_client.in_game_ui_subsystem();
        #[cfg(feature = "game_client")]
        let (mut client_frame, snow, presence) =
            self.game_client.split_frame_snow_and_drawable_presence();
        #[cfg(feature = "game_client")]
        let cinematic_window = &self.window;
        #[cfg(feature = "game_client")]
        let mut cinematic_cursor = |visible| cinematic_window.set_cursor_visible(visible);
        #[cfg(feature = "game_client")]
        let cinematic_input = super::mouse::CinematicInputLoan {
            ui: cinematic_ui,
            local_player: self.current_player_id,
            modes: &mut self.look_at_modes,
            selected: &mut self.selected_objects,
            selected_receipt: &mut self.host_match_selected_ids,
            presentation: &mut self.last_presentation_frame,
            rotating: &mut self.is_mmb_rotating,
            rotate_anchor: &mut self.mmb_anchor,
            scrolling: &mut self.is_rmb_scrolling,
            scroll_anchor: &mut self.rmb_scroll_anchor,
            held: [
                &mut self.camera_rotate_left_held,
                &mut self.camera_rotate_right_held,
                &mut self.camera_zoom_in_held,
                &mut self.camera_zoom_out_held,
            ],
            waypoint: &mut self.sticky_waypoint_mode,
            dragging: &mut self.is_dragging,
            selection_start: &mut self.selection_start,
            selection_screen: &mut self.selection_start_screen,
            cursor_visible: &mut cinematic_cursor,
        };
        #[cfg(feature = "game_client")]
        let mut ui = super::control_bar_viewport::NativeScriptUi::new(
            snow,
            &mut self.control_bar,
            &mut self.control_bar_callbacks,
        )
        .with_cinematic_input(cinematic_input);
        #[cfg(feature = "game_client")]
        let mut view = super::native_view_effects::NativeRendererViewEffects::new(
            self.render_pipeline.borrow_native_installed_poses(),
            &mut self.graphics_system,
            presence,
            self.host_direct_visual_world_epoch,
            self.current_player_id,
        );
        #[cfg(feature = "game_client")]
        let effects = crate::game_logic::ScriptPresentationEffects {
            ui: Some(&mut ui),
            view: Some(&mut view),
        };
        #[cfg(not(feature = "game_client"))]
        let effects = crate::game_logic::ScriptPresentationEffects::default();
        let timing = if shell {
            None
        } else {
            self.last_frame_timing.as_ref()
        };
        let mut observe = |logic: &mut crate::game_logic::GameLogic| {
            if !shell {
                Self::host_drain_physical_gather_dropoffs(
                    &mut *logic,
                    gather_eligible,
                    local_player_id,
                    tracked_carriers,
                    playability,
                );
            }
            // Wave 682/925: post-logic host→GameWorld residual batch.
            // The callback borrows this driving GameLogic after each
            // completed step; all eager channels retain their order.
            if !shell {
                *presentation_entity_count = crate::gameworld_shadow::run_owned_host_boundary(
                    owned_observer.as_deref_mut(),
                    logic,
                );
            }
        };
        match operation {
            NativeLogicOperation::Begin => self.game_logic.begin_native_logic_tick(
                dt,
                timing,
                budget,
                effects,
                |frame| {
                    #[cfg(feature = "game_client")]
                    client_frame.set_frame(frame);
                    #[cfg(not(feature = "game_client"))]
                    let _ = frame;
                },
                &mut observe,
            ),
            NativeLogicOperation::Resume(cursor, completion) => {
                self.game_logic.resume_native_logic_tick(
                    cursor,
                    completion,
                    effects,
                    |frame| {
                        #[cfg(feature = "game_client")]
                        client_frame.set_frame(frame);
                        #[cfg(not(feature = "game_client"))]
                        let _ = frame;
                    },
                    &mut observe,
                )
            }
        }
    }

    fn capture_source_preparation(&self) -> SourcePreparationBefore {
        let global = game_engine::common::global_data::read();
        SourcePreparationBefore {
            mode: self.game_logic.game_mode(),
            session_difficulty: self.game_logic.session_difficulty,
            script_difficulty: self
                .game_logic
                .owned_script_engine
                .as_ref()
                .expect("receiving interpreter")
                .get_global_difficulty(),
            use_fps_limit: global.writable.use_fps_limit,
            fps: global.writable.frames_per_second_limit,
            pending_file: global.pending_file.clone(),
            map_name: global.writable.map_name.clone(),
            identity: self.last_new_game_identity(),
            #[cfg(feature = "game_client")]
            background_image: self
                .source_new_game_background
                .as_ref()
                .and_then(|layout| layout.borrow().get_first_window())
                .map(|window| {
                    window
                        .borrow()
                        .get_status()
                        .contains(game_client::gui::game_window::WindowStatus::IMAGE)
                }),
        }
    }

    fn reject_source_preparation(&mut self, before: SourcePreparationBefore) {
        self.last_new_game_identity = before.identity;
        self.game_logic.prepare_native_command_mode(before.mode);
        self.game_logic.session_difficulty = before.session_difficulty;
        self.game_logic
            .owned_script_engine
            .as_ref()
            .expect("receiving interpreter")
            .set_global_difficulty(before.script_difficulty);
        {
            let mut global = game_engine::common::global_data::write();
            global.writable.use_fps_limit = before.use_fps_limit;
            global.writable.frames_per_second_limit = before.fps;
            global.pending_file = before.pending_file;
            global.writable.map_name = before.map_name;
        }
        #[cfg(feature = "game_client")]
        match before.background_image {
            None => self.destroy_source_new_game_background(),
            Some(image) => {
                if let Some(window) = self
                    .source_new_game_background
                    .as_ref()
                    .and_then(|layout| layout.borrow().get_first_window())
                {
                    if image {
                        window
                            .borrow_mut()
                            .set_status(game_client::gui::game_window::WindowStatus::IMAGE);
                    } else {
                        window
                            .borrow_mut()
                            .clear_status(game_client::gui::game_window::WindowStatus::IMAGE);
                    }
                }
            }
        }
        warn!(
            "Rejected native NewGame admission; prior receiving mode/difficulty/pending map retained"
        );
    }

    fn prepare_receiving_source_new_game(
        &mut self,
        message: &game_engine::common::message_stream::GameMessage,
    ) -> bool {
        let Some(dispatch) = Self::startup_new_game_dispatch_from_message(message) else {
            return false;
        };
        let before = self.capture_source_preparation();
        // Dispatch.cpp407–419 applies optional FPS before prepareNewGame.
        Self::apply_application_new_game_fps(dispatch);
        // prepareNewGame: interpreter difficulty, background, mode, pending map,
        // rank, hideShell. startNewGame(FALSE) then parks actual map loading.
        self.game_logic
            .prepare_native_command_difficulty(dispatch.difficulty_code);
        #[cfg(feature = "game_client")]
        {
            if self.source_new_game_background.is_none() {
                let mut created = None;
                game_client::gui::with_window_manager(|manager| {
                    created = Some(create_source_background_layout(
                        manager,
                        "Menus/BlankWindow.wnd",
                    ));
                });
                match created {
                    Some(Ok(layout)) => self.source_new_game_background = Some(layout),
                    Some(Err(error)) => {
                        warn!("Cannot prepare native NewGame: Menus/BlankWindow.wnd: {error}");
                        self.reject_source_preparation(before);
                        return false;
                    }
                    None => {
                        warn!(
                            "Cannot prepare native NewGame: receiving WindowManager unavailable for Menus/BlankWindow.wnd"
                        );
                        self.reject_source_preparation(before);
                        return false;
                    }
                }
            }
            let root = self
                .source_new_game_background
                .as_ref()
                .and_then(|layout| layout.borrow().get_first_window());
            let Some(root) = root else {
                warn!(
                    "Cannot prepare native NewGame: required Menus/BlankWindow.wnd has no first window"
                );
                self.reject_source_preparation(before);
                return false;
            };
            root.borrow_mut()
                .clear_status(game_client::gui::game_window::WindowStatus::IMAGE);
        }
        self.game_logic
            .prepare_native_command_mode(dispatch.game_mode);
        let request = if dispatch.game_mode == GameMode::Replay {
            // Header/map/slots belong to this same live receiving Recorder.
            Self::take_application_new_game_map();
            self.receiving_recorded_start_request(dispatch)
        } else {
            self.build_start_request_from_pending_globals_at_source(Some(dispatch), true)
        };
        let Some(request) = request else {
            self.reject_source_preparation(before);
            return false;
        };
        if !self.host_start_game_from_command_phase(request) {
            self.reject_source_preparation(before);
            return false;
        }
        self.update_last_new_game_dispatch(dispatch);
        true
    }

    pub(super) fn destroy_source_new_game_background(&mut self) {
        #[cfg(feature = "game_client")]
        if let Some(layout) = self.source_new_game_background.take() {
            game_client::gui::with_window_manager(|manager| manager.destroy_layout(&layout));
        }
    }

    /// C++ processCommandList dispatches synchronously. The native reset has no
    /// live GameLogic/UI/renderer borrows; remaining admitted messages are owned
    /// by the private cursor, outside the subsystem reset list.
    pub(super) fn drive_native_command_phase(
        &mut self,
        dt: f32,
        budget: Option<usize>,
        shell: bool,
        mut owned_observer: Option<&mut crate::gameworld_shadow::GameWorldShadow>,
    ) -> (crate::game_logic::SimTimingSnapshot, bool) {
        use crate::game_logic::{NativeCommandAction, NativeCommandCompletion, NativeTickProgress};
        let mut progress = self.borrow_native_command_phase(
            dt,
            budget,
            shell,
            NativeLogicOperation::Begin,
            owned_observer.as_deref_mut(),
        );
        let mut interrupted = false;
        let mut last_clear_score = false;
        loop {
            match progress {
                NativeTickProgress::Complete(snapshot) => {
                    if !self.game_paused {
                        self.host_stamp_sim_timing_from_snapshot(snapshot);
                    }
                    if interrupted
                        && self.pending_match_start.is_none()
                        && self.game_logic.game_mode() == GameMode::None
                    {
                        self.transition_to_state(GameState::Menu);
                        if last_clear_score {
                            self.set_runtime_host_ui_screen_override(Some("ScoreScreen"));
                        }
                    }
                    return (snapshot, interrupted);
                }
                NativeTickProgress::CommandBoundary(mut cursor) => {
                    let completion = match cursor.take_action() {
                        NativeCommandAction::ClearGameData => {
                            last_clear_score = !matches!(
                                self.game_logic.game_mode(),
                                GameMode::None | GameMode::Shell
                            );
                            let reset_performed = self.clear_receiving_game_data(true);
                            if reset_performed {
                                interrupted = true;
                                // The normal match observer is temporarily moved out of self;
                                // clear this exact observer before any receiving post-step copy.
                                if let Some(observer) = owned_observer.as_deref_mut() {
                                    observer.reset_for_world_boundary();
                                }
                            }
                            NativeCommandCompletion::Cleared { reset_performed }
                        }
                        NativeCommandAction::PrepareNewGame(message) => {
                            let prepared = self.prepare_receiving_source_new_game(&message);
                            interrupted |= prepared;
                            NativeCommandCompletion::NewGamePrepared { prepared }
                        }
                    };
                    progress = self.borrow_native_command_phase(
                        dt,
                        budget,
                        shell,
                        NativeLogicOperation::Resume(cursor, completion),
                        owned_observer.as_deref_mut(),
                    );
                }
            }
        }
    }
}

#[cfg(all(test, feature = "game_client"))]
mod source_background_tests {
    use super::*;

    #[test]
    fn actual_source_background_creation_rejects_missing_and_empty_layouts() {
        let directory = tempfile::tempdir().unwrap();
        let mut receiving = game_client::gui::window_manager::WindowManager::new();
        let missing = directory.path().join("SourceMissingBlankWindow-501.wnd");
        assert!(
            create_source_background_layout(&mut receiving, missing.to_str().unwrap()).is_err()
        );
        assert_eq!(receiving.window_count(), 0);
        let empty = directory.path().join("SourceEmptyBlankWindow-501.wnd");
        std::fs::write(&empty, "FILE_VERSION = 2;\n").unwrap();
        assert!(matches!(
            create_source_background_layout(&mut receiving, empty.to_str().unwrap()),
            Err(game_client::gui::game_window::WindowError::InvalidParameter)
        ));
        assert_eq!(receiving.window_count(), 0);
        assert_eq!(receiving.root_window_count(), 0);
    }
}
