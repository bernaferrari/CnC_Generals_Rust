// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl ControlBar {
    pub fn new() -> Self {
        Self {
            context: Arc::new(RwLock::new(ControlBarContext::default())),
            window_manager: None,
            scheme_manager: None,
            resizer: None,
            current_window: None,
            is_animating: false,
            animation_start_time: Instant::now(),
            animation_duration: Duration::from_millis(500),
            button_states: HashMap::new(),
            observer_mode: false,
            multi_select_mode: false,
            ui_dirty: false,
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
            special_power_shortcuts: Vec::new(),
            special_power_shortcut_count: 0,
            presentation_radar_count: 0,
            presentation_radar_disabled: false,
            presentation_queued_upgrades: Vec::new(),
            presentation_primary_command_set: String::new(),
            presentation_command_set_names: Vec::new(),
            presentation_max_garrison: 0,
            presentation_garrisoned_count: 0,
            presentation_under_construction: false,
            presentation_sold: false,
            presentation_construction_percent: 0.0,
            presentation_ocl_timer_seconds: 0,
            displayed_construct_percent: -1.0,
            displayed_ocl_timer_seconds: 0,
            border_colors: CommandBarBorderColors::default(),
        }
    }

    pub fn set_window_manager(&mut self, manager: Arc<WindowManager>) {
        self.window_manager = Some(manager);
    }

    pub fn set_scheme_manager(&mut self, manager: Arc<dyn ControlBarSchemeManager>) {
        self.scheme_manager = Some(manager);
    }

    pub fn set_resizer(&mut self, resizer: Arc<dyn ControlBarResizer>) {
        self.resizer = Some(resizer);
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
    /// C++: ControlBar::onDrawableSelected()
    pub fn on_drawable_selected(&mut self) {
        self.mark_ui_dirty();
        TheInGameUI::clear_pending_special_power();
    }

    /// Called when a drawable is deselected.
    /// C++: ControlBar::onDrawableDeselected()
    pub fn on_drawable_deselected(&mut self, select_count: usize) {
        self.mark_ui_dirty();
        if select_count == 0 {
            TheInGameUI::clear_pending_special_power();
        }
        TheInGameUI::place_build_available(None, None);
    }

    // ---------------------------------------------------------------------------
    // update - main per-frame update
    // C++ ControlBar.cpp:1359-1580
    // ---------------------------------------------------------------------------

    /// Main update loop. Mirrors C++ ControlBar::update().
    pub fn update(&mut self, delta_time: Duration) -> Result<(), Box<dyn std::error::Error>> {
        self.apply_live_hook_events();
        crate::gui::w3d_gadget_draw::ensure_scheme_draw_registered();
        crate::gui::w3d_gadget_draw::ensure_control_bar_wnd_draw_callbacks();

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

        self.current_frame = TheGameLogic::get_frame();
        self.update_star_image();
        self.update_radar_attack_glow();

        if self.observer_mode {
            self.update_observer_portrait()?;
            return Ok(());
        }

        if self.science_state.is_visible {
            self.update_context_purchase_science();
        }

        self.update_flash_buttons();

        if self.ui_dirty {
            self.evaluate_context_ui()?;
            self.populate_special_power_shortcut()?;
            self.ui_dirty = false;
        }

        self.update_place_beacon_button_enabled();

        let context = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?;
        let current_state = context.current_state;
        let selected_objects = context.selected_objects.clone();
        let player_id = context.player_id;
        drop(context);

        if current_state == ControlBarState::MultiSelect {
            self.update_context_multi_select()?;
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
            self.switch_to_context(ControlBarState::None, None)?;
            return Ok(());
        }
        // Without registry modules, skip live module context updates — presentation
        // already owns portrait/health/queue residual.
        if !registry_exists {
            return Ok(());
        }

        match current_state {
            ControlBarState::None => {}
            ControlBarState::Command => {
                self.update_context_command()?;
            }
            ControlBarState::StructureInventory => {
                self.update_context_structure_inventory()?;
            }
            ControlBarState::Beacon => {
                self.update_context_beacon()?;
            }
            ControlBarState::UnderConstruction => {
                self.update_context_under_construction(delta_time)?;
            }
            ControlBarState::OclTimer => {
                self.update_context_ocl_timer(delta_time)?;
            }
            ControlBarState::Observer => {
                self.update_context_observer()?;
            }
            ControlBarState::MultiSelect => {
                self.update_context_multi_select()?;
            }
        }

        if let Ok(mut context) = self.context.write() {
            for item in context.construction_queue.iter_mut() {
                if item.progress < 1.0 && item.build_time > 0.0 {
                    item.progress += delta_time.as_secs_f32() / item.build_time;
                    item.progress = item.progress.min(1.0);
                }
            }
        }

        let context_snapshot = self.context.read().ok().map(|c| c.clone());
        if let Some(context) = context_snapshot {
            self.refresh_button_states(&context, player_id);
        }

        self.update_special_power_shortcut_availability();

        set_live_control_bar_observer_look_at(self.get_observer_look_at_player_index());

        Ok(())
    }

    fn apply_live_hook_events(&mut self) {
        let events = drain_live_control_bar_events();
        if events.ui_dirty {
            self.mark_ui_dirty();
        }
        if events.hide_purchase_science {
            self.hide_purchase_science();
        }
        if events.toggle_purchase_science {
            self.toggle_purchase_science();
        }
        if events.show_special_power_shortcut {
            self.show_special_power_shortcut();
        }
        if events.hide_special_power_shortcut {
            self.hide_special_power_shortcut();
        }
        if let Some(enabled) = events.animate_special_power_shortcut {
            self.animate_special_power_shortcut(enabled);
        }
        if events.toggle_control_bar_stage {
            self.toggle_control_bar_stage();
        }
        if events.init_special_power_shortcut_for_player.is_some() {
            self.init_special_power_shortcut_bar();
        }
        if events.set_scheme_by_player.is_some() {
            self.set_control_bar_scheme_by_player();
        }
        for (control_id, msg) in events.clicks {
            self.process_context_sensitive_button_click(control_id, msg);
        }
        let _ = events.transitions;
    }

    // ---------------------------------------------------------------------------
    // evaluateContextUI - determine what context to show
    // C++ ControlBar.cpp:1689-1888
    // ---------------------------------------------------------------------------

    fn evaluate_context_ui(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.ui_dirty = false;

        let mut context = {
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            std::mem::take(&mut *guard)
        };

        if context.selected_objects.is_empty() {
            context.current_state = ControlBarState::None;
            context.available_commands.clear();
            context.construction_queue.clear();
            self.build_queue_data.clear();
            self.displayed_queue_count = 0;
            self.portrait_state = PortraitDisplayState::default();
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        }

        let multi_select = context.selected_objects.len() > 1;
        let single_drawable_id = if multi_select {
            None
        } else {
            context.selected_objects.first().copied()
        };

        if multi_select {
            context.current_state = ControlBarState::MultiSelect;
            self.rebuild_command_buttons(&mut context)?;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        }

        let Some(obj_id) = single_drawable_id else {
            context.current_state = ControlBarState::None;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
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
                let mut guard = self
                    .context
                    .write()
                    .map_err(|_| "Failed to acquire context write lock")?;
                *guard = context;
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
            if context.current_state != ControlBarState::None {
                self.rebuild_command_buttons(&mut context)?;
            }
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        };
        let Ok(obj) = obj_arc.read() else {
            context.current_state = ControlBarState::None;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        };

        if obj.test_status(OBJECT_STATUS_SOLD) {
            drop(obj);
            context.current_state = ControlBarState::None;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
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

        self.rebuild_command_buttons(&mut context)?;

        if context.current_state == ControlBarState::Command {
            self.populate_build_queue(&mut context, obj_id)?;
        }

        let mut guard = self
            .context
            .write()
            .map_err(|_| "Failed to acquire context write lock")?;
        *guard = context;

        Ok(())
    }
}
