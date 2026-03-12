//! Undead Body Module - Bodies that can be revived after first death
//!
//! First death is intercepted and triggers a "second life" state with reduced health.
//! The body sets special armor flags and can trigger slow death animations.
//! Second death is handled normally.

use std::any::Any;
use std::sync::{Arc, RwLock};

use super::active_body::{ActiveBody, ActiveBodyModuleData};
use super::body_module::{
    ArmorSetType, BodyDamageType, BodyError, BodyModuleInterface, BodyResult, DamageInfo,
    DamageInfoInput, DamageType, MaxHealthChangeType, ObjectId, VeterancyLevel,
};
use crate::helpers::get_game_logic_random_value;
use crate::modules::SlowDeathBehaviorInterface;
use crate::object::behavior::battle_bus_slow_death_behavior::BattleBusSlowDeathBehaviorModule;
use crate::object::behavior::neutron_missile_slow_death_update::NeutronMissileSlowDeathUpdate;
use crate::object::behavior::slow_death_behavior::SlowDeathBehavior;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::Module;

/// Configuration data specific to undead bodies
#[derive(Debug, Clone)]
pub struct UndeadBodyModuleData {
    /// Base active body module data
    pub base: ActiveBodyModuleData,
    /// Maximum health for the second life
    pub second_life_max_health: f32,
}

impl Default for UndeadBodyModuleData {
    fn default() -> Self {
        Self {
            base: ActiveBodyModuleData::default(),
            second_life_max_health: 1.0,
        }
    }
}

impl From<ActiveBodyModuleData> for UndeadBodyModuleData {
    fn from(base: ActiveBodyModuleData) -> Self {
        Self {
            base,
            second_life_max_health: 1.0,
        }
    }
}

fn parse_second_life_max_health(
    _ini: &mut INI,
    data: &mut UndeadBodyModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.second_life_max_health = INI::parse_real(token)?;
    Ok(())
}

const UNDEAD_BODY_FIELDS: &[FieldParse<UndeadBodyModuleData>] = &[FieldParse {
    token: "SecondLifeMaxHealth",
    parse: parse_second_life_max_health,
}];

impl UndeadBodyModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields(self, UNDEAD_BODY_FIELDS)
    }
}

crate::impl_legacy_module_data_via_base!(UndeadBodyModuleData, base);

impl Snapshotable for UndeadBodyModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)?;
        xfer.xfer_real(&mut self.second_life_max_health)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

impl Snapshotable for UndeadBody {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.active_body.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        self.active_body.xfer(xfer)?;

        let mut state = self
            .state
            .write()
            .map_err(|_| "UndeadBody state lock poisoned".to_string())?;
        xfer.xfer_bool(&mut state.is_second_life)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.active_body.load_post_process()
    }
}

/// Thread-safe state specific to undead bodies
#[derive(Debug, Default)]
struct UndeadBodyState {
    /// Whether we're in our second life (after first death)
    is_second_life: bool,
}

/// Undead body implementation - revivable units
pub struct UndeadBody {
    /// Base active body functionality
    active_body: ActiveBody,
    /// Undead-specific configuration
    module_data: Arc<UndeadBodyModuleData>,
    /// Thread-safe mutable state
    state: Arc<RwLock<UndeadBodyState>>,
}

impl UndeadBody {
    /// Create a new undead body
    pub fn new(module_data: UndeadBodyModuleData, owner_id: ObjectId) -> Self {
        let active_body = ActiveBody::new_with_owner(module_data.base.clone(), owner_id);
        let state = Arc::new(RwLock::new(UndeadBodyState {
            is_second_life: false,
        }));

        Self {
            active_body,
            module_data: Arc::new(module_data),
            state,
        }
    }

    /// Check if this body is in its second life
    pub fn is_second_life(&self) -> bool {
        self.state
            .read()
            .map(|state| state.is_second_life)
            .unwrap_or(false)
    }

    /// Start the second life after first death
    fn start_second_life(&mut self, damage_info: &DamageInfo) -> BodyResult<()> {
        // Mark as second life
        if let Ok(mut state) = self.state.write() {
            state.is_second_life = true;
        }

        // Set max health to second life value and fully heal
        self.active_body.set_max_health(
            self.module_data.second_life_max_health,
            MaxHealthChangeType::FullyHeal,
        )?;

        // Set armor set flag for second life
        self.active_body
            .set_armor_set_flag(ArmorSetType::SecondLife)?;

        let owner = match self.active_body.owner_handle() {
            Some(owner) => owner,
            None => return Ok(()),
        };

        let behavior_modules = match owner.try_read() {
            Ok(guard) => guard.behavior_modules(),
            Err(_) => return Ok(()),
        };

        let mut total_probability: i32 = 0;
        for module in &behavior_modules {
            module.with_module(|module| {
                let _ = with_slow_death_interface(module, |slow_death| {
                    if slow_death.is_die_applicable(damage_info) {
                        total_probability += slow_death.get_probability_modifier(damage_info);
                    }
                });
            });
        }

        debug_assert!(
            total_probability > 0,
            "UndeadBody: no SlowDeathBehavior candidates for second life"
        );

        if total_probability <= 0 {
            return Ok(());
        }

        let mut roll = get_game_logic_random_value(1, total_probability);
        for module in behavior_modules {
            let mut selected = false;
            let mut result: Result<(), Box<dyn std::error::Error + Send + Sync>> = Ok(());
            module.with_module(|module| {
                let _ = with_slow_death_interface(module, |slow_death| {
                    if slow_death.is_die_applicable(damage_info) {
                        roll -= slow_death.get_probability_modifier(damage_info);
                        if roll <= 0 {
                            selected = true;
                            result = slow_death.begin_slow_death(damage_info);
                        }
                    }
                });
            });
            if selected {
                result.map_err(|_| BodyError::OperationNotSupported)?;
                break;
            }
        }

        Ok(())
    }

    /// Get the active body reference for delegated operations
    pub fn active_body(&self) -> &ActiveBody {
        &self.active_body
    }

    /// Get mutable active body reference for delegated operations
    pub fn active_body_mut(&mut self) -> &mut ActiveBody {
        &mut self.active_body
    }

    /// Check if damage is health-affecting damage
    fn is_health_damaging_damage(damage_type: DamageType) -> bool {
        // Most damage types affect health, except for special types
        !matches!(
            damage_type,
            DamageType::Status
                | DamageType::Deploy
                | DamageType::Surrender
                | DamageType::Hack
                | DamageType::KillPilot
                | DamageType::KillGarrisoned
                | DamageType::Disarm
        )
    }
}

enum SlowDeathModuleKindMut<'a> {
    SlowDeath(&'a mut SlowDeathBehavior),
    Neutron(&'a mut NeutronMissileSlowDeathUpdate),
    BattleBus(&'a mut BattleBusSlowDeathBehaviorModule),
}

impl<'a> SlowDeathModuleKindMut<'a> {
    fn into_interface(self) -> &'a mut dyn SlowDeathBehaviorInterface {
        match self {
            Self::SlowDeath(module) => module,
            Self::Neutron(module) => module,
            Self::BattleBus(module) => module.behavior_mut(),
        }
    }
}

fn slow_death_module_kind(module: &mut dyn Module) -> Option<SlowDeathModuleKindMut<'_>> {
    if module.as_any().is::<SlowDeathBehavior>() {
        let module = (module as &mut dyn Any)
            .downcast_mut::<SlowDeathBehavior>()
            .expect("type check and downcast must match");
        return Some(SlowDeathModuleKindMut::SlowDeath(module));
    }
    if module.as_any().is::<NeutronMissileSlowDeathUpdate>() {
        let module = (module as &mut dyn Any)
            .downcast_mut::<NeutronMissileSlowDeathUpdate>()
            .expect("type check and downcast must match");
        return Some(SlowDeathModuleKindMut::Neutron(module));
    }
    if module.as_any().is::<BattleBusSlowDeathBehaviorModule>() {
        let module = (module as &mut dyn Any)
            .downcast_mut::<BattleBusSlowDeathBehaviorModule>()
            .expect("type check and downcast must match");
        return Some(SlowDeathModuleKindMut::BattleBus(module));
    }
    None
}

fn with_slow_death_interface<R>(
    module: &mut dyn Module,
    f: impl FnOnce(&mut dyn SlowDeathBehaviorInterface) -> R,
) -> Option<R> {
    slow_death_module_kind(module).map(|kind| f(kind.into_interface()))
}

// Delegate most BodyModuleInterface methods to the underlying ActiveBody
// The key override is attempt_damage to intercept the first death
impl BodyModuleInterface for UndeadBody {
    fn attempt_damage(&mut self, damage_info: &mut DamageInfo) -> BodyResult<()> {
        // Check if we should start second life
        // This happens when:
        // 1. We're on our first life (not second life yet)
        // 2. The damage is not unresistable
        // 3. The damage would kill us (amount >= current health)
        // 4. The damage is health-damaging damage
        let should_start_second_life = if !self.is_second_life()
            && damage_info.input.damage_type != DamageType::Unresistable
            && Self::is_health_damaging_damage(damage_info.input.damage_type)
        {
            let current_health = self.get_health();
            damage_info.input.amount >= current_health
        } else {
            false
        };

        // If we should start second life, limit damage to leave 1 health
        if should_start_second_life {
            let current_health = self.get_health();
            damage_info.input.amount = damage_info.input.amount.min(current_health - 1.0);
        }

        // Apply the damage
        self.active_body.attempt_damage(damage_info)?;

        // After applying damage, start second life if needed
        if should_start_second_life {
            self.start_second_life(damage_info)?;
        }

        Ok(())
    }

    fn attempt_healing(&mut self, healing_info: &mut DamageInfo) -> BodyResult<()> {
        self.active_body.attempt_healing(healing_info)
    }

    fn estimate_damage(&self, damage_info: &DamageInfoInput) -> BodyResult<f32> {
        let estimated = self.active_body.estimate_damage(damage_info)?;

        // If on first life and damage would kill, limit to current health - 1
        if !self.is_second_life()
            && damage_info.damage_type != DamageType::Unresistable
            && Self::is_health_damaging_damage(damage_info.damage_type)
        {
            let current_health = self.get_health();
            if estimated >= current_health {
                return Ok(estimated.min(current_health - 1.0));
            }
        }

        Ok(estimated)
    }

    fn get_health(&self) -> f32 {
        self.active_body.get_health()
    }

    fn get_max_health(&self) -> f32 {
        self.active_body.get_max_health()
    }

    fn get_initial_health(&self) -> f32 {
        self.active_body.get_initial_health()
    }

    fn get_previous_health(&self) -> f32 {
        self.active_body.get_previous_health()
    }

    fn get_subdual_damage_heal_rate(&self) -> u32 {
        self.active_body.get_subdual_damage_heal_rate()
    }

    fn get_subdual_damage_heal_amount(&self) -> f32 {
        self.active_body.get_subdual_damage_heal_amount()
    }

    fn has_any_subdual_damage(&self) -> bool {
        self.active_body.has_any_subdual_damage()
    }

    fn get_current_subdual_damage_amount(&self) -> f32 {
        self.active_body.get_current_subdual_damage_amount()
    }

    fn get_damage_state(&self) -> BodyDamageType {
        self.active_body.get_damage_state()
    }

    fn set_damage_state(&mut self, new_state: BodyDamageType) -> BodyResult<()> {
        self.active_body.set_damage_state(new_state)
    }

    fn set_aflame(&mut self, setting: bool) -> BodyResult<()> {
        self.active_body.set_aflame(setting)
    }

    fn on_veterancy_level_changed(
        &mut self,
        old_level: VeterancyLevel,
        new_level: VeterancyLevel,
        provide_feedback: bool,
    ) -> BodyResult<()> {
        self.active_body
            .on_veterancy_level_changed(old_level, new_level, provide_feedback)
    }

    fn set_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()> {
        self.active_body.set_armor_set_flag(armor_type)
    }

    fn clear_armor_set_flag(&mut self, armor_type: ArmorSetType) -> BodyResult<()> {
        self.active_body.clear_armor_set_flag(armor_type)
    }

    fn test_armor_set_flag(&self, armor_type: ArmorSetType) -> bool {
        self.active_body.test_armor_set_flag(armor_type)
    }

    fn get_last_damage_info(&self) -> Option<DamageInfo> {
        self.active_body.get_last_damage_info()
    }

    fn get_last_damage_timestamp(&self) -> u32 {
        self.active_body.get_last_damage_timestamp()
    }

    fn get_last_healing_timestamp(&self) -> u32 {
        self.active_body.get_last_healing_timestamp()
    }

    fn get_clearable_last_attacker(&self) -> ObjectId {
        self.active_body.get_clearable_last_attacker()
    }

    fn clear_last_attacker(&mut self) {
        self.active_body.clear_last_attacker()
    }

    fn get_front_crushed(&self) -> bool {
        self.active_body.get_front_crushed()
    }

    fn get_back_crushed(&self) -> bool {
        self.active_body.get_back_crushed()
    }

    fn set_initial_health(&mut self, initial_percent: i32) -> BodyResult<()> {
        self.active_body.set_initial_health(initial_percent)
    }

    fn set_max_health(
        &mut self,
        max_health: f32,
        change_type: MaxHealthChangeType,
    ) -> BodyResult<()> {
        self.active_body.set_max_health(max_health, change_type)
    }

    fn set_front_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        self.active_body.set_front_crushed(crushed)
    }

    fn set_back_crushed(&mut self, crushed: bool) -> BodyResult<()> {
        self.active_body.set_back_crushed(crushed)
    }

    fn apply_damage_scalar(&mut self, scalar: f32) -> BodyResult<()> {
        self.active_body.apply_damage_scalar(scalar)
    }

    fn get_damage_scalar(&self) -> f32 {
        self.active_body.get_damage_scalar()
    }

    fn internal_change_health(&mut self, delta: f32) -> BodyResult<()> {
        self.active_body.internal_change_health(delta)
    }

    fn set_indestructible(&mut self, indestructible: bool) -> BodyResult<()> {
        self.active_body.set_indestructible(indestructible)
    }

    fn is_indestructible(&self) -> bool {
        self.active_body.is_indestructible()
    }

    fn evaluate_visual_condition(&mut self) -> BodyResult<()> {
        self.active_body.evaluate_visual_condition()
    }

    fn update_body_particle_systems(&mut self) -> BodyResult<()> {
        self.active_body.update_body_particle_systems()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_undead_body() -> UndeadBody {
        let mut module_data = UndeadBodyModuleData::default();
        module_data.base.max_health = 100.0;
        module_data.base.initial_health = 100.0;
        module_data.second_life_max_health = 25.0;

        UndeadBody::new(module_data, 1)
    }

    fn make_damage_info(damage_type: DamageType, amount: f32) -> DamageInfo {
        let mut info = DamageInfo::new();
        info.input.damage_type = damage_type;
        info.input.amount = amount;
        info.sync_from_input();
        info
    }

    #[test]
    fn test_undead_body_creation() {
        let body = create_test_undead_body();

        assert_eq!(body.get_health(), 100.0);
        assert_eq!(body.get_max_health(), 100.0);
        assert_eq!(body.get_initial_health(), 100.0);
        assert_eq!(body.get_damage_state(), BodyDamageType::Pristine);
        assert!(!body.is_second_life());
    }

    #[test]
    fn test_first_death_triggers_second_life() {
        let mut body = create_test_undead_body();

        // Apply lethal damage
        let mut damage_info = make_damage_info(DamageType::Explosion, 150.0);

        assert!(body.attempt_damage(&mut damage_info).is_ok());

        // Should now be in second life
        assert!(body.is_second_life());

        // Should have second life max health
        assert_eq!(body.get_max_health(), 25.0);
        assert_eq!(body.get_health(), 25.0);

        // Should have SecondLife armor flag set
        assert!(body.test_armor_set_flag(ArmorSetType::SecondLife));
    }

    #[test]
    fn test_normal_damage_before_first_death() {
        let mut body = create_test_undead_body();

        // Apply non-lethal damage
        let mut damage_info = make_damage_info(DamageType::SmallArms, 30.0);

        assert!(body.attempt_damage(&mut damage_info).is_ok());
        assert_eq!(body.get_health(), 70.0);
        assert!(!body.is_second_life());

        // Apply more non-lethal damage
        let mut damage_info2 = make_damage_info(DamageType::SmallArms, 30.0);

        assert!(body.attempt_damage(&mut damage_info2).is_ok());
        assert_eq!(body.get_health(), 40.0);
        assert!(!body.is_second_life());
    }

    #[test]
    fn test_unresistable_damage_kills_immediately() {
        let mut body = create_test_undead_body();

        // Unresistable damage should kill without triggering second life
        let mut damage_info = make_damage_info(DamageType::Unresistable, 150.0);

        assert!(body.attempt_damage(&mut damage_info).is_ok());

        // Should be dead, not in second life
        assert_eq!(body.get_health(), 0.0);
        assert!(!body.is_second_life());
    }

    #[test]
    fn test_second_life_can_die_normally() {
        let mut body = create_test_undead_body();

        // Trigger first death and second life
        let mut first_death = make_damage_info(DamageType::Explosion, 100.0);

        assert!(body.attempt_damage(&mut first_death).is_ok());
        assert!(body.is_second_life());
        assert_eq!(body.get_health(), 25.0);

        // Now apply normal lethal damage - should kill
        let mut second_death = make_damage_info(DamageType::Explosion, 30.0);

        assert!(body.attempt_damage(&mut second_death).is_ok());

        // Should be dead now
        assert_eq!(body.get_health(), 0.0);
        assert!(body.is_second_life()); // Still marked as second life
    }

    #[test]
    fn test_healing_works_normally() {
        let mut body = create_test_undead_body();

        // Damage first
        let mut damage_info = make_damage_info(DamageType::SmallArms, 50.0);

        assert!(body.attempt_damage(&mut damage_info).is_ok());
        assert_eq!(body.get_health(), 50.0);

        // Heal
        let mut healing_info = make_damage_info(DamageType::Healing, 25.0);

        assert!(body.attempt_healing(&mut healing_info).is_ok());
        assert_eq!(body.get_health(), 75.0);
        assert!(!body.is_second_life());
    }

    #[test]
    fn test_damage_estimation() {
        let body = create_test_undead_body();

        // Normal damage should be estimated normally
        let normal_damage = DamageInfoInput {
            damage_type: DamageType::SmallArms,
            amount: 50.0,
            ..Default::default()
        };

        let estimated = body.estimate_damage(&normal_damage).unwrap();
        assert_eq!(estimated, 50.0);

        // Lethal damage should be limited in estimation
        let lethal_damage = DamageInfoInput {
            damage_type: DamageType::Explosion,
            amount: 150.0,
            ..Default::default()
        };

        let estimated_lethal = body.estimate_damage(&lethal_damage).unwrap();
        assert_eq!(estimated_lethal, 99.0); // Current health (100) - 1
    }

    #[test]
    fn test_status_damage_doesnt_trigger_second_life() {
        let mut body = create_test_undead_body();

        // Status damage shouldn't trigger second life even if large
        let mut status_damage = make_damage_info(DamageType::Status, 200.0);

        assert!(body.attempt_damage(&mut status_damage).is_ok());

        // Should not trigger second life (Status is not health-damaging)
        assert!(!body.is_second_life());
    }

    #[test]
    fn test_multiple_non_lethal_then_lethal() {
        let mut body = create_test_undead_body();

        // Apply multiple non-lethal damages
        for _ in 0..3 {
            let mut damage = make_damage_info(DamageType::SmallArms, 25.0);
            assert!(body.attempt_damage(&mut damage).is_ok());
        }

        assert_eq!(body.get_health(), 25.0);
        assert!(!body.is_second_life());

        // Now apply lethal damage
        let mut lethal = make_damage_info(DamageType::Explosion, 50.0);

        assert!(body.attempt_damage(&mut lethal).is_ok());

        // Should trigger second life
        assert!(body.is_second_life());
        assert_eq!(body.get_max_health(), 25.0);
        assert_eq!(body.get_health(), 25.0);
    }

    #[test]
    fn test_healing_in_second_life() {
        let mut body = create_test_undead_body();

        // Trigger second life
        let mut first_death = make_damage_info(DamageType::Explosion, 100.0);

        assert!(body.attempt_damage(&mut first_death).is_ok());
        assert!(body.is_second_life());
        assert_eq!(body.get_health(), 25.0);

        // Damage in second life
        let mut damage = make_damage_info(DamageType::SmallArms, 10.0);

        assert!(body.attempt_damage(&mut damage).is_ok());
        assert_eq!(body.get_health(), 15.0);

        // Heal in second life
        let mut healing = make_damage_info(DamageType::Healing, 5.0);

        assert!(body.attempt_healing(&mut healing).is_ok());
        assert_eq!(body.get_health(), 20.0);

        // Should still be capped at second life max
        let mut overheal = make_damage_info(DamageType::Healing, 100.0);

        assert!(body.attempt_healing(&mut overheal).is_ok());
        assert_eq!(body.get_health(), 25.0); // Capped at second life max
    }
}
