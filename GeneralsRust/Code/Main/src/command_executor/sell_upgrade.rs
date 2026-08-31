//! Sell, science purchase, and upgrade queue/cancel.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    AIState, GameLogic, KindOf, ObjectId, ObjectType, PendingSpecialAbility, Resources, Team,
    radar_notifications::RadarKind,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::AsciiString;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

impl<'a> CommandExecutor<'a> {
    pub(super) fn execute_sell(&mut self, object_id: ObjectId, player_id: u32) -> CommandResult {
        if let Some(obj) = self.game_logic.host_object(object_id) {
            if obj.owner_player_id != Some(player_id)
                || !obj.is_alive()
                || !obj.is_kind_of(KindOf::Structure)
            {
                return CommandResult::InvalidTarget;
            }
            if obj.status.sold
                || obj.is_script_unsellable()
                || self.game_logic.is_object_being_sold(object_id)
            {
                // C++ ControlBar hides GUI_COMMAND_SELL for SCRIPT_UNSELLABLE.
                // sellObject itself has no UC / reconstructing gate.
                return CommandResult::InvalidCommand;
            }
            // C++ BuildAssistant::sellObject multi-frame residual (scaffold → SOLD → refund).
            if self.game_logic.start_sell_object(object_id) {
                CommandResult::Success
            } else {
                CommandResult::InvalidCommand
            }
        } else {
            CommandResult::InvalidTarget
        }
    }

    /// C++ AIGroup::groupSell residual — sell every selected friendly structure.
    pub(crate) fn execute_sell_selected(
        &mut self,
        units: &[ObjectId],
        player_id: u32,
    ) -> CommandResult {
        let mut any = false;
        for &id in units {
            if matches!(self.execute_sell(id, player_id), CommandResult::Success) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Economy Commands ===

    pub(super) fn normalize_command_token(name: &str) -> String {
        name.chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .collect::<String>()
            .to_ascii_lowercase()
    }

    pub(super) fn resolve_science_cost_supplies(science_name: &str) -> u32 {
        // Main currently models science purchases through resources; use deterministic
        // command-name costs instead of unstable string-length heuristics.
        match Self::normalize_command_token(science_name).as_str() {
            "spydrone" | "radarscan" | "cashbounty1" | "cashbounty" => 500,
            "emergencyrepair1" | "emergencyrepair" | "clustermines" | "battleplan1" => 1000,
            "a10strike1" | "carpetbomb1" | "artillerybarrage1" | "anthraxbomb1" => 1500,
            "a10strike2" | "carpetbomb2" | "artillerybarrage2" | "anthraxbomb2" => 2000,
            "a10strike3" | "spectregunship" | "fuelairbomb" | "particlecannon" => 3000,
            "nuclearmissile" | "scudstorm" => 5000,
            _ => 1000,
        }
    }

    pub(super) fn resolve_upgrade_cost_supplies(&self, upgrade_name: &str) -> u32 {
        // Prefer catalog template cost when present.
        if let Some(template) = self.game_logic.get_templates().get(upgrade_name) {
            if template.build_cost.supplies > 0 {
                return template.build_cost.supplies;
            }
        }

        // The live Upgrade.ini loader registers actual upgrade definitions in
        // GameLogic's UpgradeCenter rather than the object ThingTemplate map.
        // Use that parsed cost before falling back to the built-in retail
        // table, so mod/map overrides remain authoritative.
        let parsed_cost = gamelogic::upgrade::center::with_upgrade_center(|center| {
            center
                .find_upgrade(upgrade_name)
                .map(|template| template.get_cost())
        });
        if let Some(cost) = parsed_cost.filter(|cost| *cost > 0) {
            return cost as u32;
        }

        // Wave 79: apply HostUpgradeKind retail Upgrade.ini BuildCost residual.
        use crate::game_logic::host_upgrades::HostUpgradeKind;
        let kind = HostUpgradeKind::from_name(upgrade_name);
        let retail = kind.retail_build_cost();
        if retail > 0 {
            return retail;
        }

        // Fallback residual matrix for non-HostUpgradeKind research names.
        match Self::normalize_command_token(upgrade_name).as_str() {
            "upgradeamericatowmissile" => 800,
            "upgradeamericacompositearmor" => 2000,
            "upgradeamericarangercapturebuilding"
            | "upgradechinaredguardcapturebuilding"
            | "upgradeglarebelcapturebuilding"
            | "upgradeinfantrycapturebuilding" => 1000,
            // Retail Upgrade_AmericaRangerFlashBangGrenade BuildCost 800.
            "upgradeamericaflashbanggrenade" | "upgradeamericarangerflashbanggrenade" => 800,
            // Retail Upgrade_AmericaSupplyLines BuildCost 800.
            "upgradeamericasupplylines" => 800,
            // Retail Upgrade_AmericaAdvancedTraining BuildCost 1500.
            "upgradeamericaadvancedtraining" | "upgradeadvancedtraining" => 1500,
            "upgradeglaapbullets" => 2000,
            // Retail Upgrade_GLAWorkerShoes BuildCost 1000 (was incorrect 500).
            "upgradeglaworkershoes" => 1000,
            "upgradechinanuclearengines" | "upgradechinanucleartanks" => 2000,
            "upgradenationalism" | "upgradefanaticism" => 1500,
            _ => 1000,
        }
    }

    /// C++ `UpgradeTemplate::calcTimeToBuild` source for a ControlBar
    /// research entry.  Upgrade.ini is loaded into the global UpgradeCenter,
    /// not the ThingTemplate catalog, so querying the parsed template is what
    /// keeps the live queue in sync with retail and modded BuildTime values.
    /// A zero/missing value keeps the old minimum-one-logic-frame behavior
    /// only for unknown legacy entries.
    pub(super) fn resolve_upgrade_build_time_secs(&self, upgrade_name: &str) -> f32 {
        let parsed_secs = gamelogic::upgrade::center::with_upgrade_center(|center| {
            center
                .find_upgrade(upgrade_name)
                .map(|template| template.get_build_time())
        });
        let fallback_secs =
            crate::game_logic::host_upgrades::HostUpgradeKind::from_name(upgrade_name)
                .retail_build_time_secs();
        parsed_secs
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
            .unwrap_or(fallback_secs)
            .max(1.0 / 30.0)
    }

    pub(super) fn execute_purchase_science(
        &mut self,
        player_id: u32,
        science_name: &str,
    ) -> CommandResult {
        if science_name.trim().is_empty() {
            return CommandResult::InvalidCommand;
        }

        // C++ Player::attemptToPurchaseScience residual: science purchase points,
        // not supply cash. Cost 0 / missing prereqs / insufficient points → fail.
        let unlocked = {
            let Some(player) = self.game_logic.get_player_mut(player_id) else {
                return CommandResult::InvalidCommand;
            };
            if !player.attempt_to_purchase_science(science_name) {
                return CommandResult::InvalidCommand;
            }
            debug!(
                "Player {} purchased science {} (spp left={})",
                player_id, science_name, player.science_purchase_points
            );
            true
        };

        if unlocked {
            // SCIENCE_StealthFighter residual: record unlock honesty on purchase.

            if crate::game_logic::host_stealth_fighter::is_stealth_fighter_science(science_name) {
                self.game_logic.record_stealth_fighter_science_unlock();
            }
            // C++ SpecialPowerModule::onSpecialPowerCreation residual.
            self.game_logic
                .on_special_power_science_creation(player_id, science_name);
            crate::game_logic::host_sp_science_upgrade_player_team_residual_wave109::sync_host_science_to_crate_player(
                player_id,
                science_name,
            );
            return CommandResult::Success;
        }
        CommandResult::InvalidCommand
    }

    pub(super) fn execute_queue_upgrade(
        &mut self,
        units: &[ObjectId],
        upgrade_name: &str,
    ) -> CommandResult {
        if upgrade_name.trim().is_empty() {
            return CommandResult::InvalidCommand;
        }

        use crate::game_logic::buildings::DEFAULT_PRODUCTION_QUEUE_LIMIT;

        let mut seen_players = HashSet::new();
        let mut any = false;
        let cost = Resources {
            supplies: self.resolve_upgrade_cost_supplies(upgrade_name),
            power: 0,
        };
        // Collect successful queues then record honesty (avoids borrow conflicts).
        let mut recorded: Vec<(u32, crate::game_logic::Team, ObjectId)> = Vec::new();
        for &unit_id in units {
            // C++ queueMaxed / MaxQueueEntries residual: refuse before charging.
            let producer_ok =
                self.game_logic.host_object(unit_id).is_some_and(|source| {
                    if !Self::can_source_queue_upgrade(source) {
                        return false;
                    }
                    let Some(building) = source.building_data.as_ref() else {
                        return false;
                    };
                    if building.production_queue.len() >= DEFAULT_PRODUCTION_QUEUE_LIMIT {
                        return false;
                    }
                    // C++ isUpgradeInQueue residual.
                    if building.production_queue.iter().any(|i| {
                        i.is_upgrade() && i.template_name.eq_ignore_ascii_case(upgrade_name)
                    }) {
                        return false;
                    }
                    // C++ queueUpgrade OBJECT: hasUpgrade || !affectedByUpgrade.
                    if crate::game_logic::host_upgrades::is_object_scoped_upgrade(upgrade_name)
                        && source.refuses_object_upgrade(upgrade_name)
                    {
                        return false;
                    }
                    // C++ ProductionUpdate::queueUpgrade — STOP cheaters:
                    // Object::canProduceUpgrade CommandSet walk.
                    if !crate::game_logic::host_upgrades::object_can_produce_upgrade(
                        source,
                        upgrade_name,
                    ) {
                        return false;
                    }

                    true
                });
            if !producer_ok {
                continue;
            }
            // `GameCommand::player_id` and the selected producer identify the
            // actual research owner.  A faction can have multiple players;
            // never charge/queue against the first player with this Team.
            // C++ ProductionUpdate::queueUpgrade charges the producer's
            // getControllingPlayer(); a host producer without a concrete
            // owner resolves through the legacy team-owner fallback when the
            // whole world is ownerless (synthetic/scripted spawns).
            let owner = self.game_logic.host_object(unit_id).and_then(|source| {
                if source.owner_player_id == Some(self.current_player_id) {
                    Some((self.current_player_id, source.team))
                } else if source.owner_player_id.is_none()
                    && self.game_logic.uses_legacy_team_ownership_fallback()
                {
                    self.game_logic.unique_player_id_for_team(source.team)
                        .map(|player_id| (player_id, source.team))
                } else {
                    None
                }
            });
            let Some((player_id, team)) = owner else {
                continue;
            };
            if !seen_players.insert(player_id) {
                continue;
            }
            let player_is_matching_owner = self
                .game_logic
                .get_player(player_id)
                .is_some_and(|player| player.is_alive && player.team == team);
            if !player_is_matching_owner {
                continue;
            }
            if let Some(player) = self.game_logic.get_player_mut(player_id) {
                if player.queue_upgrade(upgrade_name, &cost) {
                    recorded.push((player_id, team, unit_id));
                }
            }
        }
        for (player_id, team, unit_id) in recorded {
            // C++ ProductionUpdate::queueUpgrade — research advances on the producer.
            let research_secs = self.resolve_upgrade_build_time_secs(upgrade_name);
            let research_frames = (research_secs * 30.0).round().max(1.0) as u32;
            // Wave 233: producer upgrade queue via GameLogic authority API.
            let building_queued = self.game_logic.unit_command_building_add_upgrade_to_queue(
                unit_id,
                upgrade_name,
                research_secs,
                cost.clone(),
            );
            if !building_queued {
                // Fail-closed: refund player if PRODUCTION_UPGRADE could not be placed.
                if let Some(player) = self.game_logic.get_player_mut(player_id) {
                    let _ = player.cancel_queued_upgrade(upgrade_name, &cost);
                }
                continue;
            }
            self.game_logic.record_host_upgrade_queued(
                player_id,
                team,
                upgrade_name,
                Some(unit_id),
            );
            // Wave 79: stamp residual build cost paid (retail application honesty).
            self.game_logic.host_upgrades_mut().set_build_cost_paid(
                upgrade_name,
                player_id,
                cost.supplies,
            );
            self.game_logic
                .host_upgrades_mut()
                .set_resolved_research_frames(upgrade_name, player_id, research_frames);
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_cancel_upgrade(
        &mut self,
        units: &[ObjectId],
        upgrade_name: &str,
    ) -> CommandResult {
        // C++ cancel queue head residual: Command_CancelUpgrade may omit the name;
        // resolve from the first selected producer's PRODUCTION_UPGRADE head.
        let resolved_name = if upgrade_name.trim().is_empty() {
            units.iter().find_map(|&unit_id| {
                self.game_logic.host_object(unit_id).and_then(|obj| {
                    obj.building_data.as_ref().and_then(|b| {
                        b.production_queue
                            .iter()
                            .find(|i| i.is_upgrade())
                            .map(|i| i.template_name.clone())
                    })
                })
            })
        } else {
            Some(upgrade_name.to_string())
        };
        let Some(upgrade_name) = resolved_name.filter(|s| !s.trim().is_empty()) else {
            return CommandResult::InvalidCommand;
        };

        let mut seen_players = HashSet::new();
        let mut refunded = false;
        let refund = Resources {
            supplies: self.resolve_upgrade_cost_supplies(&upgrade_name),
            power: 0,
        };
        let mut cancelled_players: Vec<u32> = Vec::new();
        for &unit_id in units {
            let owner = self
                .game_logic
                .host_object(unit_id)
                .filter(|source| Self::can_source_queue_upgrade(source))
                .and_then(|source| {
                    if source.owner_player_id == Some(self.current_player_id) {
                        Some((self.current_player_id, source.team))
                    } else if source.owner_player_id.is_none()
                        && self.game_logic.uses_legacy_team_ownership_fallback()
                    {
                        // C++ getControllingPlayer legacy team-owner fallback
                        // for ownerless synthetic/scripted producers.
                        self.game_logic.unique_player_id_for_team(source.team)
                            .map(|player_id| (player_id, source.team))
                    } else {
                        None
                    }
            });
            let Some((player_id, team)) = owner else {
                continue;
            };
            if !seen_players.insert(player_id) {
                continue;
            }
            let player_is_matching_owner = self
                .game_logic
                .get_player(player_id)
                .is_some_and(|player| player.is_alive && player.team == team);
            if !player_is_matching_owner {
                continue;
            }
            if let Some(player) = self.game_logic.get_player_mut(player_id) {
                if player.cancel_queued_upgrade(&upgrade_name, &refund) {
                    refunded = true;
                    cancelled_players.push(player_id);
                }
            }
        }
        for player_id in cancelled_players {
            self.game_logic
                .record_host_upgrade_cancelled(player_id, &upgrade_name);
        }
        // C++ cancelUpgrade also removes the PRODUCTION_UPGRADE entry from the producer queue.
        let mut removed_from_building = false;
        for &unit_id in units {
            // Wave 233: remove producer upgrade queue entry via GameLogic authority API.
            if self
                .game_logic
                .unit_command_building_remove_upgrade_from_queue(unit_id, &upgrade_name)
            {
                removed_from_building = true;
            }
        }
        // If player queue was already empty but building still held the entry, treat as success
        // after removing the PRODUCTION_UPGRADE residual (refund already applied or N/A).
        if refunded || removed_from_building {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn can_source_queue_upgrade(source: &crate::game_logic::Object) -> bool {
        source.building_data.is_some() && source.is_alive() && source.is_constructed()
    }
}
