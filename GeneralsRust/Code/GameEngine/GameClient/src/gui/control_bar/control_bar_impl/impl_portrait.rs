// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl ControlBar {

    // ---------------------------------------------------------------------------
    // Star image / general button flash
    // C++ ControlBar.cpp:1621-1647
    // ---------------------------------------------------------------------------

    fn leftover_apply_button_general_image(&self, highlight: bool) {
        let candidates = if highlight {
            [
                "GenStarHighlight",
                "ButtonGeneralHilite",
                "ControlBarGeneralHighlight",
                "SSChevronStarOn",
            ]
        } else {
            [
                "GenStarEnable",
                "ButtonGeneralEnable",
                "ControlBarGeneralEnable",
                "SSChevronStar",
            ]
        };
        for name in candidates {
            if leftover_mapped_image(name).is_some() {
                leftover_set_enabled_image(BUTTON_GENERAL, name);
                return;
            }
        }
        if !highlight {
            if let Some(win) = leftover_find_window(BUTTON_GENERAL) {
                // Copy the image out of the owned draw data first: the
                // `win.borrow()` guard in a nested if-let lives until the
                // whole block ends, so a nested `borrow_mut` panics
                // ("RefCell already borrowed") on the live path.
                let image = win
                    .borrow()
                    .get_enabled_draw_data(0)
                    .and_then(|data| data.image);
                if let Some(image) = image {
                    let _ = win.borrow_mut().set_enabled_image(0, image);
                }
            }
        }
    }

    fn update_star_image(&mut self) {
        // C++ getStarImage: flash while sciencePurchasePoints > 0.
        // Leftover PlayerList is empty on the live host — use presentation SPP.
        let leftover_points = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| {
                p.read()
                    .ok()
                    .map(|guard| guard.get_science_purchase_points())
            })
            .unwrap_or(0);
        let current_points = if leftover_points > 0 {
            leftover_points
        } else {
            self.science_state
                .live_science_purchase_points
                .unwrap_or(0)
        };

        if self.last_flashed_at_point_value > current_points || current_points <= 0 {
            self.gen_star_flash = false;
        } else {
            self.last_flashed_at_point_value = current_points;
            self.gen_star_flash = true;
        }

        if !self.gen_star_flash {
            self.leftover_apply_button_general_image(false);
            return;
        }
        let highlight =
            self.current_frame % LOGICFRAMES_PER_SECOND > LOGICFRAMES_PER_SECOND / 2;
        self.leftover_apply_button_general_image(highlight);
    }

    fn leftover_set_win_u_attack_enabled(&mut self, enabled: bool) {
        self.radar_glow_window_enabled = enabled;
        leftover_enable_window(WIN_U_ATTACK, enabled);
    }

    fn update_radar_attack_glow(&mut self) {
        if gamelogic::helpers::TheControlBar::take_radar_attack_glow() {
            self.trigger_radar_attack_glow();
        }
        if !self.radar_attack_glow_on {
            return;
        }
        if self.remaining_radar_attack_glow_frames == 0 {
            self.radar_attack_glow_on = false;
            self.leftover_set_win_u_attack_enabled(true);
            return;
        }
        self.remaining_radar_attack_glow_frames =
            self.remaining_radar_attack_glow_frames.saturating_sub(1);
        if self.remaining_radar_attack_glow_frames == 0 {
            self.radar_attack_glow_on = false;
            self.leftover_set_win_u_attack_enabled(true);
            return;
        }
        if self
            .remaining_radar_attack_glow_frames
            .is_multiple_of(RADAR_ATTACK_GLOW_NUM_TIMES)
        {
            let enabled = !self.radar_glow_window_enabled;
            self.leftover_set_win_u_attack_enabled(enabled);
        }
    }

    pub fn trigger_radar_attack_glow(&mut self) {
        self.radar_attack_glow_on = true;
        self.remaining_radar_attack_glow_frames = RADAR_ATTACK_GLOW_FRAMES;
        if leftover_find_window(WIN_U_ATTACK).is_none() {
            return;
        }
        if self.radar_glow_window_enabled {
            self.leftover_set_win_u_attack_enabled(false);
        }
    }

    pub fn show_rally_point(&mut self, loc: Option<[f32; 3]>) {
        if loc.is_none() {
            if self.rally_point_drawable_id != 0 {
                gamelogic::helpers::TheGameClient.destroy_drawable(self.rally_point_drawable_id);
                self.rally_point_drawable_id = 0;
            }
            self.portrait_state.rally_point = None;
            return;
        }
        let loc = loc.unwrap();
        if self.rally_point_drawable_id == 0 {
            if let Some(template) = TheThingFactory::find_template("RallyPointMarker") {
                let id = gamelogic::helpers::TheGameClient.create_drawable(template.as_ref());
                if id != 0 {
                    self.rally_point_drawable_id = id;
                }
            }
        }
        if self.rally_point_drawable_id == 0 {
            return;
        }
        let pos = gamelogic::common::Coord3D {
            x: loc[0],
            y: loc[1],
            z: loc[2],
        };
        gamelogic::helpers::TheGameClient
            .set_drawable_position(self.rally_point_drawable_id, &pos);
        let data = game_engine::common::global_data::read();
        let downwind = data.downwind_angle;
        let night = matches!(
            data.time_of_day,
            game_engine::common::global_data::TimeOfDay::Night
        );
        drop(data);
        gamelogic::helpers::TheGameClient
            .set_drawable_orientation(self.rally_point_drawable_id, downwind);
        if let Some(player_arc) = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
        {
            if let Ok(player) = player_arc.read() {
                let color = if night {
                    player.get_player_night_color()
                } else {
                    player.get_player_color()
                };
                gamelogic::helpers::TheGameClient
                    .set_drawable_indicator_color(self.rally_point_drawable_id, color);
            }
        }
        self.portrait_state.rally_point = Some(loc);
    }

    // ---------------------------------------------------------------------------
    // Portrait display
    // C++ ControlBar.cpp:2534-2663
    // ---------------------------------------------------------------------------

    fn update_portrait_for_object(&mut self, obj_id: u32) {
        // Wave 249/1008/1014: dual-world peels portrait residual from translator catalog
        // every call (health/veterancy/production refresh, not only first fill).
        if Self::dual_world_registry_unavailable() {
            if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(obj_id)
            {
                // Wave 1053: destroyed/sold/unselectable clear portrait residual.
                // Wave 1080: masked/disabled also clear dual portrait residual.
                if entry.destroyed
                    || entry.sold
                    || entry.unselectable
                    || entry.masked
                    || entry.disabled
                {
                    self.portrait_state = PortraitDisplayState::default();
                    self.build_queue_data.clear();
                    self.displayed_queue_count = 0;
                    return;
                }
                // Wave 1041: C++ ControlBar disguise portrait swap for non-allied viewers.
                let apparent =
                    crate::presentation_translator_residual::translator_entry_apparent_template(
                        &entry,
                    )
                    .to_string();
                if !self.portrait_state.is_visible || self.portrait_state.portrait_image.is_empty()
                {
                    self.portrait_state.portrait_image = apparent.clone();
                } else if entry.disguised
                    && !crate::presentation_translator_residual::translator_entry_is_local(&entry)
                {
                    self.portrait_state.portrait_image = apparent;
                }
                self.portrait_state.is_visible = true;
                self.portrait_state.selected_count = self.portrait_state.selected_count.max(1);
                // Wave 1083: UC dual portrait fails closed for special-power ready residual.
                self.portrait_state.special_power_ready =
                    entry.special_power_ready && !entry.under_construction;
                // Wave 1011/1014: health residual refresh.
                if entry.health_maximum > 0.0 {
                    self.portrait_state.health_current = entry.health_current;
                    self.portrait_state.health_maximum = entry.health_maximum;
                }
                // Wave 1012/1014: veterancy chevron residual refresh.
                if entry.veterancy_overlay.is_some() {
                    self.portrait_state.veterancy_overlay = entry.veterancy_overlay.clone();
                }
                // Wave 1013/1014: production head residual refresh.
                // Wave 1083: under-construction dual portrait clears production residual.
                if entry.under_construction {
                    self.portrait_state.production_progress = None;
                    self.portrait_state.production_template = None;
                    self.portrait_state.production_paused = false;
                } else if entry.production_template.is_some() || entry.production_progress.is_some()
                {
                    self.portrait_state.production_progress = entry.production_progress;
                    self.portrait_state.production_template = entry.production_template.clone();
                    self.portrait_state.production_paused = entry.production_paused;
                }
                // Wave 1015/1017: command-set residual refresh for dual-world ControlBar.
                // Accumulate distinct names so multi-select intersection can peel.
                if !entry.command_set_name.is_empty() {
                    self.presentation_primary_command_set = entry.command_set_name.clone();
                    if !self
                        .presentation_command_set_names
                        .iter()
                        .any(|n| n.eq_ignore_ascii_case(&entry.command_set_name))
                    {
                        self.presentation_command_set_names
                            .push(entry.command_set_name.clone());
                    }
                }
            }
            return;
        }
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            // Presentation residual already owns portrait/health/queue via
            // sync_selection_display_from_presentation — do not wipe it.
            if !self.portrait_state.is_visible {
                self.portrait_state = PortraitDisplayState::default();
            }
            return;
        };
        let Ok(obj) = obj_arc.read() else {
            if !self.portrait_state.is_visible {
                self.portrait_state = PortraitDisplayState::default();
            }
            return;
        };

        if obj.is_kind_of(KindOf::ShowPortraitWhenControlled) && !obj.is_locally_controlled() {
            self.portrait_state = PortraitDisplayState::default();
            return;
        }

        let template_name = obj.get_template_name().to_string();

        let veterancy = obj.get_veterancy_level();
        let veterancy_overlay = match veterancy {
            gamelogic::common::types::VeterancyLevel::Veteran => Some("SSChevron1L".to_string()),
            gamelogic::common::types::VeterancyLevel::Elite => Some("SSChevron2L".to_string()),
            gamelogic::common::types::VeterancyLevel::Heroic => Some("SSChevron3L".to_string()),
            _ => None,
        };

        // Live-registry portrait path: health stays 0 here; presentation overlay
        // (`sync_selection_display_from_presentation`) supplies snapshot HP.
        let context_commands = self
            .context
            .read()
            .ok()
            .map(|ctx| ctx.available_commands.clone());
        let upgrade_cameos = leftover_authored_upgrade_cameo_names(&template_name)
            .into_iter()
            .filter(|name| !name.is_empty())
            .map(|name| {
                let is_completed = leftover_object_or_player_has_upgrade(&obj, &name);
                UpgradeCameoState {
                    button_image: resolve_upgrade_cameo_button_image(
                        &name,
                        context_commands.as_deref(),
                    ),
                    upgrade_name: name,
                    is_completed,
                    is_visible: true,
                }
            })
            .collect();
        self.portrait_state = PortraitDisplayState {
            portrait_image: template_name,
            veterancy_overlay,
            upgrade_cameos,
            is_visible: true,
            health_current: 0.0,
            health_maximum: 0.0,
            selected_count: 1,
            production_progress: None,
            production_template: None,
            production_paused: false,
            special_power_ready: false,
            special_power_cooldown_remaining: 0.0,
            special_power_cooldown_total: 0.0,
            rally_point: None,
        };
    }

    fn update_observer_portrait(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self
            .current_frame
            .is_multiple_of(LOGICFRAMES_PER_SECOND / 2)
        {
            return Ok(());
        }
        self.update_context_observer()?;

        let obj_id = {
            self.context
                .read()
                .map_err(|_| "Failed to acquire context read lock")?
                .selected_objects
                .first()
                .copied()
        };
        if let Some(id) = obj_id {
            self.update_portrait_for_object(id);
        }

        Ok(())
    }

    pub fn set_portrait_by_object_id(&mut self, obj_id: Option<u32>) {
        // Wave 249/1006/1018: dual-world peels catalog portrait on selection;
        // deselection still clears residual portrait/queue (C++ clear path).
        if Self::dual_world_registry_unavailable() {
            if let Some(id) = obj_id {
                // Wave 1018: selection path must refresh portrait residual from catalog
                // (health/veterancy/production/command-set), not only keep prior freeze.
                self.update_portrait_for_object(id);
            } else {
                self.portrait_state = PortraitDisplayState::default();
                self.build_queue_data.clear();
                self.displayed_queue_count = 0;
                self.presentation_primary_command_set.clear();
                self.presentation_command_set_names.clear();
            }
            return;
        }
        if let Some(id) = obj_id {
            self.update_portrait_for_object(id);
        } else {
            self.portrait_state = PortraitDisplayState::default();
        }
    }

    pub fn get_portrait_state(&self) -> &PortraitDisplayState {
        &self.portrait_state
    }

    /// Sync selection panel (portrait + health) from presentation snapshot fields.
    ///
    /// Prefer this over `update_portrait_for_object` for production dual-tick /
    /// headless host paths: no OBJECT_REGISTRY re-read, health is snapshot-owned.
    /// Does not require WindowManager / ControlBar.wnd load (fail-closed for WND).
    pub fn sync_selection_display_from_presentation(
        &mut self,
        primary_template_name: Option<&str>,
        health_current: f32,
        health_maximum: f32,
        selected_count: usize,
        veterancy_overlay: Option<&str>,
        production_progress: Option<f32>,
        production_template: Option<&str>,
        production_queue: &[(String, f32, bool)],
        production_paused: bool,
    ) {
        match primary_template_name {
            Some(name) if !name.is_empty() && selected_count > 0 => {
                self.portrait_state = PortraitDisplayState {
                    portrait_image: name.to_string(),
                    veterancy_overlay: veterancy_overlay.map(str::to_string),
                    upgrade_cameos: std::mem::take(&mut self.portrait_state.upgrade_cameos),
                    is_visible: true,
                    health_current,
                    health_maximum: health_maximum.max(1.0),
                    selected_count,
                    production_progress,
                    production_template: production_template.map(str::to_string),
                    production_paused,
                    // Upgrades/specials filled by sync_upgrades_and_specials_from_presentation.
                    special_power_ready: self.portrait_state.special_power_ready,
                    special_power_cooldown_remaining: self
                        .portrait_state
                        .special_power_cooldown_remaining,
                    special_power_cooldown_total: self.portrait_state.special_power_cooldown_total,
                    rally_point: self.portrait_state.rally_point,
                };
                // Feed construction queue residual from presentation snapshot (no OBJECT_REGISTRY).
                if let Ok(mut context) = self.context.write() {
                    context.construction_queue = production_queue
                        .iter()
                        .map(|(template_name, progress, is_upgrade)| ProductionItem {
                            template_name: template_name.clone(),
                            production_type: if *is_upgrade {
                                ProductionType::Upgrade
                            } else {
                                ProductionType::Unit
                            },
                            progress: *progress,
                            cost: std::collections::HashMap::new(),
                            build_time: 0.0,
                        })
                        .collect();
                }
                self.displayed_queue_count = production_queue.len();
                self.build_queue_data = production_queue
                    .iter()
                    .enumerate()
                    .map(
                        |(idx, (template_name, _progress, is_upgrade))| BuildQueueEntry {
                            production_type: if *is_upgrade {
                                QueueProductionType::Upgrade
                            } else {
                                QueueProductionType::Unit
                            },
                            production_id: idx as u32,
                            upgrade_name: template_name.clone(),
                        },
                    )
                    .collect();
                self.mark_ui_dirty();
            }
            _ => {
                self.portrait_state = PortraitDisplayState::default();
                if let Ok(mut context) = self.context.write() {
                    context.construction_queue.clear();
                    context.ui_dirty = true;
                }
                self.build_queue_data.clear();
                self.displayed_queue_count = 0;
            }
        }
    }

    /// Selection panel health currently shown (presentation-fed when available).

    /// Feed garrison inventory + under-construction commands from PresentationFrame.
    ///
    /// Prefer this over OBJECT_REGISTRY contain lookups for dual-tick / headless host.
    /// Fail-closed: does not claim full WND button layout parity.

    /// Feed upgrade cameos, special-power ready, and rally residual from PresentationFrame.
    ///
    /// C++ `ControlBar::setPortraitByObject` iterates authored `UpgradeCameo1..5`
    /// in slot order, hides empty/unknown names, and `winEnable`s only when the
    /// object or controlling player has the upgrade complete. Incomplete slots
    /// stay visible as a disabled preview. Applied-upgrade leftovers are not
    /// the slot list.
    pub fn sync_upgrades_and_specials_from_presentation(
        &mut self,
        applied_upgrades: &[String],
        rally_point: Option<[f32; 3]>,
        special_power_ready: bool,
        special_power_cooldown_remaining: f32,
        special_power_cooldown_total: f32,
    ) {
        self.sync_upgrade_cameos_from_presentation(
            &[],
            applied_upgrades,
            rally_point,
            special_power_ready,
            special_power_cooldown_remaining,
            special_power_cooldown_total,
        );
    }

    /// Presentation-fed C++ `setPortraitByObject` cameo strip.
    ///
    /// `authored_cameos` is `UpgradeCameo1..5` (empty slots omitted). When
    /// empty, leftover ThingFactory is consulted from the portrait template
    /// name. `completed_upgrades` is object `hasUpgrade` plus player
    /// `hasUpgradeComplete`. Applied names that are not authored never appear.
    pub fn sync_upgrade_cameos_from_presentation(
        &mut self,
        authored_cameos: &[String],
        completed_upgrades: &[String],
        rally_point: Option<[f32; 3]>,
        special_power_ready: bool,
        special_power_cooldown_remaining: f32,
        special_power_cooldown_total: f32,
    ) {
        let context_commands = self
            .context
            .read()
            .ok()
            .map(|ctx| ctx.available_commands.clone());
        let leftover_slots = leftover_authored_upgrade_cameo_names(
            &self.portrait_state.portrait_image,
        );
        let authored: Vec<String> = if !authored_cameos.is_empty() {
            authored_cameos
                .iter()
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .collect()
        } else {
            leftover_slots
                .into_iter()
                .filter(|name| !name.is_empty())
                .collect()
        };
        let cameos: Vec<UpgradeCameoState> = authored
            .into_iter()
            .map(|name| {
                let is_completed = completed_upgrades
                    .iter()
                    .any(|owned| owned.eq_ignore_ascii_case(&name));
                UpgradeCameoState {
                    button_image: resolve_upgrade_cameo_button_image(
                        &name,
                        context_commands.as_deref(),
                    ),
                    upgrade_name: name,
                    is_completed,
                    is_visible: true,
                }
            })
            .collect();
        self.portrait_state.upgrade_cameos = cameos;
        self.portrait_state.special_power_ready = special_power_ready;
        self.portrait_state.special_power_cooldown_remaining = special_power_cooldown_remaining;
        self.portrait_state.special_power_cooldown_total = special_power_cooldown_total;
        self.show_rally_point(rally_point);
        self.mark_ui_dirty();
    }

    /// Populate command buttons from a presentation-frozen CommandSet name.
    ///
    /// Prefer this over live `OBJECT_REGISTRY` / `get_command_set_string` dual-reads
    /// when a PresentationFrame is installed. Fail-closed: not full multi-select
    /// intersection / ScriptOnly / prerequisite filter matrix.
    pub fn sync_command_set_from_presentation(&mut self, command_set_name: Option<&str>) {
        let Some(name) = command_set_name.map(str::trim).filter(|s| !s.is_empty()) else {
            self.presentation_primary_command_set.clear();
            self.presentation_command_set_names.clear();
            return;
        };
        self.presentation_primary_command_set = name.to_string();
        self.presentation_command_set_names = vec![name.to_string()];
        let Some(control_bar) = get_control_bar_bridge() else {
            return;
        };
        let Some(common_bar) = get_ini_control_bar() else {
            return;
        };
        let command_set = control_bar
            .find_command_set_by_name(name)
            .or_else(|| control_bar.find_command_set_by_name(&name.to_ascii_uppercase()));
        let Some(command_set) = command_set else {
            return;
        };
        let Ok(mut context) = self.context.write() else {
            return;
        };
        // Replace object-command buttons with presentation residual set.
        // Keep non-Command inventory/exit buttons already injected by other syncs.
        let keep: Vec<CommandButton> = context
            .available_commands
            .iter()
            .filter(|b| {
                let n = b.command_name.to_ascii_lowercase();
                n.contains("evacuate")
                    || n.contains("structureexit")
                    || n.contains("cancelconstruction")
                    || n.contains("stop")
            })
            .cloned()
            .collect();
        context.available_commands.clear();
        context.current_state = ControlBarState::Command;
        const WND_SLOTS: usize = 14;
        for slot in 0..WND_SLOTS {
            let button = command_set.buttons.get(slot).and_then(|b| b.as_ref());
            context
                .available_commands
                .push(Self::command_from_set_slot(&common_bar, button));
        }
        for b in keep {
            if let Some(existing) = context
                .available_commands
                .iter_mut()
                .find(|x| x.command_name.eq_ignore_ascii_case(&b.command_name))
            {
                if existing.exit_object_id.is_none() {
                    existing.exit_object_id = b.exit_object_id;
                    existing.button_image = b.button_image;
                    existing.overlay_image = b.overlay_image;
                }
            }
        }
        context.ui_dirty = true;
        drop(context);
        self.mark_ui_dirty();
    }

    /// Multi-select command intersection from presentation command-set names
    /// (host path — no dual-world OBJECT_REGISTRY).
    /// Mirrors `control_bar_multi_select::populate_multi_select_commands` slot rules.
    pub fn sync_multi_select_command_sets_from_presentation(
        &mut self,
        command_set_names: &[String],
    ) {
        if command_set_names.len() < 2 {
            return;
        }
        self.presentation_command_set_names = command_set_names.to_vec();
        if let Some(first) = command_set_names.first() {
            self.presentation_primary_command_set = first.clone();
        }
        let Some(control_bar) = get_control_bar_bridge() else {
            return;
        };
        let Some(common_bar) = get_ini_control_bar() else {
            return;
        };

        let mut common_slots: Vec<Option<gamelogic::command_button::CommandButton>> =
            vec![None; gamelogic::command_button::MAX_COMMANDS_PER_SET];
        let mut saw_first = false;

        for name in command_set_names {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let command_set = control_bar
                .find_command_set_by_name(name)
                .or_else(|| control_bar.find_command_set_by_name(&name.to_ascii_uppercase()));
            let Some(command_set) = command_set else {
                common_slots.fill(None);
                saw_first = true;
                break;
            };

            if !saw_first {
                for slot in 0..gamelogic::command_button::MAX_COMMANDS_PER_SET {
                    let Some(button) = command_set
                        .buttons
                        .get(slot)
                        .and_then(|button| button.as_ref())
                    else {
                        continue;
                    };
                    if (button.get_options_bits() & CommandOption::OkForMultiSelect as u32) != 0 {
                        common_slots[slot] = Some(button.clone());
                    }
                }
                saw_first = true;
                continue;
            }

            for slot in 0..gamelogic::command_button::MAX_COMMANDS_PER_SET {
                let command = command_set
                    .buttons
                    .get(slot)
                    .and_then(|button| button.as_ref());
                let common = common_slots[slot].as_ref();

                let attack_move = command
                    .map(|button| {
                        button.get_command_type()
                            == gamelogic::commands::CommandType::DoAttackMoveTo
                    })
                    .unwrap_or(false)
                    || common
                        .map(|button| {
                            button.get_command_type()
                                == gamelogic::commands::CommandType::DoAttackMoveTo
                        })
                        .unwrap_or(false);

                if attack_move && common_slots[slot].is_none() {
                    common_slots[slot] = command.cloned();
                    continue;
                }
                if attack_move {
                    continue;
                }

                let matches = match (command, common) {
                    (Some(a), Some(b)) => a.get_id() == b.get_id(),
                    (None, None) => true,
                    _ => false,
                };
                if !matches {
                    common_slots[slot] = None;
                }
            }
        }

        if !saw_first {
            return;
        }

        let Ok(mut context) = self.context.write() else {
            return;
        };
        context.current_state = ControlBarState::MultiSelect;
        context.available_commands.clear();
        for slot in 0..14 {
            let button = common_slots.get(slot).and_then(|b| b.as_ref());
            context
                .available_commands
                .push(Self::command_from_set_slot(&common_bar, button));
        }
        context.ui_dirty = true;
        drop(context);
        self.mark_ui_dirty();
    }

    pub fn sync_presentation_occupants(
        &mut self,
        occupants: Vec<super::control_bar_structure_inventory::StructureInventoryOccupant>,
    ) {
        self.presentation_occupants = occupants;
    }

    pub fn sync_structure_context_from_presentation(
        &mut self,
        max_garrison: usize,
        garrisoned_count: usize,
        under_construction: bool,
        construction_percent: f32,
    ) {
        self.presentation_max_garrison = max_garrison;
        self.presentation_garrisoned_count = garrisoned_count;
        self.presentation_under_construction = under_construction;
        self.presentation_construction_percent = construction_percent.clamp(0.0, 1.0);
        if under_construction {
            self.displayed_construct_percent = self.presentation_construction_percent;
        }
        let mut inventory_synced = false;
        if let Ok(mut context) = self.context.write() {
            context.last_recorded_inventory_count = garrisoned_count as u32;
            if under_construction {
                let cancel = CommandButton {
                    command_name: "Command_CancelConstruction".into(),
                    ..CommandButton::default()
                };
                if !context
                    .available_commands
                    .iter()
                    .any(|b| b.command_name.eq_ignore_ascii_case(&cancel.command_name))
                {
                    context.available_commands.push(cancel);
                }
            }
            if max_garrison > 0 {
                if context.current_state == ControlBarState::StructureInventory {
                    let _ = super::control_bar_structure_inventory::append_structure_inventory_commands_with_presentation(
                        &mut context,
                        max_garrison,
                        garrisoned_count,
                        &self.presentation_occupants,
                    );
                } else {
                    let _ = super::control_bar_structure_inventory::do_transport_inventory_ui(
                        &mut context,
                        max_garrison,
                        garrisoned_count,
                        &self.presentation_occupants,
                    );
                }
                context.ui_dirty = true;
                inventory_synced = true;
            } else {
                context.ui_dirty = true;
            }
        }
        self.mark_ui_dirty();
        if inventory_synced {
            return;
        }
    }

    /// Wave 1031: host/presentation OCL timer residual (ControlBar OclTimer context).
    /// Wave 1033: host/presentation sold residual (ControlBar clear path).
    pub fn sync_sold_from_presentation(&mut self, sold: bool) {
        self.presentation_sold = sold;
    }

    pub fn sync_ocl_timer_from_presentation(&mut self, seconds: u32) {
        self.presentation_ocl_timer_seconds = seconds;
        if seconds != self.displayed_ocl_timer_seconds {
            self.displayed_ocl_timer_seconds = seconds;
        }
    }

    pub fn selection_panel_health(&self) -> Option<(f32, f32)> {
        if self.portrait_state.is_visible && self.portrait_state.health_maximum > 0.0 {
            Some((
                self.portrait_state.health_current,
                self.portrait_state.health_maximum,
            ))
        } else {
            None
        }
    }
}

/// C++ `ThingTemplate::getUpgradeCameoName(0..4)` via leftover ThingFactory.
pub fn leftover_authored_upgrade_cameo_names(template_name: &str) -> [String; 5] {
    let trimmed = template_name.trim();
    if trimmed.is_empty() {
        return Default::default();
    }
    let Ok(factory_guard) = game_engine::common::thing::get_thing_factory() else {
        return Default::default();
    };
    let Some(factory) = factory_guard.as_ref() else {
        return Default::default();
    };
    let Some(template) = factory.find_template(trimmed, false) else {
        return Default::default();
    };
    let mut names = [String::new(), String::new(), String::new(), String::new(), String::new()];
    for (i, slot) in names.iter_mut().enumerate() {
        let name = template.get_upgrade_cameo_name(i);
        let text = name.as_str().trim();
        if !text.is_empty() {
            *slot = text.to_string();
        }
    }
    names
}

/// C++ `obj->hasUpgrade(ut) || player->hasUpgradeComplete(ut)`.
fn leftover_object_or_player_has_upgrade(obj: &gamelogic::object::Object, name: &str) -> bool {
    let Some(upgrade) = gamelogic::upgrade::center::with_upgrade_center(|center| {
        center.find_upgrade(name)
    }) else {
        return false;
    };
    if obj.has_upgrade(upgrade.as_ref()) {
        return true;
    }
    obj.get_controlling_player().is_some_and(|player_arc| {
        player_arc
            .read()
            .ok()
            .is_some_and(|player| player.has_upgrade_complete(upgrade.as_ref()))
    })
}
