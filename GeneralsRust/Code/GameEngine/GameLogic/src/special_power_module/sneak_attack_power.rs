//! Sneak Attack Special Power
//!
//! GLA special power that spawns rebel ambush units from tunnels near the target.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::special_power_module::integration::get_integration_context;

const SNEAK_ATTACK_UNIT_COUNT: Int = 6;

#[derive(Debug, Clone)]
pub struct SneakAttackPowerData {
    pub base: SpecialPowerModuleData,
    pub unit_count: Int,
    pub spawn_radius: Real,
    pub unit_ocl: AsciiString,
}

impl SneakAttackPowerData {
    pub fn new(name: AsciiString, unit_ocl: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::SneakAttack);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING | SpecialPowerFlags::AFFECTS_FRIENDLY;
        base.recharge_time = 150.0; // 2.5 minutes (150000 ms)
        base.cost = 0;
        base.range = 0.0;
        base.radius = 50.0;

        Self {
            base,
            unit_count: SNEAK_ATTACK_UNIT_COUNT,
            spawn_radius: 50.0,
            unit_ocl,
        }
    }
}

pub struct SneakAttackPower {
    data: SneakAttackPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    spawned_units: Vec<ObjectID>,
    last_owner_id: Option<ObjectID>,
}

impl SneakAttackPower {
    pub fn new(data: SneakAttackPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            spawned_units: Vec::new(),
            last_owner_id: None,
        }
    }

    fn execute_attack(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        log::info!(
            "Sneak Attack activated at position {:?}, spawning {} units",
            targeting.position,
            self.data.unit_count
        );

        self.spawned_units.clear();

        // Calculate spawn positions around target
        let spawn_positions = self.calculate_spawn_positions(targeting);

        let owner_id = self
            .last_owner_id
            .ok_or_else(|| "SneakAttackPower missing owner id".to_string())?;
        let integration = get_integration_context()
            .ok_or_else(|| "SpecialPower integration context not initialized".to_string())?;
        let ocl_system = integration
            .read()
            .ok()
            .and_then(|ctx| ctx.ocl_system.clone())
            .ok_or_else(|| {
                "OCL system not available in SpecialPower integration context".to_string()
            })?;

        // Spawn units from tunnels/underground via OCL.
        for (i, pos) in spawn_positions.iter().enumerate() {
            log::debug!("Spawning sneak attack unit {} at {:?}", i, pos);

            let mut system = ocl_system
                .write()
                .map_err(|_| "Failed to lock OCL system".to_string())?;
            let created = system.create_ocl(
                self.data.unit_ocl.as_str(),
                owner_id,
                pos,
                &targeting.position,
                0.0,
            )?;
            self.spawned_units.extend(created);
        }

        Ok(())
    }

    fn calculate_spawn_positions(&self, targeting: &TargetingInfo) -> Vec<Coord3D> {
        let mut positions = Vec::new();

        // Spawn in circle around target
        let angle_step = (2.0 * std::f32::consts::PI) / self.data.unit_count as Real;

        for i in 0..self.data.unit_count {
            let angle = i as Real * angle_step;
            positions.push(Coord3D::new(
                targeting.position.x + angle.cos() * self.data.spawn_radius,
                targeting.position.y + angle.sin() * self.data.spawn_radius,
                targeting.position.z,
            ));
        }

        positions
    }

    pub fn get_spawned_units(&self) -> &[ObjectID] {
        &self.spawned_units
    }
}

impl SpecialPowerModuleInterface for SneakAttackPower {
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
        let targeting = match targeting {
            Some(t) => t,
            None => {
                return ActivationResult::InvalidTarget {
                    reason: "Sneak Attack requires targeting".to_string(),
                };
            }
        };

        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        self.last_owner_id = Some(player_id);

        if let Err(reason) = self.execute(targeting) {
            return ActivationResult::Failed { reason };
        }

        self.cooldown.start_cooldown(current_frame);
        self.stats
            .record_activation(current_frame, self.data.base.cost);

        ActivationResult::Success
    }

    fn execute(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        self.execute_attack(targeting)
    }
}
