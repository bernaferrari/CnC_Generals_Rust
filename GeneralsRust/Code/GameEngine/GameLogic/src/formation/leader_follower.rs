//! Leader-Follower System
//!
//! Manages leadership within formations, follower relationships,
//! and leadership transitions.

use super::{FormationError, FormationResult};
use crate::common::{Coord3D, ObjectID, Real};
use std::collections::HashMap;

/// Leader selection strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaderSelection {
    /// First unit in formation
    FirstUnit,
    /// Unit at center of mass
    CenterMass,
    /// Highest ranking/veteran unit
    HighestRank,
    /// Player explicitly designated leader
    PlayerDesignated,
    /// Front-most unit in formation
    FrontUnit,
    /// Automatic based on unit type priority
    Automatic,
}

impl Default for LeaderSelection {
    fn default() -> Self {
        LeaderSelection::Automatic
    }
}

/// Follower role in relation to leader
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FollowerRole {
    /// Standard follower
    Standard,
    /// Wing position (left/right flank)
    Wing,
    /// Rear guard position
    RearGuard,
    /// Support position
    Support,
    /// Scout ahead of formation
    Scout,
}

/// Leadership transfer reason
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeadershipTransfer {
    /// Leader was killed
    LeaderDeath,
    /// Leader became incapacitated
    LeaderDisabled,
    /// Leader was explicitly changed
    PlayerCommand,
    /// Leader left formation
    LeaderLeft,
    /// Better candidate became available
    BetterCandidate,
}

/// Leader information
#[derive(Debug, Clone)]
pub struct LeaderInfo {
    /// Leader unit ID
    pub unit_id: ObjectID,

    /// Current leader position
    pub position: Coord3D,

    /// Leader heading/orientation
    pub heading: Real,

    /// Leader movement speed
    pub speed: Real,

    /// Leadership quality (0.0 to 1.0)
    pub quality: Real,

    /// Time as leader (frames)
    pub time_as_leader: u32,

    /// Is leader currently active
    pub is_active: bool,
}

/// Follower information
#[derive(Debug, Clone)]
pub struct FollowerInfo {
    /// Follower unit ID
    pub unit_id: ObjectID,

    /// Current position
    pub position: Coord3D,

    /// Target position (formation slot)
    pub target_position: Coord3D,

    /// Role in formation
    pub role: FollowerRole,

    /// Distance from assigned position
    pub deviation: Real,

    /// Movement speed
    pub speed: Real,

    /// Is unit keeping formation
    pub in_formation: bool,
}

/// Leader-Follower System
pub struct LeaderFollowerSystem {
    /// Current leader
    leader: Option<LeaderInfo>,

    /// Backup leaders in priority order
    backup_leaders: Vec<ObjectID>,

    /// All followers
    followers: HashMap<ObjectID, FollowerInfo>,

    /// Leader selection strategy
    selection_strategy: LeaderSelection,

    /// Minimum quality threshold for leadership
    min_leadership_quality: Real,

    /// Leadership stability time (frames before reconsidering)
    leadership_stability: u32,

    /// Current frame
    current_frame: u32,

    /// Unit quality lookup
    unit_qualities: HashMap<ObjectID, Real>,

    /// Unit ranks/veterancy
    unit_ranks: HashMap<ObjectID, u32>,
}

impl LeaderFollowerSystem {
    /// Create new leader-follower system
    pub fn new(selection_strategy: LeaderSelection) -> Self {
        Self {
            leader: None,
            backup_leaders: Vec::new(),
            followers: HashMap::new(),
            selection_strategy,
            min_leadership_quality: 0.3,
            leadership_stability: 150, // 5 seconds at 30fps
            current_frame: 0,
            unit_qualities: HashMap::new(),
            unit_ranks: HashMap::new(),
        }
    }

    /// Select a leader from available units
    pub fn select_leader(&mut self, units: &[ObjectID]) -> FormationResult<ObjectID> {
        if units.is_empty() {
            return Err(FormationError::NoUnits);
        }

        let leader_id = match self.selection_strategy {
            LeaderSelection::FirstUnit => units.first().copied(),
            LeaderSelection::PlayerDesignated => {
                // Check if current leader is still in units
                if let Some(ref leader) = self.leader {
                    if units.contains(&leader.unit_id) {
                        return Ok(leader.unit_id);
                    }
                }
                // Fall back to first unit
                units.first().copied()
            }
            LeaderSelection::HighestRank => self.select_highest_rank_unit(units),
            LeaderSelection::CenterMass | LeaderSelection::FrontUnit => {
                // These require position data, fall back to first unit
                units.first().copied()
            }
            LeaderSelection::Automatic => self.select_automatic_leader(units),
        };

        leader_id.ok_or(FormationError::NoLeader)
    }

    /// Select highest rank unit as leader
    fn select_highest_rank_unit(&self, units: &[ObjectID]) -> Option<ObjectID> {
        units
            .iter()
            .max_by_key(|&&unit_id| self.unit_ranks.get(&unit_id).unwrap_or(&0))
            .copied()
    }

    /// Automatically select best leader
    fn select_automatic_leader(&self, units: &[ObjectID]) -> Option<ObjectID> {
        units
            .iter()
            .max_by(|&&a, &&b| {
                let quality_a = self.unit_qualities.get(&a).unwrap_or(&0.5);
                let quality_b = self.unit_qualities.get(&b).unwrap_or(&0.5);
                let rank_a = self.unit_ranks.get(&a).unwrap_or(&0);
                let rank_b = self.unit_ranks.get(&b).unwrap_or(&0);

                // Combine quality and rank
                let score_a = quality_a + (*rank_a as Real * 0.1);
                let score_b = quality_b + (*rank_b as Real * 0.1);

                score_a
                    .partial_cmp(&score_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .copied()
    }

    /// Set current leader
    pub fn set_leader(
        &mut self,
        unit_id: ObjectID,
        position: Coord3D,
        heading: Real,
        speed: Real,
    ) -> FormationResult<()> {
        // Remove from followers if present
        self.followers.remove(&unit_id);

        self.leader = Some(LeaderInfo {
            unit_id,
            position,
            heading,
            speed,
            quality: self.unit_qualities.get(&unit_id).copied().unwrap_or(0.7),
            time_as_leader: 0,
            is_active: true,
        });

        // Update backup leaders
        self.update_backup_leaders();

        Ok(())
    }

    /// Get current leader
    pub fn get_leader(&self) -> Option<&LeaderInfo> {
        self.leader.as_ref()
    }

    /// Get leader ID
    pub fn get_leader_id(&self) -> Option<ObjectID> {
        self.leader.as_ref().map(|l| l.unit_id)
    }

    /// Update leader position and state
    pub fn update_leader(
        &mut self,
        position: Coord3D,
        heading: Real,
        speed: Real,
    ) -> FormationResult<()> {
        if let Some(ref mut leader) = self.leader {
            leader.position = position;
            leader.heading = heading;
            leader.speed = speed;
            leader.time_as_leader += 1;
            Ok(())
        } else {
            Err(FormationError::NoLeader)
        }
    }

    /// Add or update follower
    pub fn add_follower(
        &mut self,
        unit_id: ObjectID,
        position: Coord3D,
        target_position: Coord3D,
        role: FollowerRole,
        speed: Real,
    ) -> FormationResult<()> {
        let deviation = Self::calculate_distance(&position, &target_position);
        let in_formation = deviation < 100.0; // Within 100 units

        self.followers.insert(
            unit_id,
            FollowerInfo {
                unit_id,
                position,
                target_position,
                role,
                deviation,
                speed,
                in_formation,
            },
        );

        Ok(())
    }

    /// Remove follower
    pub fn remove_follower(&mut self, unit_id: ObjectID) -> bool {
        self.followers.remove(&unit_id).is_some()
    }

    /// Get follower information
    pub fn get_follower(&self, unit_id: ObjectID) -> Option<&FollowerInfo> {
        self.followers.get(&unit_id)
    }

    /// Get all followers
    pub fn get_all_followers(&self) -> Vec<&FollowerInfo> {
        self.followers.values().collect()
    }

    /// Update follower position
    pub fn update_follower(
        &mut self,
        unit_id: ObjectID,
        position: Coord3D,
        target_position: Coord3D,
    ) -> FormationResult<()> {
        if let Some(follower) = self.followers.get_mut(&unit_id) {
            follower.position = position;
            follower.target_position = target_position;
            follower.deviation = Self::calculate_distance(&position, &target_position);
            follower.in_formation = follower.deviation < 100.0;
            Ok(())
        } else {
            Err(FormationError::UnitNotInFormation)
        }
    }

    /// Transfer leadership to new unit
    pub fn transfer_leadership(
        &mut self,
        reason: LeadershipTransfer,
        units: &[ObjectID],
    ) -> FormationResult<ObjectID> {
        let old_leader = self.leader.take();
        let old_leader_id = old_leader.as_ref().map(|info| info.unit_id);

        // Add old leader to followers if still alive
        if let Some(ref old_leader_info) = old_leader {
            if reason != LeadershipTransfer::LeaderDeath && units.contains(&old_leader_info.unit_id)
            {
                self.followers.insert(
                    old_leader_info.unit_id,
                    FollowerInfo {
                        unit_id: old_leader_info.unit_id,
                        position: old_leader_info.position,
                        target_position: old_leader_info.position,
                        role: FollowerRole::Standard,
                        deviation: 0.0,
                        speed: old_leader_info.speed,
                        in_formation: true,
                    },
                );
            }
        }

        let leaderless_units: Vec<ObjectID> = match (reason, old_leader_id) {
            (LeadershipTransfer::LeaderDeath, Some(leader_id)) => units
                .iter()
                .copied()
                .filter(|unit_id| *unit_id != leader_id)
                .collect(),
            _ => units.to_vec(),
        };

        // Try backup leaders first
        if let Some(&backup_id) = self.backup_leaders.first() {
            if leaderless_units.contains(&backup_id) {
                self.backup_leaders.remove(0);
                return Ok(backup_id);
            }
        }

        // Select new leader
        self.select_leader(&leaderless_units)
    }

    /// Update backup leaders list
    fn update_backup_leaders(&mut self) {
        // Sort followers by quality to determine backup leaders
        let mut candidates: Vec<_> = self
            .followers
            .keys()
            .map(|&id| {
                let quality = self.unit_qualities.get(&id).copied().unwrap_or(0.5);
                let rank = self.unit_ranks.get(&id).copied().unwrap_or(0);
                (id, quality + rank as Real * 0.1)
            })
            .collect();

        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        self.backup_leaders = candidates
            .into_iter()
            .take(3) // Keep top 3 backups
            .map(|(id, _)| id)
            .collect();
    }

    /// Check if leadership should be reconsidered
    pub fn should_reconsider_leadership(&self) -> bool {
        if let Some(ref leader) = self.leader {
            // Reconsider if leader quality is too low
            if leader.quality < self.min_leadership_quality {
                return true;
            }

            // Reconsider if leadership has been stable long enough
            if leader.time_as_leader > self.leadership_stability {
                // Check if there's a significantly better candidate
                if let Some(&backup_id) = self.backup_leaders.first() {
                    let backup_quality =
                        self.unit_qualities.get(&backup_id).copied().unwrap_or(0.5);
                    if backup_quality > leader.quality + 0.2 {
                        return true;
                    }
                }
            }

            false
        } else {
            true // No leader, definitely reconsider
        }
    }

    /// Set unit quality for leadership consideration
    pub fn set_unit_quality(&mut self, unit_id: ObjectID, quality: Real) {
        self.unit_qualities.insert(unit_id, quality.clamp(0.0, 1.0));
    }

    /// Set unit rank/veterancy
    pub fn set_unit_rank(&mut self, unit_id: ObjectID, rank: u32) {
        self.unit_ranks.insert(unit_id, rank);
    }

    /// Get formation coherence (how well followers maintain formation)
    pub fn get_formation_coherence(&self) -> Real {
        if self.followers.is_empty() {
            return 1.0;
        }

        let in_formation_count = self.followers.values().filter(|f| f.in_formation).count();

        in_formation_count as Real / self.followers.len() as Real
    }

    /// Get average follower deviation
    pub fn get_average_deviation(&self) -> Real {
        if self.followers.is_empty() {
            return 0.0;
        }

        let total_deviation: Real = self.followers.values().map(|f| f.deviation).sum();

        total_deviation / self.followers.len() as Real
    }

    /// Update frame counter
    pub fn update(&mut self, frame: u32) {
        self.current_frame = frame;

        // Age leader
        if let Some(ref mut leader) = self.leader {
            leader.time_as_leader += 1;
        }

        // Check if backup leaders need updating
        if frame % 30 == 0 {
            // Every second
            self.update_backup_leaders();
        }
    }

    /// Calculate distance between two positions
    fn calculate_distance(a: &Coord3D, b: &Coord3D) -> Real {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.leader = None;
        self.backup_leaders.clear();
        self.followers.clear();
        self.unit_qualities.clear();
        self.unit_ranks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leader_selection() {
        let mut system = LeaderFollowerSystem::new(LeaderSelection::FirstUnit);
        let units = vec![100, 101, 102];

        let leader_id = system.select_leader(&units).unwrap();
        assert_eq!(leader_id, 100);
    }

    #[test]
    fn test_set_leader() {
        let mut system = LeaderFollowerSystem::new(LeaderSelection::Automatic);
        let position = Coord3D::new(0.0, 0.0, 0.0);

        system.set_leader(100, position, 0.0, 10.0).unwrap();

        assert_eq!(system.get_leader_id(), Some(100));
    }

    #[test]
    fn test_followers() {
        let mut system = LeaderFollowerSystem::new(LeaderSelection::FirstUnit);

        system
            .add_follower(
                101,
                Coord3D::new(10.0, 0.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
                FollowerRole::Standard,
                10.0,
            )
            .unwrap();

        assert_eq!(system.followers.len(), 1);
        assert!(system.get_follower(101).is_some());
    }

    #[test]
    fn test_leadership_transfer() {
        let mut system = LeaderFollowerSystem::new(LeaderSelection::FirstUnit);
        let units = vec![100, 101, 102];

        system
            .set_leader(100, Coord3D::new(0.0, 0.0, 0.0), 0.0, 10.0)
            .unwrap();
        system
            .add_follower(
                101,
                Coord3D::new(10.0, 0.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
                FollowerRole::Standard,
                10.0,
            )
            .unwrap();

        let new_leader = system
            .transfer_leadership(LeadershipTransfer::LeaderDeath, &units)
            .unwrap();
        assert_eq!(new_leader, 101);
    }

    #[test]
    fn test_formation_coherence() {
        let mut system = LeaderFollowerSystem::new(LeaderSelection::FirstUnit);

        // Add followers, some in formation, some not
        system
            .add_follower(
                101,
                Coord3D::new(10.0, 0.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
                FollowerRole::Standard,
                10.0,
            )
            .unwrap();

        system
            .add_follower(
                102,
                Coord3D::new(5.0, 0.0, 0.0),
                Coord3D::new(0.0, 0.0, 0.0),
                FollowerRole::Standard,
                10.0,
            )
            .unwrap();

        let coherence = system.get_formation_coherence();
        assert!(coherence >= 0.0 && coherence <= 1.0);
    }
}
