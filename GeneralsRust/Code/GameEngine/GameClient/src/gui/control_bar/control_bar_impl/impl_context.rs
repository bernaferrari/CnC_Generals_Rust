// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

impl ControlBar {
    // ---------------------------------------------------------------------------
    // switchToContext - change the active context
    // C++ ControlBar.cpp:2098-2359
    // ---------------------------------------------------------------------------

    fn switch_to_context(
        &mut self,
        new_state: ControlBarState,
        _draw_id: Option<u32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.portrait_state = PortraitDisplayState::default();
        if new_state == ControlBarState::None {
            self.show_rally_point(None);
        }

        let mut context = {
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            guard.current_state = new_state;
            std::mem::take(&mut *guard)
        };

        context.available_commands.clear();
        context.construction_queue.clear();
        self.build_queue_data.clear();
        self.displayed_queue_count = 0;
        self.displayed_construct_percent = -1.0;
        self.displayed_ocl_timer_seconds = 0;

        if let Some(&obj_id) = context.selected_objects.first() {
            self.update_portrait_for_object(obj_id);
        }

        // C++ ControlBar.cpp:2177-2188 CB_CONTEXT_COMMAND: show CommandWindow,
        // hide UnderConstruction / OCLTimer / Observer list / Beacon.
        // populateCommand (ControlBarCommand.cpp:317) winHide(FALSE) ButtonCommand01-14.
        if new_state == ControlBarState::Command {
            reveal_ingame_command_window();
        }
        if new_state == ControlBarState::Observer {
            super::control_bar_observer::init_observer_controls();
            super::control_bar_observer::reveal_observer_list_window();
        } else if new_state == ControlBarState::OclTimer {
            // C++ ControlBar.cpp:2293-2295 CB_CONTEXT_OCL_TIMER hides both
            // observer parents and lets the OCL lane own CP_OCL_TIMER.
            super::control_bar_observer::hide_observer_context_windows();
        } else {
            // C++ ControlBar.cpp:2135-2137, 2186-2188, 2229-2231, 2250-2252,
            // 2272-2274: every other context hides CP_OCL_TIMER plus both
            // observer context parents.
            super::control_bar_observer::hide_observer_and_ocl_context_windows();
        }
        if new_state == ControlBarState::StructureInventory {
            reveal_ingame_command_window();
        }

        self.rebuild_command_buttons(&mut context)?;

        if new_state == ControlBarState::Command {
            if let Some(&obj_id) = context.selected_objects.first() {
                let _ = self.populate_build_queue(&mut context, obj_id);
            }
        }

        let mut guard = self
            .context
            .write()
            .map_err(|_| "Failed to acquire context write lock")?;
        *guard = context;
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // updateContextCommand - per-frame command context update
    // C++ ControlBarCommand.cpp:678-891
    // ---------------------------------------------------------------------------

    /// Wave 249: host/presentation path has no dual-world factory objects.
    #[inline]
    fn dual_world_registry_unavailable() -> bool {
        OBJECT_REGISTRY.is_empty()
    }

    /// Wave 1032: C++ beacon template/command-set residual name match.
    fn presentation_name_is_beacon(name: &str) -> bool {
        name.to_ascii_uppercase().contains("BEACON")
    }

    fn get_object_production_info(&self, obj_id: u32) -> (usize, bool) {
        // Presentation residual first (host path has no dual-world registry modules).
        if !self.build_queue_data.is_empty() {
            return (self.build_queue_data.len(), true);
        }
        if self.portrait_state.production_progress.is_some()
            || self.portrait_state.production_template.is_some()
        {
            return (self.displayed_queue_count.max(1), true);
        }
        // Wave 1029: dual-world peels catalog production residual when registry empty.
        if OBJECT_REGISTRY.get_object(obj_id).is_none() {
            if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(obj_id)
            {
                if entry.production_template.is_some() || entry.production_progress.is_some() {
                    return (self.displayed_queue_count.max(1), true);
                }
                if !entry.command_set_name.is_empty() {
                    const FACTORY_KINDS: &[&str] = &[
                        "FSBarracks",
                        "FSWarFactory",
                        "FSAirfield",
                        "CommandCenter",
                        "Structure",
                    ];
                    if entry.kind_names.iter().any(|k| {
                        FACTORY_KINDS
                            .iter()
                            .any(|f| k == f || k.eq_ignore_ascii_case(f))
                    }) {
                        return (0, true);
                    }
                }
            }
            return (0, false);
        }
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return (0, false);
        };
        let Ok(obj) = obj_arc.read() else {
            return (0, false);
        };
        for module in obj.get_behavior_modules() {
            if let Ok(mut guard) = module.lock() {
                if guard.get_production_update_interface().is_some() {
                    return (0, true);
                }
            }
        }
        (0, false)
    }

    fn get_first_production_progress(&self, obj_id: u32) -> Option<f32> {
        // Presentation residual owns host InGame queue progress display.
        if let Some(p) = self.portrait_state.production_progress {
            if p > 0.0 {
                return Some(p);
            }
        }
        if let Ok(context) = self.context.read() {
            if let Some(first) = context.construction_queue.first() {
                if first.progress > 0.0 {
                    return Some(first.progress);
                }
            }
        }
        // Wave 1029: dual-world peels catalog production_progress residual.
        if OBJECT_REGISTRY.get_object(obj_id).is_none() {
            if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(obj_id)
            {
                if let Some(p) = entry.production_progress {
                    if p > 0.0 {
                        return Some(p);
                    }
                }
            }
            return None;
        }
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return None;
        };
        let Ok(obj) = obj_arc.read() else {
            return None;
        };
        for module in obj.get_behavior_modules() {
            if let Ok(mut guard) = module.lock() {
                if let Some(pu) = guard.get_production_update_interface() {
                    let progress = pu.get_production_progress();
                    if progress > 0.0 {
                        return Some(progress);
                    }
                }
            }
        }
        None
    }

    fn map_logic_production_type(
        production_type: gamelogic::object::production::queue::ProductionType,
    ) -> ProductionType {
        match production_type {
            gamelogic::object::production::queue::ProductionType::Unit => ProductionType::Unit,
            gamelogic::object::production::queue::ProductionType::Upgrade => {
                ProductionType::Upgrade
            }
            gamelogic::object::production::queue::ProductionType::SpecialPower => {
                ProductionType::SpecialPower
            }
        }
    }

    fn map_logic_queue_type(
        production_type: gamelogic::object::production::queue::ProductionType,
    ) -> QueueProductionType {
        match production_type {
            gamelogic::object::production::queue::ProductionType::Unit => QueueProductionType::Unit,
            gamelogic::object::production::queue::ProductionType::Upgrade => {
                QueueProductionType::Upgrade
            }
            gamelogic::object::production::queue::ProductionType::SpecialPower => {
                QueueProductionType::Invalid
            }
        }
    }

    fn get_object_has_production(&self, obj_id: u32) -> bool {
        if !self.build_queue_data.is_empty()
            || self.portrait_state.production_progress.is_some()
            || self.portrait_state.production_template.is_some()
            || self.displayed_queue_count > 0
        {
            return true;
        }
        // Wave 249/997/1009/1046: presentation residual above; dual-world peels selected
        // producer command-set / construction / catalog factory KindOf residual.
        // Wave 1046: destroyed/sold/disabled producers fail-closed (no production UI).
        if Self::dual_world_registry_unavailable() {
            if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(obj_id)
            {
                // Wave 1070: masked/under-construction producers also fail-closed.
                if entry.destroyed
                    || entry.sold
                    || entry.disabled
                    || entry.unselectable
                    || entry.masked
                    || entry.under_construction
                {
                    return false;
                }
                // Actual production residual beats factory KindOf heuristics.
                if entry.production_template.is_some()
                    || entry.production_progress.is_some()
                    || entry.production_paused
                {
                    return true;
                }
            }
            let selected = self
                .context
                .read()
                .ok()
                .and_then(|c| c.selected_objects.first().copied());
            if selected == Some(obj_id)
                && (!self.presentation_primary_command_set.is_empty()
                    || !self.presentation_command_set_names.is_empty()
                    || self.presentation_under_construction)
            {
                // Wave 1046: still fail-closed if catalog says sold/destroyed.
                if let Some(entry) =
                    crate::presentation_translator_residual::translator_catalog_entry(obj_id)
                {
                    // Wave 1070: masked/UC/unselectable producer residual fail-closed.
                    if entry.destroyed
                        || entry.sold
                        || entry.disabled
                        || entry.unselectable
                        || entry.masked
                        || entry.under_construction
                    {
                        return false;
                    }
                }
                return true;
            }
            // Wave 1009/1015: translator catalog factory KindOf / command-set residual.
            if let Some(entry) =
                crate::presentation_translator_residual::translator_catalog_entry(obj_id)
            {
                if !entry.command_set_name.is_empty() {
                    return true;
                }
                const FACTORY_KINDS: &[&str] = &[
                    "FSBarracks",
                    "FSWarFactory",
                    "FSAirfield",
                    "CommandCenter",
                    "Structure",
                ];
                if entry.kind_names.iter().any(|k| {
                    FACTORY_KINDS
                        .iter()
                        .any(|f| k == f || k.eq_ignore_ascii_case(f))
                }) {
                    return true;
                }
            }
            return false;
        }
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return false;
        };
        let Ok(obj) = obj_arc.read() else {
            return false;
        };
        for module in obj.get_behavior_modules() {
            if let Ok(mut guard) = module.lock() {
                if guard.get_production_update_interface().is_some() {
                    return true;
                }
            }
        }
        false
    }

    fn set_object_production_paused(obj_id: u32, paused: bool) {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj) = obj_arc.read() else {
            return;
        };

        for module in obj.get_behavior_modules() {
            let Ok(mut guard) = module.lock() else {
                continue;
            };
            let Some(production) = guard.get_production_update_interface() else {
                continue;
            };
            if paused {
                production.pause_production();
            } else {
                production.resume_production();
            }
            break;
        }
    }

    fn cancel_production_by_id(obj_id: u32, production_id: u32) {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj) = obj_arc.read() else {
            return;
        };
        let queue_index = production_id as usize;

        for module in obj.get_behavior_modules() {
            let Ok(mut guard) = module.lock() else {
                continue;
            };
            let Some(production) = guard.get_production_update_interface() else {
                continue;
            };
            let _ = production.cancel_production(queue_index);
            break;
        }
    }
}
