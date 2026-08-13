/// Level of Detail (LOD) System
/// This module implements the LOD management system from C++ hlod.h/cpp, distlod.h/cpp, and predlod.h/cpp
///
/// The LOD system provides:
/// - Distance-based LOD switching
/// - Hierarchical LOD for animated models
/// - Predictive LOD for performance optimization
/// - Screen-space LOD metrics
use crate::{CameraClass, RenderObj};
use glam::{Mat4, Vec2, Vec3};
use std::sync::Arc;

/// W3D's authored "never switch higher than this" sentinel.
///
/// C++ writes this as `WWMATH_FLOAT_MAX`, not `-1.0`.  HLOD arrays are ordered
/// from the lowest-detail model at index zero to the highest-detail model at
/// the final index; an object bigger than a level's maximum screen area moves
/// to the next, more detailed level.
pub const NO_MAX_SCREEN_SIZE: f32 = f32::MAX;

/// C++ `RenderObjClass::AT_MIN_LOD`: a forced lower bound for an HLOD level.
pub const AT_MIN_LOD: f32 = f32::MAX;

/// C++ `RenderObjClass::AT_MAX_LOD`: marks that no more detailed level exists.
pub const AT_MAX_LOD: f32 = -1.0;

const MAX_PREDICTIVE_LOD_COST: f32 = 1.0e6;

/// All frozen camera/object data used by C++ `RenderObjClass::Get_Screen_Size`.
///
/// This deliberately does not accept a projection matrix: C++ uses the
/// viewport and view plane directly.  A future visual-dispatch pass must freeze
/// these values per view before selecting multi-LOD W3D models.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HLodScreenAreaInput {
    pub sphere_center: Vec3,
    pub sphere_radius: f32,
    pub camera_position: Vec3,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub view_plane_min: Vec2,
    pub view_plane_max: Vec2,
}

/// Transcription of C++ `RenderObjClass::Get_Screen_Size`.
///
/// Invalid frozen camera/model inputs are rejected rather than being converted
/// into a guessed distance threshold.  A zero camera-to-sphere distance is a
/// valid C++ input and yields zero because the source initializes `radius` to
/// zero before its guarded division.
pub fn calculate_hlod_screen_area(input: HLodScreenAreaInput) -> Option<f32> {
    if !input.sphere_center.is_finite()
        || !input.camera_position.is_finite()
        || !input.sphere_radius.is_finite()
        || input.sphere_radius < 0.0
        || !input.viewport_width.is_finite()
        || !input.viewport_height.is_finite()
        || input.viewport_width <= 0.0
        || input.viewport_height <= 0.0
        || !input.view_plane_min.is_finite()
        || !input.view_plane_max.is_finite()
    {
        return None;
    }

    let view_plane_size = input.view_plane_max - input.view_plane_min;
    if !view_plane_size.is_finite() || view_plane_size.x <= 0.0 || view_plane_size.y <= 0.0 {
        return None;
    }

    let distance = (input.sphere_center - input.camera_position).length();
    if !distance.is_finite() {
        return None;
    }

    let normalized_radius = if distance != 0.0 {
        input.sphere_radius / distance
    } else {
        0.0
    };
    let screen_area = std::f32::consts::PI
        * normalized_radius
        * normalized_radius
        * (input.viewport_width / view_plane_size.x)
        * (input.viewport_height / view_plane_size.y);

    screen_area.is_finite().then_some(screen_area)
}

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

/// LOD level - contains models for a single level of detail.
///
/// The source W3D array order is lowest detail to highest detail.  This is
/// important because C++ `Increment_LOD` moves to a *more* detailed index.
#[derive(Clone, Debug)]
pub struct LodLevel {
    /// Models at this LOD level
    pub models: Vec<ModelNode>,
    /// Maximum normalized screen area before C++ moves to the next detail level.
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

/// Hierarchical LOD - Animated model with multiple levels of detail.
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
    /// Array of LOD levels (from lowest to highest detail).
    pub lod_levels: Vec<LodLevel>,
    /// Current active LOD index (`0` is lowest detail).
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
        if let Some(last_lod) = self.lod_levels.len().checked_sub(1) {
            // C++ clamps instead of silently ignoring an out-of-range request.
            self.current_lod = lod.min(last_lod);
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
            // C++ refreshes static factors and initializes cost/value arrays at
            // one pixel whenever an authored threshold changes.
            self.recalculate_static_lod_factors();
            self.cost_array.clear();
            self.value_array.clear();
            if let Some(min_lod) = self.calculate_cost_value_arrays(1.0) {
                if self.current_lod < min_lod {
                    self.set_lod_level(min_lod);
                }
            }
        }
    }

    /// Get the C++ current-LOD polygon count, including additional models.
    pub fn get_num_polys(&self) -> usize {
        self.lod_levels
            .get(self.current_lod)
            .map(LodLevel::get_num_polys)
            .unwrap_or(0)
            .saturating_add(
                self.additional_models
                    .iter()
                    .map(|model| model.model.get_num_polys())
                    .sum(),
            )
    }

    /// Deprecated compatibility entry point.
    ///
    /// The lightweight `CameraClass` does not contain C++'s viewport/view-plane
    /// data and this HLOD wrapper does not retain an authoritative world-space
    /// bounding sphere.  Selecting a LOD from a guessed radius or projection is
    /// observably wrong, so this intentionally performs no selection.  Call
    /// [`Self::prepare_lod_for_screen_area`] with a frozen
    /// [`HLodScreenAreaInput`] calculation at the view-dispatch boundary.
    #[deprecated(
        note = "use prepare_lod_for_screen_area with calculate_hlod_screen_area at the frozen per-view boundary"
    )]
    pub fn prepare_lod(&mut self, _camera: &CameraClass) {}

    /// Prepare C++ cost/value arrays for one already-frozen view and enforce its
    /// authored minimum detail.  This is the per-object portion of
    /// `HLodClass::Prepare_LOD`; callers then batch all visible multi-LOD HLODs
    /// through [`optimize_prepared_hlods`] for the C++ scene-wide budget pass.
    ///
    /// Returns `false` without changing selection when the source data is hidden,
    /// absent, or malformed.  That fail-closed behavior is intentional until the
    /// active Main visual dispatcher owns the complete frozen input.
    pub fn prepare_lod_for_screen_area(&mut self, screen_area: f32) -> bool {
        if self.hidden || self.lod_levels.is_empty() {
            return false;
        }

        self.current_lod = self.current_lod.min(self.lod_levels.len() - 1);
        let Some(min_lod) = self.calculate_cost_value_arrays(screen_area) else {
            return false;
        };
        if self.current_lod < min_lod {
            self.set_lod_level(min_lod);
        }
        true
    }

    /// Increment LOD level (higher detail in the source array).
    pub fn increment_lod(&mut self) {
        if self.current_lod + 1 < self.lod_levels.len() {
            self.current_lod += 1;
        }
    }

    /// Decrement LOD level (lower detail in the source array).
    pub fn decrement_lod(&mut self) {
        if self.current_lod > 0 {
            self.current_lod -= 1;
        }
    }

    /// Calculate C++ `HLodClass::Calculate_Cost_Value_Arrays` transactionally.
    ///
    /// The returned index is the source's `minlod`, and `value_array` contains
    /// one extra `AT_MAX_LOD` sentinel so post-increment probes are exact.
    /// Malformed values are rejected rather than manufacturing a LOD choice.
    pub fn calculate_cost_value_arrays(&mut self, screen_area: f32) -> Option<usize> {
        let lod_count = self.lod_levels.len();
        if lod_count == 0
            || !screen_area.is_finite()
            || screen_area < 0.0
            || !self.lod_bias.is_finite()
        {
            return None;
        }

        let mut costs = vec![0.0; lod_count];
        let mut values = vec![0.0; lod_count + 1];

        for (i, lod) in self.lod_levels.iter().enumerate() {
            if !lod.max_screen_size.is_finite()
                || !lod.non_pixel_cost.is_finite()
                || !lod.pixel_cost_per_area.is_finite()
                || !lod.benefit_factor.is_finite()
                || lod.non_pixel_cost < 0.0
                || lod.pixel_cost_per_area < 0.0
            {
                return None;
            }

            let cost = lod.non_pixel_cost + lod.pixel_cost_per_area * screen_area;
            if !cost.is_finite() || cost <= 0.0 || cost >= MAX_PREDICTIVE_LOD_COST {
                return None;
            }
            costs[i] = cost;
        }

        let mut lod = 0;
        while lod < lod_count && self.lod_levels[lod].max_screen_size < screen_area {
            values[lod] = AT_MIN_LOD;
            lod += 1;
        }

        if lod >= lod_count {
            lod = lod_count - 1;
        } else {
            values[lod] = AT_MIN_LOD;
        }
        let min_lod = lod;

        lod += 1;
        while lod < lod_count {
            let value =
                self.lod_levels[lod].benefit_factor * screen_area * self.lod_bias / costs[lod];
            if !value.is_finite() {
                return None;
            }
            values[lod] = value;
            lod += 1;
        }
        values[lod_count] = AT_MAX_LOD;

        self.cost_array = costs;
        self.value_array = values;
        Some(min_lod)
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
        self.value_array
            .get(self.current_lod.saturating_add(1))
            .copied()
            .unwrap_or(AT_MAX_LOD)
    }

    /// Recalculate static LOD factors (cost/benefit metrics)
    pub fn recalculate_static_lod_factors(&mut self) {
        for lod in &mut self.lod_levels {
            let poly_count = lod.get_num_polys();
            let polygons = poly_count as f32;

            // C++ currently has no pixel component.  It sums non-hidden models;
            // this compatibility wrapper retains no per-submodel hidden state, so
            // its represented model list is the corresponding visible set.
            lod.pixel_cost_per_area = 0.0;
            if poly_count == 0 {
                lod.non_pixel_cost = 0.000001;
                lod.benefit_factor = 0.0;
            } else {
                lod.non_pixel_cost = polygons;
                lod.benefit_factor = 1.0 - 0.5 / (polygons * polygons);
            }
        }
    }

    /// Set the LOD bias (affects value calculations)
    pub fn set_lod_bias(&mut self, bias: f32) {
        // C++ asserts `bias > 0` in debug and then clamps it to zero in all
        // builds.  Invalid Rust input is kept fail-closed rather than poisoning
        // the next prepared value array with NaN.
        self.lod_bias = if bias.is_finite() { bias.max(0.0) } else { 0.0 };
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

#[derive(Clone, Copy, Debug)]
struct PredictiveLodHeapNode {
    item: usize,
    key: f32,
}

/// One-indexed max-heap transcribed from the local `LODHeap` in C++
/// `predlod.cpp`.  Input order intentionally resolves equal keys exactly as
/// the source heap does; do not replace this with a sorted vector because that
/// changes which equally-valued HLOD is promoted first.
struct PredictiveLodHeap {
    nodes: Vec<PredictiveLodHeapNode>,
}

impl PredictiveLodHeap {
    fn new(items: impl IntoIterator<Item = PredictiveLodHeapNode>) -> Self {
        let mut nodes = Vec::new();
        nodes.push(PredictiveLodHeapNode {
            item: usize::MAX,
            key: 0.0,
        });
        nodes.extend(items);

        let mut heap = Self { nodes };
        for index in (1..=heap.len() / 2).rev() {
            heap.downheap(index);
        }
        heap
    }

    fn len(&self) -> usize {
        self.nodes.len().saturating_sub(1)
    }

    fn top(&self) -> Option<PredictiveLodHeapNode> {
        self.nodes.get(1).copied()
    }

    /// C++ `Change_Key_Top` always down-heaps the root.
    fn change_key_top(&mut self, new_key: f32) {
        if self.len() == 0 {
            return;
        }
        self.nodes[1].key = new_key;
        self.downheap(1);
    }

    fn change_key(&mut self, item: usize, new_key: f32) {
        for index in 1..=self.len() {
            if self.nodes[index].item != item {
                continue;
            }

            let old_key = self.nodes[index].key;
            self.nodes[index].key = new_key;
            if new_key < old_key {
                self.downheap(index);
            } else {
                self.upheap(index);
            }
            break;
        }
    }

    fn upheap(&mut self, mut index: usize) {
        let node = self.nodes[index];
        self.nodes[0].key = f32::MAX;
        while self.nodes[index / 2].key <= node.key {
            self.nodes[index] = self.nodes[index / 2];
            index /= 2;
        }
        self.nodes[index] = node;
    }

    fn downheap(&mut self, mut index: usize) {
        let node = self.nodes[index];
        let count = self.len();
        while index <= count / 2 {
            let mut child_index = index * 2;
            if child_index < count && self.nodes[child_index].key < self.nodes[child_index + 1].key
            {
                child_index += 1;
            }
            if node.key >= self.nodes[child_index].key {
                break;
            }
            self.nodes[index] = self.nodes[child_index];
            index = child_index;
        }
        self.nodes[index] = node;
    }
}

fn prepared_hlod_is_valid(hlod: &HLod) -> bool {
    let lod_count = hlod.lod_levels.len();
    lod_count > 1
        && hlod.current_lod < lod_count
        && hlod.cost_array.len() == lod_count
        && hlod.value_array.len() == lod_count + 1
        && hlod
            .cost_array
            .iter()
            .all(|cost| cost.is_finite() && *cost >= 0.0 && *cost < MAX_PREDICTIVE_LOD_COST)
        && hlod.value_array.iter().all(|value| value.is_finite())
        && hlod.value_array[lod_count] == AT_MAX_LOD
}

/// Run C++ `PredictiveLODOptimizerClass::Optimize_LODs` on visible, prepared
/// multi-LOD HLODs.
///
/// `fixed_cost` is the cost C++ accumulates through `Add_Cost` for visible
/// single-LOD render objects before it optimizes the multi-LOD candidates.
/// This function is intentionally inert until a caller has an exact frozen
/// view, exact visible-object order, and a C++ scene budget.  The Generals
/// W3D game path currently disables this dynamic optimization, so Main must
/// not call it simply because a W3D asset contains multiple LOD arrays.
///
/// Returns `false` without mutating candidates when their prepared source data
/// is malformed.  Otherwise the C++ heap/update order is preserved exactly.
pub fn optimize_prepared_hlods(hlods: &mut [HLod], fixed_cost: f32, max_cost: f32) -> bool {
    if hlods.is_empty() {
        return true;
    }
    if !fixed_cost.is_finite()
        || fixed_cost < 0.0
        || !max_cost.is_finite()
        || max_cost < 0.0
        || !hlods.iter().all(prepared_hlod_is_valid)
    {
        return false;
    }

    let mut total_cost = fixed_cost;
    for hlod in hlods.iter() {
        total_cost += hlod.get_cost();
    }
    if !total_cost.is_finite() {
        return false;
    }

    let mut min_current_value_queue =
        PredictiveLodHeap::new(hlods.iter().enumerate().map(|(item, hlod)| {
            PredictiveLodHeapNode {
                item,
                key: -hlod.get_value(),
            }
        }));
    let mut max_post_increment_value_queue =
        PredictiveLodHeap::new(hlods.iter().enumerate().map(|(item, hlod)| {
            PredictiveLodHeapNode {
                item,
                key: hlod.get_post_increment_value(),
            }
        }));

    loop {
        let mut incremented_item = None;

        if total_cost <= max_cost {
            let Some(top) = max_post_increment_value_queue.top() else {
                break;
            };
            if top.key == AT_MAX_LOD {
                break;
            }

            let old_cost = hlods[top.item].get_cost();
            hlods[top.item].increment_lod();
            total_cost = total_cost - old_cost + hlods[top.item].get_cost();

            max_post_increment_value_queue
                .change_key_top(hlods[top.item].get_post_increment_value());
            min_current_value_queue.change_key(top.item, -hlods[top.item].get_value());
            incremented_item = Some(top.item);
        }

        while total_cost > max_cost {
            let Some(top) = min_current_value_queue.top() else {
                return true;
            };
            if top.key == -AT_MIN_LOD {
                return true;
            }

            let old_cost = hlods[top.item].get_cost();
            hlods[top.item].decrement_lod();
            total_cost = total_cost - old_cost + hlods[top.item].get_cost();

            min_current_value_queue.change_key_top(-hlods[top.item].get_value());
            max_post_increment_value_queue
                .change_key(top.item, hlods[top.item].get_post_increment_value());

            // C++ terminates when a single outer iteration promotes then
            // immediately demotes the same candidate.
            if incremented_item == Some(top.item) {
                return true;
            }
        }
    }

    true
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RenderInfoClass;
    use glam::{Mat4, Vec2, Vec3};

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

        hlod.add_lod_level(0.1);
        hlod.add_lod_level(0.5);
        hlod.add_lod_level(NO_MAX_SCREEN_SIZE);

        assert_eq!(hlod.get_lod_count(), 3);
        assert_eq!(hlod.get_max_screen_size(0), 0.1);
        assert_eq!(hlod.get_max_screen_size(1), 0.5);
        assert_eq!(NO_MAX_SCREEN_SIZE, f32::MAX);
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
        assert_eq!(hlod.calculate_cost_value_arrays(0.25), Some(0));

        assert!(hlod.get_cost() >= 0.0);
        assert!(hlod.get_value() >= 0.0);
    }

    #[test]
    fn hlod_screen_area_matches_cxx_rendobj_formula_and_rejects_bad_view_data() {
        let input = HLodScreenAreaInput {
            sphere_center: Vec3::new(3.0, 4.0, 0.0),
            sphere_radius: 5.0,
            camera_position: Vec3::ZERO,
            viewport_width: 800.0,
            viewport_height: 600.0,
            view_plane_min: Vec2::new(-2.0, -1.0),
            view_plane_max: Vec2::new(2.0, 1.0),
        };
        let expected = std::f32::consts::PI * 200.0 * 300.0;
        assert!((calculate_hlod_screen_area(input).unwrap() - expected).abs() < 0.5);

        let at_camera = HLodScreenAreaInput {
            sphere_center: Vec3::ZERO,
            ..input
        };
        assert_eq!(calculate_hlod_screen_area(at_camera), Some(0.0));

        let malformed_view = HLodScreenAreaInput {
            view_plane_max: Vec2::new(-2.0, 1.0),
            ..input
        };
        assert_eq!(calculate_hlod_screen_area(malformed_view), None);
    }

    #[test]
    fn hlod_cost_values_keep_cxx_min_lod_clamp_and_post_increment_sentinel() {
        let mut hlod = HLod::new("Thresholds".to_string());
        for max_screen_size in [2.0, 10.0, NO_MAX_SCREEN_SIZE] {
            hlod.add_lod_level(max_screen_size);
        }
        for lod in &mut hlod.lod_levels {
            lod.non_pixel_cost = 1.0;
            lod.pixel_cost_per_area = 0.0;
        }
        hlod.lod_levels[2].benefit_factor = 0.75;

        assert_eq!(hlod.calculate_cost_value_arrays(3.0), Some(1));
        assert_eq!(
            hlod.value_array,
            vec![AT_MIN_LOD, AT_MIN_LOD, 2.25, AT_MAX_LOD]
        );
        assert_eq!(hlod.current_lod, 0);
        assert!(hlod.prepare_lod_for_screen_area(3.0));
        assert_eq!(hlod.current_lod, 1);
        assert_eq!(hlod.get_post_increment_value(), 2.25);

        hlod.set_lod_level(usize::MAX);
        assert_eq!(hlod.current_lod, 2);
        assert_eq!(hlod.get_post_increment_value(), AT_MAX_LOD);
    }

    #[test]
    fn hlod_threshold_and_bias_setters_keep_cxx_initialization_invariants() {
        let mut hlod = HLod::new("Setters".to_string());
        hlod.add_lod_level(2.0);
        hlod.add_lod_level(NO_MAX_SCREEN_SIZE);

        // C++ Set_Max_Screen_Size recalculates at one pixel and clamps to a
        // newly required higher-detail level.
        hlod.set_max_screen_size(0, 0.5);
        assert_eq!(hlod.current_lod, 1);
        assert_eq!(hlod.value_array.last(), Some(&AT_MAX_LOD));

        // C++ clamps a release-build nonpositive bias to zero.
        hlod.set_lod_bias(-3.0);
        assert_eq!(hlod.lod_bias, 0.0);
        hlod.set_lod_bias(f32::NAN);
        assert_eq!(hlod.lod_bias, 0.0);
    }

    #[test]
    fn hlod_static_costs_and_scene_budget_optimizer_match_cxx_heap_order() {
        let mut static_factors = HLod::new("StaticFactors".to_string());
        let low = static_factors.add_lod_level(2.0);
        let high = static_factors.add_lod_level(NO_MAX_SCREEN_SIZE);
        static_factors.add_lod_model(low, MockModel::new("low", 1), 0);
        static_factors.add_lod_model(high, MockModel::new("high", 10), 0);
        static_factors.recalculate_static_lod_factors();
        assert_eq!(static_factors.lod_levels[low].non_pixel_cost, 1.0);
        assert_eq!(static_factors.lod_levels[low].pixel_cost_per_area, 0.0);
        assert_eq!(static_factors.lod_levels[low].benefit_factor, 0.5);
        assert_eq!(static_factors.lod_levels[high].non_pixel_cost, 10.0);
        assert!((static_factors.lod_levels[high].benefit_factor - 0.995).abs() < 0.000_001);

        fn prepared(name: &str, high_detail_benefit: f32) -> HLod {
            let mut hlod = HLod::new(name.to_string());
            hlod.add_lod_level(2.0);
            hlod.add_lod_level(NO_MAX_SCREEN_SIZE);
            hlod.lod_levels[0].non_pixel_cost = 1.0;
            hlod.lod_levels[1].non_pixel_cost = 10.0;
            hlod.lod_levels[1].benefit_factor = high_detail_benefit;
            assert_eq!(hlod.calculate_cost_value_arrays(1.0), Some(0));
            hlod.set_lod_level(1);
            hlod
        }

        let mut visible = vec![prepared("high-value", 0.9), prepared("low-value", 0.5)];
        // C++ starts over budget, demotes the lowest current value, then its
        // next outer iteration promotes/demotes that same tuple and stops.
        assert!(optimize_prepared_hlods(&mut visible, 0.0, 15.0));
        assert_eq!(visible[0].current_lod, 1);
        assert_eq!(visible[1].current_lod, 0);
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
