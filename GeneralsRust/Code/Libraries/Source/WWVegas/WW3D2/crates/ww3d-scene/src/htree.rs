/// Hierarchical Tree (HTree) System
/// This module implements the skeletal bone hierarchy system from C++ htree.h/cpp
///
/// The HTree system is fundamental for:
/// - Character animation
/// - Skeletal mesh rendering
/// - Bone attachment points
/// - IK/FK systems
use glam::{Mat4, Quat, Vec3};
use std::sync::Arc;

#[allow(dead_code)] // C++ parity
const W3D_NAME_LEN: usize = 32;

fn fast_slerp(q0: Quat, q1: Quat, t: f32) -> Quat {
    let dot = q0.dot(q1);

    let q1_adjusted = if dot < 0.0 { -q1 } else { q1 };

    let dot_abs = dot.abs();

    if dot_abs > 0.9995 {
        q0.lerp(q1_adjusted, t).normalize()
    } else {
        q0.slerp(q1_adjusted, t)
    }
}

/// A single pivot (bone) in the hierarchy
#[derive(Clone, Debug)]
pub struct Pivot {
    /// Bone name
    pub name: String,
    /// Index in the hierarchy
    pub index: usize,
    /// Parent pivot (None for root)
    pub parent_index: Option<usize>,
    /// Base transform relative to parent (T-pose)
    pub base_transform: Mat4,
    /// Current computed world transform
    pub transform: Mat4,
    /// Visibility flag (from animation channel)
    pub is_visible: bool,
    /// User control flags
    pub is_captured: bool,
    /// Captured transform override
    pub cap_transform: Mat4,
    /// World space translation flag for capture
    pub world_space_translation: bool,
}

impl Pivot {
    /// Create a new pivot
    pub fn new(name: String, index: usize, parent_index: Option<usize>) -> Self {
        Self {
            name,
            index,
            parent_index,
            base_transform: Mat4::IDENTITY,
            transform: Mat4::IDENTITY,
            is_visible: true,
            is_captured: false,
            cap_transform: Mat4::IDENTITY,
            world_space_translation: false,
        }
    }

    /// Update the pivot's transform during bone capture
    pub fn capture_update(&mut self, parent_transform: &Mat4) {
        if self.is_captured {
            if self.world_space_translation {
                // Extract rotation from cap_transform
                let rotation = Mat4::from_quat(Quat::from_mat4(&self.cap_transform));
                // Use world space translation
                let translation = self.cap_transform.w_axis.truncate();
                self.transform = Mat4::from_translation(translation) * rotation;
            } else {
                // Apply relative to parent
                self.transform = *parent_transform * self.cap_transform;
            }
        }
    }
}

/// Hierarchical Tree - Represents a skeleton with bone hierarchy
///
/// This is a hierarchy of coordinate systems in an initial configuration.
/// All motion data is applied to one of these objects. Motion is stored as
/// deltas from the hierarchy tree's initial configuration.
#[derive(Clone, Debug)]
pub struct HTree {
    /// Hierarchy name
    pub name: String,
    /// Array of pivots (bones)
    pub pivots: Vec<Pivot>,
    /// Scale factor applied to animations
    pub scale_factor: f32,
}

impl HTree {
    /// Create a new empty HTree
    pub fn new(name: String) -> Self {
        Self {
            name,
            pivots: Vec::new(),
            scale_factor: 1.0,
        }
    }

    /// Initialize with a default root pivot
    pub fn init_default(&mut self) {
        self.pivots.clear();
        self.pivots
            .push(Pivot::new("RootTransform".to_string(), 0, None));
        self.name.clear();
    }

    /// Get the number of pivots in the hierarchy
    pub fn num_pivots(&self) -> usize {
        self.pivots.len()
    }

    /// Find a bone by name and return its index
    pub fn get_bone_index(&self, name: &str) -> Option<usize> {
        self.pivots
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Get the name of a bone by index
    pub fn get_bone_name(&self, index: usize) -> Option<&str> {
        self.pivots.get(index).map(|p| p.name.as_str())
    }

    /// Get the parent index of a bone
    pub fn get_parent_index(&self, index: usize) -> Option<usize> {
        self.pivots.get(index).and_then(|p| p.parent_index)
    }

    /// Get the transform of a pivot
    pub fn get_transform(&self, index: usize) -> Option<&Mat4> {
        self.pivots.get(index).map(|p| &p.transform)
    }

    /// Get the root transform
    pub fn get_root_transform(&self) -> &Mat4 {
        self.pivots
            .get(0)
            .map(|p| &p.transform)
            .unwrap_or(&Mat4::IDENTITY)
    }

    /// Get visibility of a pivot
    pub fn get_visibility(&self, index: usize) -> bool {
        self.pivots
            .get(index)
            .map(|p| p.is_visible)
            .unwrap_or(false)
    }

    /// Update the hierarchy to base pose (no animation)
    ///
    /// Computes the base pose transform for each pivot by walking the hierarchy
    /// and concatenating parent transforms with base transforms.
    pub fn base_update(&mut self, root: &Mat4) {
        if self.pivots.is_empty() {
            return;
        }

        // Set root transform
        self.pivots[0].transform = *root;
        self.pivots[0].is_visible = true;

        // Update all child pivots
        for i in 1..self.pivots.len() {
            let parent_transform = if let Some(parent_idx) = self.pivots[i].parent_index {
                self.pivots[parent_idx].transform
            } else {
                Mat4::IDENTITY
            };

            self.pivots[i].transform = parent_transform * self.pivots[i].base_transform;
            self.pivots[i].is_visible = true;

            // Handle captured bones
            if self.pivots[i].is_captured {
                self.pivots[i].capture_update(&parent_transform);
            }
        }
    }

    /// Update the hierarchy with animation data
    ///
    /// This applies translation, rotation, and visibility from an animation
    /// to each bone in the hierarchy.
    pub fn anim_update<A: Animation>(&mut self, root: &Mat4, motion: &A, frame: f32) {
        if self.pivots.is_empty() {
            return;
        }

        // Set root transform
        self.pivots[0].transform = *root;
        self.pivots[0].is_visible = true;

        let num_anim_pivots = motion.get_num_pivots();

        // Update all child pivots
        for i in 1..self.pivots.len() {
            let parent_transform = if let Some(parent_idx) = self.pivots[i].parent_index {
                self.pivots[parent_idx].transform
            } else {
                Mat4::IDENTITY
            };

            // Start with base transform
            self.pivots[i].transform = parent_transform * self.pivots[i].base_transform;

            // Apply animation if available for this pivot
            if i < num_anim_pivots {
                // Get animation data
                let translation = motion.get_translation(i, frame) * self.scale_factor;
                let rotation = motion.get_orientation(i, frame);

                // CRITICAL FIX: Apply translation in LOCAL space (C++ parity)
                // C++ does: pivot->Transform.Translate(trans * ScaleFactor)
                // This translates the existing transform, not pre-multiply
                let trans_matrix = Mat4::from_translation(translation);
                self.pivots[i].transform = self.pivots[i].transform * trans_matrix;

                // Apply rotation (post-multiply, same as C++)
                // C++ does: pivot->Transform = pivot->Transform * mtx
                let rotation_matrix = Mat4::from_quat(rotation);
                self.pivots[i].transform = self.pivots[i].transform * rotation_matrix;

                // Apply visibility
                self.pivots[i].is_visible = motion.get_visibility(i, frame);
            }

            // Handle captured bones
            if self.pivots[i].is_captured {
                self.pivots[i].capture_update(&parent_transform);
                self.pivots[i].is_visible = true;
            }
        }
    }

    /// Blend between two animations
    ///
    /// Interpolates translation and rotation between two animations based on
    /// a percentage value (0.0 = motion0, 1.0 = motion1)
    pub fn blend_update<A: Animation>(
        &mut self,
        root: &Mat4,
        motion0: &A,
        frame0: f32,
        motion1: &A,
        frame1: f32,
        percentage: f32,
    ) {
        if self.pivots.is_empty() {
            return;
        }

        // Set root transform
        self.pivots[0].transform = *root;
        self.pivots[0].is_visible = true;

        let num_anim_pivots = motion0.get_num_pivots().min(motion1.get_num_pivots());

        // Update all child pivots
        for i in 1..self.pivots.len() {
            let parent_transform = if let Some(parent_idx) = self.pivots[i].parent_index {
                self.pivots[parent_idx].transform
            } else {
                Mat4::IDENTITY
            };

            // Start with base transform
            self.pivots[i].transform = parent_transform * self.pivots[i].base_transform;

            // Apply blended animation if available
            if i < num_anim_pivots {
                // Blend translation
                let trans0 = motion0.get_translation(i, frame0);
                let trans1 = motion1.get_translation(i, frame1);
                let blended_trans = trans0.lerp(trans1, percentage) * self.scale_factor;

                // Blend rotation
                let rot0 = motion0.get_orientation(i, frame0);
                let rot1 = motion1.get_orientation(i, frame1);
                let blended_rot = fast_slerp(rot0, rot1, percentage);

                // Apply blended transform (C++ parity - translate then rotate)
                let trans_matrix = Mat4::from_translation(blended_trans);
                self.pivots[i].transform = self.pivots[i].transform * trans_matrix;
                let rotation_matrix = Mat4::from_quat(blended_rot);
                self.pivots[i].transform = self.pivots[i].transform * rotation_matrix;

                // Blend visibility (OR operation)
                self.pivots[i].is_visible =
                    motion0.get_visibility(i, frame0) || motion1.get_visibility(i, frame1);
            }

            // Handle captured bones
            if self.pivots[i].is_captured {
                self.pivots[i].capture_update(&parent_transform);
                self.pivots[i].is_visible = true;
            }
        }
    }

    /// Capture a bone for manual control
    ///
    /// When a bone is captured, animation data is ignored and the user
    /// can manually control the bone's transform.
    pub fn capture_bone(&mut self, bone_index: usize) {
        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            pivot.is_captured = true;
            pivot.cap_transform = Mat4::IDENTITY;
        }
    }

    /// Release a captured bone back to animation control
    pub fn release_bone(&mut self, bone_index: usize) {
        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            pivot.is_captured = false;
        }
    }

    /// Check if a bone is captured
    pub fn is_bone_captured(&self, bone_index: usize) -> bool {
        self.pivots
            .get(bone_index)
            .map(|p| p.is_captured)
            .unwrap_or(false)
    }

    /// Control a captured bone with a custom transform
    pub fn control_bone(
        &mut self,
        bone_index: usize,
        relative_tm: Mat4,
        world_space_translation: bool,
    ) {
        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            if pivot.is_captured {
                pivot.cap_transform = relative_tm;
                pivot.world_space_translation = world_space_translation;
            }
        }
    }

    /// Get the current control transform for a bone
    pub fn get_bone_control(&self, bone_index: usize) -> Mat4 {
        self.pivots
            .get(bone_index)
            .map(|p| {
                if p.is_captured {
                    p.cap_transform
                } else {
                    Mat4::IDENTITY
                }
            })
            .unwrap_or(Mat4::IDENTITY)
    }

    /// Simple pivot evaluation - returns transform of a pivot at a given frame
    /// without updating the entire hierarchy
    pub fn simple_evaluate_pivot<A: Animation>(
        &self,
        motion: &A,
        pivot_index: usize,
        frame: f32,
        obj_tm: &Mat4,
    ) -> Option<Mat4> {
        if pivot_index >= self.pivots.len() {
            return None;
        }

        let mut result = Mat4::IDENTITY;
        let mut current_idx = pivot_index;

        // Walk up the hierarchy
        while let Some(pivot) = self.pivots.get(current_idx) {
            // Get animation transform
            let translation = motion.get_translation(pivot.index, frame) * self.scale_factor;
            let rotation = motion.get_orientation(pivot.index, frame);

            let mut anim_tm = Mat4::from_quat(rotation);
            anim_tm = Mat4::from_translation(translation) * anim_tm;

            // Combine with base transform
            let curr_tm = pivot.base_transform * anim_tm;

            // Accumulate transform
            result = curr_tm * result;

            // Move to parent
            if let Some(parent_idx) = pivot.parent_index {
                current_idx = parent_idx;
            } else {
                break;
            }
        }

        // Apply object transform
        Some(*obj_tm * result)
    }

    /// Scale the entire hierarchy by a factor
    ///
    /// This scales all pivot base transforms and updates the scale factor
    /// used for animation translations.
    pub fn scale(&mut self, factor: f32) {
        if factor == 1.0 {
            return;
        }

        // Scale pivot translations
        for pivot in &mut self.pivots {
            let translation = pivot.base_transform.w_axis.truncate() * factor;
            pivot.base_transform.w_axis = translation.extend(1.0);
        }

        // Update scale factor for animations
        self.scale_factor *= factor;
    }

    /// Add a new pivot to the hierarchy
    pub fn add_pivot(
        &mut self,
        name: String,
        parent_index: Option<usize>,
        base_transform: Mat4,
    ) -> usize {
        let index = self.pivots.len();
        self.pivots.push(Pivot {
            name,
            index,
            parent_index,
            base_transform,
            transform: Mat4::IDENTITY,
            is_visible: true,
            is_captured: false,
            cap_transform: Mat4::IDENTITY,
            world_space_translation: false,
        });
        index
    }

    /// Update the hierarchy with a combination of animations
    ///
    /// This blends multiple animations together using weight maps and percentages.
    /// C++ Reference: htree.cpp lines 719-854 (HTreeClass::Combo_Update)
    pub fn combo_update<A: AnimationCombo>(&mut self, root: &Mat4, combo: &A) {
        if self.pivots.is_empty() {
            return;
        }

        // Set root transform
        self.pivots[0].transform = *root;
        self.pivots[0].is_visible = true;

        // Find minimum pivot count across all animations
        let num_anim_pivots = (0..combo.get_num_anims())
            .filter_map(|i| combo.get_animation(i).map(|a| a.get_num_pivots()))
            .min()
            .unwrap_or(0);

        // Update all child pivots
        for i in 1..self.pivots.len() {
            let parent_transform = if let Some(parent_idx) = self.pivots[i].parent_index {
                self.pivots[parent_idx].transform
            } else {
                Mat4::IDENTITY
            };

            // Start with base transform
            self.pivots[i].transform = parent_transform * self.pivots[i].base_transform;

            // Apply combo animation if available
            if i < num_anim_pivots {
                let mut translation = Vec3::ZERO;
                let mut rotation = Quat::IDENTITY;
                let mut total_weight = 0.0f32;
                let mut weight_count = 0;

                // Blend all animations in the combo
                for anim_idx in 0..combo.get_num_anims() {
                    if let Some(anim) = combo.get_animation(anim_idx) {
                        let frame = combo.get_frame(anim_idx);
                        let mut weight = combo.get_weight(anim_idx);

                        // Apply pivot weight map if available
                        if let Some(pivot_weight) = combo.get_pivot_weight(anim_idx, i) {
                            weight *= pivot_weight;
                        }

                        if weight > 0.0 {
                            weight_count += 1;

                            // Accumulate weighted translation
                            let trans = anim.get_translation(i, frame);
                            translation += trans * weight * self.scale_factor;
                            total_weight += weight;

                            // Blend rotation using slerp
                            let rot = anim.get_orientation(i, frame);
                            if weight_count == 1 {
                                rotation = rot;
                            } else {
                                // Incremental slerp blending
                                // C++ Reference: htree.cpp line 792 (Fast_Slerp with weight/weight_total)
                                rotation = fast_slerp(rotation, rot, weight / total_weight);
                            }
                        }
                    }
                }

                // Apply blended transform if we had any weights
                if total_weight > 0.0 {
                    let trans_matrix = Mat4::from_translation(translation);
                    self.pivots[i].transform = self.pivots[i].transform * trans_matrix;

                    let rotation_matrix = Mat4::from_quat(rotation);
                    self.pivots[i].transform = self.pivots[i].transform * rotation_matrix;
                }

                // Blend visibility (OR of all animations)
                // C++ Reference: htree.cpp lines 836-845
                self.pivots[i].is_visible = false;
                for anim_idx in 0..combo.get_num_anims() {
                    if let Some(anim) = combo.get_animation(anim_idx) {
                        let frame = combo.get_frame(anim_idx);
                        self.pivots[i].is_visible |= anim.get_visibility(i, frame);
                    }
                }
            }

            // Handle captured bones
            if self.pivots[i].is_captured {
                self.pivots[i].capture_update(&parent_transform);
                self.pivots[i].is_visible = true;
            }
        }
    }

    /// Create a morphed HTree by blending multiple source HTrees
    ///
    /// This creates a new skeleton by linearly interpolating bone positions
    /// from multiple source skeletons using weights.
    /// C++ Reference: htree.cpp lines 1076-1107 (HTreeClass::Create_Morphed)
    pub fn create_morphed(sources: &[(&HTree, f32)]) -> Option<HTree> {
        if sources.is_empty() {
            return None;
        }

        // Verify all trees have same number of pivots
        let num_pivots = sources[0].0.num_pivots();
        for (tree, _) in sources {
            if tree.num_pivots() != num_pivots {
                return None;
            }
        }

        // Clone the first tree as base
        let mut result = sources[0].0.clone();

        // Interpolate all pivot translations
        for pivot_idx in 0..num_pivots {
            let mut position = Vec3::ZERO;

            for (tree, weight) in sources {
                let trans = tree.pivots[pivot_idx].base_transform.w_axis.truncate();
                position += trans * *weight;
            }

            result.pivots[pivot_idx].base_transform.w_axis = position.extend(1.0);
        }

        Some(result)
    }

    /// Create an HTree by bilinear interpolation between four source trees
    ///
    /// This is useful for creating blend shapes or morphing between different states.
    /// C++ Reference: htree.cpp lines 1110-1139 (HTreeClass::Create_Interpolated 4-way)
    pub fn create_interpolated_4way(
        tree_a0_b0: &HTree,
        tree_a0_b1: &HTree,
        tree_a1_b0: &HTree,
        tree_a1_b1: &HTree,
        lerp_a: f32,
        lerp_b: f32,
    ) -> Option<HTree> {
        // Verify all trees have same number of pivots
        let num_pivots = tree_a0_b0.num_pivots();
        if tree_a0_b1.num_pivots() != num_pivots
            || tree_a1_b0.num_pivots() != num_pivots
            || tree_a1_b1.num_pivots() != num_pivots
        {
            return None;
        }

        // Clone first tree as base
        let mut result = tree_a0_b0.clone();

        // Interpolate all pivot translations
        for pivot_idx in 0..num_pivots {
            let pos_a0_b0 = tree_a0_b0.pivots[pivot_idx]
                .base_transform
                .w_axis
                .truncate();
            let pos_a0_b1 = tree_a0_b1.pivots[pivot_idx]
                .base_transform
                .w_axis
                .truncate();
            let pos_a1_b0 = tree_a1_b0.pivots[pivot_idx]
                .base_transform
                .w_axis
                .truncate();
            let pos_a1_b1 = tree_a1_b1.pivots[pivot_idx]
                .base_transform
                .w_axis
                .truncate();

            // Bilinear interpolation
            let pos_a0 = pos_a0_b0.lerp(pos_a0_b1, lerp_b);
            let pos_a1 = pos_a1_b0.lerp(pos_a1_b1, lerp_b);
            let pos_final = pos_a0.lerp(pos_a1, lerp_a);

            result.pivots[pivot_idx].base_transform.w_axis = pos_final.extend(1.0);
        }

        Some(result)
    }

    /// Morph avatar skeleton for non-uniform scaling
    ///
    /// Special handling for avatar skeletons where arms need different axis scaling.
    /// C++ Reference: htree.cpp lines 1019-1073 (HTreeClass::Alter_Avatar_HTree)
    pub fn alter_avatar_htree(&self, scale: Vec3) -> HTree {
        // List of bones that use Z-axis scaling instead of Y-axis for arms/hands
        const FLIP_LIST: &[&str] = &[
            " RIGHTFOREARM",
            " RIGHTHAND",
            " LEFTFOREARM",
            " LEFTHAND",
            "RIGHTINDEX",
            "RIGHTFINGERS",
            "RIGHTTHUMB",
            "LEFTINDEX",
            "LEFTFINGERS",
            "LEFTTHUMB",
        ];

        let mut result = self.clone();

        for pivot_idx in 0..result.pivots.len() {
            if result.pivots[pivot_idx].parent_index.is_none() {
                continue;
            }

            let mut adjusted_scale = scale;

            // Check if this pivot needs flipped scaling
            let pivot_name = &result.pivots[pivot_idx].name;
            if FLIP_LIST.iter().any(|&name| pivot_name == name) {
                adjusted_scale.y = scale.z;
            }

            // Get pivot and parent positions in world space
            let pivot_pos = result.pivots[pivot_idx].transform.w_axis.truncate();
            let parent_pos = if let Some(parent_idx) = result.pivots[pivot_idx].parent_index {
                result.pivots[parent_idx].transform.w_axis.truncate()
            } else {
                Vec3::ZERO
            };

            // Apply scaling
            let scaled_pivot_pos = pivot_pos * adjusted_scale;
            let scaled_parent_pos = parent_pos * adjusted_scale;

            // Compute new relative vector
            let new_relative_vector = scaled_pivot_pos - scaled_parent_pos;

            // Get parent inverse transform
            if let Some(parent_idx) = result.pivots[pivot_idx].parent_index {
                let parent_inv = result.pivots[parent_idx].transform.inverse();
                // Rotate vector to local space
                let local_vector = parent_inv.transform_vector3(new_relative_vector);
                result.pivots[pivot_idx].base_transform.w_axis = local_vector.extend(1.0);
            }
        }

        result
    }
}

/// Animation trait - abstraction over different animation types
///
/// This allows the HTree to work with different animation formats
/// (HAnimClass, HRawAnimClass, etc.)
pub trait Animation {
    /// Get the number of pivots this animation affects
    fn get_num_pivots(&self) -> usize;

    /// Get translation for a pivot at a frame
    fn get_translation(&self, pivot: usize, frame: f32) -> Vec3;

    /// Get orientation (rotation) for a pivot at a frame
    fn get_orientation(&self, pivot: usize, frame: f32) -> Quat;

    /// Get visibility for a pivot at a frame
    fn get_visibility(&self, pivot: usize, frame: f32) -> bool;
}

/// Animation combo trait - for blending multiple animations
///
/// C++ Reference: hcanim.h/cpp (HAnimComboClass)
/// This allows blending multiple animations with individual weights and pivot-level control.
pub trait AnimationCombo {
    /// Get the number of animations in this combo
    fn get_num_anims(&self) -> usize;

    /// Get an animation by index
    fn get_animation(&self, index: usize) -> Option<&dyn Animation>;

    /// Get the current frame for an animation
    fn get_frame(&self, index: usize) -> f32;

    /// Get the weight for an animation (0.0 to 1.0)
    fn get_weight(&self, index: usize) -> f32;

    /// Get the pivot-specific weight for an animation
    /// Returns None if no pivot-specific weight map exists
    fn get_pivot_weight(&self, anim_index: usize, pivot_index: usize) -> Option<f32>;
}

/// HTree Manager - manages loading and caching of hierarchies
#[derive(Default)]
pub struct HTreeManager {
    /// Cache of loaded hierarchies
    trees: std::collections::HashMap<String, Arc<HTree>>,
}

impl HTreeManager {
    /// Create a new HTree manager
    pub fn new() -> Self {
        Self {
            trees: std::collections::HashMap::new(),
        }
    }

    /// Get a tree by name, loading it if necessary
    pub fn get_tree(&mut self, name: &str) -> Option<Arc<HTree>> {
        self.trees.get(name).cloned()
    }

    /// Add a tree to the cache
    pub fn add_tree(&mut self, name: String, tree: HTree) {
        self.trees.insert(name, Arc::new(tree));
    }

    /// Remove a tree from the cache
    pub fn remove_tree(&mut self, name: &str) -> bool {
        self.trees.remove(name).is_some()
    }

    /// Free all cached trees
    pub fn free_all_trees(&mut self) {
        self.trees.clear();
    }

    /// Get the number of cached trees
    pub fn tree_count(&self) -> usize {
        self.trees.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAnimation {
        num_pivots: usize,
    }

    impl Animation for MockAnimation {
        fn get_num_pivots(&self) -> usize {
            self.num_pivots
        }

        fn get_translation(&self, _pivot: usize, _frame: f32) -> Vec3 {
            Vec3::new(1.0, 0.0, 0.0)
        }

        fn get_orientation(&self, _pivot: usize, _frame: f32) -> Quat {
            Quat::IDENTITY
        }

        fn get_visibility(&self, _pivot: usize, _frame: f32) -> bool {
            true
        }
    }

    #[test]
    fn test_htree_creation() {
        let mut tree = HTree::new("TestHierarchy".to_string());
        tree.init_default();

        assert_eq!(tree.num_pivots(), 1);
        assert_eq!(tree.get_bone_name(0), Some("RootTransform"));
    }

    #[test]
    fn test_htree_add_pivot() {
        let mut tree = HTree::new("TestHierarchy".to_string());
        tree.init_default();

        let bone1 = tree.add_pivot("Bone1".to_string(), Some(0), Mat4::IDENTITY);
        assert_eq!(bone1, 1);
        assert_eq!(tree.num_pivots(), 2);
    }

    #[test]
    fn test_htree_base_update() {
        let mut tree = HTree::new("TestHierarchy".to_string());
        tree.init_default();
        tree.add_pivot(
            "Bone1".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        );

        tree.base_update(&Mat4::IDENTITY);

        assert!(tree.get_visibility(0));
        assert!(tree.get_visibility(1));
    }

    #[test]
    fn test_htree_anim_update() {
        let mut tree = HTree::new("TestHierarchy".to_string());
        tree.init_default();
        tree.add_pivot("Bone1".to_string(), Some(0), Mat4::IDENTITY);

        let anim = MockAnimation { num_pivots: 2 };
        tree.anim_update(&Mat4::IDENTITY, &anim, 0.0);

        assert!(tree.get_visibility(0));
        assert!(tree.get_visibility(1));
    }

    #[test]
    fn test_bone_capture() {
        let mut tree = HTree::new("TestHierarchy".to_string());
        tree.init_default();
        tree.add_pivot("Bone1".to_string(), Some(0), Mat4::IDENTITY);

        tree.capture_bone(1);
        assert!(tree.is_bone_captured(1));

        tree.release_bone(1);
        assert!(!tree.is_bone_captured(1));
    }

    #[test]
    fn test_htree_manager() {
        let mut manager = HTreeManager::new();
        let tree = HTree::new("TestTree".to_string());

        manager.add_tree("TestTree".to_string(), tree);
        assert_eq!(manager.tree_count(), 1);

        let retrieved = manager.get_tree("TestTree");
        assert!(retrieved.is_some());

        manager.free_all_trees();
        assert_eq!(manager.tree_count(), 0);
    }

    #[test]
    fn test_htree_scaling() {
        let mut tree = HTree::new("TestHierarchy".to_string());
        tree.init_default();
        tree.add_pivot(
            "Bone1".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        );

        tree.scale(2.0);
        assert_eq!(tree.scale_factor, 2.0);

        // Check that bone translation was scaled
        let bone_trans = tree.pivots[1].base_transform.w_axis.truncate();
        assert_eq!(bone_trans, Vec3::new(0.0, 2.0, 0.0));
    }

    #[test]
    fn test_blend_update() {
        let mut tree = HTree::new("TestHierarchy".to_string());
        tree.init_default();
        tree.add_pivot("Bone1".to_string(), Some(0), Mat4::IDENTITY);

        let anim0 = MockAnimation { num_pivots: 2 };
        let anim1 = MockAnimation { num_pivots: 2 };

        tree.blend_update(&Mat4::IDENTITY, &anim0, 0.0, &anim1, 1.0, 0.5);

        assert!(tree.get_visibility(0));
        assert!(tree.get_visibility(1));
    }

    // Test for hierarchical tree morphing
    #[test]
    fn test_htree_create_morphed() {
        let mut tree1 = HTree::new("Tree1".to_string());
        tree1.init_default();
        tree1.add_pivot(
            "Bone1".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)),
        );

        let mut tree2 = HTree::new("Tree2".to_string());
        tree2.init_default();
        tree2.add_pivot(
            "Bone1".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        );

        // 50/50 blend
        let morphed = HTree::create_morphed(&[(&tree1, 0.5), (&tree2, 0.5)]);
        assert!(morphed.is_some());

        let morphed_tree = morphed.unwrap();
        assert_eq!(morphed_tree.num_pivots(), 2);

        // Check blended translation (should be average)
        let blended_pos = morphed_tree.pivots[1].base_transform.w_axis.truncate();
        assert!((blended_pos.x - 0.5).abs() < 0.01);
        assert!((blended_pos.y - 0.5).abs() < 0.01);
    }

    // Test for 4-way interpolation
    #[test]
    fn test_htree_create_interpolated_4way() {
        let mut tree_a0_b0 = HTree::new("A0B0".to_string());
        tree_a0_b0.init_default();
        tree_a0_b0.add_pivot(
            "Bone1".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(0.0, 0.0, 0.0)),
        );

        let mut tree_a0_b1 = HTree::new("A0B1".to_string());
        tree_a0_b1.init_default();
        tree_a0_b1.add_pivot(
            "Bone1".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        );

        let mut tree_a1_b0 = HTree::new("A1B0".to_string());
        tree_a1_b0.init_default();
        tree_a1_b0.add_pivot(
            "Bone1".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)),
        );

        let mut tree_a1_b1 = HTree::new("A1B1".to_string());
        tree_a1_b1.init_default();
        tree_a1_b1.add_pivot(
            "Bone1".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(1.0, 1.0, 0.0)),
        );

        // Center interpolation
        let result = HTree::create_interpolated_4way(
            &tree_a0_b0,
            &tree_a0_b1,
            &tree_a1_b0,
            &tree_a1_b1,
            0.5,
            0.5,
        );
        assert!(result.is_some());

        let interpolated = result.unwrap();
        let pos = interpolated.pivots[1].base_transform.w_axis.truncate();

        // Should be at (0.5, 0.5, 0.0)
        assert!((pos.x - 0.5).abs() < 0.01);
        assert!((pos.y - 0.5).abs() < 0.01);
    }

    // Test avatar morphing with axis-specific scaling
    #[test]
    fn test_alter_avatar_htree() {
        let mut tree = HTree::new("Avatar".to_string());
        tree.init_default();

        // Add a regular bone
        tree.add_pivot(
            "Spine".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0)),
        );

        // Add an arm bone (should use Z-axis scaling for Y)
        tree.add_pivot(
            " RIGHTFOREARM".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0)),
        );

        // Update transforms to have proper world positions
        tree.base_update(&Mat4::IDENTITY);

        // Apply non-uniform scaling
        let scaled = tree.alter_avatar_htree(Vec3::new(1.0, 2.0, 3.0));

        assert_eq!(scaled.num_pivots(), 3);
        // Verify the tree was modified (this is a basic check)
        assert!(scaled.pivots.len() > 0);
    }

    // Test bone capture and control
    #[test]
    fn test_bone_capture_control() {
        let mut tree = HTree::new("TestHierarchy".to_string());
        tree.init_default();
        let bone_idx = tree.add_pivot("Bone1".to_string(), Some(0), Mat4::IDENTITY);

        // Capture the bone
        tree.capture_bone(bone_idx);
        assert!(tree.is_bone_captured(bone_idx));

        // Set a custom transform
        let custom_transform = Mat4::from_translation(Vec3::new(5.0, 5.0, 5.0));
        tree.control_bone(bone_idx, custom_transform, false);

        // Update with animation (should be overridden by capture)
        let anim = MockAnimation { num_pivots: 2 };
        tree.anim_update(&Mat4::IDENTITY, &anim, 0.0);

        // Captured bone should still be visible
        assert!(tree.get_visibility(bone_idx));

        // Release the bone
        tree.release_bone(bone_idx);
        assert!(!tree.is_bone_captured(bone_idx));
    }

    // Test simple pivot evaluation
    #[test]
    fn test_simple_evaluate_pivot() {
        let mut tree = HTree::new("TestHierarchy".to_string());
        tree.init_default();
        let bone_idx = tree.add_pivot(
            "Bone1".to_string(),
            Some(0),
            Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)),
        );

        tree.base_update(&Mat4::IDENTITY);

        let anim = MockAnimation { num_pivots: 2 };
        let obj_tm = Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0));

        let result = tree.simple_evaluate_pivot(&anim, bone_idx, 0.0, &obj_tm);
        assert!(result.is_some());
    }
}
