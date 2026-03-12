//! Rebel Ambush Special Power
//!
//! GLA special power that spawns a large number of rebels to ambush enemy forces.

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::special_power_module::integration::get_integration_context;

const REBEL_AMBUSH_COUNT: Int = 12;

#[derive(Debug, Clone)]
pub struct RebelAmbushPowerData {
    pub base: SpecialPowerModuleData,
    pub rebel_count: Int,
    pub spawn_radius: Real,
    pub rebel_ocl: AsciiString,
    pub upgrade_ocl: Vec<(AsciiString, AsciiString)>,
}

impl RebelAmbushPowerData {
    pub fn new(name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::Ambush);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING | SpecialPowerFlags::AFFECTS_FRIENDLY;
        base.recharge_time = 240.0; // 4 minutes (240000 ms)
        base.cost = 0;
        base.range = 0.0;
        base.radius = 50.0;

        let upgrade_ocl = vec![
            (
                "SCIENCE_RebelAmbush3".into(),
                "SUPERWEAPON_RebelAmbush3".into(),
            ),
            (
                "SCIENCE_RebelAmbush2".into(),
                "SUPERWEAPON_RebelAmbush2".into(),
            ),
            (
                "Chem_SCIENCE_RebelAmbush3".into(),
                "Chem_SUPERWEAPON_RebelAmbush3".into(),
            ),
            (
                "Chem_SCIENCE_RebelAmbush2".into(),
                "Chem_SUPERWEAPON_RebelAmbush2".into(),
            ),
            (
                "Demo_SCIENCE_RebelAmbush3".into(),
                "Demo_SUPERWEAPON_RebelAmbush3".into(),
            ),
            (
                "Demo_SCIENCE_RebelAmbush2".into(),
                "Demo_SUPERWEAPON_RebelAmbush2".into(),
            ),
            (
                "Slth_SCIENCE_RebelAmbush3".into(),
                "Slth_SUPERWEAPON_RebelAmbush3".into(),
            ),
            (
                "Slth_SCIENCE_RebelAmbush2".into(),
                "Slth_SUPERWEAPON_RebelAmbush2".into(),
            ),
        ];

        Self {
            base,
            rebel_count: REBEL_AMBUSH_COUNT,
            spawn_radius: 50.0,
            rebel_ocl: "OCL_RebelAmbush".into(),
            upgrade_ocl,
        }
    }
}

pub struct RebelAmbushPower {
    data: RebelAmbushPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    spawned_rebels: Vec<ObjectID>,
    last_owner_id: Option<ObjectID>,
}

impl RebelAmbushPower {
    pub fn new(data: RebelAmbushPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            spawned_rebels: Vec::new(),
            last_owner_id: None,
        }
    }

    fn execute_ambush(&mut self, targeting: &TargetingInfo) -> Result<(), String> {
        log::info!(
            "Rebel Ambush activated at position {:?}, spawning {} rebels",
            targeting.position,
            self.data.rebel_count
        );

        self.spawned_rebels.clear();

        // Calculate spawn positions
        let spawn_positions = self.calculate_spawn_positions(targeting);

        let owner_id = self
            .last_owner_id
            .ok_or_else(|| "RebelAmbushPower missing owner id".to_string())?;
        let integration = get_integration_context()
            .ok_or_else(|| "SpecialPower integration context not initialized".to_string())?;
        let ocl_system = integration
            .read()
            .ok()
            .and_then(|ctx| ctx.ocl_system.clone())
            .ok_or_else(|| {
                "OCL system not available in SpecialPower integration context".to_string()
            })?;

        let ocl_name = self.select_ocl_name();
        let ocl_name = ocl_name
            .as_ref()
            .map(|s| s.as_str())
            .unwrap_or(self.data.rebel_ocl.as_str());

        // Spawn rebels using OCL.
        for (i, pos) in spawn_positions.iter().enumerate() {
            log::debug!("Spawning rebel {} at {:?}", i, pos);

            let mut system = ocl_system
                .write()
                .map_err(|_| "Failed to lock OCL system".to_string())?;
            let created =
                system.create_ocl(ocl_name, owner_id, pos, &targeting.position, INVALID_ANGLE)?;
            self.spawned_rebels.extend(created);
        }

        Ok(())
    }

    fn calculate_spawn_positions(&self, targeting: &TargetingInfo) -> Vec<Coord3D> {
        let mut positions = Vec::new();

        // Spawn in scattered pattern around target
        let angle_step = (2.0 * std::f32::consts::PI) / self.data.rebel_count as Real;

        for i in 0..self.data.rebel_count {
            let angle = i as Real * angle_step + GameLogicRandomValueReal(-0.3, 0.3);
            let distance =
                GameLogicRandomValueReal(self.data.spawn_radius * 0.5, self.data.spawn_radius);

            positions.push(Coord3D::new(
                targeting.position.x + angle.cos() * distance,
                targeting.position.y + angle.sin() * distance,
                targeting.position.z,
            ));
        }

        positions
    }

    pub fn get_spawned_rebels(&self) -> &[ObjectID] {
        &self.spawned_rebels
    }

    fn select_ocl_name(&self) -> Option<AsciiString> {
        if !self.data.upgrade_ocl.is_empty() {
            if let Some(manager) = super::player_science::get_player_science_manager() {
                if let Ok(mgr) = manager.read() {
                    if let Some(player_id) = self.last_owner_id {
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
        if self.data.rebel_ocl.is_empty() {
            None
        } else {
            Some(self.data.rebel_ocl.clone())
        }
    }
}

impl SpecialPowerModuleInterface for RebelAmbushPower {
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
                    reason: "Rebel Ambush requires targeting".to_string(),
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
        self.execute_ambush(targeting)
    }
}
