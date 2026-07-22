//! Aurora Strike Special Power
//!
//! USA special power that calls in Aurora Alpha bombers for a precision strike.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::OclCreateLocType;
use super::types::*;
use crate::common::*;
use crate::helpers::TheAudio;
use crate::helpers::TheObjectCreationListStore;
use crate::helpers::ThePartitionManager;
use crate::helpers::TheTerrainLogic;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_creation_list::live_creation_context;
use crate::player::player_list;
use std::sync::{Arc, RwLock};

const CREATE_ABOVE_LOCATION_HEIGHT: Real = 300.0;
const MAX_ADJUST_RADIUS: Real = 500.0;
const AURORA_COUNT: Int = 2;
const AURORA_HEIGHT: Real = 400.0;
const AURORA_BOMB_DAMAGE: Real = 800.0;

#[derive(Debug, Clone)]
pub struct AuroraStrikePowerData {
    pub base: SpecialPowerModuleData,
    pub bomber_count: Int,
    pub flight_height: Real,
    pub bomb_damage: Real,
    pub bomb_radius: Real,
    pub bomber_template: AsciiString,
    pub ocl_name: AsciiString,
    pub upgrade_ocl: Vec<(AsciiString, AsciiString)>,
    pub create_loc: OclCreateLocType,
    pub adjust_position_to_passable: Bool,
}

impl AuroraStrikePowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::OCL);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::AFFECTS_BUILDINGS
            | SpecialPowerFlags::SUPERWEAPON;
        base.recharge_time = 360.0; // 6 minutes
        base.cost = 0;
        base.range = 0.0;
        base.radius = 75.0;

        Self {
            base,
            bomber_count: AURORA_COUNT,
            flight_height: AURORA_HEIGHT,
            bomb_damage: AURORA_BOMB_DAMAGE,
            bomb_radius: 75.0,
            bomber_template: "AmericaJetAurora".into(),
            ocl_name: AsciiString::new(),
            upgrade_ocl: Vec::new(),
            create_loc: OclCreateLocType::CreateAtEdgeNearSource,
            adjust_position_to_passable: false,
        }
    }
}

pub struct AuroraStrikePower {
    data: AuroraStrikePowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    active_bombers: Vec<ObjectID>,
    owner_player_id: Option<ObjectID>,
    owner_object_id: ObjectID,
}

impl AuroraStrikePower {
    pub fn new(data: AuroraStrikePowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            active_bombers: Vec::new(),
            owner_player_id: None,
            owner_object_id: INVALID_ID,
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
            return Err("Aurora Strike requires targeting".to_string());
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

    fn execute_strike(
        &mut self,
        player_id: ObjectID,
        targeting: &TargetingInfo,
    ) -> Result<(), String> {
        log::info!(
            "Aurora Strike activated at position {:?}, {} bombers",
            targeting.position,
            self.data.bomber_count
        );

        self.active_bombers.clear();

        self.owner_player_id = Some(player_id);

        let ocl_name = self
            .select_ocl_name()
            .ok_or_else(|| "Aurora Strike requires an OCL configuration".to_string())?;
        let ocl = TheObjectCreationListStore::find_object_creation_list(ocl_name.as_str())
            .ok_or_else(|| format!("OCL '{}' not found for aurora strike", ocl_name))?;
        let owner = self
            .resolve_owner_object()
            .ok_or_else(|| "Aurora Strike requires an owning object".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "Aurora strike owner lock poisoned".to_string())?;
        if owner_guard.is_disabled() {
            return Ok(());
        }

        let mut target_coord = targeting.position;
        if let Some(target_id) = targeting.target_object {
            if let Some(pos) =
                OBJECT_REGISTRY.with_object(target_id, |target_guard| *target_guard.get_position())
            {
                target_coord = pos;
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
        let created = if create_owner {
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

        if let Some(obj) = created {
            if let Ok(guard) = obj.read() {
                self.active_bombers.push(guard.get_id());
            }
        }

        Ok(())
    }

    fn calculate_spawn_positions(&self, targeting: &TargetingInfo) -> Vec<Coord3D> {
        let mut positions = Vec::new();
        if self.data.bomber_count <= 0 {
            return positions;
        }
        if self.data.bomber_count == 1 {
            positions.push(Coord3D::new(
                targeting.position.x - 600.0,
                targeting.position.y,
                targeting.position.z + self.data.flight_height,
            ));
            return positions;
        }

        for i in 0..self.data.bomber_count {
            positions.push(Coord3D::new(
                targeting.position.x - 600.0,
                targeting.position.y + (i as Real * 50.0) - 25.0,
                targeting.position.z + self.data.flight_height,
            ));
        }

        positions
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

    fn select_ocl_name(&self) -> Option<AsciiString> {
        if !self.data.upgrade_ocl.is_empty() {
            if let Some(manager) = super::player_science::get_player_science_manager() {
                if let Ok(mgr) = manager.read() {
                    if let Some(player_id) = self.owner_player_id {
                        if let Some(player_science) = mgr.get_player(player_id) {
                            for (science, ocl) in &self.data.upgrade_ocl {
                                if player_science.has_science(science.as_str()) {
                                    return Some(ocl.clone());
                                }
                            }
                        }
                    }
                }
            }
        }

        if self.data.ocl_name.is_empty() {
            None
        } else {
            Some(self.data.ocl_name.clone())
        }
    }
}

impl SpecialPowerModuleInterface for AuroraStrikePower {
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
                    reason: "Aurora Strike requires targeting".to_string(),
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

        if let Err(reason) = self.execute_strike(player_id, targeting) {
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
        let player_id = self
            .owner_player_id
            .ok_or_else(|| "Aurora Strike requires an owning player".to_string())?;
        self.execute_strike(player_id, targeting)
    }
}
