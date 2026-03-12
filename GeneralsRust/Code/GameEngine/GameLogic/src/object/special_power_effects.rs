// FILE: special_power_effects.rs
// Port of Special Power effects system
// Author: Rust Port
// Desc: Special power effect application (damage, spawning, buffs/debuffs)

use crate::common::Coord3D;
use crate::common::KindOf;
use crate::helpers::TheGameLogic;
use crate::object::special_power_module::ObjectId;
use crate::object::special_power_types::SpecialPowerType;
use crate::player::player_list;
use serde::{Deserialize, Serialize};

/// Damage type for special power effects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DamageType {
    /// Explosive damage (reduced by armor)
    Explosive,
    /// Fire damage (damage over time)
    Fire,
    /// Radiation damage (ignores armor, affects over time)
    Radiation,
    /// EMP damage (disables electronics)
    Emp,
    /// Chemical/Anthrax damage
    Chemical,
    /// Direct damage (no reduction)
    Direct,
}

/// Status effect type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StatusEffectType {
    /// Unit is on fire
    Burning,
    /// Unit is disabled (EMP)
    Disabled,
    /// Unit is slowed
    Slowed,
    /// Unit is afraid/demoralized
    Demoralized,
    /// Unit has enhanced vision
    EnhancedVision,
    /// Unit is poisoned (anthrax)
    Poisoned,
    /// Unit is regenerating health
    Regenerating,
    /// Unit has bonus damage
    DamageBoost,
    /// Unit has bonus armor
    ArmorBoost,
}

/// Buff/debuff application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEffect {
    /// Effect type
    pub effect_type: StatusEffectType,
    /// Duration in frames
    pub duration: u32,
    /// Strength/magnitude (damage per tick, speed modifier, etc.)
    pub strength: f32,
    /// Frame when applied
    pub applied_frame: u32,
}

impl StatusEffect {
    /// Create a new status effect
    pub fn new(
        effect_type: StatusEffectType,
        duration: u32,
        strength: f32,
        applied_frame: u32,
    ) -> Self {
        Self {
            effect_type,
            duration,
            strength,
            applied_frame,
        }
    }

    /// Check if effect is still active
    pub fn is_active(&self, current_frame: u32) -> bool {
        current_frame < self.applied_frame + self.duration
    }

    /// Get remaining duration in frames
    pub fn remaining_duration(&self, current_frame: u32) -> u32 {
        if self.is_active(current_frame) {
            (self.applied_frame + self.duration).saturating_sub(current_frame)
        } else {
            0
        }
    }
}

/// Damage application result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageResult {
    /// Amount of damage dealt
    pub damage_dealt: f32,
    /// Was target killed
    pub target_killed: bool,
    /// Was target structure destroyed
    pub structure_destroyed: bool,
}

/// Object spawn request for special powers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnRequest {
    /// Object type to spawn (template name)
    pub object_template: String,
    /// Location to spawn at
    pub location: Coord3D,
    /// Owner player ID
    pub owner_player_id: u32,
    /// Initial health percentage (1.0 = full health)
    pub health_percentage: f32,
    /// Is this a temporary object (auto-delete after time)
    pub temporary: bool,
    /// Lifetime in frames (if temporary)
    pub lifetime: u32,
}

impl SpawnRequest {
    /// Create a spawn request for a unit
    pub fn new_unit(template: impl Into<String>, location: Coord3D, owner: u32) -> Self {
        Self {
            object_template: template.into(),
            location,
            owner_player_id: owner,
            health_percentage: 1.0,
            temporary: false,
            lifetime: 0,
        }
    }

    /// Create a temporary spawn request (like view objects)
    pub fn new_temporary(
        template: impl Into<String>,
        location: Coord3D,
        owner: u32,
        lifetime: u32,
    ) -> Self {
        Self {
            object_template: template.into(),
            location,
            owner_player_id: owner,
            health_percentage: 1.0,
            temporary: true,
            lifetime,
        }
    }
}

/// Special power effect definition
/// Matches C++ behavior from various special power update modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialPowerEffect {
    /// Power type this effect is for
    pub power_type: SpecialPowerType,

    /// Damage configuration
    pub damage: Option<DamageConfig>,

    /// Status effects to apply
    pub status_effects: Vec<StatusEffectConfig>,

    /// Objects to spawn
    pub spawn_objects: Vec<SpawnConfig>,

    /// Area of effect radius (0 = single target)
    pub area_radius: f32,

    /// Maximum targets affected (0 = unlimited)
    pub max_targets: u32,

    /// Only affect enemy units
    pub enemy_only: bool,

    /// Only affect friendly units
    pub friendly_only: bool,

    /// Only affect buildings
    pub buildings_only: bool,

    /// Only affect vehicles
    pub vehicles_only: bool,

    /// Only affect infantry
    pub infantry_only: bool,
}

/// Damage configuration for a power effect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageConfig {
    /// Base damage amount
    pub amount: f32,
    /// Damage type
    pub damage_type: DamageType,
    /// Damage falloff (percentage at edge of radius, 1.0 = no falloff)
    pub falloff: f32,
}

/// Status effect configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusEffectConfig {
    /// Effect type
    pub effect_type: StatusEffectType,
    /// Duration in frames
    pub duration: u32,
    /// Effect strength
    pub strength: f32,
}

/// Spawn configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnConfig {
    /// Object template name
    pub template: String,
    /// Number to spawn
    pub count: u32,
    /// Spawn radius (random within this radius)
    pub radius: f32,
    /// Is temporary
    pub temporary: bool,
    /// Lifetime if temporary
    pub lifetime: u32,
}

impl Default for SpecialPowerEffect {
    fn default() -> Self {
        Self {
            power_type: SpecialPowerType::Invalid,
            damage: None,
            status_effects: Vec::new(),
            spawn_objects: Vec::new(),
            area_radius: 0.0,
            max_targets: 0,
            enemy_only: false,
            friendly_only: false,
            buildings_only: false,
            vehicles_only: false,
            infantry_only: false,
        }
    }
}

impl SpecialPowerEffect {
    /// Create a damage effect (like A10 Strike, Artillery Barrage)
    pub fn damage_effect(
        power_type: SpecialPowerType,
        amount: f32,
        damage_type: DamageType,
        radius: f32,
        falloff: f32,
    ) -> Self {
        Self {
            power_type,
            damage: Some(DamageConfig {
                amount,
                damage_type,
                falloff,
            }),
            area_radius: radius,
            ..Default::default()
        }
    }

    /// Create a spawn effect (like Paratroopers, Rebel Ambush)
    pub fn spawn_effect(
        power_type: SpecialPowerType,
        template: impl Into<String>,
        count: u32,
        radius: f32,
    ) -> Self {
        Self {
            power_type,
            spawn_objects: vec![SpawnConfig {
                template: template.into(),
                count,
                radius,
                temporary: false,
                lifetime: 0,
            }],
            ..Default::default()
        }
    }

    /// Create a buff effect (like Emergency Repair, Frenzy)
    pub fn buff_effect(
        power_type: SpecialPowerType,
        effect_type: StatusEffectType,
        duration: u32,
        strength: f32,
        radius: f32,
    ) -> Self {
        Self {
            power_type,
            status_effects: vec![StatusEffectConfig {
                effect_type,
                duration,
                strength,
            }],
            area_radius: radius,
            friendly_only: true,
            ..Default::default()
        }
    }

    /// Check if an object meets the filter criteria
    pub fn matches_filter(&self, object_id: ObjectId) -> bool {
        let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) else {
            return false;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return false;
        };

        if self.buildings_only && !obj_guard.is_structure() {
            return false;
        }
        if self.vehicles_only && !obj_guard.is_kind_of(KindOf::Vehicle) {
            return false;
        }
        if self.infantry_only && !obj_guard.is_kind_of(KindOf::Infantry) {
            return false;
        }

        if self.enemy_only || self.friendly_only {
            let Ok(list) = player_list().read() else {
                return true;
            };
            let local_player = list.get_local_player().cloned();
            let owner_player = obj_guard.get_controlling_player();

            if let (Some(local_arc), Some(owner_arc)) = (local_player, owner_player) {
                if let (Ok(local_guard), Ok(owner_guard)) = (local_arc.read(), owner_arc.read()) {
                    if self.enemy_only && !local_guard.is_enemy_with_player(&*owner_guard) {
                        return false;
                    }
                    if self.friendly_only && !local_guard.is_allied_with_player(&*owner_guard) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Calculate damage at a given distance from epicenter
    /// Matches C++ damage falloff calculations
    pub fn calculate_damage_at_distance(&self, distance: f32) -> Option<f32> {
        if let Some(ref damage_config) = self.damage {
            if self.area_radius == 0.0 {
                // Point target, no falloff
                return Some(damage_config.amount);
            }

            if distance > self.area_radius {
                // Outside radius
                return Some(0.0);
            }

            // Calculate falloff
            let distance_ratio = distance / self.area_radius;
            let damage_mult = 1.0 - (distance_ratio * (1.0 - damage_config.falloff));
            Some(damage_config.amount * damage_mult)
        } else {
            None
        }
    }

    /// Get spawn requests for this effect at a location
    pub fn get_spawn_requests(&self, location: Coord3D, owner: u32) -> Vec<SpawnRequest> {
        let mut requests = Vec::new();

        for spawn_config in &self.spawn_objects {
            for i in 0..spawn_config.count {
                // Calculate offset position within spawn radius
                let angle = (i as f32 / spawn_config.count as f32) * std::f32::consts::TAU;
                let offset_x = angle.cos() * spawn_config.radius;
                let offset_y = angle.sin() * spawn_config.radius;

                let spawn_location =
                    Coord3D::new(location.x + offset_x, location.y + offset_y, location.z);

                if spawn_config.temporary {
                    requests.push(SpawnRequest::new_temporary(
                        &spawn_config.template,
                        spawn_location,
                        owner,
                        spawn_config.lifetime,
                    ));
                } else {
                    requests.push(SpawnRequest::new_unit(
                        &spawn_config.template,
                        spawn_location,
                        owner,
                    ));
                }
            }
        }

        requests
    }
}

/// Special power effects registry
/// Stores effect definitions for all power types
#[derive(Debug, Default)]
pub struct SpecialPowerEffectsRegistry {
    effects: std::collections::HashMap<SpecialPowerType, SpecialPowerEffect>,
}

impl SpecialPowerEffectsRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Register an effect
    pub fn register_effect(&mut self, effect: SpecialPowerEffect) {
        self.effects.insert(effect.power_type, effect);
    }

    /// Get effect for a power type
    pub fn get_effect(&self, power_type: SpecialPowerType) -> Option<&SpecialPowerEffect> {
        self.effects.get(&power_type)
    }

    /// Initialize with default effects for all Generals powers
    /// Matches C++ default configurations
    pub fn init_default_effects(&mut self) {
        // A10 Strike - Explosive damage in line
        self.register_effect(SpecialPowerEffect::damage_effect(
            SpecialPowerType::A10ThunderboltStrike,
            300.0,
            DamageType::Explosive,
            50.0,
            0.5,
        ));

        // Artillery Barrage - Multiple explosive impacts
        self.register_effect(SpecialPowerEffect::damage_effect(
            SpecialPowerType::ArtilleryBarrage,
            200.0,
            DamageType::Explosive,
            75.0,
            0.7,
        ));

        // Carpet Bomb - Large area explosive
        self.register_effect(SpecialPowerEffect::damage_effect(
            SpecialPowerType::CarpetBomb,
            400.0,
            DamageType::Explosive,
            100.0,
            0.6,
        ));

        // Daisy Cutter - Massive explosive
        self.register_effect(SpecialPowerEffect::damage_effect(
            SpecialPowerType::DaisyCutter,
            800.0,
            DamageType::Explosive,
            150.0,
            0.8,
        ));

        // Napalm Strike - Fire damage over time
        let mut napalm = SpecialPowerEffect::damage_effect(
            SpecialPowerType::NapalmStrike,
            150.0,
            DamageType::Fire,
            60.0,
            0.5,
        );
        napalm.status_effects.push(StatusEffectConfig {
            effect_type: StatusEffectType::Burning,
            duration: 300, // 10 seconds at 30 fps
            strength: 5.0,
        });
        self.register_effect(napalm);

        // Anthrax Bomb - Chemical damage over time
        let mut anthrax = SpecialPowerEffect::damage_effect(
            SpecialPowerType::AnthraxBomb,
            100.0,
            DamageType::Chemical,
            80.0,
            0.6,
        );
        anthrax.status_effects.push(StatusEffectConfig {
            effect_type: StatusEffectType::Poisoned,
            duration: 600, // 20 seconds
            strength: 3.0,
        });
        self.register_effect(anthrax);

        // Neutron Missile - Radiation damage
        self.register_effect(SpecialPowerEffect::damage_effect(
            SpecialPowerType::NeutronMissile,
            500.0,
            DamageType::Radiation,
            120.0,
            0.7,
        ));

        // EMP Pulse - Disabling
        let mut emp = SpecialPowerEffect::default();
        emp.power_type = SpecialPowerType::EmpPulse;
        emp.area_radius = 200.0;
        emp.status_effects.push(StatusEffectConfig {
            effect_type: StatusEffectType::Disabled,
            duration: 450, // 15 seconds
            strength: 1.0,
        });
        self.register_effect(emp);

        // Paradrop America - Spawn rangers
        self.register_effect(SpecialPowerEffect::spawn_effect(
            SpecialPowerType::ParadropAmerica,
            "AmericaVehicleRanger",
            8,
            30.0,
        ));

        // Rebel Ambush - Spawn rebels
        self.register_effect(SpecialPowerEffect::spawn_effect(
            SpecialPowerType::Ambush,
            "GLAInfantryRebel",
            12,
            40.0,
        ));

        // Emergency Repair - Healing buff
        self.register_effect(SpecialPowerEffect::buff_effect(
            SpecialPowerType::RepairVehicles,
            StatusEffectType::Regenerating,
            300,  // 10 seconds
            10.0, // HP per second
            200.0,
        ));

        // Frenzy - Damage boost
        self.register_effect(SpecialPowerEffect::buff_effect(
            SpecialPowerType::Frenzy,
            StatusEffectType::DamageBoost,
            450, // 15 seconds
            1.5, // 50% damage boost
            300.0,
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_effect_creation() {
        let effect = StatusEffect::new(StatusEffectType::Burning, 300, 5.0, 0);
        assert_eq!(effect.effect_type, StatusEffectType::Burning);
        assert_eq!(effect.duration, 300);
        assert_eq!(effect.strength, 5.0);
    }

    #[test]
    fn test_status_effect_active() {
        let effect = StatusEffect::new(StatusEffectType::Disabled, 100, 1.0, 0);
        assert!(effect.is_active(50));
        assert!(effect.is_active(99));
        assert!(!effect.is_active(100));
        assert!(!effect.is_active(150));
    }

    #[test]
    fn test_damage_falloff() {
        let effect = SpecialPowerEffect::damage_effect(
            SpecialPowerType::A10ThunderboltStrike,
            100.0,
            DamageType::Explosive,
            50.0,
            0.5, // 50% damage at edge
        );

        // At epicenter
        assert_eq!(effect.calculate_damage_at_distance(0.0), Some(100.0));

        // At edge (50m)
        assert_eq!(effect.calculate_damage_at_distance(50.0), Some(50.0));

        // Outside radius
        assert_eq!(effect.calculate_damage_at_distance(100.0), Some(0.0));

        // Halfway (25m) - should be 75% damage
        let mid_damage = effect.calculate_damage_at_distance(25.0).unwrap();
        assert!((mid_damage - 75.0).abs() < 0.1);
    }

    #[test]
    fn test_spawn_requests() {
        let effect =
            SpecialPowerEffect::spawn_effect(SpecialPowerType::ParadropAmerica, "Ranger", 4, 20.0);

        let requests = effect.get_spawn_requests(Coord3D::new(100.0, 100.0, 0.0), 1);
        assert_eq!(requests.len(), 4);

        for request in requests {
            assert_eq!(request.object_template, "Ranger");
            assert_eq!(request.owner_player_id, 1);
            assert!(!request.temporary);
        }
    }

    #[test]
    fn test_registry() {
        let mut registry = SpecialPowerEffectsRegistry::new();
        registry.init_default_effects();

        // Check A10 strike is registered
        let a10_effect = registry.get_effect(SpecialPowerType::A10ThunderboltStrike);
        assert!(a10_effect.is_some());
        assert!(a10_effect.unwrap().damage.is_some());

        // Check paradrop is registered
        let paradrop_effect = registry.get_effect(SpecialPowerType::ParadropAmerica);
        assert!(paradrop_effect.is_some());
        assert!(!paradrop_effect.unwrap().spawn_objects.is_empty());
    }

    #[test]
    fn test_spawn_request_creation() {
        let unit = SpawnRequest::new_unit("Tank", Coord3D::new(10.0, 20.0, 0.0), 1);
        assert!(!unit.temporary);
        assert_eq!(unit.health_percentage, 1.0);

        let temp = SpawnRequest::new_temporary("ViewObject", Coord3D::new(10.0, 20.0, 0.0), 1, 600);
        assert!(temp.temporary);
        assert_eq!(temp.lifetime, 600);
    }
}
