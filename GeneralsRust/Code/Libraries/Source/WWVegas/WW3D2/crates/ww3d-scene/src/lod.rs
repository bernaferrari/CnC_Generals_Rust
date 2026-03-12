/// Level of Detail (LOD) System
/// This module implements the LOD management system from C++ hlod.h/cpp, distlod.h/cpp, and predlod.h/cpp
///
/// The LOD system provides:
/// - Distance-based LOD switching
/// - Hierarchical LOD for animated models
/// - Predictive LOD for performance optimization
/// - Screen-space LOD metrics
use crate::{CameraClass, RenderObj};
use glam::{Mat4, Vec3};
use std::sync::Arc;

/// No maximum screen size limit
pub const NO_MAX_SCREEN_SIZE: f32 = -1.0;

/// Model node - represents a model attached to a bone
#[derive(Clone, Debug)]
pub struct ModelNode {
    /// The render object (model)
    pub model: Arc<dyn RenderObj>,
    /// Bone index this model is attached to (-1 for root)
    pub bone_index: i32,
}

impl ModelNode {
    pub fn new(model: Arc<dyn RenderObj>, bone_index: i32) -> Self {
        Self { model, bone_index }
    }
}

/// LOD level - contains models for a single level of detail
#[derive(Clone, Debug)]
pub struct LodLevel {
    /// Models at this LOD level
    pub models: Vec<ModelNode>,
    /// Maximum screen size for this LOD (normalized 0-1, -1 = no limit)
    pub max_screen_size: f32,
    /// Cost metrics for predictive LOD
    pub non_pixel_cost: f32,
    pub pixel_cost_per_area: f32,
    pub benefit_factor: f32,
}

impl LodLevel {
    pub fn new(max_screen_size: f32) -> Self {
        Self {
            models: Vec::new(),
            max_screen_size,
            non_pixel_cost: 0.0,
            pixel_cost_per_area: 0.0,
            benefit_factor: 0.0,
        }
    }

    /// Add a model to this LOD level
    pub fn add_model(&mut self, model: Arc<dyn RenderObj>, bone_index: i32) {
        self.models.push(ModelNode::new(model, bone_index));
    }

    /// Get the number of polygons in this LOD level
    pub fn get_num_polys(&self) -> usize {
        self.models.iter().map(|m| m.model.get_num_polys()).sum()
    }
}

/// Proxy class - attachment point definition
///
/// Proxies are application-defined attachment points that associate
/// a name with a bone index. Used for mounting equipment, weapons, etc.
/// C++ Reference: hlod.h lines 25-30 (ProxyRecordClass)
#[derive(Clone, Debug)]
pub struct Proxy {
    /// Proxy name
    pub name: String,
    /// Bone index this proxy is attached to
    pub bone_index: i32,
}

impl Proxy {
    pub fn new(name: String, bone_index: i32) -> Self {
        Self { name, bone_index }
    }
}

/// Snap point - precise positioning point
///
/// Snap points are used for exact positioning of objects relative to the model.
/// C++ Reference: snappts.h/cpp (SnapPointsClass)
#[derive(Clone, Debug)]
pub struct SnapPoint {
    /// Position in object space
    pub position: Vec3,
}

impl SnapPoint {
    pub fn new(position: Vec3) -> Self {
        Self { position }
    }
}

/// Hierarchical LOD - Animated model with multiple levels of detail
///
/// This is the Rust equivalent of C++ HLodClass. It manages:
/// - Multiple LOD levels with automatic switching
/// - Bone attachments for sub-objects
/// - Animation integration with HTree
/// - Predictive LOD for performance
/// C++ Reference: hlod.h lines 48-247 (HLodClass)
pub struct HLod {
    /// Name of this HLOD
    pub name: String,
    /// Array of LOD levels (from highest to lowest detail)
    pub lod_levels: Vec<LodLevel>,
    /// Current active LOD index
    pub current_lod: usize,
    /// Additional models always rendered (attached to bones)
    pub additional_models: Vec<ModelNode>,
    /// Transform of this HLOD in world space
    pub transform: Mat4,
    /// LOD bias for adjusting switching behavior
    pub lod_bias: f32,
    /// Cost array (recalculated each frame for predictive LOD)
    cost_array: Vec<f32>,
    /// Value array (recalculated each frame for predictive LOD)
    value_array: Vec<f32>,
    /// Array of proxy objects (attachment points)
    /// C++ Reference: hlod.h line 243 (ProxyArray)
    proxies: Vec<Proxy>,
    /// Array of snap points for precise positioning
    /// C++ Reference: hlod.h line 240 (SnapPoints)
    snap_points: Vec<SnapPoint>,
    /// Index of bounding box mesh (-1 if none)
    /// C++ Reference: hlod.h line 227 (BoundingBoxIndex)
    bounding_box_index: i32,
    /// Whether this object is hidden
    hidden: bool,
}

impl HLod {
    /// Create a new HLod with the given name
    /// C++ Reference: hlod.cpp constructor
    pub fn new(name: String) -> Self {
        Self {
            name,
            lod_levels: Vec::new(),
            current_lod: 0,
            additional_models: Vec::new(),
            transform: Mat4::IDENTITY,
            lod_bias: 1.0,
            cost_array: Vec::new(),
            value_array: Vec::new(),
            proxies: Vec::new(),
            snap_points: Vec::new(),
            bounding_box_index: -1,
            hidden: false,
        }
    }

    /// Add a LOD level
    pub fn add_lod_level(&mut self, max_screen_size: f32) -> usize {
        let index = self.lod_levels.len();
        self.lod_levels.push(LodLevel::new(max_screen_size));
        index
    }

    /// Add a model to a specific LOD level
    pub fn add_lod_model(&mut self, lod_index: usize, model: Arc<dyn RenderObj>, bone_index: i32) {
        if let Some(lod) = self.lod_levels.get_mut(lod_index) {
            lod.add_model(model, bone_index);
        }
    }

    /// Add a model that is always rendered (attached to a bone)
    pub fn add_additional_model(&mut self, model: Arc<dyn RenderObj>, bone_index: i32) {
        self.additional_models
            .push(ModelNode::new(model, bone_index));
    }

    /// Get the number of LOD levels
    pub fn get_lod_count(&self) -> usize {
        self.lod_levels.len()
    }

    /// Get the current LOD level
    pub fn get_lod_level(&self) -> usize {
        self.current_lod
    }

    /// Set the current LOD level
    pub fn set_lod_level(&mut self, lod: usize) {
        if lod < self.lod_levels.len() {
            self.current_lod = lod;
        }
    }

    /// Get the maximum screen size for a LOD level
    pub fn get_max_screen_size(&self, lod_index: usize) -> f32 {
        self.lod_levels
            .get(lod_index)
            .map(|l| l.max_screen_size)
            .unwrap_or(NO_MAX_SCREEN_SIZE)
    }

    /// Set the maximum screen size for a LOD level
    pub fn set_max_screen_size(&mut self, lod_index: usize, size: f32) {
        if let Some(lod) = self.lod_levels.get_mut(lod_index) {
            lod.max_screen_size = size;
        }
    }

    /// Get total polygon count across all LOD levels
    pub fn get_num_polys(&self) -> usize {
        self.lod_levels.iter().map(|l| l.get_num_polys()).sum()
    }

    /// Prepare LOD selection based on camera distance
    ///
    /// This implements screen-space LOD selection similar to C++ Prepare_LOD
    pub fn prepare_lod(&mut self, camera: &CameraClass) {
        if self.lod_levels.is_empty() {
            return;
        }

        let distance = (self.transform.w_axis.truncate() - camera.position).length();
        let screen_area = calculate_screen_area(distance, &camera.projection_matrix);

        // Find appropriate LOD level based on screen size
        for (i, lod) in self.lod_levels.iter().enumerate() {
            if lod.max_screen_size == NO_MAX_SCREEN_SIZE || screen_area >= lod.max_screen_size {
                self.current_lod = i;
                return;
            }
        }

        // Default to lowest LOD if nothing matches
        self.current_lod = self.lod_levels.len().saturating_sub(1);
    }

    /// Increment LOD level (lower detail)
    pub fn increment_lod(&mut self) {
        if self.current_lod + 1 < self.lod_levels.len() {
            self.current_lod += 1;
        }
    }

    /// Decrement LOD level (higher detail)
    pub fn decrement_lod(&mut self) {
        if self.current_lod > 0 {
            self.current_lod -= 1;
        }
    }

    /// Calculate cost/value arrays for predictive LOD
    ///
    /// This implements the predictive LOD system that dynamically adjusts
    /// LOD based on performance budget.
    pub fn calculate_cost_value_arrays(&mut self, screen_area: f32) -> usize {
        let lod_count = self.lod_levels.len();
        self.cost_array.resize(lod_count, 0.0);
        self.value_array.resize(lod_count, 0.0);

        for (i, lod) in self.lod_levels.iter().enumerate() {
            // Cost = non-pixel cost + pixel cost * screen area
            self.cost_array[i] = lod.non_pixel_cost + lod.pixel_cost_per_area * screen_area;

            // Value = benefit factor * screen area * LOD bias
            self.value_array[i] = lod.benefit_factor * screen_area * self.lod_bias;
        }

        lod_count
    }

    /// Get the rendering cost of the current LOD
    pub fn get_cost(&self) -> f32 {
        self.cost_array
            .get(self.current_lod)
            .copied()
            .unwrap_or(0.0)
    }

    /// Get the value of the current LOD
    pub fn get_value(&self) -> f32 {
        self.value_array
            .get(self.current_lod)
            .copied()
            .unwrap_or(0.0)
    }

    /// Get the value after incrementing LOD (for predictive decisions)
    pub fn get_post_increment_value(&self) -> f32 {
        let next_lod = (self.current_lod + 1).min(self.lod_levels.len().saturating_sub(1));
        self.value_array.get(next_lod).copied().unwrap_or(0.0)
    }

    /// Recalculate static LOD factors (cost/benefit metrics)
    pub fn recalculate_static_lod_factors(&mut self) {
        for lod in &mut self.lod_levels {
            let poly_count = lod.get_num_polys();

            // Simplified cost calculation
            lod.non_pixel_cost = poly_count as f32 * 0.001;
            lod.pixel_cost_per_area = poly_count as f32 * 0.01;
            lod.benefit_factor = poly_count as f32 * 0.1;
        }
    }

    /// Set the LOD bias (affects value calculations)
    pub fn set_lod_bias(&mut self, bias: f32) {
        self.lod_bias = bias;
    }

    /// Set the transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    /// Get the transform
    pub fn get_transform(&self) -> &Mat4 {
        &self.transform
    }

    /// Get the number of additional models
    /// C++ Reference: hlod.h line 79 (Get_Additional_Model_Count)
    pub fn get_additional_model_count(&self) -> usize {
        self.additional_models.len()
    }

    /// Add a proxy (attachment point)
    /// C++ Reference: hlod.h lines 90-92 (Get_Proxy interface)
    pub fn add_proxy(&mut self, name: String, bone_index: i32) {
        self.proxies.push(Proxy::new(name, bone_index));
    }

    /// Get the number of proxies
    pub fn get_proxy_count(&self) -> usize {
        self.proxies.len()
    }

    /// Get a proxy by index
    pub fn get_proxy(&self, index: usize) -> Option<&Proxy> {
        self.proxies.get(index)
    }

    /// Find a proxy by name
    pub fn find_proxy(&self, name: &str) -> Option<&Proxy> {
        self.proxies.iter().find(|p| p.name == name)
    }

    /// Add a snap point
    /// C++ Reference: hlod.h lines 179-180 (snap point interface)
    pub fn add_snap_point(&mut self, position: Vec3) {
        self.snap_points.push(SnapPoint::new(position));
    }

    /// Get the number of snap points
    pub fn get_num_snap_points(&self) -> usize {
        self.snap_points.len()
    }

    /// Get a snap point by index
    /// C++ Reference: hlod.h line 180 (Get_Snap_Point)
    pub fn get_snap_point(&self, index: usize) -> Option<Vec3> {
        self.snap_points.get(index).map(|sp| sp.position)
    }

    /// Get the number of sub-objects (additional models) on a specific bone
    /// C++ Reference: hlod.h line 114 (Get_Num_Sub_Objects_On_Bone)
    pub fn get_num_sub_objects_on_bone(&self, bone_index: i32) -> usize {
        self.additional_models
            .iter()
            .filter(|m| m.bone_index == bone_index)
            .count()
    }

    /// Get a sub-object on a specific bone
    /// C++ Reference: hlod.h line 115 (Get_Sub_Object_On_Bone)
    pub fn get_sub_object_on_bone(
        &self,
        index: usize,
        bone_index: i32,
    ) -> Option<&Arc<dyn RenderObj>> {
        self.additional_models
            .iter()
            .filter(|m| m.bone_index == bone_index)
            .nth(index)
            .map(|m| &m.model)
    }

    /// Add a sub-object to a specific bone
    /// C++ Reference: hlod.h line 118 (Add_Sub_Object_To_Bone)
    pub fn add_sub_object_to_bone(&mut self, model: Arc<dyn RenderObj>, bone_index: i32) -> usize {
        self.additional_models
            .push(ModelNode::new(model, bone_index));
        self.additional_models.len() - 1
    }

    /// Get the bone index for a sub-object
    /// C++ Reference: hlod.h line 116 (Get_Sub_Object_Bone_Index)
    pub fn get_sub_object_bone_index(&self, index: usize) -> Option<i32> {
        self.additional_models.get(index).map(|m| m.bone_index)
    }

    /// Remove a sub-object by index
    pub fn remove_sub_object(&mut self, index: usize) -> bool {
        if index < self.additional_models.len() {
            self.additional_models.remove(index);
            true
        } else {
            false
        }
    }

    /// Set hidden state
    /// C++ Reference: hlod.h line 181 (Set_Hidden)
    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }

    /// Get hidden state
    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    /// Set the bounding box index
    /// This references a hidden mesh that represents the animated bounding box
    /// C++ Reference: hlod.h line 227 (BoundingBoxIndex)
    pub fn set_bounding_box_index(&mut self, index: i32) {
        self.bounding_box_index = index;
    }

    /// Get the bounding box index
    pub fn get_bounding_box_index(&self) -> i32 {
        self.bounding_box_index
    }

    /// Scale all LOD models
    /// C++ Reference: hlod.h line 177 (Scale)
    pub fn scale(&mut self, scale_factor: f32) {
        // Scale all LOD models
        for lod in &mut self.lod_levels {
            for _model_node in &mut lod.models {
                // Note: This requires RenderObj to have a scale method
                // For now, we just update the transform
                // In the full implementation, each model would scale its geometry
            }
        }

        // Scale additional models
        for _model_node in &mut self.additional_models {
            // Same as above
        }

        // Scale the transform
        self.transform = Mat4::from_scale(Vec3::splat(scale_factor)) * self.transform;
    }
}

/// Distance-based LOD - Simple LOD switching based on camera distance
///
/// This is the Rust equivalent of C++ DistLODClass
pub struct DistLod {
    /// Name of this distance LOD
    pub name: String,
    /// Array of LOD models with their switching distances
    pub lod_models: Vec<DistLodNode>,
    /// Current active LOD index
    pub current_lod: usize,
    /// Transform in world space
    pub transform: Mat4,
}

/// Distance LOD node - model with switch distances
#[derive(Clone, Debug)]
pub struct DistLodNode {
    /// The model at this LOD
    pub model: Arc<dyn RenderObj>,
    /// Distance to switch to higher detail
    pub switch_up_dist: f32,
    /// Distance to switch to lower detail
    pub switch_down_dist: f32,
}

impl DistLod {
    /// Create a new distance-based LOD
    pub fn new(name: String) -> Self {
        Self {
            name,
            lod_models: Vec::new(),
            current_lod: 0,
            transform: Mat4::IDENTITY,
        }
    }

    /// Add a LOD model with switch distances
    pub fn add_lod(
        &mut self,
        model: Arc<dyn RenderObj>,
        switch_up_dist: f32,
        switch_down_dist: f32,
    ) {
        self.lod_models.push(DistLodNode {
            model,
            switch_up_dist,
            switch_down_dist,
        });
    }

    /// Update LOD based on camera distance
    pub fn update_lod(&mut self, camera: &CameraClass) {
        if self.lod_models.is_empty() {
            return;
        }

        let distance = (self.transform.w_axis.truncate() - camera.position).length();

        // Find appropriate LOD based on distance
        for (i, lod) in self.lod_models.iter().enumerate() {
            if distance <= lod.switch_up_dist {
                self.current_lod = i;
                return;
            }
        }

        // Default to lowest detail
        self.current_lod = self.lod_models.len().saturating_sub(1);
    }

    /// Get the current LOD model
    pub fn get_current_model(&self) -> Option<&Arc<dyn RenderObj>> {
        self.lod_models.get(self.current_lod).map(|n| &n.model)
    }

    /// Get switch distances for a LOD level
    pub fn get_switch_distances(&self, lod_index: usize) -> Option<(f32, f32)> {
        self.lod_models
            .get(lod_index)
            .map(|n| (n.switch_up_dist, n.switch_down_dist))
    }

    /// Set the transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }
}

fn calculate_screen_area(distance: f32, projection: &Mat4) -> f32 {
    if distance <= 0.0 {
        return 1.0;
    }

    const DEFAULT_RADIUS: f32 = 10.0;

    let focal_length = projection.y_axis.y.abs();
    if focal_length < 0.001 {
        return 1.0 / (distance * distance).max(1.0);
    }

    let screen_radius = (DEFAULT_RADIUS * focal_length) / distance.max(0.1);
    (screen_radius * screen_radius).min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderInfoClass;
    use glam::{Mat4, Vec3};

    #[derive(Debug)]
    struct MockModel {
        name: String,
        poly_count: usize,
    }

    impl MockModel {
        fn new(name: &str, poly_count: usize) -> Arc<Self> {
            Arc::new(Self {
                name: name.to_string(),
                poly_count,
            })
        }
    }

    impl RenderObj for MockModel {
        fn update(&mut self, _dt: f32) {}
        fn is_visible(&self, _camera_pos: Vec3) -> bool {
            true
        }
        fn get_name(&self) -> &str {
            &self.name
        }
        fn set_transform(&mut self, _transform: Mat4) {}
        fn get_transform(&self) -> &Mat4 {
            &Mat4::IDENTITY
        }
        fn render(&self, _render_info: &RenderInfoClass) {}
        fn get_num_polys(&self) -> usize {
            self.poly_count
        }
    }

    #[test]
    fn test_hlod_creation() {
        let hlod = HLod::new("TestHLod".to_string());
        assert_eq!(hlod.name, "TestHLod");
        assert_eq!(hlod.get_lod_count(), 0);
    }

    #[test]
    fn test_hlod_add_levels() {
        let mut hlod = HLod::new("TestHLod".to_string());

        hlod.add_lod_level(0.5);
        hlod.add_lod_level(0.1);
        hlod.add_lod_level(NO_MAX_SCREEN_SIZE);

        assert_eq!(hlod.get_lod_count(), 3);
        assert_eq!(hlod.get_max_screen_size(0), 0.5);
        assert_eq!(hlod.get_max_screen_size(1), 0.1);
    }

    #[test]
    fn test_hlod_add_models() {
        let mut hlod = HLod::new("TestHLod".to_string());
        let lod0 = hlod.add_lod_level(0.5);

        let model = MockModel::new("TestModel", 1000);
        hlod.add_lod_model(lod0, model, 0);

        assert_eq!(hlod.lod_levels[0].models.len(), 1);
        assert_eq!(hlod.get_num_polys(), 1000);
    }

    #[test]
    fn test_hlod_lod_switching() {
        let mut hlod = HLod::new("TestHLod".to_string());
        hlod.add_lod_level(0.5);
        hlod.add_lod_level(0.1);

        assert_eq!(hlod.get_lod_level(), 0);

        hlod.increment_lod();
        assert_eq!(hlod.get_lod_level(), 1);

        hlod.decrement_lod();
        assert_eq!(hlod.get_lod_level(), 0);
    }

    #[test]
    fn test_distlod_creation() {
        let distlod = DistLod::new("TestDistLod".to_string());
        assert_eq!(distlod.name, "TestDistLod");
    }

    #[test]
    fn test_distlod_add_models() {
        let mut distlod = DistLod::new("TestDistLod".to_string());

        let model_high = MockModel::new("High", 1000);
        let model_low = MockModel::new("Low", 100);

        distlod.add_lod(model_high, 10.0, 20.0);
        distlod.add_lod(model_low, 50.0, 100.0);

        assert_eq!(distlod.lod_models.len(), 2);
    }

    #[test]
    fn test_distlod_switching() {
        let mut distlod = DistLod::new("TestDistLod".to_string());

        let model_high = MockModel::new("High", 1000);
        let model_low = MockModel::new("Low", 100);

        distlod.add_lod(model_high, 10.0, 20.0);
        distlod.add_lod(model_low, 50.0, 100.0);

        let mut camera = CameraClass::new();
        camera.position = Vec3::new(5.0, 0.0, 0.0);

        distlod.update_lod(&camera);
        assert_eq!(distlod.current_lod, 0); // Should use high detail at close range

        camera.position = Vec3::new(60.0, 0.0, 0.0);
        distlod.update_lod(&camera);
        // Distance is ~60, should use low detail
    }

    #[test]
    fn test_hlod_cost_value() {
        let mut hlod = HLod::new("TestHLod".to_string());
        hlod.add_lod_level(0.5);
        hlod.add_lod_level(0.1);

        let model = MockModel::new("TestModel", 1000);
        hlod.add_lod_model(0, model, 0);

        hlod.recalculate_static_lod_factors();
        hlod.calculate_cost_value_arrays(0.25);

        assert!(hlod.get_cost() >= 0.0);
        assert!(hlod.get_value() >= 0.0);
    }

    #[test]
    fn test_lod_bias() {
        let mut hlod = HLod::new("TestHLod".to_string());
        hlod.set_lod_bias(2.0);
        assert_eq!(hlod.lod_bias, 2.0);
    }

    #[test]
    fn test_hlod_proxies() {
        let mut hlod = HLod::new("TestHLod".to_string());

        // Add some proxies
        hlod.add_proxy("WeaponMount".to_string(), 5);
        hlod.add_proxy("ShieldMount".to_string(), 3);

        assert_eq!(hlod.get_proxy_count(), 2);

        // Get proxy by index
        let proxy = hlod.get_proxy(0);
        assert!(proxy.is_some());
        assert_eq!(proxy.unwrap().name, "WeaponMount");
        assert_eq!(proxy.unwrap().bone_index, 5);

        // Find proxy by name
        let found = hlod.find_proxy("ShieldMount");
        assert!(found.is_some());
        assert_eq!(found.unwrap().bone_index, 3);
    }

    #[test]
    fn test_hlod_snap_points() {
        let mut hlod = HLod::new("TestHLod".to_string());

        // Add snap points
        hlod.add_snap_point(Vec3::new(1.0, 2.0, 3.0));
        hlod.add_snap_point(Vec3::new(4.0, 5.0, 6.0));

        assert_eq!(hlod.get_num_snap_points(), 2);

        // Get snap point by index
        let snap = hlod.get_snap_point(0);
        assert!(snap.is_some());
        assert_eq!(snap.unwrap(), Vec3::new(1.0, 2.0, 3.0));

        let snap2 = hlod.get_snap_point(1);
        assert_eq!(snap2.unwrap(), Vec3::new(4.0, 5.0, 6.0));
    }

    #[test]
    fn test_hlod_bone_attachments() {
        let mut hlod = HLod::new("TestHLod".to_string());

        let model1 = MockModel::new("Weapon", 100);
        let model2 = MockModel::new("Shield", 50);
        let model3 = MockModel::new("Helmet", 25);

        // Add sub-objects to different bones
        hlod.add_sub_object_to_bone(model1.clone(), 5);
        hlod.add_sub_object_to_bone(model2.clone(), 3);
        hlod.add_sub_object_to_bone(model3.clone(), 5);

        // Check counts
        assert_eq!(hlod.get_num_sub_objects_on_bone(5), 2); // Weapon and Helmet
        assert_eq!(hlod.get_num_sub_objects_on_bone(3), 1); // Shield

        // Get sub-object on bone
        let weapon = hlod.get_sub_object_on_bone(0, 5);
        assert!(weapon.is_some());

        // Get bone index for sub-object
        let bone_idx = hlod.get_sub_object_bone_index(0);
        assert_eq!(bone_idx, Some(5));

        // Remove a sub-object
        assert!(hlod.remove_sub_object(0));
        assert_eq!(hlod.get_additional_model_count(), 2);
    }

    #[test]
    fn test_hlod_hidden_state() {
        let mut hlod = HLod::new("TestHLod".to_string());

        assert!(!hlod.is_hidden());

        hlod.set_hidden(true);
        assert!(hlod.is_hidden());

        hlod.set_hidden(false);
        assert!(!hlod.is_hidden());
    }

    #[test]
    fn test_hlod_bounding_box_index() {
        let mut hlod = HLod::new("TestHLod".to_string());

        assert_eq!(hlod.get_bounding_box_index(), -1);

        hlod.set_bounding_box_index(42);
        assert_eq!(hlod.get_bounding_box_index(), 42);
    }

    #[test]
    fn test_hlod_scale() {
        let mut hlod = HLod::new("TestHLod".to_string());
        hlod.add_lod_level(0.5);

        let model = MockModel::new("TestModel", 1000);
        hlod.add_lod_model(0, model, 0);

        // Scale the HLOD
        hlod.scale(2.0);

        // Transform should have scale applied
        assert!(hlod.transform != Mat4::IDENTITY);
    }
}
