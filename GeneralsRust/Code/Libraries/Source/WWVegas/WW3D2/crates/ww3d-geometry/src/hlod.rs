//! Hierarchical Level of Detail (HLOD) System
//!
//! This module provides comprehensive HLOD functionality for managing
//! multiple levels of detail in 3D models, matching the C++ WW3D HLOD system.

use crate::*;
use glam::Mat4;
use std::sync::Arc;

/// C++ `NO_MAX_SCREEN_SIZE` (`w3d_file.h`) is `WWMATH_FLOAT_MAX`.
pub const NO_MAX_SCREEN_SIZE: f32 = f32::MAX;

/// C++ `RenderObjClass::AT_MIN_LOD`.
pub const AT_MIN_LOD: f32 = f32::MAX;

/// C++ `RenderObjClass::AT_MAX_LOD`.
pub const AT_MAX_LOD: f32 = -1.0;

/// Model node for HLOD sub-objects
#[derive(Debug, Clone)]
pub struct ModelNode {
    /// The render object for this model
    pub model: Option<Arc<dyn RenderObject>>,
    /// Bone index this model is attached to (-1 for root)
    pub bone_index: i32,
    /// Transform for this model node
    pub transform: Mat4,
}

impl ModelNode {
    pub fn new(model: Option<Arc<dyn RenderObject>>, bone_index: i32) -> Self {
        Self {
            model,
            bone_index,
            transform: Mat4::IDENTITY,
        }
    }
}

/// Model array for a single LOD level
#[derive(Debug, Clone)]
pub struct ModelArray {
    /// Models in this LOD level
    pub models: Vec<ModelNode>,
    /// Maximum screen size for this LOD level
    pub max_screen_size: f32,
    /// Non-pixel cost heuristics
    pub non_pixel_cost: f32,
    /// Pixel cost per area
    pub pixel_cost_per_area: f32,
    /// Benefit factor
    pub benefit_factor: f32,
}

impl Default for ModelArray {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelArray {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            max_screen_size: NO_MAX_SCREEN_SIZE,
            non_pixel_cost: 0.0,
            pixel_cost_per_area: 0.0,
            benefit_factor: 0.0,
        }
    }

    /// Add a model to this LOD level
    pub fn add_model(&mut self, model: Arc<dyn RenderObject>, bone_index: i32) {
        self.models.push(ModelNode::new(Some(model), bone_index));
    }

    /// Get model count
    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    /// Get model at index
    pub fn get_model(&self, index: usize) -> Option<&Arc<dyn RenderObject>> {
        self.models.get(index)?.model.as_ref()
    }

    /// Get bone index for model at index
    pub fn get_bone_index(&self, index: usize) -> i32 {
        self.models.get(index).map_or(-1, |node| node.bone_index)
    }
}

/// Hierarchical Level of Detail (HLOD) system
#[derive(Debug)]
pub struct HLod {
    /// Name of this HLOD
    pub name: String,
    /// LOD levels (each containing multiple models)
    pub lods: Vec<ModelArray>,
    /// Current LOD level being used
    pub current_lod: usize,
    /// Additional models (always rendered, attached to bones)
    pub additional_models: ModelArray,
    /// Snap points for model attachment
    pub snap_points: Option<SnapPoints>,
    /// Proxy objects
    pub proxy_array: Option<ProxyArray>,
    /// LOD bias factor
    pub lod_bias: f32,
    /// Transform matrix
    pub transform: Mat4,
    /// Bounding sphere
    pub bounding_sphere: Sphere,
    /// Bounding box
    pub bounding_box: AABox,
    /// Cost array for LOD calculation
    pub costs: Vec<f32>,
    /// Value array for LOD calculation
    pub values: Vec<f32>,
    /// Whether NULL LOD is included
    pub null_lod_included: bool,
    /// Animation state
    pub animation_state: Option<AnimationState>,
    /// C++ `HIDE_FLAG` for this HLOD.
    pub hidden: bool,
    /// C++ `HLodClass::BoundingBoxIndex` (`hlod.h:227`). `-1` if none.
    pub bounding_box_index: i32,
}

impl HLod {
    /// Create new HLOD with given name
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            lods: Vec::new(),
            current_lod: 0,
            additional_models: ModelArray::new(),
            snap_points: None,
            proxy_array: None,
            lod_bias: 1.0,
            transform: Mat4::IDENTITY,
            bounding_sphere: Sphere::new(Vec3::ZERO, 1.0),
            bounding_box: AABox::new(Vec3::ZERO, Vec3::ONE),
            costs: Vec::new(),
            values: Vec::new(),
            null_lod_included: false,
            animation_state: None,
            hidden: false,
            bounding_box_index: -1,
        }
    }

    /// Create HLOD from multiple LOD models
    pub fn from_models(name: &str, models: Vec<Arc<dyn RenderObject>>, model_count: usize) -> Self {
        let mut hlod = Self::new(name);

        // Create LOD levels from models
        let mut current_index = 0;
        while current_index < models.len() {
            let mut lod_array = ModelArray::new();
            let models_in_lod = (models.len() - current_index).min(model_count);

            for i in 0..models_in_lod {
                if current_index + i < models.len() {
                    lod_array.add_model(models[current_index + i].clone(), -1);
                }
            }

            hlod.lods.push(lod_array);
            current_index += model_count;
        }

        hlod.update_bounding_volumes();
        hlod.recalculate_static_lod_factors();
        hlod
    }

    /// Set maximum screen size for an LOD level
    pub fn set_max_screen_size(&mut self, lod_index: usize, size: f32) {
        if lod_index < self.lods.len() {
            self.lods[lod_index].max_screen_size = size;
        }
    }

    /// Get maximum screen size for an LOD level
    pub fn get_max_screen_size(&self, lod_index: usize) -> f32 {
        self.lods
            .get(lod_index)
            .map_or(NO_MAX_SCREEN_SIZE, |lod| lod.max_screen_size)
    }

    /// Get LOD count
    pub fn get_lod_count(&self) -> usize {
        self.lods.len()
    }

    /// Get model count in a specific LOD level
    pub fn get_lod_model_count(&self, lod_index: usize) -> usize {
        self.lods.get(lod_index).map_or(0, |lod| lod.model_count())
    }

    /// Peek at a model in a specific LOD level (without incrementing ref count)
    pub fn peek_lod_model(
        &self,
        lod_index: usize,
        model_index: usize,
    ) -> Option<&Arc<dyn RenderObject>> {
        self.lods.get(lod_index)?.get_model(model_index)
    }

    /// Get a model in a specific LOD level (with ref count)
    pub fn get_lod_model(
        &self,
        lod_index: usize,
        model_index: usize,
    ) -> Option<Arc<dyn RenderObject>> {
        self.peek_lod_model(lod_index, model_index).cloned()
    }

    /// Get bone index for a model in a specific LOD level
    pub fn get_lod_model_bone(&self, lod_index: usize, model_index: usize) -> i32 {
        self.lods
            .get(lod_index)
            .map_or(-1, |lod| lod.get_bone_index(model_index))
    }

    /// Get additional model count
    pub fn get_additional_model_count(&self) -> usize {
        self.additional_models.model_count()
    }

    /// Peek at an additional model
    pub fn peek_additional_model(&self, model_index: usize) -> Option<&Arc<dyn RenderObject>> {
        self.additional_models.get_model(model_index)
    }

    /// Get an additional model
    pub fn get_additional_model(&self, model_index: usize) -> Option<Arc<dyn RenderObject>> {
        self.peek_additional_model(model_index).cloned()
    }

    /// Get bone index for an additional model
    pub fn get_additional_model_bone(&self, model_index: usize) -> i32 {
        self.additional_models.get_bone_index(model_index)
    }

    /// Add a model to a specific LOD level
    pub fn add_lod_model(
        &mut self,
        lod_index: usize,
        model: Arc<dyn RenderObject>,
        bone_index: i32,
    ) {
        while self.lods.len() <= lod_index {
            self.lods.push(ModelArray::new());
        }
        self.lods[lod_index].add_model(model, bone_index);
        self.update_bounding_volumes();
        self.rescan_bounding_box_index();
    }

    /// Add an additional model
    pub fn add_additional_model(&mut self, model: Arc<dyn RenderObject>, bone_index: i32) {
        self.additional_models.add_model(model, bone_index);
        self.update_bounding_volumes();
    }

    /// Check if NULL LOD is included
    pub fn is_null_lod_included(&self) -> bool {
        self.null_lod_included
    }

    /// Include or exclude NULL LOD
    pub fn include_null_lod(&mut self, include: bool) {
        self.null_lod_included = include;
    }

    /// Get proxy count
    pub fn get_proxy_count(&self) -> usize {
        self.proxy_array
            .as_ref()
            .map_or(0, |proxies| proxies.proxies.len())
    }

    /// Get proxy at index
    pub fn get_proxy(&self, index: usize) -> Option<&Proxy> {
        self.proxy_array.as_ref()?.proxies.get(index)
    }

    /// Set LOD bias
    pub fn set_lod_bias(&mut self, bias: f32) {
        self.lod_bias = bias;
    }

    /// Get LOD bias
    pub fn get_lod_bias(&self) -> f32 {
        self.lod_bias
    }

    /// Set current LOD level
    pub fn set_lod_level(&mut self, lod: usize) {
        if lod < self.lods.len() {
            self.current_lod = lod;
        }
    }

    /// Get current LOD level
    pub fn get_lod_level(&self) -> usize {
        self.current_lod
    }

    /// Increment LOD level (lower detail)
    pub fn increment_lod(&mut self) {
        if self.current_lod < self.lods.len().saturating_sub(1) {
            self.current_lod += 1;
        }
    }

    /// Decrement LOD level (higher detail)
    pub fn decrement_lod(&mut self) {
        if self.current_lod > 0 {
            self.current_lod = self.current_lod.saturating_sub(1);
        }
    }

    /// Get current LOD model
    pub fn get_current_lod(&self) -> Option<&ModelArray> {
        self.lods.get(self.current_lod)
    }

    /// Get polygon count for current LOD
    pub fn get_num_polys(&self) -> usize {
        let mut total = 0;
        if let Some(current_lod) = self.get_current_lod() {
            for model_node in &current_lod.models {
                if let Some(model) = &model_node.model {
                    if model.is_not_hidden_at_all() {
                        total += model.get_num_polys();
                    }
                }
            }
        }

        for model_node in &self.additional_models.models {
            if let Some(model) = &model_node.model {
                if model.is_not_hidden_at_all() {
                    total += model.get_num_polys();
                }
            }
        }

        total
    }

    /// Prepare LOD based on camera — C++ `HLodClass::Prepare_LOD`.
    pub fn prepare_lod(&mut self, camera: &Camera) {
        if self.hidden {
            return;
        }
        let screen_area = self.calculate_screen_area(camera);
        if self.lods.len() > 1 {
            let minlod = self.calculate_cost_value_arrays_internal(screen_area);
            if self.current_lod < minlod {
                self.set_lod_level(minlod);
            }
        }
    }

    /// Recalculate static LOD factors — C++ `Recalculate_Static_LOD_Factors`.
    pub fn recalculate_static_lod_factors(&mut self) {
        for lod in &mut self.lods {
            lod.pixel_cost_per_area = 0.0;

            let mut polycount = 0usize;
            for model_node in &lod.models {
                if let Some(model) = &model_node.model {
                    if model.is_not_hidden_at_all() {
                        polycount += model.get_num_polys();
                    }
                }
            }

            lod.non_pixel_cost = if polycount != 0 {
                polycount as f32
            } else {
                0.000001
            };
            lod.benefit_factor = if polycount != 0 {
                1.0 - (0.5 / ((polycount * polycount) as f32))
            } else {
                0.0
            };
        }
    }

    /// Calculate cost/value arrays and clamp current LOD to minlod.
    fn calculate_and_select_best_lod(&mut self, screen_area: f32) {
        let minlod = self.calculate_cost_value_arrays_internal(screen_area);
        if self.current_lod < minlod {
            self.set_lod_level(minlod);
        }
    }

    /// Calculate cost/value arrays for LOD selection.
    ///
    /// Returns the C++ `minlod` index. `values` is sized `LodCount+1` with
    /// `AT_MAX_LOD` in the last slot.
    pub fn calculate_cost_value_arrays(
        &self,
        screen_area: f32,
        values: &mut Vec<f32>,
        costs: &mut Vec<f32>,
    ) -> usize {
        let lod_count = self.lods.len();
        if lod_count == 0 {
            values.clear();
            costs.clear();
            return 0;
        }

        costs.resize(lod_count, 0.0);
        values.resize(lod_count + 1, 0.0);

        for (i, lod) in self.lods.iter().enumerate() {
            costs[i] = lod.non_pixel_cost + lod.pixel_cost_per_area * screen_area;
        }

        let mut lod = 0;
        while lod < lod_count && self.lods[lod].max_screen_size < screen_area {
            values[lod] = AT_MIN_LOD;
            lod += 1;
        }

        if lod >= lod_count {
            lod = lod_count - 1;
        } else {
            values[lod] = AT_MIN_LOD;
        }
        let minlod = lod;

        lod += 1;
        while lod < lod_count {
            let cost = costs[lod];
            values[lod] = if cost != 0.0 {
                (self.lods[lod].benefit_factor * screen_area * self.lod_bias) / cost
            } else {
                0.0
            };
            lod += 1;
        }
        values[lod_count] = AT_MAX_LOD;
        minlod
    }

    fn calculate_cost_value_arrays_internal(&mut self, screen_area: f32) -> usize {
        let mut values = Vec::new();
        let mut costs = Vec::new();
        let minlod = self.calculate_cost_value_arrays(screen_area, &mut values, &mut costs);
        self.values = values;
        self.costs = costs;
        minlod
    }

    /// Get cost for current LOD
    pub fn get_cost(&self) -> f32 {
        self.costs.get(self.current_lod).copied().unwrap_or(0.0)
    }

    /// Get value for current LOD
    pub fn get_value(&self) -> f32 {
        self.values.get(self.current_lod).copied().unwrap_or(0.0)
    }

    /// Get value for next LOD level (after increment)
    pub fn get_post_increment_value(&self) -> f32 {
        self.values
            .get(self.current_lod.saturating_add(1))
            .copied()
            .unwrap_or(AT_MAX_LOD)
    }

    /// Set transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        self.update_sub_object_transforms();
    }

    /// Set position
    pub fn set_position(&mut self, position: Vec3) {
        self.transform = Mat4::from_translation(position);
        self.update_sub_object_transforms();
    }

    /// Get transform
    pub fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    /// Get bounding sphere
    pub fn get_bounding_sphere(&self) -> &Sphere {
        &self.bounding_sphere
    }

    /// Get bounding box
    pub fn get_bounding_box(&self) -> &AABox {
        &self.bounding_box
    }

    /// Get object space bounding sphere
    pub fn get_obj_space_bounding_sphere(&self) -> Sphere {
        if self.bounding_box_index >= 0 {
            let box_ = self.get_obj_space_bounding_box();
            return crate::hlod_bounding_box::sphere_from_aabox(&box_);
        }
        self.bounding_sphere
    }

    /// Get object space bounding box
    pub fn get_obj_space_bounding_box(&self) -> AABox {
        if let Some(box_) = self.posed_bounding_box_obj_space() {
            return box_;
        }
        self.bounding_box
    }

    fn rescan_bounding_box_index(&mut self) {
        self.bounding_box_index = crate::hlod_bounding_box::scan_bounding_box_index(self);
    }

    fn posed_bounding_box_obj_space(&self) -> Option<AABox> {
        let index = usize::try_from(self.bounding_box_index).ok()?;
        let high = self.lods.last()?;
        let node = high.models.get(index)?;
        let model = node.model.as_ref()?;
        if !crate::hlod_bounding_box::is_bounding_box_obbox(model.get_name(), model.class_id()) {
            return None;
        }
        let (center, extent) = model.local_center_extent().unwrap_or_else(|| {
            (
                model.get_bounding_box().center,
                model.get_bounding_box().extent,
            )
        });
        Some(crate::hlod_bounding_box::obj_space_box_from_obbox(
            self.transform,
            node.transform,
            center,
            extent,
        ))
    }

    /// Get snap point count
    pub fn get_num_snap_points(&self) -> usize {
        self.snap_points.as_ref().map_or(0, |sp| sp.points.len())
    }

    /// Get snap point at index
    pub fn get_snap_point(&self, index: usize, point: &mut Vec3) {
        if let Some(snap_points) = &self.snap_points {
            if let Some(sp) = snap_points.points.get(index) {
                *point = *sp;
            }
        }
    }

    /// Set hidden state — C++ `HLodClass::Set_Hidden` stores HIDE_FLAG.
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    /// C++ `Is_Not_Hidden_At_All`.
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Scale the HLOD
    pub fn scale(&mut self, scale: f32) {
        // Scale the transform
        let scale_vec = Vec3::new(scale, scale, scale);
        // Apply scaling to transform
        let scale_matrix = Mat4::from_scale(scale_vec);
        self.transform = scale_matrix * self.transform;

        // Scale bounding volumes
        self.bounding_sphere.radius *= scale;
        self.bounding_box.extent *= scale_vec;

        // Scale all sub-objects
        for lod in &mut self.lods {
            for model_node in &mut lod.models {
                if let Some(_model) = &mut model_node.model {
                    // Scale individual models
                    // model.scale(scale);
                }
            }
        }

        for model_node in &mut self.additional_models.models {
            if let Some(_model) = &mut model_node.model {
                // Scale individual models
                // model.scale(scale);
            }
        }

        self.update_sub_object_transforms();
    }

    /// Get sub-object count
    pub fn get_num_sub_objects(&self) -> usize {
        let current_lod_count = self.get_current_lod().map_or(0, |lod| lod.model_count());
        current_lod_count + self.additional_models.model_count()
    }

    /// Get sub-object at index
    pub fn get_sub_object(&self, index: usize) -> Option<Arc<dyn RenderObject>> {
        let current_lod_count = self.get_current_lod().map_or(0, |lod| lod.model_count());

        if index < current_lod_count {
            self.get_current_lod()
                .and_then(|lod| lod.get_model(index))
                .cloned()
        } else {
            let additional_index = index - current_lod_count;
            self.additional_models.get_model(additional_index).cloned()
        }
    }

    /// Add sub-object
    pub fn add_sub_object(&mut self, subobj: Arc<dyn RenderObject>) -> usize {
        self.additional_models.add_model(subobj, -1);
        self.update_bounding_volumes();
        self.additional_models.model_count() - 1
    }

    /// Remove sub-object
    pub fn remove_sub_object(&mut self, subobj: &Arc<dyn RenderObject>) -> bool {
        // Remove from additional models
        self.additional_models.models.retain(
            |node| !matches!(node.model.as_ref(), Some(model) if Arc::ptr_eq(model, subobj)),
        );
        self.update_bounding_volumes();
        true
    }

    /// Get sub-objects on a specific bone
    pub fn get_num_sub_objects_on_bone(&self, bone_index: i32) -> usize {
        let mut count = 0;

        // Count in current LOD
        if let Some(current_lod) = self.get_current_lod() {
            count += current_lod
                .models
                .iter()
                .filter(|node| node.bone_index == bone_index)
                .count();
        }

        // Count in additional models
        count += self
            .additional_models
            .models
            .iter()
            .filter(|node| node.bone_index == bone_index)
            .count();

        count
    }

    /// Get sub-object on bone at index
    pub fn get_sub_object_on_bone(
        &self,
        index: usize,
        bone_index: i32,
    ) -> Option<Arc<dyn RenderObject>> {
        let mut current_index = 0;

        // Search in current LOD
        if let Some(current_lod) = self.get_current_lod() {
            for model_node in &current_lod.models {
                if model_node.bone_index == bone_index {
                    if current_index == index {
                        return model_node.model.clone();
                    }
                    current_index += 1;
                }
            }
        }

        // Search in additional models
        for model_node in &self.additional_models.models {
            if model_node.bone_index == bone_index {
                if current_index == index {
                    return model_node.model.clone();
                }
                current_index += 1;
            }
        }

        None
    }

    /// Get bone index for sub-object
    pub fn get_sub_object_bone_index(&self, subobj: &Arc<dyn RenderObject>) -> i32 {
        // Search in current LOD
        if let Some(current_lod) = self.get_current_lod() {
            for model_node in &current_lod.models {
                if matches!(model_node.model.as_ref(), Some(model) if Arc::ptr_eq(model, subobj)) {
                    return model_node.bone_index;
                }
            }
        }

        // Search in additional models
        for model_node in &self.additional_models.models {
            if matches!(model_node.model.as_ref(), Some(model) if Arc::ptr_eq(model, subobj)) {
                return model_node.bone_index;
            }
        }

        -1
    }

    /// Add sub-object to bone
    pub fn add_sub_object_to_bone(
        &mut self,
        subobj: Arc<dyn RenderObject>,
        bone_index: i32,
    ) -> usize {
        self.additional_models.add_model(subobj, bone_index);
        self.update_bounding_volumes();
        self.additional_models.model_count() - 1
    }

    // Internal helper methods
    fn update_bounding_volumes(&mut self) {
        self.rescan_bounding_box_index();
        if let Some(local) = self.posed_bounding_box_obj_space() {
            self.bounding_box = crate::hlod_bounding_box::transform_center_extent_aabox(
                local.center,
                local.extent,
                self.transform,
            );
            self.bounding_sphere = Sphere::new(self.bounding_box.center, local.extent.length());
            return;
        }

        // Calculate combined bounding volumes from all models
        let min = Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY);
        let max = Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);

        // Include all LOD models
        for lod in &self.lods {
            for model_node in &lod.models {
                if let Some(_model) = &model_node.model {
                    // Get bounding box from model
                    // let model_box = model.get_bounding_box();
                    // min = min.min(model_box.min());
                    // max = max.max(model_box.max());
                }
            }
        }

        // Include additional models
        for model_node in &self.additional_models.models {
            if let Some(_model) = &model_node.model {
                // Get bounding box from model
                // let model_box = model.get_bounding_box();
                // min = min.min(model_box.min());
                // max = max.max(model_box.max());
            }
        }

        // Update bounding box
        let center = (min + max) / 2.0;
        let extent = (max - min) / 2.0;
        self.bounding_box = AABox::new(center, extent);

        // Update bounding sphere
        let radius = extent.length();
        self.bounding_sphere = Sphere::new(center, radius);
    }

    fn update_sub_object_transforms(&mut self) {
        // Update transforms for all sub-objects based on bone positions
        // This requires animation system integration
        // For now, apply the HLOD transform to all sub-objects
        for lod in &mut self.lods {
            for model_node in &mut lod.models {
                model_node.transform = self.transform * model_node.transform;
            }
        }

        for model_node in &mut self.additional_models.models {
            model_node.transform = self.transform * model_node.transform;
        }
    }

    fn calculate_screen_area(&self, camera: &Camera) -> f32 {
        // C++ `RenderObjClass::Get_Screen_Size`.
        let view_plane_w = camera.view_plane_max.x - camera.view_plane_min.x;
        let view_plane_h = camera.view_plane_max.y - camera.view_plane_min.y;
        if view_plane_w <= 0.0 || view_plane_h <= 0.0 {
            return 0.0;
        }
        let width_factor = camera.viewport_width / view_plane_w;
        let height_factor = camera.viewport_height / view_plane_h;
        let dist = (self.bounding_sphere.center - camera.position).length();
        let radius = if dist != 0.0 {
            self.bounding_sphere.radius / dist
        } else {
            0.0
        };
        std::f32::consts::PI * radius * radius * width_factor * height_factor
    }

    #[allow(dead_code)] // C++ parity
    fn select_best_lod(&mut self) {
        if self.values.is_empty() || self.costs.is_empty() {
            return;
        }

        self.select_best_lod_from_arrays(self.lods.len());
    }

    fn select_best_lod_from_arrays(&mut self, lod_count: usize) {
        // Find LOD with best value/cost ratio
        let mut best_lod = 0;
        let mut best_ratio = 0.0;

        for i in 0..lod_count.min(self.values.len()).min(self.costs.len()) {
            let value = self.values[i];
            let cost = self.costs[i];

            if cost > 0.0 {
                let ratio = value / cost;
                if ratio > best_ratio {
                    best_ratio = ratio;
                    best_lod = i;
                }
            }
        }

        self.current_lod = best_lod;
    }
}

impl Clone for HLod {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            lods: self.lods.clone(),
            current_lod: self.current_lod,
            additional_models: self.additional_models.clone(),
            snap_points: self.snap_points.clone(),
            proxy_array: self.proxy_array.clone(),
            lod_bias: self.lod_bias,
            transform: self.transform,
            bounding_sphere: self.bounding_sphere,
            bounding_box: self.bounding_box,
            costs: self.costs.clone(),
            values: self.values.clone(),
            null_lod_included: self.null_lod_included,
            animation_state: self.animation_state.clone(),
            hidden: self.hidden,
            bounding_box_index: self.bounding_box_index,
        }
    }
}

/// Snap points for model attachment
#[derive(Debug, Clone)]
pub struct SnapPoints {
    pub points: Vec<Vec3>,
}

/// Proxy objects for application-defined usage
#[derive(Debug, Clone)]
pub struct ProxyArray {
    pub proxies: Vec<Proxy>,
}

/// Proxy object
#[derive(Debug, Clone)]
pub struct Proxy {
    pub name: String,
    pub bone_index: i32,
}

/// Animation state for HLOD
#[derive(Debug, Clone)]
pub struct AnimationState {
    // Animation state fields would be added here when animation system is integrated
    pub current_time: f32,
    pub playing: bool,
}

/// Camera for LOD calculations
#[derive(Debug, Clone)]
pub struct Camera {
    pub position: Vec3,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub view_plane_min: glam::Vec2,
    pub view_plane_max: glam::Vec2,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            viewport_width: 1.0,
            viewport_height: 1.0,
            view_plane_min: glam::Vec2::new(-1.0, -1.0),
            view_plane_max: glam::Vec2::new(1.0, 1.0),
        }
    }
}

/// Render object trait (placeholder - should be defined elsewhere)
pub trait RenderObject: std::fmt::Debug + Send + Sync {
    // Basic render object interface
    #[cfg(feature = "wgpu")]
    fn render(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
    ) -> Result<(), Box<dyn std::error::Error>>;
    fn get_bounding_box(&self) -> &AABox;
    fn get_transform(&self) -> &Mat4;
    fn set_transform(&mut self, transform: Mat4);
    fn get_name(&self) -> &str;
    fn get_num_polys(&self) -> usize {
        0
    }
    fn is_not_hidden_at_all(&self) -> bool {
        true
    }
    fn class_id(&self) -> u32 {
        0
    }
    fn local_center_extent(&self) -> Option<(Vec3, Vec3)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Mock render object for testing
    #[derive(Debug)]
    struct MockRenderObject {
        bounding_box: AABox,
        transform: Mat4,
        name: String,
    }

    impl MockRenderObject {
        fn new() -> Self {
            Self {
                bounding_box: AABox::new(Vector3::ZERO, Vector3::ONE),
                transform: Mat4::IDENTITY,
                name: String::new(),
            }
        }
    }

    impl RenderObject for MockRenderObject {
        #[cfg(feature = "wgpu")]
        fn render(&self, _render_pass: &mut wgpu::RenderPass) -> Result<()> {
            Ok(())
        }

        fn get_bounding_box(&self) -> &AABox {
            &self.bounding_box
        }

        fn get_transform(&self) -> &Mat4 {
            &self.transform
        }

        fn set_transform(&mut self, transform: Mat4) {
            self.transform = transform;
        }

        fn get_name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_hlod_creation() {
        let hlod = HLod::new("test_hlod");
        assert_eq!(hlod.name, "test_hlod");
        assert_eq!(hlod.get_lod_count(), 0);
        assert_eq!(hlod.current_lod, 0);
    }

    #[test]
    fn test_hlod_from_models() {
        let models: Vec<Arc<dyn RenderObject>> = vec![
            Arc::new(MockRenderObject::new()),
            Arc::new(MockRenderObject::new()),
            Arc::new(MockRenderObject::new()),
        ];

        let hlod = HLod::from_models("test_hlod", models, 2);
        assert_eq!(hlod.name, "test_hlod");
        assert_eq!(hlod.get_lod_count(), 2);
        assert_eq!(hlod.get_lod_model_count(0), 2);
        assert_eq!(hlod.get_lod_model_count(1), 1);
    }

    #[test]
    fn test_lod_management() {
        let mut hlod = HLod::new("test_hlod");

        let model1 = Arc::new(MockRenderObject::new());
        let model2 = Arc::new(MockRenderObject::new());

        hlod.add_lod_model(0, model1.clone(), -1);
        hlod.add_lod_model(0, model2.clone(), 0);

        assert_eq!(hlod.get_lod_count(), 1);
        assert_eq!(hlod.get_lod_model_count(0), 2);
        assert_eq!(hlod.get_lod_model_bone(0, 1), 0);

        // Test current LOD
        hlod.set_lod_level(0);
        assert_eq!(hlod.get_lod_level(), 0);

        // Test LOD increment/decrement
        hlod.increment_lod();
        assert_eq!(hlod.get_lod_level(), 0); // Can't increment beyond available LODs

        hlod.add_lod_model(1, Arc::new(MockRenderObject::new()), -1);
        hlod.increment_lod();
        assert_eq!(hlod.get_lod_level(), 1);

        hlod.decrement_lod();
        assert_eq!(hlod.get_lod_level(), 0);
    }

    #[test]
    fn test_additional_models() {
        let mut hlod = HLod::new("test_hlod");

        let model = Arc::new(MockRenderObject::new());
        hlod.add_additional_model(model.clone(), 5);

        assert_eq!(hlod.get_additional_model_count(), 1);
        assert_eq!(hlod.get_additional_model_bone(0), 5);
        assert_eq!(hlod.get_num_sub_objects(), 1);
    }

    #[test]
    fn test_sub_object_management() {
        let mut hlod = HLod::new("test_hlod");

        let model1 = Arc::new(MockRenderObject::new());
        let model2 = Arc::new(MockRenderObject::new());

        let index1 = hlod.add_sub_object(model1.clone());
        let index2 = hlod.add_sub_object_to_bone(model2.clone(), 10);

        assert_eq!(index1, 0);
        assert_eq!(index2, 1);
        assert_eq!(hlod.get_num_sub_objects(), 2);
        assert_eq!(hlod.get_num_sub_objects_on_bone(10), 1);
        let model_trait: Arc<dyn RenderObject> = model2.clone();
        assert_eq!(hlod.get_sub_object_bone_index(&model_trait), 10);
    }
}
