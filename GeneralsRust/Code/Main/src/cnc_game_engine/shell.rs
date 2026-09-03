#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
impl CnCGameEngine {
    pub(super) fn enter_shell_menu_from_runtime_host(
        &mut self,
        override_screen: Option<&'static str>,
    ) {
        self.set_runtime_host_ui_screen_override(override_screen);
        self.ui_manager.suspend_for_shell_overlay();
        // Wave 845: shell residual — presentation peels treat FOW shell bypass as true.
        self.host_match_in_shell = Some(true);
        if self.current_state != GameState::Menu {
            self.request_state_change(GameState::Menu);
        }
    }

    pub(super) fn enter_shell_screen_from_runtime_host(
        &mut self,
        override_screen: Option<&'static str>,
        layout_file: &'static str,
    ) {
        self.enter_shell_menu_from_runtime_host(override_screen);
        #[cfg(feature = "game_client")]
        {
            // GENERALS_RUNTIME_HOST_WND=0 keeps soft UI override without shell push.
            // Executable smoke defaults to WND=1 so ButtonStart residual can run.
            let push_wnd = std::env::var("GENERALS_RUNTIME_HOST_WND")
                .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
                .unwrap_or(true);
            if push_wnd {
                self.show_shell_menu();
                let push_result = game_client::gui::with_shell_mut(|shell| {
                    shell.push(layout_file, false)
                })
                .unwrap_or_else(|| {
                    game_client::gui::queue_shell_operation(move |shell| {
                        if let Err(error) = shell.push(layout_file, false) {
                            warn!(
                                "Runtime host deferred shell screen {layout_file} failed: {error:?}"
                            );
                        }
                    });
                    Ok(())
                });
                if let Err(err) = push_result {
                    warn!("Runtime host failed to push shell screen {layout_file}: {err:?}");
                }
            } else {
                log::debug!(
                    "Runtime host soft shell screen {override_screen:?} (shell/WND push disabled)"
                );
            }
        }
    }

    pub(super) fn enter_shell_options_from_runtime_host(&mut self) {
        self.enter_shell_menu_from_runtime_host(Some("Options"));
        #[cfg(feature = "game_client")]
        {
            self.show_shell_menu();
            let initialized = game_client::gui::with_shell_mut(|shell| {
                let Some(layout) = shell.get_options_layout(true) else {
                    return false;
                };
                if let Err(err) = layout.run_init(None) {
                    warn!("Runtime host failed to init shell options layout: {err:?}");
                    return false;
                }
                layout.hide(false);
                layout.bring_forward();
                true
            })
            .unwrap_or(false);
            if !initialized {
                warn!("Runtime host failed to create shell options layout");
            }
        }
    }

    pub(super) fn loading_visual_phase(elapsed_seconds: f32) -> (&'static str, f32) {
        if elapsed_seconds < 1.0 {
            ("Initializing engine", (elapsed_seconds / 1.0) * 0.15)
        } else if elapsed_seconds < 4.0 {
            (
                "Loading map data",
                0.15 + ((elapsed_seconds - 1.0) / 3.0) * 0.30,
            )
        } else if elapsed_seconds < 10.0 {
            (
                "Spawning world objects",
                0.45 + ((elapsed_seconds - 4.0) / 6.0) * 0.35,
            )
        } else {
            (
                "Finalizing startup",
                0.80 + ((elapsed_seconds - 10.0) / 6.0).clamp(0.0, 1.0) * 0.15,
            )
        }
    }

    pub(super) fn ui_window_manager_has_windows(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            game_client::gui::with_window_manager_ref(|wm| wm.window_count() > 0)
        }
        #[cfg(not(feature = "game_client"))]
        {
            false
        }
    }

    pub(super) fn gameplay_ui_active(&self) -> bool {
        #[cfg(feature = "game_client")]
        {
            game_client::helpers::TheInGameUI::get_input_enabled()
        }
        #[cfg(not(feature = "game_client"))]
        {
            false
        }
    }

    #[cfg(feature = "game_client")]
    pub(super) fn load_screen_game_mode(
        mode: GameMode,
    ) -> game_client::gui::load_screen::LoadScreenGameMode {
        match mode {
            GameMode::SinglePlayer => {
                game_client::gui::load_screen::LoadScreenGameMode::SinglePlayer
            }
            GameMode::Skirmish => game_client::gui::load_screen::LoadScreenGameMode::Skirmish,
            GameMode::Multiplayer => game_client::gui::load_screen::LoadScreenGameMode::Multiplayer,
            GameMode::Replay => game_client::gui::load_screen::LoadScreenGameMode::Replay,
            GameMode::Internet => game_client::gui::load_screen::LoadScreenGameMode::Internet,
            GameMode::Lan => game_client::gui::load_screen::LoadScreenGameMode::Lan,
            GameMode::Shell => game_client::gui::load_screen::LoadScreenGameMode::Shell,
            GameMode::None => game_client::gui::load_screen::LoadScreenGameMode::None,
        }
    }

    #[cfg(feature = "game_client")]
    pub(super) fn select_cpp_load_screen(
        &self,
        mode: GameMode,
        loading_save_game: bool,
    ) -> Option<game_client::gui::load_screen::LoadScreenKind> {
        let (has_current_campaign, current_campaign_is_challenge) = {
            let campaign_manager = game_client::gui::campaign_manager::get_campaign_manager();
            campaign_manager
                .get_current_campaign()
                .map(|campaign| (true, campaign.is_challenge_campaign()))
                .unwrap_or((false, false))
        };

        game_client::gui::load_screen::select_load_screen(
            game_client::gui::load_screen::LoadScreenRequest {
                mode: Self::load_screen_game_mode(mode),
                loading_save_game,
                has_current_campaign,
                current_campaign_is_challenge,
            },
        )
    }

    #[cfg(feature = "game_client")]
    pub(super) fn prepare_cpp_load_screen_for_mode(
        &mut self,
        mode: GameMode,
        loading_save_game: bool,
    ) {
        self.active_load_screen = self.select_cpp_load_screen(mode, loading_save_game);
    }

    #[cfg(feature = "game_client")]
    pub(super) fn load_screen_init_context(
        &self,
    ) -> game_client::gui::load_screen::LoadScreenInitContext {
        // Prefer presentation game_mode residual when installed.
        let game_info_context = match self.presentation_or_live_game_mode() {
            GameMode::Lan | GameMode::Multiplayer => Some({
                let setup = game_client::gui::get_lan_setup();
                game_client::gui::load_screen::load_screen_init_context_from_game_info(
                    setup.game_info(),
                )
            }),
            GameMode::Skirmish => Some({
                let setup = game_client::gui::get_skirmish_setup();
                game_client::gui::load_screen::load_screen_init_context_from_game_info(
                    setup.game_info().game_info(),
                )
            }),
            _ => None,
        };
        if let Some(context) = game_info_context {
            if !context.slots.is_empty() {
                return context;
            }
        }

        // Prefer full presentation roster when installed (InGame residual);
        // live get_player only boot/menu when no frame.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            if !frame.players.is_empty() {
                let local = frame
                    .player_info(frame.local_player_id)
                    .or_else(|| frame.players.iter().find(|p| p.is_local))
                    .or_else(|| frame.players.first());
                let mut context = game_client::gui::load_screen::LoadScreenInitContext::default();
                if let Some(local) = local {
                    context.local_player_name = local.name.clone();
                    context.local_side_name = local.team.get_name().to_string();
                    context.local_team_number = local.id as i32;
                }
                context.slots = frame
                    .players
                    .iter()
                    .map(|player| {
                        // apparent_color is multiplayer color *index* (progress bar art).
                        // Fail-closed: index not frozen on presentation — leave None.
                        // apparent_text_color is packed 0x00RRGGBB from frozen color_rgb.
                        let (r, g, b) = player.color_rgb;
                        let text_color = ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                        game_client::gui::load_screen::LoadScreenSlotInitContext {
                            player_id: player.id as i32,
                            player_name: player.name.clone(),
                            side_name: player.team.get_name().to_string(),
                            team_number: player.id as i32,
                            apparent_color: None,
                            apparent_text_color: Some(text_color),
                            is_ai: player.is_ai,
                            has_map: true,
                            visible: player.is_alive,
                        }
                    })
                    .collect();
                return context;
            }
        }

        // Wave 237: boot load-screen roster via ui_player_info (presentation-first helper).
        let player = self
            .game_logic
            .local_player_id()
            .and_then(|id| self.ui_player_info(id))
            .or_else(|| self.ui_player_info(self.current_player_id));

        if let Some(player) = player {
            let slot = game_client::gui::load_screen::LoadScreenSlotInitContext {
                player_id: player.id as i32,
                player_name: player.name.clone(),
                side_name: player.team.get_name().to_string(),
                team_number: player.id as i32,
                apparent_color: None,
                apparent_text_color: None,
                is_ai: player.is_ai,
                has_map: true,
                visible: true,
            };
            let mut context = game_client::gui::load_screen::LoadScreenInitContext::default();
            context.local_player_name = slot.player_name.clone();
            context.local_side_name = slot.side_name.clone();
            context.local_team_number = slot.team_number;
            context.slots = vec![slot];
            context
        } else {
            game_client::gui::load_screen::LoadScreenInitContext::default()
        }
    }

    pub(super) fn ensure_shell_loading_overlay(&mut self) {
        if self.startup_loading_phase.trim().is_empty() {
            self.startup_loading_phase = DEFAULT_LOADING_PHASE.to_string();
        }
        self.set_runtime_ui_state_projection(UISystemState::Loading);

        #[cfg(feature = "game_client")]
        {
            if self.loading_overlay_active {
                return;
            }

            let kind = self
                .active_load_screen
                .or_else(|| {
                    // Prefer presentation game_mode residual when installed.
                    self.select_cpp_load_screen(self.presentation_or_live_game_mode(), false)
                })
                .unwrap_or(game_client::gui::load_screen::LoadScreenKind::ShellGame);
            self.active_load_screen = Some(kind);

            let context = self.load_screen_init_context();
            if !game_client::gui::load_screen::init_load_screen(kind, &context) {
                warn!(
                    "Failed to load {:?} load screen from .wnd assets; loading screen unavailable",
                    kind
                );
                error!(
                    "The selected load screen could not be loaded — the loading overlay will not be visible. \
                     Ensure game assets (BIG archives or extracted Data/) are in the correct path. \
                     The game will continue without a loading screen."
                );
                self.active_load_screen = None;
                return;
            }

            self.loading_overlay_active = true;
            LOADING_PROGRESS.with(|p| p.set(0.0));
            LOADING_PHASE.with(|p| *p.borrow_mut() = self.startup_loading_phase.clone());
            info!("Loading screen overlay created as {:?}", kind);
        }
    }

    /// C++ campaign and Challenge load screens finish their movie/min-spec
    /// prelude before map initialization.  This delegates only to GameClient's
    /// safe WindowManager/display/FP pump; it must not dispatch the host
    /// platform event loop while `host_start_game_from_ui` is synchronous.
    #[cfg(feature = "game_client")]
    pub(super) fn run_cpp_load_screen_prelude(&mut self) {
        if !self.loading_overlay_active {
            return;
        }
        let Some(kind) = self.active_load_screen else {
            return;
        };
        let _ = game_client::gui::load_screen::run_load_screen_prelude(kind);
    }

    pub(super) fn hide_shell_loading_overlay(&mut self) {
        if self.startup_loading_phase.trim().is_empty() {
            self.startup_loading_phase = "Startup complete".to_string();
        }

        #[cfg(feature = "game_client")]
        {
            // Do not early-return on a stale `loading_overlay_active` flag.
            // Timeout Loading→Menu and failed overlay init can leave
            // ShellGameLoadScreen.wnd (92%) stacked over MainMenu.
            if let Some(kind) = self.active_load_screen.take() {
                game_client::gui::load_screen::reset_load_screen(kind);
            }
            for kind in [
                game_client::gui::load_screen::LoadScreenKind::ShellGame,
                game_client::gui::load_screen::LoadScreenKind::SinglePlayer,
                game_client::gui::load_screen::LoadScreenKind::Challenge,
                game_client::gui::load_screen::LoadScreenKind::Multiplayer,
                game_client::gui::load_screen::LoadScreenKind::GameSpy,
                game_client::gui::load_screen::LoadScreenKind::MapTransfer,
            ] {
                game_client::gui::load_screen::reset_load_screen(kind);
            }
            self.loading_overlay_active = false;
        }
    }

    /// Product-name title + load-screen teardown for every Menu enter.
    /// C++ GameText.cpp:347 SetWindowText; load screens are not the shell menu.
    pub(super) fn apply_shell_menu_window_chrome(&mut self) {
        self.last_loading_title_update = None;
        self.hide_shell_loading_overlay();
        self.window.set_title(SHELL_MENU_WINDOW_TITLE);
    }

    /// C++ parity: Shell::push("Menus/MainMenu.wnd") + Shell::doPush()
    /// GameLogic::startNewGame() line 2198: TheShell->push("Menus/MainMenu.wnd")
    /// when m_gameMode == GAME_SHELL && screenCount == 0.
    pub(super) fn show_shell_menu(&mut self) {
        #[cfg(feature = "game_client")]
        {
            // C++ Shell::showShell / startNewGame GAME_SHELL inspect the live
            // stack (getScreenCount / top), not a host bookkeeping flag.
            // A stale shell_menu_active==true would skip restore after a WM
            // wipe; a stale false paired with hide_shell_menu tears down
            // MainMenu (hq-pzya / hq-3vo4).

            // C++ TheDisplay size == window client; WND CREATIONRESOLUTION
            // 800x600 scales to that. Set WM screen_size before push so
            // gadgets fill the logical window instead of a 800x600 corner.
            let scale = self.window.scale_factor().max(0.0001);
            let size = self.window.inner_size();
            let logical_w = ((size.width as f64) / scale).round().max(1.0) as i32;
            let logical_h = ((size.height as f64) / scale).round().max(1.0) as i32;
            game_client::gui::with_window_manager(|manager| {
                manager.set_screen_size(logical_w, logical_h);
            });

            // C++ TheShell is initialized via SubsystemInterface before GAME_SHELL push.
            // Thread-local Shell starts uninitialized; push() fails without init.
            let result = game_client::gui::with_shell_mut(|shell| {
                if let Err(e) = game_client::system::SubsystemInterface::init(shell) {
                    return Err(e);
                }
                shell.show_shell_map(true);
                if shell.get_screen_count() == 0 {
                    shell.push("Menus/MainMenu.wnd", false)?;
                } else if let Some(top) = shell.top() {
                    top.hide(false);
                    top.bring_forward();
                }
                let screens = shell.get_screen_count();
                if screens == 0 {
                    return Err(Box::<dyn std::error::Error>::from(
                        "Shell::push returned Ok but screen stack is empty",
                    ));
                }
                shell.set_shell_active(true);
                Ok(screens)
            });
            let screens = match result {
                Some(Ok(screens)) => screens,
                Some(Err(e)) => {
                    warn!(
                        "Failed to init or activate Shell before MainMenu push: {:?}",
                        e
                    );
                    error!(
                        "Shell subsystem failed to initialize — the main menu will not be visible."
                    );
                    return;
                }
                None => {
                    warn!("Shell was re-entered while activating MainMenu; request skipped");
                    return;
                }
            };
            self.shell_menu_active = true;
            info!(
                "Shell menu activated from Menus/MainMenu.wnd (screens={})",
                screens
            );
        }
    }

    pub(super) fn hide_shell_menu(&mut self) {
        #[cfg(feature = "game_client")]
        {
            if !self.shell_menu_active {
                return;
            }

            if let Some(Err(err)) = game_client::gui::with_shell_mut(|shell| shell.hide_shell()) {
                warn!("Failed to hide shell menu: {:?}", err);
            }

            self.shell_menu_active = false;
        }
    }

    pub(super) fn log_startup_health_summary(&mut self) {
        if self.startup_health_summary_logged {
            return;
        }

        if self.startup_stall_events == 0 {
            info!("Startup health: all checks succeeded (progress=100%, stalls=0, render_boot=ok)");
        } else {
            info!(
                "Startup health: completed with {} transient stalls (max_stall={:.2}s), no fatal startup errors",
                self.startup_stall_events,
                self.startup_max_stall_duration.as_secs_f32()
            );
        }

        self.startup_health_summary_logged = true;
    }

    pub(super) fn update_shell_loading_progress(&mut self, progress: f32, phase: Option<&str>) {
        self.startup_last_reported_progress = progress.clamp(0.0, 1.0);
        if let Some(phase) = phase {
            let phase = phase.trim();
            if !phase.is_empty() {
                self.startup_loading_phase = phase.to_string();
            }
        }

        #[cfg(feature = "game_client")]
        {
            LOADING_PROGRESS.with(|p| p.set(self.startup_last_reported_progress));
            LOADING_PHASE.with(|p| *p.borrow_mut() = self.startup_loading_phase.clone());

            if let Some(kind) = self.active_load_screen {
                let percent = self.startup_last_reported_progress * 100.0;
                game_client::gui::load_screen::update_load_screen(kind, percent);
            }
        }
    }

    /// Drive the C++ load-screen frame work from a synchronous map-load
    /// milestone without borrowing the host engine. `host_load_map_or_default`
    /// holds `self.game_logic` mutably while it invokes this path, so keeping
    /// this state in the already-existing thread-local presentation bridge is
    /// what lets every real milestone update the visible .wnd screen.
    #[cfg(feature = "game_client")]
    pub(super) fn pump_cpp_load_screen_progress(
        kind: game_client::gui::load_screen::LoadScreenKind,
        progress: f32,
        phase: &str,
    ) {
        let progress = progress.clamp(0.0, 1.0);
        LOADING_PROGRESS.with(|value| value.set(progress));
        let phase = phase.trim();
        if !phase.is_empty() {
            LOADING_PHASE.with(|value| *value.borrow_mut() = phase.to_string());
        }
        game_client::gui::load_screen::update_load_screen(kind, progress * 100.0);
    }

    pub(super) fn observe_startup_progress(&mut self, progress: f32, phase: &str) {
        let progress = progress.clamp(0.0, 1.0);
        if progress > self.startup_last_reported_progress + 0.001 {
            self.startup_last_reported_progress = progress;
            self.startup_last_progress_change_at = Instant::now();
            self.startup_last_stall_warning_at = None;
            return;
        }

        let stalled_for = self.startup_last_progress_change_at.elapsed();
        let stall_threshold = Self::startup_stall_warning_threshold(progress, phase);
        if stalled_for < Duration::from_secs(2) || stalled_for < stall_threshold {
            return;
        }

        let should_warn = self
            .startup_last_stall_warning_at
            .map(|last| last.elapsed() >= Duration::from_secs(2))
            .unwrap_or(true);
        if !should_warn {
            return;
        }

        self.startup_stall_events = self.startup_stall_events.saturating_add(1);
        self.startup_max_stall_duration = self.startup_max_stall_duration.max(stalled_for);
        if stalled_for >= Duration::from_secs(8) {
            warn!(
                "Startup progress stalled at {:.0}% in phase '{}' for {:.2}s (game_state={:?})",
                progress * 100.0,
                phase,
                stalled_for.as_secs_f32(),
                self.current_state
            );
        } else {
            debug!(
                "Startup progress waiting at {:.0}% in phase '{}' for {:.2}s (game_state={:?})",
                progress * 100.0,
                phase,
                stalled_for.as_secs_f32(),
                self.current_state
            );
        }
        self.startup_last_stall_warning_at = Some(Instant::now());
    }

    pub(super) fn startup_stall_warning_threshold(progress: f32, phase: &str) -> Duration {
        let phase = phase.trim().to_ascii_lowercase();
        if phase.contains("priming shell simulation") {
            Duration::from_secs(25)
        } else if phase.contains("initializing asset manager")
            || phase.contains("loading map data")
            || phase.contains("spawning world objects")
            || phase.contains("finalizing startup")
        {
            Duration::from_secs(20)
        } else {
            let _ = progress;
            Duration::from_secs(12)
        }
    }

    /// Hide in-game layouts when returning to shell menus (C++ HideControlBar parity).
    pub(super) fn hide_gameplay_layouts(&mut self) {
        info!(
            "hide_gameplay_layouts: ControlBar / in-game layout teardown (shell overlay owns UI)"
        );
        #[cfg(feature = "game_client")]
        {
            let _ = game_client::gui::callbacks::control_bar_callbacks::hide_control_bar(true);
        }
        // Window manager layouts are also suspended via ui_manager.suspend_for_shell_overlay()
        // on the Menu transition path.
    }

    /// Ensure ControlBar / in-game layout is available when entering gameplay.
    ///
    /// C++ `ShowControlBar` loads ControlBar.wnd. This is **not** a silent no-op:
    /// it resolves retail assets, validates them, and attempts a window load when
    /// the client GUI is available. Missing assets are logged honestly.
    pub(super) fn ensure_gameplay_layouts(&mut self) {
        // C++ ShowControlBar: resolve + validate + load into TheWindowManager
        // when assets present. Ready-gate window_count is the live instance.
        let honesty = crate::gameplay_layout::control_bar_layout_honesty(true);
        let report = crate::gameplay_layout::format_control_bar_honesty(&honesty);
        match &honesty.status {
            crate::gameplay_layout::GameplayLayoutStatus::Ready { path, loaded } => {
                info!(
                    "ensure_gameplay_layouts: {} (path={}, loaded={}, windows={})",
                    report, path, loaded, honesty.window_count
                );
                // Windowed InGame: materialise ControlBar.wnd into TheWindowManager
                // only when the ControlBar parent gadget is missing. Leftover
                // MainMenu/Skirmish window_count must not skip the load.
                #[cfg(feature = "game_client")]
                if !self.runtime_host_headless {
                    if crate::gameplay_layout::control_bar_parent_is_live() {
                        info!("ensure_gameplay_layouts: ControlBar parent already live ({path})");
                    } else {
                        let loaded = crate::gameplay_layout::materialise_live_control_bar();
                        if loaded {
                            info!(
                                "ensure_gameplay_layouts: TheWindowManager ControlBar live=true path={path}"
                            );
                        } else {
                            error!(
                                "ensure_gameplay_layouts: TheWindowManager ControlBar live=false path={path} searched={:?}",
                                crate::gameplay_layout::CONTROL_BAR_CANDIDATES
                            );
                        }
                    }
                    let _ =
                        game_client::gui::callbacks::control_bar_callbacks::show_control_bar(true);
                    // show_control_bar no-ops when ControlBarState.visible is
                    // already true; the WND parent can still be HIDDEN from
                    // HideControlBar at boot. Force the retail parent visible.
                    game_client::gui::with_window_manager(|wm| {
                        if let Some(parent) =
                            wm.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
                        {
                            if parent.borrow().is_hidden() {
                                let _ = parent.borrow_mut().hide(false);
                            }
                        }
                    });
                    // C++ GameLogic.cpp:2233 setControlBarSchemeByPlayer(local)
                    // then ShowControlBar switches DEFAULT (ControlBarCallback.cpp:489).
                    if crate::gameplay_layout::control_bar_parent_is_live() {
                        let side = self
                            .ui_local_player_team_name()
                            .filter(|name| !name.is_empty())
                            .unwrap_or_else(|| "Observer".to_string());
                        game_client::helpers::TheControlBar::set_control_bar_scheme_by_player(
                            &side,
                        );
                        self.control_bar
                            .set_control_bar_scheme_by_player_side(&side);
                        self.control_bar.switch_control_bar_stage(
                            game_client::gui::control_bar::ControlBarStage::Default,
                        );
                    }
                }
            }
            crate::gameplay_layout::GameplayLayoutStatus::AssetsUnavailable { searched } => {
                error!(
                    "ensure_gameplay_layouts: ControlBar assets unavailable (searched {:?}). {}",
                    searched, report
                );
            }
            crate::gameplay_layout::GameplayLayoutStatus::LoadFailed { path, error } => {
                error!(
                    "ensure_gameplay_layouts: ControlBar load failed path={} error={} ({})",
                    path, error, report
                );
            }
        }
        // C++ InGameUI::createControlBar + ShowControlBar always leave
        // ControlBarParent on TheWindowManager. Disk-probe Ready/miss must
        // not skip the archive resolver when the live parent is still gone.
        #[cfg(feature = "game_client")]
        if !self.runtime_host_headless && !crate::gameplay_layout::control_bar_parent_is_live() {
            let loaded = crate::gameplay_layout::materialise_live_control_bar();
            if loaded {
                info!(
                    "ensure_gameplay_layouts: ControlBarParent materialised on live WM after enter"
                );
                let _ = game_client::gui::callbacks::control_bar_callbacks::show_control_bar(true);
                game_client::gui::with_window_manager(|wm| {
                    if let Some(parent) =
                        wm.find_window_by_name(crate::gameplay_layout::CONTROL_BAR_PARENT_NAME)
                    {
                        if parent.borrow().is_hidden() {
                            let _ = parent.borrow_mut().hide(false);
                        }
                    }
                });
            } else {
                error!(
                    "ensure_gameplay_layouts: ControlBarParent missing on live WM after InGame enter; searched={:?}",
                    crate::gameplay_layout::CONTROL_BAR_CANDIDATES
                );
            }
        }
    }

    pub(super) fn to_engine_timing(clock: ClockFrameTiming, frame_start: Instant) -> FrameTiming {
        let sync_time = clock.total_time.as_millis() as u32;
        let previous_sync_time = sync_time.saturating_sub(clock.delta_time.as_millis() as u32);
        FrameTiming {
            frame_number: clock.frame_number,
            delta_time: clock.delta_time,
            total_time: clock.total_time,
            fps: if clock.delta_time.as_secs_f32() > 0.0 {
                1.0 / clock.delta_time.as_secs_f32()
            } else {
                0.0
            },
            frame_start,
            sync_time,
            previous_sync_time,
        }
    }

    /// C++ GameEngine.cpp:633-642 disables ShellMapOn only when MapCache lacks
    /// `m_shellMapName`. An empty boot cache must not hide a disk-resident
    /// ShellMapMD extract (hq-4yc1).
    pub(super) fn resolve_windowed_shell_map_asset(name: &str) -> Option<String> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        if game_client::map_util::is_map_cached_without_refresh(name)
            || crate::game_logic::find_map_file(name).is_some()
        {
            return Some(name.to_string());
        }
        // GameData.ini uses Maps\ShellMapMD\ShellMapMD.map; the extract lives
        // under MapsZH/Maps/ShellMapMD/ShellMapMD.map. Try the leaf so a
        // disk-resident map is not treated as missing (hq-4yc1).
        let normalized = name.replace('\\', "/");
        let leaf = std::path::Path::new(&normalized)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .trim();
        if !leaf.is_empty()
            && (game_client::map_util::is_map_cached_without_refresh(leaf)
                || crate::game_logic::find_map_file(leaf).is_some())
        {
            return Some(leaf.to_string());
        }
        None
    }

    pub(super) fn startup_shell_map_asset_available(name: &str) -> bool {
        Self::resolve_windowed_shell_map_asset(name).is_some()
    }

    /// ZH MD GameData.ini `ShellMapName` used when GlobalData still has the
    /// empty Common default at windowed boot.
    pub(super) const DEFAULT_WINDOWED_SHELL_MAP: &'static str = r"Maps\ShellMapMD\ShellMapMD.map";

    /// C++ Shell::showShellMap(TRUE) starts GAME_SHELL with pending_file =
    /// ShellMapName. Windowed boot must attempt that map when the asset exists
    /// and fail-soft when it does not (hq-4yc1).
    pub(super) fn windowed_shell_map_load_decision(
        start_in_menu: bool,
        map_to_load: Option<&str>,
    ) -> bool {
        let _ = start_in_menu;
        map_to_load
            .map(|name| !name.trim().is_empty())
            .unwrap_or(false)
    }

    pub(super) fn configured_startup_shell_map() -> Option<String> {
        let global = game_engine::common::global_data::read();
        if !global.writable.shell_map_on {
            info!("configured_startup_shell_map: shell_map_on=false");
            return None;
        }
        let mut shell_map_name = global.writable.shell_map_name.clone();
        drop(global);

        if shell_map_name.trim().is_empty() {
            shell_map_name = Self::DEFAULT_WINDOWED_SHELL_MAP.to_string();
        }

        if let Some(resolved) = Self::resolve_windowed_shell_map_asset(&shell_map_name) {
            info!("configured_startup_shell_map: using {resolved}");
            let mut global = game_engine::common::global_data::write();
            if global.writable.shell_map_name.trim().is_empty() {
                global.writable.shell_map_name = shell_map_name;
            }
            return Some(resolved);
        }

        // Main GlobalData default is Generals ShellMap1. ZH GameData.ini
        // uses ShellMapMD. Remap only that stale default so a truly missing
        // override still disables shell_map_on (GameEngine.cpp:633-642).
        let zh_name = Self::DEFAULT_WINDOWED_SHELL_MAP;
        let stale_generals = shell_map_name
            .replace('\\', "/")
            .to_ascii_lowercase()
            .contains("shellmap1");
        if stale_generals {
            if let Some(resolved) = Self::resolve_windowed_shell_map_asset(zh_name) {
                let mut global = game_engine::common::global_data::write();
                global.writable.shell_map_name = zh_name.to_string();
                return Some(resolved);
            }
            if let Some(resolved) = Self::resolve_windowed_shell_map_asset("ShellMapMD") {
                let mut global = game_engine::common::global_data::write();
                global.writable.shell_map_name = zh_name.to_string();
                return Some(resolved);
            }
        }

        warn!(
            "Configured shell map '{}' was not found in map cache or on disk; starting without a shell background map",
            shell_map_name
        );
        // C++ parity (GameEngine.cpp:633-642): disable shell-map mode globally
        // when the configured shell map is missing so later UI flow sees it off.
        let mut global = game_engine::common::global_data::write();
        global.writable.shell_map_on = false;
        None
    }

    pub(super) fn current_startup_logic_frame(&self) -> u64 {
        // Use engine frame cadence for startup budgeting. Game-logic frame counters can jump
        // during long blocking startup operations, which over-ages menu startup budgets.
        self.frame_counter as u64
    }

    pub(super) fn shell_start_frame(&self) -> Option<u64> {
        // Anchor startup age to the frame where menu state became active when available.
        // Shell enqueue can happen earlier during loading and should not age out menu
        // startup budgets before first visible menu frames.
        self.menu_enter_frame.or(self.shell_ui_enqueued_frame)
    }

    pub(super) fn startup_deferred_model_load_budget(
        current_state: GameState,
        startup_frame: Option<u64>,
        current_logic_frame: u64,
    ) -> usize {
        // C++ AssetManager is async. Menu/GAME_SHELL must never flip
        // allow_sync_model_loads (budget==0) or 271 missing W3Ds beachball
        // the render thread (hq-fq1h). Non-Menu match play may sync.
        if current_state != GameState::Menu {
            return 0;
        }

        let startup_age = startup_frame
            .map(|frame| current_logic_frame.saturating_sub(frame))
            .unwrap_or(0);
        match startup_age {
            0 => 4,
            1..=2 => 8,
            3..=7 => 12,
            _ => 16,
        }
    }

    pub(super) fn maybe_trigger_deferred_caustic_warmup(&mut self) {
        let _ = self;
    }

    #[cfg(feature = "game_client")]
    pub(super) fn should_skip_world_scene_for_shell_menu(&self) -> bool {
        // C++ W3DDisplay::draw always paints the 3D scene (shell map) then UI.
        // Skipping the world after a few Menu frames left a frozen/empty terrain
        // backdrop while MainMenu buttons were still first-run hidden.
        // Loading: no 3D world. Menu and InGame: always draw.
        matches!(self.current_state, GameState::Loading)
    }

    #[cfg(not(feature = "game_client"))]
    pub(super) fn should_skip_world_scene_for_shell_menu(&self) -> bool {
        false
    }

    pub(super) fn configured_startup_camera_defaults() -> StartupCameraDefaults {
        let global = game_engine::common::global_data::read();
        StartupCameraDefaults {
            pitch_degrees: global.camera_pitch,
            yaw_degrees: global.camera_yaw,
            camera_height: global.camera_height,
            max_camera_height: global.max_camera_height,
        }
    }

    pub(super) fn select_startup_camera_focus(
        is_shell_game: bool,
        metadata_target: Option<Vec2>,
        team_target: Option<Vec2>,
        world_center: Vec2,
    ) -> Vec2 {
        if is_shell_game {
            // C++ shell startup prefers InitialCameraPosition and only falls back to the
            // legacy W3DView seed when the waypoint is absent.
            metadata_target.unwrap_or(Vec2::new(
                87.0 * gamelogic::common::MAP_XY_FACTOR,
                77.0 * gamelogic::common::MAP_XY_FACTOR,
            ))
        } else {
            metadata_target.or(team_target).unwrap_or(world_center)
        }
    }

    pub(super) fn bootstrap_camera_for_loaded_map(
        // Wave 473: presentation freeze only — no live GameLogic dual-read.
        is_shell_game: bool,
        current_player_id: u32,
        defaults: StartupCameraDefaults,
        presentation: Option<&crate::presentation_frame::PresentationFrame>,
    ) -> (Vec3, Vec3, f32) {
        const DEFAULT_VIEW_WIDTH: f32 = 640.0;
        const DEFAULT_VIEW_HEIGHT: f32 = 480.0;
        let _ = (DEFAULT_VIEW_WIDTH, DEFAULT_VIEW_HEIGHT); // retained for C++ parity docs
        let (world_min, world_max) = if let Some(pres) = presentation {
            pres.world_env.world_bounds_vec3()
        } else {
            (Vec3::new(-500.0, 0.0, -500.0), Vec3::new(500.0, 0.0, 500.0))
        };
        let world_center = Vec3::new(
            (world_min.x + world_max.x) * 0.5,
            (world_min.y + world_max.y) * 0.5,
            (world_min.z + world_max.z) * 0.5,
        );
        let metadata_initial_camera: Option<Vec3> = if let Some(pres) = presentation {
            // C++ startNewGame looks at InitialCameraPosition waypoint.
            // presentation.camera_focus is the script MOVE_CAMERA_TO queue
            // (often y=0) and must not replace the map waypoint (hq-bq4n).
            pres.world_env
                .initial_camera_position
                .map(|f| Vec3::new(f[0], f[1], f[2]))
                .or_else(|| pres.camera_focus.map(|f| Vec3::new(f[0], f[1], f[2])))
        } else {
            None
        };
        // Gameplay ground is X/Z (Y-up). InitialCamera.y is height, not a map axis.
        let metadata_target = metadata_initial_camera.map(|pos| Vec2::new(pos.x, pos.z));

        let clamp_focus_to_world = |focus: Vec2| {
            Vec2::new(
                focus.x.clamp(world_min.x, world_max.x),
                focus.y.clamp(world_min.z, world_max.z),
            )
        };
        let team_target = if let Some(pres) = presentation {
            // Wave 223/458: frozen local team base; no live get_player/team_base dual-read.
            pres.local_team_base_position
                .map(|pos| Vec2::new(pos.x, pos.z))
        } else {
            None
        };
        let focus_2d = clamp_focus_to_world(Self::select_startup_camera_focus(
            is_shell_game,
            metadata_target,
            team_target,
            Vec2::new(world_center.x, world_center.z),
        ));

        // Match C++ W3DView::lookAt(): unlike the old 2D View::lookAt(), the W3D path writes the
        // requested world coordinate directly into m_pos and builds the camera transform from that.
        let terrain_target = Vec3::new(focus_2d.x, 0.0, focus_2d.y);
        // Wave 473: sample heights from presentation freeze only.
        let (camera_anchor_ground_height, terrain_height_max) =
            Self::sample_startup_camera_heights(terrain_target, world_center.y, presentation);
        let focus_target = Vec3::new(focus_2d.x, 0.0, focus_2d.y);
        let (focus_ground_height, _) =
            Self::sample_startup_camera_heights(focus_target, world_center.y, presentation);

        // Keep the C++ zoom/offset sampling from the top-left anchor, but aim the modern
        // Rust camera at the requested scene focus. This remains the closest visible match for the
        // current renderer bridge.
        let camera_target = Vec3::new(focus_2d.x, focus_ground_height, focus_2d.y);
        let camera_offset_z = camera_anchor_ground_height + defaults.camera_height.max(0.0);
        let pitch_radians = defaults.pitch_degrees.to_radians();
        let yaw_radians = defaults.yaw_degrees.to_radians();
        let camera_offset_y = if pitch_radians.tan().abs() > f32::EPSILON {
            -(camera_offset_z / pitch_radians.tan())
        } else {
            0.0
        };
        let camera_offset_x = -(camera_offset_y * yaw_radians.tan());

        // Match W3DView::setZoomToDefault exactly: desired zoom is the visible terrain max
        // around the look-at point plus max camera height, divided by the base offset height.
        let zoom = Self::compute_default_camera_zoom_from_heights(
            camera_anchor_ground_height,
            terrain_height_max,
            defaults,
            1.0,
        );

        // Match W3DView::buildCameraTransform when angle/pitch defaults are zero:
        // source = cameraOffset * zoom; source *= (1 - ground / source.z); then translate.
        let source_z = camera_offset_z * zoom;
        let factor = if source_z.abs() > f32::EPSILON {
            1.0 - (camera_anchor_ground_height / source_z)
        } else {
            1.0
        };
        let source = Vec3::new(
            camera_offset_x * zoom * factor,
            camera_offset_z * zoom * factor,
            camera_offset_y * zoom * factor,
        );
        let camera_position = camera_target + source;

        info!(
            "Startup camera bootstrap: raw_initial={:?} requested_focus_2d={:?} target={:?} position={:?} ground_height={:.2} terrain_height_max={:.2} camera_offset=({:.2}, {:.2}, {:.2}) pitch_deg={:.2} yaw_deg={:.2} zoom={:.2} factor={:.3}",
            metadata_initial_camera,
            focus_2d,
            camera_target,
            camera_position,
            camera_anchor_ground_height,
            terrain_height_max,
            camera_offset_x,
            camera_offset_y,
            camera_offset_z,
            defaults.pitch_degrees,
            defaults.yaw_degrees,
            zoom,
            factor,
        );

        (camera_target, camera_position, zoom)
    }

    pub(super) fn sample_startup_camera_heights(
        // Wave 473: presentation height grid only — no live GameLogic dual-read.
        terrain_target: Vec3,
        fallback_ground_height: f32,
        presentation: Option<&crate::presentation_frame::PresentationFrame>,
    ) -> (f32, f32) {
        const MAX_GROUND_LEVEL: f32 = 120.0;
        const TERRAIN_SAMPLE_SIZE: f32 = 40.0;

        // Prefer presentation-frozen height grid / bounds when a frame is installed.
        let (world_min, world_max) = if let Some(pres) = presentation {
            pres.world_env.world_bounds_vec3()
        } else {
            (Vec3::new(-500.0, 0.0, -500.0), Vec3::new(500.0, 0.0, 500.0))
        };

        let sample_one = |pos: Vec3| -> f32 {
            let clamped = Vec3::new(
                pos.x.clamp(world_min.x, world_max.x),
                pos.y,
                pos.z.clamp(world_min.z, world_max.z),
            );
            if let Some(pres) = presentation {
                if let Some(h) = pres.world_env.sample_height(clamped.x, clamped.z) {
                    return h.min(MAX_GROUND_LEVEL);
                }
            }
            // Wave 473: fail-closed fallback when no presentation height sample.
            fallback_ground_height.min(MAX_GROUND_LEVEL)
        };

        let mut ground_height = sample_one(terrain_target);
        if ground_height > MAX_GROUND_LEVEL {
            ground_height = MAX_GROUND_LEVEL;
        }

        let sample_positions = [
            terrain_target,
            terrain_target + Vec3::new(TERRAIN_SAMPLE_SIZE, 0.0, -TERRAIN_SAMPLE_SIZE),
            terrain_target + Vec3::new(-TERRAIN_SAMPLE_SIZE, 0.0, -TERRAIN_SAMPLE_SIZE),
            terrain_target + Vec3::new(TERRAIN_SAMPLE_SIZE, 0.0, TERRAIN_SAMPLE_SIZE),
            terrain_target + Vec3::new(-TERRAIN_SAMPLE_SIZE, 0.0, TERRAIN_SAMPLE_SIZE),
        ];
        let terrain_height_max = sample_positions
            .into_iter()
            .map(sample_one)
            .fold(ground_height, f32::max);
        (ground_height, terrain_height_max)
    }

    pub(super) fn compute_default_camera_zoom_from_heights(
        ground_height: f32,
        terrain_height_max: f32,
        defaults: StartupCameraDefaults,
        max_height_scale: f32,
    ) -> f32 {
        let camera_offset_z = ground_height + defaults.camera_height.max(0.0);
        // Match C++ W3DView::setDefaultView()/setZoomToDefault():
        // maxHeight is a scale on GlobalData.maxCameraHeight, and angle does not participate.
        let desired_height =
            terrain_height_max + (defaults.max_camera_height * max_height_scale.max(0.0)).max(0.0);
        if camera_offset_z.abs() > f32::EPSILON {
            desired_height / camera_offset_z
        } else {
            1.0
        }
    }

    pub(super) fn compute_default_camera_zoom_for_target(
        &self,
        target: Vec3,
        max_height_scale: f32,
    ) -> f32 {
        let defaults = Self::configured_startup_camera_defaults();
        // Wave 241: no live dual-read when presentation freeze is installed.
        let (ground_height, terrain_height_max) = Self::sample_startup_camera_heights(
            target,
            target.y,
            self.last_presentation_frame.as_ref(),
        );
        Self::compute_default_camera_zoom_from_heights(
            ground_height,
            terrain_height_max,
            defaults,
            max_height_scale,
        )
    }

    pub(super) fn write_startup_debug_state(&self) {
        let _ = self;
    }

    pub(super) fn emit_startup_load_progress(
        sender: &mpsc::Sender<StartupLoadMessage>,
        progress: f32,
        phase: &str,
    ) {
        let _ = sender.send(StartupLoadMessage::Progress {
            progress: progress.clamp(0.0, 0.995),
            phase: phase.to_string(),
        });
    }

    pub(super) fn spawn_startup_map_load(
        start_in_menu: bool,
        map_to_load: Option<String>,
        map_requested_from_cli: bool,
        map_requested_from_initial_file: bool,
        replay_to_load: Option<String>,
        replay_requested_from_cli: bool,
        player_name: Option<String>,
    ) -> StartupLoadState {
        let (sender, receiver) = mpsc::channel();
        let worker_gen = startup_worker_generation();
        thread::spawn(move || {
            Self::emit_startup_load_progress(&sender, 0.03, "Preparing startup archive access");
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
                || -> std::result::Result<StartupLoadResult, String> {
                    let worker_stop_if_abandoned = || -> std::result::Result<(), String> {
                        if startup_worker_owns(worker_gen) {
                            Ok(())
                        } else {
                            Err(
                                "startup worker abandoned; host owns session (skip INI/stream)"
                                    .into(),
                            )
                        }
                    };
                    worker_stop_if_abandoned()?;
                    let mut start_in_menu = start_in_menu;
                    let mut map_to_load = map_to_load;
                    let replay_startup_requested = replay_to_load.is_some();
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|err| {
                            format!("failed to create startup tokio runtime for archive access: {err}")
                        })?;
                    Self::emit_startup_load_progress(&sender, 0.14, "Startup archives ready");

                    let extract_ini_text_from_archives = |virtual_path: &str| -> Option<String> {
                        // Prefer extracted INIZH on disk — archive extract holds the
                        // asset-manager mutex and can stall Loading for minutes.
                        if let Some(text) = Self::read_startup_ini_from_disk(virtual_path) {
                            return Some(text);
                        }
                        runtime.block_on(async {
                            let Some(manager_arc) = crate::assets::manager::get_asset_manager()
                            else {
                                return None;
                            };
                            let Ok(mut manager) = manager_arc.try_lock() else {
                                warn!(
                                    "Asset manager busy while extracting '{}'; skipping",
                                    virtual_path
                                );
                                return None;
                            };
                            match manager.extract_file(virtual_path).await {
                                Ok(bytes) => {
                                    match String::from_utf8(bytes) {
                                        Ok(text) => Some(text),
                                        Err(err) => {
                                            warn!(
                                                "INI file '{}' was not valid UTF-8: {err}; skipping",
                                                virtual_path
                                            );
                                            None
                                        }
                                    }
                                }
                                Err(_) => None,
                            }
                        })
                    };

                    // C++ parity: force eager initialization of the lazy stores/managers that
                    // the original boot path expects to exist before game-session setup.
                    worker_stop_if_abandoned()?;
                    game_engine::common::ini::initialize_ini_systems();

                    Self::emit_startup_load_progress(
                        &sender,
                        0.145,
                        "Preloading water and weather settings",
                    );
                    Self::preload_startup_water_weather_inis();
                    Self::emit_startup_load_progress(
                        &sender,
                        0.15,
                        "Water and weather settings ready",
                    );

                    {
                        let lexicon =
                            game_engine::common::system::function_lexicon::get_function_lexicon();
                        let guard = lexicon.lock();
                        if let Ok(mut lexicon_guard) = guard {
                            if let Err(err) = game_engine::common::system::SubsystemInterface::init(&mut *lexicon_guard) {
                                warn!("FunctionLexicon init failed during startup bootstrap: {err}. Continuing without function lexicon.");
                            }
                        } else {
                            warn!("Function lexicon lock poisoned during startup bootstrap; skipping");
                        }
                    }

                    // These bootstrap calls are required for startup parity. Any panic in this
                    // section is caught by the outer startup worker guard and treated as fatal.
                    game_engine::common::ini::init_rank_info_store();

                    // C++ parity: GameEngine.cpp:398 — load Science.ini (Default + override)
                    // into the global ScienceStore via the general INI block parser.
                    {
                        for sci_path in ["Data/INI/Default/Science.ini", "Data/INI/Science.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(sci_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded science definitions from {}", sci_path),
                                    Err(err) => warn!("Failed parsing Science.ini '{}': {}", sci_path, err),
                                }
                            }
                        }
                    }

                    // C++ parity: GameEngine.cpp:427 — load Rank.ini into TheRankInfoStore.
                    // No Default/ prefix variant exists for Rank.ini.
                    {
                        if let Some(content) = extract_ini_text_from_archives("Data/INI/Rank.ini") {
                            let mut ini = game_engine::common::ini::INI::new();
                            match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                Ok(()) => {
                                    let store = game_engine::common::ini::ini_rank::get_rank_info_store();
                                    if store.is_empty() {
                                        warn!("Rank.ini loaded 0 rank definitions — continuing without rank data");
                                    }
                                }
                                Err(err) => {
                                    warn!("Failed parsing Rank.ini: {}", err);
                                }
                            }
                        } else {
                            warn!("Rank.ini not found in archives — continuing without rank data");
                        }
                    }

                    // C++ parity: GameEngine.cpp:428 — load PlayerTemplate.ini (Default + override)
                    {
                        for pt_path in ["Data/INI/Default/PlayerTemplate.ini", "Data/INI/PlayerTemplate.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(pt_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded player template definitions from {}", pt_path),
                                    Err(err) => warn!("Failed parsing PlayerTemplate.ini '{}': {}", pt_path, err),
                                }
                            }
                        }
                    }

                    // C++ parity: GameEngine.cpp:399 — load Multiplayer.ini (Default + override)
                    // into the global MULTIPLAYER_SETTINGS OnceCell.
                    {
                        let mut loaded_any_multiplayer = false;
                        for mp_path in [
                            "Data/INI/Default/Multiplayer.ini",
                            "Data/INI/Multiplayer.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(mp_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => {
                                        loaded_any_multiplayer = true;
                                        info!("Loaded {}", mp_path);
                                    }
                                    Err(err) => {
                                        warn!("Failed parsing {}: {}", mp_path, err);
                                    }
                                }
                            }
                        }
                        if !loaded_any_multiplayer {
                            warn!("No Multiplayer.ini found in archives — continuing without multiplayer settings");
                        }
                    }

                    let _ = game_engine::common::ini::ini_terrain::initialize_terrain_types();
                    // C++ parity: GameEngine.cpp:400 — load Terrain.ini (Default + override)
                    {
                        for terrain_path in ["Data/INI/Default/Terrain.ini", "Data/INI/Terrain.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(terrain_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded terrain definitions from {}", terrain_path),
                                    Err(err) => warn!("Failed parsing Terrain.ini '{}': {}", terrain_path, err),
                                }
                            }
                        }
                    }

                    let _ = game_engine::common::ini::ini_terrain_bridge::initialize_terrain_roads();
                    // C++ parity: GameEngine.cpp:401 — load Roads.ini (Default + override)
                    {
                        for roads_path in ["Data/INI/Default/Roads.ini", "Data/INI/Roads.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(roads_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded road definitions from {}", roads_path),
                                    Err(err) => warn!("Failed parsing Roads.ini '{}': {}", roads_path, err),
                                }
                            }
                        }
                    }

                    game_engine::common::ini::ini_special_power::initialize_special_power_store();

                    // C++ parity: GameEngine.cpp:439 — load FXList.ini (Default + override)
                    {
                        for fxl_path in ["Data/INI/Default/FXList.ini", "Data/INI/FXList.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(fxl_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded FX list definitions from {}", fxl_path),
                                    Err(err) => warn!("Failed parsing FXList.ini '{}': {}", fxl_path, err),
                                }
                            }
                        }
                    }

                    // C++ parity: GameEngine.cpp:440 — load Weapon.ini into TheWeaponStore.
                    // No Default/ prefix variant exists for Weapon.ini.
                    worker_stop_if_abandoned()?;
                    game_engine::common::ini::ini_weapon::initialize_weapon_store();
                    {
                        if let Some(content) = extract_ini_text_from_archives("Data/INI/Weapon.ini") {
                            let mut ini = game_engine::common::ini::INI::new();
                            match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                Ok(()) => info!("Loaded weapon definitions from Data/INI/Weapon.ini"),
                                Err(err) => warn!("Failed parsing Weapon.ini: {}", err),
                            }
                        } else {
                            warn!("Weapon.ini not found in archives — continuing without weapon data");
                        }
                    }

                    // C++ parity: GameEngine.cpp:443 — load SpecialPower.ini (Default + override)
                    // into the global SpecialPowerStore via the general INI block parser.
                    {
                        let mut loaded_any_special_power = false;
                        for sp_path in [
                            "Data/INI/Default/SpecialPower.ini",
                            "Data/INI/SpecialPower.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(sp_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| {
                                    ini.parse_current_file()
                                }) {
                                    Ok(()) => {
                                        loaded_any_special_power = true;
                                    }
                                    Err(err) => {
                                        warn!(
                                            "Failed parsing special power definitions from '{}': {}",
                                            sp_path, err
                                        );
                                    }
                                }
                            }
                        }
                        if !loaded_any_special_power {
                            warn!("SpecialPower.ini bootstrap loaded 0 templates — continuing without special power INI data");
                        }
                    }

                    game_engine::common::ini::ini_damage_fx::init_global_damage_fx_store();
                    game_engine::common::damage_fx::initialize_damage_fx_store();

                    // C++ parity: GameEngine.cpp:444 — load DamageFX.ini into TheDamageFXStore.
                    // No Default/ prefix variant exists for DamageFX.ini.
                    {
                        if let Some(content) = extract_ini_text_from_archives("Data/INI/DamageFX.ini") {
                            let mut ini = game_engine::common::ini::INI::new();
                            match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                Ok(()) => {
                                    if let Some(store) = game_engine::common::ini::ini_damage_fx::get_damage_fx_store() {
                                        if store.get_damage_fx_names().is_empty() {
                                            warn!("DamageFX.ini loaded 0 definitions — continuing without damage FX data");
                                        }
                                    }
                                }
                                Err(err) => {
                                    warn!("Failed parsing DamageFX.ini: {}", err);
                                }
                            }
                        } else {
                            warn!("DamageFX.ini not found in archives — continuing without damage FX data");
                        }
                    }

                    game_engine::common::system::build_assistant::init_build_assistant();

                    // C++ parity: bootstrap OCL/Armor from archive-backed INI content, not just
                    // extracted-file paths, so startup behavior matches original archive loading.
                    {
                        gamelogic::object_creation_list::init_object_creation_list_store();
                        let mut loaded_any_ocl = false;
                        for ocl_path in [
                            "Data/INI/Default/ObjectCreationList.ini",
                            "Data/INI/ObjectCreationList.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(ocl_path) {
                                match gamelogic::object_creation_list::store::load_object_creation_lists_from_str(&content) {
                                    Ok(count) => {
                                        loaded_any_ocl |= count > 0;
                                    }
                                    Err(load_err) => {
                                        warn!(
                                            "Failed parsing OCL definitions from '{}': {}",
                                            ocl_path, load_err
                                        );
                                    }
                                }
                            }
                        }
                        if !loaded_any_ocl {
                            gamelogic::object_creation_list::store::ensure_default_object_creation_lists_loaded();
                        }
                        let ocl_count = gamelogic::object_creation_list::get_object_creation_list_store()
                            .as_ref()
                            .map(|store| store.get_ocl_count())
                            .unwrap_or(0);
                        if ocl_count == 0 {
                            warn!("ObjectCreationListStore bootstrap loaded 0 templates — continuing without OCL data");
                        }
                    }

                    {
                        let mut loaded_any_armor = false;
                        for armor_path in ["Data/INI/Armor.ini", "Data/INI/Default/Armor.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(armor_path) {
                                match gamelogic::object::armor::load_armor_templates_from_str(
                                    &content,
                                    Some(Path::new(armor_path)),
                                ) {
                                    Ok(count) => {
                                        loaded_any_armor |= count > 0;
                                    }
                                    Err(load_err) => {
                                        warn!(
                                            "Failed parsing armor templates from '{}': {}",
                                            armor_path, load_err
                                        );
                                    }
                                }
                            }
                        }
                        if !loaded_any_armor {
                            gamelogic::object::armor::ensure_default_templates_loaded();
                        }
                        let armor_count = gamelogic::object::armor::TheArmorStore::read().len();
                        if armor_count == 0 {
                            warn!("Armor bootstrap loaded 0 templates — continuing without armor data");
                        }
                    }

                    // C++ parity: GameEngine.cpp:442 — TheLocomotorStore loads
                    // Default/Locomotor.ini then Locomotor.ini from archives.
                    {
                        let mut loaded_any_locomotor = false;
                        for loco_path in [
                            "Data/INI/Default/Locomotor.ini",
                            "Data/INI/Locomotor.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(loco_path) {
                                match game_engine::common::ini::ini_locomotor::load_locomotors_from_str(&content) {
                                    Ok(count) => {
                                        loaded_any_locomotor |= count > 0;
                                    }
                                    Err(load_err) => {
                                        warn!(
                                            "Failed parsing locomotor templates from '{}': {}",
                                            loco_path, load_err
                                        );
                                    }
                                }
                            }
                        }
                        if !loaded_any_locomotor {
                            warn!("Locomotor bootstrap loaded 0 templates from archives — relying on hardcoded defaults");
                        }
                    }

                    // C++ parity: GameEngine.cpp:468 — load Upgrade.ini (Default + override)
                    game_engine::common::ini::ini_upgrade::initialize_upgrade_center();
                    {
                        for upgrade_path in ["Data/INI/Default/Upgrade.ini", "Data/INI/Upgrade.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(upgrade_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded upgrade definitions from {}", upgrade_path),
                                    Err(err) => warn!("Failed parsing Upgrade.ini '{}': {}", upgrade_path, err),
                                }
                            }
                        }
                    }

                    // C++ parity: GameEngine.cpp:480 — AIData.ini after Upgrade, before Crate.
                    {
                        let mut loaded_any = false;
                        for ai_path in Self::startup_ai_data_ini_paths() {
                            if let Some(content) = extract_ini_text_from_archives(ai_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file())
                                {
                                    Ok(()) => {
                                        info!("Loaded AIData from {}", ai_path);
                                        loaded_any = true;
                                    }
                                    Err(err) => warn!(
                                        "Failed parsing AIData.ini '{}': {}",
                                        ai_path, err
                                    ),
                                }
                            }
                        }
                        if loaded_any {
                            Self::apply_startup_ai_data_enable_repulsors();
                        } else {
                            Self::preload_startup_ai_data_inis();
                        }
                        // EnableRepulsors is parsed before SideInfo; apply leftover even if
                        // a later nested field warned.
                        let store = game_engine::common::ini::get_ai_data_store();
                        if store.read().expect("AI data store read lock").get_active().is_some()
                        {
                            Self::apply_startup_ai_data_enable_repulsors();
                        }
                    }

                    // C++ parity: GameEngine.cpp:483 — load Crate.ini (Default + override) into ParsedCrateSystem.
                    {
                        for crate_ini_path in &[
                            "Data/INI/Default/Crate.ini",
                            "Data/INI/Crate.ini",
                        ] {
                            if let Some(content) = extract_ini_text_from_archives(crate_ini_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => {
                                        info!("Loaded crate definitions from '{}'", crate_ini_path);
                                    }
                                    Err(err) => {
                                        warn!("Failed parsing '{}': {}", crate_ini_path, err);
                                    }
                                }
                            } else {
                                warn!("'{}' not found in archives — skipping crate definitions from this file", crate_ini_path);
                            }
                        }
                    }

                    if let Err(err) = game_engine::common::thing::init_thing_system() {
                        warn!("Thing system init failed during startup bootstrap: {err}. Continuing without thing system.");
                    }
                    if !game_engine::common::thing::thing_factory::ensure_system_ini_drawable_only_templates()
                    {
                        warn!(
                            "System.ini GenericTracer (DRAWABLE_ONLY + W3DTracerDraw) failed to register"
                        );
                    }

                    {
                        let object_ini_paths =
                            super::object_ini_boot::collect_object_ini_virtual_paths();
                        let mut total_loaded = 0usize;
                        for ini_path in &object_ini_paths {
                            if let Some(content) = extract_ini_text_from_archives(ini_path) {
                                total_loaded += game_engine::common::thing::load_templates_from_ini_text(
                                    &content,
                                    ini_path,
                                );
                            }
                        }
                        if total_loaded > 0 {
                            info!("Bootstrapped {} object templates from Default/Object + FileSystem dir", total_loaded);
                        }
                    }

                    {
                        let crc = super::ini_crc_boot::calculate_game_engine_ini_crc(
                            |path| extract_ini_text_from_archives(path),
                        );
                        super::ini_crc_boot::publish_ini_crc(crc);
                        info!("GameEngine init-table INI CRC: {crc:08X}");
                    }

                    // C++ ControlBar data is a paired catalog: CommandSet entries
                    // refer to CommandButton entries, whose UNIT_BUILD `Object =`
                    // field is the authoritative producible template identity.
                    // Load it after Object INI so the GameLogic bridge can retain
                    // typed thing-template references instead of name heuristics.
                    let mut command_buttons_parsed = false;
                    for button_path in [
                        "Data/INI/Default/CommandButton.ini",
                        "Data/INI/CommandButton.ini",
                    ] {
                        if let Some(content) = extract_ini_text_from_archives(button_path) {
                            let mut ini = game_engine::common::ini::INI::new();
                            match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                Ok(()) => {
                                    command_buttons_parsed = true;
                                    info!("Loaded command button definitions from {}", button_path);
                                }
                                Err(err) => {
                                    warn!(
                                        "Failed parsing CommandButton.ini '{}': {}",
                                        button_path, err
                                    );
                                    command_buttons_parsed = false;
                                    break;
                                }
                            }
                        }
                    }

                    let mut command_sets_parsed = false;
                    if command_buttons_parsed {
                        for set_path in ["Data/INI/Default/CommandSet.ini", "Data/INI/CommandSet.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(set_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => {
                                        command_sets_parsed = true;
                                        info!("Loaded command set definitions from {}", set_path);
                                    }
                                    Err(err) => {
                                        warn!(
                                            "Failed parsing CommandSet.ini '{}': {}",
                                            set_path, err
                                        );
                                        command_sets_parsed = false;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    let parsed_button_count = game_engine::common::ini::ini_command_button::get_control_bar()
                        .map(|bar| bar.count())
                        .unwrap_or(0);
                    let parsed_set_count = game_engine::common::ini::ini_command_set::get_command_set_manager()
                        .map(|sets| sets.count())
                        .unwrap_or(0);
                    if command_buttons_parsed
                        && command_sets_parsed
                        && parsed_button_count > 0
                        && parsed_set_count > 0
                    {
                        // GameClient reads this bridge for the same live
                        // CommandSet buttons shown to the player; Main's
                        // production authorization reads its typed UNIT_BUILD
                        // identities through the bridge as well.
                        if let Err(err) = gamelogic::control_bar::refresh_control_bar_bridge_from_common() {
                            warn!(
                                "Failed to refresh parsed ControlBar bridge; production catalog unavailable: {}",
                                err
                            );
                        }
                    } else {
                        warn!(
                            "CommandButton/CommandSet catalog unavailable (buttons={}, sets={}); retaining fail-closed compatibility production fallback",
                            parsed_button_count,
                            parsed_set_count
                        );
                    }

                    // C++ parity: GameEngine.cpp:500-501 — load CommandMap.ini (language-specific + fallback).
                    // C++ loads "Data\{language}\CommandMap.ini" then "Data\INI\CommandMap.ini".
                    game_engine::common::ini::ini_command_map::init_meta_map();
                    {
                        let language = game_engine::common::ini::ini_webpage_url::get_registry_language();
                        let lang_path = format!("Data/{}/CommandMap.ini", language.as_str());
                        for cmd_path in &[lang_path.as_str(), "Data/INI/CommandMap.ini"] {
                            if let Some(content) = extract_ini_text_from_archives(cmd_path) {
                                let mut ini = game_engine::common::ini::INI::new();
                                match ini.with_inline_source(&content, |ini| ini.parse_current_file()) {
                                    Ok(()) => info!("Loaded command map from {}", cmd_path),
                                    Err(err) => warn!("Failed parsing CommandMap.ini '{}': {}", cmd_path, err),
                                }
                            }
                        }
                    }

                    worker_stop_if_abandoned()?;
                    Self::emit_startup_load_progress(&sender, 0.18, "Creating game session");
                    let mut game_logic = GameLogic::initialize();
                    Self::emit_startup_load_progress(&sender, 0.22, "Priming object templates");

                    if map_requested_from_initial_file {
                        // C++ parity: .map initial-file startup enqueues MSG_NEW_GAME
                        // (GAME_SINGLE_PLAYER, DIFFICULTY_NORMAL, 0) and seeds RNG with 0.
                        let stream = game_engine::common::message_stream::get_message_stream();
                        if let Ok(mut stream_guard) = stream.write() {
                            let msg = stream_guard
                                .append_message(game_engine::common::message_stream::GameMessageType::NewGame);
                            msg.append_integer_argument(0); // GAME_SINGLE_PLAYER
                            msg.append_integer_argument(1); // DIFFICULTY_NORMAL
                            msg.append_integer_argument(0); // rank points
                        } else {
                            warn!("Failed to queue startup NewGame message for initial-file map");
                        }
                        game_engine::common::random_value::init_random_with_seed(0);
                    }

                    if let Some(replay_to_load) = replay_to_load.as_ref() {
                        Self::emit_startup_load_progress(
                            &sender,
                            0.24,
                            "Starting replay playback",
                        );

                        // C++ parity: bootstrap startup replay through the legacy recorder.
                        crate::save_load::apply_host_recorder_init_controls();
                        game_engine::common::recorder::init_recorder();
                        let startup_command_sink: Arc<
                            dyn Fn(game_engine::common::message_stream::GameMessage)
                                + Send
                                + Sync,
                        > = Arc::new(|message| {
                            let stream = game_engine::common::message_stream::get_message_stream();
                            let write_result = stream.write();
                            match write_result {
                                Ok(mut stream_guard) => {
                                    CnCGameEngine::append_common_message_to_stream(
                                        &mut stream_guard,
                                        &message,
                                    );
                                }
                                Err(err) => {
                                    warn!(
                                        "Failed to forward recorder startup command into message stream: {}",
                                        err
                                    );
                                }
                            }
                        });
                        let _ = game_engine::common::recorder::with_recorder_mut(|recorder| {
                            recorder.set_command_sink(Some(startup_command_sink));
                        });
                        let replay_to_play = replay_to_load.to_ascii_lowercase();
                        match game_engine::common::recorder::with_recorder_mut(|recorder| {
                            recorder.playback_file(replay_to_play.clone())
                        }) {
                            Some(Ok(true)) => {}
                            Some(Ok(false)) => {
                                warn!(
                                    "Legacy recorder rejected startup replay '{}'",
                                    replay_to_load
                                );
                            }
                            Some(Err(err)) => {
                                warn!(
                                    "Legacy recorder replay bootstrap failed for '{}': {}",
                                    replay_to_load, err
                                );
                            }
                            None => {
                                warn!(
                                    "Legacy recorder unavailable for startup replay '{}'",
                                    replay_to_load
                                );
                            }
                        }
                    }

                    worker_stop_if_abandoned()?;
                    let startup_messages = Self::take_startup_messages_from_stream(worker_gen)
                        .unwrap_or_default();
                    let startup_new_game =
                        Self::startup_new_game_dispatch_from_messages(&startup_messages);

                    if replay_startup_requested && startup_new_game.is_none() {
                        warn!(
                            "Startup replay did not emit a queued NewGame message; falling back to menu startup"
                        );
                        start_in_menu = true;
                        map_to_load = None;
                        game_engine::common::global_data::write().pending_file.clear();
                    }

                    let startup_mode = Self::resolve_startup_mode_from_dispatch(
                        &mut start_in_menu,
                        &mut map_to_load,
                        startup_new_game,
                        replay_startup_requested,
                    );

                    if replay_startup_requested && !start_in_menu && map_to_load.is_none() {
                        warn!(
                            "Startup replay did not resolve a playable map; falling back to menu startup"
                        );
                        start_in_menu = true;
                        game_engine::common::global_data::write().pending_file.clear();
                    }

                    worker_stop_if_abandoned()?;
                    game_logic.start_new_game(startup_mode);

                    let mut loaded_map_name = None;
                    // C++ Shell::showShellMap(TRUE) (Shell.cpp:448-466) starts
                    // GAME_SHELL with m_shellMapName even on the main menu.
                    // Windowed boot used to skip ShellMapMD so Menu could paint;
                    // load fail-soft instead (hq-4yc1). Missing assets still skip.
                    if let Some(map_to_load) = map_to_load
                        .as_ref()
                        .filter(|_| {
                            Self::windowed_shell_map_load_decision(
                                start_in_menu,
                                map_to_load.as_deref(),
                            )
                        })
                        .cloned()
                    {
                        Self::emit_startup_load_progress(&sender, 0.24, "Loading map data");
                        let map_loaded =
                            game_logic.load_map_with_progress(&map_to_load, |progress, phase| {
                                Self::emit_startup_load_progress(&sender, progress, phase);
                            });
                        if !map_loaded {
                            if start_in_menu {
                                warn!(
                                    "Failed to load shell map '{}'; continuing startup without a shell background map",
                                    map_to_load
                                );
                                game_logic.clearGameData();
                            } else if map_requested_from_cli {
                                warn!(
                                    "Failed to load startup map '{}'; falling back to menu startup",
                                    map_to_load
                                );
                                game_logic.start_new_game(GameMode::Shell);
                                start_in_menu = true;
                            } else {
                                warn!(
                                    "Failed to load startup map '{}'; falling back to menu startup with empty scene",
                                    map_to_load
                                );
                                game_logic.start_new_game(GameMode::Shell);
                                start_in_menu = true;
                            }
                        } else {
                            loaded_map_name = Some(map_to_load);
                        }
                    } else {
                        Self::emit_startup_load_progress(&sender, 0.24, "Skipping shell map load");
                        if start_in_menu {
                            info!(
                                "No shell background map available; entering menu without a shell background map"
                            );
                        }
                    }

                    if let Some(player_name) = player_name.as_deref() {
                        if game_logic.set_player_name(0, player_name) {
                            info!("Set local player name to '{}'", player_name);
                        } else {
                            warn!("Failed to apply player name '{}'", player_name);
                        }
                    }

                    if start_in_menu && game_logic.isInShellGame() {
                        // C++ ticks GAME_SHELL on the main loop. Two worker
                        // update_shell_with_budget steps were hanging minutes
                        // (pathfinding/scripts + ASSET_MANAGER) and never sent
                        // Complete, so TerrainVisual never installed.
                        Self::emit_startup_load_progress(
                            &sender,
                            0.968,
                            "Priming shell simulation",
                        );
                        info!("Startup shell simulation warmup deferred to Menu ticks");
                    }

                    Self::emit_startup_load_progress(&sender, 0.984, "Finalizing startup data");

                    Ok(StartupLoadResult {
                        game_logic,
                        loaded_map_name,
                        start_in_menu,
                        map_requested_from_cli,
                        replay_requested: replay_startup_requested,
                    })
                },
            ))
            .map_err(|panic_payload| {
                if let Some(message) = panic_payload.downcast_ref::<&str>() {
                    format!("startup map load panicked: {message}")
                } else if let Some(message) = panic_payload.downcast_ref::<String>() {
                    format!("startup map load panicked: {message}")
                } else {
                    "startup map load panicked with non-string payload".to_string()
                }
            })
            .and_then(|inner| inner);

            let _ = sender.send(StartupLoadMessage::Complete(result));
        });

        StartupLoadState::InProgress {
            receiver,
            started_at: Instant::now(),
            last_worker_progress: 0.0,
            last_worker_phase: None,
            last_worker_logged_bucket: 0,
        }
    }

    /// Wave 610: via `host_finalize_startup_map_load`.
    pub(super) fn finalize_startup_map_load(&mut self, result: StartupLoadResult) -> Result<()> {
        // Wave 610: thin wrapper — residual via host helper.
        self.host_finalize_startup_map_load(result)
    }

    pub(super) fn host_finalize_startup_map_load(
        &mut self,
        result: StartupLoadResult,
    ) -> Result<()> {
        // Wave 610: host residual helper.
        self.update_shell_loading_progress(0.995, Some("Finalizing startup"));
        self.host_replace_game_logic(result.game_logic);
        // The boot presentation frame describes the pre-load, terrain-less
        // GameLogic instance.  Rebuild it after the worker handoff so WGPU sees
        // the retail map's decoded HeightMapData, lighting, roads, and bounds.
        self.render_pipeline.set_presentation_frame(None);
        self.last_presentation_frame = None;

        if let Some(active_map_name) = result.loaded_map_name.as_ref() {
            if result.replay_requested {
                info!("Loaded startup replay map: {}", active_map_name);
            } else if result.map_requested_from_cli {
                info!("Loaded map from command line: {}", active_map_name);
            } else if result.start_in_menu {
                info!("Loaded startup shell map: {}", active_map_name);
            } else {
                info!("Loaded startup initial-file map: {}", active_map_name);
            }

            // Wave 455: seed presentation env then apply presentation-only heightmap/skybox hints.
            // Windowed menu (hq-4yc1) must still install the C++ ShellMapMD backdrop
            // before GameState::Menu; fail-soft if later GPU setup warns.
            self.ensure_presentation_env_seeded();
            Self::apply_heightmap_hint(&mut self.render_pipeline);
            Self::apply_skybox_hint(&mut self.render_pipeline);
            self.ensure_presentation_env_seeded();
            Self::sync_render_terrain_visual(
                &mut self.render_pipeline,
                &self.graphics_system,
                active_map_name.as_str(),
            );
            if let Err(err) = self.reinitialize_minimap_renderer() {
                warn!(
                    "Failed to reinitialize minimap renderer: {err}. Continuing without minimap."
                );
            }
            self.ensure_presentation_env_seeded();
            Self::apply_map_lighting(&mut self.graphics_system, &mut self.render_pipeline);
            let startup_camera_defaults = Self::configured_startup_camera_defaults();
            // Wave 458: prefer pipeline presentation freeze; live GameLogic only if missing.
            let startup_camera_presentation = self
                .render_pipeline
                .presentation_frame()
                .or(self.last_presentation_frame.as_ref());
            // Wave 540/552: prefer presentation fow_shell_bypass when freeze present.
            let in_shell_camera = self.shell_bypass_from_presentation(startup_camera_presentation);
            (self.camera_target, self.camera_position, self.camera_zoom) =
                Self::bootstrap_camera_for_loaded_map(
                    in_shell_camera,
                    self.current_player_id,
                    startup_camera_defaults,
                    startup_camera_presentation,
                );
            self.sync_orbit_from_camera_transform();
        }

        let fallback_to_menu = result.start_in_menu
            || (result.map_requested_from_cli && result.loaded_map_name.is_none());
        if fallback_to_menu {
            // Menu must not stay in Loading after the worker completed.
            if result.map_requested_from_cli && result.loaded_map_name.is_none() {
                warn!("QuickStart map load failed; falling back to menu startup");
            }
            self.pending_shell_model_prewarm.clear();
            self.last_shell_prewarm_log = None;
            self.shell_prewarm_completion_logged = true;
            self.ui_manager.suspend_for_shell_overlay();
            self.set_runtime_ui_state_projection(UISystemState::MainMenu);
            let _ = self.startup_target_state.take();
            self.transition_to_state(GameState::Menu);
            self.startup_load_state = StartupLoadState::Complete;
            self.last_loading_title_update = None;
            self.update_shell_loading_progress(1.0, Some("Startup complete"));
            self.startup_last_reported_progress = 1.0;
            self.startup_last_progress_change_at = Instant::now();
            self.startup_last_stall_warning_at = None;
            self.hide_shell_loading_overlay();
            self.log_startup_health_summary();
            self.window.set_title(SHELL_MENU_WINDOW_TITLE);
            self.window.request_redraw();
            return Ok(());
        }
        let target_state = self.startup_target_state.take();

        if let Some(target_state) = target_state {
            // Apply the post-load state transition immediately so we do not render additional
            // loading/world-only frames after shell/menu resources are already initialized.
            // A CLI/initial-file startup can go straight from Loading to
            // InGame. `host_replace_game_logic` intentionally invalidated the
            // runtime direct Drawable bindings above, so hydrate a frozen
            // frame and its direct client associations before the first
            // redraw. Menu starts remain intentionally unseeded.
            if target_state == GameState::InGame {
                self.seed_presentation_after_match_start();
            }
            self.transition_to_state(target_state);
        }
        self.startup_load_state = StartupLoadState::Complete;
        self.last_loading_title_update = None;
        self.update_shell_loading_progress(1.0, Some("Startup complete"));
        self.startup_last_reported_progress = 1.0;
        self.startup_last_progress_change_at = Instant::now();
        self.startup_last_stall_warning_at = None;
        self.hide_shell_loading_overlay();
        self.log_startup_health_summary();
        self.window.set_title(SHELL_MENU_WINDOW_TITLE);
        self.window.request_redraw();
        Ok(())
    }

    /// Drop an in-flight boot worker so a shipped `start_game_from_ui` / Menu
    /// release owns the session. The worker's later Complete is ignored.
    pub(super) fn abandon_startup_load_worker(&mut self) {
        if matches!(self.startup_load_state, StartupLoadState::InProgress { .. }) {
            // Invalidate the in-flight worker so it cannot clear NewGame or
            // keep mutating INI/weapon stores after the host owns the session.
            bump_startup_worker_generation();
            self.startup_load_state = StartupLoadState::Complete;
            let _ = self.startup_target_state.take();
        }
    }

    /// True when the boot overlay should release to Menu instead of waiting
    /// forever for INI / optional shell-map decode.
    pub(super) fn startup_load_should_release_to_menu(&self) -> bool {
        let StartupLoadState::InProgress {
            started_at,
            last_worker_progress,
            ..
        } = &self.startup_load_state
        else {
            return false;
        };
        let wants_menu = self.startup_start_in_menu
            || matches!(self.startup_target_state, Some(GameState::Menu) | None);
        if !wants_menu {
            return false;
        }
        // Session create is 0.18; after that the remaining work is optional shell.
        (*last_worker_progress >= 0.18 && started_at.elapsed() >= Duration::from_secs(8))
            || started_at.elapsed() >= Duration::from_secs(15)
    }

    pub(super) fn update_startup_loading(&mut self) -> Result<()> {
        let mut result: Option<std::result::Result<StartupLoadResult, String>> = None;
        let mut visual_phase = None::<String>;
        let mut visual_progress = None::<f32>;
        match &mut self.startup_load_state {
            StartupLoadState::Idle | StartupLoadState::Complete => return Ok(()),
            StartupLoadState::InProgress {
                receiver,
                started_at,
                last_worker_progress,
                last_worker_phase,
                last_worker_logged_bucket,
            } => {
                loop {
                    match receiver.try_recv() {
                        Ok(StartupLoadMessage::Progress { progress, phase }) => {
                            let clamped = progress.clamp(0.0, 0.995);
                            if clamped > *last_worker_progress {
                                *last_worker_progress = clamped;
                            }
                            if last_worker_phase.as_deref() != Some(phase.as_str()) {
                                info!(
                                    "Startup worker phase: {} ({:.0}%)",
                                    phase,
                                    (*last_worker_progress) * 100.0
                                );
                            }
                            let bucket = ((*last_worker_progress * 100.0).floor() as i32)
                                .div_euclid(10)
                                .clamp(0, 10) as u8;
                            if bucket > *last_worker_logged_bucket {
                                debug!(
                                    "Startup worker progress: {:.0}% ({})",
                                    (*last_worker_progress) * 100.0,
                                    phase
                                );
                                *last_worker_logged_bucket = bucket;
                            }
                            *last_worker_phase = Some(phase);
                        }
                        Ok(StartupLoadMessage::Complete(complete)) => {
                            info!(
                                "Startup shell/game load completed in {:.2}s",
                                started_at.elapsed().as_secs_f32()
                            );
                            result = Some(complete);
                            break;
                        }
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => {
                            return Err(anyhow::anyhow!("startup load worker disconnected"));
                        }
                    }
                }

                if result.is_none() {
                    let elapsed = started_at.elapsed().as_secs_f32();
                    let (fallback_phase, fallback_progress) = Self::loading_visual_phase(elapsed);
                    let chosen_progress = (*last_worker_progress).max(fallback_progress);
                    let chosen_phase = last_worker_phase
                        .as_deref()
                        .unwrap_or(fallback_phase)
                        .to_string();
                    visual_phase = Some(chosen_phase);
                    visual_progress = Some(chosen_progress);
                }
            }
        }

        if let (Some(phase), Some(progress)) = (visual_phase, visual_progress) {
            self.update_shell_loading_progress(progress, Some(&phase));
            self.observe_startup_progress(progress, &phase);
            if self
                .last_loading_title_update
                // Avoid hammering native window-title updates during startup; on macOS these
                // updates can become expensive when issued every frame.
                .map(|last| last.elapsed() >= Duration::from_millis(350))
                .unwrap_or(true)
            {
                self.window.set_title(&format!(
                    "Command & Conquer Generals Zero Hour - Loading {phase} ({:.0}%)",
                    progress * 100.0
                ));
                self.last_loading_title_update = Some(Instant::now());
            }
            self.window.request_redraw();
            return Ok(());
        }

        match result.expect("startup completion result missing") {
            Ok(load_result) => self.finalize_startup_map_load(load_result),
            Err(err) => Err(anyhow::anyhow!(err)),
        }
    }
}
