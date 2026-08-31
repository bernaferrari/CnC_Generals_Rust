//! Special powers, weapons, and charge-queue helpers.
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
fn special_power_has_overridable_destination(
    power_type: &crate::command_system::SpecialPowerType,
) -> bool {
    matches!(
        power_type,
        crate::command_system::SpecialPowerType::ParticleCannon
            | crate::command_system::SpecialPowerType::SuperweaponParticleCannon
            | crate::command_system::SpecialPowerType::LaserCannon
            | crate::command_system::SpecialPowerType::SpectreGunship
            | crate::command_system::SpecialPowerType::AirForceSpectreGunship
    )
}

fn leftover_special_power_type(
    power_type: &SpecialPowerType,
) -> Option<gamelogic::object::special_power_types::SpecialPowerType> {
    let name =
        crate::game_logic::host_special_power_enum_residual::host_command_power_cpp_enum_name(
            power_type,
        )?;
    gamelogic::object::special_power_types::SpecialPowerType::from_str(name)
}

/// C++ ActionManager::canDoSpecialPowerAtLocation leftover gates.
/// Underwater (paradrop/crate-drop) + fully-shrouded damaging powers.
pub(super) fn leftover_can_do_special_power_at_location(
    power_type: &SpecialPowerType,
    loc: Vec3,
    player_index: i32,
) -> bool {
    let Some(leftover_type) = leftover_special_power_type(power_type) else {
        return true;
    };
    // Leftover/C++ Coord3D is X/Y map plane; live host is Y-up (X/Z).
    let leftover_loc = LogicCoord3D::new(loc.x, loc.z, loc.y);
    gamelogic::action_manager::TheActionManager::can_do_special_power_at_location_for_player(
        leftover_type,
        &leftover_loc,
        player_index,
        true,
    )
}

/// C++ ActionManager::canDoSpecialPower leftover type switch (no-option fire).
pub(super) fn leftover_can_do_special_power(power_type: &SpecialPowerType) -> bool {
    crate::command_system::leftover_special_power_is_no_target(power_type)
}

/// C++ ActionManager.cpp:1558-1626 `canDoSpecialPowerAtObject` leftover replay.
/// Dual-world leftover Objects are empty on the live player path, so this
/// replays the dead / FOGGED preamble then the type match using host
/// relationship + KindOf.
pub(super) fn leftover_can_do_special_power_at_object(
    power_type: &SpecialPowerType,
    relationship: gamelogic::common::Relationship,
    target_is_vehicle: bool,
    target_dead: bool,
    target_shrouded_for_action: bool,
) -> bool {
    use gamelogic::common::Relationship;
    // C++ ActionManager.cpp:1569-1590 — after module/ready, before the type switch.
    if target_dead || target_shrouded_for_action {
        return false;
    }
    match power_type {
        // C++ SPECIAL_BATTLESHIP_BOMBARDMENT: relationship != ALLIES.
        SpecialPowerType::BattleshipBombardment => relationship != Relationship::Allies,
        // C++ SPECIAL_MISSILE_DEFENDER_LASER_GUIDED_MISSILES: VEHICLE && ENEMIES.
        SpecialPowerType::MissileDefenderLaserGuided | SpecialPowerType::LaserGuidedHowitzer => {
            target_is_vehicle && relationship == Relationship::Enemies
        }
        // Leftover can_do_special_power_at_object plant path (RemoteCharges).
        SpecialPowerType::DemoKellRemoteCharges | SpecialPowerType::BurtonRemoteCharges => true,
        // Leftover can_do_special_power no-target types: at_object is FALSE.
        _ if leftover_can_do_special_power(power_type) => false,
        _ => !crate::command_system::leftover_special_power_is_location_target_only(power_type),
    }
}

/// C++ `Object::getRelationship` for the leftover object-click gate.
/// Owner map / alliance_team wins when both sides have owners; otherwise
/// leftover team fallback (same team ALLIES, Neutral team NEUTRAL, else ENEMIES).
fn leftover_object_click_relationship(
    logic: &GameLogic,
    source: &crate::game_logic::Object,
    target: &crate::game_logic::Object,
) -> gamelogic::common::Relationship {
    use gamelogic::common::Relationship;
    if source.owner_player_id.is_some() && target.owner_player_id.is_some() {
        return logic.object_relationship(source, target);
    }
    if source.team == target.team {
        Relationship::Allies
    } else if source.team == Team::Neutral || target.team == Team::Neutral {
        Relationship::Neutral
    } else {
        Relationship::Enemies
    }
}

/// Live replay of leftover `can_do_special_power_at_object` using host
/// relationship, KindOf, dead, and `is_object_shrouded_for_action`.
fn leftover_object_click_allowed(
    logic: &GameLogic,
    unit: &crate::game_logic::Object,
    target: &crate::game_logic::Object,
    power_type: &SpecialPowerType,
) -> bool {
    leftover_can_do_special_power_at_object(
        power_type,
        leftover_object_click_relationship(logic, unit, target),
        target.is_kind_of(KindOf::Vehicle),
        !target.is_alive(),
        logic.is_object_shrouded_for_action(unit, target),
    )
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
        let mut voiced: Vec<ObjectId> = Vec::new();
        for &unit_id in units {
            if self
                .game_logic
                .unit_command_set_special_power_overridable_destination(unit_id, location)
            {
                any = true;
                voiced.push(unit_id);
            }
        }
        if any {
            // C++ SpectreGunshipUpdate::setSpecialPowerOverridableDestination
            // plays VoiceAttack for the local controlling player.
            let local = self
                .game_logic
                .get_player(self.current_player_id)
                .map(|p| p.is_local)
                .unwrap_or(false);
            if local {
                let spectre: Vec<ObjectId> = voiced
                    .into_iter()
                    .filter(|&id| {
                        self.game_logic.host_object(id).is_some_and(|o| {
                            o.spectre_gunship_update
                                .as_ref()
                                .is_some_and(|d| d.status.overridable_destination_active())
                        })
                    })
                    .collect();
                if !spectre.is_empty() {
                    self.game_logic.queue_picked_unit_voice(
                        &spectre,
                        crate::game_logic::audio_dispatch_impl::UnitVoiceSlot::Attack,
                    );
                }
            }
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ AIGroup::getSpecialPowerSourceObject residual —
    /// first living member that carries the matching SpecialPowerModule.
    pub(crate) fn special_power_source_object(
        &self,
        units: &[ObjectId],
        power_type: &crate::command_system::SpecialPowerType,
    ) -> Option<ObjectId> {
        // C++ walks members for SpecialPowerModule matching template
        // (`AIGroup.cpp` / `Object::getSpecialPowerModule`).
        for &id in units {
            let Some(o) = self.game_logic.host_object(id) else {
                continue;
            };
            if !o.is_alive() {
                continue;
            }
            if o.thing
                .template
                .special_power_module_for_command(power_type)
                .is_some()
                || o.special_power_cooldowns.contains_key(power_type)
            {
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
        // C++ CommandXlat.cpp:1055-1078 NEED_TARGET_POS: unit under the cursor
        // is only object-in-way. Leftover canDoSpecialPowerAtObject is FALSE;
        // leftover canDoSpecialPowerAtLocation is the gate.
        let location_from_unit = match target {
            PowerTarget::Object(id)
                if crate::command_system::leftover_special_power_is_location_target_only(
                    power_type,
                ) =>
            {
                match self.game_logic.host_object(*id).map(|o| o.get_position()) {
                    Some(pos) => Some(pos),
                    None => return CommandResult::InvalidTarget,
                }
            }
            _ => None,
        };
        let retargeted;
        let target = if let Some(pos) = location_from_unit {
            retargeted = PowerTarget::Location(pos);
            &retargeted
        } else {
            target
        };

        // Basic validation: ensure object targets exist when required and power is ready.
        if let PowerTarget::Object(id) = target {
            if self.game_logic.host_object(*id).is_none() {
                return CommandResult::InvalidTarget;
            }
        }
        if let Some(loc) = target.location_pos() {
            if !loc.x.is_finite() || !loc.y.is_finite() || !loc.z.is_finite() {
                return CommandResult::InvalidLocation;
            }
            // C++ ActionManager.cpp:1439-1523 leftover can_do_special_power_at_location
            // participates only in the leftover NEED_TARGET_POS path (unit-under-cursor
            // clicks are AT_LOCATION, CommandXlat.cpp:1055-1078). Object-only /
            // no-target powers (CashHack, Defector, Baikonur, ...) never reach that
            // gate: their ground clicks fall through to the per-caster loop below,
            // where each power's doSpecialPowerAtLocation residual is a no-op
            // (CashHackSpecialPower.cpp:76-82) and nothing is issued or consumed.
            if crate::command_system::leftover_special_power_is_location_target_only(power_type)
            {
                let player_index = {
                    let src = self.special_power_source_object(units, power_type);
                    src.and_then(|id| {
                        let (owner, team) = self
                            .game_logic
                            .host_object(id)
                            .map(|o| (o.owner_player_id, o.team))?;
                        owner.or_else(|| self.game_logic.player_id_for_team(team))
                    })
                    .map(|id| id as i32)
                    .unwrap_or(-1)
                };
                if !leftover_can_do_special_power_at_location(power_type, loc, player_index) {
                    return CommandResult::InvalidLocation;
                }
            }
        }


        // Resolve impact position for residual superweapon path
        // (DaisyCutter/A10/Scud/PUC/NuclearMissile/AnthraxBomb/SpectreGunship/
        // CarpetBomb/ArtilleryBarrage/CruiseMissile).
        let target_position: Option<Vec3> = match target {
            PowerTarget::Location(loc) => Some(*loc),
            PowerTarget::LocationFacing { pos, .. } => Some(*pos),
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
        // C++ AIGroup::groupDoSpecialPower* (AIGroup.cpp:2614-2735) loops every
        // member. Each Object::doSpecialPower requires canUseSpecialPower →
        // SpecialPowerModule (SpecialPower.cpp:308). Do not fall back to
        // every selected unit when nobody carries the module — that let any
        // tank fire Frenzy/CashHack.
        let casters: Vec<ObjectId> = units
            .iter()
            .copied()
            .filter(|&id| {
                self.game_logic.host_object(id).is_some_and(|o| {
                    o.is_alive()
                        && (o
                            .thing
                            .template
                            .special_power_module_for_command(power_type)
                            .is_some()
                            || o.special_power_cooldowns.contains_key(power_type))
                })
            })
            .collect();

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
                let allowed = match (
                    self.game_logic.host_object(unit_id),
                    self.game_logic.host_object(*target_id),
                ) {
                    (Some(unit), Some(tgt)) => {
                        leftover_object_click_allowed(self.game_logic, unit, tgt, power_type)
                    }
                    _ => false,
                };
                allowed
                    && matches!(
                        self.execute_capture_building_for_power(
                            &[unit_id],
                            *target_id,
                            Some(capture_power),
                        ),
                        CommandResult::Success
                    )
            });
            return if any {
                // C++ CommandXlat.cpp:637-651 MSG_DO_SPECIAL_POWER* InitiateSound.
                self.play_special_power_initiate_sound(&casters, power_type);
                CommandResult::Success
            } else {
                CommandResult::InvalidCommand
            };
        }

        // Hacker / Microwave Disable Building share C++ SPECIAL_HACKER_DISABLE_BUILDING.
        // Both are paired SpecialAbilityUpdate channels (walk/unpack/DISABLED_HACKED).
        // Charge starts at preparation, not on this target click.
        if matches!(
            *power_type,
            SpecialPowerType::HackerDisableBuilding | SpecialPowerType::MicrowaveDisableBuilding
        ) {
            let PowerTarget::Object(target_id) = target else {
                return CommandResult::InvalidTarget;
            };
            let any = casters.iter().copied().any(|unit_id| {
                let allowed = match (
                    self.game_logic.host_object(unit_id),
                    self.game_logic.host_object(*target_id),
                ) {
                    (Some(unit), Some(tgt)) => {
                        leftover_object_click_allowed(self.game_logic, unit, tgt, power_type)
                    }
                    _ => false,
                };
                allowed
                    && matches!(
                        self.execute_hacker_disable_building(&[unit_id], *target_id),
                        CommandResult::Success
                    )
            });
            return if any {
                self.play_special_power_initiate_sound(&casters, power_type);
                CommandResult::Success
            } else {
                CommandResult::InvalidCommand
            };
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
            // C++ ActionManager::canDoSpecialPowerAtObject leftover click gates.
            // Battleship is not consume_at_prep — refuse before spending charge.
            if let PowerTarget::Object(tid) = target {
                let allowed = match (
                    self.game_logic.host_object(unit_id),
                    self.game_logic.host_object(*tid),
                ) {
                    (Some(unit), Some(tgt)) => {
                        leftover_object_click_allowed(self.game_logic, unit, tgt, power_type)
                    }
                    _ => false,
                };
                if !allowed {
                    continue;
                }
            }

            // C++ markSpecialPowerTriggered is startPreparation, after
            // unpack/face/range. Steal/disable leftover must not consume on click.
            // C++ CashHackSpecialPower.cpp:76-82 / DefectorSpecialPower.cpp:69-76
            // doSpecialPowerAtLocation returns after the disabled check and never
            // calls triggerSpecialPower / startPowerRecharge. Consume only after
            // a valid object-target activation.
            let consume_after_valid_object = matches!(
                *power_type,
                SpecialPowerType::CashHack | SpecialPowerType::Defector
            );
            let never_consume = matches!(*power_type, SpecialPowerType::CleanupArea);
            let consume_at_prep = matches!(
                *power_type,
                SpecialPowerType::BlackLotusStealCash
                    | SpecialPowerType::BlackLotusDisableVehicle
                    | SpecialPowerType::TankHunterTnt
                    | SpecialPowerType::DemoRebelTimedCharges
                    | SpecialPowerType::DemoKellTimedCharges
                    | SpecialPowerType::DemoKellStickyCharges
                    | SpecialPowerType::DemoKellRemoteCharges
                    | SpecialPowerType::BattleBusDemoTrapRollout
                    | SpecialPowerType::BurtonTimedCharges
                    | SpecialPowerType::BurtonRemoteCharges
                    | SpecialPowerType::HelixNapalmBomb
                    | SpecialPowerType::HelixNukeBomb
                    | SpecialPowerType::MissileDefenderLaserGuided
                    | SpecialPowerType::LaserGuidedHowitzer
            );
            // C++ ActionManager::canDoSpecialPowerAtObject returns false for
            // SPECIAL_LAUNCH_BAIKONUR_ROCKET — never spend the charge on an object click.
            if *power_type == SpecialPowerType::BaikonurRocket
                && matches!(target, PowerTarget::Object(_))
            {
                continue;
            }
            if !consume_at_prep
                && !consume_after_valid_object
                && !never_consume
                && !self
                    .game_logic
                    .consume_special_power_charge_for(unit_id, power_type)
            {
                continue;
            }
            if !never_consume {
                let _ = self
                    .game_logic
                    .unit_command_set_ai_state(unit_id, AIState::SpecialAbility);
            }

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
            } else if *power_type == SpecialPowerType::CashHack {
                // C++ CashHackSpecialPower::doSpecialPowerAtLocation is a no-op.
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                let Some(_stolen) = self.game_logic.activate_cash_hack(
                    self.current_player_id,
                    Some(unit_id),
                    Some(*tid),
                ) else {
                    continue;
                };
                if !self
                    .game_logic
                    .consume_special_power_charge_for(unit_id, power_type)
                {
                    continue;
                }
            } else if *power_type == SpecialPowerType::Defector {
                // C++ DefectorSpecialPower::doSpecialPowerAtLocation is a no-op.
                let PowerTarget::Object(tid) = target else {
                    continue;
                };
                if !self.game_logic.activate_defector(unit_id, *tid) {
                    continue;
                }
                if !self
                    .game_logic
                    .consume_special_power_charge_for(unit_id, power_type)
                {
                    continue;
                }
            } else if *power_type == SpecialPowerType::BaikonurRocket {
                // C++ BaikonurLaunchPower: no-target/script opens DOOR_1_OPENING;
                // location only ThingFactory-creates DetonationObject.
                match target {
                    PowerTarget::Location(loc) => {
                        if !self.game_logic.activate_baikonur_detonation(unit_id, *loc) {
                            continue;
                        }
                    }
                    PowerTarget::LocationFacing { pos, .. } => {
                        if !self.game_logic.activate_baikonur_detonation(unit_id, *pos) {
                            continue;
                        }
                    }
                    PowerTarget::None => {
                        if !self.game_logic.activate_baikonur_launch_door(unit_id) {
                            continue;
                        }
                    }
                    PowerTarget::Object(_) => continue,
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
                        // C++ createViewObject still fires when OCL create fails.
                        let _ = self.game_logic.create_special_power_view_object_at(
                            unit_id,
                            pos,
                            crate::game_logic::host_mines::CLUSTER_MINES_VIEW_OBJECT_RANGE,
                            crate::game_logic::host_mines::CLUSTER_MINES_VIEW_OBJECT_DURATION_FRAMES,
                        );
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
                        .queue_sneak_attack_facing(
                            power_type,
                            unit_id,
                            pos,
                            target.location_angle(),
                        )
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
                    if !self.queue_helix_napalm_bomb(unit_id, pos) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::CrateDrop {
                    let _n = self.game_logic.activate_crate_drop(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    );
                } else if *power_type == SpecialPowerType::CleanupArea {
                    if !self.game_logic.activate_cleanup_area(
                        self.current_player_id,
                        pos,
                        Some(unit_id),
                    ) {
                        continue;
                    }
                } else if *power_type == SpecialPowerType::BattleshipBombardment {
                    // C++ FireWeaponPower: object click stays aiAttackObject;
                    // location click is aiAttackPosition. Not Artillery Barrage.
                    match target {
                        PowerTarget::Object(tid) => {
                            if !self
                                .game_logic
                                .activate_fire_weapon_power_at_object(unit_id, *tid)
                            {
                                continue;
                            }
                        }
                        _ => {
                            if !self.game_logic.activate_fire_weapon_power(unit_id, pos) {
                                continue;
                            }
                        }
                    }
                } else {
                    let _ = self
                        .game_logic
                        .queue_special_power_strike(power_type, unit_id, pos);
                }
            }
            if *power_type != SpecialPowerType::CleanupArea
                && crate::game_logic::special_power_strikes::HostSuperweaponKind::from_command_power(
                    power_type,
                )
                .is_none()
                && !consume_at_prep
            {
                // C++ aboutToDoSpecialPower + CompletionDie analog for instant powers.
                // UpdateModuleStartsAttack prep powers notify at startPreparation.
                self.game_logic.notify_script_engine_special_power_event(
                    unit_id,
                    power_type,
                    true,
                    true,
                );
            }
            any = true;
            if special_power_has_overridable_destination(power_type) {
                if let Some(pos) = target_position {
                    let _ = self
                        .game_logic
                        .unit_command_set_special_power_overridable_destination(unit_id, pos);
                }
            }
        }
        if any {
            // C++ CommandXlat.cpp:637-651 spmInterface->getInitiateSound().
            // CleanupAreaPower does not call the SpecialPowerModule base.
            if *power_type != SpecialPowerType::CleanupArea {
                self.play_special_power_initiate_sound(&casters, power_type);
            }
            CommandResult::Success
        } else {
            CommandResult::InvalidCommand
        }
    }

    /// C++ `pickAndPlayUnitVoiceResponse` MSG_DO_SPECIAL_POWER* (skip=TRUE).
    fn play_special_power_initiate_sound(
        &mut self,
        casters: &[ObjectId],
        power_type: &SpecialPowerType,
    ) {
        let power_key = format!("{power_type:?}");
        let retail =
            crate::game_logic::special_power_strikes::HostSuperweaponKind::from_command_power(
                power_type,
            )
            .map(|kind| kind.retail_initiate_sound());
        for &id in casters {
            let Some(obj) = self.game_logic.host_object(id) else {
                continue;
            };
            let module_name = obj
                .thing
                .template
                .special_power_module_for_command(power_type)
                .map(|m| m.special_power_template.clone());
            let pos = obj.get_position();
            let Some(event) =
                crate::game_logic::audio_dispatch_impl::resolve_special_power_initiate_sound(
                    &power_key,
                    module_name.as_deref(),
                    retail,
                )
            else {
                continue;
            };
            self.game_logic.queue_audio_event(
                AudioEventRequest::new(&event)
                    .with_object(id)
                    .with_position(pos)
                    .with_priority(100),
            );
            return;
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
            // C++ MSG_DO_WEAPON_AT_OBJECT / MSG_DO_WEAPON_AT_LOCATION
            // (`CommandXlat.cpp:511-627`) — VoiceAttack then specialty upgrade.
            let (target_id, at_location) = match target {
                WeaponTarget::Object(id) => (Some(*id), false),
                WeaponTarget::Location(_) => (None, true),
            };
            self.game_logic
                .queue_attack_voice(units, target_id, true, at_location, Some(slot));
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
        if !crate::game_logic::host_hero_abilities::leftover_charge_plant_target_ok(
            target.is_alive(),
            target.is_kind_of(KindOf::Bridge),
            target.is_kind_of(KindOf::BridgeTower),
            target.is_kind_of(KindOf::Structure),
            target.is_kind_of(KindOf::Vehicle),
        ) {
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
        if !crate::game_logic::host_hero_abilities::leftover_charge_plant_target_ok(
            target.is_alive(),
            target.is_kind_of(KindOf::Bridge),
            target.is_kind_of(KindOf::BridgeTower),
            target.is_kind_of(KindOf::Structure),
            target.is_kind_of(KindOf::Vehicle),
        ) {
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
            TNT_START_ABILITY_RANGE, is_tank_hunter_template, tnt_in_start_range, tnt_ready,
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
        if !crate::game_logic::host_hero_abilities::leftover_tank_hunter_tnt_target_ok(
            target.is_alive(),
            target.is_kind_of(KindOf::Structure),
            target.is_kind_of(KindOf::Vehicle),
            target.is_kind_of(KindOf::Aircraft) || target.status.airborne_target,
        ) {
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

    /// C++ SpecialAbilityUpdate: initiateIntent then approach until
    /// StartAbilityRange 3; markSpecialPowerTriggered at startPreparation.
    pub(super) fn queue_helix_napalm_bomb(&mut self, unit_id: ObjectId, pos: Vec3) -> bool {
        use crate::game_logic::host_helix_napalm::{
            UPGRADE_HELIX_NAPALM_BOMB, UPGRADE_HELIX_NUKE_BOMB, helix_napalm_unlocked,
            is_helix_napalm_caster,
        };
        use crate::game_logic::{AIState, PendingSpecialAbility};

        let Some(unit) = self.game_logic.host_object(unit_id) else {
            return false;
        };
        if !unit.is_alive() || !is_helix_napalm_caster(&unit.template_name) {
            return false;
        }
        let has_upgrade = unit.has_upgrade_tag(UPGRADE_HELIX_NAPALM_BOMB)
            || unit.has_upgrade_tag("Upgrade_HelixNapalmBomb")
            || unit.has_upgrade_tag(UPGRADE_HELIX_NUKE_BOMB)
            || unit.has_upgrade_tag("Nuke_Upgrade_HelixNukeBomb")
            || unit.has_upgrade_tag("Upgrade_HelixNukeBomb");
        if !helix_napalm_unlocked(&unit.template_name, has_upgrade) {
            return false;
        }
        let _ = self
            .game_logic
            .unit_command_stop_moving_order_target(unit_id, None);
        let _ = self.path_to_goal_with_state(unit_id, pos, AIState::SpecialAbility);
        self.game_logic
            .queue_pending_special_ability(unit_id, PendingSpecialAbility::helix_napalm_at(pos));
        true
    }
}
