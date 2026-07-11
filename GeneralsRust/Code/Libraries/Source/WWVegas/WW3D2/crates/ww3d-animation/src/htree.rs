//! HTree (Hierarchy Tree) implementation
//!
//! This module implements the HTreeClass from WW3D, which represents
//! a hierarchy of coordinate systems in an initial configuration.

use crate::combo::HAnimCombo;
use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;

/// Pivot structure representing a bone in the hierarchy
/// This closely matches the C++ PivotClass structure
#[derive(Debug, Clone)]
pub struct Pivot {
    pub name: String,
    pub index: usize,
    pub parent_idx: i32,
    pub base_transform: Mat4,            // BaseTransform in C++
    pub transform: Mat4,                 // Transform in C++
    pub is_visible: bool,                // IsVisible in C++
    pub is_captured: bool,               // For bone capture functionality
    pub capture_transform: Option<Mat4>, // For captured bone override
}

/// HTreeClass - represents a hierarchy tree structure
/// This matches the C++ HTreeClass
#[derive(Debug, Clone)]
pub struct HTreeClass {
    pub name: String,
    pub pivots: Vec<Pivot>,
    pub pivot_name_to_index: HashMap<String, usize>,
    pub scale_factor: f32, // ScaleFactor in C++
}

impl HTreeClass {
    /// Create a new empty hierarchy tree
    pub fn new() -> Self {
        Self {
            name: String::new(),
            pivots: Vec::new(),
            pivot_name_to_index: HashMap::new(),
            scale_factor: 1.0,
        }
    }

    /// Create hierarchy tree with given name
    pub fn with_name(name: &str) -> Self {
        Self {
            name: name.to_string(),
            pivots: Vec::new(),
            pivot_name_to_index: HashMap::new(),
            scale_factor: 1.0,
        }
    }

    /// Initialize default hierarchy with root transform
    /// Matches C++ Init_Default()
    pub fn init_default(&mut self) {
        self.pivots.clear();
        self.pivot_name_to_index.clear();

        let root_pivot = Pivot {
            name: "RootTransform".to_string(),
            index: 0,
            parent_idx: -1,
            base_transform: Mat4::IDENTITY,
            transform: Mat4::IDENTITY,
            is_visible: true,
            is_captured: false,
            capture_transform: None,
        };

        self.pivot_name_to_index
            .insert("RootTransform".to_string(), 0);
        self.pivots.push(root_pivot);
        self.scale_factor = 1.0;
    }

    /// Add a pivot to the hierarchy
    /// Matches C++ pivot structure with proper base transform calculation
    pub fn add_pivot(&mut self, name: &str, parent_idx: i32, translation: Vec3, rotation: Quat) {
        let index = self.pivots.len();

        // Build base transform from translation and rotation
        let base_transform = Mat4::from_rotation_translation(rotation, translation);

        let pivot = Pivot {
            name: name.to_string(),
            index,
            parent_idx,
            base_transform,
            transform: Mat4::IDENTITY,
            is_visible: true,
            is_captured: false,
            capture_transform: None,
        };

        self.pivot_name_to_index.insert(name.to_string(), index);
        self.pivots.push(pivot);
    }

    /// Add a pivot providing the base transform directly.
    pub fn add_pivot_from_base(&mut self, name: &str, parent_idx: i32, base_transform: Mat4) {
        let index = self.pivots.len();
        let pivot = Pivot {
            name: name.to_string(),
            index,
            parent_idx,
            base_transform,
            transform: Mat4::IDENTITY,
            is_visible: true,
            is_captured: false,
            capture_transform: None,
        };
        self.pivot_name_to_index.insert(name.to_string(), index);
        self.pivots.push(pivot);
    }

    /// Get the number of pivots
    pub fn num_pivots(&self) -> usize {
        self.pivots.len()
    }

    /// Get pivot by index
    pub fn get_pivot(&self, index: usize) -> Option<&Pivot> {
        self.pivots.get(index)
    }

    /// Get pivot by name
    pub fn get_pivot_by_name(&self, name: &str) -> Option<&Pivot> {
        self.pivot_name_to_index
            .get(name)
            .and_then(|&index| self.pivots.get(index))
    }

    /// Get pivot index by name
    pub fn get_pivot_index(&self, name: &str) -> Option<usize> {
        self.pivot_name_to_index.get(name).copied()
    }

    /// Locate a pivot index ignoring ASCII case differences.
    pub fn find_pivot_index(&self, name: &str) -> Option<usize> {
        self.get_pivot_index(name).or_else(|| {
            let upper = name.to_ascii_uppercase();
            self.pivots
                .iter()
                .position(|pivot| pivot.name.to_ascii_uppercase() == upper)
        })
    }

    /// Get parent index of a pivot
    pub fn get_parent_index(&self, pivot_idx: usize) -> Option<i32> {
        self.pivots.get(pivot_idx).map(|p| p.parent_idx)
    }

    /// Base update - compute base pose transforms for each pivot
    /// Matches C++ Base_Update(const Matrix3D & root)
    pub fn base_update(&mut self, root_transform: Mat4) {
        // Set root transform and visibility
        if !self.pivots.is_empty() {
            self.pivots[0].transform = root_transform;
            self.pivots[0].is_visible = true;
        }

        // Update all other pivots based on their parent
        // This matches the C++ implementation exactly
        for i in 1..self.pivots.len() {
            let parent_idx = self.pivots[i].parent_idx;

            if parent_idx >= 0 && (parent_idx as usize) < self.pivots.len() {
                let parent_transform = self.pivots[parent_idx as usize].transform;
                let base_transform = self.pivots[i].base_transform;

                // Matrix3D::Multiply(pivot->Parent->Transform, pivot->BaseTransform, &(pivot->Transform));
                self.pivots[i].transform = parent_transform * base_transform;
                self.pivots[i].is_visible = true;

                // Handle captured bones
                if self.pivots[i].is_captured {
                    self.capture_update(i);
                }
            }
        }
    }

    /// Animation update - apply motion to pivots
    /// Matches C++ Anim_Update(const Matrix3D & root,HAnimClass * motion,float frame)
    pub fn anim_update(&mut self, root_transform: Mat4, translations: &[Vec3], rotations: &[Quat]) {
        // Set root transform and visibility
        if !self.pivots.is_empty() {
            self.pivots[0].transform = root_transform;
            self.pivots[0].is_visible = true;
        }

        // Apply animation to each pivot starting from index 1 (skip root)
        for i in 1..self.pivots.len() {
            let parent_idx = self.pivots[i].parent_idx;

            if parent_idx >= 0 && (parent_idx as usize) < self.pivots.len() {
                let parent_transform = self.pivots[parent_idx as usize].transform;
                let base_transform = self.pivots[i].base_transform;

                // Start with base pose: parent_transform * base_transform
                self.pivots[i].transform = parent_transform * base_transform;

                // Apply animation if available
                if i < translations.len() && i < rotations.len() {
                    // Apply translation with scale factor
                    let scaled_translation = translations[i] * self.scale_factor;
                    self.pivots[i].transform *= Mat4::from_translation(scaled_translation);

                    // Apply rotation
                    let rotation_matrix = Mat4::from_quat(rotations[i]);
                    self.pivots[i].transform *= rotation_matrix;
                }

                // Handle captured bones
                if self.pivots[i].is_captured {
                    self.capture_update(i);
                    self.pivots[i].is_visible = true;
                }
            }
        }
    }

    /// Apply an animation combo to this hierarchy, mirroring the legacy Combo_Update.
    pub fn combo_update(&mut self, root_transform: Mat4, combo: &HAnimCombo) {
        if self.pivots.is_empty() {
            return;
        }

        self.pivots[0].transform = root_transform;
        self.pivots[0].is_visible = true;

        let pivot_count = self.pivots.len();
        if pivot_count <= 1 {
            return;
        }

        let mut translations = vec![Vec3::ZERO; pivot_count];
        let mut rotations = vec![Quat::IDENTITY; pivot_count];
        let mut visibility = vec![true; pivot_count];

        combo.blend_into(
            self,
            &mut translations,
            &mut rotations,
            Some(&mut visibility),
        );

        for idx in 1..pivot_count {
            let parent_idx = self.pivots[idx].parent_idx;
            if parent_idx < 0 || (parent_idx as usize) >= pivot_count {
                continue;
            }

            let parent_transform = self.pivots[parent_idx as usize].transform;
            let base_transform = self.pivots[idx].base_transform;
            let mut transform = parent_transform * base_transform;

            let translation = Mat4::from_translation(translations[idx] * self.scale_factor);
            let rotation = Mat4::from_quat(rotations[idx]);
            transform = transform * translation * rotation;

            self.pivots[idx].transform = transform;
            self.pivots[idx].is_visible = visibility.get(idx).copied().unwrap_or(true);

            if self.pivots[idx].is_captured {
                self.capture_update(idx);
                self.pivots[idx].is_visible = true;
            }
        }
    }

    /// Get the transform for a specific pivot
    pub fn get_transform(&self, pivot_idx: usize) -> Option<Mat4> {
        self.pivots.get(pivot_idx).map(|p| p.transform)
    }

    /// Get bone name from index
    pub fn get_bone_name(&self, bone_idx: usize) -> Option<&str> {
        self.pivots.get(bone_idx).map(|p| p.name.as_str())
    }

    /// Get bone index by name (matches C++ Get_Bone_Index)
    pub fn get_bone_index(&self, name: &str) -> Option<usize> {
        self.pivot_name_to_index.get(name).copied()
    }

    /// Set scale factor for animations
    pub fn set_scale_factor(&mut self, factor: f32) {
        self.scale_factor = factor;
    }

    /// Get scale factor
    pub fn get_scale_factor(&self) -> f32 {
        self.scale_factor
    }

    /// Capture a bone for user control
    pub fn capture_bone(&mut self, bone_index: usize) {
        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            pivot.is_captured = true;
        }
    }

    /// Release a captured bone
    pub fn release_bone(&mut self, bone_index: usize) {
        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            pivot.is_captured = false;
            pivot.capture_transform = None;
        }
    }

    /// Check if bone is captured
    pub fn is_bone_captured(&self, bone_index: usize) -> bool {
        self.pivots.get(bone_index).is_some_and(|p| p.is_captured)
    }

    /// Control a captured bone with custom transform
    pub fn control_bone(&mut self, bone_index: usize, relative_transform: Mat4) {
        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            if pivot.is_captured {
                pivot.capture_transform = Some(relative_transform);
            }
        }
    }

    /// Update captured bone transform
    fn capture_update(&mut self, bone_index: usize) {
        // Get parent index first to avoid borrow issues
        let parent_idx = if bone_index < self.pivots.len() {
            self.pivots[bone_index].parent_idx
        } else {
            -1
        };

        let parent_transform = if parent_idx >= 0 && (parent_idx as usize) < self.pivots.len() {
            self.pivots[parent_idx as usize].transform
        } else {
            Mat4::IDENTITY
        };

        if let Some(pivot) = self.pivots.get_mut(bone_index) {
            if let Some(capture_transform) = pivot.capture_transform {
                pivot.transform = parent_transform * capture_transform;
            }
        }
    }

    /// Get visibility of a pivot
    pub fn get_visibility(&self, pivot_idx: usize) -> bool {
        self.pivots
            .get(pivot_idx)
            .map(|pivot| pivot.is_visible)
            .unwrap_or(true)
    }

    /// Get root transform
    pub fn get_root_transform(&self) -> Mat4 {
        self.pivots.first().map_or(Mat4::IDENTITY, |p| p.transform)
    }

    /// Update visibility flags for pivots.
    pub fn update_visibility(&mut self, visibility: &[bool]) {
        for (idx, pivot) in self.pivots.iter_mut().enumerate() {
            if let Some(flag) = visibility.get(idx) {
                pivot.is_visible = *flag;
            }
        }
    }

    /// Retrieve the transform for a pivot if available.
    pub fn transform(&self, pivot_idx: usize) -> Option<Mat4> {
        self.pivots.get(pivot_idx).map(|pivot| pivot.transform)
    }
}

/// Default implementation
impl Default for HTreeClass {
    fn default() -> Self {
        let mut htree = Self::new();
        htree.init_default();
        htree
    }
}
