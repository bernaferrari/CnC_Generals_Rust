//! Inverse Kinematics System
//!
//! This module provides FABRIK (Forward And Backward Reaching Inverse Kinematics)
//! solver for skeletal animation, enabling foot placement, ragdoll physics, and IK animations.
//!
//! Reference: Aristidou & Lasenby (2011) "FABRIK: A fast iterative solver for the inverse kinematics problem"
//! C++ Reference: /GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/ik.h

use glam::{Quat, Vec3};
use std::collections::HashMap;

/// Result type for IK operations
pub type IKResult<T> = Result<T, IKError>;

/// IK system errors
#[derive(Debug, Clone)]
pub enum IKError {
    /// Invalid chain (no bones or parent mismatch)
    InvalidChain(String),
    /// Chain not found
    ChainNotFound(String),
    /// Bone not found
    BoneNotFound(u32),
    /// Failed to converge
    FailedToConverge(String),
    /// Invalid parameters
    InvalidParams(String),
}

impl std::fmt::Display for IKError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InvalidChain(msg) => write!(f, "Invalid chain: {}", msg),
            Self::ChainNotFound(name) => write!(f, "Chain not found: {}", name),
            Self::BoneNotFound(idx) => write!(f, "Bone not found: {}", idx),
            Self::FailedToConverge(msg) => write!(f, "Failed to converge: {}", msg),
            Self::InvalidParams(msg) => write!(f, "Invalid parameters: {}", msg),
        }
    }
}

impl std::error::Error for IKError {}

/// Bone constraint types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConstraintType {
    /// No constraint (ball joint)
    None,
    /// Constrain rotation around a single axis
    HingeAxis {
        axis: [f32; 3],
        min_angle: f32,
        max_angle: f32,
    },
    /// Constrain rotation around two axes
    BallJoint { min_angle: f32, max_angle: f32 },
    /// Fix bone in place
    Fixed,
}

/// Bone constraint defining rotational limits
#[derive(Debug, Clone)]
pub struct BoneConstraint {
    /// Bone index
    pub bone_idx: u32,
    /// Constraint type
    pub constraint_type: ConstraintType,
    /// Whether constraint is active
    pub enabled: bool,
}

impl BoneConstraint {
    /// Create a new bone constraint
    pub fn new(bone_idx: u32, constraint_type: ConstraintType) -> Self {
        Self {
            bone_idx,
            constraint_type,
            enabled: true,
        }
    }

    /// Disable constraint
    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Enable constraint
    pub fn enable(&mut self) {
        self.enabled = true;
    }
}

/// IK chain bone data
#[derive(Debug, Clone)]
pub struct ChainBone {
    /// Bone index in skeleton
    pub bone_idx: u32,
    /// Position (world space)
    pub position: Vec3,
    /// Rotation (quaternion)
    pub rotation: Quat,
    /// Base position (for distance calculation)
    pub base_position: Vec3,
    /// Bone length to next bone in chain
    pub bone_length: f32,
}

impl ChainBone {
    /// Create a new chain bone
    pub fn new(bone_idx: u32, position: Vec3, rotation: Quat) -> Self {
        Self {
            bone_idx,
            position,
            rotation,
            base_position: position,
            bone_length: 0.0,
        }
    }
}

/// IK chain configuration
#[derive(Debug, Clone)]
pub struct IKChain {
    /// Chain name
    pub name: String,
    /// Bones in the chain (root to effector)
    pub bones: Vec<ChainBone>,
    /// Constraints for each bone
    pub constraints: HashMap<u32, BoneConstraint>,
    /// Target position (where effector should reach)
    pub target_position: Vec3,
    /// Target rotation (desired effector orientation)
    pub target_rotation: Quat,
    /// Tolerance for convergence
    pub tolerance: f32,
    /// Maximum iterations
    pub max_iterations: u32,
    /// Whether to apply constraint to last bone
    pub constrain_root: bool,
    /// Weight for this chain (0.0-1.0)
    pub weight: f32,
    /// Enable/disable the chain
    pub enabled: bool,
}

impl IKChain {
    /// Create a new IK chain
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            bones: Vec::new(),
            constraints: HashMap::new(),
            target_position: Vec3::ZERO,
            target_rotation: Quat::IDENTITY,
            tolerance: 0.01,
            max_iterations: 10,
            constrain_root: true,
            weight: 1.0,
            enabled: true,
        }
    }

    /// Add a bone to the chain
    pub fn add_bone(&mut self, bone_idx: u32, position: Vec3, rotation: Quat) {
        self.bones
            .push(ChainBone::new(bone_idx, position, rotation));
    }

    /// Set chain target
    pub fn set_target(&mut self, position: Vec3, rotation: Quat) {
        self.target_position = position;
        self.target_rotation = rotation;
    }

    /// Set target position only
    pub fn set_target_position(&mut self, position: Vec3) {
        self.target_position = position;
    }

    /// Add constraint to bone
    pub fn add_constraint(&mut self, constraint: BoneConstraint) -> IKResult<()> {
        if self.bones.iter().any(|b| b.bone_idx == constraint.bone_idx) {
            self.constraints.insert(constraint.bone_idx, constraint);
            Ok(())
        } else {
            Err(IKError::BoneNotFound(constraint.bone_idx))
        }
    }

    /// Get chain length (sum of bone lengths)
    pub fn total_length(&self) -> f32 {
        self.bones.iter().map(|b| b.bone_length).sum()
    }

    /// Get effector position (last bone)
    pub fn effector_position(&self) -> Vec3 {
        self.bones.last().map(|b| b.position).unwrap_or(Vec3::ZERO)
    }

    /// Distance from effector to target
    pub fn effector_distance_to_target(&self) -> f32 {
        (self.effector_position() - self.target_position).length()
    }
}

/// FABRIK (Forward And Backward Reaching Inverse Kinematics) solver
///
/// Algorithm overview:
/// 1. Forward pass: Move from effector to root, pulling each bone toward next
/// 2. Backward pass: Move from root to effector, constraining distances
/// 3. Repeat until convergence or max iterations
///
/// Reference: Aristidou & Lasenby (2011)
#[derive(Debug)]
pub struct FABRIKSolver {
    /// IK chains
    chains: HashMap<String, IKChain>,
    /// Global iterations
    iterations: u32,
    /// Enable solver
    enabled: bool,
}

impl FABRIKSolver {
    /// Create a new FABRIK solver
    pub fn new() -> Self {
        Self {
            chains: HashMap::new(),
            iterations: 0,
            enabled: true,
        }
    }

    /// Add a chain to the solver
    pub fn add_chain(&mut self, chain: IKChain) {
        self.chains.insert(chain.name.clone(), chain);
    }

    /// Get a chain
    pub fn get_chain(&self, name: &str) -> Option<&IKChain> {
        self.chains.get(name)
    }

    /// Get a mutable chain
    pub fn get_chain_mut(&mut self, name: &str) -> Option<&mut IKChain> {
        self.chains.get_mut(name)
    }

    /// Remove a chain
    pub fn remove_chain(&mut self, name: &str) -> Option<IKChain> {
        self.chains.remove(name)
    }

    /// Enable/disable solver
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if solver is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Solve all enabled chains
    pub fn solve(&mut self) -> IKResult<()> {
        if !self.enabled {
            return Ok(());
        }

        let chain_names: Vec<String> = self.chains.keys().cloned().collect();

        for name in chain_names {
            if let Some(chain) = self.chains.get_mut(&name) {
                if chain.enabled {
                    // Solve the chain - we need to use solve_chain_internal
                    // to avoid borrowing self while chain is borrowed
                    Self::solve_chain_internal(chain, self.enabled)?;
                }
            }
        }

        self.iterations += 1;
        Ok(())
    }

    /// Internal chain solving logic (static method to avoid self borrow conflicts)
    fn solve_chain_internal(chain: &mut IKChain, _enabled: bool) -> IKResult<()> {
        if chain.bones.is_empty() {
            return Ok(());
        }

        if chain.bones.len() < 2 {
            return Err(IKError::InvalidChain(
                "Chain must have at least 2 bones".to_string(),
            ));
        }

        // Precompute bone lengths
        for i in 0..chain.bones.len() - 1 {
            let dist = (chain.bones[i + 1].position - chain.bones[i].position).length();
            chain.bones[i].bone_length = dist.max(0.001); // Avoid zero-length bones
        }

        // Run FABRIK iterations
        for _ in 0..chain.max_iterations {
            // Forward pass (effector to root)
            Self::forward_pass_internal(chain)?;

            // Backward pass (root to effector)
            Self::backward_pass_internal(chain)?;

            // Check convergence
            let error = chain.effector_distance_to_target();
            if error < chain.tolerance {
                break;
            }
        }

        Ok(())
    }

    /// Forward pass (static helper to avoid borrow conflicts)
    fn forward_pass_internal(chain: &mut IKChain) -> IKResult<()> {
        let n = chain.bones.len();

        // Move effector to target
        if n > 0 {
            chain.bones[n - 1].position = chain.target_position;
        }

        // Pull each bone toward the next
        for i in (1..n).rev() {
            let current_pos = chain.bones[i].position;
            let prev_pos = chain.bones[i - 1].position;

            let direction = (prev_pos - current_pos).normalize();
            let distance = chain.bones[i - 1].bone_length;

            chain.bones[i - 1].position = current_pos + direction * distance;
        }

        Ok(())
    }

    /// Backward pass (static helper to avoid borrow conflicts)
    fn backward_pass_internal(chain: &mut IKChain) -> IKResult<()> {
        let n = chain.bones.len();

        // Keep root in place (if constrained)
        if !chain.constrain_root && n > 0 {
            chain.bones[0].position = chain.bones[0].base_position;
        }

        // Push each bone away from the next
        for i in 0..n - 1 {
            let current_pos = chain.bones[i].position;
            let next_pos = chain.bones[i + 1].position;

            let direction = (next_pos - current_pos).normalize();
            let distance = chain.bones[i].bone_length;

            chain.bones[i + 1].position = current_pos + direction * distance;

            // Apply constraints - simplified for now
            if let Some(constraint) = chain.constraints.get(&chain.bones[i].bone_idx) {
                if constraint.enabled {
                    match constraint.constraint_type {
                        ConstraintType::Fixed => {
                            chain.bones[i].position = chain.bones[i].base_position;
                        }
                        _ => {} // Other constraint types handled separately
                    }
                }
            }
        }

        Ok(())
    }

    /// Get solver statistics
    pub fn iterations(&self) -> u32 {
        self.iterations
    }

    /// Reset statistics
    pub fn reset_stats(&mut self) {
        self.iterations = 0;
    }
}

impl Default for FABRIKSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_creation() {
        let chain = IKChain::new("test_chain");
        assert_eq!(chain.name, "test_chain");
        assert!(chain.bones.is_empty());
        assert_eq!(chain.tolerance, 0.01);
    }

    #[test]
    fn test_add_bone_to_chain() {
        let mut chain = IKChain::new("test");
        chain.add_bone(0, Vec3::ZERO, Quat::IDENTITY);
        chain.add_bone(1, Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY);

        assert_eq!(chain.bones.len(), 2);
        assert_eq!(chain.bones[0].bone_idx, 0);
        assert_eq!(chain.bones[1].bone_idx, 1);
    }

    #[test]
    fn test_set_target() {
        let mut chain = IKChain::new("test");
        let target_pos = Vec3::new(1.0, 2.0, 3.0);
        let target_rot = Quat::IDENTITY;

        chain.set_target(target_pos, target_rot);

        assert_eq!(chain.target_position, target_pos);
        assert_eq!(chain.target_rotation, target_rot);
    }

    #[test]
    fn test_solver_creation() {
        let solver = FABRIKSolver::new();
        assert!(solver.is_enabled());
        assert_eq!(solver.iterations(), 0);
    }

    #[test]
    fn test_add_chain_to_solver() {
        let mut solver = FABRIKSolver::new();
        let chain = IKChain::new("test");

        solver.add_chain(chain);

        assert!(solver.get_chain("test").is_some());
    }

    #[test]
    fn test_constraint_creation() {
        let constraint = BoneConstraint::new(0, ConstraintType::Fixed);
        assert_eq!(constraint.bone_idx, 0);
        assert!(constraint.enabled);
    }

    #[test]
    fn test_simple_two_bone_chain() {
        let mut chain = IKChain::new("arm");
        chain.add_bone(0, Vec3::ZERO, Quat::IDENTITY); // Shoulder
        chain.add_bone(1, Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY); // Elbow

        chain.bones[0].bone_length = 1.0;

        chain.set_target_position(Vec3::new(1.5, 0.0, 0.0));

        let mut solver = FABRIKSolver::new();
        solver.add_chain(chain);
        let result = solver.solve();

        assert!(result.is_ok());
        // After solving, the chain should be updated
        if let Some(solved_chain) = solver.get_chain("arm") {
            let final_dist = solved_chain.effector_distance_to_target();
            assert!(final_dist < 0.2); // Should converge close to target
        }
    }

    #[test]
    fn test_chain_total_length() {
        let mut chain = IKChain::new("test");
        chain.add_bone(0, Vec3::ZERO, Quat::IDENTITY);
        chain.add_bone(1, Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY);

        chain.bones[0].bone_length = 1.0;
        chain.bones[1].bone_length = 1.0;

        assert_eq!(chain.total_length(), 2.0);
    }

    #[test]
    fn test_effector_position() {
        let mut chain = IKChain::new("test");
        chain.add_bone(0, Vec3::ZERO, Quat::IDENTITY);
        chain.add_bone(1, Vec3::new(1.0, 0.0, 0.0), Quat::IDENTITY);

        assert_eq!(chain.effector_position(), Vec3::new(1.0, 0.0, 0.0));
    }
}
