use crate::assets::W3DMaterial;
use crate::fow_rendering::ObjectVisibility;
use crate::game_logic::ObjectId;
use glam::{Mat4, Vec3};

use super::render_pipeline::RenderPass;

/// Render item abstraction - equivalent to C++ SAGE RenderItem
#[derive(Debug, Clone)]
pub struct RenderItem {
    /// Object ID for debugging and tracking
    pub object_id: ObjectId,

    /// Debug name for render item
    pub debug_name: String,

    /// Source model name (used for WW3D renderer integration)
    pub model_name: String,

    /// Mesh index inside the source model
    pub mesh_index: usize,

    /// Material definition for this mesh
    pub material: W3DMaterial,

    /// World transform matrix
    pub world_matrix: Mat4,

    /// Mesh-local transform matrix
    pub mesh_local_transform: Mat4,

    /// World position (for sorting)
    pub world_position: Vec3,

    /// Distance from camera (for sorting)
    pub distance: f32,

    /// Material key for batching
    pub material_key: String,

    /// Render pass this item belongs to
    pub render_pass: RenderPass,

    /// Mesh resource key
    pub mesh_key: String,

    /// Vertex buffer range
    pub vertex_buffer_range: Option<(u32, u32)>, // (start, count)

    /// Index buffer range
    pub index_buffer_range: Option<(u32, u32)>, // (start, count)

    /// Sorting key for efficient rendering - equivalent to C++ RenderItem::SortingKey
    pub sorting_key: u64,

    /// FOW visibility data for this render item
    pub fow_visibility: ObjectVisibility,
}

impl RenderItem {
    /// Create new render item - equivalent to C++ RenderItem constructor
    pub fn new(
        object_id: ObjectId,
        model_name: String,
        mesh_index: usize,
        world_position: Vec3,
        world_matrix: Mat4,
        material: &W3DMaterial,
        render_pass: RenderPass,
    ) -> Self {
        let mesh_key = format!("{}_{}", model_name, mesh_index);
        let distance = world_position.length();
        let texture_tag = material
            .texture_name
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let material_key = format!("{}::{}", material.name, texture_tag);
        let sorting_key = Self::generate_sorting_key(render_pass, &material_key, distance);

        Self {
            object_id,
            debug_name: format!("{}_{}", object_id.0, mesh_key),
            model_name,
            mesh_index,
            material: material.clone(),
            world_matrix,
            mesh_local_transform: Mat4::IDENTITY,
            world_position,
            distance,
            material_key,
            render_pass,
            mesh_key,
            vertex_buffer_range: None,
            index_buffer_range: None,
            sorting_key,
            fow_visibility: ObjectVisibility::default(), // Default to fully visible
        }
    }

    /// Generate sorting key for render ordering - equivalent to C++ RenderItem::GenerateSortingKey()
    fn generate_sorting_key(render_pass: RenderPass, material_key: &str, distance: f32) -> u64 {
        // Sorting key format (64-bit):
        // Bits 56-63: Render pass (8 bits)
        // Bits 32-55: Material hash (24 bits)
        // Bits 0-31:  Distance (32 bits, inverted for front-to-back)

        let pass_bits = (render_pass as u64) << 56;

        // Simple hash of material key
        let mut material_hash = 0u64;
        for byte in material_key.bytes() {
            material_hash = material_hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        let material_bits = (material_hash & 0xFFFFFF) << 32;

        // Distance bits (inverted for front-to-back sorting)
        let distance_u32 = (distance * 1000.0) as u32;
        let distance_bits = (!distance_u32) as u64;

        pass_bits | material_bits | distance_bits
    }

    /// Update world matrix - equivalent to C++ RenderItem::SetWorldMatrix()
    pub fn set_world_matrix(&mut self, matrix: Mat4) {
        self.world_matrix = matrix;
        // Extract position from matrix
        self.world_position = Vec3::new(matrix.w_axis.x, matrix.w_axis.y, matrix.w_axis.z);
        self.distance = self.world_position.length();

        // Regenerate sorting key
        self.sorting_key =
            Self::generate_sorting_key(self.render_pass, &self.material_key, self.distance);
    }

    pub fn set_mesh_local_transform(&mut self, matrix: Mat4) {
        self.mesh_local_transform = matrix;
    }

    /// Set vertex buffer range - equivalent to C++ RenderItem::SetVertexRange()
    pub fn set_vertex_range(&mut self, start: u32, count: u32) {
        self.vertex_buffer_range = Some((start, count));
    }

    /// Set index buffer range - equivalent to C++ RenderItem::SetIndexRange()
    pub fn set_index_range(&mut self, start: u32, count: u32) {
        self.index_buffer_range = Some((start, count));
    }

    /// Get render pass
    pub fn get_render_pass(&self) -> RenderPass {
        self.render_pass
    }

    /// Get material key
    pub fn get_material_key(&self) -> &str {
        &self.material_key
    }

    /// Get mesh key
    pub fn get_mesh_key(&self) -> &str {
        &self.mesh_key
    }

    /// Set FOW visibility for this render item
    pub fn set_fow_visibility(&mut self, visibility: ObjectVisibility) {
        self.fow_visibility = visibility;
    }

    /// Get FOW visibility for this render item
    pub fn get_fow_visibility(&self) -> ObjectVisibility {
        self.fow_visibility
    }
}

/// Implement ordering for render items - equivalent to C++ RenderItem::operator<
impl PartialOrd for RenderItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RenderItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sorting_key.cmp(&other.sorting_key)
    }
}

impl PartialEq for RenderItem {
    fn eq(&self, other: &Self) -> bool {
        self.sorting_key == other.sorting_key
    }
}

impl Eq for RenderItem {}
