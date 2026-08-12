//! Special powers, weapons, and charge-queue helpers.
#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
use crate::command_system::{
    CommandResult, CommandType, DropTarget, GameCommand, GuardTarget, PowerTarget,
    SpecialPowerType, WeaponSlot, WeaponTarget,
};
use crate::game_logic::game_logic::AudioEventRequest;
use crate::game_logic::{
    radar_notifications::RadarKind, AIState, GameLogic, KindOf, ObjectId, ObjectType,
    PendingSpecialAbility, Resources, Team,
};
use crate::localization;
use crate::ui::audio::translate_audio_event;
use gamelogic::common::types::Coord3D as LogicCoord3D;
use gamelogic::common::AsciiString;
use gamelogic::system::beacon_manager::get_beacon_manager;
use gamelogic::system::game_logic::current_frame;
use glam::Vec3;
use log::{debug, warn};
use std::collections::{HashMap, HashSet};

/// Translate the public command slot into the host object's indexed weapon set.
///
/// Keep the conversion explicit so an unrepresented `AntiAir` request, or an
/// arbitrary slot value, can never silently fall through to PRIMARY.  The host
/// carries three concrete WeaponSet slots; their presence is validated by
/// `unit_command_select_weapon_slot` before an attack is issued.
fn host_weapon_slot_index(weapon_slot: &WeaponSlot) -> Option<u8> {
    match weapon_slot {
        WeaponSlot::Primary => Some(0),
        WeaponSlot::Secondary => Some(1),
        // Preserve the C++ ordinal; the target unit must actually carry a
        // tertiary weapon before the command is accepted.
        WeaponSlot::Tertiary => Some(2),
        // This is a target capability, not an Object weapon-set ordinal.
        WeaponSlot::AntiAir => None,
        // Do not truncate an arbitrary command value into a different slot.
        WeaponSlot::Slot(slot) => u8::try_from(*slot).ok(),
    }
}

impl<'a> CommandExecutor<'a> {
    /// C++ AIGroup::groupOverrideSpecialPowerDestination residual.
    pub(crate) fn execute_override_special_power_destination(
        &mut self,
        units: &[ObjectId],
        location: Vec3,
    ) -> CommandResult {
        // Wave 233: special-power destination override via GameLogic authority API.
        if !location.x.is_finite() || !location.z.is_finite() {
            return CommandResult::InvalidLocation;
        }
        let mut any = false;
        for &unit_id in units {
            if self
                .game_logic
                .unit_command_set_special_power_overridable_destination(unit_id, location)
            {
                any = true;
            }
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::getSpecialPowerSourceObject residual —
    /// first living member that can execute `power_type`.
    pub(crate) fn special_power_source_object(
        &self,
        units: &[ObjectId],
        power_type: &crate::command_system::SpecialPowerType,
    ) -> Option<ObjectId> {
        // C++ walks members for SpecialPowerModule matching template.
        // Host: only members that explicitly track this power (cooldown map).
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            if o.special_power_cooldowns.contains_key(power_type) {
                return Some(id);
            }
        }
        None
    }

    /// C++ AIGroup::groupDoSpecialPowerAtLocation residual.
    pub(crate) fn execute_special_power_at_location(
        &mut self,
        units: &[ObjectId],
        power_type: &crate::command_system::SpecialPowerType,
        location: Vec3,
    ) -> CommandResult {
        self.execute_special_power(
            units,
            power_type,
            &crate::command_system::PowerTarget::Location(location),
        )
    }

    /// C++ AIGroup::groupDoSpecialPowerAtObject residual.
    pub(crate) fn execute_special_power_at_object(
        &mut self,
        units: &[ObjectId],
        power_type: &crate::command_system::SpecialPowerType,
        target: ObjectId,
    ) -> CommandResult {
        self.execute_special_power(
            units,
            power_type,
            &crate::command_system::PowerTarget::Object(target),
        )
    }

    // === Special Powers ===

    pub(super) fn execute_special_power(
        &mut self,
        units: &[ObjectId],
        power_type: &SpecialPowerType,
        target: &PowerTarget,
    ) -> CommandResult {
        // Basic validation: ensure object targets exist when required and power is ready.
        if let PowerTarget::Object(id) = target {
            if self.game_logic.host_object(*id).is_none() {
                return CommandResult::InvalidTarget;
            }
        }
        if let PowerTarget::Location(loc) = target {
            if !loc.x.is_finite() || !loc.y.is_finite() || !loc.z.is_finite() {
                return CommandResult::InvalidLocation;
            }
        }

        // Resolve impact position for residual superweapon path
        // (DaisyCutter/A10/Scud/PUC/NuclearMissile/AnthraxBomb/SpectreGunship/
        // CarpetBomb/ArtilleryBarrage/CruiseMissile).
        let target_position: Option<Vec3> = match target {
            PowerTarget::Location(loc) => Some(*loc),
            PowerTarget::Object(id) => self
                .game_logic
                .host_object(*id)
                .map(|obj| obj.get_position()),
            PowerTarget::None => {
                // C++ overridable destination residual wins when set on caster.
                let src = self.special_power_source_object(units, power_type);
                src.and_then(|id| {
                    self.game_logic
                        .host_object(id)
                        .and_then(|o| o.special_power_override_destination)
                })
                .or_else(|| {
                    units.iter().find_map(|id| {
                        self.game_logic
                            .host_object(*id)
                            .and_then(|o| o.special_power_override_destination)
                    })
                })
                .or_else(|| {
                    src.or_else(|| units.first().copied()).and_then(|id| {
                        self.game_logic
                            .host_object(id)
                            .map(|obj| obj.get_position())
                    })
                })
            }
        };

        debug!(
            "Executing special power {:?} with target {:?}",
            power_type, target
        );
        // C++ AIGroup::groupDoSpecialPower* uses getSpecialPowerSourceObject —
        // only the module owner fires, not every selected unit.
        let casters: Vec<ObjectId> =
            if let Some(src) = self.special_power_source_object(units, power_type) {
                vec![src]
            } else {
                // Fall back: any ready member (capture/skills on multi infantry).
                units.to_vec()
            };

        // Capture is a unit SpecialAbility with its own target legality.  It
        // must validate *before* spending charge; the former generic path
        // consumed the timer and only then discovered an immune/garrisoned
        // target.  `execute_capture_building_for_power` consumes only after a
        // legal source successfully accepts its walk-to order.
        let capture_power =
            crate::game_logic::CapturePowerKind::from_special_power_type(power_type);
        if capture_power != crate::game_logic::CapturePowerKind::None {
            let PowerTarget::Object(target_id) = target else {
                return CommandResult::InvalidTarget;
            };
            let any = casters.iter().copied().any(|unit_id| {
                matches!(
                    self.execute_capture_building_for_power(
                        &[unit_id],
                        *target_id,
                        Some(capture_power),
                    ),
                    CommandResult::Success
                )
            });
            return if any {
                CommandResult::Success
            } else {
                CommandResult::InvalidCommand
            };
        }

        // Hacker Disable Building is a paired, persistent unit
        // SpecialAbilityUpdate.  Its typed authority and charge timing are
        // intentionally outside the generic SpecialPower path below: C++
        // starts reload at preparation, not on this target click.
        if *power_type == SpecialPowerType::HackerDisableBuilding {
            let PowerTarget::Object(target_id) = target else {
                return CommandResult::InvalidTarget;
            };
            let any = casters.iter().copied().any(|unit_id| {
                matches!(
                    self.execute_hacker_disable_building(&[unit_id], *target_id),
                    CommandResult::Success
                )
            });
            return if any {
                CommandResult::Success
            } else {
                CommandResult::InvalidCommand
            };
        }

        // Microwave has distinct C++ module/weapon semantics.  It must never
        // borrow the Hacker Disable Building parser or persistent channel.
        // Until its own typed runtime exists, reject it without spending a
        // generic charge or mutating an unrelated target.
        if *power_type == SpecialPowerType::MicrowaveDisableBuilding {
            return CommandResult::InvalidCommand;
        }

        let mut any = false;
        for &unit_id in &casters {
            // SharedSyncedTimer residual: player-wide gate for superweapons.
            let ready = self
                .game_logic
                .is_special_power_ready_for(unit_id, power_type);
            if !ready {
                continue;
            }

            if !self
                .game_logic
                .consume_special_power_charge_for(unit_id, power_type)
            {
                continue;
            }
            // Wave 233: special-power AI state via GameLogic authority API.
            let _ = self
                .game_logic
                .unit_command_set_ai_state(unit_id, AIState::SpecialAbility);

            // Host residual: queue superweapon strike that will complete with
            // area damage (DaisyCutter / A10 / ScudStorm / ParticleCannon /
            // NuclearMissile + radiation residual / AnthraxBomb + toxin residual /
            // SpectreGunship + delayed orbit damage ticks residual /
            // CarpetBomb + delayed line multi-strike residual /
            // ArtilleryBarrage + delayed multi-shell scatter residual /
            // CruiseMissile + delayed loft MOAB area damage residual).
            // ClusterMines residual places a ring of land mines at target.
            // RadarScan residual temporarily reveals FOW at target (RadarVanPing).
            // SpySatellite residual temporarily reveals FOW at target (SpySatellitePing).
            // CiaIntelligence residual temporarily vision-spies all enemy units (SpyVision).
            // Paradrop residual queues America Airborne infantry drop at target.
            // Ambush residual queues GLA Rebel Ambush infantry spawn at target.
            // FireWall residual creates a line of fire damage zones toward target.
            // HelixNapalmBomb residual drops NapalmBomb blast + FirestormSmall at target.
            // EmpPulse residual disables vehicles/structures in radius (DISABLED_EMP).
            // Frenzy residual buffs ally attack damage in radius (FRENZY_ONE/TWO/THREE).
            // BattlePlan* residual selects USA Strategy Center army battle plan bonuses.
            // EmergencyRepair residual SingleBurst-heals ally vehicles in radius.
            // GpsScrambler residual grants STEALTHED to ally vehicles/infantry in radius.
            // LeafletDrop residual delays then disables enemy infantry/vehicles (DISABLED_EMP).
            // SneakAttack residual delays then spawns a GLA tunnel + shockwave damage.
            //
            // CIA Intelligence is no-target (SpyVision setUnitsVisionSpied residual).
            // Missile Defender laser guided needs an object target (lock secondary + attack).
            // Hero/unit disable & capture specials → existing walk-to residual command paths.
            if *power_type == SpecialPowerType::DisguiseAsVehiclePower {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !matches!(
                    self.execute_disguise_as_vehicle(&[unit_id], *tid),
                    CommandResult::Success
                ) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::BlackLotusDisableVehicle {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !matches!(
                    self.execute_disable_vehicle_hack(&[unit_id], *tid),
                    CommandResult::Success
                ) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::BlackLotusStealCash {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !matches!(
                    self.execute_steal_cash_hack(&[unit_id], *tid),
                    CommandResult::Success
                ) {
                    continue;
                }
            } else if matches!(
                *power_type,
                SpecialPowerType::DemoRebelTimedCharges
                    | SpecialPowerType::DemoKellTimedCharges
                    | SpecialPowerType::DemoKellStickyCharges
                    | SpecialPowerType::BattleBusDemoTrapRollout
                    | SpecialPowerType::BurtonTimedCharges
            ) {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !self.queue_special_timed_charge(unit_id, *tid, power_type) {
                    continue;
                }
            } else if matches!(
                *power_type,
                SpecialPowerType::DemoKellRemoteCharges | SpecialPowerType::BurtonRemoteCharges
            ) {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !self.queue_special_remote_charge(unit_id, *tid) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::TankHunterTnt {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                // Walk-to-target then plant timed charge residual (same as command path).
                if !self.queue_tank_hunter_tnt(unit_id, *tid) {
                    continue;
                }
            } else if *power_type == SpecialPowerType::MissileDefenderLaserGuided
                || *power_type == SpecialPowerType::LaserGuidedHowitzer
            {
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !self
                    .game_logic
                    .activate_missile_defender_laser_guided(unit_id, *tid)
                {
                    continue;
                }
            } else if *power_type == SpecialPowerType::CiaIntelligence
                || *power_type == SpecialPowerType::CommunicationsDownload
            {
                let team = self
                    .game_logic
                    .host_object(unit_id)
                    .map(|o| o.team)
                    .unwrap_or(crate::game_logic::Team::Neutral);
                if !self.game_logic.activate_cia_intelligence(
                    self.current_player_id,
                    team,
                    Some(unit_id),
                ) {
                    continue;
                }
            } else if let Some(pos) = target_position {
                if *power_type == SpecialPowerType::ClusterMines
                    || *power_type == SpecialPowerType::NukeDrop
                {
                    let team = self
                        .game_logic
                        .host_object(unit_id)
                        .map(|o| o.team)
                        .unwrap_or(crate::game_logic::Team::Neutral);
                    // C++ SUPERWEAPON_ClusterMines DeliverPayload residual
                    // (ChinaJetCargoPlane + bomb); mines place on bomb impact.
                    if self
                        .game_logic
                        .spawn_cluster_mines_flight(unit_id, pos)
                        .is_none()
                    {
                        // Fail-open residual: place mines immediately if flight spawn fails.
                        let placed = self
                            .game_logic
                            .place_cluster_mines(team, pos, Some(unit_id));
                        if placed.is_empty() {
                            continue;
                        }
                    }
                } else if *power_type == SpecialPowerType::RadarScan {
                    let team = self
                        .game_logic
                        .host_object(unit_id)
                        .map(|o| o.team)
                        .unwrap_or(crate::game_logic::Team::Neutral);
                    if !self.game_logic.activate_radar_scan(
                        self.current_player_id,
                        team,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::SpySatellite {
                    let team = self
                        .game_logic
                        .host_object(unit_id)
                        .map(|o| o.team)
                        .unwrap_or(crate::game_logic::Team::Neutral);
                    if !self.game_logic.activate_spy_satellite(
                        self.current_player_id,
                        team,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::SpyDrone {
                    let team = self
                        .game_logic
                        .host_object(unit_id)
                        .map(|o| o.team)
                        .unwrap_or(crate::game_logic::Team::Neutral);
                    if !self.game_logic.activate_spy_drone(
                        self.current_player_id,
                        team,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::EmpPulse {
                    if !self.game_logic.activate_emp_pulse(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::Frenzy
                    || *power_type == SpecialPowerType::EarlyFrenzy
                {
                    let level = {
                        use crate::game_logic::host_frenzy::highest_frenzy_level_from_sciences;
                        let sciences = self
                            .game_logic
                            .player_unlocked_sciences(self.current_player_id);
                        highest_frenzy_level_from_sciences(sciences.iter().map(|s| s.as_str()))
                    };
                    if !self.game_logic.activate_frenzy(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                        level,
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::BattlePlanBombardment
                    || *power_type == SpecialPowerType::BattlePlanHoldTheLine
                    || *power_type == SpecialPowerType::BattlePlanSearchAndDestroy
                {
                    // USA Strategy Center battle-plan residual (no location required).
                    // Fail-closed: not full pack/unpack animation / paralyze matrix.
                    use crate::game_logic::host_strategy_center::HostBattlePlan;
                    let plan = match power_type {
                        SpecialPowerType::BattlePlanHoldTheLine => HostBattlePlan::HoldTheLine,
                        SpecialPowerType::BattlePlanSearchAndDestroy => {
                            HostBattlePlan::SearchAndDestroy
                        }
                        _ => HostBattlePlan::Bombardment,
                    };
                    if !self.game_logic.activate_battle_plan(
                        self.current_player_id,
                        plan,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::EmergencyRepair
                    || *power_type == SpecialPowerType::EarlyEmergencyRepair
                {
                    let level = {
                        use crate::game_logic::host_emergency_repair::highest_emergency_repair_level_from_sciences;
                        let sciences = self
                            .game_logic
                            .player_unlocked_sciences(self.current_player_id);
                        highest_emergency_repair_level_from_sciences(
                            sciences.iter().map(|s| s.as_str()),
                        )
                    };
                    if !self.game_logic.activate_emergency_repair(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                        level,
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::GpsScrambler
                    || *power_type == SpecialPowerType::StealthGpsScrambler
                {
                    if !self.game_logic.activate_gps_scrambler(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::Paradrop
                    || *power_type == SpecialPowerType::InfantryParadrop
                    || *power_type == SpecialPowerType::TankParadrop
                {
                    if self
                        .game_logic
                        .queue_paradrop(power_type, unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::Ambush
                    || *power_type == SpecialPowerType::TerrorCell
                {
                    if self
                        .game_logic
                        .queue_ambush(power_type, unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::LeafletDrop
                    || *power_type == SpecialPowerType::EarlyLeafletDrop
                {
                    if self
                        .game_logic
                        .queue_leaflet_drop(power_type, unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::SneakAttack {
                    if self
                        .game_logic
                        .queue_sneak_attack(power_type, unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::FireWall {
                    if self.game_logic.activate_firewall(unit_id, pos).is_none() {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::HelixNapalmBomb
                    || *power_type == SpecialPowerType::HelixNukeBomb
                {
                    if self
                        .game_logic
                        .activate_helix_napalm_bomb(unit_id, pos)
                        .is_none()
                    {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::CrateDrop {
                    let _n = self.game_logic.activate_crate_drop(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    );
                } else if *power_type == SpecialPowerType::CashHack {
                    let _stolen = self
                        .game_logic
                        .activate_cash_hack(self.current_player_id, Some(unit_id));
                    // Always treat as success residual once activated (even 0 stolen).
                } else if *power_type == SpecialPowerType::Defector {
                    // C++ DefectorSpecialPower::doSpecialPowerAtObject residual.
                    let PowerTarget::Object(tid) = target else {
                        continue;
                    };
                    if !self.game_logic.activate_defector(unit_id, *tid) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::BaikonurRocket {
                    // C++ BaikonurLaunchPower: no-loc → door; location → door + detonation.
                    match target {
                        PowerTarget::Location(loc) => {
                            let _ = self.game_logic.activate_baikonur_launch_door(unit_id);
                            if !self.game_logic.activate_baikonur_detonation(unit_id, *loc) {
                                continue;
                            }
                        }
                        PowerTarget::None | PowerTarget::Object(_) => {
                            if !self.game_logic.activate_baikonur_launch_door(unit_id) {
                                continue;
                            }
                        }
                    }
                } else if *power_type == SpecialPowerType::CleanupArea {
                    if !self.game_logic.activate_cleanup_area(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else {
                    let _ = self
                        .game_logic
                        .queue_special_power_strike(power_type, unit_id, pos);
                }
            }
            any = true;
        }
        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    pub(super) fn execute_weapon(
        &mut self,
        units: &[ObjectId],
        weapon_slot: &WeaponSlot,
        max_shots_to_fire: i32,
        target: &WeaponTarget,
    ) -> CommandResult {
        let Some(slot) = host_weapon_slot_index(weapon_slot) else {
            warn!(
                "Rejecting unindexed weapon command {:?}: it has no host weapon-set slot",
                weapon_slot
            );
            return CommandResult::InvalidCommand;
        };

        // C++ GUI_COMMAND_FIRE_WEAPON locks the requested weapon slot temporarily
        // before it issues the attack. Do this through GameLogic so a requested
        // secondary/tertiary action cannot accidentally fire the unit's primary.
        let mut any = false;
        for &unit_id in units {
            if !self
                .game_logic
                .unit_command_select_weapon_slot(unit_id, slot)
            {
                warn!(
                    "Unit {} cannot use requested weapon slot {:?}",
                    unit_id.0, weapon_slot
                );
                continue;
            }

            let fired = match target {
                WeaponTarget::Object(target_id) => self.game_logic.unit_command_fire_weapon(
                    unit_id,
                    Some(*target_id),
                    None,
                    max_shots_to_fire,
                ),
                WeaponTarget::Location(pos) => self.game_logic.unit_command_fire_weapon(
                    unit_id,
                    None,
                    Some(*pos),
                    max_shots_to_fire,
                ),
            };

            if fired {
                any = true;
                debug!(
                    "Unit {} firing weapon {:?} at {:?}",
                    unit_id.0, weapon_slot, target
                );
            }
        }

        if any {
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// GLA Bomb Truck residual: SpecialAbilityDisguiseAsVehicle.
    ///
    /// C++ residual: any ground vehicle target (ally/enemy/neutral) except
    /// bomb trucks / trains / aircraft. Completes without approach walk
    /// (StartAbilityRange = 1e6). Fail-closed: not full drawable model swap.
    /// Timed-charge special residual (Burton/Demo/BattleBus): walk + plant timed charge.
    pub(super) fn queue_special_timed_charge(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
        power_type: &SpecialPowerType,
    ) -> bool {
        use crate::game_logic::{AIState, PendingSpecialAbility};

        let Some(unit) = self.game_logic.host_object(unit_id) else {
            return false;
        };
        if !unit.is_alive() || !unit.can_move() {
            return false;
        }
        let Some(target) = self.game_logic.host_object(target_id) else {
            return false;
        };
        if !target.is_alive() {
            return false;
        }
        let target_team = target.team;
        if target_team == unit.team || target_team == crate::game_logic::Team::Neutral {
            // BattleBus trap rollout may target ground near self; allow structure/vehicle enemies only residual.
            if !matches!(*power_type, SpecialPowerType::BattleBusDemoTrapRollout) {
                return false;
            }
        }
        let target_pos = target.get_position();
        // Wave 233: stop-moving + order-target via GameLogic authority API.
        let _ = self
            .game_logic
            .unit_command_stop_moving_order_target(unit_id, Some(target_id));
        let _ = self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility);
        self.game_logic.queue_pending_special_ability(
            unit_id,
            PendingSpecialAbility::PlantTimedDemoCharge { target_id },
        );
        true
    }

    /// Remote-charge special residual (Burton/Demo Kell): walk + plant remote charge.
    pub(super) fn queue_special_remote_charge(
        &mut self,
        unit_id: ObjectId,
        target_id: ObjectId,
    ) -> bool {
        use crate::game_logic::{AIState, PendingSpecialAbility};

        let Some(unit) = self.game_logic.host_object(unit_id) else {
            return false;
        };
        if !unit.is_alive() || !unit.can_move() {
            return false;
        }
        let Some(target) = self.game_logic.host_object(target_id) else {
            return false;
        };
        if !target.is_alive() {
            return false;
        }
        let target_pos = target.get_position();
        // Wave 233: stop-moving + order-target via GameLogic authority API.
        let _ = self
            .game_logic
            .unit_command_stop_moving_order_target(unit_id, Some(target_id));
        let _ = self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility);
        self.game_logic.queue_pending_special_ability(
            unit_id,
            PendingSpecialAbility::PlantRemoteDemoCharge { target_id },
        );
        true
    }

    /// Tank Hunter TNT special residual: path to target and plant timed sticky charge.
    pub(super) fn queue_tank_hunter_tnt(&mut self, unit_id: ObjectId, target_id: ObjectId) -> bool {
        use crate::game_logic::host_tank_hunter::{
            is_tank_hunter_template, tnt_in_start_range, tnt_ready, TNT_START_ABILITY_RANGE,
        };
        use crate::game_logic::{AIState, PendingSpecialAbility};

        let Some(unit) = self.game_logic.host_object(unit_id) else {
            return false;
        };
        if !unit.is_alive() || !is_tank_hunter_template(&unit.template_name) {
            return false;
        }
        if !tnt_ready(
            self.game_logic.get_frame(),
            self.game_logic.tank_hunter_tnt_last_plant_frame(unit_id),
        ) {
            return false;
        }
        let Some(target) = self.game_logic.host_object(target_id) else {
            return false;
        };
        if !target.is_alive() {
            return false;
        }
        let target_pos = target.get_position();
        // Always queue walk-to; plant resolves on reach (StartAbilityRange 5 residual).
        // Wave 233: stop-moving + order-target via GameLogic authority API.
        let _ = self
            .game_logic
            .unit_command_stop_moving_order_target(unit_id, Some(target_id));
        if !self.path_to_goal_with_state(unit_id, target_pos, AIState::SpecialAbility) {
            // If already in range, still queue plant.
            let unit_pos = self
                .game_logic
                .host_object(unit_id)
                .map(|o| o.get_position())
                .unwrap_or(target_pos);
            let dx = unit_pos.x - target_pos.x;
            let dz = unit_pos.z - target_pos.z;
            let dist = (dx * dx + dz * dz).sqrt();
            if !tnt_in_start_range(dist) && dist > TNT_START_ABILITY_RANGE * 2.0 {
                return false;
            }
        }
        self.game_logic.queue_pending_special_ability(
            unit_id,
            PendingSpecialAbility::PlantTimedDemoCharge { target_id },
        );
        let _ = TNT_START_ABILITY_RANGE;
        true
    }
}
