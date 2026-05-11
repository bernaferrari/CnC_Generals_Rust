//! Area Damage System for Special Powers
//!
//! Handles damage application for area-of-effect special powers like nukes,
//! airstrikes, and explosions. Matches C++ damage system behavior.

use super::types::*;
use crate::common::*;
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::TheTerrainLogic;
use once_cell::sync::Lazy;
use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex,
};

/// Damage falloff type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DamageFalloff {
    /// No falloff - constant damage throughout radius
    None,
    /// Linear falloff from max damage to zero
    Linear,
    /// Inverse square falloff (realistic physics)
    InverseSquare,
    /// Custom falloff with inner and outer radius
    TwoStage { inner_radius: Real },
}

/// Area damage configuration
#[derive(Debug, Clone)]
pub struct AreaDamageConfig {
    /// Maximum damage at epicenter
    pub max_damage: Real,
    /// Damage radius
    pub radius: Real,
    /// Minimum damage at edge (for non-zero falloff)
    pub min_damage: Real,
    /// Falloff type
    pub falloff: DamageFalloff,
    /// Damage type flags
    pub damage_type: DamageTypeFlags,
    /// Whether to damage friendlies
    pub affects_friendlies: bool,
    /// Whether to damage buildings
    pub affects_buildings: bool,
    /// Whether to damage terrain
    pub affects_terrain: bool,
}

impl AreaDamageConfig {
    pub fn new(max_damage: Real, radius: Real) -> Self {
        Self {
            max_damage,
            radius,
            min_damage: 0.0,
            falloff: DamageFalloff::Linear,
            damage_type: DamageTypeFlags::EXPLOSION,
            affects_friendlies: false,
            affects_buildings: true,
            affects_terrain: false,
        }
    }

    /// Calculate damage at given distance from epicenter
    pub fn calculate_damage_at_distance(&self, distance: Real) -> Real {
        if distance >= self.radius {
            return 0.0;
        }

        match self.falloff {
            DamageFalloff::None => self.max_damage,
            DamageFalloff::Linear => {
                let falloff_ratio = 1.0 - (distance / self.radius);
                let damage_range = self.max_damage - self.min_damage;
                self.min_damage + (damage_range * falloff_ratio)
            }
            DamageFalloff::InverseSquare => {
                if distance <= 1.0 {
                    return self.max_damage;
                }
                let falloff = 1.0 / (distance * distance);
                (self.max_damage * falloff).max(self.min_damage)
            }
            DamageFalloff::TwoStage { inner_radius } => {
                if distance <= inner_radius {
                    self.max_damage
                } else {
                    let outer_range = self.radius - inner_radius;
                    let outer_distance = distance - inner_radius;
                    let falloff_ratio = 1.0 - (outer_distance / outer_range);
                    let damage_range = self.max_damage - self.min_damage;
                    self.min_damage + (damage_range * falloff_ratio)
                }
            }
        }
    }
}

/// Damage application result
#[derive(Debug, Clone)]
pub struct DamageResult {
    /// Objects damaged
    pub objects_damaged: Vec<ObjectID>,
    /// Total damage dealt
    pub total_damage: Real,
    /// Units killed
    pub units_killed: UnsignedInt,
    /// Buildings destroyed
    pub buildings_destroyed: UnsignedInt,
}

impl DamageResult {
    pub fn new() -> Self {
        Self {
            objects_damaged: Vec::new(),
            total_damage: 0.0,
            units_killed: 0,
            buildings_destroyed: 0,
        }
    }

    pub fn add_damage(&mut self, object_id: ObjectID, damage: Real) {
        self.objects_damaged.push(object_id);
        self.total_damage += damage;
    }
}

impl Default for DamageResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Area damage applicator
pub struct AreaDamageApplicator;

impl AreaDamageApplicator {
    fn primary_damage_type(flags: DamageTypeFlags) -> DamageType {
        if flags.is_empty() {
            return DamageType::Explosion;
        }
        let bit_index = flags.bits().trailing_zeros();
        DamageType::from_u32(bit_index)
    }

    /// Apply area damage at location
    /// Returns damage result with statistics
    pub fn apply_damage_at_location(
        config: &AreaDamageConfig,
        center: &Coord3D,
        attacker_id: ObjectID,
    ) -> Result<DamageResult, String> {
        let mut result = DamageResult::new();

        log::info!(
            "Applying area damage at {:?}: max_damage={}, radius={}",
            center,
            config.max_damage,
            config.radius
        );

        let (source_template, source_player_mask, source_off_map) = if attacker_id != INVALID_ID {
            let mut template = None;
            let mut mask = PlayerMaskType::none();
            let mut off_map = None;
            if let Some(attacker_arc) = crate::helpers::TheGameLogic::find_object_by_id(attacker_id)
            {
                if let Ok(attacker_guard) = attacker_arc.read() {
                    template = Some(attacker_guard.get_template().clone());
                    off_map = Some(attacker_guard.is_off_map());
                    if let Some(player_id) = attacker_guard.get_controlling_player_id() {
                        if player_id < 8 {
                            mask = PlayerMaskType::from_bits_truncate(1u32 << player_id);
                        }
                    }
                }
            }
            (template, mask, off_map)
        } else {
            (None, PlayerMaskType::none(), None)
        };

        let object_ids = crate::helpers::ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(center, config.radius))
            .unwrap_or_default();

        let radius_sqr = config.radius * config.radius;
        let damage_type = Self::primary_damage_type(config.damage_type);

        for obj_id in object_ids {
            let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };

            let (should_damage, distance_2d, was_destroyed, was_structure) = {
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };

                if obj_guard.is_destroyed() {
                    continue;
                }
                if let Some(off_map) = source_off_map {
                    if obj_guard.is_off_map() != off_map {
                        continue;
                    }
                }

                let obj_pos = obj_guard.get_position();
                let dx = obj_pos.x - center.x;
                let dy = obj_pos.y - center.y;
                let dist_sqr = dx * dx + dy * dy;
                if dist_sqr > radius_sqr {
                    continue;
                }

                let dist = dist_sqr.sqrt();
                let should = Self::should_damage_object(&obj_guard, config, attacker_id);
                (
                    should,
                    dist,
                    obj_guard.is_destroyed(),
                    obj_guard.is_structure(),
                )
            };

            if !should_damage {
                continue;
            }

            let damage = config.calculate_damage_at_distance(distance_2d);
            if damage <= 0.0 {
                continue;
            }

            let mut damage_info =
                DamageInfo::with_simple(damage, attacker_id, damage_type, DeathType::Normal);
            damage_info.input.damage_fx_override = damage_type;
            damage_info.input.source_template = source_template.clone();
            damage_info.input.source_player_mask = source_player_mask;
            damage_info.sync_from_input();

            if let Ok(mut obj_write) = obj_arc.write() {
                let _ = obj_write.attempt_damage(&mut damage_info);
            }

            result.add_damage(obj_id, damage);

            let is_destroyed = obj_arc
                .read()
                .ok()
                .map(|g| g.is_destroyed())
                .unwrap_or(false);
            if !was_destroyed && is_destroyed {
                if was_structure {
                    result.buildings_destroyed += 1;
                } else {
                    result.units_killed += 1;
                }
            }
        }

        log::debug!(
            "Area damage complete: {} objects damaged, {} total damage",
            result.objects_damaged.len(),
            result.total_damage
        );

        Ok(result)
    }

    /// Check if object should be damaged based on config and relationship
    fn should_damage_object(
        object: &Object,
        config: &AreaDamageConfig,
        attacker_id: ObjectID,
    ) -> bool {
        if !config.affects_buildings {
            if object.is_structure() {
                return false;
            }
        }

        if config.damage_type.contains(DamageTypeFlags::POISON)
            && !object.is_kind_of(KindOf::Infantry)
        {
            return false;
        }

        if !config.affects_friendlies {
            if let Some(attacker_arc) = crate::helpers::TheGameLogic::find_object_by_id(attacker_id)
            {
                if let Ok(attacker_guard) = attacker_arc.read() {
                    if matches!(attacker_guard.relationship_to(object), Relationship::Allies) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Create damage over time effect (for radiation, fire, etc.)
    pub fn create_damage_over_time(
        config: &AreaDamageConfig,
        center: &Coord3D,
        duration: Real,
        tick_rate: Real,
        attacker_id: ObjectID,
    ) -> Result<ObjectID, String> {
        let tick_rate = tick_rate.max(0.0);
        let duration = duration.max(0.0);
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let mut center = *center;
        if let Some(terrain) = TheTerrainLogic::get() {
            center.z = terrain.get_ground_height(center.x, center.y, None);
        }
        let tick_frames = (tick_rate * LOGICFRAMES_PER_SECOND as Real).max(1.0) as UnsignedInt;
        let end_frame = current_frame + (duration * LOGICFRAMES_PER_SECOND as Real) as UnsignedInt;

        log::info!(
            "Creating damage-over-time field at {:?}: duration={}s, tick_rate={}s",
            center,
            duration,
            tick_rate
        );

        let id = DAMAGE_OVER_TIME_NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let mut fields = DAMAGE_OVER_TIME_FIELDS
            .lock()
            .expect("DOT field lock poisoned");
        fields.push(DamageOverTimeField {
            id,
            config: config.clone(),
            center,
            end_frame,
            next_tick_frame: current_frame,
            tick_frames,
            attacker_id,
        });

        Ok(id)
    }
}

#[derive(Debug, Clone)]
struct DamageOverTimeField {
    id: ObjectID,
    config: AreaDamageConfig,
    center: Coord3D,
    end_frame: UnsignedInt,
    next_tick_frame: UnsignedInt,
    tick_frames: UnsignedInt,
    attacker_id: ObjectID,
}

static DAMAGE_OVER_TIME_NEXT_ID: AtomicU32 = AtomicU32::new(50000);
static DAMAGE_OVER_TIME_FIELDS: Lazy<Mutex<Vec<DamageOverTimeField>>> =
    Lazy::new(|| Mutex::new(Vec::new()));

pub fn update_damage_over_time(current_frame: UnsignedInt) {
    let mut fields = DAMAGE_OVER_TIME_FIELDS
        .lock()
        .expect("DOT field lock poisoned");
    fields.retain_mut(|field| {
        if current_frame >= field.end_frame {
            return false;
        }
        if current_frame < field.next_tick_frame {
            return true;
        }

        let _ = AreaDamageApplicator::apply_damage_at_location(
            &field.config,
            &field.center,
            field.attacker_id,
        );

        field.next_tick_frame = current_frame + field.tick_frames;
        true
    });
}

pub fn clear_damage_over_time(
    center: &Coord3D,
    radius: Real,
    damage_mask: DamageTypeFlags,
) -> usize {
    let mut removed = 0usize;
    let radius_sqr = radius * radius;
    let mut fields = DAMAGE_OVER_TIME_FIELDS
        .lock()
        .expect("DOT field lock poisoned");

    fields.retain(|field| {
        let dx = field.center.x - center.x;
        let dy = field.center.y - center.y;
        let dist_sqr = dx * dx + dy * dy;
        let in_range = dist_sqr <= radius_sqr;
        let matches = !damage_mask.is_empty() && field.config.damage_type.intersects(damage_mask);

        if in_range && matches {
            removed += 1;
            false
        } else {
            true
        }
    });

    removed
}

/// Helper for nuclear weapon effects
pub struct NuclearDamageHelper;

impl NuclearDamageHelper {
    /// Apply nuclear blast damage with radiation
    pub fn apply_nuclear_blast(
        blast_center: &Coord3D,
        blast_radius: Real,
        blast_damage: Real,
        radiation_radius: Real,
        radiation_duration: Real,
        attacker_id: ObjectID,
    ) -> Result<DamageResult, String> {
        log::info!("Applying nuclear blast at {:?}", blast_center);

        // Apply immediate blast damage
        let blast_config = AreaDamageConfig {
            max_damage: blast_damage,
            radius: blast_radius,
            min_damage: blast_damage * 0.1, // 10% damage at edge
            falloff: DamageFalloff::InverseSquare,
            damage_type: DamageTypeFlags::EXPLOSION,
            affects_friendlies: true, // Nuclear weapons damage everything
            affects_buildings: true,
            affects_terrain: true,
        };

        let blast_result = AreaDamageApplicator::apply_damage_at_location(
            &blast_config,
            blast_center,
            attacker_id,
        )?;

        // Create radiation field
        let radiation_config = AreaDamageConfig {
            max_damage: blast_damage * 0.05, // 5% damage per tick
            radius: radiation_radius,
            min_damage: 0.0,
            falloff: DamageFalloff::Linear,
            damage_type: DamageTypeFlags::RADIATION,
            affects_friendlies: true,
            affects_buildings: false, // Radiation doesn't damage buildings
            affects_terrain: false,
        };

        AreaDamageApplicator::create_damage_over_time(
            &radiation_config,
            blast_center,
            radiation_duration,
            1.0, // 1 second tick rate
            attacker_id,
        )?;

        Ok(blast_result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_damage_falloff_none() {
        let config = AreaDamageConfig {
            max_damage: 1000.0,
            radius: 100.0,
            min_damage: 0.0,
            falloff: DamageFalloff::None,
            damage_type: DamageTypeFlags::EXPLOSION,
            affects_friendlies: false,
            affects_buildings: true,
            affects_terrain: false,
        };

        assert_eq!(config.calculate_damage_at_distance(0.0), 1000.0);
        assert_eq!(config.calculate_damage_at_distance(50.0), 1000.0);
        assert_eq!(config.calculate_damage_at_distance(99.0), 1000.0);
        assert_eq!(config.calculate_damage_at_distance(100.0), 0.0);
    }

    #[test]
    fn test_damage_falloff_linear() {
        let config = AreaDamageConfig {
            max_damage: 1000.0,
            radius: 100.0,
            min_damage: 0.0,
            falloff: DamageFalloff::Linear,
            damage_type: DamageTypeFlags::EXPLOSION,
            affects_friendlies: false,
            affects_buildings: true,
            affects_terrain: false,
        };

        assert_eq!(config.calculate_damage_at_distance(0.0), 1000.0);
        assert!((config.calculate_damage_at_distance(50.0) - 500.0).abs() < 0.1);
        assert_eq!(config.calculate_damage_at_distance(100.0), 0.0);
        assert_eq!(config.calculate_damage_at_distance(150.0), 0.0);
    }

    #[test]
    fn test_damage_falloff_two_stage() {
        let config = AreaDamageConfig {
            max_damage: 1000.0,
            radius: 100.0,
            min_damage: 0.0,
            falloff: DamageFalloff::TwoStage { inner_radius: 50.0 },
            damage_type: DamageTypeFlags::EXPLOSION,
            affects_friendlies: false,
            affects_buildings: true,
            affects_terrain: false,
        };

        // Full damage within inner radius
        assert_eq!(config.calculate_damage_at_distance(0.0), 1000.0);
        assert_eq!(config.calculate_damage_at_distance(25.0), 1000.0);
        assert_eq!(config.calculate_damage_at_distance(50.0), 1000.0);

        // Falloff in outer ring
        assert!((config.calculate_damage_at_distance(75.0) - 500.0).abs() < 0.1);
        assert_eq!(config.calculate_damage_at_distance(100.0), 0.0);
    }

    #[test]
    fn test_area_damage_config_creation() {
        let config = AreaDamageConfig::new(2000.0, 150.0);

        assert_eq!(config.max_damage, 2000.0);
        assert_eq!(config.radius, 150.0);
        assert_eq!(config.min_damage, 0.0);
        assert!(matches!(config.falloff, DamageFalloff::Linear));
    }

    #[test]
    fn test_damage_result() {
        let mut result = DamageResult::new();

        assert_eq!(result.total_damage, 0.0);
        assert_eq!(result.objects_damaged.len(), 0);

        result.add_damage(1, 500.0);
        result.add_damage(2, 300.0);

        assert_eq!(result.total_damage, 800.0);
        assert_eq!(result.objects_damaged.len(), 2);
    }
}
