use super::*;

impl MainMenu {
    /// Accept resolution change
    /// Port of AcceptResolution() - C++ lines 704-710
    pub fn accept_resolution(&mut self) {
        let mut state = &mut self.state;
        state.old_disp_settings = state.new_disp_settings;
        state.disp_changed = false;
        log::info!(
            "Resolution change accepted: {}x{}",
            state.new_disp_settings.x_res,
            state.new_disp_settings.y_res
        );
    }

    /// Decline resolution change
    /// Port of DeclineResolution() - C++ lines 715-752
    pub fn decline_resolution(
        &mut self,
        effects: &mut WindowEventEffects<'_>,
    ) -> MainMenuResult<()> {
        let new_disp_settings = self.rollback_resolution_state(effects);

        if let Some(global) = get_global_data() {
            let mut global = global.write();
            global.x_resolution = new_disp_settings.x_res;
            global.y_resolution = new_disp_settings.y_res;
        }
        get_header_template_manager()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .header_notify_resolution_change();

        let mut prefs = UserPreferences::new();
        let _ = prefs.load("Options.ini");
        prefs.set_string(
            "Resolution",
            format!("{} {}", new_disp_settings.x_res, new_disp_settings.y_res),
        );
        let _ = prefs.write();

        // C++ DeclineResolution (MainMenu.cpp:739-750) deletes/recreates
        // TheShell then TheShell->push("Menus/MainMenu.wnd"). Rust
        // show_shell only pushes when shell map is OFF, so reset+show_shell
        // leaves an empty stack on the live GAME_SHELL menu (hq-90rl).
        effects
            .window_manager_owner
            .expect("resolution change requires receiving window owner")
            .queue_shell_operation_with_effects(|shell, effects| {
                let _ = shell.reset();
                if let Err(err) = crate::system::SubsystemInterface::init(shell) {
                    log::warn!("DeclineResolution Shell::init failed: {err}");
                    return;
                }
                if let Err(err) = shell.push("Menus/MainMenu.wnd", false, effects) {
                    log::warn!("DeclineResolution push MainMenu.wnd failed: {err}");
                    return;
                }
                shell.set_shell_active(true);
            });

        effects.request_menu_host(MenuHostRequest::ResetPlacement);

        log::info!(
            "Resolution change declined - reverted to: {}x{}",
            new_disp_settings.x_res,
            new_disp_settings.y_res
        );
        Ok(())
    }

    pub(super) fn rollback_resolution_state(
        &mut self,
        effects: &mut WindowEventEffects<'_>,
    ) -> DisplaySettings {
        let mut state = &mut self.state;

        // C++ DeclineResolution: TheDisplay->setDisplayMode(old settings).
        let old = state.old_disp_settings;
        let (display, mouse) = effects.script_display_and_mouse();
        let _ = crate::core::script_action_handler::apply_script_display_mode(
            display,
            old.x_res.max(0) as u32,
            old.y_res.max(0) as u32,
            old.bit_depth.max(0) as u32,
            old.windowed,
            mouse,
        );
        state.disp_changed = false;
        state.new_disp_settings = old;
        state.new_disp_settings
    }

    pub fn set_pending_resolution_change(
        &mut self,
        old_settings: DisplaySettings,
        new_settings: DisplaySettings,
    ) {
        let mut state = &mut self.state;
        state.old_disp_settings = old_settings;
        state.new_disp_settings = new_settings;
        state.disp_changed = true;
    }

    /// Show resolution dialog
    /// Port of DoResolutionDialog() - C++ lines 757-773
    pub fn do_resolution_dialog(&mut self, effects: &mut WindowEventEffects<'_>) {
        let (x_res, y_res) = {
            let state = &self.state;
            (state.new_disp_settings.x_res, state.new_disp_settings.y_res)
        };
        effects
            .pending_main_menu_actions
            .push((PendingMainMenuAction::ResolutionDialog(x_res, y_res), None));
    }

    pub(super) fn show_resolution_dialog(
        x_res: i32,
        y_res: i32,
        effects: &mut WindowEventEffects<'_>,
    ) {
        let title = crate::game_text::GameText::fetch("GUI:Resolution");
        let body = format!("{}: {}x{}", title, x_res, y_res);
        let ok: MessageBoxFunc = Box::new(|effects| {
            effects.with_main_menu(|menu, _| menu.accept_resolution());
        });
        let cancel: MessageBoxFunc = Box::new(|effects| {
            if let Err(err) =
                effects.with_main_menu(|menu, effects| menu.decline_resolution(effects))
            {
                log::warn!("DeclineResolution callback failed: {}", err);
            }
        });
        let _ = effects.with_manager(|manager, effects| {
            message_box_ok_cancel(&title, &body, Some(ok), Some(cancel), manager, effects)
        });
    }

    /// Handle canceled download
    /// Port of HandleCanceledDownload() - C++ lines 203-211
    pub(super) fn handle_canceled_download_state(
        state: &mut MainMenuState,
        reset_dropdown: bool,
        effects: &mut WindowEventEffects<'_>,
        mut selected: Option<&mut crate::gui::game_window::GameWindow>,
    ) {
        state.button_pushed = false;

        if reset_dropdown {
            Self::set_dropdown_hidden(
                state,
                DropdownType::Main,
                false,
                effects,
                selected.as_deref_mut(),
            );
            Self::transition_set_group(
                "MainMenuDefaultMenuLogoFade",
                false,
                effects,
                selected.as_deref_mut(),
            );
            log::info!("Download canceled - resetting dropdown");
        }
    }

    pub(super) fn cancel_patch_check_callback_state(state: &mut MainMenuState) {
        state.button_pushed = false;
        state.checking_for_patch_before_gamespy = false;
        state.cant_connect_before_online = false;
        state.checks_left_before_online = 0;
        state.online_cancel_window_open = false;
    }

    pub(super) fn finish_online_handoff_state(state: &mut MainMenuState) {
        #[cfg(feature = "online_ui")]
        if matches!(
            main_menu_online::start_online(state),
            main_menu_online::OnlineHandoff::NeedPatchDownload
        ) {
            Self::queue_action(state, PendingMainMenuAction::StartDownloadingPatches);
        }

        #[cfg(not(feature = "online_ui"))]
        let _ = main_menu_online::start_online(state);
    }

    pub(super) fn http_think_wrapper(state: &mut MainMenuState) {
        if !state.checking_for_patch_before_gamespy {
            return;
        }

        if state.cant_connect_before_online {
            Self::cancel_patch_check_callback_state(state);
            log::info!("Patch check ended because servserv was unreachable");
            return;
        }

        if state.checks_left_before_online > 0 {
            state.checks_left_before_online -= 1;
            if state.checks_left_before_online == 0 {
                Self::finish_online_handoff_state(state);
            }
        } else {
            Self::finish_online_handoff_state(state);
        }
    }

    pub fn handle_canceled_download(
        &mut self,
        reset_dropdown: bool,
        effects: &mut WindowEventEffects<'_>,
        selected: Option<&mut crate::gui::GameWindow>,
    ) {
        let mut state = &mut self.state;
        Self::handle_canceled_download_state(&mut state, reset_dropdown, effects, selected);
    }
}

pub(crate) fn build_window_ids() -> WindowIds {
    WindowIds {
        main_menu_id: NameKeyGenerator::name_to_key("MainMenu.wnd:MainMenuParent"),
        skirmish_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonSkirmish"),
        online_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonOnline"),
        network_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonNetwork"),
        options_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonOptions"),
        exit_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonExit"),
        motd_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonMOTD"),
        world_builder_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonWorldBuilder"),
        get_update_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonGetUpdate"),
        button_training_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonTRAINING"),
        button_challenge_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonChallenge"),
        button_usa_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonUSA"),
        button_gla_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonGLA"),
        button_china_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonChina"),
        button_usa_recent_save_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonUSARecentSave",
        ),
        button_usa_load_game_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonUSALoadGame"),
        button_gla_recent_save_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonGLARecentSave",
        ),
        button_gla_load_game_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonGLALoadGame"),
        button_china_recent_save_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonChinaRecentSave",
        ),
        button_china_load_game_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonChinaLoadGame",
        ),
        button_single_player_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonSinglePlayer"),
        button_multi_player_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonMultiplayer"),
        button_multi_back_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonMultiBack"),
        button_single_back_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonSingleBack"),
        button_load_replay_back_id: NameKeyGenerator::name_to_key(
            "MainMenu.wnd:ButtonLoadReplayBack",
        ),
        button_replay_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonReplay"),
        button_load_replay_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonLoadReplay"),
        button_load_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonLoadGame"),
        button_credits_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonCredits"),
        button_easy_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonEasy"),
        button_medium_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonMedium"),
        button_hard_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonHard"),
        button_diff_back_id: NameKeyGenerator::name_to_key("MainMenu.wnd:ButtonDiffBack"),
    }
}
