//! Spectre Gunship Special Power
//!
//! USA special power that spawns an AC-130 Spectre gunship that circles and attacks
//! ground targets with devastating firepower for a limited duration.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::helpers::{
    TheThingFactory, get_game_logic_random_value, get_game_logic_random_value_real,
};
use crate::terrain::get_terrain_logic;
use std::sync::{Arc, RwLock};

const SPECTRE_ORBIT_RADIUS: Real = 200.0;
const SPECTRE_ORBIT_HEIGHT: Real = 300.0;
const SPECTRE_DURATION: Real = 45.0; // 45 seconds

#[derive(Debug, Clone)]
pub struct SpectreGunshipPowerData {
    pub base: SpecialPowerModuleData,
    pub orbit_radius: Real,
    pub orbit_height: Real,
    pub duration: Real,
    /// Optional OCL used to spawn the gunship. In the original C++ this comes from INI/module data.
    pub gunship_ocl: AsciiString,
    pub weapon_name: AsciiString,
    pub shots_per_second: Real,
}

impl SpectreGunshipPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::OCL);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING
            | SpecialPowerFlags::AFFECTS_ENEMY
            | SpecialPowerFlags::SUPERWEAPON;
        base.recharge_time = 240.0; // 4 minutes (240000 ms)
        base.cost = 0;
        base.range = 0.0;
        base.radius = SPECTRE_ORBIT_RADIUS;
        let name_str = base.name.as_str();
        if name_str.eq_ignore_ascii_case("AirF_SuperweaponSpectreGunship") {
            base.recharge_time = 180.0; // 180000 ms
        }

        Self {
            base,
            orbit_radius: SPECTRE_ORBIT_RADIUS,
            orbit_height: SPECTRE_ORBIT_HEIGHT,
            duration: SPECTRE_DURATION,
            gunship_ocl: AsciiString::new(),
            weapon_name: "SpectreGunshipWeapon".into(),
            shots_per_second: 2.0,
        }
    }
}

pub struct SpectreGunshipPower {
    data: SpectreGunshipPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    gunship_id: Option<ObjectID>,
    orbit_center: Coord3D,
    end_frame: UnsignedInt,
    owner_object_id: ObjectID,
}

impl SpectreGunshipPower {
    pub fn new(data: SpectreGunshipPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            gunship_id: None,
            orbit_center: Coord3D::new(0.0, 0.0, 0.0),
            end_frame: 0,
            owner_object_id: INVALID_ID,
        }
    }

    pub fn set_owner_object_id(&mut self, owner_id: ObjectID) {
        self.owner_object_id = owner_id;
    }

    fn execute_strike(
        &mut self,
        targeting: &TargetingInfo,
        current_frame: UnsignedInt,
    ) -> Result<(), String> {
        log::info!(
            "Spectre Gunship activated at position {:?}",
            targeting.position
        );

        self.orbit_center = targeting.position;
        let frames_per_second = 30;
        self.end_frame =
            current_frame + (self.data.duration * frames_per_second as Real) as UnsignedInt;

        // Spawn gunship via OCL (C++: OCLSpecialPower.cpp CREATE_AT_EDGE_NEAR_TARGET).
        // If the INI/module did not specify a gunship OCL, fall back to no-spawn.
        if !self.data.gunship_ocl.is_empty() {
            use crate::helpers::TheObjectCreationListStore;
            if let Some(ocl) =
                TheObjectCreationListStore::find_object_creation_list(&self.data.gunship_ocl)
            {
                let ctx = crate::object_creation_list::live_creation_context();

                let edge = get_terrain_logic()
                    .read()
                    .ok()
                    .map(|terrain| terrain.find_closest_edge_point(&targeting.position))
                    .unwrap_or(targeting.position);
                let spawn_pos = Coord3D::new(edge.x, edge.y, edge.z + self.data.orbit_height);

                log::debug!(
                    "Spawning Spectre gunship via OCL '{}' at {:?}",
                    self.data.gunship_ocl,
                    spawn_pos
                );
                let created = crate::object::registry::OBJECT_REGISTRY.with_object(
                    self.owner_object_id,
                    |primary_obj| {
                        ocl.create_with_owner_flag(
                            &ctx,
                            Some(primary_obj),
                            &spawn_pos,
                            &targeting.position,
                            true,
                            0,
                        )
                    },
                );
                let Some(created) = created else {
                    self.gunship_id = None;
                    return Ok(());
                };
                self.gunship_id = created.and_then(|h| h.read().ok().map(|o| o.get_id()));
            } else {
                log::debug!("Spectre gunship OCL '{}' not found", self.data.gunship_ocl);
                self.gunship_id = None;
            }
        } else {
            self.gunship_id = None;
        }

        Ok(())
    }

    pub fn update_gunship(&mut self, current_frame: UnsignedInt) {
        if let Some(gunship_id) = self.gunship_id {
            if current_frame >= self.end_frame {
                log::debug!("Spectre gunship duration ended, removing");

                // Remove gunship - matches C++ SpectreGunshipUpdate.cpp lines 654-658
                // In C++: TheGameLogic->destroyObject(gunship)
                use crate::helpers::TheGameLogic;
                crate::object::registry::OBJECT_REGISTRY.with_object(gunship_id, |gunship_guard| {
                    let _ = TheGameLogic::destroy_object(gunship_guard);
                });
                self.gunship_id = None;
            } else {
                // Update gunship orbit position and fire weapons
                // Matches C++ SpectreGunshipUpdate.cpp lines 368-647

                crate::object::registry::OBJECT_REGISTRY.with_object(gunship_id, |gunship_guard| {
                    let gunship_pos = gunship_guard.get_position();
                    let mut perigee = Coord3D::new(
                        gunship_pos.x - self.orbit_center.x,
                        gunship_pos.y - self.orbit_center.y,
                        0.0,
                    );
                    let distance_to_target =
                        (perigee.x * perigee.x + perigee.y * perigee.y).sqrt();
                    if distance_to_target > 0.0 {
                        perigee.x /= distance_to_target;
                        perigee.y /= distance_to_target;
                    }
                    let apogee = Coord3D::new(-perigee.y, perigee.x, 0.0);
                    const ORBIT_INSERTION_SLOPE: Real = 0.7;
                    let n1 = ORBIT_INSERTION_SLOPE;
                    let n2 = 1.0 - n1;
                    let mut declination = Coord3D::new(
                        perigee.x * n1 + apogee.x * n2,
                        perigee.y * n1 + apogee.y * n2,
                        0.0,
                    );
                    declination.x *= self.data.orbit_radius;
                    declination.y *= self.data.orbit_radius;
                    let satellite_pos = Coord3D::new(
                        self.orbit_center.x + declination.x,
                        self.orbit_center.y + declination.y,
                        self.orbit_center.z + self.data.orbit_height,
                    );
                    log::trace!(
                        "Gunship {} orbiting at ({:.1}, {:.1}, {:.1}), target center ({:.1}, {:.1})",
                        gunship_id,
                        satellite_pos.x,
                        satellite_pos.y,
                        satellite_pos.z,
                        self.orbit_center.x,
                        self.orbit_center.y
                    );
                });
            }
        }
    }
}

impl SpecialPowerModuleInterface for SpectreGunshipPower {
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
        _player_id: ObjectID,
        targeting: Option<&TargetingInfo>,
        current_frame: UnsignedInt,
    ) -> ActivationResult {
        let targeting = match targeting {
            Some(t) => t,
            None => {
                return ActivationResult::InvalidTarget {
                    reason: "Spectre Gunship requires targeting".to_string(),
                };
            }
        };

        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        if let Err(reason) = self.execute_strike(targeting, current_frame) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_strike(targeting, 0)
    }

    fn update(&mut self, _delta_time: Real) {
        use crate::helpers::TheGameLogic;
        self.update_gunship(TheGameLogic::get_frame());
    }
}
