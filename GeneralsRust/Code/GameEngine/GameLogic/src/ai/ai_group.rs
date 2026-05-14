//! AIGroup - Unit group management and coordination
//!
//! This module implements AI group management for coordinated unit actions,
//! formation movement, group pathfinding, and tactical coordination.
//! Groups allow AI to manage multiple units as a single entity.
//!
//! Author: Converted from C++ original

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};
use crate::common::{Coord2D, Coord3D, KindOf, ObjectID, Real};
use crate::helpers::TheGameLogic;
use crate::object::registry::OBJECT_REGISTRY;
use crate::ai::{AiError, GuardMode, AI, AttitudeType, AiCommandInterface, AiData, AiCommandParams, AiCommandType, THE_AI};

/// Formation types for group movement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationType {
    None,           // No formation
    Line,           // Units in a line
    Column,         // Units in a column
    Wedge,          // V-shaped formation
    Diamond,        // Diamond formation
    Circle,         // Circular formation
    Square,         // Square formation
    Custom,         // Custom formation defined by positions
}

impl Default for FormationType {
    fn default() -> Self {
        FormationType::None
    }
}

/// Group movement speed calculation methods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupSpeedMethod {
    SlowestMember,   // Group moves at speed of slowest member
    AverageSpeed,    // Group moves at average speed of all members
    FastestMember,   // Group moves at speed of fastest member (units may lag behind)
    WeightedAverage, // Weighted average based on unit importance
}

/// Group combat stance
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupCombatStance {
    Defensive,       // Defensive posture
    Aggressive,      // Aggressive posture
    Balanced,        // Balanced approach
    Evasive,         // Try to avoid combat
    Berserk,         // All-out attack mode
}

impl Default for GroupCombatStance {
    fn default() -> Self {
        GroupCombatStance::Balanced
    }
}

/// Individual member information within a group
#[derive(Debug, Clone)]
pub struct GroupMember {
    pub object_id: ObjectID,
    pub formation_position: Coord2D,    // Position within formation
    pub role: GroupMemberRole,          // Role within the group
    pub priority: i32,                  // Priority within group (0 = highest)
    pub last_known_position: Coord3D,   // Last known world position
    pub status: GroupMemberStatus,      // Current status
    pub health_percentage: f32,         // Health as percentage (0.0 to 1.0)
    pub combat_effectiveness: f32,      // Combat capability (0.0 to 1.0)
}

/// Roles that units can have within a group
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMemberRole {
    Leader,          // Group leader (pathfinding, decision making)
    Fighter,         // Primary combat unit
    Support,         // Support unit (repair, supply, etc.)
    Scout,           // Reconnaissance unit
    Heavy,           // Heavy assault unit
    Medic,           // Medical support
    Engineer,        // Engineering support
    Transport,       // Transport vehicle
    Artillery,       // Long-range support
    AntiAir,         // Anti-aircraft specialist
}

/// Status of individual group members
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupMemberStatus {
    Active,          // Ready and active
    Moving,          // Currently moving
    InCombat,        // Engaged in combat
    Injured,         // Damaged but functional
    Retreating,      // Pulling back
    Disabled,        // Temporarily disabled
    Dead,            // Unit destroyed
    Separated,       // Lost contact with group
}

/// Group tactical formation data
#[derive(Debug, Clone)]
pub struct FormationData {
    pub formation_type: FormationType,
    pub formation_width: Real,          // Width of formation
    pub formation_depth: Real,          // Depth of formation
    pub unit_spacing: Real,             // Spacing between units
    pub leader_position: FormationPosition, // Where leader should be
    pub custom_positions: Vec<Coord2D>, // Custom formation positions
    pub auto_adjust: bool,              // Auto-adjust formation for terrain
}

/// Leader position within formation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationPosition {
    Front,           // Leader at front
    Center,          // Leader in center
    Rear,            // Leader at rear
    Side,            // Leader on side
}

impl Default for FormationData {
    fn default() -> Self {
        Self {
            formation_type: FormationType::None,
            formation_width: 50.0,
            formation_depth: 50.0,
            unit_spacing: 20.0,
            leader_position: FormationPosition::Front,
            custom_positions: Vec::new(),
            auto_adjust: true,
        }
    }
}

/// Group pathfinding options
#[derive(Debug, Clone)]
pub struct GroupPathfindingOptions {
    pub use_group_pathfinding: bool,    // Use group pathfinding vs individual
    pub pathfind_leader_only: bool,     // Only pathfind for leader
    pub formation_pathfinding: bool,    // Maintain formation while pathing
    pub obstacle_avoidance: bool,       // Advanced obstacle avoidance
    pub prefer_roads: bool,             // Prefer to use roads when available
    pub max_path_deviation: Real,       // Max deviation from optimal path
    pub repath_frequency: u32,          // Frames between repath attempts
}

impl Default for GroupPathfindingOptions {
    fn default() -> Self {
        Self {
            use_group_pathfinding: true,
            pathfind_leader_only: false,
            formation_pathfinding: true,
            obstacle_avoidance: true,
            prefer_roads: false,
            max_path_deviation: 100.0,
            repath_frequency: 30, // Every second
        }
    }
}

/// Group coordination state
#[derive(Debug, Clone)]
pub struct GroupCoordination {
    pub coordination_level: f32,        // How well coordinated (0.0 to 1.0)
    pub last_coordination_update: u32,  // Frame of last coordination update
    pub leader_id: Option<ObjectID>,    // Current group leader
    pub backup_leaders: Vec<ObjectID>,  // Backup leaders in priority order
    pub command_coherence: f32,         // How well group follows commands (0.0 to 1.0)
    pub morale_level: f32,             // Group morale (0.0 to 1.0)
    pub experience_level: f32,          // Group experience (0.0 to 1.0)
}

impl Default for GroupCoordination {
    fn default() -> Self {
        Self {
            coordination_level: 0.8,
            last_coordination_update: 0,
            leader_id: None,
            backup_leaders: Vec::new(),
            command_coherence: 0.9,
            morale_level: 0.8,
            experience_level: 0.5,
        }
    }
}

/// Main AI Group structure
#[derive(Debug)]
pub struct AiGroup {
    /// Unique group identifier
    id: u32,
    
    /// Group members
    members: HashMap<ObjectID, GroupMember>,
    
    /// Formation and positioning
    formation: FormationData,
    
    /// Pathfinding configuration
    pathfinding_options: GroupPathfindingOptions,
    
    /// Group coordination state
    coordination: GroupCoordination,
    
    /// Group properties
    group_speed: Real,
    dirty_speed: bool,                  // Speed needs recalculation
    speed_method: GroupSpeedMethod,
    
    /// Combat configuration  
    combat_stance: GroupCombatStance,
    attitude: AttitudeType,
    
    /// Current group state
    current_position: Coord3D,          // Center position of group
    target_position: Option<Coord3D>,   // Where group is moving to
    current_path: Option<Vec<Coord3D>>, // Current movement path
    
    /// Group status
    is_moving: bool,
    is_in_combat: bool,
    is_disbanded: bool,
    
    /// Timing and updates
    last_update_frame: u32,
    last_position_update: u32,
    last_formation_update: u32,
    
    /// Statistics
    total_damage_dealt: f32,
    total_damage_taken: f32,
    enemies_destroyed: u32,
    distance_traveled: f32,
    
    /// Special abilities and upgrades
    group_abilities: HashSet<String>,   // Special group abilities
    applied_upgrades: HashSet<String>,  // Upgrades applied to group
}

impl AiGroup {
    /// Create new AI group
    pub fn new(id: u32) -> Self {
        Self {
            id,
            members: HashMap::new(),
            formation: FormationData::default(),
            pathfinding_options: GroupPathfindingOptions::default(),
            coordination: GroupCoordination::default(),
            group_speed: 0.0,
            dirty_speed: true,
            speed_method: GroupSpeedMethod::SlowestMember,
            combat_stance: GroupCombatStance::default(),
            attitude: AttitudeType::Normal,
            current_position: Coord3D::new(0.0, 0.0, 0.0),
            target_position: None,
            current_path: None,
            is_moving: false,
            is_in_combat: false,
            is_disbanded: false,
            last_update_frame: 0,
            last_position_update: 0,
            last_formation_update: 0,
            total_damage_dealt: 0.0,
            total_damage_taken: 0.0,
            enemies_destroyed: 0,
            distance_traveled: 0.0,
            group_abilities: HashSet::new(),
            applied_upgrades: HashSet::new(),
        }
    }

    /// Get group ID
    pub fn get_id(&self) -> u32 {
        self.id
    }

    /// Add unit to group
    pub fn add_member(&mut self, object_id: ObjectID) -> Result<(), AiError> {
        if self.members.contains_key(&object_id) {
            return Err(AiError::InvalidObject); // Already in group
        }

        // Determine role for new member based on unit type
        let role = self.determine_member_role(object_id)?;
        
        // Find formation position for new member
        let formation_position = self.find_formation_position_for_new_member()?;

        let member = GroupMember {
            object_id,
            formation_position,
            role,
            priority: self.calculate_member_priority(object_id, role)?,
            last_known_position: Coord3D::new(0.0, 0.0, 0.0), // Will be updated
            status: GroupMemberStatus::Active,
            health_percentage: 1.0,
            combat_effectiveness: 1.0,
        };

        self.members.insert(object_id, member);
        
        // Update group properties
        self.dirty_speed = true;
        self.update_group_leader()?;
        self.update_formation()?;

        Ok(())
    }

    /// Remove unit from group
    pub fn remove_member(&mut self, object_id: ObjectID) -> Result<bool, AiError> {
        if let Some(_member) = self.members.remove(&object_id) {
            // Update group properties
            self.dirty_speed = true;
            
            // Check if removed unit was the leader
            if self.coordination.leader_id == Some(object_id) {
                self.select_new_leader()?;
            }
            
            // Update formation
            self.update_formation()?;
            
            // Return true if group is now empty (should be disbanded)
            Ok(self.members.is_empty())
        } else {
            Ok(false)
        }
    }

    /// Check if unit is member of this group
    pub fn is_member(&self, object_id: ObjectID) -> bool {
        self.members.contains_key(&object_id)
    }

    /// Get number of members in group
    pub fn get_count(&self) -> usize {
        self.members.len()
    }

    /// Check if group is empty
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Get all member IDs
    pub fn get_all_ids(&self) -> Vec<ObjectID> {
        self.members.keys().copied().collect()
    }

    /// Get group center position
    pub fn get_center(&mut self) -> Result<Coord3D, AiError> {
        if self.members.is_empty() {
            return Err(AiError::EmptyGroup);
        }

        self.update_positions()?;
        Ok(self.current_position)
    }

    /// Get group speed
    pub fn get_speed(&mut self) -> Result<Real, AiError> {
        if self.dirty_speed {
            self.recalculate_group_speed()?;
        }
        Ok(self.group_speed)
    }

    /// Set group formation
    pub fn set_formation(&mut self, formation_type: FormationType) -> Result<(), AiError> {
        if self.formation.formation_type != formation_type {
            self.formation.formation_type = formation_type;
            self.update_formation()?;
        }
        Ok(())
    }

    /// Set group combat stance
    pub fn set_combat_stance(&mut self, stance: GroupCombatStance) {
        self.combat_stance = stance;
    }

    /// Set group attitude
    pub fn set_attitude(&mut self, attitude: AttitudeType) {
        self.attitude = attitude;
    }

    /// Get group attitude
    pub fn get_attitude(&self) -> AttitudeType {
        self.attitude
    }

    /// Check if group is idle
    pub fn is_idle(&self) -> bool {
        !self.is_moving && !self.is_in_combat
    }

    /// Check if group is busy
    pub fn is_busy(&self) -> bool {
        self.is_moving || self.is_in_combat
    }

    /// Move group to position
    pub fn move_to_position(&mut self, position: Coord3D, add_waypoint: bool) -> Result<(), AiError> {
        self.target_position = Some(position);
        self.is_moving = true;
        
        // Calculate path
        self.calculate_group_path(position)?;
        
        // Update formation for movement
        if self.pathfinding_options.formation_pathfinding {
            self.calculate_formation_positions_for_move(position)?;
        }

        Ok(())
    }

    /// Attack target object
    pub fn attack_object(&mut self, target_id: ObjectID, max_shots: i32) -> Result<(), AiError> {
        // Set group to combat mode
        self.is_in_combat = true;
        
        // Coordinate attack among group members
        self.coordinate_attack(target_id, max_shots)?;
        
        Ok(())
    }

    /// Guard position
    pub fn guard_position(&mut self, position: Coord3D, guard_mode: GuardMode) -> Result<(), AiError> {
        self.target_position = Some(position);
        self.combat_stance = match guard_mode {
            GuardMode::GuardWithoutPursuit => GroupCombatStance::Defensive,
            GuardMode::GuardFlyingUnitsOnly => GroupCombatStance::Balanced,
            _ => GroupCombatStance::Balanced,
        };
        
        // Set up defensive formation
        self.setup_defensive_formation(position)?;
        
        Ok(())
    }

    /// Update group (called each frame)
    pub fn update(&mut self, current_frame: u32) -> Result<(), AiError> {
        self.last_update_frame = current_frame;
        
        // Update member positions and status
        self.update_member_status()?;
        
        // Update group position
        self.update_positions()?;
        
        // Update formation if needed
        if current_frame - self.last_formation_update > 30 { // Update every second
            self.update_formation()?;
            self.last_formation_update = current_frame;
        }
        
        // Update coordination
        self.update_coordination()?;
        
        // Check for movement completion
        if self.is_moving {
            self.check_movement_completion()?;
        }
        
        // Update combat status
        if self.is_in_combat {
            self.update_combat_status()?;
        }

        Ok(())
    }

    /// Disband the group
    pub fn disband(&mut self) -> Result<(), AiError> {
        self.is_disbanded = true;
        self.members.clear();
        Ok(())
    }

    // Private implementation methods

    /// Determine role for new member based on unit type
    fn determine_member_role(&self, object_id: ObjectID) -> Result<GroupMemberRole, AiError> {
        let Some(object) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(AiError::InvalidObject);
        };
        let object_guard = object.read().map_err(|_| AiError::LockFailed)?;

        if self.members.is_empty() {
            return Ok(GroupMemberRole::Leader);
        }

        if object_guard.is_kind_of(KindOf::Transport)
            || object_guard.is_kind_of(KindOf::AmphibiousTransport)
        {
            return Ok(GroupMemberRole::Transport);
        }

        if object_guard.is_kind_of(KindOf::Dozer)
            || object_guard.get_template_name().to_ascii_lowercase().contains("engineer")
        {
            return Ok(GroupMemberRole::Engineer);
        }

        let template_name = object_guard.get_template_name().to_ascii_lowercase();
        if template_name.contains("medic") || template_name.contains("ambulance") {
            return Ok(GroupMemberRole::Medic);
        }
        if template_name.contains("scout")
            || template_name.contains("recon")
            || template_name.contains("radar")
        {
            return Ok(GroupMemberRole::Scout);
        }
        if template_name.contains("artillery")
            || template_name.contains("howitzer")
            || template_name.contains("scud")
            || template_name.contains("rocket")
            || template_name.contains("mortar")
        {
            return Ok(GroupMemberRole::Artillery);
        }
        if (template_name.contains("anti") && template_name.contains("air"))
            || template_name.contains("stinger")
            || template_name.contains("avenger")
            || template_name.contains("gatling")
        {
            return Ok(GroupMemberRole::AntiAir);
        }

        if object_guard.is_kind_of(KindOf::Aircraft) {
            return Ok(GroupMemberRole::Scout);
        }
        if object_guard.is_kind_of(KindOf::Infantry) {
            return Ok(GroupMemberRole::Fighter);
        }
        if object_guard.is_kind_of(KindOf::Vehicle) {
            let damage = object_guard.get_max_damage_potential();
            if damage >= 250.0 {
                return Ok(GroupMemberRole::Heavy);
            }
            if damage > 0.0 {
                return Ok(GroupMemberRole::Fighter);
            }
            return Ok(GroupMemberRole::Support);
        }
        if object_guard.is_kind_of(KindOf::Structure) {
            return Ok(GroupMemberRole::Support);
        }

        if object_guard.has_any_weapon() {
            return Ok(GroupMemberRole::Fighter);
        }

        Ok(GroupMemberRole::Support)
    }

    /// Find formation position for new member
    fn find_formation_position_for_new_member(&self) -> Result<Coord2D, AiError> {
        let member_count = self.members.len();
        
        match self.formation.formation_type {
            FormationType::Line => {
                let x = (member_count as Real) * self.formation.unit_spacing;
                Ok([x, 0.0])
            }
            FormationType::Column => {
                let y = (member_count as Real) * self.formation.unit_spacing;
                Ok([0.0, y])
            }
            FormationType::Circle => {
                let angle = (member_count as Real) * 2.0 * std::f32::consts::PI / 8.0; // Assume max 8 units
                let radius = self.formation.formation_width * 0.5;
                let x = radius * angle.cos();
                let y = radius * angle.sin();
                Ok([x, y])
            }
            _ => {
                // Default spacing
                let x = (member_count % 3) as Real * self.formation.unit_spacing;
                let y = (member_count / 3) as Real * self.formation.unit_spacing;
                Ok([x, y])
            }
        }
    }

    /// Calculate member priority within group
    fn calculate_member_priority(&self, object_id: ObjectID, role: GroupMemberRole) -> Result<i32, AiError> {
        let base_priority = match role {
            GroupMemberRole::Leader => 0,
            GroupMemberRole::Heavy => 1,
            GroupMemberRole::Fighter => 2,
            GroupMemberRole::Support => 3,
            GroupMemberRole::Artillery => 4,
            GroupMemberRole::AntiAir => 5,
            GroupMemberRole::Scout => 6,
            GroupMemberRole::Medic => 7,
            GroupMemberRole::Engineer => 8,
            GroupMemberRole::Transport => 9,
        };
        
        Ok(base_priority)
    }

    /// Update group leader selection
    fn update_group_leader(&mut self) -> Result<(), AiError> {
        if self.coordination.leader_id.is_none() {
            self.select_new_leader()?;
        }
        Ok(())
    }

    /// Select new group leader
    fn select_new_leader(&mut self) -> Result<(), AiError> {
        // Find best candidate for leader
        let leader_candidate = self.members.iter()
            .filter(|(_, member)| member.status == GroupMemberStatus::Active)
            .min_by_key(|(_, member)| member.priority)
            .map(|(id, _)| *id);
            
        self.coordination.leader_id = leader_candidate;
        
        Ok(())
    }

    /// Update formation positions
    fn update_formation(&mut self) -> Result<(), AiError> {
        if self.members.len() < 2 {
            return Ok(()); // No formation needed for single unit
        }

        match self.formation.formation_type {
            FormationType::Line => self.arrange_line_formation()?,
            FormationType::Column => self.arrange_column_formation()?,
            FormationType::Wedge => self.arrange_wedge_formation()?,
            FormationType::Diamond => self.arrange_diamond_formation()?,
            FormationType::Circle => self.arrange_circle_formation()?,
            FormationType::Square => self.arrange_square_formation()?,
            FormationType::Custom => self.arrange_custom_formation()?,
            FormationType::None => {} // No formation
        }

        Ok(())
    }

    /// Arrange line formation
    fn arrange_line_formation(&mut self) -> Result<(), AiError> {
        let mut sorted_members: Vec<_> = self.members.iter_mut().collect();
        sorted_members.sort_by_key(|(_, member)| member.priority);

        for (i, (_, member)) in sorted_members.iter_mut().enumerate() {
            let x = (i as Real) * self.formation.unit_spacing;
            member.formation_position = [x, 0.0];
        }

        Ok(())
    }

    /// Arrange column formation
    fn arrange_column_formation(&mut self) -> Result<(), AiError> {
        let mut sorted_members: Vec<_> = self.members.iter_mut().collect();
        sorted_members.sort_by_key(|(_, member)| member.priority);

        for (i, (_, member)) in sorted_members.iter_mut().enumerate() {
            let y = (i as Real) * self.formation.unit_spacing;
            member.formation_position = [0.0, y];
        }

        Ok(())
    }

    /// Arrange wedge formation
    fn arrange_wedge_formation(&mut self) -> Result<(), AiError> {
        let mut sorted_members: Vec<_> = self.members.iter_mut().collect();
        sorted_members.sort_by_key(|(_, member)| member.priority);

        for (i, (_, member)) in sorted_members.iter_mut().enumerate() {
            if i == 0 {
                // Leader at front
                member.formation_position = [0.0, 0.0];
            } else {
                // Others form the wedge
                let side = if i % 2 == 1 { -1.0 } else { 1.0 };
                let rank = (i + 1) / 2;
                let x = side * (rank as Real) * self.formation.unit_spacing;
                let y = (rank as Real) * self.formation.unit_spacing;
                member.formation_position = [x, y];
            }
        }

        Ok(())
    }

    /// Arrange diamond formation
    fn arrange_diamond_formation(&mut self) -> Result<(), AiError> {
        let count = self.members.len();
        if count < 4 {
            return self.arrange_line_formation(); // Fall back to line for small groups
        }

        let mut sorted_members: Vec<_> = self.members.iter_mut().collect();
        sorted_members.sort_by_key(|(_, member)| member.priority);

        let spacing = self.formation.unit_spacing;
        
        // Diamond positions: front, left, right, back, then fill in
        let positions = vec![
            [0.0, 0.0],           // Front
            [-spacing, spacing],   // Left
            [spacing, spacing],    // Right
            [0.0, spacing * 2.0],  // Back
        ];

        for (i, (_, member)) in sorted_members.iter_mut().enumerate() {
            if i < positions.len() {
                member.formation_position = positions[i];
            } else {
                // Additional units fill in the diamond
                let extra_index = i - 4;
                let x = ((extra_index % 2) as Real - 0.5) * spacing * 0.5;
                let y = spacing + (extra_index / 2) as Real * spacing * 0.5;
                member.formation_position = [x, y];
            }
        }

        Ok(())
    }

    /// Arrange circle formation
    fn arrange_circle_formation(&mut self) -> Result<(), AiError> {
        let count = self.members.len();
        let radius = self.formation.formation_width * 0.5;
        
        for (i, (_, member)) in self.members.iter_mut().enumerate() {
            let angle = (i as Real) * 2.0 * std::f32::consts::PI / (count as Real);
            let x = radius * angle.cos();
            let y = radius * angle.sin();
            member.formation_position = [x, y];
        }

        Ok(())
    }

    /// Arrange square formation
    fn arrange_square_formation(&mut self) -> Result<(), AiError> {
        let count = self.members.len();
        let side_length = (count as Real).sqrt().ceil() as usize;
        
        for (i, (_, member)) in self.members.iter_mut().enumerate() {
            let row = i / side_length;
            let col = i % side_length;
            let x = (col as Real) * self.formation.unit_spacing;
            let y = (row as Real) * self.formation.unit_spacing;
            member.formation_position = [x, y];
        }

        Ok(())
    }

    /// Arrange custom formation
    fn arrange_custom_formation(&mut self) -> Result<(), AiError> {
        let positions = &self.formation.custom_positions;
        
        for (i, (_, member)) in self.members.iter_mut().enumerate() {
            if i < positions.len() {
                member.formation_position = positions[i];
            } else {
                // Fall back to default positioning
                let x = (i as Real) * self.formation.unit_spacing;
                member.formation_position = [x, 0.0];
            }
        }

        Ok(())
    }

    /// Recalculate group speed based on members
    fn recalculate_group_speed(&mut self) -> Result<(), AiError> {
        if self.members.is_empty() {
            self.group_speed = 0.0;
            self.dirty_speed = false;
            return Ok(());
        }

        // Get speeds of all active members
        let member_speeds: Vec<Real> = self.members.values()
            .filter(|member| member.status == GroupMemberStatus::Active)
            .map(|member| self.get_member_speed(member.object_id).unwrap_or(100.0))
            .collect();

        if member_speeds.is_empty() {
            self.group_speed = 0.0;
        } else {
            self.group_speed = match self.speed_method {
                GroupSpeedMethod::SlowestMember => {
                    member_speeds.iter().fold(f32::INFINITY, |a, &b| a.min(b))
                }
                GroupSpeedMethod::FastestMember => {
                    member_speeds.iter().fold(0.0, |a, &b| a.max(b))
                }
                GroupSpeedMethod::AverageSpeed => {
                    member_speeds.iter().sum::<Real>() / member_speeds.len() as Real
                }
                GroupSpeedMethod::WeightedAverage => {
                    // Weight by priority (lower priority = higher weight)
                    let mut total_weighted_speed = 0.0;
                    let mut total_weight = 0.0;
                    
                    for (speed, member) in member_speeds.iter().zip(self.members.values()) {
                        let weight = 1.0 / (member.priority as f32 + 1.0);
                        total_weighted_speed += speed * weight;
                        total_weight += weight;
                    }
                    
                    if total_weight > 0.0 {
                        total_weighted_speed / total_weight
                    } else {
                        100.0 // Default speed
                    }
                }
            };
        }

        self.dirty_speed = false;
        Ok(())
    }

    /// Get speed of individual member
    fn get_member_speed(&self, object_id: ObjectID) -> Result<Real, AiError> {
        let Some(obj) = OBJECT_REGISTRY.get_object(object_id) else {
            return Err(AiError::InvalidObject);
        };
        let guard = obj.read().map_err(|_| AiError::LockFailed)?;
        if let Some(physics) = guard.get_physics() {
            let physics_guard = physics.lock().map_err(|_| AiError::LockFailed)?;
            return Ok(physics_guard.get_velocity().length());
        }
        Ok(0.0)
    }

    /// Update member status and positions
    fn update_member_status(&mut self) -> Result<(), AiError> {
        for (_, member) in &mut self.members {
            if member.status == GroupMemberStatus::Dead {
                continue; // Skip dead members
            }
            
            if let Some(obj) = OBJECT_REGISTRY.get_object(member.object_id) {
                if let Ok(guard) = obj.read() {
                    member.last_known_position = *guard.get_position();
                    member.health_percentage = guard.get_health_percentage();
                } else {
                    member.status = GroupMemberStatus::Dead;
                    continue;
                }
            } else {
                member.status = GroupMemberStatus::Dead;
                continue;
            }
            
            // Update status based on health
            if member.health_percentage <= 0.0 {
                member.status = GroupMemberStatus::Dead;
            } else if member.health_percentage < 0.3 {
                member.status = GroupMemberStatus::Injured;
            }
        }

        // Remove dead members
        self.members.retain(|_, member| member.status != GroupMemberStatus::Dead);

        Ok(())
    }

    /// Update group center position
    fn update_positions(&mut self) -> Result<(), AiError> {
        if self.members.is_empty() {
            return Ok(());
        }

        let mut sum_x = 0.0;
        let mut sum_y = 0.0;
        let mut sum_z = 0.0;
        let mut count = 0;

        for member in self.members.values() {
            sum_x += member.last_known_position[0];
            sum_y += member.last_known_position[1];
            sum_z += member.last_known_position[2];
            count += 1;
        }

        if count > 0 {
            self.current_position = [
                sum_x / count as Real,
                sum_y / count as Real,
                sum_z / count as Real,
            ];
        }

        Ok(())
    }

    /// Update group coordination
    fn update_coordination(&mut self) -> Result<(), AiError> {
        // Calculate coordination based on:
        // - Distance between members
        // - Formation adherence
        // - Command response time
        
        let mut total_distance = 0.0;
        let mut distance_count = 0;
        
        // Calculate average distance from center
        for member in self.members.values() {
            let dx = member.last_known_position[0] - self.current_position[0];
            let dy = member.last_known_position[1] - self.current_position[1];
            let dz = member.last_known_position[2] - self.current_position[2];
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            
            total_distance += distance;
            distance_count += 1;
        }
        
        let average_distance = if distance_count > 0 {
            total_distance / distance_count as Real
        } else {
            0.0
        };
        
        // Coordination decreases with distance (units spread out)
        self.coordination.coordination_level = (200.0 / (average_distance + 50.0)).min(1.0);
        
        Ok(())
    }

    /// Calculate path for group movement
    fn calculate_group_path(&mut self, destination: Coord3D) -> Result<(), AiError> {
        if self.pathfinding_options.use_group_pathfinding {
            // Use group pathfinding
            self.calculate_group_pathfinding(destination)?;
        } else {
            // Use individual pathfinding for each member
            self.calculate_individual_pathfinding(destination)?;
        }
        
        Ok(())
    }

    /// Calculate group pathfinding (single path for group)
    /// C++ reference: AIGroup.cpp line 614 — findGroundPath for the group center,
    /// then distribute formation positions along the path.
    fn calculate_group_pathfinding(&mut self, destination: Coord3D) -> Result<(), AiError> {
        let unit_ids: Vec<ObjectID> = self.members.keys().copied().collect();
        if unit_ids.is_empty() {
            self.current_path = Some(vec![self.current_position, destination]);
            return Ok(());
        }

        let mut unit_positions = HashMap::new();
        for (&id, member) in &self.members {
            unit_positions.insert(id, member.last_known_position);
        }

        let formation = self.formation_to_group_formation();

        if let Ok(mut group_pf) = self.create_group_pathfinder() {
            if let Some(ai_guard) = THE_AI.read().ok() {
                if let Some(pf_system) = ai_guard.pathfinding_system() {
                    if let Ok(pf_sys) = pf_system.read() {
                        let paths = group_pf.find_group_paths(
                            &pf_sys,
                            &unit_ids,
                            &unit_positions,
                            destination,
                            formation,
                            super::pathfind_complete::SURFACE_GROUND,
                            false,
                            self.formation.unit_spacing,
                        );

                        let leader_id = self.coordination.leader_id.unwrap_or(unit_ids[0]);
                        if let Some(leader_result) = paths.get(&leader_id) {
                            if leader_result.success && !leader_result.waypoints.is_empty() {
                                self.current_path = Some(leader_result.waypoints.clone());
                                return Ok(());
                            }
                        }

                        for result in paths.values() {
                            if result.success && !result.waypoints.is_empty() {
                                self.current_path = Some(result.waypoints.clone());
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        self.current_path = Some(vec![self.current_position, destination]);
        Ok(())
    }

    /// C++ reference: AIGroup.cpp friend_moveInfantryToPos / friend_moveToPos —
    /// each member gets its own path to its formation slot at the destination.
    fn calculate_individual_pathfinding(&mut self, destination: Coord3D) -> Result<(), AiError> {
        let unit_ids: Vec<ObjectID> = self.members.keys().copied().collect();
        if unit_ids.is_empty() {
            return Ok(());
        }

        let mut unit_positions = HashMap::new();
        for (&id, member) in &self.members {
            unit_positions.insert(id, member.last_known_position);
        }

        let formation = self.formation_to_group_formation();

        if let Ok(mut group_pf) = self.create_group_pathfinder() {
            if let Some(ai_guard) = THE_AI.read().ok() {
                if let Some(pf_system) = ai_guard.pathfinding_system() {
                    if let Ok(pf_sys) = pf_system.read() {
                        let paths = group_pf.find_group_paths(
                            &pf_sys,
                            &unit_ids,
                            &unit_positions,
                            destination,
                            formation,
                            super::pathfind_complete::SURFACE_GROUND,
                            false,
                            self.formation.unit_spacing,
                        );

                        let leader_id = self.coordination.leader_id.unwrap_or(unit_ids[0]);
                        if let Some(leader_result) = paths.get(&leader_id) {
                            if leader_result.success && !leader_result.waypoints.is_empty() {
                                self.current_path = Some(leader_result.waypoints.clone());
                            }
                        }

                        for (&member_id, member) in &mut self.members {
                            if let Some(result) = paths.get(&member_id) {
                                if result.success && !result.waypoints.is_empty() {
                                    member.last_known_position = result.waypoints[result.waypoints.len() - 1];
                                }
                            }
                        }
                        return Ok(());
                    }
                }
            }
        }

        for (_, member) in &mut self.members {
            let member_destination = self.calculate_member_destination(destination, member)?;
            member.last_known_position = member_destination;
        }

        Ok(())
    }

    /// Calculate destination for individual member based on formation
    fn calculate_member_destination(&self, group_destination: Coord3D, member: &GroupMember) -> Result<Coord3D, AiError> {
        let formation_offset = member.formation_position;
        
        Ok([
            group_destination[0] + formation_offset[0],
            group_destination[1] + formation_offset[1],
            group_destination[2],
        ])
    }

    fn formation_to_group_formation(&self) -> super::group_pathfinding::FormationType {
        match self.formation.formation_type {
            FormationType::None => super::group_pathfinding::FormationType::None,
            FormationType::Line => super::group_pathfinding::FormationType::Line,
            FormationType::Column => super::group_pathfinding::FormationType::Column,
            FormationType::Wedge => super::group_pathfinding::FormationType::Wedge,
            FormationType::Diamond | FormationType::Square => super::group_pathfinding::FormationType::Box,
            FormationType::Circle | FormationType::Custom => super::group_pathfinding::FormationType::Scatter,
        }
    }

    fn create_group_pathfinder(&self) -> Result<super::group_pathfinding::GroupPathfinder, AiError> {
        let spacing = self.formation.unit_spacing.max(10.0);
        let mut gp = super::group_pathfinding::GroupPathfinder::new(spacing);
        if let Some(leader_id) = self.coordination.leader_id {
            gp.set_leader(leader_id);
        }
        Ok(gp)
    }

    /// Calculate formation positions for movement
    fn calculate_formation_positions_for_move(&mut self, destination: Coord3D) -> Result<(), AiError> {
        // Update formation positions based on movement direction
        // This ensures formation is oriented correctly for movement
        
        let direction_x = destination[0] - self.current_position[0];
        let direction_y = destination[1] - self.current_position[1];
        let angle = direction_y.atan2(direction_x);
        
        // Rotate formation to face movement direction
        for (_, member) in &mut self.members {
            let pos = member.formation_position;
            let rotated_x = pos[0] * angle.cos() - pos[1] * angle.sin();
            let rotated_y = pos[0] * angle.sin() + pos[1] * angle.cos();
            member.formation_position = [rotated_x, rotated_y];
        }
        
        Ok(())
    }

    /// Coordinate attack among group members
    fn coordinate_attack(&mut self, target_id: ObjectID, max_shots: i32) -> Result<(), AiError> {
        // Distribute attack among capable members
        let combat_members: Vec<_> = self.members.values()
            .filter(|member| matches!(member.role, 
                GroupMemberRole::Fighter | GroupMemberRole::Heavy | GroupMemberRole::Artillery))
            .collect();
        
        if combat_members.is_empty() {
            return Ok(()); // No combat units
        }
        
        let shots_per_member = max_shots / combat_members.len() as i32;
        
        // Issue attack commands to combat members
        for member in combat_members {
            // This would issue attack command to individual unit
            // For now, just record the intent
        }
        
        Ok(())
    }

    /// Set up defensive formation around position
    fn setup_defensive_formation(&mut self, position: Coord3D) -> Result<(), AiError> {
        // Choose defensive formation based on group composition
        let has_heavy_units = self.members.values()
            .any(|member| member.role == GroupMemberRole::Heavy);
        let has_artillery = self.members.values()
            .any(|member| member.role == GroupMemberRole::Artillery);
        
        if has_artillery {
            // Artillery in back, others in front
            self.set_formation(FormationType::Line)?;
        } else if has_heavy_units {
            // Heavy units form defensive line
            self.set_formation(FormationType::Line)?;
        } else {
            // Default circular defense
            self.set_formation(FormationType::Circle)?;
        }
        
        self.target_position = Some(position);
        Ok(())
    }

    /// Check if movement is complete
    fn check_movement_completion(&mut self) -> Result<(), AiError> {
        if let Some(target) = self.target_position {
            let distance_to_target = {
                let dx = self.current_position[0] - target[0];
                let dy = self.current_position[1] - target[1];
                let dz = self.current_position[2] - target[2];
                (dx * dx + dy * dy + dz * dz).sqrt()
            };
            
            // Consider movement complete when close enough
            let arrival_threshold = 50.0; // Adjustable threshold
            if distance_to_target < arrival_threshold {
                self.is_moving = false;
                self.target_position = None;
                self.current_path = None;
            }
        }
        
        Ok(())
    }

    /// Update combat status
    fn update_combat_status(&mut self) -> Result<(), AiError> {
        // Check if any members are still in combat
        let in_combat = self.members.values()
            .any(|member| member.status == GroupMemberStatus::InCombat);
        
        if !in_combat {
            self.is_in_combat = false;
        }
        
        Ok(())
    }
}

impl AiCommandInterface for AiGroup {
    fn ai_do_command(&mut self, params: &AiCommandParams) -> Result<(), AiError> {
        match params.cmd {
            AiCommandType::MoveToPosition => {
                self.move_to_position(params.pos, false)
            }
            AiCommandType::AttackObject => {
                if let Some(target_id) = params.obj {
                    self.attack_object(target_id, params.int_value)
                } else {
                    Err(AiError::InvalidTarget)
                }
            }
            AiCommandType::GuardPosition => {
                let guard_mode = match params.int_value {
                    0 => GuardMode::Normal,
                    1 => GuardMode::GuardWithoutPursuit,
                    2 => GuardMode::GuardFlyingUnitsOnly,
                    _ => GuardMode::Normal,
                };
                self.guard_position(params.pos, guard_mode)
            }
            AiCommandType::Idle => {
                self.is_moving = false;
                self.is_in_combat = false;
                self.target_position = None;
                Ok(())
            }
            AiCommandType::AttackArea => {
                for member_id in self.members.keys() {
                    if let Some(obj_arc) = TheGameLogic::find_object_by_id(*member_id) {
                        if let Ok(obj_guard) = obj_arc.read() {
                            if let Some(ai_arc) = obj_guard.get_ai_update_interface() {
                                let _ = ai_arc.lock().ok().map(|mut ai| {
                                    let _ = ai.execute_command(params);
                                });
                            }
                        }
                    }
                }
                Ok(())
            }
            _ => {
                // Forward other commands to individual members
                for (object_id, _) in &self.members {
                    // This would send command to individual unit
                    // Implementation depends on your object system
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_group_creation() {
        let group = AiGroup::new(1);
        assert_eq!(group.get_id(), 1);
        assert!(group.is_empty());
        assert_eq!(group.get_count(), 0);
    }

    #[test]
    fn test_group_member_management() {
        let mut group = AiGroup::new(1);
        
        // Add members
        assert!(group.add_member(100).is_ok());
        assert!(group.add_member(101).is_ok());
        assert_eq!(group.get_count(), 2);
        assert!(group.is_member(100));
        assert!(group.is_member(101));
        
        // Remove member
        assert!(!group.remove_member(100).unwrap()); // Should not be empty
        assert_eq!(group.get_count(), 1);
        assert!(!group.is_member(100));
        assert!(group.is_member(101));
        
        // Remove last member
        assert!(group.remove_member(101).unwrap()); // Should be empty
        assert_eq!(group.get_count(), 0);
        assert!(group.is_empty());
    }

    #[test]
    fn test_formation_positioning() {
        let mut group = AiGroup::new(1);
        
        // Add some members
        group.add_member(100).unwrap();
        group.add_member(101).unwrap();
        group.add_member(102).unwrap();
        
        // Test line formation
        group.set_formation(FormationType::Line).unwrap();
        
        // Check that positions were assigned
        for (_, member) in &group.members {
            // Formation positions should be set
            assert!(member.formation_position[0] >= 0.0 || member.formation_position[1] >= 0.0);
        }
    }

    #[test]
    fn test_group_commands() {
        let mut group = AiGroup::new(1);
        group.add_member(100).unwrap();
        
        let params = AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
        let mut move_params = params;
        move_params.pos = [100.0, 200.0, 0.0];
        
        assert!(group.ai_do_command(&move_params).is_ok());
        assert!(group.is_moving);
        assert_eq!(group.target_position, Some([100.0, 200.0, 0.0]));
    }

    #[test]
    fn test_formation_types() {
        assert_eq!(FormationType::default(), FormationType::None);
        
        let formation = FormationData::default();
        assert_eq!(formation.formation_type, FormationType::None);
        assert!(formation.auto_adjust);
    }
}
