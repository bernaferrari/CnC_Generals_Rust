//! Nuclear Missile Special Power
//!
//! China superweapon that launches a nuclear missile causing massive damage
//! and radiation fallout.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::helpers::{TheAudio, TheObjectCreationListStore, ThePartitionManager, TheTerrainLogic};
use crate::object::registry::OBJECT_REGISTRY;
use super::types::OclCreateLocType;
use crate::object_creation_list::live_creation_context;
use crate::player::player_list;
use std::sync::{Arc, RwLock};

const CREATE_ABOVE_LOCATION_HEIGHT: Real = 300.0;
const MAX_ADJUST_RADIUS: Real = 500.0;
const NUKE_DAMAGE_RADIUS: Real = 210.0;

#[derive(Debug, Clone)]
pub struct NuclearMissilePowerData {
    pub base: SpecialPowerModuleData,
    pub ocl_name: AsciiString,
    pub create_loc: OclCreateLocType,
    pub adjust_position_to_passable: Bool,
}

impl NuclearMissilePowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::OCL);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::AFFECTS_BUILDINGS
            | SpecialPowerFlags::AFFECTS_TERRAIN
            | SpecialPowerFlags::SUPERWEAPON
            | SpecialPowerFlags::RADAR_EFFECT;
        base.recharge_time = 360.0; // 6 minutes
        base.cost = 0;
        base.range = 0.0;
        base.radius = NUKE_DAMAGE_RADIUS;
        let name_str = base.name.as_str();
        let mut ocl_name: AsciiString = "SUPERWEAPON_NeutronMissile".into();
        if name_str.eq_ignore_ascii_case("Nuke_SuperweaponNeutronMissile") {
            base.recharge_time = 300.0; // 300000 ms
        } else if name_str.eq_ignore_ascii_case("SupW_SuperweaponNeutronMissile") {
            base.recharge_time = 240.0; // 240000 ms
            ocl_name = "SupW_SUPERWEAPON_NeutronMissile".into();
        }

        Self {
            base,
            ocl_name,
            create_loc: OclCreateLocType::CreateAtEdgeNearSource,
            adjust_position_to_passable: false,
        }
    }
}

pub struct NuclearMissilePower {
    data: NuclearMissilePowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    owner_player_id: Option<ObjectID>,
    owner_object_id: ObjectID,
}

impl NuclearMissilePower {
    pub fn new(data: NuclearMissilePowerData, owner_object_id: ObjectID) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            owner_player_id: None,
            owner_object_id,
        }
    }

    pub fn set_owner_object_id(&mut self, owner_id: ObjectID) {
        self.owner_object_id = owner_id;
    }

    fn can_afford(&self, player_id: ObjectID) -> Bool {
        if self.data.base.cost <= 0 {
            return true;
        }

        self.get_player_money(player_id)
            .map(|money| money >= self.data.base.cost)
            .unwrap_or(false)
    }

    fn deduct_cost(&mut self, player_id: ObjectID) -> Bool {
        if self.data.base.cost <= 0 {
            return true;
        }

        let player_list = crate::player::player_list();
        let Ok(list_guard) = player_list.read() else {
            return false;
        };
        let Some(player_arc) = list_guard.get_player(player_id as PlayerIndex) else {
            return false;
        };
        let Ok(mut player_guard) = player_arc.write() else {
            return false;
        };

        if !player_guard
            .get_money_mut()
            .subtract_money(self.data.base.cost)
        {
            return false;
        }

        if self.data.base.cost > 0 {
            player_guard
                .get_score_keeper_mut()
                .add_money_spent(self.data.base.cost as u32);
        }

        true
    }

    fn get_player_money(&self, player_id: ObjectID) -> Option<Int> {
        let player_list = crate::player::player_list();
        let list_guard = player_list.read().ok()?;
        let player_arc = list_guard.get_player(player_id as PlayerIndex)?;
        let player_guard = player_arc.read().ok()?;
        Some(player_guard.get_money().get_money())
    }

    fn check_prerequisites(&self, player_id: ObjectID) -> Bool {
        self.data.base.check_prerequisites(player_id)
    }

    fn validate_targeting(&self, targeting: Option<&TargetingInfo>) -> Result<(), String> {
        if self.data.base.requires_targeting() && targeting.is_none() {
            return Err("Nuclear Missile requires targeting".to_string());
        }
        if self.data.base.is_instant() && targeting.is_some() {
            return Err("Instant power does not accept targeting".to_string());
        }
        Ok(())
    }

    fn play_sound(&self) {
        if !self.data.base.sound_effect.is_empty() {
            if let Some(audio) = TheAudio::get() {
                let event =
                    crate::common::audio::AudioEventRts::new(self.data.base.sound_effect.as_str());
                audio.add_audio_event(&event);
            }
        }
    }

    fn execute_strike(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        log::info!(
            "Nuclear Missile launched at position {:?}",
            targeting.position
        );

        let ocl =
            TheObjectCreationListStore::find_object_creation_list(self.data.ocl_name.as_str())
                .ok_or_else(|| {
                    format!("OCL '{}' not found for nuclear missile", self.data.ocl_name)
                })?;
        let owner = self
            .resolve_owner_object()
            .ok_or_else(|| "Nuclear Missile requires an owning object".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "Nuclear missile owner lock poisoned".to_string())?;
        if owner_guard.is_disabled() {
            return Ok(());
        }

        let mut target_coord = targeting.position;
        if let Some(target_id) = targeting.target_object {
            if let Some(target) = OBJECT_REGISTRY.get_object(target_id) {
                if let Ok(target_guard) = target.read() {
                    target_coord = *target_guard.get_position();
                }
            }
        }
        if self.data.adjust_position_to_passable {
            if let Some(partition) = ThePartitionManager::get() {
                let center = target_coord;
                let mut options = crate::helpers::FindPositionOptions::default();
                options.min_radius = 0.0;
                options.max_radius = MAX_ADJUST_RADIUS;
                options.flags = crate::helpers::FPF_CLEAR_CELLS_ONLY;
                if !partition.find_position_around_with_options(
                    &center,
                    &options,
                    &mut target_coord,
                ) {
                    target_coord = targeting.position;
                }
            }
        }

        let creation_coord = match self.data.create_loc {
            OclCreateLocType::CreateAtEdgeNearSource => TheTerrainLogic::get()
                .map(|terrain| terrain.find_closest_edge_point(owner_guard.get_position()))
                .unwrap_or(*owner_guard.get_position()),
            OclCreateLocType::CreateAtEdgeNearTarget => TheTerrainLogic::get()
                .map(|terrain| terrain.find_closest_edge_point(&target_coord))
                .unwrap_or(target_coord),
            OclCreateLocType::CreateAtEdgeFarthestFromTarget => {
                let mut coord = TheTerrainLogic::get()
                    .map(|terrain| terrain.find_farthest_edge_point(&target_coord))
                    .unwrap_or(target_coord);
                coord.z += CREATE_ABOVE_LOCATION_HEIGHT;
                coord
            }
            OclCreateLocType::CreateAtLocation => target_coord,
            OclCreateLocType::UseOwnerObject => target_coord,
            OclCreateLocType::CreateAboveLocation => {
                let mut coord = target_coord;
                coord.z += CREATE_ABOVE_LOCATION_HEIGHT;
                coord
            }
        };

        let ctx = live_creation_context();
        let create_owner = self.data.create_loc != OclCreateLocType::UseOwnerObject;
        let _ = if create_owner {
            ocl.create_with_angle(
                &ctx,
                Some(&*owner_guard),
                &creation_coord,
                &target_coord,
                0.0,
                0,
            )
        } else {
            ocl.create_with_angle_and_owner_flag(
                &ctx,
                Some(&*owner_guard),
                &creation_coord,
                &target_coord,
                0.0,
                false,
                0,
            )
        };

        Ok(())
    }

    fn resolve_owner_object(&self) -> Option<Arc<RwLock<crate::object::Object>>> {
        if self.owner_object_id != INVALID_ID {
            if let Some(owner) = OBJECT_REGISTRY.get_object(self.owner_object_id) {
                return Some(owner);
            }
        }
        let player_id = self.owner_player_id?;
        let list = player_list().read().ok()?;
        let player = list.get_player(player_id as Int).cloned()?;
        let player_guard = player.read().ok()?;
        let owned = player_guard.get_all_objects();
        drop(player_guard);
        for object_id in owned {
            if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                return Some(obj);
            }
        }
        None
    }
}

impl SpecialPowerModuleInterface for NuclearMissilePower {
    fn get_data(&self) -> &SpecialPowerModuleData {
        &self.data.base
    }

    fn get_data_mut(&mut self) -> &mut SpecialPowerModuleData {
        &mut self.data.base
    }

    fn get_cooldown_state(&self) -> &CooldownState {
        &self.cooldown
    }

    fn get_cooldown_state_mut(&mut self) -> &mut CooldownState {
        &mut self.cooldown
    }

    fn get_stats(&self) -> &SpecialPowerStats {
        &self.stats
    }

    fn get_stats_mut(&mut self) -> &mut SpecialPowerStats {
        &mut self.stats
    }

    fn try_activate(
        &mut self,
        player_id: ObjectID,
        targeting: Option<&TargetingInfo>,
        current_frame: UnsignedInt,
    ) -> ActivationResult {
        if let Err(reason) = self.validate_targeting(targeting) {
            return ActivationResult::InvalidTarget { reason };
        }

        let targeting = match targeting {
            Some(t) => t,
            None => {
                return ActivationResult::InvalidTarget {
                    reason: "Nuclear Missile requires targeting".to_string(),
                };
            }
        };

        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        if !self.can_afford(player_id) {
            let available = self.get_player_money(player_id).unwrap_or(0);
            return ActivationResult::InsufficientFunds {
                cost: self.data.base.cost,
                available,
            };
        }

        if !self.check_prerequisites(player_id) {
            return ActivationResult::MissingPrerequisites {
                required: self.data.base.required_science.clone(),
            };
        }

        self.owner_player_id = Some(player_id);
        if let Some(owner) = self.resolve_owner_object() {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return ActivationResult::Disabled;
                }
            }
        }

        if let Err(reason) = self.execute_strike(targeting) {
            return ActivationResult::Failed { reason };
        }

        if !self.deduct_cost(player_id) {
            return ActivationResult::Failed {
                reason: "Failed to deduct cost".to_string(),
            };
        }

        self.play_sound();
        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_strike(targeting)
    }

    fn update(&mut self, _delta_time: Real) {
        // OCL-based strike has no persistent missile tracking.
    }
}
