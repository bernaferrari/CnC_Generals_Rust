//! Hijack, sabotage, demo, hack, disguise, and similar abilities.
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
    pub(super) fn execute_hack_internet(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if self.game_logic.start_hacker_internet_hack(unit_id) {
                any = true;
            }
        }
        if any {
            // C++ CommandXlat.cpp:654-658 MSG_INTERNET_HACK → PerUnitSound VoiceHackInternet.
            self.game_logic.queue_picked_unit_voice(
                units,
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::HackInternet,
            );
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    // === Special Unit Abilities ===

    pub(crate) fn execute_hijack(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        // C++ ConvertToHijackedVehicleCrateCollide residual: enemy ground vehicle
        // only, not already HIJACKED, not neutral, not airborne.
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_hijacked,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_hijacked(),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !target_alive
            || !target_is_vehicle
            || target_is_airborne
            || target_hijacked
            || target_team == Team::Neutral
        {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();

        for &unit_id in units {
            let can_issue = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    unit.is_alive()
                        && unit.can_move()
                        && unit.team != target_team
                        && unit.template_name.to_ascii_lowercase().contains("hijacker")
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::Hijack { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_sabotage(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        // C++ Sabotage*CrateCollide residual: GLA Saboteur only → enemy structure.
        let (target_team, target_pos, target_alive, target_is_structure) =
            match self.game_logic.host_object(target_id) {
                Some(target) => (
                    target.team,
                    target.get_position(),
                    target.is_alive(),
                    target.is_kind_of(KindOf::Structure),
                ),
                None => return CommandResult::InvalidTarget,
            };

        if !target_alive || !target_is_structure || target_team == Team::Neutral {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    unit.is_alive()
                        && unit.can_move()
                        && unit.team != target_team
                        && crate::game_logic::host_saboteur::is_saboteur_template(
                            &unit.template_name,
                        )
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::Sabotage { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(crate) fn execute_convert_carbomb(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        // C++ ConvertToCarBombCrateCollide: vehicle only (not aircraft/boat),
        // not already IS_CARBOMB. Neutral civilian cars are valid.
        let (target_pos, target_team, target_ok) = match self.game_logic.host_object(target_id) {
            Some(target) if target.is_alive() => {
                let is_vehicle = target.is_kind_of(KindOf::Vehicle);
                let is_airborne =
                    target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target;
                let already_bomb = target.status.is_carbomb;
                (
                    target.get_position(),
                    target.team,
                    is_vehicle && !is_airborne && !already_bomb,
                )
            }
            Some(_) => return CommandResult::InvalidTarget,
            None => return CommandResult::InvalidTarget,
        };
        if !target_ok {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    unit.is_alive()
                        && unit.can_move()
                        && unit_id != target_id
                        && crate::game_logic::is_terrorist_template(&unit.template_name)
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::CarBomb { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_capture_building(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        self.execute_capture_building_for_power(units, target_id, None)
    }

    /// Capture authority used both by the ordinary RMB action and an explicit
    /// capture SpecialPower command.  The optional kind prevents a caller
    /// from asking a Ranger command button to silently fire a different
    /// capture module on the same selection.
    pub(super) fn execute_capture_building_for_power(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
        required_power: Option<crate::game_logic::CapturePowerKind>,
    ) -> CommandResult {
        let building_pos = match self.game_logic.host_object(target_id) {
            Some(building) => building.get_position(),
            None => return CommandResult::InvalidTarget,
        };

        let mut any = false;
        for &unit_id in units {
            if unit_id == target_id {
                continue;
            }

            let Some(power) = self
                .game_logic
                .host_object(unit_id)
                .and_then(|unit| unit.thing.template.capture_power.special_power_type())
            else {
                continue;
            };
            if required_power.is_some_and(|required| {
                crate::game_logic::CapturePowerKind::from_special_power_type(&power) != required
            }) || !self
                .game_logic
                .can_unit_capture_building(unit_id, target_id, true)
            {
                continue;
            }

            // C++ `initiateIntentToDoSpecialPower` records a capture intent
            // and approaches the target, but does not begin ReloadTime until
            // `SpecialAbilityUpdate::startPreparation` in StartAbilityRange.
            if !self
                .game_logic
                .unit_command_begin_capture(unit_id, target_id)
            {
                continue;
            }
            // `assign_unit_path` quite correctly returns false when the
            // source is already at the target's center.  That is an accepted
            // C++ capture order, not a path failure: keep its target and
            // explicitly enter Capturing so the state machine can apply the
            // authored StartAbilityRange on the next logic tick.
            let already_at_target = self
                .game_logic
                .host_object(unit_id)
                .is_some_and(|unit| unit.get_position().distance(building_pos) <= 0.1);
            if self.path_to_goal_with_state(unit_id, building_pos, AIState::Capturing)
                || already_at_target
            {
                any = true;
            } else {
                let _ = self
                    .game_logic
                    .unit_command_stop_moving_order_target(unit_id, None);
                let _ = self
                    .game_logic
                    .unit_command_set_ai_state(unit_id, AIState::Idle);
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_snipe_vehicle(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let (
            target_team,
            target_pos,
            target_radius,
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_unmanned,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.selection_radius,
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_unmanned(),
            ),
            None => return CommandResult::InvalidTarget,
        };

        // Kill-pilot residual only applies to manned enemy ground vehicles.
        if !target_alive
            || !target_is_vehicle
            || target_is_airborne
            || target_unmanned
            || target_team == Team::Neutral
        {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let Some((unit_pos, unit_radius, can_issue)) =
                self.game_logic.host_object(unit_id).map(|unit| {
                    (
                        unit.get_position(),
                        unit.selection_radius,
                        unit.is_alive() && unit.can_move() && unit.team != target_team,
                    )
                })
            else {
                continue;
            };
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            // C++ CommandXlat.cpp:548-556 VoiceSnipePilot + ActionManager.cpp:1944-1950
            // MSG_DO_WEAPON_AT_OBJECT DAMAGE_KILLPILOT. Retail
            // GLAJarmenKellVehiclePilotSniperRifle AttackRange is 225, not
            // contact radii+4. Already-in-range must not require a path to
            // the occupied vehicle cell.
            let in_killpilot_range =
                crate::game_logic::host_hero_abilities::leftover_snipe_in_killpilot_range(
                    unit_pos,
                    unit_radius,
                    target_pos,
                    target_radius,
                );
            if in_killpilot_range {
                let _ = self
                    .game_logic
                    .unit_command_set_ai_state(unit_id, AIState::SpecialAbility);
                issued_units.push(unit_id);
                any = true;
            } else if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::SnipeVehicle { target_id },
            );
        }

        if any {
            // C++ CommandXlat.cpp:548-556 MSG_DO_WEAPON_AT_OBJECT DAMAGE_KILLPILOT.
            self.game_logic.queue_picked_unit_voice(
                units,
                crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::SnipePilot,
            );
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Colonel Burton residual: plant timed demo charge on enemy structure/vehicle.

    pub(super) fn execute_plant_booby_trap(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let (target_team, target_pos, target_alive, target_is_structure) =
            match self.game_logic.host_object(target_id) {
                Some(target) => (
                    target.team,
                    target.get_position(),
                    target.is_alive(),
                    target.is_kind_of(KindOf::Structure),
                ),
                None => return CommandResult::InvalidTarget,
            };

        if !target_alive || !target_is_structure {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        let target_owner = self
            .game_logic
            .host_object(target_id)
            .and_then(|target| target.owner_player_id);
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    use crate::game_logic::host_booby_trap::{
                        has_booby_trap_upgrade, is_booby_trap_planter_template,
                    };
                    use gamelogic::common::Relationship;
                    // C++ ActionManager.cpp:1610-1618 — STRUCTURE && (NEUTRAL || ALLIES).
                    let rel = match (unit.owner_player_id, target_owner) {
                        (Some(src), Some(tgt)) => self.game_logic.player_relationship(src, tgt),
                        _ => Relationship::Neutral,
                    };
                    unit.is_alive()
                        && unit.can_move()
                        && matches!(rel, Relationship::Neutral | Relationship::Allies)
                        && is_booby_trap_planter_template(&unit.template_name)
                        && has_booby_trap_upgrade(&unit.applied_upgrades)
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::PlantBoobyTrap { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_plant_timed_demo_charge(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_is_vehicle,
            target_is_airborne,
            target_is_bridge,
            target_is_bridge_tower,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_kind_of(KindOf::Bridge),
                target.is_kind_of(KindOf::BridgeTower),
            ),
            None => return CommandResult::InvalidTarget,
        };

        let valid_target = crate::game_logic::host_hero_abilities::leftover_charge_plant_target_ok(
            target_alive,
            target_is_bridge,
            target_is_bridge_tower,
            target_is_structure,
            target_is_vehicle && !target_is_airborne,
        ) && target_team != Team::Neutral;
        if !valid_target {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| unit.is_alive() && unit.can_move() && unit.team != target_team)
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::PlantTimedDemoCharge { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Colonel Burton residual: plant remote demo charge on enemy structure/vehicle.
    /// Fail-closed: not full StickyBombUpdate attach bones / max-charge list.
    pub(super) fn execute_plant_remote_demo_charge(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_is_vehicle,
            target_is_airborne,
            target_is_bridge,
            target_is_bridge_tower,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_kind_of(KindOf::Bridge),
                target.is_kind_of(KindOf::BridgeTower),
            ),
            None => return CommandResult::InvalidTarget,
        };

        let valid_target = crate::game_logic::host_hero_abilities::leftover_charge_plant_target_ok(
            target_alive,
            target_is_bridge,
            target_is_bridge_tower,
            target_is_structure,
            target_is_vehicle && !target_is_airborne,
        ) && target_team != Team::Neutral;
        if !valid_target {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| unit.is_alive() && unit.can_move() && unit.team != target_team)
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::PlantRemoteDemoCharge { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Colonel Burton residual: detonate all remote charges planted by selected units.
    /// Matches C++ SPECIAL_REMOTE_CHARGES no-target path (detonate special object list).
    pub(super) fn execute_detonate_remote_demo_charges(
        &mut self,
        units: &[ObjectId],
    ) -> CommandResult {
        let producers: Vec<ObjectId> = units
            .iter()
            .copied()
            .filter(|id| {
                self.game_logic
                    .host_object(*id)
                    .map(|u| u.is_alive())
                    .unwrap_or(false)
            })
            .collect();
        if producers.is_empty() {
            return CommandResult::InvalidCommand;
        }
        let detonated = self.game_logic.detonate_remote_demo_charges(&producers);
        if detonated > 0 {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Demo SuicideBomb residual: intentional SUICIDED PlusFire detonation.
    ///
    /// Fail-closed: requires SuicideBomb CommandSetUpgrade residual tag.
    pub(super) fn execute_demo_tertiary_suicide(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if self.game_logic.issue_demo_tertiary_suicide(unit_id) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Black Lotus residual: steal cash from enemy supply/cash building.
    ///
    /// Fail-closed: only Black Lotus templates; target must be residual
    /// cash generator (C++ KINDOF_CASH_GENERATOR). StartAbilityRange 150
    /// resolved on reach in GameLogic SpecialAbility update.
    pub(super) fn execute_steal_cash_hack(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        use crate::game_logic::host_hero_abilities::{
            can_activate_black_lotus_ability, is_black_lotus_template, is_cash_hack_target,
            is_legal_steal_cash_target,
        };

        let (
            target_team,
            target_pos,
            target_alive,
            target_is_structure,
            target_under_construction,
            is_cash_generator,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Structure),
                target.status.under_construction,
                is_cash_hack_target(
                    &target.template_name,
                    target.is_kind_of(KindOf::SupplyCenter),
                    target.is_kind_of(KindOf::FSSupplyCenter),
                    target.is_kind_of(KindOf::FSBlackMarket),
                    target.is_kind_of(KindOf::FSSupplyDropzone),
                ),
            ),
            None => return CommandResult::InvalidTarget,
        };

        // Target residual: enemy cash generator structure (not under construction).
        // Per-unit enemy check below; here require non-neutral cash structure.
        if !is_legal_steal_cash_target(
            target_alive,
            target_is_structure,
            target_under_construction,
            target_team != Team::Neutral,
            is_cash_generator,
        ) {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    can_activate_black_lotus_ability(
                        is_black_lotus_template(&unit.template_name),
                        unit.is_alive(),
                    ) && unit.can_move()
                        && unit.team != target_team
                        && unit.team != Team::Neutral
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::StealCashHack { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Black Lotus residual: disable enemy ground vehicle (DISABLED_HACKED).
    ///
    /// Fail-closed: only Black Lotus templates. C++ ActionManager
    /// canDisableVehicleViaHacking residual: enemy ground vehicle, not already
    /// hacked-disabled, not unmanned. StartAbilityRange 150 on reach.
    pub(super) fn execute_disable_vehicle_hack(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        use crate::game_logic::host_hero_abilities::{
            can_activate_black_lotus_ability, is_black_lotus_template,
            is_legal_disable_vehicle_target,
        };

        let (
            target_team,
            target_pos,
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_hacked,
            target_unmanned,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.team,
                target.get_position(),
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_hacked_disabled(),
                target.is_unmanned(),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !is_legal_disable_vehicle_target(
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_team != Team::Neutral,
            target_hacked,
            target_unmanned,
        ) {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    can_activate_black_lotus_ability(
                        is_black_lotus_template(&unit.template_name),
                        unit.is_alive(),
                    ) && unit.can_move()
                        && unit.team != target_team
                        && unit.team != Team::Neutral
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            if self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                issued_units.push(unit_id);
                any = true;
            }
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::DisableVehicleHack { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// Disable-building residual (DISABLED_HACKED).
    /// SpecialAbilityHackerDisableBuilding / SpecialAbilityMicrowaveDisableBuilding.
    pub(super) fn execute_hacker_disable_building(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            // C++ ActionManager authority is module- and readiness-based.
            // Do not derive HDB from the Hacker basename, nor consume its
            // reload at click time: the typed channel does so in
            // SpecialAbilityUpdate::startPreparation.
            if !self
                .game_logic
                .can_unit_hacker_disable_building(unit_id, target_id, true)
            {
                continue;
            }
            if !self
                .game_logic
                .unit_command_begin_hacker_disable_building(unit_id, target_id)
            {
                continue;
            }
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::HackerDisableBuilding { target_id },
            );
            any = true;
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_disguise_as_vehicle(
        &mut self,
        units: &[ObjectId],
        target_id: ObjectId,
    ) -> CommandResult {
        use crate::game_logic::host_bomb_truck_disguise::{
            is_bomb_truck_template, is_legal_disguise_target,
        };
        use crate::game_logic::host_car_bomb::object_definition_has_kind;

        let (
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_is_boat,
            target_disguised,
            target_template,
            target_pos,
        ) = match self.game_logic.host_object(target_id) {
            Some(target) => (
                target.is_alive(),
                target.is_kind_of(KindOf::Vehicle),
                target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
                target.is_kind_of(KindOf::Boat)
                    || object_definition_has_kind(&target.template_name, "BOAT"),
                target.status.disguised,
                target.template_name.clone(),
                target.get_position(),
            ),
            None => return CommandResult::InvalidTarget,
        };

        if !is_legal_disguise_target(
            target_alive,
            target_is_vehicle,
            target_is_airborne,
            target_is_boat,
            &target_template,
            target_disguised,
        ) {
            return CommandResult::InvalidTarget;
        }

        let mut any = false;
        let mut issued_units = Vec::new();
        for &unit_id in units {
            let can_issue = self
                .game_logic
                .host_object(unit_id)
                .map(|unit| {
                    unit.is_alive()
                        && unit.can_move()
                        && unit_id != target_id
                        && is_bomb_truck_template(&unit.template_name)
                })
                .unwrap_or(false);
            if !can_issue {
                continue;
            }

            // Wave 233: stop-moving + order-target via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_stop_moving_order_target(unit_id, Some(target_id));
            // C++ SpecialAbilityDisguiseAsVehicle StartAbilityRange = 1e6
            // residual: the ability arms and completes without an approach
            // walk, so a failed A* allocation (e.g. no loaded map in a bare
            // world) must not swallow the order.
            if !self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
                let _ = self
                    .game_logic
                    .unit_command_set_ai_state(unit_id, AIState::SpecialAbility);
            }
            issued_units.push(unit_id);
            any = true;
        }

        for unit_id in issued_units {
            self.game_logic.queue_pending_special_ability(
                unit_id,
                PendingSpecialAbility::DisguiseAsVehicle { target_id },
            );
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_switch_weapons(&mut self, units: &[ObjectId], slot: u8) -> CommandResult {
        // C++ GameLogicDispatch.cpp:583-590 MSG_SWITCH_WEAPONS:
        // currentlySelectedGroup->setWeaponLockForGroup(weaponSlot, LOCKED_PERMANENTLY).
        let result = self.execute_set_weapon_lock(units, slot, 2);
        if result == CommandResult::Success {
            // C++ CommandXlat.cpp:474-493 / ControlBarCommandProcessing.cpp:788-792
            // pickAndPlay PerUnitSound Voice*WeaponMode for the locked slot (skip=true).
            let voice = match slot {
                0 => Some(crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::PrimaryWeaponMode),
                1 => {
                    Some(crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::SecondaryWeaponMode)
                }
                2 => {
                    Some(crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::TertiaryWeaponMode)
                }
                _ => None,
            };
            if let Some(voice) = voice {
                self.game_logic.queue_picked_unit_voice(units, voice);
            }
        }
        result
    }

    pub(super) fn execute_toggle_overcharge(&mut self, units: &[ObjectId]) -> CommandResult {
        let mut any = false;
        for &unit_id in units {
            if self.game_logic.toggle_overcharge_object(unit_id) {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }
}
