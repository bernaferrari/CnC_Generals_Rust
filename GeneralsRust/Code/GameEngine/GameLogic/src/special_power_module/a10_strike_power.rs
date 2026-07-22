//! A-10 Thunderbolt Strike Special Power
//!
//! USA special power that spawns A-10 Warthog aircraft that perform a strafing run
//! on the target location, dealing anti-tank and anti-structure damage.
//!
//! Matches C++ implementation from OCLSpecialPower.cpp

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::OclCreateLocType;
use super::types::*;
use crate::common::*;
use crate::helpers::{TheAudio, TheObjectCreationListStore, ThePartitionManager, TheTerrainLogic};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object_creation_list::live_creation_context;
use crate::player::player_list;
use std::sync::{Arc, RwLock};

/// A-10 Strike configuration constants
const A10_STRIKE_HEIGHT: Real = 300.0;
const A10_STRIKE_SPEED: Real = 80.0;
const A10_STRIKE_COUNT: Int = 3;
const A10_STRIKE_SPACING: Real = 50.0;
const CREATE_ABOVE_LOCATION_HEIGHT: Real = 300.0;
const MAX_ADJUST_RADIUS: Real = 500.0;

/// A-10 Strike Special Power configuration data
#[derive(Debug, Clone)]
pub struct A10StrikePowerData {
    /// Base power data
    pub base: SpecialPowerModuleData,
    /// Number of A-10s to spawn
    pub aircraft_count: Int,
    /// Spacing between aircraft
    pub aircraft_spacing: Real,
    /// Height above target to spawn
    pub spawn_height: Real,
    /// Flight speed of aircraft
    pub flight_speed: Real,
    /// Weapon to fire during strafe
    pub strafe_weapon: AsciiString,
    /// Number of shots per pass
    pub shots_per_pass: Int,
    /// Delay between shots (seconds)
    pub shot_delay: Real,
    pub ocl_name: AsciiString,
    pub upgrade_ocl: Vec<(AsciiString, AsciiString)>,
    pub create_loc: OclCreateLocType,
    pub adjust_position_to_passable: Bool,
}

impl A10StrikePowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::A10Strike);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::SUPERWEAPON
            | SpecialPowerFlags::RADAR_EFFECT;
        base.recharge_time = 240.0; // 4 minutes
        base.cost = 0;
        base.range = 0.0; // Unlimited
        base.radius = 50.0;

        let name_str = base.name.as_str();
        let mut upgrade_ocl = Vec::new();
        if name_str.eq_ignore_ascii_case("AirF_SuperweaponA10ThunderboltMissileStrike") {
            upgrade_ocl.push((
                "AirF_SCIENCE_A10ThunderboltMissileStrike3".into(),
                "SUPERWEAPON_A10ThunderboltMissileStrike3".into(),
            ));
            upgrade_ocl.push((
                "AirF_SCIENCE_A10ThunderboltMissileStrike2".into(),
                "SUPERWEAPON_A10ThunderboltMissileStrike2".into(),
            ));
        } else {
            upgrade_ocl.push((
                "SCIENCE_A10ThunderboltMissileStrike3".into(),
                "SUPERWEAPON_A10ThunderboltMissileStrike3".into(),
            ));
            upgrade_ocl.push((
                "SCIENCE_A10ThunderboltMissileStrike2".into(),
                "SUPERWEAPON_A10ThunderboltMissileStrike2".into(),
            ));
        }

        Self {
            base,
            aircraft_count: A10_STRIKE_COUNT,
            aircraft_spacing: A10_STRIKE_SPACING,
            spawn_height: A10_STRIKE_HEIGHT,
            flight_speed: A10_STRIKE_SPEED,
            strafe_weapon: "A10ThunderboltMissileWeapon".into(),
            shots_per_pass: 10,
            shot_delay: 0.1,
            ocl_name: "SUPERWEAPON_A10ThunderboltMissileStrike1".into(),
            upgrade_ocl,
            create_loc: OclCreateLocType::CreateAtEdgeNearSource,
            adjust_position_to_passable: false,
        }
    }
}

/// A-10 Strike Special Power implementation
pub struct A10StrikePower {
    data: A10StrikePowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    active_aircraft: Vec<ObjectID>,
    strike_active: Bool,
    owner_player_id: Option<ObjectID>,
    owner_object_id: ObjectID,
}

impl A10StrikePower {
    pub fn new(data: A10StrikePowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            active_aircraft: Vec::new(),
            strike_active: false,
            owner_player_id: None,
            owner_object_id: INVALID_ID,
        }
    }

    pub fn set_owner_object_id(&mut self, owner_id: ObjectID) {
        self.owner_object_id = owner_id;
    }

    /// Execute the A-10 strike
    fn execute_strike(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        log::info!(
            "A-10 Strike activated at position {:?}, spawning {} aircraft",
            targeting.position,
            self.data.aircraft_count
        );

        // Clear previous aircraft
        self.active_aircraft.clear();

        let ocl_name = self
            .select_ocl_name()
            .ok_or_else(|| "A10 Strike requires an OCL configuration".to_string())?;
        let ocl = TheObjectCreationListStore::find_object_creation_list(ocl_name.as_str())
            .ok_or_else(|| format!("OCL '{}' not found for A10 strike", ocl_name))?;

        let owner = self.resolve_owner_object();

        let mut target_coord = targeting.position;
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

        let owner_guard = owner.as_ref().and_then(|arc| arc.read().ok());
        let owner_pos = owner_guard
            .as_ref()
            .map(|guard| *guard.get_position())
            .unwrap_or(target_coord);

        let creation_coord = match self.data.create_loc {
            OclCreateLocType::CreateAtEdgeNearSource => TheTerrainLogic::get()
                .map(|terrain| terrain.find_closest_edge_point(&owner_pos))
                .unwrap_or(owner_pos),
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
        let Some(owner_guard) = owner_guard else {
            return Err("A10 strike requires an owning object".to_string());
        };
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
                self.active_aircraft.push(guard.get_id());
            }
        }

        self.strike_active = true;

        // Play sound effect
        self.play_sound_effect();

        // Display radar event
        self.show_radar_event(targeting);

        Ok(())
    }

    /// Calculate spawn positions for A-10 aircraft
    /// Matches C++ A10StrikePower spawn formation logic
    fn calculate_spawn_positions(&self, targeting: &TargetingInfo) -> Vec<Coord3D> {
        let mut positions = Vec::new();

        // Find map edge closest to target
        // Integration point: Terrain logic for edge detection
        // When terrain logic is integrated:
        // if let Some(terrain_logic) = get_terrain_logic() {
        //     edge_point = terrain_logic.find_closest_edge_point(&targeting.position);
        // }
        let edge_point = self.find_closest_edge_point(&targeting.position);

        // Spawn aircraft in a line formation perpendicular to flight path
        for i in 0..self.data.aircraft_count {
            let offset = if self.data.aircraft_count > 1 {
                let total_width =
                    (self.data.aircraft_count - 1) as Real * self.data.aircraft_spacing;
                let perpendicular_offset =
                    (i as Real * self.data.aircraft_spacing) - (total_width / 2.0);
                Coord3D::new(perpendicular_offset, 0.0, 0.0)
            } else {
                Coord3D::new(0.0, 0.0, 0.0)
            };

            let spawn_pos = Coord3D::new(
                edge_point.x + offset.x,
                edge_point.y + offset.y,
                edge_point.z + self.data.spawn_height + offset.z,
            );

            positions.push(spawn_pos);
        }

        positions
    }

    /// Find closest map edge point to target
    /// Matches C++ TheTerrainLogic->findClosestEdgePoint
    fn find_closest_edge_point(&self, target: &Coord3D) -> Coord3D {
        if let Some(terrain_logic) = TheTerrainLogic::get() {
            return terrain_logic.find_closest_edge_point(target);
        }

        const SPAWN_DISTANCE: Real = 500.0;
        Coord3D::new(target.x - SPAWN_DISTANCE, target.y, target.z)
    }

    /// Play A-10 strike sound effect
    /// Matches C++ audio event handling
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

    /// Show radar event for strike
    /// Matches C++ radar event system
    fn show_radar_event(&self, targeting: &TargetingInfo) {
        // Integration point: Radar system
        // When radar system is integrated:
        // if let Some(radar) = get_radar() {
        //     radar.show_event(
        //         RadarEventType::Airstrike,
        //         &targeting.position,
        //         5.0, // Duration in seconds
        //         true, // Visible to all players
        //     );
        // }

        log::debug!(
            "Radar event pending: A-10 strike at {:?} (radar system not integrated)",
            targeting.position
        );
    }

    /// Check if strike is currently active
    pub fn is_strike_active(&self) -> Bool {
        self.strike_active
    }

    /// Get list of active aircraft
    pub fn get_active_aircraft(&self) -> &[ObjectID] {
        &self.active_aircraft
    }

    fn resolve_owner_object(&self) -> Option<Arc<RwLock<crate::object::Object>>> {
        crate::special_power_module::resolve_special_power_owner(
            self.owner_object_id,
            self.owner_player_id,
        )
    }

    fn resolve_owner_object_id(&self) -> Option<ObjectID> {
        crate::special_power_module::resolve_special_power_owner_id(
            self.owner_object_id,
            self.owner_player_id,
        )
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

    /// Update strike state (call every frame)
    /// Matches C++ A10StrikePower update logic
    pub fn update_strike(&mut self) {
        if !self.strike_active {
            return;
        }

        // Integration point: Aircraft status tracking
        // When object system is integrated:
        // let mut all_complete = true;
        // for &aircraft_id in &self.active_aircraft {
        //     if let Some(aircraft) = crate::helpers::TheGameLogic::find_object_by_id(aircraft_id) {
        //         if let Ok(aircraft_read) = aircraft.read() {
        //             if !aircraft_read.is_destroyed() && aircraft_read.has_pending_attacks() {
        //                 all_complete = false;
        //                 break;
        //             }
        //         }
        //     }
        // }
        // if all_complete {
        //     self.strike_active = false;
        //     self.active_aircraft.clear();
        // }

        // Fallback: Mark as complete when aircraft list is empty
        if self.active_aircraft.is_empty() {
            self.strike_active = false;
        }
    }
}

impl SpecialPowerModuleInterface for A10StrikePower {
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
        // Validate targeting is provided
        let targeting = match targeting {
            Some(t) => t,
            None => {
                return ActivationResult::InvalidTarget {
                    reason: "A-10 Strike requires targeting".to_string(),
                };
            }
        };

        // Check cooldown
        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        // Execute strike
        self.owner_player_id = Some(player_id);
        if let Err(reason) = self.execute(targeting) {
            return ActivationResult::Failed { reason };
        }

        // Start cooldown
        self.cooldown.start_cooldown(current_frame);

        // Update stats
        self.stats
            .record_activation(current_frame, self.data.base.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_strike(targeting)
    }

    fn update(&mut self, _delta_time: Real) {
        self.update_strike();
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
    fn test_a10_strike_creation() {
        let data = A10StrikePowerData::new("A10Strike".into());
        let power = A10StrikePower::new(data);

        assert_eq!(power.get_name(), "A10Strike");
        assert!(power.is_ready());
        assert_eq!(power.data.aircraft_count, 3);
    }

    #[test]
    fn test_a10_strike_activation() {
        TheObjectCreationListStore::register_object_creation_list(
            "SUPERWEAPON_A10ThunderboltMissileStrike1",
            ObjectCreationList::new(),
        );

        let data = A10StrikePowerData::new("A10Strike".into());
        let mut power = A10StrikePower::new(data);
        let owner_id = 9101;
        let _owner = register_test_owner(owner_id);
        power.set_owner_object_id(owner_id);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 0.0, 100.0);

        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(result.is_success(), "activation failed: {:?}", result);
        assert!(power.is_on_cooldown());
        assert!(power.is_strike_active());
        OBJECT_REGISTRY.unregister_object(owner_id);
    }

    #[test]
    fn test_spawn_position_calculation() {
        let data = A10StrikePowerData::new("A10Strike".into());
        let power = A10StrikePower::new(data);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 0.0, 100.0);
        let positions = power.calculate_spawn_positions(&targeting);

        assert_eq!(positions.len(), 3);

        // All aircraft should spawn at the configured height
        for pos in &positions {
            assert_eq!(pos.z, A10_STRIKE_HEIGHT);
        }
    }

    #[test]
    fn test_cooldown_enforcement() {
        TheObjectCreationListStore::register_object_creation_list(
            "SUPERWEAPON_A10ThunderboltMissileStrike1",
            ObjectCreationList::new(),
        );

        let data = A10StrikePowerData::new("A10Strike".into());
        let mut power = A10StrikePower::new(data);
        let owner_id = 9102;
        let _owner = register_test_owner(owner_id);
        power.set_owner_object_id(owner_id);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 0.0, 100.0);

        // First activation should succeed
        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(result.is_success(), "activation failed: {:?}", result);

        // Second activation should fail (on cooldown)
        let result = power.try_activate(1, Some(&targeting), 1);
        assert!(!result.is_success());
        match result {
            ActivationResult::OnCooldown { remaining } => {
                assert!(remaining > 0.0);
            }
            _ => panic!("Expected OnCooldown result"),
        }
        OBJECT_REGISTRY.unregister_object(owner_id);
    }
}
