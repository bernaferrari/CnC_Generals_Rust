#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

impl CnCGameEngine {
    pub(super) fn runtime_host_cmd_exit(&mut self, _args: &HashMap<String, String>) {
        self.request_state_change(GameState::Exiting);
    }

    pub(super) fn runtime_host_cmd_menu(&mut self, _args: &HashMap<String, String>) {
        self.enter_shell_menu_from_runtime_host(None);
        self.runtime_host_last_gameplay_cmd = "menu_ok".into();
    }

    pub(super) fn runtime_host_cmd_toggle_pause(&mut self, _args: &HashMap<String, String>) {
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "pause_fail_bad_state".into();
        } else {
            let was_paused = self.game_paused || matches!(self.current_state, GameState::Paused);
            self.toggle_pause();
            let now_paused = self.game_paused || matches!(self.current_state, GameState::Paused);
            self.runtime_host_last_gameplay_cmd = if !was_paused && now_paused {
                "pause_ok:paused".into()
            } else if was_paused && !now_paused {
                "pause_ok:resumed".into()
            } else if now_paused {
                "pause_ok:paused".into()
            } else {
                "pause_ok:resumed".into()
            };
        }
    }

    pub(super) fn runtime_host_cmd_open_message_of_the_day(
        &mut self,
        _args: &HashMap<String, String>,
    ) {
        self.enter_shell_menu_from_runtime_host(Some("MessageOfDay"));
    }

    pub(super) fn runtime_host_cmd_open_get_updates(&mut self, _args: &HashMap<String, String>) {
        self.enter_shell_menu_from_runtime_host(Some("GetUpdates"));
    }

    pub(super) fn runtime_host_cmd_open_world_builder(&mut self, _args: &HashMap<String, String>) {
        self.enter_shell_menu_from_runtime_host(Some("WorldBuilder"));
    }

    pub(super) fn runtime_host_cmd_options_probe(&mut self, _args: &HashMap<String, String>) {
        // Honesty residual: prove options host wiring without leaving InGame
        // (full open_options pauses / swaps UI and is covered separately).
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "options_probe_ok".into();
        } else {
            self.runtime_host_last_gameplay_cmd = "options_probe_fail_bad_state".into();
        }
    }

    pub(super) fn runtime_host_cmd_open_options(&mut self, _args: &HashMap<String, String>) {
        self.set_runtime_host_ui_screen_override(None);
        let mut wnd_ok = false;
        if matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.ui_manager.transition_to_screen(Screen::Options);
            if self.current_state == GameState::InGame {
                self.request_state_change(GameState::Paused);
            }
        } else {
            #[cfg(feature = "game_client")]
            {
                wnd_ok = game_client::gui::simulate_main_menu_options_button_gadget_selected();
            }
            self.enter_shell_options_from_runtime_host();
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_options_ok_wnd".into()
        } else {
            "options_ok".into()
        };
    }

    pub(super) fn runtime_host_cmd_open_credits(&mut self, _args: &HashMap<String, String>) {
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            wnd_ok = game_client::gui::simulate_main_menu_credits_button_gadget_selected();
            let _ = game_client::gui::callbacks::simulate_credits_menu_bind_controls();
        }
        self.enter_shell_screen_from_runtime_host(Some("Credits"), "Menus/CreditsMenu.wnd");
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            "open_credits_ok_wnd".into()
        } else {
            "open_credits_ok".into()
        };
    }

    pub(super) fn runtime_host_cmd_click_credits_menu(&mut self, args: &HashMap<String, String>) {
        // Retail CreditsMenu ESC skip / finished / shutdown residual.
        let action = args
            .get("action")
            .map(|v| v.trim().to_ascii_lowercase())
            .unwrap_or_else(|| "skip".to_string());
        let mut wnd_ok = false;
        #[cfg(feature = "game_client")]
        {
            use game_client::gui::callbacks::{
                drive_os_wnd_credits_menu_finished_like_cpp,
                drive_os_wnd_credits_menu_prepare_skip_like_cpp,
                drive_os_wnd_credits_menu_skip_like_cpp, simulate_credits_menu_finished,
                simulate_credits_menu_prepare_skip, simulate_credits_menu_shutdown,
                simulate_credits_menu_skip,
            };
            wnd_ok = match action.as_str() {
                "finished" | "finish" => {
                    drive_os_wnd_credits_menu_finished_like_cpp()
                        || simulate_credits_menu_finished()
                }
                "shutdown" => simulate_credits_menu_shutdown(),
                "prepare_skip" => {
                    drive_os_wnd_credits_menu_prepare_skip_like_cpp()
                        || simulate_credits_menu_prepare_skip()
                }
                _ => drive_os_wnd_credits_menu_skip_like_cpp() || simulate_credits_menu_skip(),
            };
        }
        self.runtime_host_last_gameplay_cmd = if wnd_ok {
            format!("click_credits_menu_ok_wnd_{action}")
        } else {
            "click_credits_menu_miss".into()
        };
    }

    /// Windowed sit-through: left-click a named WND gadget through the same
    /// `handle_mouse_button_input` path as physical winit input so
    /// `note_menu_wnd_click` can latch. Headless always fails closed.
    ///
    /// Not `drive_os_wnd_*` — that path never enters `input` / evidence.
    /// Does **not** call `note_menu_wnd_click` directly.
    pub(super) fn runtime_host_cmd_winit_click_named(&mut self, args: &HashMap<String, String>) {
        if self.runtime_host_headless {
            self.runtime_host_last_gameplay_cmd = "winit_click_named_fail_headless".into();
            return;
        }
        let name = args
            .get("name")
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("");
        if name.is_empty() {
            self.runtime_host_last_gameplay_cmd = "winit_click_named_fail_no_name".into();
            return;
        }
        // Only push MainMenu when shell stack is empty — never re-enter an active
        // MainMenu (shutdown/init stall / SIGSEGV class failures).
        if self.current_state == GameState::Menu && !self.shell_menu_active {
            self.enter_shell_screen_from_runtime_host(Some("MainMenu"), "Menus/MainMenu.wnd");
        }
        #[cfg(feature = "game_client")]
        {
            let _ = game_client::gui::reveal_main_menu_first_input_like_cpp();
        }
        let ok = self.inject_winit_equivalent_named_gadget_click(name);
        self.runtime_host_last_gameplay_cmd = if ok {
            format!(
                "winit_click_named_ok:{name}:menu_click={}",
                self.interactive_playability.menu_wnd_click
            )
        } else {
            format!("winit_click_named_miss:{name}")
        };
    }

    /// Windowed sit-through: CHAR reveal + Single Player + Skirmish via
    /// `inject_winit_equivalent_named_gadget_click` only (shared mouse latch).
    ///
    /// Menu→match path only: `ButtonSinglePlayer` then `ButtonSkirmish`.
    /// Parent/Ruler/Options layout chrome **never** counts as nav success.
    /// Fails closed when shell is not active or gadgets are not under-cursor hittable.
    /// Never calls `note_menu_wnd_click` directly.
    pub(super) fn runtime_host_cmd_winit_menu_nav(&mut self, _args: &HashMap<String, String>) {
        log::info!(
            "winit_menu_nav: enter state={:?} shell_active={}",
            self.current_state,
            self.shell_menu_active
        );
        if self.runtime_host_headless {
            self.runtime_host_last_gameplay_cmd = "winit_menu_nav_fail_headless".into();
            return;
        }
        // Require live shell WND tree. Prefer engine residual; also accept
        // GameClient shell active (status `shell_active` can be true from either).
        #[cfg(feature = "game_client")]
        let shell_live = self.shell_menu_active
            || game_client::gui::with_shell_ref(|shell| shell.is_shell_active()).unwrap_or(false);
        #[cfg(not(feature = "game_client"))]
        let shell_live = self.shell_menu_active;
        if !shell_live {
            self.runtime_host_last_gameplay_cmd = format!(
                "winit_menu_nav_miss:shell_not_active:state={:?}",
                self.current_state
            );
            return;
        }
        if self.current_state != GameState::Menu {
            self.runtime_host_last_gameplay_cmd =
                format!("winit_menu_nav_fail_not_menu:{:?}", self.current_state);
            return;
        }
        // Keep engine residual aligned with live shell for inject consume residual.
        self.shell_menu_active = true;
        // C++ first-run CHAR reveal so SP/Skirmish gadgets are visible.
        // Avoid bulk tick_main_menu_transitions here (WM update op-drain hang);
        // transition ticks only after a successful SP click.
        #[cfg(feature = "game_client")]
        {
            // Soft reveal only — full GWM_CHAR path can hang in transition_set_group
            // / WM op drain while control commands are applied mid-frame.
            log::info!("winit_menu_nav: soft CHAR reveal for host inject");
            let _ = game_client::gui::soft_reveal_main_menu_for_host_inject();
        }
        log::info!("winit_menu_nav: after soft reveal, probing menu→match gadgets");
        // Menu→match gadgets only — never Parent/Ruler/Options.
        const MENU_MATCH_GADGETS: &[&str] = &[
            "MainMenu.wnd:ButtonSinglePlayer",
            "MainMenu.wnd:ButtonSkirmish",
            "SkirmishGameOptionsMenu.wnd:ButtonStart",
            "SkirmishMapSelectMenu.wnd:ButtonOk",
        ];
        let mut hit_name = String::new();
        let mut saw_menu_match_gadget = false;
        for name in MENU_MATCH_GADGETS {
            log::info!("winit_menu_nav: trying {name}");
            // Real under-cursor hit residual (not geometry-only).
            if self.named_gadget_center_if_hittable(name).is_none() {
                log::info!("winit_menu_nav: under-cursor miss for {name}");
                // After SP open, Skirmish may need a short transition.
                if name.contains("Skirmish") {
                    #[cfg(feature = "game_client")]
                    game_client::gui::tick_main_menu_transitions(2);
                    if self.named_gadget_center_if_hittable(name).is_none() {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            log::info!("winit_menu_nav: under-cursor hit ok for {name}, injecting");
            if self.inject_winit_equivalent_named_gadget_click(name) {
                hit_name = (*name).to_string();
                saw_menu_match_gadget = true;
                if name.contains("Skirmish") {
                    self.runtime_host_saw_skirmish_menu = true;
                    // Host start_game owns match start. Do not push
                    // SkirmishGameOptionsMenu layout mid-inject (RefCell panic).
                    #[cfg(feature = "game_client")]
                    game_client::gui::clear_deferred_shell_pushes();
                }
                // Allow dropdown transition before Skirmish (bounded ticks).
                if name.contains("SinglePlayer") {
                    #[cfg(feature = "game_client")]
                    game_client::gui::tick_main_menu_transitions(2);
                }
            }
        }
        log::info!(
            "winit_menu_nav: hit={hit_name:?} menu_click={} skirmish={}",
            self.interactive_playability.menu_wnd_click,
            self.runtime_host_saw_skirmish_menu
        );
        // Success requires a menu→match gadget hit (SP and/or Skirmish) AND
        // real menu_wnd_click latch. Parent-geometry weasel cannot pass.
        self.runtime_host_last_gameplay_cmd = if saw_menu_match_gadget
            && self.interactive_playability.menu_wnd_click
            && self.interactive_playability.skirmish_path
            && !hit_name.is_empty()
        {
            format!("winit_menu_nav_ok:hit={hit_name}:menu_click=true:skirmish_path=true")
        } else if saw_menu_match_gadget {
            format!(
                "winit_menu_nav_partial:hit={hit_name}:menu_click={}:skirmish_path={}",
                self.interactive_playability.menu_wnd_click,
                self.interactive_playability.skirmish_path
            )
        } else {
            "winit_menu_nav_miss:no_menu_match_gadget".into()
        };
    }

    /// Windowed sit-through: select (host) then RMB order through
    /// `inject_winit_equivalent_gameplay_order_click` so evidence only latches
    /// inside `handle_mouse_button_input` (never a direct evidence setter call).
    pub(super) fn runtime_host_cmd_winit_gameplay_order(
        &mut self,
        _args: &HashMap<String, String>,
    ) {
        if self.runtime_host_headless {
            self.runtime_host_last_gameplay_cmd = "winit_gameplay_order_fail_headless".into();
            return;
        }
        if !matches!(self.current_state, GameState::InGame | GameState::Paused) {
            self.runtime_host_last_gameplay_cmd = "winit_gameplay_order_fail_not_ingame".into();
            return;
        }
        // Host select for selection residual; evidence still requires RMB inject.
        // Prefer engine/host selection residual (host_set_selection stamps presentation).
        if self.selected_objects.is_empty()
            && self.ui_selected_ids(self.current_player_id).is_empty()
        {
            self.runtime_host_cmd_select_local_unit(_args);
        }
        let had_sel = !self.selected_objects.is_empty()
            || !self.ui_selected_ids(self.current_player_id).is_empty();
        // Only path that may latch gameplay_order: shared mouse handler via inject.
        // handle_mouse_button_input also uses ui_selected_ids for had_selection —
        // presentation stamp in host_set_selection keeps those residual sources aligned.
        log::info!(
            "winit_gameplay_order: pre-inject had_sel={} match_started={} menu_click={} selected={}",
            had_sel,
            self.interactive_playability.match_started_from_menu_wnd,
            self.interactive_playability.menu_wnd_click,
            self.selected_objects.len()
        );
        self.inject_winit_equivalent_gameplay_order_click();
        log::info!(
            "winit_gameplay_order: post-inject order={} complete={}",
            self.interactive_playability.gameplay_order,
            self.interactive_playability.gameplay_complete()
        );
        // Optional host move residual (does not touch interactive evidence setters).
        if had_sel {
            self.runtime_host_cmd_move(_args);
        }
        self.runtime_host_last_gameplay_cmd = if self.interactive_playability.gameplay_complete() {
            "winit_gameplay_order_ok".into()
        } else if had_sel {
            format!(
                "winit_gameplay_order_partial:selected_no_order_latch:match={}:order={}:sel={}",
                self.interactive_playability.match_started_from_menu_wnd,
                self.interactive_playability.gameplay_order,
                self.selected_objects.len()
            )
        } else {
            "winit_gameplay_order_fail_no_selection".into()
        };
    }

    /// Park the OS window in point-space so a HID clicker can hit gadgets
    /// that would otherwise sit under overlapping IDE/agent windows.
    /// Does not inject mouse and does not touch playable_claim evidence.
    pub(super) fn runtime_host_cmd_window_move(&mut self, args: &HashMap<String, String>) {
        if self.runtime_host_headless {
            self.runtime_host_last_gameplay_cmd = "window_move_fail_headless".into();
            return;
        }
        let x: i32 = args.get("x").and_then(|s| s.parse().ok()).unwrap_or(0);
        let y: i32 = args.get("y").and_then(|s| s.parse().ok()).unwrap_or(80);
        let scale = self.window.scale_factor().max(0.0001);
        let px = ((x as f64) * scale).round() as i32;
        let py = ((y as f64) * scale).round() as i32;
        self.window
            .set_outer_position(winit::dpi::PhysicalPosition::new(px, py));
        self.window.focus_window();
        make_host_window_key_and_accept_mouse(&self.window);
        let (ox, oy, ow, oh) = self.runtime_host_window_outer_rect();
        self.runtime_host_last_gameplay_cmd = format!("window_move_ok:{ox},{oy},{ow}x{oh}");
        log::info!("window_move: requested {x},{y} pt -> winit outer {ox},{oy} {ow}x{oh}");
    }

    /// Advance live MainMenu transitions (`WM::update` → transitions.update).
    ///
    /// C++ plays Menu FLASH/BUTTONFLASH groups at 60 fps
    /// (GameWindowTransitions.cpp `TransitionGroup::update` advances one frame
    /// per call), so the SP dropdown becomes hittable ~15 frames (~250 ms)
    /// after the ButtonSinglePlayer GBM_SELECTED. Windowed Menu frames take
    /// seconds, and `FlashTransition::update` frames 0-3 hold
    /// `winHide(TRUE)` on the active `dropDownWindows[]` border
    /// (MainMenu.cpp:1320 `winHide(FALSE)` loses that race), leaving
    /// ButtonSkirmish parent-hidden and the physical click falling through to
    /// MainMenuRuler. This command advances the same per-frame ticks the
    /// injected `winit_menu_nav` path already uses — it never touches
    /// playable_claim evidence.
    pub(super) fn runtime_host_cmd_tick_main_menu_transitions(
        &mut self,
        args: &HashMap<String, String>,
    ) {
        if self.runtime_host_headless {
            self.runtime_host_last_gameplay_cmd = "tick_transitions_fail_headless".into();
            return;
        }
        if self.current_state != GameState::Menu {
            self.runtime_host_last_gameplay_cmd =
                format!("tick_transitions_fail_not_menu:{:?}", self.current_state);
            return;
        }
        let times: u32 = args
            .get("times")
            .and_then(|s| s.parse().ok())
            .unwrap_or(16)
            .clamp(1, 64);
        #[cfg(feature = "game_client")]
        game_client::gui::tick_main_menu_transitions(times);
        self.runtime_host_last_gameplay_cmd = format!("tick_transitions_ok:{times}");
        log::info!("tick_main_menu_transitions: advanced {times} WM updates");
    }

    /// Commit the retail `UseAlternateMouse` option through the exact
    /// Options-menu path (C++ OptionsMenu.cpp:1157-1159 checkbox →
    /// (*pref)["UseAlternateMouse"] + GlobalData). In alternate mouse mode the
    /// physical RMB is the context-command button (C++ CommandXlat.cpp:3656:
    /// "right click is only actioned here if we're in alternate mouse mode"),
    /// which is the scheme the RMB order latch is authored against. The typed
    /// host bridge (`HostOptionsRequest::AlternateMouse`) applies it to Main's
    /// live input residence on the next `host_tick_options_bridge`.
    pub(super) fn runtime_host_cmd_alternate_mouse(&mut self, args: &HashMap<String, String>) {
        if self.runtime_host_headless {
            self.runtime_host_last_gameplay_cmd = "alternate_mouse_fail_headless".into();
            return;
        }
        let enabled = args
            .get("on")
            .map(|v| {
                !(v.eq_ignore_ascii_case("0")
                    || v.eq_ignore_ascii_case("false")
                    || v.eq_ignore_ascii_case("no")
                    || v.eq_ignore_ascii_case("off"))
            })
            .unwrap_or(true);
        #[cfg(feature = "game_client")]
        {
            game_client::gui::OptionsMenu::commit_alternate_mouse_setting(enabled);
        }
        self.runtime_host_last_gameplay_cmd = format!("alternate_mouse_ok:{enabled}");
        log::info!(
            "alternate_mouse: committed UseAlternateMouse={enabled} (host bridge applies next tick)"
        );
    }
}
