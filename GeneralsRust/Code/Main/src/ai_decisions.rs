use crate::game_logic::*;
use glam::Vec3;

/// Game phase enumeration for build strategy decisions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    Early, // 0-5 minutes: Basic units, economy focus
    Mid,   // 5-15 minutes: Mixed forces, tech upgrades
    Late,  // 15+ minutes: Advanced units, multiple attack groups
}

impl GamePhase {
    /// Determine game phase from elapsed game time
    pub fn from_time(game_time: f32) -> Self {
        match game_time {
            t if t < 300.0 => GamePhase::Early, // First 5 minutes
            t if t < 900.0 => GamePhase::Mid,   // 5-15 minutes
            _ => GamePhase::Late,               // After 15 minutes
        }
    }
}

/// Threat level assessment for a position
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThreatAssessment {
    pub threat_level: f32, // Overall threat value (0.0 = safe, higher = more dangerous)
    pub enemy_count: u32,  // Number of enemies detected
    pub closest_enemy_distance: f32, // Distance to closest enemy
    pub has_anti_air: bool, // Whether enemies have anti-air capability
    pub has_anti_armor: bool, // Whether enemies have anti-armor capability
}

/// Attack decision result
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttackDecision {
    Attack,        // Attack the target
    Hold,          // Hold position, don't attack yet
    Retreat,       // Retreat from combat
    FindNewTarget, // Find a different target
}

/// Target priority for selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TargetPriority {
    Critical = 4, // Must be destroyed immediately (e.g., threats to base)
    High = 3,     // High value targets (e.g., commanders, key buildings)
    Medium = 2,   // Standard targets (e.g., military units)
    Low = 1,      // Low priority (e.g., workers, scouts)
    Ignore = 0,   // Should not be targeted
}

/// AI Decision System - provides tactical decision making functions
pub struct AIDecisionSystem;

impl AIDecisionSystem {
    /// Find the nearest enemy to a given position within search radius
    /// Returns (ObjectId, distance) of the nearest enemy, or None if no enemies found
    /// Broadphase: partition radius when warm, else full object table.
    #[inline]
    fn candidate_object_ids(
        game_logic: &GameLogic,
        position: Vec3,
        search_radius: f32,
    ) -> Vec<ObjectId> {
        let near = game_logic.object_ids_near(position, search_radius);
        if !near.is_empty() {
            return near;
        }
        game_logic.get_objects().keys().copied().collect()
    }

    pub fn find_nearest_enemy(
        game_logic: &GameLogic,
        position: Vec3,
        team: Team,
        search_radius: f32,
    ) -> Option<(ObjectId, f32)> {
        // Pure residual acquire: nearest enemy targetable by `team` within radius (3D).
        // Partition broadphase when physics has registered cells this frame.
        let candidates: Vec<_> = Self::candidate_object_ids(game_logic, position, search_radius)
            .into_iter()
            .filter_map(|object_id| {
                let object = game_logic.find_object(object_id)?;
                // Skip if not an enemy (includes stealthed-undetected residual gate).
                if !object.is_targetable_by_enemy_of(team) {
                    return None;
                }
                Some(
                    crate::game_logic::host_residual_acquire::ResidualAcquireCandidate {
                        id: object_id,
                        team: object.team,
                        position: object.get_position(),
                        is_alive: object.is_alive(),
                        is_neutral: object.team == Team::Neutral,
                        under_construction: object.status.under_construction,
                        combat_kind: true,
                        effectively_stealthed: object.is_effectively_stealthed(),
                        is_air: object.is_kind_of(crate::game_logic::KindOf::Aircraft)
                            || object.status.airborne_target,
                        eject_invulnerable: object.is_eject_invulnerable(),
                    },
                )
            })
            .collect();
        crate::game_logic::host_residual_acquire::pick_nearest_residual_target(
            ObjectId(u32::MAX),
            team,
            position,
            candidates,
            |_| search_radius,
            |_| true,
        )
        .map(|(id, dist, _)| (id, dist))
    }

    /// Find the best target based on multiple criteria (distance, health, threat level)
    /// Returns ObjectId of the best target, or None if no valid targets
    pub fn find_best_target(
        game_logic: &GameLogic,
        attacker_id: ObjectId,
        position: Vec3,
        team: Team,
        search_radius: f32,
        prefer_weak: bool,      // Prefer low-health targets
        prefer_close: bool,     // Prefer close targets
        prefer_dangerous: bool, // Prefer high-threat targets
    ) -> Option<ObjectId> {
        let mut best_target: Option<(ObjectId, f32)> = None; // (id, score)

        for object_id in Self::candidate_object_ids(game_logic, position, search_radius) {
            // Skip self
            if object_id == attacker_id {
                continue;
            }
            let Some(object) = game_logic.find_object(object_id) else {
                continue;
            };
            // Skip if not a valid target (stealthed+undetected are not targetable).
            if !object.is_targetable_by_enemy_of(team) {
                continue;
            }

            let target_pos = object.get_position();
            let distance = position.distance(target_pos);

            // Check if within search radius
            if distance > search_radius {
                continue;
            }

            // Calculate target score based on preferences
            let mut score = 100.0;

            // Distance factor (closer = higher score if prefer_close)
            if prefer_close {
                let distance_factor = 1.0 - (distance / search_radius).min(1.0);
                score += distance_factor * 50.0;
            }

            // Health factor (weaker = higher score if prefer_weak)
            if prefer_weak {
                let health_factor = 1.0 - object.health.percentage();
                score += health_factor * 40.0;
            }

            // Threat factor (more dangerous = higher score if prefer_dangerous)
            if prefer_dangerous {
                let threat_score = Self::calculate_unit_threat_value(object);
                score += threat_score * 30.0;
            }

            // Priority bonus for high-value targets
            let priority_bonus = match Self::get_target_priority(object) {
                TargetPriority::Critical => 100.0,
                TargetPriority::High => 50.0,
                TargetPriority::Medium => 20.0,
                TargetPriority::Low => 0.0,
                TargetPriority::Ignore => -1000.0, // Ensure ignored targets are never selected
            };
            score += priority_bonus;

            // Update best target if this score is higher
            match best_target {
                Some((_, best_score)) if score > best_score => {
                    best_target = Some((object_id, score));
                }
                None => {
                    best_target = Some((object_id, score));
                }
                _ => {}
            }
        }

        best_target.map(|(id, _)| id)
    }

    /// Determine if an attacker should attack a specific target
    /// Returns AttackDecision enum indicating what action to take
    pub fn should_attack(
        game_logic: &GameLogic,
        attacker_id: ObjectId,
        target_id: ObjectId,
    ) -> AttackDecision {
        // Get attacker and target
        let attacker = match game_logic.find_object(attacker_id) {
            Some(obj) => obj,
            None => return AttackDecision::Hold,
        };

        let target = match game_logic.find_object(target_id) {
            Some(obj) => obj,
            None => return AttackDecision::FindNewTarget,
        };

        // Don't attack if target is not alive
        if !target.is_alive() {
            return AttackDecision::FindNewTarget;
        }

        // Don't attack friendly units
        if target.team == attacker.team {
            return AttackDecision::Hold;
        }

        // C++ residual: stealthed + not detected is not a valid victim.
        if target.is_effectively_stealthed() {
            return AttackDecision::FindNewTarget;
        }

        // Check if attacker can actually attack
        if !attacker.can_attack() {
            return AttackDecision::Hold;
        }

        // Check if target is in range
        let distance = attacker.get_position().distance(target.get_position());
        let attack_range = attacker.weapon.as_ref().map(|w| w.range).unwrap_or(0.0);

        if distance > attack_range * 1.5 {
            // Target too far, need to move closer
            return AttackDecision::Hold;
        }

        // Assess threat - check if we're severely outnumbered
        let attacker_pos = attacker.get_position();
        let attacker_team = attacker.team;
        let threat = Self::assess_threat(game_logic, attacker_pos, attacker_team, 150.0);

        // If health is low and threat is high, consider retreating
        if attacker.health.percentage() < 0.3 && threat.threat_level > 300.0 {
            return AttackDecision::Retreat;
        }

        // Check if target is already being attacked by many allies
        let allies_attacking =
            Self::count_allies_attacking_target(game_logic, target_id, attacker_team);
        if allies_attacking > 3 {
            // Target has enough attackers, find a different target
            return AttackDecision::FindNewTarget;
        }

        // All checks passed, attack!
        AttackDecision::Attack
    }

    /// Select which unit to produce based on game phase and team
    /// Returns template name of unit to produce, or None if shouldn't produce
    pub fn select_production_unit(
        game_logic: &GameLogic,
        team: Team,
        game_phase: GamePhase,
        player_id: u32,
    ) -> Option<String> {
        // Check if player has enough resources
        let player = game_logic.get_player(player_id)?;

        // Minimum resource threshold before producing units
        let min_resources = match game_phase {
            GamePhase::Early => 200,
            GamePhase::Mid => 400,
            GamePhase::Late => 600,
        };

        if player.resources.supplies < min_resources {
            return None;
        }

        // Count existing military units for this team
        let military_count = game_logic
            .get_objects()
            .values()
            .filter(|obj| obj.team == team && obj.is_alive() && obj.can_attack())
            .count();

        // Don't over-produce in early game
        if game_phase == GamePhase::Early && military_count > 10 {
            return None;
        }

        // Select unit based on team and game phase
        match (team, game_phase) {
            // USA units
            (Team::USA, GamePhase::Early) => {
                // Early game: Rangers and Humvees
                if military_count % 3 == 0 {
                    Some("USA_Humvee".to_string())
                } else {
                    Some("USA_Ranger".to_string())
                }
            }
            (Team::USA, GamePhase::Mid) => {
                // Mid game: Mix of Crusader tanks and infantry
                if military_count % 4 == 0 {
                    Some("USA_CrusaderTank".to_string())
                } else if military_count % 4 == 1 {
                    Some("USA_MissileDefender".to_string())
                } else {
                    Some("USA_Ranger".to_string())
                }
            }
            (Team::USA, GamePhase::Late) => {
                // Late game: Advanced units
                if player.resources.supplies > 1500 {
                    if military_count % 5 == 0 {
                        Some("USA_PaladinTank".to_string())
                    } else if military_count % 5 == 1 {
                        Some("USA_Raptor".to_string())
                    } else {
                        Some("USA_CrusaderTank".to_string())
                    }
                } else {
                    Some("USA_CrusaderTank".to_string())
                }
            }

            // China units
            (Team::China, GamePhase::Early) => {
                // Early game: Red Guards
                if military_count % 4 == 0 {
                    Some("China_TankHunter".to_string())
                } else {
                    Some("China_RedGuard".to_string())
                }
            }
            (Team::China, GamePhase::Mid) => {
                // Mid game: Battlemaster tanks
                if military_count % 3 == 0 {
                    Some("China_BattlemasterTank".to_string())
                } else {
                    Some("China_RedGuard".to_string())
                }
            }
            (Team::China, GamePhase::Late) => {
                // Late game: Overlord tanks and aircraft
                if player.resources.supplies > 1800 {
                    if military_count % 5 == 0 {
                        Some("China_OverlordTank".to_string())
                    } else if military_count % 5 == 1 {
                        Some("China_MiG".to_string())
                    } else {
                        Some("China_BattlemasterTank".to_string())
                    }
                } else {
                    Some("China_BattlemasterTank".to_string())
                }
            }

            // GLA units
            (Team::GLA, GamePhase::Early) => {
                // Early game: Cheap fast units
                if military_count % 3 == 0 {
                    Some("GLA_Technical".to_string())
                } else {
                    Some("GLA_Soldier".to_string())
                }
            }
            (Team::GLA, GamePhase::Mid) => {
                // Mid game: Scorpion tanks
                if military_count % 3 == 0 {
                    Some("GLA_ScorpionTank".to_string())
                } else if military_count % 3 == 1 {
                    Some("GLA_RPGTrooper".to_string())
                } else {
                    Some("GLA_Technical".to_string())
                }
            }
            (Team::GLA, GamePhase::Late) => {
                // Late game: Marauder tanks
                if player.resources.supplies > 1300 {
                    if military_count % 4 == 0 {
                        Some("GLA_MarauderTank".to_string())
                    } else {
                        Some("GLA_ScorpionTank".to_string())
                    }
                } else {
                    Some("GLA_ScorpionTank".to_string())
                }
            }

            _ => None,
        }
    }

    /// Assess threat level at a position
    /// Returns ThreatAssessment with detailed threat information
    pub fn assess_threat(
        game_logic: &GameLogic,
        position: Vec3,
        team: Team,
        scan_radius: f32,
    ) -> ThreatAssessment {
        let mut threat_level = 0.0;
        let mut enemy_count = 0;
        let mut closest_distance = f32::MAX;
        let mut has_anti_air = false;
        let mut has_anti_armor = false;

        for object_id in Self::candidate_object_ids(game_logic, position, scan_radius) {
            let Some(object) = game_logic.find_object(object_id) else {
                continue;
            };
            // Skip non-enemies
            if object.team == team || !object.is_alive() {
                continue;
            }

            let distance = object.get_position().distance(position);

            // Check if within scan radius
            if distance > scan_radius {
                continue;
            }

            // Update closest distance
            if distance < closest_distance {
                closest_distance = distance;
            }

            // Count enemy
            enemy_count += 1;

            // Calculate threat contribution
            let threat_value = Self::calculate_unit_threat_value(object);

            // Closer enemies are more threatening
            let distance_factor = 1.0 - (distance / scan_radius);
            threat_level += threat_value * distance_factor;

            // Check for specialized threats
            if object.is_kind_of(KindOf::Infantry) {
                // Infantry might have anti-air capabilities
                if object.template_name.contains("Missile") || object.template_name.contains("RPG")
                {
                    has_anti_air = true;
                }
            }

            if object.is_kind_of(KindOf::Vehicle) {
                // Vehicles typically have anti-armor
                if object.template_name.contains("Tank") {
                    has_anti_armor = true;
                }
            }
        }

        ThreatAssessment {
            threat_level,
            enemy_count,
            closest_enemy_distance: if closest_distance == f32::MAX {
                0.0
            } else {
                closest_distance
            },
            has_anti_air,
            has_anti_armor,
        }
    }

    /// Calculate threat value of a unit (how dangerous it is)
    fn calculate_unit_threat_value(object: &Object) -> f32 {
        let mut threat = 0.0;

        // Base threat from health
        threat += object.health.current * 0.5;

        // Weapon damage contribution
        if let Some(weapon) = &object.weapon {
            threat += weapon.damage * 2.0;
        }

        // Type modifiers
        if object.is_kind_of(KindOf::Infantry) {
            threat *= 1.0; // Base threat
        } else if object.is_kind_of(KindOf::Vehicle) {
            threat *= 1.5; // Vehicles are more threatening
        } else if object.is_kind_of(KindOf::Aircraft) {
            threat *= 1.3; // Aircraft are fast and dangerous
        } else if object.is_kind_of(KindOf::Structure) {
            threat *= 2.0; // Buildings are high-value targets
        }

        // Veterancy bonus
        threat *= match object.experience.level {
            VeterancyLevel::Rookie => 1.0,
            VeterancyLevel::Veteran => 1.2,
            VeterancyLevel::Elite => 1.5,
            VeterancyLevel::Heroic => 2.0,
        };

        threat
    }

    /// Get target priority level
    fn get_target_priority(object: &Object) -> TargetPriority {
        // Command centers are critical
        if object.is_kind_of(KindOf::CommandCenter) {
            return TargetPriority::Critical;
        }

        // Supply centers and production buildings are high priority
        if object.is_kind_of(KindOf::SupplyCenter) {
            return TargetPriority::High;
        }

        // Military units are medium priority
        if object.can_attack() {
            return TargetPriority::Medium;
        }

        // Other structures are low priority
        if object.is_kind_of(KindOf::Structure) {
            return TargetPriority::Low;
        }

        // Default medium priority
        TargetPriority::Medium
    }

    /// Count how many allies are attacking a specific target
    fn count_allies_attacking_target(
        game_logic: &GameLogic,
        target_id: ObjectId,
        team: Team,
    ) -> u32 {
        let mut count = 0;

        for object in game_logic.get_objects().values() {
            if object.team == team && object.is_alive() {
                if let Some(current_target) = object.target {
                    if current_target == target_id {
                        count += 1;
                    }
                }
            }
        }

        count
    }

    /// Find nearest movement position toward enemy (for pursuit/engagement)
    pub fn find_movement_target(
        game_logic: &GameLogic,
        current_position: Vec3,
        team: Team,
        search_radius: f32,
    ) -> Option<Vec3> {
        // Find nearest enemy
        let (enemy_id, enemy_distance) =
            Self::find_nearest_enemy(game_logic, current_position, team, search_radius)?;

        // Get enemy position
        let enemy = game_logic.find_object(enemy_id)?;
        let enemy_pos = enemy.get_position();

        // If enemy is close, move to their position
        if enemy_distance < 50.0 {
            return Some(enemy_pos);
        }

        // Otherwise, move toward enemy (but not all the way)
        let direction = (enemy_pos - current_position).normalize();
        let move_distance = 100.0_f32.min(enemy_distance * 0.5); // Move halfway or 100 units
        let target_pos = current_position + direction * move_distance;

        Some(target_pos)
    }

    /// Check if a unit should produce (for production buildings)
    pub fn should_produce_unit(
        game_logic: &GameLogic,
        building_id: ObjectId,
        player_id: u32,
    ) -> bool {
        // Get building
        let building = match game_logic.find_object(building_id) {
            Some(obj) => obj,
            None => return false,
        };

        // Only produce from completed, alive buildings
        if !building.is_constructed() || !building.is_alive() {
            return false;
        }

        // Only production buildings should produce
        if !building.is_kind_of(KindOf::Structure) {
            return false;
        }

        // Check if player has resources
        let player = match game_logic.get_player(player_id) {
            Some(p) => p,
            None => return false,
        };

        // Need at least basic resources to produce
        player.resources.supplies >= 100
    }

    /// Calculate optimal defensive position around a base
    pub fn calculate_defensive_position(
        base_center: Vec3,
        threat_direction: Vec3,
        defense_radius: f32,
    ) -> Vec3 {
        // Position unit between base and threat
        let to_threat = (threat_direction - base_center).normalize();
        base_center + to_threat * defense_radius
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_phase_from_time() {
        assert_eq!(GamePhase::from_time(100.0), GamePhase::Early);
        assert_eq!(GamePhase::from_time(600.0), GamePhase::Mid);
        assert_eq!(GamePhase::from_time(1000.0), GamePhase::Late);
    }

    #[test]
    fn test_threat_assessment_empty() {
        let game_logic = GameLogic::new();
        let threat = AIDecisionSystem::assess_threat(&game_logic, Vec3::ZERO, Team::USA, 100.0);

        assert_eq!(threat.enemy_count, 0);
        assert_eq!(threat.threat_level, 0.0);
    }

    #[test]
    fn test_attack_decision_no_target() {
        use crate::game_logic::thing::ThingTemplate;

        let mut game_logic = GameLogic::new();
        let attacker = Object::new(ThingTemplate::new("TestUnit"), ObjectId(1), Team::USA);
        game_logic.add_object(attacker);
        let decision = AIDecisionSystem::should_attack(
            &game_logic,
            ObjectId(1),
            ObjectId(999), // Non-existent target
        );

        assert_eq!(decision, AttackDecision::FindNewTarget);
    }
}
