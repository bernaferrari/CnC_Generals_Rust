// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl ControlBar {
    /// Admit immutable UI definitions from the driving match before its input/
    /// frame operation. Cloned templates retain identity; no live game state
    /// is mirrored or discovered through a singleton.
    pub fn set_receiving_upgrade_definitions(
        &mut self,
        catalog: &gamelogic::upgrade::center::UpgradeCenter,
        actions: Option<
            Arc<game_engine::common::thing::receiving_module_content::ReceivingActionDefinitions>,
        >,
    ) {
        self.receiving_upgrade_catalog = Some(catalog.clone());
        self.receiving_upgrade_actions = actions;
    }

    pub fn new() -> Self {
        Self {
            receiving_upgrade_catalog: None,
            receiving_upgrade_actions: None,
            tooltip: Default::default(),
            context: ControlBarContext::default(),
            is_animating: false,
            animation_start_time: Instant::now(),
            animation_duration: Duration::from_millis(500),
            button_states: HashMap::new(),
            observer_mode: false,
            multi_select_mode: false,
            ui_dirty: false,
            pending_command_clicks: Vec::new(),
            build_queue_data: Vec::new(),
            displayed_queue_count: 0,
            current_frame: 0,
            flash_active: false,
            control_bar_stage: ControlBarStage::Default,
            portrait_state: PortraitDisplayState::default(),
            science_state: SciencePurchaseState::default(),
            gen_star_flash: true,
            last_flashed_at_point_value: -1,
            radar_attack_glow_on: false,
            remaining_radar_attack_glow_frames: 0,
            science_layout_loaded: false,
            rally_point_drawable_id: 0,
            default_control_bar_x: 0,
            default_control_bar_y: 0,
            default_control_bar_captured: false,
            special_power_shortcut_layout: String::new(),
            radar_glow_window_enabled: true,
            special_power_shortcuts: Vec::new(),
            special_power_shortcut_count: 0,
            presentation_radar_count: 0,
            presentation_radar_disabled: false,
            presentation_queued_upgrades: Vec::new(),
            presentation_primary_command_set: String::new(),
            presentation_command_set_names: Vec::new(),
            presentation_max_garrison: 0,
            presentation_garrisoned_count: 0,
            presentation_occupants: Vec::new(),
            presentation_under_construction: false,
            presentation_sold: false,
            presentation_selection_controllable: true,
            presentation_construction_percent: 0.0,
            presentation_ocl_timer_seconds: 0,
            displayed_construct_percent: -1.0,
            displayed_ocl_timer_seconds: 0,
            presentation_money: None,
            presentation_power: (0, 0),
            last_displayed_money: -1,
            presentation_can_make: Vec::new(),
            presentation_unlocked_sciences: Vec::new(),
            #[cfg(test)]
            last_science_transition_group: None,
            ingame_entry_context_applied: false,
            presentation_availability: PresentationAvailabilityResidual::default(),

            border_colors: CommandBarBorderColors::default(),
        }
    }

    /// C++ ControlBar::updateFlashButtons — pulse ready generals-star / command buttons.
    fn update_flash_buttons(
        &mut self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        self.update_star_image(receiving_manager);
    }

    // ---------------------------------------------------------------------------
    // markUIDirty / onDrawableSelected / onDrawableDeselected
    // C++ ControlBar.cpp:114-1617
    // ---------------------------------------------------------------------------

    /// Mark the UI dirty so context is re-evaluated on next update.
    /// C++: ControlBar::markUIDirty()
    pub fn mark_ui_dirty(&mut self) {
        self.ui_dirty = true;
    }

    /// Called when a drawable is selected. Cancels pending GUI commands.
    /// C++: ControlBar::onDrawableSelected() → TheInGameUI->setGUICommand(NULL)
    pub fn on_drawable_selected(&mut self, ui: &mut crate::core::subsystems::InGameUISubsystem) {
        self.mark_ui_dirty();
        ui.clear_pending_command();
        ui.clear_pending_special_power();
    }

    /// Called when a drawable is deselected.
    /// C++: ControlBar::onDrawableDeselected()
    pub fn on_drawable_deselected(
        &mut self,
        select_count: usize,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
    ) {
        self.mark_ui_dirty();
        if select_count == 0 {
            ui.clear_pending_command();
            ui.clear_pending_special_power();
        }
        ui.place_build_available(None, None);
    }

    /// C++ InGameUI::areSelectedObjectsControllable residual from live host.
    pub fn sync_selection_controllable_from_presentation(&mut self, controllable: bool) {
        self.presentation_selection_controllable = controllable;
    }

    // ---------------------------------------------------------------------------
    // update - main per-frame update
    // C++ ControlBar.cpp:1359-1580
    // ---------------------------------------------------------------------------

    /// Main update loop. Mirrors C++ ControlBar::update().
    pub fn update(
        &mut self,
        delta_time: Duration,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // ShowControlBar queues DEFAULT (ControlBarCallback.cpp:489) before this
        // live instance ticks. Apply it so CommandWindow HIDDEN is cleared InGame.
        self.apply_pending_control_bar_stage(receiving_manager, effects);
        self.apply_live_hook_events(true, crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone, receiving_manager, effects);
        self.update_body(delta_time, true, None, crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone, None, receiving_manager, effects)
    }

    /// Native viewport/stage changes are already admitted on this instance.
    /// Unmigrated HUD clicks, science and shortcut events retain their explicit
    /// compatibility intake until their receiving-owner migration is complete.
    pub fn update_owned(
        &mut self,
        delta_time: Duration,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.apply_live_hook_events(false, crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone, receiving_manager, effects);
        self.update_body(delta_time, false, None, crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone, None, receiving_manager, effects)
    }

    pub fn update_owned_with_observers(
        &mut self,
        delta_time: Duration,
        roster: &super::control_bar_observer::NativeObserverRoster,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.apply_live_hook_events(false, crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone, receiving_manager, effects);
        self.update_body(delta_time, false, Some(roster), crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone, None, receiving_manager, effects)?;
        if self.observer_mode {
            // Legacy art/stat refresh may enumerate its compatibility players.
            // Reassert only native selectable identities from the receiving map.
            self.refresh_observer_windows_owned(roster, receiving_manager);
        }
        Ok(())
    }
    pub fn update_owned_receiving(
        &mut self,
        delta_time: Duration,
        roster: Option<&super::control_bar_observer::NativeObserverRoster>,
        upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'_>,
        game_ending: bool,
        window_manager_owner: &crate::gui::window_manager::WindowManagerOwner,
        videos: &mut crate::gui::window_video_manager::WindowVideoManager,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.apply_live_hook_events_receiving(
            false,
            upgrade_source,
            game_ending,
            Some(window_manager_owner),
            receiving_manager,
            effects,
        );
        crate::gui::w3d_gadget_draw::ensure_control_bar_wnd_draw_callbacks_receiving(
            receiving_manager,
        );
        self.update_body(
            delta_time,
            false,
            roster,
            upgrade_source,
            Some(videos),
            receiving_manager,
            effects,
        )?;
        if self.observer_mode {
            if let Some(roster) = roster {
                self.refresh_observer_windows_owned(roster, receiving_manager);
            }
        }
        Ok(())
    }
    fn update_body(
        &mut self,
        delta_time: Duration,
        publish_standalone: bool,
        native_roster: Option<&super::control_bar_observer::NativeObserverRoster>,
        upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'_>,
        videos: Option<&mut crate::gui::window_video_manager::WindowVideoManager>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::w3d_gadget_draw::ensure_scheme_draw_registered();
        if publish_standalone {
            crate::gui::w3d_gadget_draw::ensure_control_bar_wnd_draw_callbacks_receiving(
                receiving_manager,
            );
        }
        super::control_bar_observer::init_observer_controls(receiving_manager);
        super::control_bar_observer::ensure_gen_arrow_from_mapped_images(receiving_manager);

        // C++ ControlBar::init ends with switchToContext(CB_CONTEXT_NONE)
        // (ControlBar.cpp:1262) and resetControlBar re-runs it (1304), so the
        // authored-visible ObserverPlayerInfoWindow (ControlBar.wnd:1435 has no
        // HIDDEN flag) can never leak into normal play. The port only switched
        // contexts on ui_dirty; force one entry evaluation once the
        // ControlBar.wnd layout is live so parity does not depend on a host
        // event arriving first.
        if !self.ingame_entry_context_applied
            && leftover_find_window(CONTROL_BAR_PARENT, receiving_manager).is_some()
        {
            self.ingame_entry_context_applied = true;
            // C++ initControlBarObserver (ControlBar.cpp:2741) puts observer
            // bars straight into CB_CONTEXT_OBSERVER_LIST; everyone else gets
            // the CB_CONTEXT_NONE hide matrix via evaluateContextUI.
            if self.observer_mode {
                self.switch_to_context(
                    ControlBarState::Observer,
                    None,
                    receiving_manager,
                    effects,
                )?;
            } else {
                self.evaluate_context_ui(receiving_manager, effects)?;
            }
        }
        if self.is_animating {
            let elapsed = self.animation_start_time.elapsed();
            if elapsed >= self.animation_duration {
                self.is_animating = false;
            }
        }

        let current_time = Instant::now();
        for (_, state) in self.button_states.iter_mut() {
            if let Some(flash_time) = state.flash_time {
                if current_time.duration_since(flash_time) > Duration::from_millis(500) {
                    state.flash_time = None;
                }
            }
        }

        self.current_frame =
            native_roster.map_or_else(TheGameLogic::get_frame, |roster| roster.frame);
        self.update_star_image(receiving_manager);
        self.update_radar_attack_glow(receiving_manager);
        if let Some(videos) = videos {
            videos.update();
        }
        // C++ InGameUI.cpp:1776-1815 writes MoneyDisplay / PowerWindow hide
        // immediately before TheControlBar->update(). Live host ticks ControlBar
        // from presentation apply, so this is the live InGameUI money path.
        self.update_money_and_power_windows_with_target(
            native_roster.map(|_| self.observer_target_index_owned()),
            receiving_manager,
        );

        if self.observer_mode {
            self.update_observer_portrait_preserving_target(
                native_roster.is_some(),
                receiving_manager,
            )?;
            return Ok(());
        }

        if self.science_state.is_visible {
            if self.ui_dirty {
                self.populate_purchase_science(receiving_manager);
                self.leftover_apply_purchase_science_windows(receiving_manager);
            }
            self.update_context_purchase_science(receiving_manager);
        }

        self.update_flash_buttons(receiving_manager);

        if self.ui_dirty {
            self.evaluate_context_ui(receiving_manager, effects)?;
            self.populate_special_power_shortcut_with_upgrade_source(
                upgrade_source,
                receiving_manager,
            )?;
            self.ui_dirty = false;
        }

        self.update_place_beacon_button_enabled(receiving_manager);

        let context = &self.context;
        let current_state = context.current_state;
        let selected_objects = context.selected_objects.clone();
        let player_id = context.player_id;

        if current_state == ControlBarState::MultiSelect {
            self.update_context_multi_select(receiving_manager)?;
            return Ok(());
        }

        if selected_objects.is_empty() {
            return Ok(());
        }

        let Some(&first_id) = selected_objects.first() else {
            return Ok(());
        };
        // Host/presentation path: Main feeds selection via
        // sync_selection_display_from_presentation (no OBJECT_REGISTRY).
        // Dual-world registry is opt-in; do not wipe context when registry empty.
        let registry_exists = OBJECT_REGISTRY
            .get_object(first_id)
            .map(|arc| arc.read().is_ok())
            .unwrap_or(false);
        let presentation_selection_active =
            self.portrait_state.is_visible && self.portrait_state.selected_count > 0;
        if !registry_exists && !presentation_selection_active {
            self.switch_to_context(ControlBarState::None, None, receiving_manager, effects)?;
            return Ok(());
        }
        // Host/presentation still runs command/queue/inventory updates.
        if !registry_exists {
            match current_state {
                ControlBarState::Command => {
                    self.update_context_command(receiving_manager, effects)?;
                }
                // C++ updateContextUnderConstruction / updateContextOCLTimer
                // (ControlBarUnderConstruction.cpp:82-99, ControlBar.cpp:1563-1574)
                // run off presentation residuals too: the OCL timer window
                // must reveal and tick, and the under-construction label must
                // update, without dual-world registry modules.
                ControlBarState::UnderConstruction => {
                    self.update_context_under_construction(delta_time, receiving_manager)?;
                }
                ControlBarState::OclTimer => {
                    self.update_context_ocl_timer(delta_time, receiving_manager)?;
                }
                _ => {}
            }
            return Ok(());
        }

        match current_state {
            ControlBarState::None => {}
            ControlBarState::Command => {
                self.update_context_command(receiving_manager, effects)?;
            }
            ControlBarState::StructureInventory => {
                self.update_context_structure_inventory(
                    effects.ui.as_deref_mut().expect("receiving UI"),
                )?;
            }
            ControlBarState::Beacon => {
                self.update_context_beacon(receiving_manager)?;
            }
            ControlBarState::UnderConstruction => {
                self.update_context_under_construction(delta_time, receiving_manager)?;
            }
            ControlBarState::OclTimer => {
                self.update_context_ocl_timer(delta_time, receiving_manager)?;
            }
            ControlBarState::Observer => {
                self.update_context_observer_preserving_target(
                    native_roster.is_some(),
                    receiving_manager,
                )?;
            }
            ControlBarState::MultiSelect => {
                self.update_context_multi_select(receiving_manager)?;
            }
        }

        {
            let mut context = &mut self.context;
            for item in context.construction_queue.iter_mut() {
                if item.progress < 1.0 && item.build_time > 0.0 {
                    item.progress += delta_time.as_secs_f32() / item.build_time;
                    item.progress = item.progress.min(1.0);
                }
            }
        }

        let context_snapshot = self.context.clone();
        self.refresh_button_states(&context_snapshot, player_id);

        self.update_special_power_shortcut_availability(receiving_manager);

        let observer_target = if publish_standalone {
            self.get_observer_look_at_player_index()
        } else {
            self.observer_target_index_owned()
        };
        set_live_control_bar_observer_look_at(observer_target, effects.ui_mut());

        Ok(())
    }

    /// C++ InGameUI::update money/power window residual (InGameUI.cpp:1776-1815).
    ///
    /// Formats `GUI:ControlBarMoneyDisplay` onto `ControlBar.wnd:MoneyDisplay`
    /// from ThePlayerList (observer look-at or local). PowerWindow stays visible
    /// whenever money does (C++ 1808-1809) and receives produced/consumed text
    /// from crate Energy, or the host presentation freeze when ThePlayerList
    /// has no local player.
    ///
    /// Live host: crate `ThePlayerList` is often empty even in a real offline
    /// match, so the frozen host presentation supplies the local player's cash.
    /// Keep that input separate from the last value actually written to a WND.
    pub fn update_money_and_power_windows(
        &mut self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        self.update_money_and_power_windows_with_target(None, receiving_manager);
    }
    fn update_money_and_power_windows_with_target(
        &mut self,
        native_target: Option<Option<i32>>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        let money_player = if self.observer_mode {
            native_target
                .unwrap_or_else(|| self.get_observer_look_at_player_index())
                .and_then(|idx| {
                    logic_player_list()
                        .read()
                        .ok()
                        .and_then(|list| list.get_player(idx as PlayerIndex).cloned())
                })
        } else {
            logic_player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned())
        };

        let crate_money = money_player
            .as_ref()
            .and_then(|player| player.read().ok())
            .map(|player| player.get_money().count_money() as i32);

        let crate_power = money_player.as_ref().and_then(|player| {
            player.read().ok().map(|guard| {
                let energy = guard.get_energy();
                (energy.production(), energy.consumption())
            })
        });

        (|receiving_manager: &mut crate::gui::window_manager::WindowManager| {
            let money_win = receiving_manager.find_window_by_name("ControlBar.wnd:MoneyDisplay");
            let power_win = receiving_manager.find_window_by_name("ControlBar.wnd:PowerWindow");
            // The host freeze stands in for C++ moneyPlayer while the legacy
            // PlayerList is empty. A recreated WND can still contain its
            // authored $$$ even when the cash amount has not changed.
            let money = crate_money.or(self.presentation_money);
            if let Some(win) = money_win.as_ref() {
                if let Some(money) = money {
                    let text = Self::format_control_bar_money_display(money);
                    if self.last_displayed_money != money || win.borrow().get_text() != text {
                        if win.borrow_mut().set_text(&text).is_ok() {
                            self.last_displayed_money = money;
                        }
                    }
                }
                let _ = win.borrow_mut().hide(money.is_none(), receiving_manager);
            }
            if let Some(win) = power_win.as_ref() {
                let presentation = self.presentation_power;
                let (produced, consumed) = match crate_power {
                    Some((produced, consumed))
                        if produced != 0 || consumed != 0 || presentation == (0, 0) =>
                    {
                        (produced, consumed)
                    }
                    _ => presentation,
                };
                let text = Self::format_control_bar_power_display(produced, consumed);
                let _ = win.borrow_mut().set_text(&text);
                let _ = win.borrow_mut().hide(money.is_none(), receiving_manager);
            }
        })(receiving_manager);
    }

    /// Supply the current host presentation cash for the next WND update.
    ///
    /// `PresentationFrame::apply_to_control_bar` / host tick can call this
    /// before `update()` so MoneyDisplay shows freeze `local_supplies` when
    /// crate `ThePlayerList` has no local player. This does not mark a WND as
    /// rendered, since it may not exist yet.
    pub fn apply_presentation_money(&mut self, supplies: i32) {
        self.presentation_money = Some(supplies.max(0));
    }

    /// Stamp host presentation CanMake residual for leftover WND availability.
    ///
    /// C++ ControlBarCommand.cpp:1185-1198 — canBuild / canMakeUnit / money
    /// all become COMMAND_RESTRICTED (gray). Live host has no OBJECT_REGISTRY
    /// so get_command_availability looks this map up by command.object.
    pub fn apply_presentation_can_make(&mut self, cameos: &[(String, u32)]) {
        self.presentation_can_make = cameos.to_vec();
    }

    /// The live host supplies this game's local player sciences.
    pub fn apply_presentation_science_hide_names(&mut self, names: &[String]) {
        self.presentation_unlocked_sciences = names.to_vec();
    }

    pub fn presentation_can_make(&self) -> &[(String, u32)] {
        &self.presentation_can_make
    }

    /// Frozen host CanMake input, copied into the bound window command payload.
    fn presentation_can_make_status(&self, template: &str) -> Option<u32> {
        self.presentation_can_make
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(template))
            .map(|(_, status)| *status)
    }

    /// Supply this bar's frozen player Energy for its next PowerWindow update.
    /// C++ W3DControlBar.cpp:90-126 reads the local/observer player's Energy.
    pub fn apply_presentation_power(&mut self, produced: i32, consumed: i32) {
        self.presentation_power = (produced.max(0), consumed.max(0));
    }

    /// Host-testable money format residual (C++ `GUI:ControlBarMoneyDisplay` %d).
    pub fn format_control_bar_money_display(money: i32) -> String {
        let template = GameText::fetch("GUI:ControlBarMoneyDisplay");
        if template.contains("%d") {
            template.replace("%d", &money.to_string())
        } else if template.is_empty() || template.starts_with("GUI:") {
            format!("${money}")
        } else {
            format!("{template} {money}")
        }
    }

    /// Host-testable PowerWindow text (produced / consumed).
    ///
    /// C++ PowerWindow is a W3D fill, not a static-text gadget. Live crate
    /// Energy is often 0, so the window shows the host freeze numbers.
    pub fn format_control_bar_power_display(produced: i32, consumed: i32) -> String {
        format!("{produced} / {consumed}")
    }

    fn apply_live_hook_events(
        &mut self,
        standalone_viewport: bool,
        upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'_>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.apply_live_hook_events_receiving(
            standalone_viewport,
            upgrade_source,
            gamelogic::helpers::TheScriptEngine::is_game_ending(),
            None,
            receiving_manager,
            effects,
        );
    }
    /// Finish command continuations immediately after the WND loan, before input returns.
    pub fn replay_pending_command_clicks_receiving(
        &mut self,
        owner: &crate::gui::window_manager::WindowManagerOwner,
        messages: &mut Vec<crate::message_stream::game_message::GameMessage>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
    ) {
        for click in std::mem::take(&mut self.pending_command_clicks) {
            self.process_context_sensitive_button_click(
                click,
                owner,
                messages,
                receiving_manager,
                ui,
            );
        }
    }

    pub fn discard_pending_command_clicks(&mut self) {
        self.pending_command_clicks.clear();
    }

    fn apply_live_hook_events_receiving(
        &mut self,
        standalone_viewport: bool,
        upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'_>,
        game_ending: bool,
        window_manager_owner: Option<&crate::gui::window_manager::WindowManagerOwner>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        let events = drain_live_control_bar_events(effects.ui_mut());
        let clicks = if window_manager_owner.is_some() {
            std::mem::take(&mut self.pending_command_clicks)
        } else {
            Vec::new()
        };
        if events.ui_dirty {
            self.mark_ui_dirty();
        }
        if events.hide_purchase_science {
            self.hide_purchase_science(receiving_manager);
        }
        if events.toggle_purchase_science {
            self.toggle_purchase_science_receiving(game_ending, receiving_manager, effects);
        }
        if events.show_special_power_shortcut {
            self.show_special_power_shortcut_receiving(
                upgrade_source,
                game_ending,
                receiving_manager,
                effects,
            );
        }
        if events.hide_special_power_shortcut {
            self.hide_special_power_shortcut(receiving_manager);
        }
        if let Some(enabled) = events.animate_special_power_shortcut {
            self.animate_special_power_shortcut(enabled);
        }
        if standalone_viewport && events.toggle_control_bar_stage {
            self.toggle_control_bar_stage(receiving_manager, effects);
        }
        if events.init_special_power_shortcut_for_player.is_some() {
            self.init_special_power_shortcut_bar(receiving_manager, effects);
        }
        if standalone_viewport && events.set_scheme_by_player.is_some() {
            self.set_control_bar_scheme_by_player(receiving_manager, effects);
        }
        if let Some(window_manager_owner) = window_manager_owner {
            for click in clicks {
                // Replay the receiving bar's owned click at the original update phase.
                self.process_context_sensitive_button_click(
                    click,
                    window_manager_owner,
                    effects.emitted_messages,
                    receiving_manager,
                    effects.ui.as_deref_mut().expect("receiving UI"),
                );
            }
        }
        let _ = events.transitions;
    }

    /// C++ InGameUI.cpp:4116 — first selected drawable's isLocallyControlled.
    fn first_selected_is_controllable(&self, obj_id: u32) -> bool {
        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
            if let Ok(obj) = obj_arc.read() {
                return obj.is_locally_controlled();
            }
        }
        self.presentation_selection_controllable
    }

    fn first_selected_is_beacon(&self, obj_id: u32) -> bool {
        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
            if let Ok(obj) = obj_arc.read() {
                if Self::presentation_name_is_beacon(obj.get_template_name())
                    || Self::presentation_name_is_beacon(obj.get_command_set_string())
                {
                    return true;
                }
            }
        }
        if Self::presentation_name_is_beacon(&self.presentation_primary_command_set)
            || Self::presentation_name_is_beacon(&self.portrait_state.portrait_image)
        {
            return true;
        }
        crate::presentation_translator_residual::translator_catalog_entry(obj_id)
            .map(|entry| {
                Self::presentation_name_is_beacon(&entry.template_name)
                    || Self::presentation_name_is_beacon(&entry.command_set_name)
            })
            .unwrap_or(false)
    }

    /// C++ evaluateContextUI: only NEUTRAL garrisonable containers peek past NONE.
    fn non_controllable_neutral_garrison_peek(&self, obj_id: u32) -> bool {
        if let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) {
            if let Ok(obj) = obj_arc.read() {
                let Some(contain) = obj.get_contain() else {
                    return false;
                };
                let Ok(contain) = contain.lock() else {
                    return false;
                };
                if contain.get_contain_max() <= 0 || !contain.is_garrisonable() {
                    return false;
                }
                drop(contain);
                let player_list = logic_player_list();
                let local_index = player_list
                    .read()
                    .ok()
                    .map(|list| list.get_local_player_index())
                    .unwrap_or(gamelogic::player::PLAYER_INDEX_INVALID);
                let obj_player_id =
                    obj.get_controlling_player_id().unwrap_or(0xFFFF) as PlayerIndex;
                let local_arc = player_list
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(local_index).cloned());
                let obj_player = player_list
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(obj_player_id).cloned());
                if let (Some(local_arc), Some(obj_player)) = (local_arc, obj_player) {
                    if let (Ok(local_guard), Ok(obj_guard)) = (local_arc.read(), obj_player.read())
                    {
                        return local_guard.get_relationship(&obj_guard)
                            == gamelogic::common::Relationship::Neutral;
                    }
                }
                return false;
            }
        }
        let catalog = crate::presentation_translator_residual::translator_catalog_entry(obj_id);
        let garrisonable = self.presentation_max_garrison > 0
            || catalog
                .as_ref()
                .map(|entry| entry.max_garrison > 0)
                .unwrap_or(false);
        if !garrisonable {
            return false;
        }
        catalog
            .map(|entry| entry.team_name.eq_ignore_ascii_case("Neutral"))
            .unwrap_or(false)
    }

    // ---------------------------------------------------------------------------
    // evaluateContextUI - determine what context to show
    // C++ ControlBar.cpp:1689-1888
    // ---------------------------------------------------------------------------

    fn evaluate_context_ui(
        &mut self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.ui_dirty = false;
        if self.science_state.is_visible
            || !leftover_window_is_hidden(GEN_EXP_PARENT, receiving_manager)
        {
            self.show_purchase_science(receiving_manager, effects);
        }

        let mut context = std::mem::take(&mut self.context);

        if context.selected_objects.is_empty() {
            context.current_state = ControlBarState::None;
            context.available_commands.clear();
            context.construction_queue.clear();
            self.build_queue_data.clear();
            self.displayed_queue_count = 0;
            self.portrait_state = PortraitDisplayState::default();
            // C++ switchToContext(CB_CONTEXT_NONE) hides CP_COMMAND plus every
            // other context parent (ControlBar.cpp:2128-2137). Retail shows no
            // command grid and no tab labels until a real context owns them —
            // never a DEFAULT-stage reveal of all 14 authored slots.
            apply_none_context_hide(receiving_manager);
            self.context = context;
            return Ok(());
        }

        // C++ ControlBar.cpp:1716-1778 — non-local selection is CB_CONTEXT_NONE
        // except controlling-player beacon template and NEUTRAL garrisonable peek.
        if let Some(&first_id) = context.selected_objects.first() {
            if !self.first_selected_is_controllable(first_id) {
                let peek_neutral_garrison = self.non_controllable_neutral_garrison_peek(first_id);
                let is_beacon = self.first_selected_is_beacon(first_id);
                if is_beacon {
                    context.current_state = ControlBarState::Beacon;
                } else {
                    context.current_state = ControlBarState::None;
                    context.available_commands.clear();
                    context.construction_queue.clear();
                    self.build_queue_data.clear();
                    self.displayed_queue_count = 0;
                    self.presentation_primary_command_set.clear();
                    self.presentation_command_set_names.clear();
                }
                if !peek_neutral_garrison {
                    if is_beacon {
                        self.rebuild_command_buttons(&mut context, receiving_manager)?;
                    }
                    self.context = context;
                    return Ok(());
                }
            }
        }

        let multi_select = context.selected_objects.len() > 1;
        let single_drawable_id = if multi_select {
            None
        } else {
            context.selected_objects.first().copied()
        };

        if multi_select {
            context.current_state = ControlBarState::MultiSelect;
            self.rebuild_command_buttons(&mut context, receiving_manager)?;
            self.context = context;
            return Ok(());
        }

        let Some(obj_id) = single_drawable_id else {
            context.current_state = ControlBarState::None;
            self.context = context;
            return Ok(());
        };

        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            // Presentation-only selection residual (host path, no dual-world registry).
            // Wave 1033/1034: C++ OBJECT_STATUS_SOLD / UNSELECTABLE residual — clear bar.
            // Wave 1070: destroyed/masked residual also clears dual-world ControlBar.
            let catalog_entry =
                crate::presentation_translator_residual::translator_catalog_entry(obj_id);
            let catalog_sold =
                self.presentation_sold || catalog_entry.as_ref().map(|e| e.sold).unwrap_or(false);
            // Wave 1034: unselectable residual also clears dual-world ControlBar.
            let catalog_unselectable = catalog_entry
                .as_ref()
                .map(|e| e.unselectable)
                .unwrap_or(false);
            let catalog_destroyed = catalog_entry.as_ref().map(|e| e.destroyed).unwrap_or(false);
            let catalog_masked = catalog_entry.as_ref().map(|e| e.masked).unwrap_or(false);
            if catalog_sold || catalog_unselectable || catalog_destroyed || catalog_masked {
                context.current_state = ControlBarState::None;
                context.available_commands.clear();
                context.construction_queue.clear();
                self.build_queue_data.clear();
                self.displayed_queue_count = 0;
                // Wave 1078: clear OCL/garrison dual residuals with unusable selection.
                // Wave 1079: also clear primary command-set residual (beacon/command).
                // Wave 1080: clear under-construction dual residual with unusable selection.
                self.presentation_ocl_timer_seconds = 0;
                self.displayed_ocl_timer_seconds = 0;
                self.presentation_max_garrison = 0;
                self.presentation_garrisoned_count = 0;
                self.presentation_primary_command_set.clear();
                self.presentation_under_construction = false;
                self.presentation_construction_percent = 0.0;
                self.context = context;
                return Ok(());
            }
            // Wave 1028: seed under-construction residual from catalog when freeze unset.
            if !self.presentation_under_construction {
                if let Some(entry) =
                    crate::presentation_translator_residual::translator_catalog_entry(obj_id)
                {
                    // Wave 1080: skip UC/garrison seed for unusable dual catalog entries.
                    let usable = !entry.destroyed
                        && !entry.sold
                        && !entry.disabled
                        && !entry.unselectable
                        && !entry.masked;
                    if usable && entry.under_construction {
                        self.presentation_under_construction = true;
                        self.presentation_construction_percent = entry.construction_percent;
                    }
                    // Wave 1030: seed garrison residual from catalog when freeze unset.
                    if usable && self.presentation_max_garrison == 0 && entry.max_garrison > 0 {
                        self.presentation_max_garrison = entry.max_garrison as usize;
                        self.presentation_garrisoned_count = entry.occupant_count as usize;
                    }
                    // Wave 1031: seed OCL timer residual from catalog.
                    // Wave 1078: skip OCL seed for unusable dual catalog entries.
                    if !entry.destroyed
                        && !entry.sold
                        && !entry.disabled
                        && !entry.unselectable
                        && !entry.masked
                        && self.presentation_ocl_timer_seconds == 0
                        && entry.ocl_timer_seconds > 0
                    {
                        self.presentation_ocl_timer_seconds = entry.ocl_timer_seconds;
                    }
                }
            } else if self.presentation_max_garrison == 0 {
                if let Some(entry) =
                    crate::presentation_translator_residual::translator_catalog_entry(obj_id)
                {
                    // Wave 1081: skip inventory seed for unusable dual catalog entries.
                    if !entry.destroyed
                        && !entry.sold
                        && !entry.disabled
                        && !entry.unselectable
                        && !entry.masked
                        && entry.max_garrison > 0
                    {
                        self.presentation_max_garrison = entry.max_garrison as usize;
                        self.presentation_garrisoned_count = entry.occupant_count as usize;
                    }
                }
            }
            if self.presentation_under_construction {
                context.current_state = ControlBarState::UnderConstruction;
            } else if self.presentation_ocl_timer_seconds > 0 {
                // Wave 1031: C++ CB_CONTEXT_OCL_TIMER residual (after UC, before inventory/command).
                context.current_state = ControlBarState::OclTimer;
            } else if self.presentation_max_garrison > 0
                && self.presentation_primary_command_set.is_empty()
            {
                context.current_state = ControlBarState::StructureInventory;
            } else if self.portrait_state.is_visible
                || !self.presentation_primary_command_set.is_empty()
            {
                // Wave 1032: beacon residual wins over generic Command when freeze says BEACON.
                if Self::presentation_name_is_beacon(&self.presentation_primary_command_set)
                    || Self::presentation_name_is_beacon(&self.portrait_state.portrait_image)
                {
                    context.current_state = ControlBarState::Beacon;
                } else {
                    context.current_state = ControlBarState::Command;
                }
            } else if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(obj_id)
            {
                // Wave 1027/1032: catalog residual when presentation freezes not yet stamped.
                // Wave 1079: unusable dual catalog residual fail-closed for beacon/command.
                if entry.destroyed
                    || entry.sold
                    || entry.disabled
                    || entry.unselectable
                    || entry.masked
                {
                    context.current_state = ControlBarState::None;
                } else if Self::presentation_name_is_beacon(&entry.template_name)
                    || Self::presentation_name_is_beacon(&entry.command_set_name)
                {
                    context.current_state = ControlBarState::Beacon;
                    if self.presentation_primary_command_set.is_empty()
                        && !entry.command_set_name.is_empty()
                    {
                        self.presentation_primary_command_set = entry.command_set_name.clone();
                    }
                } else if !entry.command_set_name.is_empty() || entry.selectable {
                    context.current_state = ControlBarState::Command;
                    if self.presentation_primary_command_set.is_empty()
                        && !entry.command_set_name.is_empty()
                    {
                        self.presentation_primary_command_set = entry.command_set_name.clone();
                    }
                } else {
                    context.current_state = ControlBarState::None;
                }
            } else {
                context.current_state = ControlBarState::None;
            }
            if matches!(
                context.current_state,
                ControlBarState::Command
                    | ControlBarState::MultiSelect
                    | ControlBarState::StructureInventory
            ) {
                reveal_ingame_command_window(receiving_manager);
            }
            if context.current_state != ControlBarState::None {
                self.rebuild_command_buttons(&mut context, receiving_manager)?;
            }
            self.context = context;
            return Ok(());
        };
        let Ok(obj) = obj_arc.read() else {
            context.current_state = ControlBarState::None;
            self.context = context;
            return Ok(());
        };

        if obj.test_status(OBJECT_STATUS_SOLD) {
            drop(obj);
            context.current_state = ControlBarState::None;
            self.context = context;
            return Ok(());
        }

        let under_construction = obj.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION);

        if under_construction {
            drop(obj);
            context.current_state = ControlBarState::UnderConstruction;
        } else if self.presentation_ocl_timer_seconds > 0 {
            // Wave 1031: presentation/host OCL timer residual (C++ OCLUpdate module path).
            drop(obj);
            context.current_state = ControlBarState::OclTimer;
        } else {
            let has_command_set = !obj.get_command_set_string().is_empty();

            let has_garrisonable_contain = obj
                .get_contain()
                .and_then(|contain| contain.lock().ok().map(|c| c.is_displayed_on_control_bar()))
                .unwrap_or(false);

            if has_garrisonable_contain && !has_command_set {
                drop(obj);
                context.current_state = ControlBarState::StructureInventory;
            } else if has_command_set {
                // Wave 1032: C++ beacon template residual before generic Command.
                let template_name = obj.get_template_name().to_string();
                let cmd_set = obj.get_command_set_string().to_string();
                drop(obj);
                if Self::presentation_name_is_beacon(&template_name)
                    || Self::presentation_name_is_beacon(&cmd_set)
                {
                    context.current_state = ControlBarState::Beacon;
                } else {
                    context.current_state = ControlBarState::Command;
                }
            } else {
                let template_name = obj.get_template_name().to_string();
                drop(obj);
                // Wave 1032: C++ CB_CONTEXT_BEACON when template matches beacon (no command set).
                if Self::presentation_name_is_beacon(&template_name) {
                    context.current_state = ControlBarState::Beacon;
                } else {
                    context.current_state = ControlBarState::None;
                }
            }
        }

        self.build_queue_data.clear();
        self.displayed_queue_count = 0;
        self.update_portrait_for_object(obj_id);

        if matches!(
            context.current_state,
            ControlBarState::Command
                | ControlBarState::MultiSelect
                | ControlBarState::StructureInventory
        ) {
            reveal_ingame_command_window(receiving_manager);
        }

        self.rebuild_command_buttons(&mut context, receiving_manager)?;

        if context.current_state == ControlBarState::Command {
            self.populate_build_queue(&mut context, obj_id, receiving_manager, effects)?;
        }

        self.context = context;

        Ok(())
    }
}

impl ControlBar {
    pub fn update_build_tooltip_layout_owned(
        &mut self,
        game_ending: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        crate::gui::gui_callbacks::control_bar_popup_description::update_build_tooltip_layout_owned(
            &mut self.tooltip,
            game_ending,
            receiving_manager,
        );
    }

    /// Delete exactly this bar's retained layout/animation, never a TLS peer.
    pub fn delete_build_tooltip_layout_owned(
        &mut self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        crate::gui::gui_callbacks::control_bar_popup_description::delete_build_tooltip_layout_owned(
            &mut self.tooltip,
            receiving_manager,
        );
    }
}
