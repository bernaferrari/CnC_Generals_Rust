//! Hive Structure Body Module - Propagates damage to slaves/contained units
//!
//! Hive structure bodies are structure bodies with the ability to propagate specified
//! damage types to slaves when available. If there are no slaves, then the structure
//! will take the damage (or swallow it if configured to do so).

use super::body_module::{
    ArmorSetType, BodyDamageType, BodyModuleInterface, BodyResult, DamageInfo, DamageInfoInput,
    MaxHealthChangeType, ObjectId, VeterancyLevel,
};
use super::structure_body::{StructureBody, StructureBodyModuleData};
use crate::common::{Coord3DExt, DamageTypeFlags, INVALID_ID};
use crate::damage::DamageType as CommonDamageType;
use crate::helpers::TheGameLogic;
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::Module;
use std::str::FromStr;
use std::sync::{Arc, RwLock};

/// Configuration data specific to hive structure bodies
#[derive(Debug, Clone)]
pub struct HiveStructureBodyModuleData {
    /// Base structure body module data
    pub base: StructureBodyModuleData,
    /// Damage types to propagate to slaves when they exist
    pub damage_types_to_propagate_to_slaves: DamageTypeFlags,
    /// Damage types to swallow (not take ourselves) if no slaves exist
    pub damage_types_to_swallow: DamageTypeFlags,
}

impl Default for HiveStructureBodyModuleData {
    fn default() -> Self {
        Self {
            base: StructureBodyModuleData::default(),
            damage_types_to_propagate_to_slaves: DamageTypeFlags::empty(),
            damage_types_to_swallow: DamageTypeFlags::empty(),
        }
    }
}

impl From<StructureBodyModuleData> for HiveStructureBodyModuleData {
    fn from(base: StructureBodyModuleData) -> Self {
        Self {
            base,
            damage_types_to_propagate_to_slaves: DamageTypeFlags::empty(),
            damage_types_to_swallow: DamageTypeFlags::empty(),
        }
    }
}

fn parse_damage_type_flags(tokens: &[&str]) -> Result<DamageTypeFlags, INIError> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut flags = DamageTypeFlags::empty();
    for token in tokens {
        for entry in token.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            if entry.eq_ignore_ascii_case("ALL") {
                flags = DamageTypeFlags::all_flags();
                continue;
            }
            if entry.eq_ignore_ascii_case("NONE") {
                flags = DamageTypeFlags::empty();
                continue;
            }

            let (remove, name) = if let Some(stripped) = entry.strip_prefix('-') {
                (true, stripped.trim())
            } else if let Some(stripped) = entry.strip_prefix('+') {
                (false, stripped.trim())
            } else {
                (false, entry)
            };

            if let Ok(damage_type) = CommonDamageType::from_str(name) {
                let flag = DamageTypeFlags::from_bits_truncate(1 << damage_type as u64);
                if remove {
                    flags.remove(flag);
                } else {
                    flags.insert(flag);
                }
            }
        }
    }

    Ok(flags)
}

fn parse_damage_types_to_propagate(
    _ini: &mut INI,
    data: &mut HiveStructureBodyModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_types_to_propagate_to_slaves = parse_damage_type_flags(tokens)?;
    Ok(())
}

fn parse_damage_types_to_swallow(
    _ini: &mut INI,
    data: &mut HiveStructureBodyModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.damage_types_to_swallow = parse_damage_type_flags(tokens)?;
    Ok(())
}

const HIVE_STRUCTURE_BODY_FIELDS: &[FieldParse<HiveStructureBodyModuleData>] = &[
    FieldParse {
        token: "PropagateDamageTypesToSlavesWhenExisting",
        parse: parse_damage_types_to_propagate,
    },
    FieldParse {
        token: "SwallowDamageTypesIfSlavesNotExisting",
        parse: parse_damage_types_to_swallow,
    },
];

impl HiveStructureBodyModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields(self, HIVE_STRUCTURE_BODY_FIELDS)
    }
}

crate::impl_legacy_module_data_via_base!(HiveStructureBodyModuleData, base);

impl Snapshotable for HiveStructureBodyModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)?;
        let mut propagate_bits = self.damage_types_to_propagate_to_slaves.bits();
        xfer.xfer_u64(&mut propagate_bits)
            .map_err(|e| e.to_string())?;
        let mut swallow_bits = self.damage_types_to_swallow.bits();
        xfer.xfer_u64(&mut swallow_bits)
            .map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.damage_types_to_propagate_to_slaves =
                DamageTypeFlags::from_bits_truncate(propagate_bits);
            self.damage_types_to_swallow = DamageTypeFlags::from_bits_truncate(swallow_bits);
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl Snapshotable for HiveStructureBody {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.structure_body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;
        self.structure_body.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.structure_body.load_post_process()
    }
}

/// Hive structure body implementation - propagates damage to slaves
pub struct HiveStructureBody {
    /// Base structure body functionality
    structure_body: StructureBody,
    /// Hive-specific configuration
    module_data: HiveStructureBodyModuleData,
}

impl HiveStructureBody {
    /// Create a new hive structure body
    pub fn new(module_data: HiveStructureBodyModuleData, owner_id: ObjectId) -> Self {
        let structure_body = StructureBody::new(module_data.base.clone(), owner_id);

        Self {
            structure_body,
            module_data,
        }
    }

    /// Get the structure body reference for delegated operations
    pub fn structure_body(&self) -> &StructureBody {
        &self.structure_body
    }

    /// Get mutable structure body reference for delegated operations
    pub fn structure_body_mut(&mut self) -> &mut StructureBody {
        &mut self.structure_body
    }

    /// Check if damage type should be propagated to slaves
    fn should_propagate_to_slaves(&self, damage_info: &DamageInfo) -> bool {
        self.module_data
            .damage_types_to_propagate_to_slaves
            .test_damage_type(damage_info.input.damage_type)
    }

    /// Check if damage type should be swallowed when no slaves exist
    fn should_swallow_if_no_slaves(&self, damage_info: &DamageInfo) -> bool {
        self.module_data
            .damage_types_to_swallow
            .test_damage_type(damage_info.input.damage_type)
    }

    /// Attempt to find and damage a slave.
    ///
    /// Mirrors the C++ propagation path:
    /// 1. Check SpawnBehaviorInterface and forward to the closest slave.
    /// 2. Otherwise, check ContainModuleInterface for riders.
    /// Returns true if a slave/rider was found and damaged, false otherwise.
    fn try_damage_slave(&mut self, damage_info: &mut DamageInfo) -> bool {
        let Some(owner) = self.structure_body.owner_handle() else {
            return false;
        };

        let Some(shooter) = TheGameLogic::find_object_by_id(damage_info.input.source_id) else {
            return false;
        };
        let shooter_pos = shooter.read().ok().map(|guard| *guard.get_position());
        let Some(shooter_pos) = shooter_pos else {
            return false;
        };

        let (closest_slave, contain) = match owner.read() {
            Ok(guard) => {
                let closest = guard
                    .with_spawn_behavior_full_interface(|spawn| {
                        spawn.get_closest_slave(&shooter_pos)
                    })
                    .flatten();
                (closest, guard.get_contain())
            }
            Err(_) => return false,
        };

        if let Some(slave) = closest_slave {
            if let Ok(mut slave_guard) = slave.write() {
                let _ = slave_guard.attempt_damage(damage_info);
            }
            return true;
        }

        if let Some(contain_handle) = contain {
            if let Ok(contain_guard) = contain_handle.lock() {
                let contained_ids: Vec<ObjectId> = contain_guard
                    .get_contained_objects()
                    .iter()
                    .copied()
                    .collect();
                drop(contain_guard);

                let mut closest: Option<Arc<RwLock<Object>>> = None;
                let mut closest_dist_sq = f32::INFINITY;

                for rider_id in contained_ids {
                    if let Some(rider) = TheGameLogic::find_object_by_id(rider_id) {
                        if let Ok(rider_guard) = rider.read() {
                            let rider_pos = *rider_guard.get_position();
                            let dist_sq = shooter_pos.distance_squared_to(&rider_pos);
                            if dist_sq < closest_dist_sq {
                                closest_dist_sq = dist_sq;
                                closest = Some(Arc::clone(&rider));
                            }
                        }
                    }
                }

                if let Some(rider) = closest {
                    if let Ok(mut rider_guard) = rider.write() {
                        let _ = rider_guard.attempt_damage(damage_info);
                    }
                    return true;
                }
            }
        } else {
            log::warn!(
                "HiveStructureBody missing SpawnBehavior or Contain module for object {}",
                owner
                    .read()
                    .map(|guard| guard.get_id())
                    .unwrap_or(INVALID_ID)
            );
        }

        false
    }
}

// Delegate most BodyModuleInterface methods to the underlying StructureBody
// The key override is attempt_damage to handle damage propagation
impl BodyModuleInterface for HiveStructureBody {
    fn attempt_damage(&mut self, damage_info: &mut DamageInfo) -> BodyResult<()> {
        // Check if this damage type should be propagated to slaves
        if self.should_propagate_to_slaves(damage_info) {
            // Try to find and damage a slave
            if self.try_damage_slave(damage_info) {
                // Successfully propagated to slave, we're done
                return Ok(());
            }

            // No slaves available - check if we should swallow the damage
            if self.should_swallow_if_no_slaves(damage_info)
                && TheGameLogic::find_object_by_id(damage_info.input.source_id).is_some()
            {
                // Swallow the damage - no effect (matches C++ HiveStructureBody.cpp:68-75/92-99)
                damage_info.output.actual_damage_dealt = 0.0;
                damage_info.output.actual_damage_clipped = 0.0;
                damage_info.output.no_effect = true;
                return Ok(());
            }
        }

        // Either not a propagated damage type, or no slaves to propagate to
        // and not a swallowed type, so damage ourselves normally
        self.structure_body.attempt_damage(damage_info)
    }

    fn attempt_healing(&mut self, healing_info: &mut DamageInfo) -> BodyResult<()> {
        self.structure_body.attempt_healing(healing_info)
    }

    fn estimate_damage(&self, damage_info: &DamageInfoInput) -> BodyResult<f32> {
        self.structure_body.estimate_damage(damage_info)
    }

    fn get_health(&self) -> f32 {
        self.structure_body.get_health()
    }

    fn get_max_health(&self) -> f32 {
        self.structure_body.get_max_health()
    }

    fn get_initial_health(&self) -> f32 {
        self.structure_body.get_initial_health()
    }

    fn get_previous_health(&self) -> f32 {
        self.structure_body.get_previous_health()
    }

    fn get_subdual_damage_heal_rate(&self) -> u32 {
        self.structure_body.get_subdual_damage_heal_rate()
    }

    fn get_subdual_damage_heal_amount(&self) -> f32 {
        self.structure_body.get_subdual_damage_heal_amount()
    }

    fn has_any_subdual_damage(&self) -> bool {
        self.structure_body.has_any_subdual_damage()
    }

    fn get_current_subdual_damage_amount(&self) -> f32 {
        self.structure_body.get_current_subdual_damage_amount()
    }

    fn get_damage_state(&self) -> BodyDamageType {
        self.structure_body.get_damage_state()
    }

    fn set_damage_state(&mut self, new_state: BodyDamageType) -> BodyResult<()> {
        self.structure_body.set_damage_state(new_state)
    }

    fn set_aflame(&mut self, setting: bool) -> BodyResult<()> {
        self.structure_body.set_aflame(setting)
    }

    fn on_veterancy_level_changed(
        &mut self,
        old_level: VeterancyLevel,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) -> BodyResult<()> {
        self.structure_body
            .on_veterancy_level_changed(old_level, new_level, provide_feedback)
    }

    fn set_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()> {
        self.structure_body.set_armor_set_flag(armor_type)
    }

    fn clear_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()> {
        self.structure_body.clear_armor_set_flag(armor_type)
    }

    fn test_armor_set_flag(&self, armor_type: ArmorSetType) -> bool {
        self.structure_body.test_armor_set_flag(armor_type)
    }

    fn get_last_damage_info(&self) -> Option<DamageInfo> {
        self.structure_body.get_last_damage_info()
    }

    fn get_last_damage_timestamp(&self) -> u32 {
        self.structure_body.get_last_damage_timestamp()
    }

    fn get_last_healing_timestamp(&self) -> u32 {
        self.structure_body.get_last_healing_timestamp()
    }

    fn get_clearable_last_attacker(&self) -> ObjectId {
        self.structure_body.get_clearable_last_attacker()
    }

    fn clear_last_attacker(&mut self) {
        self.structure_body.clear_last_attacker()
    }

    fn get_front_crushed(&self) -> bool {
        self.structure_body.get_front_crushed()
    }

    fn get_back_crushed(&self) -> bool {
        self.structure_body.get_back_crushed()
    }

    fn set_initial_health(&mut self, initial_percent: i32) -> BodyResult<()> {
        self.structure_body.set_initial_health(initial_percent)
    }

    fn set_max_health(
        &mut self,
        max_health: f32,
        change_type: MaxHealthChangeType,
    ) -> BodyResult<()> {
        self.structure_body.set_max_health(max_health, change_type)
    }

    fn set_front_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        self.structure_body.set_front_crushed(crushed)
    }

    fn set_back_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        self.structure_body.set_back_crushed(crushed)
    }

    fn apply_damage_scalar(&mut self, scalar: f32) -> BodyResult<()> {
        self.structure_body.apply_damage_scalar(scalar)
    }

    fn get_damage_scalar(&self) -> f32 {
        self.structure_body.get_damage_scalar()
    }

    fn internal_change_health(&mut self, delta: f32) -> BodyResult<()> {
        self.structure_body.internal_change_health(delta)
    }

    fn set_indestructible(&mut self, indestructible: bool) -> BodyResult<()> {
        self.structure_body.set_indestructible(indestructible)
    }

    fn is_indestructible(&self) -> bool {
        self.structure_body.is_indestructible()
    }

    fn evaluate_visual_condition(&mut self) -> BodyResult<()> {
        self.structure_body.evaluate_visual_condition()
    }

    fn update_body_particle_systems(&mut self) -> BodyResult<()> {
        self.structure_body.update_body_particle_systems()
    }
}

#[cfg(test)]
mod tests {
    use super::super::active_body::ActiveBodyModuleData;
    use super::*;
    use crate::object::body::body_module::DamageType;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use std::sync::{Arc, RwLock};

    fn make_damage_info(damage_type: DamageType, amount: f32) -> DamageInfo {
        let mut info = DamageInfo::new();
        info.input.damage_type = damage_type;
        info.input.amount = amount;
        info.input.source_id = 9000;
        info.sync_from_input();
        info
    }

    fn register_source_object() -> Arc<RwLock<Object>> {
        let source = Arc::new(RwLock::new(Object::new_test(9000, 100.0)));
        OBJECT_REGISTRY.register_object(9000, &source);
        source
    }

    fn create_test_hive_body() -> HiveStructureBody {
        let mut base_data = ActiveBodyModuleData::default();
        base_data.max_health = 500.0;
        base_data.initial_health = 500.0;

        let mut module_data = HiveStructureBodyModuleData {
            base: StructureBodyModuleData { base: base_data },
            damage_types_to_propagate_to_slaves: DamageTypeFlags::empty(),
            damage_types_to_swallow: DamageTypeFlags::empty(),
        };

        // Configure to propagate sniper damage
        module_data
            .damage_types_to_propagate_to_slaves
            .set_damage_type(DamageType::Sniper);

        // Configure to swallow sniper damage if no slaves
        module_data
            .damage_types_to_swallow
            .set_damage_type(DamageType::Sniper);

        HiveStructureBody::new(module_data, 0)
    }

    #[test]
    fn test_hive_structure_body_creation() {
        let body = create_test_hive_body();

        // Should behave like a structure body initially
        assert_eq!(body.get_health(), 500.0);
        assert_eq!(body.get_max_health(), 500.0);
        assert_eq!(body.get_initial_health(), 500.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
    }

    #[test]
    fn test_non_propagated_damage_works_normally() {
        let mut body = create_test_hive_body();

        // Normal damage type (not in propagate list)
        let mut damage_info = make_damage_info(DamageType::Explosion, 100.0);

        // Should damage the hive normally
        assert!(body.attempt_damage(&mut damage_info).is_ok());
        assert_eq!(body.get_health(), 400.0);
    }

    #[test]
    fn test_swallow_damage_when_no_slaves() {
        let _source = register_source_object();
        let mut body = create_test_hive_body();

        // Sniper damage should be swallowed (no slaves available in this test)
        let mut damage_info = make_damage_info(DamageType::Sniper, 200.0);

        // Should swallow the damage (no slaves, and it's in the swallow list)
        assert!(body.attempt_damage(&mut damage_info).is_ok());
        assert_eq!(damage_info.output.actual_damage_dealt, 0.0);
        assert_eq!(damage_info.output.actual_damage_clipped, 0.0);
        assert!(damage_info.output.no_effect);
        assert_eq!(body.get_health(), 500.0); // No damage taken
        OBJECT_REGISTRY.unregister_object(9000);
    }

    #[test]
    fn test_structure_body_functionality_preserved() {
        let mut body = create_test_hive_body();

        // Test that all structure body functionality works through delegation

        // Health operations
        assert!(body.internal_change_health(-100.0).is_ok());
        assert_eq!(body.get_health(), 400.0);
        assert_eq!(body.get_previous_health(), 500.0);

        // Max health operations
        assert!(body
            .set_max_health(600.0, MaxHealthChangeType::PreserveRatio)
            .is_ok());
        assert_eq!(body.get_max_health(), 600.0);
        assert_eq!(body.get_health(), 480.0); // Should preserve ratio (80%)

        // Armor flag operations
        assert!(!body.test_armor_set_flag(ArmorSetType::Veteran));
        assert!(body.set_armor_set_flag(ArmorSetType::Veteran).is_ok());
        assert!(body.test_armor_set_flag(ArmorSetType::Veteran));
    }

    #[test]
    fn test_propagate_logic_checks() {
        let body = create_test_hive_body();

        // Test that propagation checks work correctly
        let sniper_damage = make_damage_info(DamageType::Sniper, 100.0);

        assert!(body.should_propagate_to_slaves(&sniper_damage));
        assert!(body.should_swallow_if_no_slaves(&sniper_damage));

        let explosion_damage = make_damage_info(DamageType::Explosion, 100.0);

        assert!(!body.should_propagate_to_slaves(&explosion_damage));
        assert!(!body.should_swallow_if_no_slaves(&explosion_damage));
    }

    #[test]
    fn test_multiple_damage_types_configuration() {
        let _source = register_source_object();
        let mut base_data = ActiveBodyModuleData::default();
        base_data.max_health = 500.0;
        base_data.initial_health = 500.0;

        let mut module_data = HiveStructureBodyModuleData {
            base: StructureBodyModuleData { base: base_data },
            damage_types_to_propagate_to_slaves: DamageTypeFlags::empty(),
            damage_types_to_swallow: DamageTypeFlags::empty(),
        };

        // Configure multiple damage types
        module_data
            .damage_types_to_propagate_to_slaves
            .set_damage_type(DamageType::Sniper);
        module_data
            .damage_types_to_propagate_to_slaves
            .set_damage_type(DamageType::SmallArms);

        // Only swallow sniper
        module_data
            .damage_types_to_swallow
            .set_damage_type(DamageType::Sniper);

        let mut body = HiveStructureBody::new(module_data, 0);

        // Sniper should be swallowed
        let mut sniper_damage = make_damage_info(DamageType::Sniper, 100.0);

        assert!(body.attempt_damage(&mut sniper_damage).is_ok());
        assert_eq!(sniper_damage.output.actual_damage_dealt, 0.0);
        assert_eq!(body.get_health(), 500.0);

        // SmallArms should propagate but since no slaves, it should damage the hive
        // (it's not in the swallow list)
        let mut small_arms_damage = make_damage_info(DamageType::SmallArms, 100.0);

        assert!(body.attempt_damage(&mut small_arms_damage).is_ok());
        // Note: actual damage will be affected by armor, so we can't predict exact value
        // Just verify health decreased
        assert!(body.get_health() < 500.0);
        OBJECT_REGISTRY.unregister_object(9000);
    }
}
