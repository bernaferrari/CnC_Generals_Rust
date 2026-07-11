//! Static sort list system for transparent object rendering
//!
//! This module implements the C++ static_sort_list.cpp system that handles
//! sorting objects for correct transparency and rendering order.

use crate::render_object_system::{RenderInfoClass, RenderObjClass};
use glam::Vec3;
use std::cmp::Ordering;
use std::sync::Arc;

/// Maximum sort level (matches C++ MAX_SORT_LEVEL)
pub const MAX_SORT_LEVEL: usize = 32;

/// Sort entry for a single render object with distance and sort key
#[derive(Clone)]
pub struct SortEntry {
    /// The render object to be sorted
    pub render_obj: Arc<dyn RenderObjClass>,
    /// Squared distance from camera (to avoid sqrt)
    pub distance_squared: f32,
    /// Stable sort key for consistent ordering
    pub sort_key: u64,
}

impl SortEntry {
    /// Create a new sort entry
    pub fn new(render_obj: Arc<dyn RenderObjClass>, camera_position: Vec3, sort_key: u64) -> Self {
        // Calculate squared distance from camera to object center
        let object_position = render_obj.position();
        let delta = object_position - camera_position;
        let distance_squared = delta.length_squared();

        Self {
            render_obj,
            distance_squared,
            sort_key,
        }
    }

    /// Compare by distance (back to front for transparency)
    pub fn cmp_back_to_front(&self, other: &Self) -> Ordering {
        // Reverse comparison for back-to-front (farthest first)
        other
            .distance_squared
            .partial_cmp(&self.distance_squared)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.sort_key.cmp(&other.sort_key))
    }

    /// Compare by distance (front to back for opaque)
    pub fn cmp_front_to_back(&self, other: &Self) -> Ordering {
        // Normal comparison for front-to-back (nearest first)
        self.distance_squared
            .partial_cmp(&other.distance_squared)
            .unwrap_or(Ordering::Equal)
            .then_with(|| self.sort_key.cmp(&other.sort_key))
    }

    /// Compare by material/shader for state batching
    pub fn cmp_by_material(&self, other: &Self) -> Ordering {
        // Would compare material IDs, shader bits, texture handles, etc.
        // For now, use sort key as proxy
        self.sort_key.cmp(&other.sort_key)
    }
}

/// Static sort list for a single sort level
pub struct StaticSortList {
    /// Entries at this sort level
    entries: Vec<SortEntry>,
    /// Whether this list is currently being filled
    is_filling: bool,
}

impl StaticSortList {
    /// Create a new empty sort list
    pub fn new() -> Self {
        Self {
            entries: Vec::with_capacity(256), // Pre-allocate for common case
            is_filling: false,
        }
    }

    /// Add an entry to the list (matches C++ Add_Tail)
    pub fn add_tail(&mut self, render_obj: Arc<dyn RenderObjClass>, camera_position: Vec3) {
        // Generate a stable sort key based on object address for consistency
        // We use the data pointer (first field of the fat pointer) for the sort key
        let ptr = Arc::as_ptr(&render_obj);
        let data_ptr = ptr as *const () as *const u8;
        let sort_key = data_ptr as usize as u64;
        let entry = SortEntry::new(render_obj, camera_position, sort_key);
        self.entries.push(entry);
    }

    /// Sort entries back-to-front (for transparency)
    pub fn sort_back_to_front(&mut self) {
        self.entries.sort_by(|a, b| a.cmp_back_to_front(b));
    }

    /// Sort entries front-to-back (for opaque with early-Z)
    pub fn sort_front_to_back(&mut self) {
        self.entries.sort_by(|a, b| a.cmp_front_to_back(b));
    }

    /// Sort entries by material for batching
    pub fn sort_by_material(&mut self) {
        self.entries.sort_by(|a, b| a.cmp_by_material(b));
    }

    /// Remove and return the first entry (matches C++ Remove_Head)
    pub fn remove_head(&mut self) -> Option<Arc<dyn RenderObjClass>> {
        if self.entries.is_empty() {
            None
        } else {
            Some(self.entries.remove(0).render_obj)
        }
    }

    /// Get entry count
    pub fn count(&self) -> usize {
        self.entries.len()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for StaticSortList {
    fn default() -> Self {
        Self::new()
    }
}

/// Default static sort list manager (matches C++ DefaultStaticSortListClass)
pub struct DefaultStaticSortListClass {
    /// Array of sort lists, one per level
    sort_lists: [StaticSortList; MAX_SORT_LEVEL + 1],
    /// Minimum sort level to render
    min_sort: usize,
    /// Maximum sort level to render
    max_sort: usize,
}

impl DefaultStaticSortListClass {
    /// Create a new default static sort list
    pub fn new() -> Self {
        // Initialize array with default sort lists
        let sort_lists = std::array::from_fn(|_| StaticSortList::new());

        Self {
            sort_lists,
            min_sort: 1,
            max_sort: MAX_SORT_LEVEL,
        }
    }

    /// Add a render object to a specific sort level
    /// Matches C++ Add_To_List
    pub fn add_to_list(
        &mut self,
        render_obj: Arc<dyn RenderObjClass>,
        sort_level: usize,
        camera_position: Vec3,
    ) {
        if !(1..=MAX_SORT_LEVEL).contains(&sort_level) {
            eprintln!(
                "Sort level {} out of range [1, {}]",
                sort_level, MAX_SORT_LEVEL
            );
            return;
        }

        self.sort_lists[sort_level].add_tail(render_obj, camera_position);
    }

    /// Render all objects and clear lists (matches C++ Render_And_Clear)
    ///
    /// Renders from higher sort level to lower (lower sort level = higher priority = front)
    /// This matches the C++ behavior where we go from MaxSort down to MinSort
    pub fn render_and_clear(
        &mut self,
        render_info: &RenderInfoClass,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Render from high sort level to low (back to front)
        for sort_level in (self.min_sort..=self.max_sort).rev() {
            let list = &mut self.sort_lists[sort_level];

            if list.is_empty() {
                continue;
            }

            // Sort the list back-to-front for correct transparency
            list.sort_back_to_front();

            // Render each object in the list
            while let Some(render_obj) = list.remove_head() {
                // Call pre_render, render if it returns true, then post_render
                if render_obj.pre_render(render_info)? {
                    render_obj.render(render_info)?;
                }
                render_obj.post_render(render_info)?;
            }

            // List is automatically cleared by remove_head loop
        }

        Ok(())
    }

    /// Get minimum sort level
    pub fn get_min_sort(&self) -> usize {
        self.min_sort
    }

    /// Get maximum sort level
    pub fn get_max_sort(&self) -> usize {
        self.max_sort
    }

    /// Set minimum sort level (clamped to MAX_SORT_LEVEL)
    pub fn set_min_sort(&mut self, value: usize) {
        self.min_sort = value.min(MAX_SORT_LEVEL);
    }

    /// Set maximum sort level (clamped to MAX_SORT_LEVEL)
    pub fn set_max_sort(&mut self, value: usize) {
        self.max_sort = value.min(MAX_SORT_LEVEL);
    }

    /// Clear all sort lists
    pub fn clear_all(&mut self) {
        for list in &mut self.sort_lists {
            list.clear();
        }
    }
}

impl Default for DefaultStaticSortListClass {
    fn default() -> Self {
        Self::new()
    }
}

/// Sort level categories for different object types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortLevelCategory {
    /// Opaque objects (sort level 0)
    Opaque = 0,
    /// Opaque decals (sort level 1-5)
    OpaqueDecal = 3,
    /// Alpha-tested objects (sort level 6-10)
    AlphaTested = 8,
    /// Transparent objects (sort level 11-20)
    Transparent = 15,
    /// Additive blended objects (sort level 21-25)
    Additive = 23,
    /// Screen-space effects (sort level 26-30)
    Screen = 28,
    /// Final overlay (sort level 31)
    Overlay = 31,
}

impl SortLevelCategory {
    /// Get the default sort level for a category
    pub fn default_level(self) -> usize {
        self as usize
    }

    /// Determine sort level from shader properties
    pub fn from_shader(shader: &crate::rendering::shader_system::shader::ShaderClass) -> Self {
        use crate::rendering::shader_system::shader::{
            AlphaTestType, DepthMaskType, DstBlendFuncType, SrcBlendFuncType,
        };

        // Check alpha test
        if shader.get_alpha_test() == AlphaTestType::Enable {
            return Self::AlphaTested;
        }

        // Check blend mode
        let src = shader.get_src_blend_func();
        let dst = shader.get_dst_blend_func();

        match (src, dst) {
            // Additive blending
            (SrcBlendFuncType::One, DstBlendFuncType::One)
            | (SrcBlendFuncType::SrcAlpha, DstBlendFuncType::One) => Self::Additive,

            // Alpha blending
            (SrcBlendFuncType::SrcAlpha, DstBlendFuncType::InvSrcAlpha) => Self::Transparent,

            // Opaque
            (SrcBlendFuncType::One, DstBlendFuncType::Zero) => {
                // Check depth write - decals don't write depth
                if shader.get_depth_mask() == DepthMaskType::Disable {
                    Self::OpaqueDecal
                } else {
                    Self::Opaque
                }
            }

            // Default to transparent if unsure
            _ => Self::Transparent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_level_range() {
        assert!(MAX_SORT_LEVEL > 0);
        assert!(MAX_SORT_LEVEL == 32);
    }

    #[test]
    fn test_sort_level_categories() {
        assert_eq!(SortLevelCategory::Opaque.default_level(), 0);
        assert_eq!(SortLevelCategory::Transparent.default_level(), 15);
        assert_eq!(SortLevelCategory::Overlay.default_level(), 31);
    }

    #[test]
    fn test_static_sort_list_creation() {
        let list = StaticSortList::new();
        assert!(list.is_empty());
        assert_eq!(list.count(), 0);
    }

    #[test]
    fn test_default_static_sort_list_class() {
        let manager = DefaultStaticSortListClass::new();
        assert_eq!(manager.get_min_sort(), 1);
        assert_eq!(manager.get_max_sort(), MAX_SORT_LEVEL);
    }
}
