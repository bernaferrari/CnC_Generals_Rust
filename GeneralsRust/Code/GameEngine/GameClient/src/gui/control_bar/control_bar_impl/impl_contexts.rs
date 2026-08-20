// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl ControlBar {
    // ---------------------------------------------------------------------------
    // Context update helpers
    // ---------------------------------------------------------------------------

    fn update_context_multi_select(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let (player_id, selected, buttons) = {
            let context = self
                .context
                .read()
                .map_err(|_| "Failed to acquire context read lock")?;
            (
                context.player_id,
                context.selected_objects.clone(),
                context.available_commands.clone(),
            )
        };
        if selected.len() < 2 {
            return Ok(());
        }

        let mut objects_that_can: Vec<u32> = vec![0; buttons.len()];
        for obj_id in &selected {
            if let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) {
                if let Ok(obj) = obj_arc.read() {
                    if obj.is_kind_of(KindOf::IgnoredInGui) {
                        continue;
                    }
                }
            }
            for (i, button) in buttons.iter().enumerate() {
                if button.button_hidden || button.command_name.is_empty() {
                    continue;
                }
                let availability = self.get_command_availability(button, *obj_id, player_id)?;
                if matches!(
                    availability,
                    CommandAvailability::Available | CommandAvailability::Active
                ) {
                    objects_that_can[i] += 1;
                }
                if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                    match availability {
                        CommandAvailability::Hidden => bs.visible = false,
                        CommandAvailability::Restricted => {
                            bs.enabled = false;
                            bs.availability = availability;
                        }
                        CommandAvailability::NotReady => {
                            bs.enabled = false;
                            bs.availability = availability;
                        }
                        CommandAvailability::CantAfford => {
                            bs.enabled = false;
                            bs.availability = availability;
                        }
                        CommandAvailability::Active => {
                            bs.enabled = true;
                            bs.availability = availability;
                            if (button.options & CommandOption::CheckLike as u32) != 0 {
                                bs.check_like_active = true;
                            }
                        }
                        CommandAvailability::Available => {
                            bs.enabled = true;
                            bs.availability = availability;
                            if (button.options & CommandOption::CheckLike as u32) != 0 {
                                bs.check_like_active = false;
                            }
                        }
                    }
                }
            }
        }

        for (i, button) in buttons.iter().enumerate() {
            if button.button_hidden || button.command_name.is_empty() {
                continue;
            }
            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                bs.enabled = objects_that_can.get(i).copied().unwrap_or(0) > 0;
            }
        }
        Ok(())
    }


    /// C++ ControlBar.cpp:1410-1433: refresh observer info window every half-second.
    /// C++ ControlBarObserver.cpp:271 populateObserverInfoWindow: units, buildings, kills, losses.
    fn update_context_observer(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let frame = TheGameLogic::get_frame();
        if !frame.is_multiple_of(15) {
            return Ok(());
        }

        super::control_bar_observer::init_observer_controls();
        if super::control_bar_observer::observer_look_at_player_index().is_some() {
            super::control_bar_observer::populate_observer_info_window();
        } else {
            super::control_bar_observer::populate_observer_list();
        }

        let player_list = logic_player_list();
        let player_count = player_list
            .read()
            .map(|list| list.get_player_count())
            .unwrap_or(0);

        let mut observer_stats: Vec<(String, i32, i32, i32, i32, i32)> = Vec::new();
        for i in 0..player_count {
            let player_arc = player_list
                .read()
                .ok()
                .and_then(|list| list.get_player(i as PlayerIndex).cloned());

            let Some(player_arc) = player_arc else {
                continue;
            };
            let Ok(player) = player_arc.read() else {
                continue;
            };

            if player.is_player_observer() {
                continue;
            }

            let display_name = player.get_player_display_name().clone();
            let money = player.get_money().get_money();

            let score_bit = 1u64 << 45;
            let struct_bit = 1u64 << 8;
            let score_create_bit = 1u64 << 46;
            let score_destroy_bit = 1u64 << 47;

            let num_units = player.count_objects_by_kindof(score_bit as u128, struct_bit as u128);

            let num_buildings = player.count_objects_by_kindof((score_bit | struct_bit) as u128, 0)
                + player.count_objects_by_kindof((score_create_bit | struct_bit) as u128, 0)
                + player.count_objects_by_kindof((score_destroy_bit | struct_bit) as u128, 0);

            let score_keeper = player.get_score_keeper();
            let units_killed = score_keeper.get_total_units_destroyed();
            let units_lost = score_keeper.get_total_units_lost();

            observer_stats.push((
                display_name,
                money,
                num_units,
                num_buildings,
                units_killed,
                units_lost,
            ));
        }

        if let Ok(mut ctx) = self.context.write() {
            ctx.observer_player_stats = observer_stats;
            ctx.observer_look_at_player =
                super::control_bar_observer::observer_look_at_player_index();
        }

        Ok(())
    }

    /// C++ ControlBarStructureInventory.cpp:181-214: update garrison/contain inventory.
    fn update_context_structure_inventory(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let selected_object = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?
            .selected_objects
            .first()
            .copied();

        let Some(object_id) = selected_object else {
            return Ok(());
        };
        // Wave 1027: host empty dual-world peels presentation garrison residual count.
        // Wave 1077: catalog occupant residual when freeze count unset; clear on unusable.
        if Self::dual_world_registry_unavailable() {
            let mut contain_count = self.presentation_garrisoned_count as u32;
            if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(object_id)
            {
                if entry.destroyed || entry.sold || entry.masked || entry.unselectable {
                    contain_count = 0;
                } else if contain_count == 0 && entry.occupant_count > 0 {
                    contain_count = entry.occupant_count as u32;
                }
            }
            if let Ok(mut ctx) = self.context.write() {
                if ctx.last_recorded_inventory_count != contain_count {
                    ctx.last_recorded_inventory_count = contain_count;
                    ctx.ui_dirty = true;
                }
            }
            let _ = object_id;
            return Ok(());
        }
        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Ok(());
        };
        let Ok(object) = object_arc.read() else {
            return Ok(());
        };

        let player_list = logic_player_list();
        let local_player_index = player_list
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(gamelogic::player::PLAYER_INDEX_INVALID);

        let obj_player_id = object.get_controlling_player_id().unwrap_or(0xFFFF) as PlayerIndex;
        if obj_player_id != local_player_index {
            let local_arc = player_list
                .read()
                .ok()
                .and_then(|list| list.get_player(local_player_index).cloned());
            let obj_arc = player_list
                .read()
                .ok()
                .and_then(|list| list.get_player(obj_player_id).cloned());

            if let (Some(local_arc), Some(obj_arc)) = (local_arc, obj_arc) {
                if let (Ok(local_guard), Ok(obj_guard)) = (local_arc.read(), obj_arc.read()) {
                    let rel = local_guard.get_relationship(&obj_guard);
                    if rel != gamelogic::common::Relationship::Neutral {
                        TheInGameUI::deselect_all();
                        return Ok(());
                    }
                }
            }
        }

        let Some(contain) = object.get_contain() else {
            return Ok(());
        };
        let contain_count = contain.lock().map(|c| c.get_contain_count()).unwrap_or(0);

        if let Ok(mut ctx) = self.context.write() {
            if ctx.last_recorded_inventory_count != contain_count {
                ctx.last_recorded_inventory_count = contain_count;
                ctx.ui_dirty = true;
            }
        }

        Ok(())
    }

    fn update_context_beacon(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let selected_object = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?
            .selected_objects
            .first()
            .copied();

        let Some(object_id) = selected_object else {
            self.populate_beacon_windows(false, "")?;
            return Ok(());
        };
        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            // Host presentation residual: beacon UI when command-set freeze says BEACON.
            // Wave 1030: peel translator catalog template/command-set residual too.
            let catalog_beacon =
                crate::presentation_translator_residual::translator_catalog_entry(object_id)
                    .map(|e| {
                        e.template_name.to_ascii_uppercase().contains("BEACON")
                            || e.command_set_name.to_ascii_uppercase().contains("BEACON")
                    })
                    .unwrap_or(false);
            let is_beacon = catalog_beacon
                || self
                    .presentation_primary_command_set
                    .to_ascii_uppercase()
                    .contains("BEACON")
                || self
                    .portrait_state
                    .portrait_image
                    .to_ascii_uppercase()
                    .contains("BEACON");
            self.populate_beacon_windows(is_beacon, "")?;
            return Ok(());
        };
        let Ok(object) = object_arc.read() else {
            let is_beacon = self
                .presentation_primary_command_set
                .to_ascii_uppercase()
                .contains("BEACON");
            self.populate_beacon_windows(is_beacon, "")?;
            return Ok(());
        };

        let position = *object.get_position();
        let player_id = object.get_controlling_player_id().map(|id| id as i32);
        let caption = player_id
            .and_then(|player_id| {
                snapshot_beacons()
                    .into_iter()
                    .find(|entry| {
                        entry.player_id == player_id && (entry.position - position).length() <= 3.0
                    })
                    .and_then(|entry| entry.text.map(|text| text.to_string()))
            })
            .unwrap_or_default();

        self.populate_beacon_windows(object.is_locally_controlled(), &caption)?;
        Ok(())
    }

    fn update_place_beacon_button_enabled(&self) {
        let Some(window_manager) = self.window_manager.as_ref() else {
            return;
        };
        let place_button = window_manager.find_window_by_name("ControlBar.wnd:ButtonPlaceBeacon");
        let enabled = self.local_player_below_beacon_limit();
        Self::apply_place_beacon_button_enabled(&place_button, enabled);
    }

    fn local_player_below_beacon_limit(&self) -> bool {
        let Some(local_player) = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
        else {
            return false;
        };
        let Ok(local_player) = local_player.read() else {
            return false;
        };
        let Some(template_name) = local_player
            .get_player_template()
            .map(|template| template.beacon_name.clone())
        else {
            return false;
        };
        if template_name.is_empty() {
            return false;
        }
        let Some(beacon_template) = TheThingFactory::find_template(&template_name) else {
            return false;
        };
        let mut count = [0];
        local_player.count_objects_by_thing_template(
            std::slice::from_ref(&beacon_template),
            false,
            false,
            &mut count,
        );
        let max_beacons = with_multiplayer_settings(|settings| settings.max_beacons_per_player);
        count[0] < max_beacons
    }

    fn apply_place_beacon_button_enabled(
        place_button: &Option<Rc<RefCell<GameWindow>>>,
        enabled: bool,
    ) {
        if let Some(window) = place_button {
            let _ = window.borrow_mut().enable(enabled);
        }
    }

    fn populate_beacon_windows(
        &self,
        locally_controlled: bool,
        caption: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window_manager) = self.window_manager.as_ref() else {
            return Ok(());
        };

        let text_entry = window_manager.find_window_by_name("ControlBar.wnd:EditBeaconText");
        let static_text =
            window_manager.find_window_by_name("ControlBar.wnd:StaticTextBeaconLabel");
        let clear_button =
            window_manager.find_window_by_name("ControlBar.wnd:ButtonClearBeaconText");

        Self::apply_beacon_window_state(
            &text_entry,
            &static_text,
            &clear_button,
            locally_controlled,
            caption,
        );
        Ok(())
    }

    fn apply_beacon_window_state(
        text_entry: &Option<Rc<RefCell<GameWindow>>>,
        static_text: &Option<Rc<RefCell<GameWindow>>>,
        clear_button: &Option<Rc<RefCell<GameWindow>>>,
        locally_controlled: bool,
        caption: &str,
    ) {
        if locally_controlled {
            if let Some(window) = text_entry {
                let mut guard = window.borrow_mut();
                let _ = guard.hide(false);
                let _ = guard.set_text(caption);
            }
            if let Some(window) = static_text {
                let _ = window.borrow_mut().hide(false);
            }
            if let Some(window) = clear_button {
                let _ = window.borrow_mut().hide(false);
            }
        } else {
            if let Some(window) = text_entry {
                let _ = window.borrow_mut().hide(true);
            }
            if let Some(window) = static_text {
                let _ = window.borrow_mut().hide(true);
            }
            if let Some(window) = clear_button {
                let _ = window.borrow_mut().hide(true);
            }
        }
    }

    fn update_context_under_construction(
        &mut self,
        _delta_time: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let has_selection = {
            let context = self
                .context
                .read()
                .map_err(|_| "Failed to acquire context read lock")?;
            !context.selected_objects.is_empty()
        };
        if !has_selection {
            return Ok(());
        }

        // Host/presentation residual owns construct percent display. Dual-world
        // get_construct_percent() can overlay later when OBJECT_REGISTRY modules exist.
        // Wave 1029: catalog under_construction residual keeps dual-world UC context live.
        let selected_id = self
            .context
            .read()
            .ok()
            .and_then(|c| c.selected_objects.first().copied())
            .unwrap_or(0);
        if !self.presentation_under_construction {
            if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(selected_id)
            {
                if entry.under_construction {
                    self.presentation_under_construction = true;
                    self.presentation_construction_percent = entry.construction_percent;
                }
            }
        }
        if !(self.presentation_under_construction
            || self.portrait_state.is_visible
            || OBJECT_REGISTRY.get_object(selected_id).is_some())
        {
            return Ok(());
        }

        let percent = self.presentation_construction_percent;
        if (percent - self.displayed_construct_percent).abs() > 0.001 {
            self.displayed_construct_percent = percent;
            self.mark_ui_dirty();
        }
        Ok(())
    }

    fn update_context_ocl_timer(
        &mut self,
        _delta_time: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let obj_id = {
            let context = self
                .context
                .read()
                .map_err(|_| "Failed to acquire context read lock")?;
            context.selected_objects.first().copied()
        };
        let Some(obj_id) = obj_id else {
            return Ok(());
        };

        // Host/presentation residual: keep OCL display coherent without dual-world modules.
        // Dual-world path may later read OCLUpdate; until then presentation freeze owns display.
        let _registry_bound = OBJECT_REGISTRY.get_object(obj_id).is_some();
        let _ = _registry_bound;
        let seconds = self.presentation_ocl_timer_seconds;
        if seconds != self.displayed_ocl_timer_seconds {
            self.displayed_ocl_timer_seconds = seconds;
            self.mark_ui_dirty();
        }
        Ok(())
    }
}
