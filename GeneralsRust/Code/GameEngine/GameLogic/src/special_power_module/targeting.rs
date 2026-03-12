//! Targeting System for Special Powers

use super::types::*;
use crate::common::*;
use crate::terrain::get_terrain_logic;

/// Targeting information for special powers
#[derive(Debug, Clone)]
pub struct TargetingInfo {
    /// Target position in world coordinates
    pub position: Coord3D,
    /// Target object ID (if targeting specific object)
    pub target_object: Option<ObjectID>,
    /// Range of the power
    pub range: Real,
    /// Radius of effect
    pub radius: Real,
    /// Whether line of sight is required
    pub requires_los: Bool,
    /// Minimum range (for some powers)
    pub min_range: Real,
    /// Maximum altitude for targeting
    pub max_altitude: Real,
    /// Flags controlling targeting behavior
    pub flags: SpecialPowerFlags,
}

impl TargetingInfo {
    pub fn new(position: Coord3D, range: Real, radius: Real) -> Self {
        Self {
            position,
            target_object: None,
            range,
            radius,
            requires_los: false,
            min_range: 0.0,
            max_altitude: 1000.0,
            flags: SpecialPowerFlags::empty(),
        }
    }

    /// Check if a position is within range
    pub fn is_in_range(&self, source_pos: &Coord3D) -> Bool {
        if self.range <= 0.0 {
            return true;
        }
        let dx = self.position.x - source_pos.x;
        let dy = self.position.y - source_pos.y;
        let distance = (dx * dx + dy * dy).sqrt();
        distance >= self.min_range && distance <= self.range
    }

    /// Check if target is within minimum range
    pub fn is_beyond_min_range(&self, source_pos: &Coord3D) -> Bool {
        let dx = self.position.x - source_pos.x;
        let dy = self.position.y - source_pos.y;
        let distance = (dx * dx + dy * dy).sqrt();
        distance >= self.min_range
    }

    /// Get all positions within the effect radius
    pub fn get_affected_positions(&self, grid_spacing: Real) -> Vec<Coord3D> {
        let mut positions = Vec::new();
        let steps = (self.radius / grid_spacing).ceil() as i32;

        for x in -steps..=steps {
            for y in -steps..=steps {
                let offset = Coord3D::new(x as Real * grid_spacing, y as Real * grid_spacing, 0.0);
                let pos = self.position + offset;

                if (offset.length()) <= self.radius {
                    positions.push(pos);
                }
            }
        }

        positions
    }
}

/// Target validation result
#[derive(Debug, Clone, PartialEq)]
pub enum TargetValidation {
    Valid,
    OutOfRange,
    InsideMinRange,
    NoLineOfSight,
    InvalidTerrain,
    InvalidTarget,
    Custom(String),
}

impl TargetValidation {
    pub fn is_valid(&self) -> Bool {
        matches!(self, TargetValidation::Valid)
    }

    pub fn reason(&self) -> String {
        match self {
            TargetValidation::Valid => "Valid target".to_string(),
            TargetValidation::OutOfRange => "Target out of range".to_string(),
            TargetValidation::InsideMinRange => "Target too close".to_string(),
            TargetValidation::NoLineOfSight => "No line of sight".to_string(),
            TargetValidation::InvalidTerrain => "Invalid terrain".to_string(),
            TargetValidation::InvalidTarget => "Invalid target".to_string(),
            TargetValidation::Custom(reason) => reason.clone(),
        }
    }
}

/// Target validator for special powers
pub struct TargetValidator;

impl TargetValidator {
    /// Validate a target position for a special power
    pub fn validate_target(
        targeting: &TargetingInfo,
        source_pos: &Coord3D,
        _map_bounds: Option<&(Coord3D, Coord3D)>,
    ) -> TargetValidation {
        // Check range
        if !targeting.is_in_range(source_pos) {
            return TargetValidation::OutOfRange;
        }

        // Check minimum range
        if !targeting.is_beyond_min_range(source_pos) {
            return TargetValidation::InsideMinRange;
        }

        // Check altitude
        if targeting.position.z > targeting.max_altitude {
            return TargetValidation::Custom("Target altitude too high".to_string());
        }

        // Check line of sight if required
        // Matches C++ PartitionManager.cpp line 1436: isClearLineOfSightTerrain
        if targeting.requires_los {
            let has_los = get_terrain_logic()
                .read()
                .ok()
                .map(|terrain| terrain.is_clear_line_of_sight(source_pos, &targeting.position))
                .unwrap_or(true);

            if !has_los {
                return TargetValidation::NoLineOfSight;
            }
        }

        // Check terrain validity
        // Matches C++ PartitionManager.cpp terrain checking logic
        // Terrain validation would check for water, cliffs, buildability
        if let Some(terrain) = crate::helpers::TheTerrainLogic::get() {
            // Check if position is underwater when it shouldn't be
            if terrain.is_underwater(targeting.position.x, targeting.position.y, None, None) {
                // Allow underwater targets if power explicitly allows it
                // Most powers should fail on water
                return TargetValidation::InvalidTerrain;
            }

            // Check if position is on a cliff (usually invalid for most powers)
            if terrain.is_cliff_cell(targeting.position.x, targeting.position.y) {
                return TargetValidation::InvalidTerrain;
            }
        }

        TargetValidation::Valid
    }

    /// Check if an object is a valid target based on power flags
    /// Matches C++ PartitionFilter logic in PartitionManager.h lines 603-857
    pub fn is_valid_object_target(object_id: ObjectID, flags: SpecialPowerFlags) -> Bool {
        // Find the object using TheGameLogic
        // Matches C++ TheGameLogic->findObjectByID pattern
        let object = match crate::helpers::TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return false,
        };

        let obj_guard = match object.read() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        // Check object state - must be alive and not under construction
        // Matches C++ Object status checking pattern (Object.h)
        if obj_guard.is_destroyed() {
            return false;
        }

        // Check if object is under construction
        // Matches C++ OBJECT_STATUS_UNDER_CONSTRUCTION check (SpecialPowerModule.cpp line 88)
        let status_bits = obj_guard.get_status_bits();
        if status_bits.test(crate::common::ObjectStatusTypes::UnderConstruction) {
            // Most special powers can't target objects under construction
            // Currently no ALLOW_UNDER_CONSTRUCTION flag in SpecialPowerFlags
            // so we reject all objects under construction
            return false;
        }

        if obj_guard.is_structure() && !flags.contains(SpecialPowerFlags::AFFECTS_BUILDINGS) {
            return false;
        }

        if flags.intersects(
            SpecialPowerFlags::AFFECTS_FRIENDLY
                | SpecialPowerFlags::AFFECTS_ENEMY
                | SpecialPowerFlags::AFFECTS_NEUTRAL,
        ) {
            let local_player = crate::player::player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned());
            if let Some(local_player) = local_player {
                let Ok(local_guard) = local_player.read() else {
                    return false;
                };
                let relationship = if let Some(target_player) = obj_guard.get_controlling_player() {
                    if let Ok(target_guard) = target_player.read() {
                        if target_guard.get_player_index() == local_guard.get_player_index() {
                            Relationship::Friend
                        } else {
                            local_guard.get_relationship(&target_guard)
                        }
                    } else {
                        Relationship::Neutral
                    }
                } else {
                    Relationship::Neutral
                };

                let allowed = match relationship {
                    Relationship::Friend | Relationship::Ally | Relationship::Allies => {
                        flags.contains(SpecialPowerFlags::AFFECTS_FRIENDLY)
                    }
                    Relationship::Enemy => flags.contains(SpecialPowerFlags::AFFECTS_ENEMY),
                    Relationship::Neutral => flags.contains(SpecialPowerFlags::AFFECTS_NEUTRAL),
                    _ => true,
                };

                if !allowed {
                    return false;
                }
            }
        }

        // Object type filtering (building vs unit)
        // Matches C++ KindOf checking pattern
        // This would use obj_guard.is_kind_of(KINDOF_STRUCTURE) etc.

        true
    }

    /// Get all objects within the effect radius
    /// Matches C++ PartitionManager::iterateObjectsInRange (PartitionManager.cpp line 3585)
    pub fn get_objects_in_radius(
        center: &Coord3D,
        radius: Real,
        flags: SpecialPowerFlags,
    ) -> Vec<ObjectID> {
        // Use ThePartitionManager to query spatial data
        // Matches C++ ThePartitionManager->iterateObjectsInRange pattern
        // See PartitionManager.cpp lines 3585-3602 and 3565-3582
        let partition_mgr = match crate::helpers::ThePartitionManager::get() {
            Some(mgr) => mgr,
            None => return Vec::new(),
        };

        // Get all object IDs within radius using spatial partitioning.
        let object_ids = partition_mgr.get_objects_in_range(center, radius);

        // Filter objects based on special power flags
        // Matches C++ filter pattern from PartitionManager::getClosestObjects
        object_ids
            .into_iter()
            .filter(|&object_id| Self::is_valid_object_target(object_id, flags))
            .collect()
    }

    /// Calculate optimal target position for area-effect powers
    /// Implements a clustering algorithm to find the position that maximizes hits
    /// This is similar to the AI targeting logic used in C++ AIUpdate modules
    pub fn calculate_optimal_target(
        _source_pos: &Coord3D,
        enemy_positions: &[Coord3D],
        radius: Real,
    ) -> Option<Coord3D> {
        if enemy_positions.is_empty() {
            return None;
        }

        // Simple greedy algorithm: for each enemy position, count how many
        // other enemies would be within the radius if we targeted that position.
        // This matches the typical AI targeting heuristic used in C++ code
        // See AIUpdate.cpp and similar modules for target selection logic

        let mut best_position: Option<Coord3D> = None;
        let mut best_hit_count = 0;
        let radius_sqr = radius * radius;

        for candidate in enemy_positions {
            // Count how many enemies are within radius of this candidate position
            let mut hit_count = 0;

            for enemy in enemy_positions {
                let dx = enemy.x - candidate.x;
                let dy = enemy.y - candidate.y;
                let dist_sqr = dx * dx + dy * dy;

                if dist_sqr <= radius_sqr {
                    hit_count += 1;
                }
            }

            // Update best if this position hits more enemies
            if hit_count > best_hit_count {
                best_hit_count = hit_count;
                best_position = Some(*candidate);
            }
        }

        // Also consider the centroid of all enemy positions as a candidate
        // This often provides better coverage for clustered enemies
        if enemy_positions.len() > 1 {
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut sum_z = 0.0;

            for pos in enemy_positions {
                sum_x += pos.x;
                sum_y += pos.y;
                sum_z += pos.z;
            }

            let count = enemy_positions.len() as Real;
            let centroid = Coord3D::new(sum_x / count, sum_y / count, sum_z / count);

            // Count hits from centroid
            let mut centroid_hits = 0;
            for enemy in enemy_positions {
                let dx = enemy.x - centroid.x;
                let dy = enemy.y - centroid.y;
                let dist_sqr = dx * dx + dy * dy;

                if dist_sqr <= radius_sqr {
                    centroid_hits += 1;
                }
            }

            if centroid_hits > best_hit_count {
                best_position = Some(centroid);
            }
        }

        best_position
    }
}

/// Targeting cursor data (for UI integration)
#[derive(Debug, Clone)]
pub struct TargetingCursor {
    /// Current cursor position
    pub position: Coord3D,
    /// Whether cursor is valid
    pub is_valid: Bool,
    /// Validation result
    pub validation: TargetValidation,
    /// Range indicator radius
    pub range_radius: Real,
    /// Effect radius
    pub effect_radius: Real,
    /// Color for range indicator (RGBA)
    pub range_color: (Real, Real, Real, Real),
    /// Color for effect indicator (RGBA)
    pub effect_color: (Real, Real, Real, Real),
}

impl TargetingCursor {
    pub fn new(position: Coord3D, range_radius: Real, effect_radius: Real) -> Self {
        Self {
            position,
            is_valid: false,
            validation: TargetValidation::InvalidTarget,
            range_radius,
            effect_radius,
            range_color: (0.0, 1.0, 0.0, 0.3),  // Green with alpha
            effect_color: (1.0, 0.0, 0.0, 0.5), // Red with alpha
        }
    }

    /// Update cursor position and validation
    pub fn update(&mut self, targeting: &TargetingInfo, source_pos: &Coord3D) {
        self.position = targeting.position;
        self.validation = TargetValidator::validate_target(targeting, source_pos, None);
        self.is_valid = self.validation.is_valid();

        // Update colors based on validity
        if self.is_valid {
            self.range_color = (0.0, 1.0, 0.0, 0.3); // Green
            self.effect_color = (1.0, 1.0, 0.0, 0.5); // Yellow
        } else {
            self.range_color = (1.0, 0.0, 0.0, 0.3); // Red
            self.effect_color = (1.0, 0.0, 0.0, 0.5); // Red
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_targeting_info() {
        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 500.0, 50.0);

        // In range
        let source = Coord3D::new(200.0, 0.0, 200.0);
        let dx = targeting.position.x - source.x;
        let dy = targeting.position.y - source.y;
        let distance = (dx * dx + dy * dy).sqrt();
        assert!(distance < 500.0);
        assert!(targeting.is_in_range(&source));

        // Out of range
        let source = Coord3D::new(700.0, 0.0, 700.0);
        assert!(!targeting.is_in_range(&source));
    }

    #[test]
    fn test_min_range() {
        let mut targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 500.0, 50.0);
        targeting.min_range = 100.0;

        // Too close
        let source = Coord3D::new(110.0, 0.0, 110.0);
        assert!(!targeting.is_beyond_min_range(&source));

        // Valid range
        let source = Coord3D::new(200.0, 0.0, 200.0);
        assert!(targeting.is_beyond_min_range(&source));
    }

    #[test]
    fn test_target_validation() {
        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 500.0, 50.0);

        // Valid target
        let source = Coord3D::new(200.0, 0.0, 200.0);
        let validation = TargetValidator::validate_target(&targeting, &source, None);
        assert!(validation.is_valid());

        // Out of range
        let source = Coord3D::new(700.0, 0.0, 700.0);
        let validation = TargetValidator::validate_target(&targeting, &source, None);
        assert_eq!(validation, TargetValidation::OutOfRange);
    }

    #[test]
    fn test_affected_positions() {
        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 500.0, 50.0);

        let positions = targeting.get_affected_positions(10.0);
        assert!(!positions.is_empty());

        // All positions should be within radius
        for pos in positions {
            let dx = pos.x - targeting.position.x;
            let dy = pos.y - targeting.position.y;
            let distance = (dx * dx + dy * dy).sqrt();
            assert!(distance <= targeting.radius + 0.1); // Small epsilon for rounding
        }
    }

    #[test]
    fn test_targeting_cursor() {
        let mut cursor = TargetingCursor::new(Coord3D::new(100.0, 0.0, 100.0), 500.0, 50.0);

        let targeting = TargetingInfo::new(Coord3D::new(100.0, 0.0, 100.0), 500.0, 50.0);
        let source = Coord3D::new(200.0, 0.0, 200.0);

        cursor.update(&targeting, &source);
        assert!(cursor.is_valid);
        assert!(cursor.validation.is_valid());
    }
}
