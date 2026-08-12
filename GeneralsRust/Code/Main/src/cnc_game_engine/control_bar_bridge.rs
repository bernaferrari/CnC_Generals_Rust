//! Delivery of live GameClient Control Bar actions into Main's offline world.
//!
//! GameClient keeps the original C++ message-stream route for standalone use.
//! The executable deliberately has a different, Rust-owned simulation world,
//! so an enabled bridge must translate the typed UI request here instead of
//! allowing a click to mutate an unused legacy GameLogic singleton.

#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]

use super::*;
use game_client::gui::control_bar::{
    take_host_control_bar_requests, HostControlBarRequest, HostControlBarTarget,
    QueueProductionType,
};
use gamelogic::commands::CommandType as LegacyCommandType;

const COMMAND_OPTION_ONE: u32 = 0x0000_2000;
const COMMAND_OPTION_TWO: u32 = 0x0000_4000;
const COMMAND_OPTION_THREE: u32 = 0x0000_8000;

impl CnCGameEngine {
    /// Drain Control Bar work at the single-authority boundary.
    ///
    /// Requests are validated against the immutable presentation snapshot
    /// before they are allowed to alter Main's host world.  That prevents a
    /// stale UI context, an enemy id, or a legacy object id from being treated
    /// as an authority command merely because it reached this queue.
    pub(crate) fn host_tick_control_bar_bridge(&mut self) {
        let requests = take_host_control_bar_requests();
        if requests.is_empty() {
            return;
        }

        if !matches!(self.current_state, GameState::InGame) {
            debug!(
                "discarded {} Control Bar requests outside an in-game offline world",
                requests.len()
            );
            return;
        }

        // Multiplayer/replay ownership remains deferred.  Switch the client
        // back to its own legacy route rather than silently applying commands
        // to an offline-only host world.
        if matches!(
            self.host_match_game_mode,
            Some(
                crate::game_logic::GameMode::Multiplayer
                    | crate::game_logic::GameMode::Internet
                    | crate::game_logic::GameMode::Lan
                    | crate::game_logic::GameMode::Replay
            )
        ) {
            warn!(
                "Control Bar host bridge is offline-only; restoring the legacy route for this session"
            );
            game_client::gui::control_bar::set_host_control_bar_bridge_enabled(false);
            return;
        }

        for request in requests {
            self.host_apply_control_bar_request(request);
        }
    }

    fn host_apply_control_bar_request(&mut self, request: HostControlBarRequest) {
        match request {
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
                object_name,
                upgrade_name: _,
                selected_object_ids,
                player_id,
                source: _,
                science_names,
                special_power_name,
                special_power_id: _,
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
            ),
            HostControlBarRequest::ArmTarget {
                command_name,
                command_type,
                options: _,
                weapon_slot,
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
                weapon_slot,
                &selected_object_ids,
                player_id,
                target,
            ),
            HostControlBarRequest::QueueCancel {
                player_id,
                selected_object_ids,
                producer_id,
                production_id: _,
                production_type,
                upgrade_name,
                queue_index,
            } => self.host_apply_control_bar_cancel(
                player_id,
                &selected_object_ids,
                crate::game_logic::ObjectId(producer_id),
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
    ) {
        if !self.host_control_bar_is_local_player(player_id) {
            return;
        }

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
            && !self.host_control_bar_command_allows_empty_selection(command_name)
        {
            self.host_reject_control_bar_request(
                "direct command has no local selectable selection",
            );
            return;
        }
        if !selected.is_empty() {
            self.host_set_selection(player_id, selected.clone());
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
        // button is only meaningful for the retail Demo tertiary suicide
        // action; do not fire an arbitrary default weapon at a made-up target.
        if command_type == LegacyCommandType::DoAttackObject {
            if host_control_bar_key(command_name).contains("demotertiarysuicide") {
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

        self.issue_named_command_from_ui(command_name);
    }

    fn host_apply_control_bar_target(
        &mut self,
        command_name: &str,
        command_type: LegacyCommandType,
        weapon_slot: Option<u32>,
        selected_object_ids: &[u32],
        player_id: u32,
        target: HostControlBarTarget,
    ) {
        let selected = self.host_control_bar_local_selection(player_id, selected_object_ids);
        if selected.is_empty() {
            self.host_reject_control_bar_request(
                "target command has no local selectable selection",
            );
            return;
        }
        self.host_set_selection(player_id, selected);

        match target {
            HostControlBarTarget::DozerConstruct { template_name } => {
                if template_name.trim().is_empty() {
                    self.host_reject_control_bar_request("dozer construction has no template name");
                    return;
                }
                self.begin_structure_placement_from_ui(&template_name);
            }
            HostControlBarTarget::SpecialPower {
                special_power_name,
                special_power_id: _,
            } => {
                let special_key = host_control_bar_key(&special_power_name);
                if special_key == "specialabilityboobytrap" {
                    self.arm_pending_unit_ability(
                        PendingUnitAbility::PlantBoobyTrap,
                        "Booby trap: click structure",
                    );
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
                let cursor = Self::radius_cursor_type_for_special_power(&power_type);
                self.pending_map_command = Some(PendingMapCommand::SpecialPower(power_type));
                self.pending_structure_placement = None;
                self.arm_radius_cursor_for_pending(cursor);
            }
            HostControlBarTarget::Generic => {
                if command_type == LegacyCommandType::DoAttackObject {
                    let Some(weapon_slot) = host_control_bar_weapon_slot(weapon_slot) else {
                        self.host_reject_control_bar_request(
                            "FIRE_WEAPON button has no valid explicit slot",
                        );
                        return;
                    };
                    self.pending_map_command = Some(PendingMapCommand::Weapon(weapon_slot));
                    self.pending_structure_placement = None;
                    self.arm_radius_cursor_for_pending("OFFENSIVE_SPECIALPOWER");
                } else {
                    self.issue_named_command_from_ui(command_name);
                }
            }
        }
    }

    fn host_apply_control_bar_cancel(
        &mut self,
        player_id: u32,
        selected_object_ids: &[u32],
        producer: crate::game_logic::ObjectId,
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
        let Some(item) = self
            .presentation_ro(producer)
            .and_then(|object| object.production_queue.get(queue_index))
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
        if self.host_cancel_production_at_index(producer, queue_index) {
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

    fn host_control_bar_command_allows_empty_selection(&self, command_name: &str) -> bool {
        matches!(
            host_control_bar_key(command_name).as_str(),
            "commandviewcommandcenter"
                | "commandviewlastradarevent"
                | "commandplacebeacon"
                | "commandremovebeacon"
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
        debug!("Control Bar host request rejected: {reason}");
        self.game_hud.push_info_message(reason);
        self.ui_manager.game_hud_mut().push_info_message(reason);
    }
}

/// The host never receives a raw legacy enum discriminant for a weapon slot.
/// GameClient already emits the three C++ values as explicit numbers; retain
/// that contract in one checked conversion and reject unknown values.
fn host_control_bar_weapon_slot(slot: Option<u32>) -> Option<crate::command_system::WeaponSlot> {
    match slot? {
        0 => Some(crate::command_system::WeaponSlot::Primary),
        1 => Some(crate::command_system::WeaponSlot::Secondary),
        2 => Some(crate::command_system::WeaponSlot::Tertiary),
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
