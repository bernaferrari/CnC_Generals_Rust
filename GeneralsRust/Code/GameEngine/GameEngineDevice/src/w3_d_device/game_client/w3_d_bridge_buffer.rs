//! CPU-side parity port for C++ `W3DDevice/GameClient/W3DBridgeBuffer.cpp`.

/// Maximum bridge vertices accepted by C++ `W3DBridgeBuffer`.
pub const MAX_BRIDGE_VERTEX: usize = 12_000;
/// C++ bridge index buffer capacity.
pub const MAX_BRIDGE_INDEX: usize = 2 * MAX_BRIDGE_VERTEX;
/// Maximum bridge count.
pub const MAX_BRIDGES: usize = 200;
/// C++ raises bridge endpoints slightly above terrain.
pub const BRIDGE_FLOAT_AMT: f32 = 0.25;

/// C++ `BodyDamageType` subset used by bridge rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BridgeDamageState {
    /// `BODY_PRISTINE`.
    Pristine = 0,
    /// `BODY_DAMAGED`.
    Damaged = 1,
    /// `BODY_REALLYDAMAGED`.
    ReallyDamaged = 2,
    /// `BODY_RUBBLE`.
    Rubble = 3,
}

/// C++ bridge model type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeType {
    /// Single fixed bridge mesh.
    Fixed,
    /// Left/span/right sectional bridge mesh.
    Sectional,
}

/// C++ `Vector3` subset.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vec3 {
    /// X component.
    pub x: f32,
    /// Y component.
    pub y: f32,
    /// Z component.
    pub z: f32,
}

impl Vec3 {
    /// Construct a vector.
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    fn dot(self, rhs: Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    fn length2(self) -> f32 {
        self.dot(self)
    }

    fn normalize(self) -> Self {
        let len = self.length();
        if len > f32::EPSILON {
            self / len
        } else {
            self
        }
    }
}

impl std::ops::Add for Vec3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl std::ops::Mul<f32> for Vec3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::new(self.x * rhs, self.y * rhs, self.z * rhs)
    }
}

impl std::ops::Div<f32> for Vec3 {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self::new(self.x / rhs, self.y / rhs, self.z / rhs)
    }
}

/// Minimal C++ `Matrix3D` equivalent for bridge mesh transforms.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix3D {
    m: [[f32; 4]; 3],
}

impl Matrix3D {
    /// Identity transform.
    pub const fn identity() -> Self {
        Self {
            m: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ],
        }
    }

    /// Translation transform.
    pub const fn translation(x: f32, y: f32, z: f32) -> Self {
        Self {
            m: [[1.0, 0.0, 0.0, x], [0.0, 1.0, 0.0, y], [0.0, 0.0, 1.0, z]],
        }
    }

    fn transform_vector(self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z + self.m[0][3],
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z + self.m[1][3],
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z + self.m[2][3],
        )
    }

    fn rotate_vector(self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        )
    }
}

impl Default for Matrix3D {
    fn default() -> Self {
        Self::identity()
    }
}

/// Source mesh vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BridgeMeshVertex {
    /// Local position.
    pub position: Vec3,
    /// Local normal.
    pub normal: Vec3,
    /// U coordinate.
    pub u: f32,
    /// V coordinate.
    pub v: f32,
}

/// Mesh section loaded from the W3D asset manager.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeMesh {
    /// Mesh vertices.
    pub vertices: Vec<BridgeMeshVertex>,
    /// Triangle indices.
    pub triangles: Vec<[u16; 3]>,
    /// Subobject transform.
    pub transform: Matrix3D,
}

/// Loaded bridge model for one damage state.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeModel {
    /// Texture chosen from the terrain bridge template.
    pub texture_name: String,
    /// Model chosen from the terrain bridge template.
    pub model_name: String,
    /// Bridge width scale.
    pub scale: f32,
    /// Left or fixed mesh.
    pub left: BridgeMesh,
    /// Repeating span mesh.
    pub section: Option<BridgeMesh>,
    /// Right mesh.
    pub right: Option<BridgeMesh>,
}

/// Bridge model provider. Production code should bridge this to `W3DAssetManager`.
pub trait BridgeAssetProvider {
    /// Load the model/texture set for a bridge template and damage state.
    fn load_bridge_model(
        &self,
        template_name: &str,
        damage: BridgeDamageState,
    ) -> Option<BridgeModel>;
}

/// Bridge info sent to terrain logic by C++ `addBridgeToLogic`.
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeInfo {
    /// Bridge start.
    pub from: Vec3,
    /// Bridge end.
    pub to: Vec3,
    /// Width in world units.
    pub bridge_width: f32,
    /// Start-left corner.
    pub from_left: Vec3,
    /// Start-right corner.
    pub from_right: Vec3,
    /// End-left corner.
    pub to_left: Vec3,
    /// End-right corner.
    pub to_right: Vec3,
    /// Bridge index in the buffer.
    pub bridge_index: i32,
    /// Current damage state.
    pub cur_damage_state: BridgeDamageState,
}

/// Render vertex equivalent to C++ `VertexFormatXYZNDUV1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BridgeVertex {
    /// World X.
    pub x: f32,
    /// World Y.
    pub y: f32,
    /// World Z.
    pub z: f32,
    /// Packed diffuse color.
    pub diffuse: u32,
    /// Normal X.
    pub nx: f32,
    /// Normal Y.
    pub ny: f32,
    /// Normal Z.
    pub nz: f32,
    /// Texture U.
    pub u1: f32,
    /// Texture V.
    pub v1: f32,
}

/// Cached CPU bridge geometry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BridgeRenderBuffers {
    /// Render vertices.
    pub vertices: Vec<BridgeVertex>,
    /// Render indices.
    pub indices: Vec<u16>,
}

/// Individual C++ `W3DBridge` state.
#[derive(Debug, Clone)]
pub struct W3DBridge {
    start: Vec3,
    end: Vec3,
    scale: f32,
    length: f32,
    bridge_type: BridgeType,
    texture_name: String,
    model_name: String,
    left_mesh: Option<BridgeMesh>,
    section_mesh: Option<BridgeMesh>,
    right_mesh: Option<BridgeMesh>,
    min_y: f32,
    max_y: f32,
    left_min_x: f32,
    left_max_x: f32,
    section_min_x: f32,
    section_max_x: f32,
    right_min_x: f32,
    right_max_x: f32,
    first_index: usize,
    num_vertex: usize,
    first_vertex: usize,
    num_polygons: usize,
    visible: bool,
    template_name: String,
    cur_damage_state: BridgeDamageState,
    enabled: bool,
}

impl W3DBridge {
    /// Construct an empty bridge record.
    pub fn new() -> Self {
        Self {
            start: Vec3::default(),
            end: Vec3::default(),
            scale: 1.0,
            length: 1.0,
            bridge_type: BridgeType::Fixed,
            texture_name: String::new(),
            model_name: String::new(),
            left_mesh: None,
            section_mesh: None,
            right_mesh: None,
            min_y: 0.0,
            max_y: 0.0,
            left_min_x: 0.0,
            left_max_x: 0.0,
            section_min_x: 0.0,
            section_max_x: 0.0,
            right_min_x: 0.0,
            right_max_x: 0.0,
            first_index: 0,
            num_vertex: 0,
            first_vertex: 0,
            num_polygons: 0,
            visible: false,
            template_name: String::new(),
            cur_damage_state: BridgeDamageState::Pristine,
            enabled: true,
        }
    }

    /// C++ `init`.
    pub fn init(&mut self, from_loc: Vec3, to_loc: Vec3, template_name: impl Into<String>) {
        self.start = from_loc;
        self.end = to_loc;
        self.template_name = template_name.into();
        self.enabled = true;
    }

    /// C++ `load`.
    pub fn load<P: BridgeAssetProvider>(
        &mut self,
        provider: &P,
        damage: BridgeDamageState,
    ) -> bool {
        self.clear_bridge();
        let Some(model) = provider.load_bridge_model(&self.template_name, damage) else {
            return false;
        };

        self.texture_name = model.texture_name;
        self.model_name = model.model_name;
        self.scale = model.scale;
        self.left_mesh = Some(model.left);
        self.section_mesh = model.section;
        self.right_mesh = model.right;
        self.cur_damage_state = damage;
        self.bridge_type = if self.section_mesh.is_some() && self.right_mesh.is_some() {
            BridgeType::Sectional
        } else {
            BridgeType::Fixed
        };

        let Some(left_mesh) = self.left_mesh.as_ref() else {
            self.clear_bridge();
            return false;
        };
        let Some((left_min_x, left_max_x, min_y, max_y)) = section_bounds(left_mesh, true) else {
            self.clear_bridge();
            return false;
        };
        self.left_min_x = left_min_x;
        self.left_max_x = left_max_x;
        self.min_y = min_y;
        self.max_y = max_y;

        if self.bridge_type == BridgeType::Sectional {
            if let Some((min_x, max_x, _, _)) = self
                .section_mesh
                .as_ref()
                .and_then(|mesh| section_bounds(mesh, false))
            {
                self.section_min_x = min_x;
                self.section_max_x = max_x;
            }
            if let Some((min_x, max_x, _, _)) = self
                .right_mesh
                .as_ref()
                .and_then(|mesh| section_bounds(mesh, false))
            {
                self.right_min_x = min_x;
                self.right_max_x = max_x;
            }
        } else {
            self.section_min_x = self.left_max_x;
            self.section_max_x = self.left_max_x;
            self.right_min_x = self.left_max_x;
            self.right_max_x = self.left_max_x;
        }

        self.length = (self.right_max_x - self.left_min_x).max(1.0);
        if self.bridge_type == BridgeType::Sectional {
            let allowable_error = 0.05 * self.length;
            if self.left_max_x > self.section_min_x + allowable_error
                || self.right_min_x < self.section_max_x - allowable_error
            {
                self.bridge_type = BridgeType::Fixed;
            }
        }

        true
    }

    /// C++ `clearBridge`.
    pub fn clear_bridge(&mut self) {
        self.visible = false;
        self.texture_name.clear();
        self.model_name.clear();
        self.left_mesh = None;
        self.section_mesh = None;
        self.right_mesh = None;
    }

    /// C++ culling currently forces bridges visible and reports changes.
    pub fn cull_bridge(&mut self) -> bool {
        let was_visible = self.visible;
        self.visible = true;
        was_visible != self.visible
    }

    /// Return current bridge info for terrain logic.
    pub fn get_bridge_info(&self) -> BridgeInfo {
        let vec = self.end - self.start;
        let vec_normal = Vec3::new(-vec.y, vec.x, 0.0).normalize();
        let from_left = self.start + vec_normal * (self.max_y * self.scale);
        let from_right = self.start + vec_normal * (self.min_y * self.scale);
        let to_left = self.end + vec_normal * (self.max_y * self.scale);
        let to_right = self.end + vec_normal * (self.min_y * self.scale);

        BridgeInfo {
            from: self.start,
            to: self.end,
            bridge_width: (self.max_y - self.min_y) * self.scale,
            from_left,
            from_right,
            to_left,
            to_right,
            bridge_index: -1,
            cur_damage_state: self.cur_damage_state,
        }
    }

    /// C++ `getIndicesNVertices`.
    pub fn append_indices_and_vertices(&mut self, buffers: &mut BridgeRenderBuffers, diffuse: u32) {
        self.first_vertex = buffers.vertices.len();
        self.first_index = buffers.indices.len();
        self.num_vertex = 0;
        self.num_polygons = 0;

        if self.section_mesh.is_none() || self.bridge_type == BridgeType::Fixed {
            let Some(mesh) = self.left_mesh.clone() else {
                return;
            };
            self.append_fixed_mesh(buffers, &mesh, diffuse);
            return;
        }

        let Some(left) = self.left_mesh.clone() else {
            return;
        };
        let Some(section) = self.section_mesh.clone() else {
            return;
        };
        let Some(right) = self.right_mesh.clone() else {
            return;
        };

        let mut vec = self.end - self.start;
        if vec.length2() < 1.0 {
            vec = vec.normalize();
        }
        let mut vec_normal = Vec3::new(-vec.y, vec.x, 0.0).normalize();
        vec_normal = vec_normal * self.scale;

        let desired_length = vec.length().max(1.0);
        let delta_z = (self.end.z - self.start.z) / desired_length;
        let delta_x = (1.0 - delta_z * delta_z).max(0.0).sqrt();
        let vec_z = Vec3::new(-delta_z, 0.0, delta_x) * self.scale;

        let span_length = self.right_min_x - self.left_max_x;
        let mut num_spans = 1;
        if self.bridge_type != BridgeType::Fixed && span_length.abs() > f32::EPSILON {
            let spannable = desired_length - (self.length - span_length);
            num_spans = ((spannable + span_length / 2.0) / span_length).floor() as i32;
            if num_spans < 0 {
                num_spans = 0;
            }
        }

        let bridge_length = (self.length + (num_spans - 1) as f32 * span_length).max(1.0);
        vec = vec / bridge_length;
        let x_offset = -self.left_min_x;

        self.append_mesh(buffers, &left, x_offset, vec, vec_normal, vec_z, diffuse);
        for i in 0..num_spans {
            self.append_mesh(
                buffers,
                &section,
                x_offset + i as f32 * span_length,
                vec,
                vec_normal,
                vec_z,
                diffuse,
            );
        }
        self.append_mesh(
            buffers,
            &right,
            x_offset + (num_spans - 1) as f32 * span_length,
            vec,
            vec_normal,
            vec_z,
            diffuse,
        );
    }

    /// Current damage state.
    pub fn damage_state(&self) -> BridgeDamageState {
        self.cur_damage_state
    }

    /// Set enabled flag.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Whether bridge is enabled.
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    /// Whether bridge is visible.
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// Chosen bridge type.
    pub fn bridge_type(&self) -> BridgeType {
        self.bridge_type
    }

    /// Template name.
    pub fn template_name(&self) -> &str {
        &self.template_name
    }

    fn append_fixed_mesh(
        &mut self,
        buffers: &mut BridgeRenderBuffers,
        mesh: &BridgeMesh,
        diffuse: u32,
    ) {
        let mut vec = self.end - self.start;
        if vec.length2() < 1.0 {
            vec = vec.normalize();
        }
        let mut vec_normal = Vec3::new(-vec.y, vec.x, 0.0).normalize();
        let desired_length = vec.length().max(1.0);
        let delta_z = (self.end.z - self.start.z) / desired_length;
        let delta_x = (1.0 - delta_z * delta_z).max(0.0).sqrt();
        let mut vec_z = Vec3::new(-delta_z, 0.0, delta_x);
        vec = vec / self.length.max(1.0);
        vec_normal = vec_normal * self.scale;
        vec_z = vec_z * self.scale;
        self.append_mesh(
            buffers,
            mesh,
            -self.left_min_x,
            vec,
            vec_normal,
            vec_z,
            diffuse,
        );
    }

    fn append_mesh(
        &mut self,
        buffers: &mut BridgeRenderBuffers,
        mesh: &BridgeMesh,
        x_offset: f32,
        vec: Vec3,
        vec_normal: Vec3,
        vec_z: Vec3,
        diffuse: u32,
    ) -> bool {
        if buffers.vertices.len() + mesh.vertices.len() + 2 >= MAX_BRIDGE_VERTEX {
            return false;
        }
        if buffers.indices.len() + mesh.triangles.len() * 3 + 6 >= MAX_BRIDGE_INDEX {
            return false;
        }

        let vertex_offset = buffers.vertices.len() as u16;
        for source in &mesh.vertices {
            let vertex = mesh.transform.transform_vector(source.position);
            let v_loc =
                self.start + vec * (vertex.x + x_offset) + vec_normal * vertex.y + vec_z * vertex.z;
            let normal = mesh.transform.rotate_vector(source.normal);
            let normal = (vec * normal.x + vec_normal * normal.y + vec_z * normal.z).normalize();
            buffers.vertices.push(BridgeVertex {
                x: v_loc.x,
                y: v_loc.y,
                z: v_loc.z,
                diffuse: diffuse | 0xff00_0000,
                nx: normal.x,
                ny: normal.y,
                nz: normal.z,
                u1: source.u,
                v1: source.v,
            });
        }

        for tri in &mesh.triangles {
            buffers.indices.extend_from_slice(&[
                vertex_offset + tri[0],
                vertex_offset + tri[1],
                vertex_offset + tri[2],
            ]);
        }
        self.num_vertex += mesh.vertices.len();
        self.num_polygons += mesh.triangles.len();
        true
    }
}

impl Default for W3DBridge {
    fn default() -> Self {
        Self::new()
    }
}

/// W3D bridge draw buffer.
#[derive(Debug, Clone)]
pub struct W3DBridgeBuffer {
    bridges: Vec<W3DBridge>,
    cur_num_bridge_vertices: usize,
    cur_num_bridge_indices: usize,
    initialized: bool,
    update_vis: bool,
    anything_changed: bool,
    cached_buffers: BridgeRenderBuffers,
}

impl W3DBridgeBuffer {
    /// Construct with C++ initial state.
    pub fn new() -> Self {
        let mut buffer = Self {
            bridges: Vec::new(),
            cur_num_bridge_vertices: 0,
            cur_num_bridge_indices: 0,
            initialized: false,
            update_vis: false,
            anything_changed: false,
            cached_buffers: BridgeRenderBuffers::default(),
        };
        buffer.clear_all_bridges();
        buffer.allocate_bridge_buffers();
        buffer.initialized = true;
        buffer
    }

    /// C++ `clearAllBridges`.
    pub fn clear_all_bridges(&mut self) {
        for bridge in &mut self.bridges {
            bridge.clear_bridge();
        }
        self.cur_num_bridge_indices = 0;
        self.cur_num_bridge_vertices = 0;
        self.bridges.clear();
        self.cached_buffers = BridgeRenderBuffers::default();
        self.anything_changed = true;
    }

    /// C++ `doFullUpdate`.
    pub fn do_full_update(&mut self) {
        self.update_vis = true;
    }

    /// Add a loaded bridge and return the info that should be registered with terrain logic.
    pub fn add_bridge<P: BridgeAssetProvider>(
        &mut self,
        from_loc: Vec3,
        to_loc: Vec3,
        name: impl Into<String>,
        provider: &P,
    ) -> Option<BridgeInfo> {
        if self.bridges.len() >= MAX_BRIDGES || !self.initialized {
            return None;
        }
        let mut bridge = W3DBridge::new();
        bridge.init(from_loc, to_loc, name);
        if !bridge.load(provider, BridgeDamageState::Pristine) {
            return None;
        }

        let mut info = bridge.get_bridge_info();
        info.bridge_index = self.bridges.len() as i32;
        self.bridges.push(bridge);
        self.anything_changed = true;
        Some(info)
    }

    /// Update culling and rebuild buffers if needed.
    pub fn update_center(&mut self, diffuse: u32) -> &BridgeRenderBuffers {
        self.cull();
        if self.anything_changed || self.cur_num_bridge_indices == 0 {
            self.load_bridges_in_vertex_and_index_buffers(diffuse);
        }
        self.update_vis = false;
        &self.cached_buffers
    }

    /// Apply terrain-logic damage states and reload changed bridges.
    pub fn sync_damage_states<P: BridgeAssetProvider>(
        &mut self,
        provider: &P,
        states: &[(usize, BridgeDamageState)],
        diffuse: u32,
    ) -> &BridgeRenderBuffers {
        for bridge in &mut self.bridges {
            bridge.set_enabled(false);
        }

        let mut changed = false;
        for &(index, state) in states {
            let Some(bridge) = self.bridges.get_mut(index) else {
                continue;
            };
            bridge.set_enabled(true);
            if bridge.damage_state() != state {
                changed = true;
                let old_state = bridge.damage_state();
                if !bridge.load(provider, state) {
                    let _ = bridge.load(provider, old_state);
                }
            }
        }

        if changed {
            self.load_bridges_in_vertex_and_index_buffers(diffuse);
        }
        &self.cached_buffers
    }

    /// Access cached render buffers.
    pub fn render_buffers(&self) -> &BridgeRenderBuffers {
        &self.cached_buffers
    }

    /// Access loaded bridges.
    pub fn bridges(&self) -> &[W3DBridge] {
        &self.bridges
    }

    /// Mutable loaded bridges.
    pub fn bridges_mut(&mut self) -> &mut [W3DBridge] {
        &mut self.bridges
    }

    /// Number of loaded bridges.
    pub fn num_bridges(&self) -> usize {
        self.bridges.len()
    }

    fn allocate_bridge_buffers(&mut self) {
        self.cur_num_bridge_vertices = 0;
        self.cur_num_bridge_indices = 0;
        self.cached_buffers = BridgeRenderBuffers {
            vertices: Vec::with_capacity(MAX_BRIDGE_VERTEX + 4),
            indices: Vec::with_capacity(MAX_BRIDGE_INDEX + 4),
        };
    }

    fn cull(&mut self) {
        self.anything_changed = self.update_vis;
        for bridge in &mut self.bridges {
            if bridge.cull_bridge() {
                self.anything_changed = true;
            }
        }
    }

    fn load_bridges_in_vertex_and_index_buffers(&mut self, diffuse: u32) {
        if !self.initialized {
            return;
        }
        self.cur_num_bridge_vertices = 0;
        self.cur_num_bridge_indices = 0;
        self.cached_buffers.vertices.clear();
        self.cached_buffers.indices.clear();

        for bridge in &mut self.bridges {
            bridge.append_indices_and_vertices(&mut self.cached_buffers, diffuse);
        }
        self.cur_num_bridge_vertices = self.cached_buffers.vertices.len();
        self.cur_num_bridge_indices = self.cached_buffers.indices.len();
        self.anything_changed = false;
    }
}

impl Default for W3DBridgeBuffer {
    fn default() -> Self {
        Self::new()
    }
}

fn section_bounds(mesh: &BridgeMesh, include_y: bool) -> Option<(f32, f32, f32, f32)> {
    let mut iter = mesh.vertices.iter();
    let first = iter.next()?;
    let first = mesh.transform.transform_vector(first.position);
    let mut min_x = first.x;
    let mut max_x = first.x;
    let mut min_y = first.y;
    let mut max_y = first.y;

    for vertex in iter {
        let vertex = mesh.transform.transform_vector(vertex.position);
        min_x = min_x.min(vertex.x);
        max_x = max_x.max(vertex.x);
        if include_y {
            min_y = min_y.min(vertex.y);
            max_y = max_y.max(vertex.y);
        }
    }

    Some((min_x, max_x, min_y, max_y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Provider {
        models: HashMap<(String, BridgeDamageState), BridgeModel>,
    }

    impl Provider {
        fn insert(&mut self, template: &str, damage: BridgeDamageState, model: BridgeModel) {
            self.models.insert((template.to_string(), damage), model);
        }
    }

    impl BridgeAssetProvider for Provider {
        fn load_bridge_model(
            &self,
            template_name: &str,
            damage: BridgeDamageState,
        ) -> Option<BridgeModel> {
            self.models
                .get(&(template_name.to_string(), damage))
                .cloned()
        }
    }

    fn quad(min_x: f32, max_x: f32, min_y: f32, max_y: f32) -> BridgeMesh {
        BridgeMesh {
            vertices: vec![
                BridgeMeshVertex {
                    position: Vec3::new(min_x, min_y, 0.0),
                    normal: Vec3::new(0.0, 0.0, 1.0),
                    u: 0.0,
                    v: 1.0,
                },
                BridgeMeshVertex {
                    position: Vec3::new(max_x, min_y, 0.0),
                    normal: Vec3::new(0.0, 0.0, 1.0),
                    u: 1.0,
                    v: 1.0,
                },
                BridgeMeshVertex {
                    position: Vec3::new(max_x, max_y, 0.0),
                    normal: Vec3::new(0.0, 0.0, 1.0),
                    u: 1.0,
                    v: 0.0,
                },
                BridgeMeshVertex {
                    position: Vec3::new(min_x, max_y, 0.0),
                    normal: Vec3::new(0.0, 0.0, 1.0),
                    u: 0.0,
                    v: 0.0,
                },
            ],
            triangles: vec![[0, 1, 2], [0, 2, 3]],
            transform: Matrix3D::identity(),
        }
    }

    fn sectional_model(scale: f32) -> BridgeModel {
        BridgeModel {
            texture_name: "bridge.tga".into(),
            model_name: "BRIDGESECTIONAL".into(),
            scale,
            left: quad(0.0, 20.0, -5.0, 5.0),
            section: Some(quad(20.0, 60.0, -5.0, 5.0)),
            right: Some(quad(60.0, 80.0, -5.0, 5.0)),
        }
    }

    #[test]
    fn add_bridge_registers_cpp_bridge_info_corners() {
        let mut provider = Provider::default();
        provider.insert("BridgeA", BridgeDamageState::Pristine, sectional_model(2.0));
        let mut buffer = W3DBridgeBuffer::new();

        let info = buffer
            .add_bridge(
                Vec3::new(10.0, 20.0, BRIDGE_FLOAT_AMT),
                Vec3::new(110.0, 20.0, BRIDGE_FLOAT_AMT),
                "BridgeA",
                &provider,
            )
            .expect("bridge should load");

        assert_eq!(buffer.num_bridges(), 1);
        assert_eq!(info.bridge_index, 0);
        assert_eq!(info.bridge_width, 20.0);
        assert_eq!(info.from_left, Vec3::new(10.0, 30.0, BRIDGE_FLOAT_AMT));
        assert_eq!(info.from_right, Vec3::new(10.0, 10.0, BRIDGE_FLOAT_AMT));
        assert_eq!(info.to_left, Vec3::new(110.0, 30.0, BRIDGE_FLOAT_AMT));
        assert_eq!(info.to_right, Vec3::new(110.0, 10.0, BRIDGE_FLOAT_AMT));
    }

    #[test]
    fn update_center_rebuilds_sectional_bridge_vertices_and_indices() {
        let mut provider = Provider::default();
        provider.insert("BridgeA", BridgeDamageState::Pristine, sectional_model(1.0));
        let mut buffer = W3DBridgeBuffer::new();
        buffer.add_bridge(
            Vec3::new(0.0, 0.0, 0.25),
            Vec3::new(120.0, 0.0, 0.25),
            "BridgeA",
            &provider,
        );

        let rendered = buffer.update_center(0x0012_3456).clone();

        assert_eq!(rendered.vertices.len(), 16);
        assert_eq!(rendered.indices.len(), 24);
        assert_eq!(&rendered.indices[..6], &[0, 1, 2, 0, 2, 3]);
        assert_eq!(rendered.vertices[0].x, 0.0);
        assert_eq!(rendered.vertices[0].y, -5.0);
        assert_eq!(rendered.vertices[0].z, 0.25);
        assert_eq!(rendered.vertices[0].diffuse, 0xff12_3456);
    }

    #[test]
    fn missing_section_or_right_mesh_falls_back_to_fixed_bridge() {
        let mut provider = Provider::default();
        provider.insert(
            "BridgeA",
            BridgeDamageState::Pristine,
            BridgeModel {
                section: None,
                right: None,
                ..sectional_model(1.0)
            },
        );
        let mut buffer = W3DBridgeBuffer::new();
        buffer.add_bridge(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(80.0, 0.0, 0.0),
            "BridgeA",
            &provider,
        );

        assert_eq!(buffer.bridges()[0].bridge_type(), BridgeType::Fixed);
        assert_eq!(buffer.update_center(0).vertices.len(), 4);
    }

    #[test]
    fn sync_damage_states_reloads_changed_bridge_and_preserves_old_on_failure() {
        let mut provider = Provider::default();
        provider.insert("BridgeA", BridgeDamageState::Pristine, sectional_model(1.0));
        provider.insert(
            "BridgeA",
            BridgeDamageState::Damaged,
            BridgeModel {
                texture_name: "damaged.tga".into(),
                model_name: "DAMAGED".into(),
                ..sectional_model(1.0)
            },
        );
        let mut buffer = W3DBridgeBuffer::new();
        buffer.add_bridge(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(80.0, 0.0, 0.0),
            "BridgeA",
            &provider,
        );

        buffer.sync_damage_states(&provider, &[(0, BridgeDamageState::Damaged)], 0);
        assert_eq!(
            buffer.bridges()[0].damage_state(),
            BridgeDamageState::Damaged
        );

        buffer.sync_damage_states(&provider, &[(0, BridgeDamageState::Rubble)], 0);
        assert_eq!(
            buffer.bridges()[0].damage_state(),
            BridgeDamageState::Damaged
        );
    }
}
