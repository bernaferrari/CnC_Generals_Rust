//! Fuel Air Bomb (Daisy Cutter) Special Power
//!
//! USA superweapon that drops a massive fuel-air explosive bomb causing
//! devastating area damage. Also known as MOAB (Mother of All Bombs).
//!
//! Matches C++ implementation from OCLSpecialPower.cpp

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::helpers::{
    TheAudio, TheGameLogic, TheObjectCreationListStore, ThePartitionManager, TheTerrainLogic,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_powers::OclCreateLocType;
use crate::object_creation_list::live_creation_context;
use crate::player::player_list;
use crate::special_power_module::area_damage::{
    AreaDamageApplicator, AreaDamageConfig, DamageFalloff,
};
use std::sync::{Arc, RwLock};

/// Fuel Air Bomb configuration constants
const FAB_DAMAGE_RADIUS: Real = 170.0;
const FAB_DAMAGE_AMOUNT: Real = 2000.0;
const FAB_BOMBER_HEIGHT: Real = 400.0;
const FAB_FALLOFF_DISTANCE: Real = 100.0;
const CREATE_ABOVE_LOCATION_HEIGHT: Real = 300.0;
const MAX_ADJUST_RADIUS: Real = 500.0;

/// Fuel Air Bomb Special Power configuration data
#[derive(Debug, Clone)]
pub struct FuelAirBombPowerData {
    /// Base power data
    pub base: SpecialPowerModuleData,
    /// Damage radius of explosion
    pub damage_radius: Real,
    /// Maximum damage at epicenter
    pub max_damage: Real,
    /// Damage falloff distance
    pub falloff_distance: Real,
    /// Height of bomber aircraft
    pub bomber_height: Real,
    /// Bomb projectile template
    pub bomb_projectile: AsciiString,
    /// OCL used to spawn the bomber/strike payload
    pub bomber_ocl: AsciiString,
    /// Explosion FX name
    pub explosion_fx: AsciiString,
    /// Delay before bomb impacts (seconds)
    pub impact_delay: Real,
    /// OCL creation location strategy
    pub create_loc: OclCreateLocType,
    /// Whether to snap target to a passable location before creating OCL
    pub adjust_position_to_passable: Bool,
}

impl FuelAirBombPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::DaisyCutter);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::AFFECTS_BUILDINGS
            | SpecialPowerFlags::AFFECTS_TERRAIN
            | SpecialPowerFlags::SUPERWEAPON
            | SpecialPowerFlags::RADAR_EFFECT;
        base.recharge_time = 360.0; // 6 minutes
        base.cost = 0;
        base.range = 0.0; // Unlimited
        base.radius = FAB_DAMAGE_RADIUS;

        Self {
            base,
            damage_radius: FAB_DAMAGE_RADIUS,
            max_damage: FAB_DAMAGE_AMOUNT,
            falloff_distance: FAB_FALLOFF_DISTANCE,
            bomber_height: FAB_BOMBER_HEIGHT,
            bomb_projectile: "DaisyCutterBomb".into(),
            bomber_ocl: "SUPERWEAPON_DaisyCutter".into(),
            explosion_fx: "FX_DaisyCutterExplosion".into(),
            impact_delay: 3.0,
            create_loc: OclCreateLocType::CreateAtEdgeNearSource,
            adjust_position_to_passable: false,
        }
    }
}

/// Fuel Air Bomb Special Power implementation
pub struct FuelAirBombPower {
    data: FuelAirBombPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    bomber_aircraft_id: Option<ObjectID>,
    owner_player_id: Option<ObjectID>,
    owner_object_id: ObjectID,
    impact_pending: Bool,
    target_position: Coord3D,
    impact_frame: UnsignedInt,
}

impl FuelAirBombPower {
    pub fn new(data: FuelAirBombPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            bomber_aircraft_id: None,
            owner_player_id: None,
            owner_object_id: INVALID_ID,
            impact_pending: false,
            target_position: Coord3D::new(0.0, 0.0, 0.0),
            impact_frame: 0,
        }
    }

    pub fn set_owner_object_id(&mut self, owner_id: ObjectID) {
        self.owner_object_id = owner_id;
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

    fn resolve_target_position(&self, targeting: &TargetingInfo) -> Coord3D {
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

        target_coord
    }

    fn resolve_creation_position(&self, owner_pos: Coord3D, target_coord: Coord3D) -> Coord3D {
        match self.data.create_loc {
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
        }
    }

    /// Execute the fuel air bomb strike using OCL creation flow (C++ OCLSpecialPower parity path).
    fn execute_strike(
        &mut self,
        targeting: &TargetingInfo,
        current_frame: UnsignedInt,
    ) -> Result<(), String> {
        let owner = self
            .resolve_owner_object()
            .ok_or_else(|| "Fuel Air Bomb requires an owning object".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "Fuel Air Bomb owner lock poisoned".to_string())?;
        if owner_guard.is_disabled() {
            return Ok(());
        }

        let target_coord = self.resolve_target_position(targeting);
        let owner_pos = *owner_guard.get_position();
        let creation_coord = self.resolve_creation_position(owner_pos, target_coord);

        log::info!(
            "Fuel Air Bomb activated at {:?} (creation {:?})",
            target_coord,
            creation_coord
        );

        self.target_position = target_coord;
        self.spawn_bomber(&owner_guard, &creation_coord, &target_coord)?;

        // C++ logic timing is frame-based (30 FPS logic).
        const GAME_FPS: Real = 30.0;
        self.impact_frame = current_frame
            .saturating_add((self.data.impact_delay * GAME_FPS).max(0.0) as UnsignedInt);
        self.impact_pending = true;

        self.play_warning_sound();
        self.show_radar_event(targeting);
        Ok(())
    }

    fn spawn_bomber(
        &mut self,
        owner_guard: &crate::object::Object,
        creation_pos: &Coord3D,
        target_pos: &Coord3D,
    ) -> Result<(), String> {
        let ocl =
            TheObjectCreationListStore::find_object_creation_list(self.data.bomber_ocl.as_str())
                .ok_or_else(|| {
                    format!("OCL '{}' not found for fuel air bomb", self.data.bomber_ocl)
                })?;

        let ctx = live_creation_context();
        let create_owner = self.data.create_loc != OclCreateLocType::UseOwnerObject;
        let created = if create_owner {
            ocl.create_with_angle(&ctx, Some(owner_guard), creation_pos, target_pos, 0.0, 0)
        } else {
            ocl.create_with_angle_and_owner_flag(
                &ctx,
                Some(owner_guard),
                creation_pos,
                target_pos,
                0.0,
                false,
                0,
            )
        };

        self.bomber_aircraft_id = created
            .as_ref()
            .and_then(|obj| obj.read().ok().map(|guard| guard.get_id()));
        Ok(())
    }

    /// Detonate the bomb at target location
    fn detonate_bomb(&mut self) -> Result<(), String> {
        log::info!("Detonating fuel air bomb at {:?}", self.target_position);

        // Apply area damage
        self.apply_area_damage()?;

        // Trigger explosion effects
        self.trigger_explosion_effects();

        self.impact_pending = false;
        self.bomber_aircraft_id = None;

        Ok(())
    }

    /// Apply area damage to all units and structures in radius
    fn apply_area_damage(&mut self) -> Result<(), String> {
        let mut config = AreaDamageConfig::new(self.data.max_damage, self.data.damage_radius);
        config.min_damage = 0.0;
        config.falloff = DamageFalloff::TwoStage {
            inner_radius: self.data.falloff_distance,
        };
        config.damage_type = DamageTypeFlags::EXPLOSION;
        config.affects_friendlies = false;
        config.affects_buildings = true;
        config.affects_terrain = true;

        let attacker_id = if self.owner_object_id != INVALID_ID {
            self.owner_object_id
        } else {
            INVALID_ID
        };
        let result = AreaDamageApplicator::apply_damage_at_location(
            &config,
            &self.target_position,
            attacker_id,
        )?;

        self.stats.record_damage(result.total_damage);
        for object_id in result.objects_damaged {
            if let Some(object) = TheGameLogic::find_object_by_id(object_id) {
                if let Ok(object_guard) = object.read() {
                    if object_guard.is_structure() {
                        self.stats.record_building_affected();
                    } else {
                        self.stats.record_unit_affected();
                    }
                }
            }
        }
        Ok(())
    }

    /// Calculate damage falloff based on distance from epicenter
    fn calculate_falloff(&self, distance: Real) -> Real {
        if distance <= self.data.falloff_distance {
            1.0
        } else if distance >= self.data.damage_radius {
            0.0
        } else {
            let falloff_range = self.data.damage_radius - self.data.falloff_distance;
            let falloff_distance = distance - self.data.falloff_distance;
            1.0 - (falloff_distance / falloff_range)
        }
    }

    /// Trigger explosion visual and audio effects
    /// Matches C++ FX and audio system calls
    fn trigger_explosion_effects(&self) {
        log::debug!("Triggering explosion FX: {}", self.data.explosion_fx);

        // Integration point: FX system
        // When FX system is integrated:
        // if let Some(fx_mgr) = get_fx_manager() {
        //     fx_mgr.create_fx(
        //         &self.data.explosion_fx,
        //         &self.target_position,
        //         None, // No orientation
        //     );
        // }

        // Integration point: Audio system
        // When audio system is integrated:
        // if let Some(audio_mgr) = get_audio_manager() {
        //     audio_mgr.play_sound_3d(
        //         "FuelAirBombExplosion",
        //         &self.target_position,
        //         1.0, // Volume
        //     );
        // }

        // Integration point: Camera system
        // When camera system is integrated:
        // if let Some(camera) = get_camera() {
        //     camera.shake(1.0, 2.0); // Intensity and duration
        // }

        log::info!(
            "Fuel air bomb explosion effects at {:?} (FX, audio, camera shake pending)",
            self.target_position
        );
    }

    /// Play warning sound before impact
    /// Matches C++ audio event handling
    fn play_warning_sound(&self) {
        if self.data.base.sound_effect.is_empty() {
            return;
        }

        if let Some(audio) = TheAudio::get() {
            let event =
                crate::common::audio::AudioEventRts::new(self.data.base.sound_effect.as_str());
            audio.add_audio_event(&event);
        }
    }

    /// Show radar event for bomb strike
    /// Matches C++ radar event system
    fn show_radar_event(&self, targeting: &TargetingInfo) {
        // Integration point: Radar system
        // When radar system is integrated:
        // if let Some(radar) = get_radar() {
        //     radar.show_event(
        //         RadarEventType::SuperweaponAttack,
        //         &targeting.position,
        //         5.0, // Duration in seconds
        //         true, // Visible to all players
        //     );
        // }

        log::debug!(
            "Radar event pending: Fuel air bomb at {:?} (radar system not integrated)",
            targeting.position
        );
    }

    /// Update bomb strike state (call every frame)
    pub fn update_strike(&mut self, current_frame: UnsignedInt) {
        if self.impact_pending && current_frame >= self.impact_frame {
            if let Err(e) = self.detonate_bomb() {
                log::error!("Failed to detonate bomb: {}", e);
            }
        }
    }
}

impl SpecialPowerModuleInterface for FuelAirBombPower {
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
                    reason: "Fuel Air Bomb requires targeting".to_string(),
                };
            }
        };

        // Check cooldown
        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
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

        // Execute strike
        if let Err(reason) = self.execute_strike(targeting, current_frame) {
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
        self.execute_strike(targeting, TheGameLogic::get_frame())
    }

    fn update(&mut self, _delta_time: Real) {
        self.update_strike(TheGameLogic::get_frame());
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
    fn test_fuel_air_bomb_creation() {
        let data = FuelAirBombPowerData::new("DaisyCutter".into());
        let power = FuelAirBombPower::new(data);

        assert_eq!(power.get_name(), "DaisyCutter");
        assert!(power.is_ready());
        assert_eq!(power.data.damage_radius, FAB_DAMAGE_RADIUS);
    }

    #[test]
    fn test_fuel_air_bomb_activation() {
        TheObjectCreationListStore::register_object_creation_list(
            "SUPERWEAPON_DaisyCutter",
            ObjectCreationList::new(),
        );

        let data = FuelAirBombPowerData::new("DaisyCutter".into());
        let mut power = FuelAirBombPower::new(data);
        let owner_id = 9401;
        let _owner = register_test_owner(owner_id);
        power.set_owner_object_id(owner_id);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 0.0, 200.0);

        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(result.is_success(), "activation failed: {:?}", result);
        assert!(power.is_on_cooldown());
        assert!(power.impact_pending);
        assert!(power.impact_frame > 0);

        power.update_strike(power.impact_frame);
        assert!(!power.impact_pending);

        OBJECT_REGISTRY.unregister_object(owner_id);
    }

    #[test]
    fn test_damage_falloff_calculation() {
        let data = FuelAirBombPowerData::new("DaisyCutter".into());
        let power = FuelAirBombPower::new(data);

        // Full damage at epicenter
        assert_eq!(power.calculate_falloff(0.0), 1.0);

        // Full damage within falloff distance
        assert_eq!(power.calculate_falloff(FAB_FALLOFF_DISTANCE), 1.0);

        // Half damage at midpoint of falloff range
        let midpoint = FAB_FALLOFF_DISTANCE + (FAB_DAMAGE_RADIUS - FAB_FALLOFF_DISTANCE) / 2.0;
        assert!((power.calculate_falloff(midpoint) - 0.5).abs() < 0.01);

        // No damage at max radius
        assert_eq!(power.calculate_falloff(FAB_DAMAGE_RADIUS), 0.0);

        // No damage beyond radius
        assert_eq!(power.calculate_falloff(FAB_DAMAGE_RADIUS + 100.0), 0.0);
    }
}
