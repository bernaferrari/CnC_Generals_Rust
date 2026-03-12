//! OCL Special Power - Object Creation List
//!
//! Creates objects (units, effects, projectiles) at target location

use super::base_power::*;
use super::cooldown::CooldownState;
use super::targeting::TargetingInfo;
use super::types::*;
use crate::common::*;
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use crate::special_power_module::integration::get_integration_context;

/// OCL Special Power configuration data
#[derive(Debug, Clone)]
pub struct OCLSpecialPowerData {
    /// Base power data
    pub base: SpecialPowerModuleData,
    /// Object creation list name
    pub ocl_name: AsciiString,
    /// Upgrade OCLs based on science
    pub upgrade_ocl: Vec<(AsciiString, AsciiString)>,
    /// Number of objects to create
    pub create_count: Int,
    /// Spacing between created objects
    pub spacing: Real,
    /// Whether to create in formation
    pub use_formation: Bool,
    /// Delay between each object creation (in seconds)
    pub creation_delay: Real,
    /// Whether objects face target direction
    pub orient_to_target: Bool,
}

impl OCLSpecialPowerData {
    pub fn new(name: AsciiString, ocl_name: AsciiString) -> Self {
        let mut base = SpecialPowerModuleData::new(name, SpecialPowerKind::OCL);
        base.flags = SpecialPowerFlags::REQUIRES_TARGETING | SpecialPowerFlags::AFFECTS_ENEMY;
        let name_str = base.name.as_str();
        let mut upgrade_ocl = Vec::new();
        if name_str.eq_ignore_ascii_case("SuperweaponClusterMines")
            || name_str.eq_ignore_ascii_case("Nuke_SuperweaponNukeDrop")
        {
            base.recharge_time = 240.0; // 240000 ms
            base.radius = 100.0;
            base.flags |= SpecialPowerFlags::SUPERWEAPON;
        } else if name_str.eq_ignore_ascii_case("SuperweaponLeafletDrop")
            || name_str.eq_ignore_ascii_case("Early_SuperweaponLeafletDrop")
        {
            base.recharge_time = 300.0; // 300000 ms
            base.radius = 110.0;
            base.flags |= SpecialPowerFlags::SUPERWEAPON;
        } else if name_str.eq_ignore_ascii_case("SuperweaponCrateDrop") {
            base.recharge_time = 600.0; // 600000 ms
            base.radius = 100.0;
            base.flags =
                SpecialPowerFlags::REQUIRES_TARGETING | SpecialPowerFlags::AFFECTS_FRIENDLY;
        } else if name_str.eq_ignore_ascii_case("SuperweaponNapalmStrike") {
            base.recharge_time = 600.0; // 600000 ms
            base.radius = 100.0;
            base.flags |= SpecialPowerFlags::SUPERWEAPON;
        } else if name_str.eq_ignore_ascii_case("SuperweaponScudStorm") {
            base.recharge_time = 300.0; // 300000 ms
            base.radius = 200.0;
            base.flags |= SpecialPowerFlags::SUPERWEAPON;
        } else if name_str.eq_ignore_ascii_case("SuperweaponBlackMarketNuke") {
            base.recharge_time = 600.0; // 600000 ms
            base.radius = 100.0;
            base.flags |= SpecialPowerFlags::SUPERWEAPON;
        } else if name_str.eq_ignore_ascii_case("SuperweaponTerrorCell") {
            base.recharge_time = 600.0; // 600000 ms
            base.flags |= SpecialPowerFlags::SUPERWEAPON;
        } else if name_str.eq_ignore_ascii_case("SupW_CruiseMissile") {
            base.recharge_time = 120.0; // 120000 ms
            base.radius = 210.0;
            base.flags |= SpecialPowerFlags::SUPERWEAPON;
        } else if name_str.eq_ignore_ascii_case("SuperweaponFrenzy")
            || name_str.eq_ignore_ascii_case("Early_SuperweaponFrenzy")
        {
            base.recharge_time = 240.0; // 240000 ms
            base.radius = 200.0;
            base.flags = SpecialPowerFlags::REQUIRES_TARGETING
                | SpecialPowerFlags::AFFECTS_FRIENDLY
                | SpecialPowerFlags::SUPERWEAPON;
            if name_str.eq_ignore_ascii_case("Early_SuperweaponFrenzy") {
                upgrade_ocl.push(("Early_SCIENCE_Frenzy3".into(), "SUPERWEAPON_Frenzy3".into()));
                upgrade_ocl.push(("Early_SCIENCE_Frenzy2".into(), "SUPERWEAPON_Frenzy2".into()));
            } else {
                upgrade_ocl.push(("SCIENCE_Frenzy3".into(), "SUPERWEAPON_Frenzy3".into()));
                upgrade_ocl.push(("SCIENCE_Frenzy2".into(), "SUPERWEAPON_Frenzy2".into()));
            }
        } else if name_str.eq_ignore_ascii_case("SpecialPowerRadarVanScan") {
            base.recharge_time = 30.0; // 30000 ms
            base.radius = 150.0;
            base.flags = SpecialPowerFlags::REQUIRES_TARGETING
                | SpecialPowerFlags::AFFECTS_FRIENDLY
                | SpecialPowerFlags::RADAR_EFFECT
                | SpecialPowerFlags::SUPERWEAPON;
        }

        Self {
            base,
            ocl_name,
            upgrade_ocl,
            create_count: 1,
            spacing: 10.0,
            use_formation: false,
            creation_delay: 0.0,
            orient_to_target: true,
        }
    }
}

/// OCL Special Power implementation
pub struct OCLSpecialPower {
    data: OCLSpecialPowerData,
    cooldown: CooldownState,
    stats: SpecialPowerStats,
    created_objects: Vec<ObjectID>,
    creation_progress: Int,
    last_owner_id: Option<ObjectID>,
}

impl OCLSpecialPower {
    pub fn new(data: OCLSpecialPowerData) -> Self {
        let cooldown = CooldownState::new(data.base.recharge_time, data.base.init_charge_time);

        Self {
            data,
            cooldown,
            stats: SpecialPowerStats::new(),
            created_objects: Vec::new(),
            creation_progress: 0,
            last_owner_id: None,
        }
    }

    /// Create objects at target location
    fn create_objects(
        &mut self,
        owner_id: ObjectID,
        targeting: &TargetingInfo,
    ) -> Result<(), String> {
        self.created_objects.clear();
        self.creation_progress = 0;

        // Calculate positions for all objects
        let positions = self.calculate_spawn_positions(targeting);

        let integration = get_integration_context()
            .ok_or_else(|| "SpecialPower integration context not initialized".to_string())?;
        let ocl_system = integration
            .read()
            .ok()
            .and_then(|ctx| ctx.ocl_system.clone())
            .ok_or_else(|| {
                "OCL system not available in SpecialPower integration context".to_string()
            })?;

        let ocl_name = self
            .select_ocl_name(owner_id)
            .unwrap_or_else(|| self.data.ocl_name.clone());

        // Spawn OCL at each computed position.
        // Note: `create_ocl` may itself generate multiple objects; we still honor `create_count`
        // here to match the special-power spacing/formation behavior.
        for pos in positions.into_iter().take(self.data.create_count as usize) {
            let mut system = ocl_system
                .write()
                .map_err(|_| "Failed to lock OCL system".to_string())?;
            let created = system.create_ocl(
                ocl_name.as_str(),
                owner_id,
                &pos,
                &targeting.position,
                INVALID_ANGLE,
            )?;
            self.created_objects.extend(created);
            self.creation_progress += 1;
        }

        Ok(())
    }

    fn select_ocl_name(&self, owner_id: ObjectID) -> Option<AsciiString> {
        if !self.data.upgrade_ocl.is_empty() {
            if let Some(manager) = super::player_science::get_player_science_manager() {
                if let Ok(mgr) = manager.read() {
                    if let Some(player_science) = mgr.get_player(owner_id) {
                        for (science, ocl) in &self.data.upgrade_ocl {
                            if player_science.has_science(science.as_str()) {
                                return Some(ocl.clone());
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

    /// Calculate spawn positions based on configuration
    fn calculate_spawn_positions(&self, targeting: &TargetingInfo) -> Vec<Coord3D> {
        let mut positions = Vec::new();

        if self.data.use_formation {
            // Formation layout (line, circle, etc.)
            self.calculate_formation_positions(targeting, &mut positions);
        } else {
            // Simple spacing
            for i in 0..self.data.create_count {
                let offset = if self.data.create_count > 1 {
                    let total_width = (self.data.create_count - 1) as Real * self.data.spacing;
                    let x_offset = (i as Real * self.data.spacing) - (total_width / 2.0);
                    Coord3D::new(x_offset, 0.0, 0.0)
                } else {
                    Coord3D::new(0.0, 0.0, 0.0)
                };

                positions.push(targeting.position + offset);
            }
        }

        positions
    }

    /// Calculate formation positions
    fn calculate_formation_positions(
        &self,
        targeting: &TargetingInfo,
        positions: &mut Vec<Coord3D>,
    ) {
        // Circular formation
        let angle_step = (2.0 * std::f32::consts::PI) / self.data.create_count as Real;

        for i in 0..self.data.create_count {
            let angle = i as Real * angle_step;
            let offset = Coord3D::new(
                angle.cos() * self.data.spacing,
                angle.sin() * self.data.spacing,
                0.0,
            );
            positions.push(targeting.position + offset);
        }
    }

    /// Get list of created objects
    pub fn get_created_objects(&self) -> &[ObjectID] {
        &self.created_objects
    }
}

impl SpecialPowerModuleInterface for OCLSpecialPower {
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
                    reason: "OCL power requires targeting".to_string(),
                };
            }
        };

        // Check cooldown
        if self.is_on_cooldown() {
            return ActivationResult::OnCooldown {
                remaining: self.cooldown.time_remaining,
            };
        }

        self.last_owner_id = Some(player_id);

        // Execute power
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
        let owner_id = self
            .last_owner_id
            .ok_or_else(|| "OCL special power missing owner id".to_string())?;
        self.create_objects(owner_id, targeting)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::special_power_module::integration::{
        initialize_integration_context, set_ocl_system, ObjectCreationListInterface,
    };
    use std::sync::{Arc, RwLock};

    #[derive(Default)]
    struct TestOclSystem {
        next_id: ObjectID,
    }

    impl ObjectCreationListInterface for TestOclSystem {
        fn create_ocl(
            &mut self,
            _ocl_name: &str,
            _owner_id: ObjectID,
            _creation_pos: &Coord3D,
            _target_pos: &Coord3D,
            _angle: Real,
        ) -> Result<Vec<ObjectID>, String> {
            self.next_id += 1;
            Ok(vec![self.next_id])
        }
    }

    #[test]
    fn test_ocl_power_creation() {
        let data = OCLSpecialPowerData::new("A10Strike".into(), "OCL_A10Strike".into());
        let power = OCLSpecialPower::new(data);

        assert_eq!(power.get_name(), "A10Strike");
        assert!(power.is_ready());
    }

    #[test]
    fn test_ocl_activation() {
        initialize_integration_context();
        set_ocl_system(Arc::new(RwLock::new(TestOclSystem::default())));

        let mut data = OCLSpecialPowerData::new("A10Strike".into(), "OCL_A10Strike".into());
        data.base.recharge_time = 60.0;
        data.create_count = 3;

        let mut power = OCLSpecialPower::new(data);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 500.0, 50.0);

        let result = power.try_activate(1, Some(&targeting), 0);
        assert!(result.is_success());
        assert!(power.is_on_cooldown());
    }

    #[test]
    fn test_spawn_position_calculation() {
        let mut data = OCLSpecialPowerData::new("Test".into(), "OCL_Test".into());
        data.create_count = 5;
        data.spacing = 10.0;

        let power = OCLSpecialPower::new(data);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 500.0, 50.0);

        let positions = power.calculate_spawn_positions(&targeting);
        assert_eq!(positions.len(), 5);

        // Check positions are spaced correctly
        for i in 1..positions.len() {
            let distance = (positions[i] - positions[i - 1]).length();
            assert!((distance - 10.0).abs() < 0.1);
        }
    }
}
