// C++ `ControlBar::evaluateContextUI` + `populateCommand` (`ControlBar.cpp:1830-1871`,
// `ControlBarCommand.cpp:246-323`).
//
// After UNDER_CONSTRUCTION: OCLUpdate → OCLTimerWindow (hides CommandSet).
// Empty CommandSet + garrisonable → StructureInventory (10 exits + Stop/Evacuate).
// Otherwise GameHUD binds CommandSet.ini slots 1-14. Empty and SCRIPT_ONLY stay
// holes. Never invent Patrol / Scatter / Attitude / Cheer / Deploy /
// PurchaseScience or name-heuristic extras.

use super::*;
use crate::ui::{UnitCommandAvailability, UnitCommandButton};

impl PresentationFrame {
    /// Live GameHUD / unit-command-panel strip. Context first, then CommandSet.
    pub fn populate_command_set_strip(&self) -> Vec<UnitCommandButton> {
        let panel = self.control_bar_selection_panel();
        let Some(id) = panel.primary_object_id else {
            return Vec::new();
        };
        let Some(ro) = self.objects.iter().find(|o| {
            o.id == id && !o.destroyed && !o.sold && !o.unselectable && !o.masked
        }) else {
            return Vec::new();
        };
        if !self.is_owned_by_local(ro) {
            let mut cmds = Vec::new();
            let n = ro.template_name.to_ascii_lowercase();
            if n.contains("beacon") {
                cmds.push(UnitCommandButton {
                    command_name: "Command_BeaconDelete".into(),
                    enabled: true,
                    ..Default::default()
                });
                return cmds;
            }
            if ro.team == crate::game_logic::Team::Neutral && ro.max_garrison > 0 {
                return self.populate_structure_inventory_strip(ro);
            }
            return Vec::new();
        }

        if ro.under_construction {
            // C++ populateUnderConstruction — Cancel stays; script-disabled
            // wipe is for CommandSet context only (apply_host_disabled).
            return vec![UnitCommandButton {
                command_name: "Command_CancelConstruction".into(),
                enabled: true,
                ..Default::default()
            }];
        }

        // C++ ControlBar.cpp:1830-1871 — after UC, OCLUpdate replaces the 14
        // ButtonCommand slots with OCLTimerWindow (countdown + Sell/Rally).
        if ro.ocl_timer_seconds > 0 {
            return self.populate_ocl_timer_strip(ro);
        }

        // C++ ControlBar.cpp:1850-1865 — garrisonable + empty CommandSet
        // rebuilds StructureInventory (not a blank strip).
        if ro.max_garrison > 0
            && ro.command_set_name.is_empty()
            && ro.command_set_override.is_empty()
        {
            let selected = self.command_strip_selected_objects();
            if selected.len() <= 1 {
                return self.populate_structure_inventory_strip(ro);
            }
        }

        let selected = self.command_strip_selected_objects();
        let multi = selected.len() > 1;
        let slots = if multi {
            intersect_command_set_slots(&self.selected_command_set_names())
        } else {
            let cs_name = command_set_name_for_object(ro);
            command_set_slots_for(&cs_name, &ro.template_name).unwrap_or_default()
        };
        if slots.is_empty() {
            return Vec::new();
        }

        let mut cmds = Vec::with_capacity(slots.len());
        for slot in &slots {
            match slot {
                Some(command_name) => {
                    let enabled = self.command_set_slot_enabled(ro, command_name);
                    cmds.push(UnitCommandButton {
                        command_name: command_name.clone(),
                        enabled,
                        ..Default::default()
                    });
                }
                None => cmds.push(UnitCommandButton::default()),
            }
        }

        // NEED_SPECIAL_POWER_SCIENCE hides in place (C++ populateCommand).
        for cmd in cmds.iter_mut() {
            if !cmd.command_name.is_empty()
                && !self.host_need_special_power_science_owned(&cmd.command_name)
            {
                *cmd = UnitCommandButton::default();
            }
        }
        self.overlay_command_set_occupants(&mut cmds, ro);
        self.apply_command_set_upgrade_restrictions(&mut cmds, ro, &panel);
        self.apply_command_set_can_make(&mut cmds, ro);
        self.apply_host_disabled_command_strip(&mut cmds, ro);
        let eval_ids: Vec<u32> = if multi {
            selected.iter().map(|o| o.id.0).collect()
        } else {
            vec![ro.id.0]
        };
        self.apply_leftover_get_command_availability(&mut cmds, ro, &eval_ids);
        cmds
    }

    fn command_strip_selected_objects(&self) -> Vec<&RenderableObject> {
        let usable = |o: &&RenderableObject| {
            !o.destroyed && !o.sold && !o.unselectable && !o.masked
        };
        if !self.selected.is_empty() {
            self.selected
                .iter()
                .filter_map(|id| self.objects.iter().find(|o| o.id == *id && usable(o)))
                .collect()
        } else {
            self.objects
                .iter()
                .filter(|o| o.selected && usable(o))
                .collect()
        }
    }

    /// C++ `ControlBar::getCommandAvailability` via leftover ControlBar.
    fn apply_leftover_get_command_availability(
        &self,
        cmds: &mut [UnitCommandButton],
        ro: &RenderableObject,
        eval_ids: &[u32],
    ) {
        let cs_name = command_set_name_for_object(ro);
        let bar = self.leftover_availability_bar(ro, &cs_name);
        for cmd in cmds.iter_mut() {
            if cmd.command_name.is_empty() {
                continue;
            }
            let resolved = game_engine::common::ini::ini_command_button::get_control_bar()
                .and_then(|bar| bar.find_command_button_resolved(&cmd.command_name).cloned());
            if resolved.is_none() {
                // C++ ControlBar::getCommandAvailability always consults the
                // retail CommandButton definition (INI loaded at startup);
                // with no leftover definition resolved, the host slot gate
                // verdict stays authoritative instead of a placeholder default
                // that would re-enable Restricted buttons.
                continue;
            }
            if eval_ids.len() > 1 {
                // C++ updateContextMultiSelect: any COMMAND_HIDDEN hides;
                // then enable if any object is AVAILABLE/ACTIVE.
                let mut hidden = false;
                let mut can_do = 0u32;
                let mut last = game_client::gui::control_bar::CommandAvailability::Restricted;
                for id in eval_ids {
                    let avail = bar.leftover_evaluate_named_command(
                        &cmd.command_name,
                        *id,
                        self.local_player_id,
                    );
                    if matches!(
                        avail,
                        game_client::gui::control_bar::CommandAvailability::Hidden
                    ) {
                        hidden = true;
                    }
                    if matches!(
                        avail,
                        game_client::gui::control_bar::CommandAvailability::Available
                            | game_client::gui::control_bar::CommandAvailability::Active
                    ) {
                        can_do += 1;
                    }
                    last = avail;
                }
                if hidden {
                    *cmd = UnitCommandButton::default();
                    continue;
                }
                cmd.enabled = can_do > 0;
                cmd.availability = if can_do > 0 {
                    leftover_to_unit_availability(
                        game_client::gui::control_bar::CommandAvailability::Available,
                    )
                } else {
                    leftover_to_unit_availability(last)
                };
                continue;
            }
            let mut best = game_client::gui::control_bar::CommandAvailability::Hidden;
            for id in eval_ids {
                let avail = bar.leftover_evaluate_named_command(
                    &cmd.command_name,
                    *id,
                    self.local_player_id,
                );
                best = leftover_merge_command_availability(best, avail);
            }
            cmd.availability = leftover_to_unit_availability(best);
            match best {
                game_client::gui::control_bar::CommandAvailability::Hidden => {
                    *cmd = UnitCommandButton::default();
                }
                game_client::gui::control_bar::CommandAvailability::Restricted
                | game_client::gui::control_bar::CommandAvailability::NotReady
                | game_client::gui::control_bar::CommandAvailability::CantAfford => {
                    cmd.enabled = false;
                }
                game_client::gui::control_bar::CommandAvailability::Available
                | game_client::gui::control_bar::CommandAvailability::Active => {
                    cmd.enabled = true;
                }
            }
        }
        self.apply_host_restrict_a_command_strip(cmds, ro);
    }

    fn leftover_availability_bar(
        &self,
        ro: &RenderableObject,
        cs_name: &str,
    ) -> game_client::gui::control_bar::ControlBar {
        let panel = self.control_bar_selection_panel();
        let mut bar = game_client::gui::control_bar::ControlBar::new();
        let mut completed = panel.applied_upgrades.clone();
        for name in &self.local_unlocked_sciences {
            if !completed
                .iter()
                .any(|owned| owned.eq_ignore_ascii_case(name))
            {
                completed.push(name.clone());
            }
        }
        let mut residual = game_client::gui::control_bar::PresentationAvailabilityResidual {
            moving: ro.moving,
            has_production: !ro.production_queue.is_empty(),
            occupant_count: ro.occupant_count as usize,
            hacking_packing_or_unpacking: ro.hacking_packing_or_unpacking,
            overcharge_active: ro.overcharge_enabled,
            special_power_in_use: ro.using_ability,
            weapon_reload_time: ro.weapon_reload_time,
            weapon_fire_status: ro.weapon_fire_status,
            has_primary_weapon: ro.has_weapon,
            has_secondary_weapon: ro.has_secondary_weapon,
            has_tertiary_weapon: ro.projectile_clip_statuses[2].is_some()
                || ro.active_weapon_slot == 2,
            active_weapon_slot: ro.active_weapon_slot,
            battle_plan_bombardment: ro.weapon_bonus_battle_plan_bombardment,
            battle_plan_hold_the_line: ro.weapon_bonus_battle_plan_hold_the_line,
            battle_plan_search_and_destroy: ro.weapon_bonus_battle_plan_search_and_destroy,
            script_disabled: ro.disabled_script_disabled,
            script_unpowered: ro.disabled_script_underpowered,
            unmanned: ro.disabled_unmanned,
            completed_upgrades: completed.clone(),
            object_applied_upgrades: panel.applied_upgrades.clone(),
            player_completed_upgrades: self.local_completed_upgrades.clone(),
            script_unsellable: ro.script_unsellable,
            disabled_subdued: ro.disabled_subdued,
            single_use_used: ro.single_use_command_used,
            ..Default::default()
        };
        self.stamp_host_restrict_a_availability(&mut residual, Some(ro));
        bar.apply_presentation_availability(residual);
        bar.sync_command_set_from_presentation(if cs_name.is_empty() {
            None
        } else {
            Some(cs_name)
        });
        bar.sync_upgrade_cameos_from_presentation(
            &[],
            &completed,
            None,
            ro.special_power_ready,
            ro.special_power_cooldown_remaining,
            ro.special_power_cooldown,
        );
        bar
    }

    fn command_set_slot_enabled(&self, ro: &RenderableObject, name: &str) -> bool {
        let n = name.to_ascii_lowercase();
        if n.contains("combatdrop") {
            return self.host_rappeller_count(ro) > 0;
        }
        if n.contains("capture") {
            return ro.capture_power_ready && !ro.using_ability;
        }
        if n.contains("detonate") && (n.contains("charge") || n.contains("demo")) {
            return self.objects.iter().any(|charge| {
                charge.producer_id == Some(ro.id)
                    && !charge.destroyed
                    && crate::game_logic::host_mines::is_remote_demo_charge_template(
                        &charge.template_name,
                    )
            });
        }
        if (n.contains("hackinternet") || n.contains("internethack"))
            && ro.hacking_packing_or_unpacking
        {
            return false;
        }
        // C++ ControlBarCommand.cpp:1361-1372 — Evacuate Restricted at containCount<=0.
        if n.contains("evacuate") {
            return ro.normal_enter_occupant_count() > 0;
        }
        // C++ ControlBarCommand.cpp:1385-1424 — not ready => NOT_READY; in-use => RESTRICTED.
        // Live strip only had capture; PUC / snipe / leaflet stayed clickable on cooldown.
        if host_command_uses_special_power_ready(&n) {
            return ro.special_power_ready && !ro.using_ability;
        }
        true
    }

    fn apply_command_set_upgrade_restrictions(
        &self,
        cmds: &mut [UnitCommandButton],
        ro: &RenderableObject,
        panel: &crate::ui::ControlBarSelectionPanelState,
    ) {
        let queued = &self.local_queued_upgrades;
        let unlocked = &self.local_unlocked_sciences;
        let norm = |s: &str| {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .flat_map(|c| c.to_lowercase())
                .collect::<String>()
        };
        let queue_full = ro.production_queue.len()
            == crate::game_logic::host_production_buildable_command_residual::MAX_BUILD_QUEUE_BUTTONS_RESIDUAL;
        for cmd in cmds.iter_mut() {
            let cn = norm(&cmd.command_name);
            if !cn.contains("upgrade") {
                continue;
            }
            let owned = unlocked.iter().any(|x| {
                let u = norm(x);
                cn.contains(&u) || u.contains(&cn)
            });
            let researching = queued.iter().any(|x| {
                let u = norm(x);
                cn.contains(&u) || u.contains(&cn)
            }) || (panel.production_is_upgrade
                && panel
                    .production_template
                    .as_ref()
                    .map(|t| {
                        let u = norm(t);
                        cn.contains(&u) || u.contains(&cn)
                    })
                    .unwrap_or(false));
            if owned || researching || queue_full {
                cmd.enabled = false;
            }
        }
    }

    fn apply_command_set_can_make(&self, cmds: &mut Vec<UnitCommandButton>, ro: &RenderableObject) {
        if self.can_make_producer_id != Some(ro.id.0) {
            return;
        }
        let queue_full = ro.production_queue.len()
            == crate::game_logic::host_production_buildable_command_residual::MAX_BUILD_QUEUE_BUTTONS_RESIDUAL;
        for cmd in cmds.iter_mut() {
            let cn = cmd.command_name.to_ascii_lowercase();
            if !cn.contains("construct") {
                continue;
            }
            if self.can_make_cameos.iter().any(|cameo| {
                cameo.buildable_hidden && cn.contains(&cameo.template_name.to_ascii_lowercase())
            }) {
                *cmd = UnitCommandButton::default();
            }
        }
        for cmd in cmds.iter_mut() {
            let cn = cmd.command_name.to_ascii_lowercase();
            if !cn.contains("construct") {
                continue;
            }
            if ro.is_dozer_task_pending {
                cmd.enabled = false;
                continue;
            }
            if queue_full {
                cmd.enabled = false;
                continue;
            }
            if let Some(cameo) = self
                .can_make_cameos
                .iter()
                .find(|cameo| cn.contains(&cameo.template_name.to_ascii_lowercase()))
            {
                cmd.enabled = cameo.available;
            }
        }
    }

    fn overlay_command_set_occupants(
        &self,
        cmds: &mut [UnitCommandButton],
        ro: &RenderableObject,
    ) {
        // C++ doTransportInventoryUI: empty EXIT_CONTAINER starts disabled;
        // extraSlotsInUse hides overflow slots; unmanned hides all exits.
        let extra_slots: usize = ro
            .garrisoned_units
            .iter()
            .filter_map(|id| self.objects.iter().find(|o| o.id == *id && !o.destroyed))
            .map(|o| o.transport_slot_count.max(1).saturating_sub(1))
            .sum();
        let transport_max = if ro.max_garrison > 0 {
            ro.max_garrison.saturating_sub(extra_slots)
        } else {
            usize::MAX
        };
        let unmanned = ro.disabled_unmanned;
        let occupants = self.presentation_inventory_occupants(ro);
        let mut occ_iter = occupants.into_iter();
        let mut inventory_command_count = 0usize;
        for cmd in cmds.iter_mut() {
            let n = cmd.command_name.to_ascii_lowercase();
            let is_exit = (n.contains("exit") || n.contains("transport"))
                && !n.contains("evacuate")
                && !n.contains("beacon");
            if !is_exit {
                continue;
            }
            inventory_command_count += 1;
            cmd.enabled = false;
            cmd.exit_object_id = None;
            cmd.overlay_image = None;
            if unmanned || inventory_command_count > transport_max {
                *cmd = UnitCommandButton::default();
                continue;
            }
            let Some(occ) = occ_iter.next() else {
                continue;
            };
            if occ.object_id != 0 {
                cmd.exit_object_id = Some(occ.object_id);
                cmd.enabled = true;
            }
            if !occ.button_image.is_empty() {
                cmd.button_image = occ.button_image;
            }
            cmd.overlay_image = occ.overlay_image;
        }
    }

    fn presentation_inventory_occupants(
        &self,
        ro: &RenderableObject,
    ) -> Vec<game_client::gui::control_bar::StructureInventoryOccupant> {
        ro.garrisoned_units
            .iter()
            .filter_map(|id| {
                let occ = self.objects.iter().find(|o| o.id == *id && !o.destroyed)?;
                Some(game_client::gui::control_bar::occupant_from_presentation(
                    occ.id.0,
                    &occ.template_name,
                    presentation_veterancy_overlay(occ.veterancy),
                ))
            })
            .collect()
    }

    /// C++ `populateOCLTimer` — countdown display plus Sell or SetRallyPoint.
    fn populate_ocl_timer_strip(&self, ro: &RenderableObject) -> Vec<UnitCommandButton> {
        use game_client::gui::control_bar::control_bar_ocl_timer::{
            format_ocl_timer_display, ocl_timer_button_kind, ocl_timer_kind_for_object,
            OclTimerButtonKind,
        };
        let leftover_kind = ocl_timer_kind_for_object(ro.id.0);
        let kind = if leftover_kind != OclTimerButtonKind::Hidden {
            leftover_kind
        } else {
            let is_tech = Self::object_has_kind(ro, crate::game_logic::KindOf::TechBuilding);
            let is_rally = Self::object_has_kind(ro, crate::game_logic::KindOf::AutoRallypoint);
            if is_tech || is_rally {
                ocl_timer_button_kind(is_tech, is_rally)
            } else {
                // Unknown leftover + no presentation tech bits: supply-drop Sell.
                OclTimerButtonKind::Sell
            }
        };
        let (text, _) = format_ocl_timer_display(ro.ocl_timer_seconds, 0.0);
        let mut cmds = vec![UnitCommandButton {
            command_name: String::new(),
            enabled: false,
            button_image: text,
            ..Default::default()
        }];
        match kind {
            OclTimerButtonKind::Sell => cmds.push(UnitCommandButton {
                command_name: "Command_Sell".into(),
                enabled: true,
                ..Default::default()
            }),
            OclTimerButtonKind::RallyPoint => cmds.push(UnitCommandButton {
                command_name: "Command_SetRallyPoint".into(),
                enabled: true,
                ..Default::default()
            }),
            OclTimerButtonKind::Hidden => {}
        }
        // C++ populateOCLTimer does not run getCommandAvailability wipe.
        cmds
    }

    /// C++ `populateStructureInventory` — slots 0-9 StructureExit + Stop/Evacuate.
    fn populate_structure_inventory_strip(&self, ro: &RenderableObject) -> Vec<UnitCommandButton> {
        use game_client::gui::control_bar::{
            StructureInventoryOccupant, MAX_STRUCTURE_INVENTORY_BUTTONS,
            STRUCTURE_INVENTORY_EVACUATE_COMMAND_NAME, STRUCTURE_INVENTORY_EVACUATE_ID,
            STRUCTURE_INVENTORY_EXIT_COMMAND_NAME, STRUCTURE_INVENTORY_STOP_COMMAND_NAME,
            STRUCTURE_INVENTORY_STOP_ID,
        };
        let mut occupants = self.presentation_inventory_occupants(ro);
        let max_capacity = ro.max_garrison.max(occupants.len());
        if occupants.is_empty() {
            let count = ro
                .garrisoned_units
                .len()
                .max(ro.occupant_count as usize)
                .min(max_capacity);
            occupants.resize(count, StructureInventoryOccupant::default());
        }
        let slot_count = max_capacity.min(MAX_STRUCTURE_INVENTORY_BUTTONS);
        let contained = occupants.len();
        let mut cmds = Vec::with_capacity(14);
        for i in 0..14 {
            if i < MAX_STRUCTURE_INVENTORY_BUTTONS {
                if i < slot_count {
                    let occ = occupants.get(i);
                    cmds.push(UnitCommandButton {
                        command_name: STRUCTURE_INVENTORY_EXIT_COMMAND_NAME.into(),
                        enabled: occ.is_some(),
                        exit_object_id: occ.and_then(|o| {
                            (o.object_id != 0).then_some(o.object_id)
                        }),
                        button_image: occ
                            .map(|o| o.button_image.clone())
                            .unwrap_or_default(),
                        overlay_image: occ.and_then(|o| o.overlay_image.clone()),
                        ..Default::default()
                    });
                } else {
                    cmds.push(UnitCommandButton::default());
                }
            } else if i == STRUCTURE_INVENTORY_STOP_ID {
                cmds.push(UnitCommandButton {
                    command_name: STRUCTURE_INVENTORY_STOP_COMMAND_NAME.into(),
                    enabled: contained > 0,
                    ..Default::default()
                });
            } else if i == STRUCTURE_INVENTORY_EVACUATE_ID {
                cmds.push(UnitCommandButton {
                    command_name: STRUCTURE_INVENTORY_EVACUATE_COMMAND_NAME.into(),
                    enabled: contained > 0,
                    ..Default::default()
                });
            } else {
                cmds.push(UnitCommandButton::default());
            }
        }
        self.apply_host_disabled_command_strip(&mut cmds, ro);
        cmds
    }
}

fn presentation_veterancy_overlay(level: PresentationVeterancy) -> Option<String> {
    match level {
        PresentationVeterancy::Veteran => Some("SSChevron1L".to_string()),
        PresentationVeterancy::Elite => Some("SSChevron2L".to_string()),
        PresentationVeterancy::Heroic => Some("SSChevron3L".to_string()),
        _ => None,
    }
}

fn leftover_merge_command_availability(
    a: game_client::gui::control_bar::CommandAvailability,
    b: game_client::gui::control_bar::CommandAvailability,
) -> game_client::gui::control_bar::CommandAvailability {
    use game_client::gui::control_bar::CommandAvailability::*;
    let rank = |v| match v {
        Hidden => 0u8,
        Restricted => 1,
        CantAfford => 2,
        NotReady => 3,
        Active => 4,
        Available => 5,
    };
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

fn leftover_to_unit_availability(
    a: game_client::gui::control_bar::CommandAvailability,
) -> UnitCommandAvailability {
    use game_client::gui::control_bar::CommandAvailability::*;
    match a {
        Hidden => UnitCommandAvailability::Hidden,
        Restricted => UnitCommandAvailability::Restricted,
        NotReady => UnitCommandAvailability::NotReady,
        CantAfford => UnitCommandAvailability::CantAfford,
        Active => UnitCommandAvailability::Active,
        Available => UnitCommandAvailability::Available,
    }
}

fn command_set_name_for_object(ro: &RenderableObject) -> String {
    if !ro.command_set_name.is_empty() {
        ro.command_set_name.clone()
    } else if !ro.command_set_override.is_empty() {
        ro.command_set_override.clone()
    } else {
        residual_command_set_name_for_template(&ro.template_name)
            .unwrap_or("")
            .to_string()
    }
}

/// C++ `ControlBar::populateCommand` — 14 WND slots, empty/SCRIPT_ONLY stay holes.
/// `CommandSet::getCommandButton` consults leftover `GameLogic::findControlBarOverride`
/// before INI / residual buttons so COMMANDBAR_ADD/REMOVE hide or replace slots.
fn command_set_slots_for(set_name: &str, template_name: &str) -> Option<Vec<Option<String>>> {
    let mut slots = command_set_slots_from_ini(set_name)
        .or_else(|| residual_command_set_slots(set_name, template_name))?;
    apply_leftover_control_bar_overrides(set_name, &mut slots);
    Some(slots)
}

/// C++ `CommandSet::getCommandButton` (`ControlBar.cpp:810-817`).
fn apply_leftover_control_bar_overrides(set_name: &str, slots: &mut [Option<String>]) {
    if set_name.is_empty() {
        return;
    }
    let Ok(logic) = gamelogic::system::game_logic::lock_game_logic() else {
        return;
    };
    for (i, slot) in slots.iter_mut().enumerate() {
        match logic.find_control_bar_override(set_name, i as i32) {
            None => {}
            Some(None) => *slot = None,
            Some(Some(name)) => {
                *slot = if command_is_script_only(name) {
                    None
                } else {
                    Some(name.to_string())
                };
            }
        }
    }
}

fn command_set_slots_from_ini(set_name: &str) -> Option<Vec<Option<String>>> {
    if set_name.is_empty() {
        return None;
    }
    game_engine::common::ini::ini_command_set::initialize_command_set_manager();
    let manager = game_engine::common::ini::ini_command_set::get_command_set_manager()?;
    let set = manager.find_command_set_resolved(set_name).or_else(|| {
        manager
            .iter_resolved_sets()
            .into_iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(set_name))
            .map(|(_, set)| set)
    })?;
    if set.is_empty() {
        return None;
    }
    let mut slots = vec![None; 14];
    for (i, button) in set.buttons.iter().take(14).enumerate() {
        match button {
            Some(name) if !command_is_script_only(name) => {
                slots[i] = Some(name.clone());
            }
            _ => {}
        }
    }
    Some(slots)
}

fn residual_command_set_slots(set_name: &str, template_name: &str) -> Option<Vec<Option<String>>> {
    let aliased = if set_name.eq_ignore_ascii_case("CommandSetAmericaDozer") {
        "AmericaDozerCommandSet"
    } else {
        set_name
    };
    for (name, template, pack) in HUD_COMMAND_SET_RESIDUAL_PACKS {
        if name.eq_ignore_ascii_case(aliased)
            || template.eq_ignore_ascii_case(template_name)
            || template.eq_ignore_ascii_case(set_name)
        {
            let mut slots = vec![None; 14];
            for (idx, button) in *pack {
                let i = (*idx as usize).saturating_sub(1);
                if i < 14 && !command_is_script_only(button) {
                    slots[i] = Some((*button).to_string());
                }
            }
            if slots.iter().all(|s| s.is_none()) {
                return None;
            }
            return Some(slots);
        }
    }
    None
}

/// C++ `ControlBar::addCommonCommands` — slot intersection, OK_FOR_MULTI_SELECT only.
fn intersect_command_set_slots(set_names: &[String]) -> Vec<Option<String>> {
    let mut common = vec![None; 14];
    let mut saw_first = false;
    for name in set_names {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let Some(slots) = command_set_slots_for(name, name) else {
            // C++ clears the shared set when a selected object has no CommandSet.
            return vec![None; 14];
        };
        if !saw_first {
            for i in 0..14 {
                if let Some(cmd) = slots.get(i).and_then(|s| s.as_deref()) {
                    if command_ok_for_multi_select(cmd) {
                        common[i] = Some(cmd.to_string());
                    }
                }
            }
            saw_first = true;
            continue;
        }
        for i in 0..14 {
            let incoming = slots.get(i).and_then(|s| s.as_deref());
            let existing = common[i].as_deref();
            let attack = incoming.is_some_and(command_is_attack_move)
                || existing.is_some_and(command_is_attack_move);
            if attack && common[i].is_none() {
                common[i] = incoming.map(str::to_string);
                continue;
            }
            if attack {
                continue;
            }
            let matches = match (incoming, existing) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                (None, None) => true,
                _ => false,
            };
            if !matches {
                common[i] = None;
            }
        }
    }
    if !saw_first {
        return Vec::new();
    }
    common
}

fn command_is_attack_move(name: &str) -> bool {
    name.to_ascii_lowercase().contains("attackmove")
}

fn command_ok_for_multi_select(name: &str) -> bool {
    if let Some(button) = game_engine::common::ini::ini_command_button::get_control_bar()
        .and_then(|bar| bar.find_command_button_resolved(name).cloned())
    {
        return button.options.ok_for_multi_select;
    }
    let n = name.to_ascii_lowercase();
    n.contains("attackmove")
        || n.contains("guard")
        || n.ends_with("stop")
        || n.contains("evacuate")
}

fn command_set_buttons_for(set_name: &str) -> Option<Vec<String>> {
    command_set_slots_from_ini(set_name).map(|slots| {
        slots.into_iter().flatten().collect()
    })
}

fn residual_command_set_buttons(set_name: &str, template_name: &str) -> Option<Vec<String>> {
    residual_command_set_slots(set_name, template_name).map(|slots| {
        slots.into_iter().flatten().collect()
    })
}

fn command_is_script_only(name: &str) -> bool {
    if name.to_ascii_uppercase().contains("FAKECOMMAND") {
        return true;
    }
    game_engine::common::ini::ini_command_button::get_control_bar()
        .and_then(|bar| bar.find_command_button_resolved(name).cloned())
        .is_some_and(|button| button.options.scripted_only)
}

/// C++ GUI_COMMAND_SPECIAL_POWER* readiness. Capture/detonate/drop stay on their own gates.
fn host_command_uses_special_power_ready(n: &str) -> bool {
    n.contains("particleuplink")
        || n.contains("particlecannon")
        || n.contains("leafletdrop")
        || n.contains("snipe")
        || n.contains("specialpower")
        || n.contains("spysatellite")
        || n.contains("spectregunship")
        || n.contains("a10thunderbolt")
        || n.contains("scudstorm")
        || n.contains("nuclearmissile")
        || n.contains("neutronmissile")
        || n.contains("cashhack")
        || n.contains("sneakattack")
        || n.contains("ciaintelligence")
        || n.contains("gpsscrambler")
}

fn residual_command_set_name_for_template(template_name: &str) -> Option<&'static str> {
    HUD_COMMAND_SET_RESIDUAL_PACKS
        .iter()
        .find(|(_, template, _)| template.eq_ignore_ascii_case(template_name))
        .map(|(set_name, _, _)| *set_name)
}

/// Retail CommandSet.ini slots 1-14 used when the live INI manager is empty.
const HUD_COMMAND_SET_RESIDUAL_PACKS: &[(&str, &str, &[(u8, &str)])] = &[
    (
        "AmericaInfantryRangerCommandSet",
        "AmericaInfantryRanger",
        &[
            (1, "Command_AmericaRangerCaptureBuilding"),
            (2, "Command_AmericaRangerSwitchToMachineGun"),
            (4, "Command_AmericaRangerSwitchToFlagBangGrenades"),
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "AmericaVehicleSentryDroneCommandSet",
        "AmericaVehicleSentryDrone",
        &[
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "AmericaVehicleChinookCommandSet",
        "AmericaVehicleChinook",
        &[
            (1, "Command_TransportExit"),
            (9, "Command_ChinookUnload"),
            (10, "Command_CombatDrop"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "AmericaInfantryColonelBurtonCommandSet",
        "AmericaInfantryColonelBurton",
        &[
            (1, "Command_ColonelBurtonKnifeAttack"),
            (2, "Command_ColonelBurtonTimedDemoCharge"),
            (4, "Command_ColonelBurtonRemoteDemoCharge"),
            (6, "Command_ColonelBurtonDetonateCharges"),
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "AmericaBarracksCommandSet",
        "AmericaBarracks",
        &[
            (1, "Command_ConstructAmericaInfantryRanger"),
            (7, "Command_UpgradeAmericaRangerFlashBangGrenade"),
            (8, "Command_UpgradeAmericaRangerCaptureBuilding"),
            (13, "Command_SetRallyPoint"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaWarFactoryCommandSet",
        "AmericaWarFactory",
        &[
            (1, "Command_ConstructAmericaTankCrusader"),
            (13, "Command_SetRallyPoint"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaPowerPlantCommandSet",
        "AmericaPowerPlant",
        &[
            (1, "Command_UpgradeAmericaAdvancedControlRods"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "ChinaPowerPlantCommandSet",
        "ChinaPowerPlant",
        &[
            (1, "Command_Overcharge"),
            (12, "Command_UpgradeChinaMines"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaParticleUplinkCannonCommandSet",
        "AmericaParticleCannonUplink",
        &[
            (1, "Command_FireParticleUplinkCannon"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaCommandCenterCommandSet",
        "AmericaCommandCenter",
        &[
            (1, "Command_ConstructAmericaDozer"),
            (2, "Command_SpectreGunship"),
            (4, "Command_LeafletDrop"),
            (5, "Command_A10ThunderboltMissileStrike"),
            (10, "Command_SpySatelliteScan"),
            (13, "Command_SetRallyPoint"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaStrategyCenterCommandSet",
        "AmericaStrategyCenter",
        &[
            (1, "Command_InitiateBattlePlanBombardment"),
            (2, "Command_CIAIntelligence"),
            (3, "Command_InitiateBattlePlanHoldTheLine"),
            (5, "Command_InitiateBattlePlanSearchAndDestroy"),
            (11, "Command_StrategyCenter_Stop"),
            (14, "Command_Sell"),
        ],
    ),
    (
        "AmericaVehicleHumveeCommandSet",
        "AmericaVehicleHumvee",
        &[
            (4, "Command_TransportExit"),
            (9, "Command_Evacuate"),
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "GLAInfantryJarmenKellCommandSet",
        "GLAInfantryJarmenKell",
        &[
            (1, "Command_GLAInfantryJarmenKellSnipeVehicleAttack"),
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "ChinaInfantryBlackLotusCommandSet",
        "ChinaInfantryBlackLotus",
        &[
            (1, "Command_ChinaInfantryBlackLotusCaptureHack"),
            (3, "Command_ChinaInfantryBlackLotusVehicleHack"),
            (5, "Command_ChinaInfantryBlackLotusCashHack"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "GLAInfantryHijackerCommandSet",
        "GLAInfantryHijacker",
        &[(1, "Command_GLAInfantryHijack"), (14, "Command_Stop")],
    ),
    (
        "AmericaVehicleAmbulanceCommandSet",
        "AmericaVehicleAmbulance",
        &[
            (4, "Command_TransportExit"),
            (8, "Command_Evacuate"),
            (10, "Command_AmbulanceCleanupArea"),
            (11, "Command_AttackMove"),
            (13, "Command_Guard"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "ChinaInfantryHackerCommandSet",
        "ChinaInfantryHacker",
        &[
            (1, "Command_ChinaInfantryHackerDisableBuilding"),
            (3, "Command_ChinaInfantryHackerInternetHack"),
            (14, "Command_Stop"),
        ],
    ),
    (
        "AmericaDozerCommandSet",
        "AmericaVehicleDozer",
        &[
            (1, "Command_ConstructAmericaPowerPlant"),
            (14, "Command_DisarmMinesAtPosition"),
        ],
    ),
];
