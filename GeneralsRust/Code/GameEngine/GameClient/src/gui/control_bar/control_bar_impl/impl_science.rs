// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl ControlBar {
    // ---------------------------------------------------------------------------
    // Science purchase system
    // C++ ControlBar.cpp:143-485, 2907-2966
    // ---------------------------------------------------------------------------

    pub fn show_purchase_science(&mut self) {
        self.populate_purchase_science();
        self.gen_star_flash = false;
        self.science_state.is_visible = true;
    }

    pub fn hide_purchase_science(&mut self) {
        self.science_state.is_visible = false;
    }

    pub fn toggle_purchase_science(&mut self) {
        if self.science_state.is_visible {
            self.hide_purchase_science();
        } else {
            self.show_purchase_science();
        }
    }

    fn populate_purchase_science(&mut self) {
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else { return };
        let Ok(player) = player_arc.read() else {
            return;
        };

        let Some(store) = get_science_store() else {
            return;
        };

        self.science_state.available_points = player.get_science_purchase_points();
        self.science_state.rank_level = player.get_rank_level();
        self.science_state.experience_progress = 0.0;
        self.science_state.rank_title_label = format!("SCIENCE:Rank{}", player.get_rank_level());

        self.science_state.rank1_buttons.clear();
        self.science_state.rank3_buttons.clear();
        self.science_state.rank8_buttons.clear();
        self.update_context_purchase_science();
    }

    fn update_context_purchase_science(&mut self) {
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else { return };
        let Ok(player) = player_arc.read() else {
            return;
        };

        self.science_state.available_points = player.get_science_purchase_points();
    }

    /// Feed unlocked science names from PresentationFrame into purchase UI residual.
    ///
    /// Prefer this over live player-list / OBJECT_REGISTRY for dual-tick host paths.
    /// Marks matching rank buttons purchased; stores the full unlocked name list.
    /// Fail-closed: does not claim full ScienceStore / rank-point parity.
    pub fn sync_sciences_from_presentation(&mut self, unlocked_sciences: &[String]) {
        let mut unlocked: Vec<String> = unlocked_sciences.to_vec();
        unlocked.sort();
        unlocked.dedup();
        self.science_state.unlocked_sciences = unlocked.clone();

        let mark = |buttons: &mut Vec<ScienceButtonState>| {
            for btn in buttons.iter_mut() {
                let purchased = unlocked.iter().any(|name| {
                    name.eq_ignore_ascii_case(&btn.command_name)
                        || (!btn.command_name.is_empty()
                            && name
                                .to_ascii_lowercase()
                                .contains(&btn.command_name.to_ascii_lowercase()))
                        || name.eq_ignore_ascii_case(&format!("{:?}", btn.science_type))
                });
                if purchased {
                    btn.is_purchased = true;
                    btn.is_enabled = false;
                }
            }
        };
        mark(&mut self.science_state.rank1_buttons);
        mark(&mut self.science_state.rank3_buttons);
        mark(&mut self.science_state.rank8_buttons);

        // When no INI buttons are loaded yet, seed purchased placeholders so HUD
        // residual still reflects snapshot sciences (fail-closed CommandSet art).
        if self.science_state.rank1_buttons.is_empty()
            && self.science_state.rank3_buttons.is_empty()
            && self.science_state.rank8_buttons.is_empty()
            && !unlocked.is_empty()
        {
            self.science_state.rank1_buttons = unlocked
                .iter()
                .take(8)
                .map(|name| ScienceButtonState {
                    command_name: name.clone(),
                    science_type: SCIENCE_INVALID,
                    is_hidden: false,
                    is_enabled: false,
                    is_purchased: true,
                })
                .collect();
        }
        self.mark_ui_dirty();
    }

    pub fn get_science_state(&self) -> &SciencePurchaseState {
        &self.science_state
    }

    // ---------------------------------------------------------------------------
    // Control bar stage management
    // C++ ControlBar.cpp:2968-3053
    // ---------------------------------------------------------------------------

    pub fn switch_control_bar_stage(&mut self, stage: ControlBarStage) {
        self.control_bar_stage = stage;
    }

    pub fn toggle_control_bar_stage(&mut self) {
        if self.control_bar_stage == ControlBarStage::Default {
            self.switch_control_bar_stage(ControlBarStage::Low);
        } else {
            self.switch_control_bar_stage(ControlBarStage::Default);
        }
    }

    pub fn get_control_bar_stage(&self) -> ControlBarStage {
        self.control_bar_stage
    }

    // ---------------------------------------------------------------------------
    // Control bar scheme
    // C++ ControlBar.cpp:2726-2824
    // ---------------------------------------------------------------------------

    pub fn set_control_bar_scheme_by_player(&mut self) {
        let is_observer = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| p.read().ok().map(|guard| guard.is_player_observer()))
            .unwrap_or(false);

        if is_observer {
            self.observer_mode = true;
            let _ = self.switch_to_context(ControlBarState::Observer, None);
        } else {
            self.observer_mode = false;
            let _ = self.switch_to_context(ControlBarState::None, None);
        }

        self.switch_control_bar_stage(ControlBarStage::Default);
    }

    // ---------------------------------------------------------------------------
    // Player event handlers
    // C++ ControlBar.cpp:1651-1682
    // ---------------------------------------------------------------------------

    pub fn on_player_rank_changed(&mut self) {
        self.gen_star_flash = true;
        self.mark_ui_dirty();
    }

    pub fn on_player_science_purchase_points_changed(&mut self) {
        self.gen_star_flash = true;
        self.mark_ui_dirty();
    }

    // ---------------------------------------------------------------------------
    // Special power shortcut bar
    // C++ ControlBar.cpp:3198-3747
    // ---------------------------------------------------------------------------

    pub fn init_special_power_shortcut_bar(&mut self) {
        self.special_power_shortcuts.clear();
        self.special_power_shortcut_count = 0;

        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else { return };
        let Ok(player) = player_arc.read() else {
            return;
        };

        if !player.is_player_active() {
            return;
        }

        self.special_power_shortcut_count = MAX_SPECIAL_POWER_SHORTCUTS;
        for _ in 0..self.special_power_shortcut_count {
            self.special_power_shortcuts
                .push(SpecialPowerShortcutState {
                    command_name: String::new(),
                    availability: CommandAvailability::Hidden,
                    multiplier_count: 1,
                    is_hidden: true,
                });
        }
    }

    fn populate_special_power_shortcut(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.special_power_shortcut_count == 0 {
            return Ok(());
        }

        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else {
            return Ok(());
        };
        let Ok(player) = player_arc.read() else {
            return Ok(());
        };

        let control_bar = get_control_bar_bridge();
        let Some(control_bar) = control_bar else {
            return Ok(());
        };

        let command_set = control_bar
            .find_command_set_by_name("SpecialPowerShortcut")
            .or_else(|| control_bar.find_command_set_by_name("SPECIALPOWERSHORTCUT"));

        let Some(command_set) = command_set else {
            return Ok(());
        };

        let mut current_button = 0;
        for i in 0..self
            .special_power_shortcut_count
            .min(command_set.buttons.len())
        {
            let Some(logic_button) = command_set.buttons.get(i).and_then(|b| b.as_ref()) else {
                continue;
            };

            if (logic_button.get_options_bits() & CommandOption::NeedUpgrade as u32) != 0 {
                let upgrade_name = logic_button
                    .get_upgrade_template()
                    .map(|t| t.get_name().as_str().to_string())
                    .unwrap_or_default();
                if !upgrade_name.is_empty() {
                    let has_upgrade = with_upgrade_center(|c| {
                        let template = c.find_upgrade(&upgrade_name);
                        template.is_some()
                    });
                    if !has_upgrade {
                        continue;
                    }
                }
            }

            if current_button < self.special_power_shortcuts.len() {
                self.special_power_shortcuts[current_button].command_name =
                    logic_button.get_name().to_string();
                self.special_power_shortcuts[current_button].is_hidden = false;
                self.special_power_shortcuts[current_button].availability =
                    CommandAvailability::Available;
                current_button += 1;
            }
        }

        for i in current_button..self.special_power_shortcuts.len() {
            self.special_power_shortcuts[i].is_hidden = true;
        }

        Ok(())
    }

    fn update_special_power_shortcut_availability(&mut self) {
        if self.special_power_shortcut_count == 0 || self.special_power_shortcuts.is_empty() {
            return;
        }

        let player_active = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| p.read().ok().map(|g| g.is_player_active()))
            .unwrap_or(false);

        if !player_active {
            for shortcut in &mut self.special_power_shortcuts {
                shortcut.is_hidden = true;
            }
            return;
        }

        for shortcut in &mut self.special_power_shortcuts {
            if shortcut.is_hidden {
                continue;
            }
            // Default to Available; when count_ready_shortcut_special_powers_of_type
            // is ported, wire per-button availability checks here.
            shortcut.availability = CommandAvailability::Available;
        }
    }

    pub fn hide_special_power_shortcut(&mut self) {
        for shortcut in &mut self.special_power_shortcuts {
            shortcut.is_hidden = true;
        }
    }

    pub fn has_any_shortcut_selection(&self) -> bool {
        self.special_power_shortcuts
            .iter()
            .any(|s| !s.is_hidden && s.availability != CommandAvailability::Hidden)
    }

    /// Feed radar, queued upgrades, and ready special-power shortcuts from PresentationFrame.
    ///
    /// Prefer this over live player-list / OBJECT_REGISTRY for dual-tick host paths.
    /// Fail-closed: shortcut command names are template placeholders (not full CommandSet art).
    pub fn sync_radar_queues_and_specials_from_presentation(
        &mut self,
        radar_count: i32,
        radar_disabled: bool,
        queued_upgrades: &[String],
        ready_special_power_templates: &[String],
    ) {
        self.presentation_radar_count = radar_count;
        self.presentation_radar_disabled = radar_disabled;
        let mut queued = queued_upgrades.to_vec();
        queued.sort();
        queued.dedup();
        self.presentation_queued_upgrades = queued;

        // Seed shortcuts from presentation-ready special powers when empty.
        if self.special_power_shortcuts.is_empty() && !ready_special_power_templates.is_empty() {
            let mut names = ready_special_power_templates.to_vec();
            names.sort();
            names.dedup();
            self.special_power_shortcuts = names
                .into_iter()
                .take(8)
                .map(|command_name| SpecialPowerShortcutState {
                    command_name,
                    availability: CommandAvailability::Available,
                    multiplier_count: 1,
                    is_hidden: false,
                })
                .collect();
            self.special_power_shortcut_count = self.special_power_shortcuts.len();
        } else if !ready_special_power_templates.is_empty() {
            // Mark matching shortcuts available.
            for sc in self.special_power_shortcuts.iter_mut() {
                if ready_special_power_templates.iter().any(|n| {
                    n.eq_ignore_ascii_case(&sc.command_name)
                        || sc.command_name.eq_ignore_ascii_case(n)
                }) {
                    sc.availability = CommandAvailability::Available;
                    sc.is_hidden = false;
                }
            }
        }

        // Gen-star residual: flash when upgrades are queued (purchase-points proxy).
        if !self.presentation_queued_upgrades.is_empty() {
            self.gen_star_flash = true;
        }
        self.mark_ui_dirty();
    }

    pub fn presentation_radar_count(&self) -> i32 {
        self.presentation_radar_count
    }

    pub fn presentation_radar_disabled(&self) -> bool {
        self.presentation_radar_disabled
    }

    pub fn presentation_queued_upgrades(&self) -> &[String] {
        &self.presentation_queued_upgrades
    }

    pub fn get_special_power_shortcuts(&self) -> &[SpecialPowerShortcutState] {
        &self.special_power_shortcuts
    }

    // ---------------------------------------------------------------------------
    // Command bar border colors
    // C++ ControlBar.cpp:2361-2397, 2887-2893
    // ---------------------------------------------------------------------------

    pub fn set_command_bar_border_colors(
        &mut self,
        build: Option<u32>,
        action: Option<u32>,
        upgrade: Option<u32>,
        system: Option<u32>,
    ) {
        self.border_colors.build = build;
        self.border_colors.action = action;
        self.border_colors.upgrade = upgrade;
        self.border_colors.system = system;
    }

    pub fn get_border_color_for_type(
        &self,
        border_type: CommandButtonMappedBorderType,
    ) -> Option<u32> {
        match border_type {
            CommandButtonMappedBorderType::Build => self.border_colors.build,
            CommandButtonMappedBorderType::Upgrade => self.border_colors.upgrade,
            CommandButtonMappedBorderType::Action => self.border_colors.action,
            CommandButtonMappedBorderType::System => self.border_colors.system,
            CommandButtonMappedBorderType::None => None,
        }
    }

    // ---------------------------------------------------------------------------
    // Build queue display helpers
    // C++ ControlBar.cpp:2832-2870
    // ---------------------------------------------------------------------------

    pub fn get_displayed_construct_percent(&self) -> f32 {
        self.displayed_construct_percent
    }

    pub fn get_displayed_ocl_timer_seconds(&self) -> u32 {
        self.displayed_ocl_timer_seconds
    }

    pub fn get_displayed_queue_count(&self) -> usize {
        self.displayed_queue_count
    }

    // ---------------------------------------------------------------------------
    // Communicator hide
    // C++ ControlBar.cpp:2897-2902
    // ---------------------------------------------------------------------------

    pub fn hide_communicator(&mut self, hide: bool) {
        if let Some(wm) = &self.window_manager {
            if let Some(win) = wm.find_window_by_name("ControlBar.wnd:PopupCommunicator") {
                let _ = win.borrow_mut().hide(hide);
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Selection update (external entry point)
    // ---------------------------------------------------------------------------

    pub fn update_for_selection(
        &mut self,
        selected_objects: Vec<u32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut context = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            context.selected_objects = selected_objects;
        }
        self.mark_ui_dirty();
        Ok(())
    }

    pub fn set_observer_mode(&mut self, observer: bool) {
        self.observer_mode = observer;
        self.mark_ui_dirty();
    }

    fn refresh_button_states(&mut self, context: &ControlBarContext, player_id: u32) {
        let mut refreshed = HashMap::new();
        for button in &context.available_commands {
            let mut state = self
                .button_states
                .get(&button.command_name)
                .cloned()
                .unwrap_or_default();
            state.visible = true;
            refreshed.insert(button.command_name.clone(), state);
        }

        for (name, state) in refreshed.iter_mut() {
            if let Some(button) = context
                .available_commands
                .iter()
                .find(|b| &b.command_name == name)
            {
                if let Some(&first_id) = context.selected_objects.first() {
                    match self.get_command_availability(button, first_id, player_id) {
                        Ok(availability) => {
                            state.availability = availability;
                            state.enabled = matches!(
                                availability,
                                CommandAvailability::Available | CommandAvailability::Active
                            );
                            state.check_like_active = availability == CommandAvailability::Active;
                        }
                        Err(_) => {
                            state.enabled = false;
                            state.availability = CommandAvailability::Restricted;
                        }
                    }
                }
            }
        }

        self.button_states = refreshed;
    }

    pub fn get_context(&self) -> Arc<RwLock<ControlBarContext>> {
        self.context.clone()
    }

    pub fn get_build_queue_data(&self) -> &[BuildQueueEntry] {
        &self.build_queue_data
    }

    pub fn get_button_state(&self, command_name: &str) -> Option<&ButtonState> {
        self.button_states.get(command_name)
    }

    pub fn is_ui_dirty(&self) -> bool {
        self.ui_dirty
    }

    pub fn get_current_state(&self) -> ControlBarState {
        self.context
            .read()
            .map(|c| c.current_state)
            .unwrap_or(ControlBarState::None)
    }
}
