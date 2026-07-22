//! Paradrop Special Power
//!
//! USA special power that drops paratroopers or vehicles at target location.
//! Units are spawned in the air and parachute down to the ground.
//!
//! Matches C++ implementation from OCLSpecialPower.cpp

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::OclCreateLocType;
use super::types::*;
use crate::common::*;
use crate::helpers::{
    get_game_logic_random_value, get_game_logic_random_value_real, TheAudio, TheGameLogic,
    ThePartitionManager, TheTerrainLogic,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::player::player_list;
use crate::terrain::get_terrain_logic;
use std::sync::{Arc, RwLock};

/// Paradrop configuration constants
const PARADROP_HEIGHT: Real = 250.0;
const PARADROP_SPACING: Real = 30.0;
const PARADROP_DELAY: Real = 0.15;
const CREATE_ABOVE_LOCATION_HEIGHT: Real = 300.0;
const MAX_ADJUST_RADIUS: Real = 500.0;

/// Paradrop Special Power configuration data
#[derive(Debug, Clone)]
pub struct ParadropPowerData {
    /// Base power data
    pub base: SpecialPowerModuleData,
    /// OCL containing units to drop
    pub drop_ocl: AsciiString,
    /// Upgrade OCLs based on science
    pub upgrade_ocl: Vec<(AsciiString, AsciiString)>,
    /// Number of units to drop
    pub unit_count: Int,
    /// Spacing between drop points
    pub drop_spacing: Real,
    /// Height to spawn units at
    pub drop_height: Real,
    /// Delay between drops (seconds)
    pub drop_delay: Real,
    /// Pattern for drop (line, circle, etc)
    pub drop_pattern: AsciiString,
    pub create_loc: OclCreateLocType,
    pub adjust_position_to_passable: Bool,
}

impl ParadropPowerData {
    pub fn new(name: AsciiString, drop_ocl: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::OCL);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING | SpecialPowerFlags::AFFECTS_FRIENDLY;
        base.recharge_time = 240.0; // 4 minutes (240000 ms)
        base.cost = 0;
        base.range = 0.0; // Unlimited
        base.radius = 50.0;

        let name_str = base.name.as_str();
        let mut upgrade_ocl = Vec::new();
        if name_str.eq_ignore_ascii_case("Infa_SuperweaponInfantryParadrop") {
            upgrade_ocl.push((
                "Infa_SCIENCE_InfantryParadrop3".into(),
                "Infa_SUPERWEAPON_Paradrop3".into(),
            ));
            upgrade_ocl.push((
                "Infa_SCIENCE_InfantryParadrop2".into(),
                "Infa_SUPERWEAPON_Paradrop2".into(),
            ));
        } else if name_str.eq_ignore_ascii_case("Tank_SuperweaponTankParadrop") {
            upgrade_ocl.push((
                "SCIENCE_TankParadrop3".into(),
                "Tank_SUPERWEAPON_TankParadrop3".into(),
            ));
            upgrade_ocl.push((
                "SCIENCE_TankParadrop2".into(),
                "Tank_SUPERWEAPON_TankParadrop2".into(),
            ));
        } else {
            upgrade_ocl.push(("SCIENCE_Paradrop3".into(), "SUPERWEAPON_Paradrop3".into()));
            upgrade_ocl.push(("SCIENCE_Paradrop2".into(), "SUPERWEAPON_Paradrop2".into()));
        }

        Self {
            base,
            drop_ocl,
            upgrade_ocl,
            unit_count: 5,
            drop_spacing: PARADROP_SPACING,
            drop_height: PARADROP_HEIGHT,
            drop_delay: PARADROP_DELAY,
            drop_pattern: "Line".into(),
            create_loc: OclCreateLocType::CreateAtEdgeNearSource,
            adjust_position_to_passable: false,
        }
    }
}

/// Paradrop Special Power implementation
pub struct ParadropPower {
    data: ParadropPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    dropped_units: Vec<ObjectID>,
    aircraft_id: Option<ObjectID>,
    owner_player_id: Option<ObjectID>,
    owner_object_id: ObjectID,
}

impl ParadropPower {
    pub fn new(data: ParadropPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            dropped_units: Vec::new(),
            aircraft_id: None,
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
            return Err("Paradrop requires targeting".to_string());
        }
        if self.data.base.is_instant() && targeting.is_some() {
            return Err("Instant power does not accept targeting".to_string());
        }
        Ok(())
    }

    /// Execute the paradrop
    fn execute_drop(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        log::info!(
            "Paradrop activated at position {:?}, dropping {} units",
            targeting.position,
            self.data.unit_count
        );

        self.dropped_units.clear();

        use crate::helpers::TheObjectCreationListStore;
        let ocl_name = self.select_ocl_name();
        let Some(ocl) = ocl_name
            .as_ref()
            .and_then(|name| TheObjectCreationListStore::find_object_creation_list(name))
        else {
            let fallback = if self.data.drop_ocl.is_empty() {
                "<empty>"
            } else {
                self.data.drop_ocl.as_str()
            };
            return Err(format!("OCL '{}' not found for paradrop", fallback));
        };

        let owner = self
            .resolve_owner_object()
            .ok_or_else(|| "Paradrop requires an owning object".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "Paradrop owner lock poisoned".to_string())?;
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

        let ctx = crate::object_creation_list::live_creation_context();
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

        if let Some(handle) = created {
            if let Ok(guard) = handle.read() {
                self.dropped_units.push(guard.get_id());
            }
        }

        // Play sound effect
        self.play_sound_effect();

        Ok(())
    }

    /// Calculate drop positions based on pattern
    fn calculate_drop_positions(&self, targeting: &TargetingInfo) -> Vec<Coord3D> {
        let mut positions = Vec::new();
        let terrain = get_terrain_logic().read().ok();

        match self.data.drop_pattern.as_str() {
            "Circle" => {
                let angle_step = (2.0 * std::f32::consts::PI) / self.data.unit_count as Real;
                for i in 0..self.data.unit_count {
                    let angle = i as Real * angle_step;
                    let mut pos = Coord3D::new(
                        targeting.position.x + angle.cos() * self.data.drop_spacing,
                        targeting.position.y + angle.sin() * self.data.drop_spacing,
                        targeting.position.z,
                    );
                    if let Some(guard) = terrain.as_ref() {
                        pos.z = guard.get_ground_height(pos.x, pos.y, None);
                    }
                    positions.push(pos);
                }
            }
            _ => {
                // Default: Line formation
                for i in 0..self.data.unit_count {
                    let offset = if self.data.unit_count > 1 {
                        let total_width =
                            (self.data.unit_count - 1) as Real * self.data.drop_spacing;
                        (i as Real * self.data.drop_spacing) - (total_width / 2.0)
                    } else {
                        0.0
                    };

                    let mut pos = Coord3D::new(
                        targeting.position.x + offset,
                        targeting.position.y,
                        targeting.position.z,
                    );
                    if let Some(guard) = terrain.as_ref() {
                        pos.z = guard.get_ground_height(pos.x, pos.y, None);
                    }
                    positions.push(pos);
                }
            }
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
        if self.data.drop_ocl.is_empty() {
            None
        } else {
            Some(self.data.drop_ocl.clone())
        }
    }

    /// Spawn transport aircraft
    fn spawn_transport_aircraft(&mut self, target: &Coord3D) -> Result<(), String> {
        log::debug!("Spawning transport aircraft for paradrop");

        const PARADROP_PLANE_OCL: &str = "OCL_ParadropPlane";
        use crate::helpers::TheObjectCreationListStore;

        let Some(ocl) = TheObjectCreationListStore::find_object_creation_list(&AsciiString::from(
            PARADROP_PLANE_OCL,
        )) else {
            log::debug!("Paradrop transport OCL not found: {}", PARADROP_PLANE_OCL);
            self.aircraft_id = None;
            return Ok(());
        };

        let ctx = crate::object_creation_list::live_creation_context();

        let start = self.find_closest_edge(target);
        if self.owner_object_id == INVALID_ID {
            self.aircraft_id = None;
            return Ok(());
        }
        let Some(created) = OBJECT_REGISTRY.with_object(self.owner_object_id, |primary_obj| {
            ocl.create_with_owner_flag(&ctx, Some(primary_obj), &start, target, true, 0)
        }) else {
            self.aircraft_id = None;
            return Ok(());
        };
        self.aircraft_id = created.and_then(|h| h.read().ok().map(|o| o.get_id()));

        Ok(())
    }

    /// Find closest map edge
    fn find_closest_edge(&self, target: &Coord3D) -> Coord3D {
        let Ok(guard) = get_terrain_logic().read() else {
            let mut fallback = *target;
            fallback.z = fallback.z.max(self.data.drop_height);
            return fallback;
        };
        let mut edge = guard.find_closest_edge_point(target);
        edge.z = edge.z.max(self.data.drop_height);
        edge
    }

    /// Play paradrop sound effect
    fn play_sound_effect(&self) {
        if self.data.base.sound_effect.is_empty() {
            return;
        }

        if let Some(audio) = TheAudio::get() {
            let event =
                crate::common::audio::AudioEventRts::new(self.data.base.sound_effect.as_str());
            audio.add_audio_event(&event);
        }
    }

    /// Get list of dropped units
    pub fn get_dropped_units(&self) -> &[ObjectID] {
        &self.dropped_units
    }
}

impl SpecialPowerModuleInterface for ParadropPower {
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
                    reason: "Paradrop requires targeting".to_string(),
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

        if !self.deduct_cost(player_id) {
            return ActivationResult::Failed {
                reason: "Failed to deduct cost".to_string(),
            };
        }

        self.owner_player_id = Some(player_id);
        if let Err(reason) = self.execute(targeting) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_drop(targeting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effects::ObjectCreationList;
    use crate::helpers::TheObjectCreationListStore;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use std::sync::{Arc, RwLock};

    fn register_test_owner(owner_id: ObjectID) -> Arc<RwLock<Object>> {
        let owner = Arc::new(RwLock::new(Object::new_test(owner_id, 100.0)));
        OBJECT_REGISTRY.register_object(owner_id, &owner);
        owner
    }

    #[test]
    fn test_paradrop_creation() {
        let data = ParadropPowerData::new("Paradrop".into(), "OCL_Rangers".into());
        let power = ParadropPower::new(data);

        assert_eq!(power.get_name(), "Paradrop");
        assert!(power.is_ready());
    }

    #[test]
    fn test_paradrop_activation() {
        TheObjectCreationListStore::register_object_creation_list(
            "OCL_Rangers",
            ObjectCreationList::new(),
        );

        let data = ParadropPowerData::new("Paradrop".into(), "OCL_Rangers".into());
        let mut power = ParadropPower::new(data);
        let owner_id = 9201;
        let _owner = register_test_owner(owner_id);
        power.set_owner_object_id(owner_id);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 0.0, 50.0);
        let result = power.try_activate(1, Some(&targeting), 0);

        assert!(result.is_success(), "activation failed: {:?}", result);
        assert!(power.is_on_cooldown());
        // With an empty OCL in this unit test harness, no objects are spawned.
        // Integration tests that load real templates/nuggets should validate actual counts.
        assert_eq!(power.get_dropped_units().len(), 0);
        OBJECT_REGISTRY.unregister_object(owner_id);
    }
}
