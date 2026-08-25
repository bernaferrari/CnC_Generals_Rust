//! Delivery of live GameClient Control Bar actions into Main's offline world.
//!
//! GameClient keeps the original C++ message-stream route for standalone use.
//! The executable deliberately has a different, Rust-owned simulation world,
//! so an enabled bridge must translate the typed UI request here instead of
//! allowing a click to mutate an unused legacy GameLogic singleton.

#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]

use super::*;
use game_client::gui::control_bar::{
    HostControlBarRequest, HostControlBarTarget, QueueProductionType,
    take_host_control_bar_published_requests,
};
use gamelogic::commands::CommandType as LegacyCommandType;

const COMMAND_OPTION_ONE: u32 = 0x0000_2000;
const COMMAND_OPTION_TWO: u32 = 0x0000_4000;
const COMMAND_OPTION_THREE: u32 = 0x0000_8000;

/// Result of reconciling C++'s single active popup with Main's independent
/// pause owners.  A popup is allowed to release only a pause that Main marked
/// as its own; a pre-existing pause is deliberately left untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PopupPauseTransition {
    popup_pause_owned: bool,
    set_paused: Option<bool>,
}

#[inline]
fn resolve_popup_pause_transition(
    popup_pause_owned: bool,
    game_paused: bool,
    active_popup_pauses: bool,
    independent_pause_owner: bool,
) -> PopupPauseTransition {
    if independent_pause_owner {
        return PopupPauseTransition {
            popup_pause_owned: false,
            set_paused: None,
        };
    }

    if active_popup_pauses {
        if popup_pause_owned {
            return PopupPauseTransition {
                popup_pause_owned: true,
                set_paused: (!game_paused).then_some(true),
            };
        }

        // A host world that was already paused has an owner Main cannot prove
        // belongs to this popup.  Preserve it instead of claiming/releasing it
        // on a later acknowledgement.
        if game_paused {
            return PopupPauseTransition {
                popup_pause_owned: false,
                set_paused: None,
            };
        }

        return PopupPauseTransition {
            popup_pause_owned: true,
            set_paused: Some(true),
        };
    }

    if popup_pause_owned {
        return PopupPauseTransition {
            popup_pause_owned: false,
            set_paused: Some(false),
        };
    }

    PopupPauseTransition {
        popup_pause_owned: false,
        set_paused: None,
    }
}

#[inline]
fn host_control_bar_request_allowed_in_state(
    in_game: bool,
    paused_popup_ack_only: bool,
    request: &HostControlBarRequest,
) -> bool {
    in_game
        || (paused_popup_ack_only
            && matches!(
                request,
                HostControlBarRequest::DismissInGamePopupMessage { .. }
            ))
}

/// Network/replay modes stay on the crate GameMessage route.
///
/// AGENTS.md defers GameNetwork until non-network parity is done; the live
/// product match is offline Skirmish (or SinglePlayer campaign). C++
/// `ControlBar::processCommand` (ControlBarCommandProcessing.cpp) appends
/// the same `MSG_*` for every mode — Rust only hosts the offline world.
#[inline]
fn host_control_bar_bridge_is_network_or_replay(mode: Option<crate::game_logic::GameMode>) -> bool {
    matches!(
        mode,
        Some(
            crate::game_logic::GameMode::Multiplayer
                | crate::game_logic::GameMode::Internet
                | crate::game_logic::GameMode::Lan
                | crate::game_logic::GameMode::Replay
        )
    )
}

#[inline]
fn host_control_bar_bridge_is_offline_product_mode(
    mode: Option<crate::game_logic::GameMode>,
) -> bool {
    matches!(
        mode,
        Some(crate::game_logic::GameMode::SinglePlayer | crate::game_logic::GameMode::Skirmish)
    )
}

/// A host popup acknowledgement is meaningful only for the one live retained
/// popup.  Zero and missing identities fail closed; in particular, a queued
/// ButtonOk/Esc for old popup A must never consume replacement popup B.
#[inline]
fn popup_acknowledgement_matches_active_generation(
    active_popup_generation: Option<usize>,
    popup_generation: usize,
) -> bool {
    popup_generation != 0 && active_popup_generation == Some(popup_generation)
}

impl CnCGameEngine {
    /// Drain Control Bar work at the single-authority boundary.
    ///
    /// Requests are validated against the immutable presentation snapshot
    /// before they are allowed to alter Main's host world.  That prevents a
    /// stale UI context, an enemy id, or a legacy object id from being treated
    /// as an authority command merely because it reached this queue.
    pub(crate) fn host_tick_control_bar_bridge(&mut self) {
        let requests = take_host_control_bar_published_requests();
        if requests.is_empty() {
            return;
        }

        let in_game = matches!(self.current_state, GameState::InGame);
        let paused_popup_ack_only = matches!(self.current_state, GameState::Paused);
        if !in_game && !paused_popup_ack_only {
            debug!(
                "discarded {} Control Bar requests outside an in-game offline world",
                requests.len()
            );
            return;
        }

        // Multiplayer/replay ownership remains deferred (AGENTS.md). Switch
        // the client back to its own legacy route rather than silently
        // applying commands to an offline-only host world.
        if host_control_bar_bridge_is_network_or_replay(self.host_match_game_mode) {
            warn!(
                "Control Bar host bridge is offline-only; restoring the legacy route for this session"
            );
            game_client::gui::control_bar::set_host_control_bar_bridge_enabled(false);
            return;
        }

        for published in requests {
            // A popup can own a pause while the runtime is in Paused.  Its
            // acknowledgement must reach Main, but every world-mutating
            // ControlBar action remains InGame-only.
            if !host_control_bar_request_allowed_in_state(
                in_game,
                paused_popup_ack_only,
                &published.request,
            ) {
                debug!("discarded non-popup Control Bar request while paused");
                continue;
            }
            self.host_apply_control_bar_request(
                published.request,
                published.input_provenance.is_physical_window_mouse_input(),
            );
        }
    }

    fn host_apply_control_bar_request(
        &mut self,
        request: HostControlBarRequest,
        physical_os_input: bool,
    ) {
        match request {
            HostControlBarRequest::SelectNextIdleWorker => {
                self.host_select_next_idle_worker_from_control_bar();
            }
            HostControlBarRequest::CancelStructurePlacement => {
                self.cancel_structure_placement_from_ui();
            }
            HostControlBarRequest::DismissInGamePopupMessage { popup_generation } => {
                self.host_dismiss_in_game_popup_message(popup_generation);
            }
            HostControlBarRequest::Production {
                template_name,
                producer_ids,
                player_id,
                ..
            } => {
                let mut producers = self.host_control_bar_local_selection(player_id, &producer_ids);
                producers.retain(|id| self.ui_object_can_produce(*id));
                if template_name.trim().is_empty() || producers.is_empty() {
                    self.host_reject_control_bar_request(
                        "production requires a local live producer",
                    );
                    return;
                }
                self.host_set_selection(player_id, producers);
                self.queue_unit_production_from_ui(
                    &template_name,
                    self.host_control_bar_production_quantity(),
                );
                self.interactive_playability.note_control_bar_production(
                    self.host_control_bar_evidence_eligible(physical_os_input),
                );
            }
            HostControlBarRequest::Upgrade {
                upgrade_name,
                selected_object_ids,
                player_id,
                ..
            } => {
                let selected =
                    self.host_control_bar_local_selection(player_id, &selected_object_ids);
                if upgrade_name.trim().is_empty() || selected.is_empty() {
                    self.host_reject_control_bar_request(
                        "upgrade requires local selected object(s)",
                    );
                    return;
                }
                self.host_set_selection(player_id, selected.clone());
                self.host_control_bar_queue_command(
                    player_id,
                    selected,
                    crate::command_system::CommandType::QueueUpgrade { upgrade_name },
                );
            }
            HostControlBarRequest::DirectCommand {
                command_name,
                command_type,
                options,
                weapon_slot,
                max_shots_to_fire: _,
                object_name,
                upgrade_name: _,
                selected_object_ids,
                player_id,
                source: _,
                science_names,
                special_power_name,
                special_power_id: _,
                exit_object_id,
            } => self.host_apply_control_bar_direct(
                &command_name,
                command_type,
                options,
                weapon_slot,
                object_name.as_deref(),
                &selected_object_ids,
                player_id,
                &science_names,
                special_power_name.as_deref(),
                exit_object_id,
            ),
            HostControlBarRequest::ArmTarget {
                command_name,
                command_type,
                options,
                weapon_slot,
                max_shots_to_fire,
                object_name: _,
                upgrade_name: _,
                selected_object_ids,
                player_id,
                source: _,
                source_object_id: _,
                target,
            } => self.host_apply_control_bar_target(
                &command_name,
                command_type,
                options,
                weapon_slot,
                max_shots_to_fire,
                &selected_object_ids,
                player_id,
                target,
                physical_os_input,
            ),
            HostControlBarRequest::QueueCancel {
                player_id,
                selected_object_ids,
                producer_id,
                production_id,
                production_type,
                upgrade_name,
                queue_index,
            } => self.host_apply_control_bar_cancel(
                player_id,
                &selected_object_ids,
                crate::game_logic::ObjectId(producer_id),
                production_id,
                production_type,
                &upgrade_name,
                queue_index,
            ),
            HostControlBarRequest::ProductionPause {
                player_id,
                selected_object_ids,
                producer_id,
                paused,
            } => {
                let producer = crate::game_logic::ObjectId(producer_id);
                let selected =
                    self.host_control_bar_local_selection(player_id, &selected_object_ids);
                if !selected.contains(&producer) || !self.ui_object_can_produce(producer) {
                    self.host_reject_control_bar_request(
                        "production pause requires its local producer selection",
                    );
                    return;
                }
                self.host_set_selection(player_id, selected);
                if !self
                    .host_game_logic_mut()
                    .set_production_paused(producer, paused)
                {
                    self.host_reject_control_bar_request(
                        "selected producer does not own a production queue",
                    );
                }
            }
        }
    }

    /// Main owns the only offline world, so consume the single active popup
    /// residual before clearing the GameClient WND.  The mission-script queue
    /// was already consumed at its normal evaluator boundary; this is only
    /// the C++-equivalent presentation mirror.
    fn host_dismiss_in_game_popup_message(&mut self, popup_generation: usize) {
        if !popup_acknowledgement_matches_active_generation(
            self.game_logic.active_popup_message_generation(),
            popup_generation,
        ) {
            debug!(
                "ignored stale InGamePopupMessage acknowledgement generation {}",
                popup_generation
            );
            return;
        }

        self.game_logic.take_popup_message_requests();
        self.host_clear_active_popup_presentation_residual();
        // The frozen record is a presentation mirror and can have been
        // replaced/drained by the time its WND callback arrives. Reconcile
        // unconditionally so a stale popup-owned marker cannot survive an
        // acknowledgement; the resolver preserves every independent pause.
        self.host_reconcile_active_popup_pause(None);

        // The host has atomically removed the authoritative record first;
        // this cleanup only destroys the client-owned WND/presentation data.
        game_client::helpers::TheInGameUI::clear_popup_message_data();
    }

    /// A full offline-world boundary invalidates both C++'s visible popup WND
    /// and Main's mirror before a new map/reset world can accept UI input.
    /// Keep this narrower than a full Control Bar clear: only popup ACKs are
    /// invalidated, while unrelated controls retain their own boundary rules.
    pub(super) fn host_invalidate_active_popup_for_world_boundary(&mut self) {
        self.game_logic.take_popup_message_requests();
        self.host_clear_active_popup_presentation_residual();
        self.host_reconcile_active_popup_pause(None);
        game_client::gui::control_bar::clear_host_dismiss_in_game_popup_message_requests();
        game_client::helpers::TheInGameUI::clear_popup_message_data();
    }

    fn host_clear_active_popup_presentation_residual(&mut self) {
        if let Some(pres) = self.render_pipeline.presentation_frame_mut() {
            pres.pending_popup_messages.clear();
        }
        if let Some(pres) = self.last_presentation_frame.as_mut() {
            pres.pending_popup_messages.clear();
        }
        if let Some(ui) = self.last_ui_state.as_mut() {
            ui.pending_popup_messages.clear();
        }
    }

    /// Keep one active presentation popup's pause separate from Main's other
    /// pause owners.  `None` means the active popup was dismissed.
    pub(crate) fn host_reconcile_active_popup_pause(&mut self, active_popup_pauses: Option<bool>) {
        let transition = resolve_popup_pause_transition(
            self.popup_host_pause_owned,
            self.game_paused,
            active_popup_pauses.unwrap_or(false),
            self.host_popup_has_independent_pause_owner(),
        );
        self.popup_host_pause_owned = transition.popup_pause_owned;
        if let Some(paused) = transition.set_paused {
            self.host_set_paused(paused);
        }
    }

    fn host_popup_has_independent_pause_owner(&self) -> bool {
        self.quit_menu_host_active
            || self.match_over
            || matches!(
                self.current_state,
                GameState::Menu
                    | GameState::Loading
                    | GameState::Victory
                    | GameState::Defeat
                    | GameState::Exiting
            )
            || matches!(
                self.ui_manager.current_screen(),
                Some(Screen::PauseMenu | Screen::Victory)
            )
    }

    /// C++ ControlBarCommandProcessing.cpp:167-178 — first SINGLE_USE click greys the strip.
    fn host_mark_single_use_command_if_needed(
        &mut self,
        options: u32,
        selected: impl IntoIterator<Item = crate::game_logic::ObjectId>,
    ) {
        if options
            & crate::game_logic::host_production_buildable_command_residual::COMMAND_OPTION_SINGLE_USE_COMMAND
            == 0
        {
            return;
        }
        for id in selected {
            if let Some(obj) = self.game_logic.host_object_mut(id) {
                obj.mark_single_use_command_used();
            }
        }
    }

    fn host_apply_control_bar_direct(
        &mut self,
        command_name: &str,
        command_type: LegacyCommandType,
        options: u32,
        weapon_slot: Option<u32>,
        object_name: Option<&str>,
        selected_object_ids: &[u32],
        player_id: u32,
        science_names: &[String],
        special_power_name: Option<&str>,
        exit_object_id: Option<u32>,
    ) {
        if !self.host_control_bar_is_local_player(player_id) {
            return;
        }

        self.host_mark_single_use_command_if_needed(
            options,
            selected_object_ids
                .iter()
                .copied()
                .map(crate::game_logic::ObjectId),
        );

        if command_type == LegacyCommandType::PurchaseScience {
            let Some(science_name) = science_names.iter().find(|name| {
                !name.trim().is_empty()
                    && self
                        .host_game_logic()
                        .player_can_purchase_science(player_id, name)
            }) else {
                self.host_reject_control_bar_request(
                    "science button has no purchasable exact science",
                );
                return;
            };
            self.host_control_bar_queue_command(
                player_id,
                Vec::new(),
                crate::command_system::CommandType::PurchaseScience {
                    science_name: science_name.clone(),
                },
            );
            return;
        }

        if command_type == LegacyCommandType::MetaSelectMatchingUnits {
            let Some(template_name) = object_name.filter(|name| !name.trim().is_empty()) else {
                self.host_reject_control_bar_request(
                    "select-matching-units lacks an exact template name",
                );
                return;
            };
            let Some(frame) = self.last_presentation_frame.as_ref() else {
                self.host_reject_control_bar_request(
                    "select-matching-units requires an in-game presentation frame",
                );
                return;
            };
            let selected: Vec<_> = frame
                .objects
                .iter()
                .filter(|object| {
                    object.team == frame.local_team()
                        && object.template_name.eq_ignore_ascii_case(template_name)
                        && crate::unit_control::UnitControlSystem::presentation_is_selectable(
                            object,
                        )
                })
                .map(|object| object.id)
                .collect();
            if selected.is_empty() {
                self.host_reject_control_bar_request(
                    "select-matching-units found no local selectable matches",
                );
                return;
            }
            self.host_set_selection(player_id, selected);
            return;
        }

        let selected = self.host_control_bar_local_selection(player_id, selected_object_ids);
        if selected.is_empty()
            && !self.host_control_bar_command_allows_empty_selection(command_type)
        {
            self.host_reject_control_bar_request(
                "direct command has no local selectable selection",
            );
            return;
        }
        if !selected.is_empty() {
            self.host_set_selection(player_id, selected.clone());
        }

        // C++ GameLogicDispatch.cpp:978-1004 MSG_EXIT: argument 0 is the
        // occupant (`m_containData.objectID`). Fail-closed without it — do
        // not dump the container (that is MSG_EVACUATE).
        if command_type == LegacyCommandType::Exit {
            let Some(occupant) = exit_object_id.filter(|id| *id != 0) else {
                self.host_reject_control_bar_request("EXIT requires occupant object id");
                return;
            };
            let occupant_id = crate::game_logic::ObjectId(occupant);
            let occupant_ok = self.last_presentation_frame.as_ref().is_some_and(|frame| {
                frame.objects.iter().any(|object| {
                    object.id == occupant_id
                        && !object.destroyed
                        && object.team == frame.local_team()
                })
            });
            if !occupant_ok {
                self.host_reject_control_bar_request("EXIT occupant is not a live local unit");
                return;
            }
            self.host_control_bar_queue_command(
                player_id,
                vec![occupant_id],
                crate::command_system::CommandType::Exit,
            );
            return;
        }

        if let Some(special_power_name) = special_power_name {
            let special_key = host_control_bar_key(special_power_name);
            if special_key.contains("remotecharge")
                && host_control_bar_key(command_name).contains("detonate")
            {
                self.host_control_bar_queue_command(
                    player_id,
                    selected,
                    crate::command_system::CommandType::DetonateRemoteDemoCharges,
                );
                return;
            }
            if special_key == "specialabilitychangebattleplans" {
                let Some(power_type) = host_control_bar_battle_plan_from_options(options) else {
                    self.host_reject_control_bar_request(
                        "battle-plan button has no single retail option bit",
                    );
                    return;
                };
                self.host_control_bar_queue_command(
                    player_id,
                    selected,
                    crate::command_system::CommandType::DoSpecialPower {
                        power_type,
                        target: crate::command_system::PowerTarget::None,
                    },
                );
                return;
            }
            if let Some(power_type) =
                crate::command_system::special_power_type_from_template_name(special_power_name)
            {
                if power_type == crate::command_system::SpecialPowerType::HackerDisableBuilding
                    && !self.host_control_bar_selection_has_ready_hacker_disable(&selected)
                {
                    self.host_reject_control_bar_request(
                        "Hacker Disable Building requires a ready paired module",
                    );
                    return;
                }
                self.host_control_bar_queue_command(
                    player_id,
                    selected,
                    crate::command_system::CommandType::DoSpecialPower {
                        power_type,
                        target: crate::command_system::PowerTarget::None,
                    },
                );
                return;
            }
            self.host_reject_control_bar_request("unmapped exact SpecialPower name");
            return;
        }

        // GameClient represents FIRE_WEAPON as DoAttackObject.  A targetless
        // button is only meaningful for the exact retail Demo tertiary suicide
        // action; do not fire an arbitrary default weapon at a made-up target.
        if command_type == LegacyCommandType::DoAttackObject {
            if host_control_bar_is_exact_retail_button(command_name, "Demo_Command_TertiarySuicide")
            {
                self.host_control_bar_queue_command(
                    player_id,
                    selected,
                    crate::command_system::CommandType::DemoTertiarySuicide,
                );
            } else {
                let _ = weapon_slot;
                self.host_reject_control_bar_request(
                    "targetless FIRE_WEAPON is not a legal host command",
                );
            }
            return;
        }

        match host_control_bar_direct_action(command_type, command_name, weapon_slot) {
            Ok(action) => self.host_apply_control_bar_direct_action(player_id, selected, action),
            Err(reason) => self.host_reject_control_bar_request(reason),
        }
    }

    fn host_apply_control_bar_direct_action(
        &mut self,
        player_id: u32,
        selected: Vec<crate::game_logic::ObjectId>,
        action: HostControlBarDirectAction,
    ) {
        match action {
            HostControlBarDirectAction::Queue(command_type) => {
                self.host_control_bar_queue_command(player_id, selected, command_type);
            }
            HostControlBarDirectAction::FirstSelectedObject(action) => {
                let Some(object_id) = selected.first().copied() else {
                    self.host_reject_control_bar_request(
                        "selected-object Control Bar command has no local selection",
                    );
                    return;
                };
                let command_type = match action {
                    HostControlBarFirstSelectedObjectAction::CancelConstruction => {
                        crate::command_system::CommandType::DozerCancelConstruct { object_id }
                    }
                };
                self.host_control_bar_queue_command(player_id, selected, command_type);
            }
            HostControlBarDirectAction::LockWeapon(slot) => {
                // C++ MSG_SWITCH_WEAPONS locks the button's explicit slot.
                self.host_control_bar_queue_command(
                    player_id,
                    selected,
                    crate::command_system::CommandType::SwitchWeapons { slot },
                );
            }
            HostControlBarDirectAction::ArmUnitAbility(ability) => {
                self.arm_pending_unit_ability(ability);
            }
        }
    }

    fn host_apply_control_bar_target(
        &mut self,
        command_name: &str,
        command_type: LegacyCommandType,
        options: u32,
        weapon_slot: Option<u32>,
        max_shots_to_fire: Option<i32>,
        selected_object_ids: &[u32],
        player_id: u32,
        target: HostControlBarTarget,
        physical_os_input: bool,
    ) {
        let generic_action = match &target {
            HostControlBarTarget::Generic => {
                match host_control_bar_generic_target_action(
                    command_type,
                    command_name,
                    weapon_slot,
                    options,
                    max_shots_to_fire,
                ) {
                    Ok(action) => Some(action),
                    Err(reason) => {
                        self.host_reject_control_bar_request(reason);
                        return;
                    }
                }
            }
            HostControlBarTarget::DozerConstruct { .. }
            | HostControlBarTarget::SpecialPower { .. } => None,
        };
        let selected = self.host_control_bar_local_selection(player_id, selected_object_ids);
        let generic_allows_empty_selection = matches!(
            generic_action.as_ref(),
            Some(HostControlBarGenericTargetAction::PlaceBeacon)
        );
        if selected.is_empty() && !generic_allows_empty_selection {
            self.host_reject_control_bar_request(
                "target command has no local selectable selection",
            );
            return;
        }
        self.host_mark_single_use_command_if_needed(options, selected.iter().copied());
        let selected_has_dozer = selected.iter().any(|id| self.ui_object_is_dozer(*id));
        if !selected.is_empty() {
            // The frozen HDB readiness gate below must inspect the exact same
            // vetted selection after this UI-side selection update.  Preserve
            // the small object-id vector rather than recomputing it from live
            // input (which could admit a changed selection).
            self.host_set_selection(player_id, selected.clone());
        }

        match target {
            HostControlBarTarget::DozerConstruct { template_name } => {
                if template_name.trim().is_empty() {
                    self.host_reject_control_bar_request("dozer construction has no template name");
                    return;
                }
                if !selected_has_dozer {
                    self.host_reject_control_bar_request(
                        "dozer construction requires a local live dozer or worker",
                    );
                    return;
                }
                self.begin_structure_placement_from_ui(&template_name);
                self.interactive_playability.note_control_bar_construct_arm(
                    self.host_control_bar_evidence_eligible(physical_os_input),
                );
            }
            HostControlBarTarget::SpecialPower {
                special_power_name,
                special_power_id: _,
            } => {
                let special_key = host_control_bar_key(&special_power_name);
                if special_key == "specialabilityboobytrap" {
                    self.arm_pending_unit_ability(PendingUnitAbility::PlantBoobyTrap);
                    return;
                }
                let Some(power_type) = crate::command_system::special_power_type_from_template_name(
                    &special_power_name,
                ) else {
                    self.host_reject_control_bar_request(
                        "target special has no exact host mapping",
                    );
                    return;
                };
                if power_type == crate::command_system::SpecialPowerType::HackerDisableBuilding
                    && !self.host_control_bar_selection_has_ready_hacker_disable(&selected)
                {
                    self.host_reject_control_bar_request(
                        "Hacker Disable Building requires a ready paired module",
                    );
                    return;
                }
                let cursor = Self::radius_cursor_type_for_special_power(&power_type);
                self.pending_map_command = Some(PendingMapCommand::SpecialPower(power_type));
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending(cursor);
            }
            HostControlBarTarget::Generic => match generic_action {
                Some(HostControlBarGenericTargetAction::Weapon(weapon)) => {
                    // C++ ControlBarCommandProcessing sends
                    // MSG_SET_MINE_CLEARING_DETAIL while the *retail option*
                    // is armed, before its normal MSG_DO_WEAPON_AT_LOCATION.
                    // Keep the detail state on the exact vetted selection;
                    // neither a command-button name nor a unit template is
                    // authority for this behavior.
                    let uses_mine_clearing_weapon_set = weapon.uses_mine_clearing_weapon_set();
                    if uses_mine_clearing_weapon_set {
                        let logic = self.host_game_logic_mut();
                        for &unit_id in &selected {
                            let _ = logic.unit_command_set_mine_clearing_detail(unit_id, true);
                        }
                    }
                    self.pending_map_command = Some(PendingMapCommand::Weapon(weapon));
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending(if uses_mine_clearing_weapon_set {
                        "CLEARMINES"
                    } else {
                        "ATTACK_DAMAGE_AREA"
                    });
                }
                Some(HostControlBarGenericTargetAction::UnitAbility(ability)) => {
                    self.arm_pending_unit_ability(ability);
                }
                Some(HostControlBarGenericTargetAction::AttackMove) => {
                    self.pending_map_command = Some(PendingMapCommand::AttackMove);
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending("ATTACK_CONTINUE_AREA");
                }
                Some(HostControlBarGenericTargetAction::Guard(mode)) => {
                    self.pending_map_command = Some(PendingMapCommand::Guard(mode));
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending("GUARD_AREA");
                }
                Some(HostControlBarGenericTargetAction::SetRallyPoint) => {
                    self.pending_map_command = Some(PendingMapCommand::SetRallyPoint);
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending("FRIENDLY_SPECIALPOWER");
                }
                Some(HostControlBarGenericTargetAction::CombatDrop(combat_drop)) => {
                    self.pending_map_command = Some(PendingMapCommand::CombatDrop(combat_drop));
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending("COMBATDROP");
                }
                Some(HostControlBarGenericTargetAction::PlaceBeacon) => {
                    if !crate::command_executor::host_local_player_can_place_beacon(
                        &self.game_logic,
                        player_id,
                    ) {
                        self.host_reject_control_bar_request(
                            "place beacon is disabled at MaxBeaconsPerPlayer",
                        );
                        return;
                    }
                    self.pending_map_command = Some(PendingMapCommand::PlaceBeacon);
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending("RADAR");
                }
                None => self.host_reject_control_bar_request(
                    "target command reached the host without a typed translation",
                ),
            },
        }
    }
    fn host_apply_control_bar_cancel(
        &mut self,
        player_id: u32,
        selected_object_ids: &[u32],
        producer: crate::game_logic::ObjectId,
        production_id: u32,
        production_type: QueueProductionType,
        upgrade_name: &str,
        queue_index: usize,
    ) {
        let selected = self.host_control_bar_local_selection(player_id, selected_object_ids);
        if !selected.contains(&producer) || !self.ui_object_can_produce(producer) {
            self.host_reject_control_bar_request(
                "production cancellation requires its local producer selection",
            );
            return;
        }
        let queue_len = self
            .presentation_ro(producer)
            .map(|object| object.production_queue.len())
            .unwrap_or(0);
        // C++ ControlBarCommandProcessing.cpp:466-479 uses
        // `m_queueData[i].productionID`. Host ProductionItem has no stored
        // production ID; GameClient often aliases production_id = queue_index.
        let Some(cancel_index) =
            resolve_host_queue_cancel_index(production_id, queue_index, queue_len)
        else {
            self.host_reject_control_bar_request("production cancellation queue entry is stale");
            return;
        };
        let Some(item) = self
            .presentation_ro(producer)
            .and_then(|object| object.production_queue.get(cancel_index).cloned())
        else {
            self.host_reject_control_bar_request("production cancellation queue entry is stale");
            return;
        };
        let type_matches = match production_type {
            QueueProductionType::Unit => !item.is_upgrade,
            QueueProductionType::Upgrade => {
                item.is_upgrade
                    && (upgrade_name.trim().is_empty()
                        || item.template_name.eq_ignore_ascii_case(upgrade_name))
            }
            QueueProductionType::Invalid => false,
        };
        if !type_matches {
            self.host_reject_control_bar_request(
                "production cancellation does not match the displayed queue entry",
            );
            return;
        }
        self.host_set_selection(player_id, selected);
        if self.host_cancel_production_at_index(producer, cancel_index) {
            self.play_sound_effect(SoundType::Command);
        } else {
            self.host_reject_control_bar_request(
                "authoritative production cancellation was rejected",
            );
        }
    }

    fn host_control_bar_local_selection(
        &self,
        player_id: u32,
        ids: &[u32],
    ) -> Vec<crate::game_logic::ObjectId> {
        if !self.host_control_bar_is_local_player(player_id) {
            return Vec::new();
        }
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return Vec::new();
        };
        let mut selected = Vec::new();
        for raw_id in ids {
            let id = crate::game_logic::ObjectId(*raw_id);
            if selected.contains(&id) {
                continue;
            }
            let valid = frame.objects.iter().any(|object| {
                object.id == id
                    && object.team == frame.local_team()
                    && crate::unit_control::UnitControlSystem::presentation_is_selectable(object)
            });
            if valid {
                selected.push(id);
            }
        }
        selected
    }

    /// HDB targeting is armed from the frozen presentation frame.  The live
    /// executor repeats C++ ActionManager authority later; this preflight only
    /// prevents a stale control-bar click from manufacturing a cursor for a
    /// name-matched Hacker whose paired module is missing, paused, or cooling
    /// down.
    pub(super) fn host_control_bar_selection_has_ready_hacker_disable(
        &self,
        selected: &[crate::game_logic::ObjectId],
    ) -> bool {
        let Some(frame) = self.last_presentation_frame.as_ref() else {
            return false;
        };
        selected.iter().any(|id| {
            frame.objects.iter().any(|object| {
                object.id == *id
                    && frame.is_owned_by_local(object)
                    && !object.destroyed
                    && !object.sold
                    && !object.disabled
                    && object.hacker_disable_building_capable
                    && object.hacker_disable_building_ready
            })
        })
    }

    fn host_control_bar_is_local_player(&self, player_id: u32) -> bool {
        if player_id == self.local_player_id_for_ui() {
            true
        } else {
            debug!(
                "discarded Control Bar request for player {player_id}; local authoritative player is {}",
                self.local_player_id_for_ui()
            );
            false
        }
    }

    /// Physical Control Bar evidence is meaningful only in the same offline,
    /// visible session this bridge is allowed to mutate.  A host control file,
    /// direct helper call, headless process, or multiplayer/replay request
    /// cannot promote the acceptance latch.
    fn host_control_bar_evidence_eligible(&self, physical_os_input: bool) -> bool {
        physical_os_input
            && !self.runtime_host_headless
            && self.runtime_host_window_visible()
            && matches!(self.current_state, GameState::InGame)
            && host_control_bar_bridge_is_offline_product_mode(self.host_match_game_mode)
    }

    fn host_control_bar_production_quantity(&self) -> u32 {
        let ctrl = self.keys_pressed.contains(&Key::Named(NamedKey::Control));
        let shift = self.keys_pressed.contains(&Key::Named(NamedKey::Shift));
        if ctrl {
            9 // C++ DEFAULT_PRODUCTION_QUEUE_LIMIT.
        } else if shift {
            5
        } else {
            1
        }
    }

    fn host_control_bar_command_allows_empty_selection(
        &self,
        command_type: LegacyCommandType,
    ) -> bool {
        matches!(
            command_type,
            LegacyCommandType::DoStop
                | LegacyCommandType::DoScatter
                | LegacyCommandType::MetaScatter
                | LegacyCommandType::MetaViewCommandCenter
                | LegacyCommandType::MetaViewLastRadarEvent
                | LegacyCommandType::RemoveBeacon
                | LegacyCommandType::MetaRemoveBeacon
        )
    }

    fn host_control_bar_queue_command(
        &mut self,
        player_id: u32,
        selected_units: Vec<crate::game_logic::ObjectId>,
        command_type: crate::command_system::CommandType,
    ) {
        self.host_queue_and_process_command(crate::command_system::GameCommand {
            command_type,
            player_id,
            command_id: 0,
            timestamp: std::time::SystemTime::now(),
            selected_units,
            modifier_keys: crate::command_system::ModifierKeys {
                shift: self.keys_pressed.contains(&Key::Named(NamedKey::Shift)),
                ctrl: self.keys_pressed.contains(&Key::Named(NamedKey::Control)),
                alt: self.keys_pressed.contains(&Key::Named(NamedKey::Alt)),
            },
        });
    }

    fn host_reject_control_bar_request(&mut self, reason: &str) {
        // C++ leftover-WND rejects are silent except CanMake `GUI:*` labels.
        debug!("Control Bar host request rejected: {reason}");
    }
}

/// A fully typed immediate Control Bar operation.  Keeping this separate from
/// button names means a newly parsed legacy command cannot accidentally take a
/// nearby legacy name-parser path with different semantics.
#[derive(Debug, Clone, PartialEq)]
enum HostControlBarDirectAction {
    Queue(crate::command_system::CommandType),
    FirstSelectedObject(HostControlBarFirstSelectedObjectAction),
    /// C++ `MSG_SWITCH_WEAPONS`: a permanent lock for one explicit slot.
    LockWeapon(u8),
    /// A legacy target command whose button was published direct despite its
    /// exact retail identity proving that it must arm a target interaction.
    ArmUnitAbility(PendingUnitAbility),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HostControlBarFirstSelectedObjectAction {
    CancelConstruction,
}

/// A non-special target mode awaiting the next map/object click.
#[derive(Debug, Clone, PartialEq, Eq)]
enum HostControlBarGenericTargetAction {
    Weapon(PendingWeaponCommand),
    UnitAbility(PendingUnitAbility),
    AttackMove,
    Guard(crate::game_logic::GuardMode),
    SetRallyPoint,
    CombatDrop(PendingCombatDropCommand),
    PlaceBeacon,
}

/// Translate only legacy command types that have a corresponding Main command.
///
/// This intentionally does not call the name-based UI compatibility method:
/// command names are presentation identifiers, while the parsed C++ command
/// type is the authority for immediate behavior.  Unknown or incomplete
/// legacy payloads reject at the bridge rather than being guessed into a
/// different host action.
fn host_control_bar_direct_action(
    command_type: LegacyCommandType,
    command_name: &str,
    weapon_slot: Option<u32>,
) -> Result<HostControlBarDirectAction, &'static str> {
    if let Some(ability) = host_control_bar_target_unit_ability(command_type, command_name) {
        return Ok(HostControlBarDirectAction::ArmUnitAbility(ability));
    }

    use crate::command_system::CommandType as HostCommandType;
    let action = match command_type {
        LegacyCommandType::Exit => HostControlBarDirectAction::Queue(HostCommandType::Exit),
        LegacyCommandType::Evacuate => HostControlBarDirectAction::Queue(HostCommandType::Evacuate),
        LegacyCommandType::ExecuteRailedTransport => {
            HostControlBarDirectAction::Queue(HostCommandType::ExecuteRailedTransport)
        }
        LegacyCommandType::InternetHack => {
            HostControlBarDirectAction::Queue(HostCommandType::HackInternet)
        }
        LegacyCommandType::ToggleOvercharge => {
            HostControlBarDirectAction::Queue(HostCommandType::ToggleOvercharge)
        }
        LegacyCommandType::DoStop => HostControlBarDirectAction::Queue(HostCommandType::Stop),
        LegacyCommandType::DoScatter | LegacyCommandType::MetaScatter => {
            HostControlBarDirectAction::Queue(HostCommandType::Scatter)
        }
        LegacyCommandType::MetaDeploy => HostControlBarDirectAction::Queue(HostCommandType::Deploy),
        LegacyCommandType::DoCheer => HostControlBarDirectAction::Queue(HostCommandType::Cheer),
        LegacyCommandType::MetaCreateFormation => {
            HostControlBarDirectAction::Queue(HostCommandType::CreateFormation)
        }
        LegacyCommandType::MetaViewCommandCenter => {
            HostControlBarDirectAction::Queue(HostCommandType::ViewCommandCenter)
        }
        LegacyCommandType::MetaViewLastRadarEvent => {
            HostControlBarDirectAction::Queue(HostCommandType::ViewLastRadarEvent)
        }
        LegacyCommandType::RemoveBeacon | LegacyCommandType::MetaRemoveBeacon => {
            HostControlBarDirectAction::Queue(HostCommandType::RemoveBeacon)
        }
        // C++ `MSG_SELL` has no object argument; GameLogicDispatch groupSells
        // the current selection. Queue the full leftover WND selection.
        LegacyCommandType::Sell => HostControlBarDirectAction::Queue(HostCommandType::Sell {
            object_id: crate::game_logic::ObjectId(0),
        }),
        LegacyCommandType::DozerCancelConstruct => HostControlBarDirectAction::FirstSelectedObject(
            HostControlBarFirstSelectedObjectAction::CancelConstruction,
        ),
        LegacyCommandType::SwitchWeapons => {
            let Some(slot) = host_control_bar_weapon_slot_index(weapon_slot) else {
                return Err("SWITCH_WEAPON button has no valid explicit slot");
            };
            HostControlBarDirectAction::LockWeapon(slot)
        }
        // `HIJACK_VEHICLE` and `SABOTAGE_BUILDING` are both represented by
        // legacy Enter.  Only the exact retail button identities above can
        // disambiguate them.  A generic Enter must never become either action.
        LegacyCommandType::Enter => {
            return Err("legacy ENTER button is not an exact supported retail target ability");
        }
        LegacyCommandType::ConvertToCarBomb => {
            return Err("legacy CONVERT_TO_CARBOMB button is not the exact retail car-bomb action");
        }
        LegacyCommandType::CancelUnitCreate | LegacyCommandType::CancelUpgrade => {
            return Err("queue cancellation requires the displayed queue entry identity");
        }
        _ => return Err("legacy Control Bar command has no typed offline host translation"),
    };
    Ok(action)
}

/// Resolve the two legacy `Enter` aliases and `ConvertToCarBomb` solely by
/// their retail CommandButton keys.  The C++ enum intentionally collapses the
/// first two commands, so a type-only conversion would turn a normal enter
/// order into hijack or sabotage.
fn host_control_bar_target_unit_ability(
    command_type: LegacyCommandType,
    command_name: &str,
) -> Option<PendingUnitAbility> {
    match command_type {
        LegacyCommandType::Enter
            if host_control_bar_is_exact_retail_button(
                command_name,
                "Command_GLAInfantryHijack",
            ) =>
        {
            Some(PendingUnitAbility::Hijack)
        }
        LegacyCommandType::Enter
            if host_control_bar_is_exact_retail_button(
                command_name,
                "Command_SabotageBuilding",
            ) =>
        {
            Some(PendingUnitAbility::Sabotage)
        }
        LegacyCommandType::ConvertToCarBomb
            if host_control_bar_is_exact_retail_button(
                command_name,
                "Command_GLAInfantryTerroristMakeCarBomb",
            ) =>
        {
            Some(PendingUnitAbility::ConvertToCarbomb)
        }
        _ => None,
    }
}

/// Translate target-bearing non-special buttons without routing through a
/// name parser.  Guard variants retain their exact retail keys because all
/// three share the same legacy `DoGuardPosition` enum value.
fn host_control_bar_generic_target_action(
    command_type: LegacyCommandType,
    command_name: &str,
    weapon_slot: Option<u32>,
    options: u32,
    max_shots_to_fire: Option<i32>,
) -> Result<HostControlBarGenericTargetAction, &'static str> {
    if let Some(ability) = host_control_bar_target_unit_ability(command_type, command_name) {
        return Ok(HostControlBarGenericTargetAction::UnitAbility(ability));
    }

    match command_type {
        LegacyCommandType::DoAttackObject => {
            let Some(slot) = host_control_bar_weapon_slot(weapon_slot) else {
                return Err("FIRE_WEAPON button has no valid explicit slot");
            };
            let Some(max_shots_to_fire) = max_shots_to_fire else {
                return Err("FIRE_WEAPON button has no parsed MaxShotsToFire value");
            };
            Ok(HostControlBarGenericTargetAction::Weapon(
                PendingWeaponCommand {
                    weapon_slot: slot,
                    max_shots_to_fire,
                    options,
                },
            ))
        }
        LegacyCommandType::DoAttackMoveTo
            if host_control_bar_is_exact_retail_button(command_name, "Command_AttackMove") =>
        {
            Ok(HostControlBarGenericTargetAction::AttackMove)
        }
        LegacyCommandType::DoGuardPosition => {
            let mode = if host_control_bar_is_exact_retail_button(command_name, "Command_Guard") {
                crate::game_logic::GuardMode::Normal
            } else if host_control_bar_is_exact_retail_button(
                command_name,
                "Command_GuardWithoutPursuit",
            ) {
                crate::game_logic::GuardMode::WithoutPursuit
            } else if host_control_bar_is_exact_retail_button(
                command_name,
                "Command_GuardFlyingUnitsOnly",
            ) {
                crate::game_logic::GuardMode::FlyingUnitsOnly
            } else {
                return Err("GUARD button is not an exact supported retail key");
            };
            Ok(HostControlBarGenericTargetAction::Guard(mode))
        }
        LegacyCommandType::SetRallyPoint
            if host_control_bar_is_exact_retail_button(command_name, "Command_SetRallyPoint") =>
        {
            Ok(HostControlBarGenericTargetAction::SetRallyPoint)
        }
        LegacyCommandType::CombatDropAtLocation | LegacyCommandType::CombatDropAtObject
            if host_control_bar_is_exact_retail_button(command_name, "Command_CombatDrop") =>
        {
            Ok(HostControlBarGenericTargetAction::CombatDrop(
                PendingCombatDropCommand { options },
            ))
        }
        LegacyCommandType::PlaceBeacon | LegacyCommandType::MetaPlaceBeacon
            if host_control_bar_is_exact_retail_button(command_name, "Command_PlaceBeacon") =>
        {
            Ok(HostControlBarGenericTargetAction::PlaceBeacon)
        }
        LegacyCommandType::Enter => {
            Err("legacy ENTER button is not an exact supported retail target ability")
        }
        LegacyCommandType::ConvertToCarBomb => {
            Err("legacy CONVERT_TO_CARBOMB button is not the exact retail car-bomb action")
        }
        _ => Err("target Control Bar command has no typed offline host translation"),
    }
}

/// Case-insensitive comparison remains faithful to INI identifiers while
/// deliberately refusing punctuation-stripped or substring aliases.
fn host_control_bar_is_exact_retail_button(value: &str, retail_name: &str) -> bool {
    value.eq_ignore_ascii_case(retail_name)
}

/// The host never receives a raw legacy enum discriminant for a weapon slot.
/// GameClient already emits the three C++ values as explicit numbers; retain
/// that contract in one checked conversion and reject unknown values.
fn host_control_bar_weapon_slot(slot: Option<u32>) -> Option<crate::command_system::WeaponSlot> {
    match host_control_bar_weapon_slot_index(slot)? {
        0 => Some(crate::command_system::WeaponSlot::Primary),
        1 => Some(crate::command_system::WeaponSlot::Secondary),
        2 => Some(crate::command_system::WeaponSlot::Tertiary),
        _ => None,
    }
}

fn host_control_bar_weapon_slot_index(slot: Option<u32>) -> Option<u8> {
    match slot? {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        _ => None,
    }
}

fn host_control_bar_battle_plan_from_options(
    options: u32,
) -> Option<crate::command_system::SpecialPowerType> {
    use crate::command_system::SpecialPowerType;
    match options & (COMMAND_OPTION_ONE | COMMAND_OPTION_TWO | COMMAND_OPTION_THREE) {
        COMMAND_OPTION_ONE => Some(SpecialPowerType::BattlePlanBombardment),
        COMMAND_OPTION_TWO => Some(SpecialPowerType::BattlePlanHoldTheLine),
        COMMAND_OPTION_THREE => Some(SpecialPowerType::BattlePlanSearchAndDestroy),
        _ => None,
    }
}

fn host_control_bar_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

/// Resolve the queue slot to cancel.
///
/// C++ `ControlBar::processCommand` `GUI_COMMAND_CANCEL_UNIT_BUILD`
/// (`ControlBarCommandProcessing.cpp:443-479`) looks up the clicked slot,
/// then cancels `m_queueData[i].productionID` — never first-match-by-template.
/// Host `ProductionItem` has no stored production ID; GameClient often aliases
/// `production_id = queue_index`. Prefer a valid production_id slot, else the
/// displayed queue_index. Fail-closed when neither addresses the queue.
pub(crate) fn resolve_host_queue_cancel_index(
    production_id: u32,
    queue_index: usize,
    queue_len: usize,
) -> Option<usize> {
    if production_id != 0 {
        let id_index = production_id as usize;
        if id_index < queue_len {
            return Some(id_index);
        }
    }
    (queue_index < queue_len).then_some(queue_index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn popup_pause_ownership_releases_only_the_pause_it_created() {
        // pause -> nonpause: replacement releases the popup-owned pause.
        let paused_popup = resolve_popup_pause_transition(false, false, true, false);
        assert_eq!(
            paused_popup,
            PopupPauseTransition {
                popup_pause_owned: true,
                set_paused: Some(true),
            }
        );
        assert_eq!(
            resolve_popup_pause_transition(true, true, false, false),
            PopupPauseTransition {
                popup_pause_owned: false,
                set_paused: Some(false),
            }
        );

        // nonpause -> pause: only the newer pause becomes host-owned.
        assert_eq!(
            resolve_popup_pause_transition(false, false, false, false),
            PopupPauseTransition {
                popup_pause_owned: false,
                set_paused: None,
            }
        );
        assert_eq!(
            resolve_popup_pause_transition(false, false, true, false),
            paused_popup
        );

        // An acknowledgement/replacement while another owner has paused the
        // match clears Main's stale popup marker but never resumes that owner.
        assert_eq!(
            resolve_popup_pause_transition(true, true, false, true),
            PopupPauseTransition {
                popup_pause_owned: false,
                set_paused: None,
            }
        );
        assert_eq!(
            resolve_popup_pause_transition(false, true, false, true),
            PopupPauseTransition {
                popup_pause_owned: false,
                set_paused: None,
            }
        );
    }

    #[test]
    fn paused_bridge_accepts_only_popup_acknowledgement() {
        assert!(host_control_bar_request_allowed_in_state(
            false,
            true,
            &HostControlBarRequest::DismissInGamePopupMessage {
                popup_generation: 17,
            },
        ));
        assert!(
            !host_control_bar_request_allowed_in_state(
                false,
                true,
                &HostControlBarRequest::CancelStructurePlacement,
            ),
            "paused state must not admit world-mutating Control Bar work"
        );
        assert!(host_control_bar_request_allowed_in_state(
            true,
            false,
            &HostControlBarRequest::CancelStructurePlacement,
        ));
    }

    #[test]
    fn host_bridge_is_offline_skirmish_product_scope_not_network() {
        // AGENTS.md: GameNetwork deferred. Live match is Skirmish/SinglePlayer.
        // C++ ControlBarCommandProcessing.cpp appends MSG_* in every mode;
        // Rust host_tick_control_bar_bridge only mutates the offline world.
        use crate::game_logic::GameMode;
        assert!(host_control_bar_bridge_is_offline_product_mode(Some(
            GameMode::Skirmish
        )));
        assert!(host_control_bar_bridge_is_offline_product_mode(Some(
            GameMode::SinglePlayer
        )));
        assert!(!host_control_bar_bridge_is_offline_product_mode(Some(
            GameMode::Multiplayer
        )));
        assert!(!host_control_bar_bridge_is_offline_product_mode(Some(
            GameMode::Lan
        )));
        assert!(!host_control_bar_bridge_is_offline_product_mode(Some(
            GameMode::Internet
        )));
        assert!(!host_control_bar_bridge_is_offline_product_mode(Some(
            GameMode::Replay
        )));
        assert!(!host_control_bar_bridge_is_offline_product_mode(Some(
            GameMode::Shell
        )));
        assert!(!host_control_bar_bridge_is_offline_product_mode(None));

        assert!(host_control_bar_bridge_is_network_or_replay(Some(
            GameMode::Multiplayer
        )));
        assert!(host_control_bar_bridge_is_network_or_replay(Some(
            GameMode::Replay
        )));
        assert!(!host_control_bar_bridge_is_network_or_replay(Some(
            GameMode::Skirmish
        )));
        assert!(!host_control_bar_bridge_is_network_or_replay(Some(
            GameMode::SinglePlayer
        )));

        let tick = include_str!("control_bar_bridge.rs");
        let start = tick
            .find("pub(crate) fn host_tick_control_bar_bridge")
            .expect("host_tick_control_bar_bridge");
        let body = &tick[start..];
        let end = body.find("\n    fn ").unwrap_or(body.len().min(2500));
        let body = &body[..end];
        assert!(
            body.contains("host_control_bar_bridge_is_network_or_replay"),
            "live drain must refuse network/replay rather than mutate the offline host world"
        );
        assert!(
            body.contains("set_host_control_bar_bridge_enabled(false)"),
            "network/replay must restore the crate GameMessage fallback"
        );
    }

    #[test]
    fn popup_acknowledgements_require_the_current_live_generation() {
        const POPUP_A: usize = 71;
        const POPUP_B: usize = 72;

        assert!(popup_acknowledgement_matches_active_generation(
            Some(POPUP_B),
            POPUP_B
        ));
        assert!(
            !popup_acknowledgement_matches_active_generation(Some(POPUP_B), POPUP_A),
            "a delayed acknowledgement for replacement A must not consume B"
        );
        assert!(
            !popup_acknowledgement_matches_active_generation(Some(POPUP_B), 0),
            "zero is an explicit fail-closed no-popup identity"
        );
        assert!(
            !popup_acknowledgement_matches_active_generation(None, POPUP_B),
            "a world reset/replacement has no retained popup to dismiss"
        );
    }

    #[test]
    fn popup_dismissal_matches_token_before_clearing_and_reconciles_active_record() {
        let bridge = include_str!("control_bar_bridge.rs");
        let start = bridge
            .find("fn host_dismiss_in_game_popup_message")
            .expect("popup dismiss handler");
        let end = bridge[start..]
            .find("\n    fn host_clear_active_popup_presentation_residual")
            .map(|offset| start + offset)
            .expect("popup dismiss handler end");
        let dismiss = &bridge[start..end];
        let guard = dismiss
            .find("popup_acknowledgement_matches_active_generation")
            .expect("dismissal checks its live popup identity");
        let take = dismiss
            .find("self.game_logic.take_popup_message_requests();")
            .expect("matching dismissal clears Main's active record");
        assert!(
            guard < take
                && dismiss.contains("self.host_clear_active_popup_presentation_residual();")
                && dismiss.contains("self.host_reconcile_active_popup_pause(None);"),
            "only a matching acknowledgement may clear Main's active record and pause ownership"
        );
        assert!(
            dismiss.contains("return;"),
            "zero/missing/mismatched identities must fail closed before WND cleanup"
        );
    }

    #[test]
    fn popup_world_boundaries_invalidate_only_popup_ui_work() {
        let authority = include_str!("host_authority.rs");
        let reset = &authority[authority
            .find("fn host_clear_match_residuals(")
            .expect("match reset boundary")..];
        let replace = &authority[authority
            .find("fn host_replace_game_logic(")
            .expect("world replace boundary")..];
        let reset_end = reset
            .find("fn host_reset_game_logic")
            .expect("next reset authority method");
        let replace_end = replace
            .find("fn host_save_game_authority")
            .expect("next replacement method");
        assert!(
            reset[..reset_end].contains("self.host_invalidate_active_popup_for_world_boundary();")
                && replace[..replace_end]
                    .contains("self.host_invalidate_active_popup_for_world_boundary();")
                && replace[..replace_end]
                    .find("self.host_invalidate_active_popup_for_world_boundary();")
                    < replace[..replace_end].find("self.game_logic = logic;")
        );

        let bridge = include_str!("control_bar_bridge.rs");
        let start = bridge
            .find("fn host_invalidate_active_popup_for_world_boundary")
            .expect("popup world-boundary invalidation helper");
        let end = bridge[start..]
            .find("\n    fn host_clear_active_popup_presentation_residual")
            .map(|offset| start + offset)
            .expect("popup invalidation helper end");
        let invalidate = &bridge[start..end];
        assert!(
            invalidate.contains("self.game_logic.take_popup_message_requests();")
                && invalidate.contains("self.host_reconcile_active_popup_pause(None);")
                && invalidate.contains("clear_host_dismiss_in_game_popup_message_requests")
                && invalidate.contains("TheInGameUI::clear_popup_message_data();"),
            "world boundaries clear the authoritative popup, popup-owned pause, stale ACKs, and modal WND only"
        );
    }

    #[test]
    fn cancel_structure_placement_request_routes_through_the_dual_hud_clear() {
        let bridge = include_str!("control_bar_bridge.rs");
        let apply_start = bridge
            .find("fn host_apply_control_bar_request")
            .expect("Control Bar bridge request handler");
        let apply_end = bridge[apply_start..]
            .find("\n    fn host_apply_control_bar_direct")
            .map(|offset| apply_start + offset)
            .expect("end of Control Bar bridge request handler");
        let apply = &bridge[apply_start..apply_end];
        let request_start = apply
            .find("HostControlBarRequest::CancelStructurePlacement")
            .expect("placement cancellation is drained by Main");
        let request_end = apply[request_start..]
            .find("HostControlBarRequest::Production")
            .map(|offset| request_start + offset)
            .expect("placement cancellation precedes normal Control Bar commands");
        let request = &apply[request_start..request_end];
        assert!(
            request.contains("self.cancel_structure_placement_from_ui();"),
            "the bridge must use Main's authoritative placement-cancel helper"
        );

        let ui_commands = include_str!("ui_commands.rs");
        let clear_start = ui_commands
            .find("fn cancel_structure_placement_from_ui")
            .expect("authoritative placement-cancel helper");
        let clear_end = ui_commands[clear_start..]
            .find("\n    /// Update structure placement ghost legality")
            .map(|offset| clear_start + offset)
            .expect("end of authoritative placement-cancel helper");
        let clear = &ui_commands[clear_start..clear_end];
        assert!(
            clear.contains("self.pending_structure_placement = None;")
                && clear.contains("self.game_hud.construction_panel.clear_structure_placement();")
                && clear.contains("self.ui_manager")
                && clear.contains(".game_hud_mut()")
                && clear.contains(".construction_panel")
                && clear.contains(".clear_structure_placement();"),
            "the host cancel must clear Main's pending placement and both HUD ghosts"
        );
        assert!(
            !clear.contains("pending_map_command"),
            "opening Generals Experience must not cancel an unrelated pending map command"
        );
    }

    #[test]
    fn fire_weapon_slots_are_explicit_and_fail_closed() {
        use crate::command_system::WeaponSlot;

        assert_eq!(
            host_control_bar_weapon_slot(Some(0)),
            Some(WeaponSlot::Primary)
        );
        assert_eq!(
            host_control_bar_weapon_slot(Some(1)),
            Some(WeaponSlot::Secondary)
        );
        assert_eq!(
            host_control_bar_weapon_slot(Some(2)),
            Some(WeaponSlot::Tertiary)
        );
        assert_eq!(host_control_bar_weapon_slot(Some(3)), None);
        assert_eq!(host_control_bar_weapon_slot(None), None);
    }

    #[test]
    fn fire_weapon_target_arm_retains_parsed_shot_limit_and_options() {
        let options = 0x0000_0001 | 0x0000_0002 | 0x0000_0100;
        assert_eq!(
            host_control_bar_generic_target_action(
                LegacyCommandType::DoAttackObject,
                "Command_ChinaJetMIGFireNapalmMissile",
                Some(0),
                options,
                Some(1),
            ),
            Ok(HostControlBarGenericTargetAction::Weapon(
                PendingWeaponCommand {
                    weapon_slot: crate::command_system::WeaponSlot::Primary,
                    max_shots_to_fire: 1,
                    options,
                },
            ))
        );
        assert!(
            host_control_bar_generic_target_action(
                LegacyCommandType::DoAttackObject,
                "Command_ChinaJetMIGFireNapalmMissile",
                Some(0),
                options,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn retail_disarm_mines_option_is_typed_without_a_button_name_authority() {
        const USES_MINE_CLEARING_WEAPONSET: u32 = 0x0020_0000;
        // Command_DisarmMinesAtPosition: NEED_TARGET_POS | OK_FOR_MULTI_SELECT
        // | USES_MINE_CLEARING_WEAPONSET.  The generic DoWeapon translation
        // deliberately keys off the parsed option, not this string.
        let retail = host_control_bar_generic_target_action(
            LegacyCommandType::DoAttackObject,
            "Command_DisarmMinesAtPosition",
            Some(0),
            0x0000_0120 | USES_MINE_CLEARING_WEAPONSET,
            Some(-1),
        )
        .expect("retail mine button translates");
        let HostControlBarGenericTargetAction::Weapon(retail_weapon) = retail else {
            panic!("retail mine button must remain an ordinary FIRE_WEAPON");
        };
        assert!(retail_weapon.uses_mine_clearing_weapon_set());

        let spoofed_name_without_option = host_control_bar_generic_target_action(
            LegacyCommandType::DoAttackObject,
            "Command_DisarmMinesAtPosition",
            Some(0),
            0x0000_0120,
            Some(-1),
        )
        .expect("ordinary FIRE_WEAPON translates");
        let HostControlBarGenericTargetAction::Weapon(ordinary_weapon) =
            spoofed_name_without_option
        else {
            panic!("ordinary FIRE_WEAPON remains a weapon command");
        };
        assert!(
            !ordinary_weapon.uses_mine_clearing_weapon_set(),
            "a button spelling cannot authorise mine-clearing detail"
        );
    }

    #[test]
    fn battle_plan_options_require_one_retail_choice() {
        use crate::command_system::SpecialPowerType;

        assert_eq!(
            host_control_bar_battle_plan_from_options(COMMAND_OPTION_ONE),
            Some(SpecialPowerType::BattlePlanBombardment)
        );
        assert_eq!(
            host_control_bar_battle_plan_from_options(COMMAND_OPTION_TWO),
            Some(SpecialPowerType::BattlePlanHoldTheLine)
        );
        assert_eq!(
            host_control_bar_battle_plan_from_options(COMMAND_OPTION_THREE),
            Some(SpecialPowerType::BattlePlanSearchAndDestroy)
        );
        assert_eq!(
            host_control_bar_battle_plan_from_options(COMMAND_OPTION_ONE | COMMAND_OPTION_TWO),
            None
        );
    }

    #[test]
    fn direct_legacy_commands_stay_typed_and_switch_weapons_keeps_its_slot() {
        use crate::command_system::CommandType as HostCommandType;

        assert!(matches!(
            host_control_bar_direct_action(LegacyCommandType::Exit, "Command_TransportExit", None),
            Ok(HostControlBarDirectAction::Queue(HostCommandType::Exit))
        ));
        assert!(matches!(
            host_control_bar_direct_action(LegacyCommandType::Evacuate, "Command_Evacuate", None),
            Ok(HostControlBarDirectAction::Queue(HostCommandType::Evacuate))
        ));
        assert!(matches!(
            host_control_bar_direct_action(
                LegacyCommandType::ExecuteRailedTransport,
                "Command_ExecuteRailedTransport",
                None,
            ),
            Ok(HostControlBarDirectAction::Queue(
                HostCommandType::ExecuteRailedTransport
            ))
        ));
        assert!(matches!(
            host_control_bar_direct_action(
                LegacyCommandType::InternetHack,
                "Command_ChinaInfantryHackerInternetHack",
                None,
            ),
            Ok(HostControlBarDirectAction::Queue(
                HostCommandType::HackInternet
            ))
        ));
        assert!(matches!(
            host_control_bar_direct_action(
                LegacyCommandType::ToggleOvercharge,
                "Command_Overcharge",
                None,
            ),
            Ok(HostControlBarDirectAction::Queue(
                HostCommandType::ToggleOvercharge
            ))
        ));
        assert!(matches!(
            host_control_bar_direct_action(LegacyCommandType::DoStop, "Command_Stop", None),
            Ok(HostControlBarDirectAction::Queue(HostCommandType::Stop))
        ));
        assert!(matches!(
            host_control_bar_direct_action(LegacyCommandType::MetaDeploy, "Command_Deploy", None),
            Ok(HostControlBarDirectAction::Queue(HostCommandType::Deploy))
        ));
        assert!(matches!(
            host_control_bar_direct_action(LegacyCommandType::DoCheer, "Command_Cheer", None),
            Ok(HostControlBarDirectAction::Queue(HostCommandType::Cheer))
        ));
        assert!(matches!(
            host_control_bar_direct_action(LegacyCommandType::Sell, "Command_Sell", None),
            Ok(HostControlBarDirectAction::Queue(
                HostCommandType::Sell { .. }
            ))
        ));
        assert!(matches!(
            host_control_bar_direct_action(
                LegacyCommandType::SwitchWeapons,
                "Command_SetDemoTrapManualDetonation",
                Some(2),
            ),
            Ok(HostControlBarDirectAction::LockWeapon(2))
        ));
        assert!(
            host_control_bar_direct_action(
                LegacyCommandType::SwitchWeapons,
                "Command_SetDemoTrapManualDetonation",
                Some(3),
            )
            .is_err()
        );
    }

    #[test]
    fn leftover_wnd_host_rejects_do_not_invent_hud_toasts() {
        let src = include_str!("control_bar_bridge.rs");
        let reject = src
            .find("fn host_reject_control_bar_request")
            .map(|i| &src[i..src.len().min(i + 350)])
            .expect("host_reject_control_bar_request");
        assert!(
            !reject.contains("push_info_message"),
            "hq-0foy9: leftover-WND host rejects must stay silent"
        );
        let prod = src.split("#[cfg(test)]").next().unwrap_or(src);
        assert!(
            !prod.contains("FirstSelectedObjectAction::Sell"),
            "hq-5mdst: Sell must queue the full selection, not first-selected"
        );
    }

    #[test]
    fn ambiguous_legacy_target_aliases_require_exact_retail_button_keys() {
        assert_eq!(
            host_control_bar_target_unit_ability(
                LegacyCommandType::Enter,
                "Command_GLAInfantryHijack",
            ),
            Some(PendingUnitAbility::Hijack)
        );
        assert_eq!(
            host_control_bar_target_unit_ability(
                LegacyCommandType::Enter,
                "Command_SabotageBuilding",
            ),
            Some(PendingUnitAbility::Sabotage)
        );
        assert_eq!(
            host_control_bar_target_unit_ability(
                LegacyCommandType::ConvertToCarBomb,
                "Command_GLAInfantryTerroristMakeCarBomb",
            ),
            Some(PendingUnitAbility::ConvertToCarbomb)
        );

        assert!(
            host_control_bar_generic_target_action(
                LegacyCommandType::Enter,
                "Command_Enter",
                None,
                0,
                None,
            )
            .is_err()
        );
        assert!(
            host_control_bar_generic_target_action(
                LegacyCommandType::Enter,
                "Command-GLAInfantryHijack",
                None,
                0,
                None,
            )
            .is_err()
        );
        assert!(
            host_control_bar_direct_action(
                LegacyCommandType::ConvertToCarBomb,
                "Command_ConvertToCarBomb",
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn target_buttons_translate_without_name_based_ui_fallback() {
        assert!(matches!(
            host_control_bar_generic_target_action(
                LegacyCommandType::DoAttackMoveTo,
                "Command_AttackMove",
                None,
                0,
                None,
            ),
            Ok(HostControlBarGenericTargetAction::AttackMove)
        ));
        assert!(matches!(
            host_control_bar_generic_target_action(
                LegacyCommandType::DoGuardPosition,
                "Command_GuardFlyingUnitsOnly",
                None,
                0,
                None,
            ),
            Ok(HostControlBarGenericTargetAction::Guard(
                crate::game_logic::GuardMode::FlyingUnitsOnly
            ))
        ));
        assert!(matches!(
            host_control_bar_generic_target_action(
                LegacyCommandType::SetRallyPoint,
                "Command_SetRallyPoint",
                None,
                0,
                None,
            ),
            Ok(HostControlBarGenericTargetAction::SetRallyPoint)
        ));
        assert!(matches!(
            host_control_bar_generic_target_action(
                LegacyCommandType::CombatDropAtLocation,
                "Command_CombatDrop",
                None,
                // Exact parsed retail Command_CombatDrop options:
                // enemy | neutral | ally | position | multi-select | context.
                0x0000_0327,
                None,
            ),
            Ok(HostControlBarGenericTargetAction::CombatDrop(
                PendingCombatDropCommand {
                    options: 0x0000_0327,
                }
            ))
        ));
        assert!(matches!(
            host_control_bar_generic_target_action(
                LegacyCommandType::PlaceBeacon,
                "Command_PlaceBeacon",
                None,
                0,
                None,
            ),
            Ok(HostControlBarGenericTargetAction::PlaceBeacon)
        ));
    }

    #[test]
    fn queue_cancel_uses_production_id_or_index_not_template() {
        // C++ ControlBarCommandProcessing.cpp:466-479 cancels by productionID
        // from the clicked queue slot, not first-match-by-template.
        assert_eq!(resolve_host_queue_cancel_index(1, 0, 3), Some(1));
        assert_eq!(resolve_host_queue_cancel_index(0, 2, 3), Some(2));
        assert_eq!(resolve_host_queue_cancel_index(17, 1, 3), Some(1));
        assert_eq!(resolve_host_queue_cancel_index(0, 5, 3), None);
        assert_eq!(resolve_host_queue_cancel_index(9, 5, 3), None);

        let src = include_str!("control_bar_bridge.rs");
        let start = src
            .find("fn host_apply_control_bar_cancel")
            .expect("cancel apply");
        let apply = &src[start..];
        let apply = apply
            .split("\n    fn host_control_bar_local_selection")
            .next()
            .unwrap();
        assert!(
            apply
                .contains("resolve_host_queue_cancel_index(production_id, queue_index, queue_len)")
                && apply.contains("host_cancel_production_at_index(producer, cancel_index)")
                && !apply.contains("host_cancel_production_and_sync_hud"),
            "live cancel must keep productionID/index identity, not template first-match"
        );
        assert!(
            !src.contains("production_id: _"),
            "QueueCancel must not discard production_id"
        );
    }

    #[test]
    fn exit_direct_command_keeps_occupant_id_not_unload_all() {
        // C++ GameLogicDispatch.cpp:978-1004 MSG_EXIT uses argument 0 occupant.
        let src = include_str!("control_bar_bridge.rs");
        let start = src
            .find("fn host_apply_control_bar_direct(")
            .expect("direct apply");
        let apply = &src[start..];
        let apply = apply
            .split("\n    fn host_apply_control_bar_direct_action")
            .next()
            .unwrap();
        assert!(
            apply.contains("exit_object_id")
                && apply.contains("LegacyCommandType::Exit")
                && apply.contains("vec![occupant_id]")
                && apply.contains("EXIT requires occupant object id"),
            "EXIT must queue the occupant, not the container dump-all"
        );
        assert!(
            !src.contains("exit_object_id: _"),
            "DirectCommand must not discard exit_object_id"
        );
    }
}
