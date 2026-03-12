//! Sorting Renderer Implementation
//!
//! This module implements the SortingRendererClass for rendering transparent
//! geometry in correct depth order, essential for particle systems.

use glam::{Mat4, Vec3, Vec4};
use std::sync::Arc;
use wgpu::{Device, Queue};

/// Triangle indices structure for sorting
#[derive(Debug, Clone, Copy)]
pub struct TriangleIndices {
    pub i: u16,
    pub j: u16,
    pub k: u16,
}

/// Temporary index structure for depth sorting
#[derive(Debug, Clone)]
pub struct TempIndexStruct {
    pub tri: TriangleIndices,
    pub node_id: usize,
    pub z_depth: f32,
}

impl TempIndexStruct {
    pub fn new(tri: TriangleIndices, node_id: usize, z_depth: f32) -> Self {
        Self {
            tri,
            node_id,
            z_depth,
        }
    }
}

/// Sorting node representing a batch of geometry to be sorted
#[derive(Debug, Clone)]
pub struct SortingNode {
    pub bounding_sphere: BoundingSphere,
    pub transformed_center: Vec3,
    pub start_index: u16,
    pub polygon_count: u16,
    pub min_vertex_index: u16,
    pub vertex_count: u16,
    pub layer_count: u16,
    pub is_volume_particle: bool,
}

impl SortingNode {
    pub fn new(
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
    ) -> Self {
        Self {
            bounding_sphere,
            transformed_center: Vec3::ZERO,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
            layer_count: 1,
            is_volume_particle: false,
        }
    }

    pub fn volume_particle(
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        layer_count: u16,
    ) -> Self {
        Self {
            bounding_sphere,
            transformed_center: Vec3::ZERO,
            start_index,
            polygon_count: polygon_count * layer_count,
            min_vertex_index,
            vertex_count: vertex_count * layer_count,
            layer_count,
            is_volume_particle: true,
        }
    }

    /// Update the transformed center for sorting
    pub fn update_transformed_center(&mut self, world_view_matrix: &Mat4) {
        let center_4d = Vec4::new(
            self.bounding_sphere.center.x,
            self.bounding_sphere.center.y,
            self.bounding_sphere.center.z,
            1.0,
        );
        let transformed = *world_view_matrix * center_4d;
        self.transformed_center = Vec3::new(transformed.x, transformed.y, transformed.z);
    }
}

/// Bounding sphere for frustum culling and sorting
#[derive(Debug, Clone, Copy)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl BoundingSphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }
}

/// Sorting renderer for transparent geometry
#[allow(dead_code)]
#[derive(Debug)]
pub struct SortingRenderer {
    // Configuration
    pub enable_triangle_draw: bool,
    pub min_vertex_buffer_size: u32,

    // Internal state
    sorted_nodes: Vec<SortingNode>,
    temp_indices: Vec<TempIndexStruct>,
    overlapping_nodes: Vec<SortingNode>,
    overlapping_indices: Vec<TempIndexStruct>,

    // GPU resources
    device: Arc<Device>,
    queue: Arc<Queue>,
}

impl SortingRenderer {
    /// Create a new sorting renderer
    pub fn new(device: Arc<Device>, queue: Arc<Queue>) -> Self {
        Self {
            enable_triangle_draw: true,
            min_vertex_buffer_size: 32768,
            sorted_nodes: Vec::new(),
            temp_indices: Vec::new(),
            overlapping_nodes: Vec::new(),
            overlapping_indices: Vec::new(),
            device,
            queue,
        }
    }

    /// Set minimum vertex buffer size
    pub fn set_min_vertex_buffer_size(&mut self, size: u32) {
        self.min_vertex_buffer_size = size;
    }

    /// Enable or disable triangle drawing
    pub fn enable_triangle_draw(&mut self, enable: bool) {
        self.enable_triangle_draw = enable;
    }

    /// Check if triangle drawing is enabled
    pub fn is_triangle_draw_enabled(&self) -> bool {
        self.enable_triangle_draw
    }

    /// Insert triangles into the sorting system
    pub fn insert_triangles(
        &mut self,
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        world_view_matrix: &Mat4,
    ) {
        let mut node = SortingNode::new(
            bounding_sphere,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
        );

        node.update_transformed_center(world_view_matrix);

        // Insert into sorted list based on Z depth
        self.insert_sorted_node(node);
    }

    /// Insert volume particle triangles
    pub fn insert_volume_particle(
        &mut self,
        bounding_sphere: BoundingSphere,
        start_index: u16,
        polygon_count: u16,
        min_vertex_index: u16,
        vertex_count: u16,
        layer_count: u16,
        world_view_matrix: &Mat4,
    ) {
        let mut node = SortingNode::volume_particle(
            bounding_sphere,
            start_index,
            polygon_count,
            min_vertex_index,
            vertex_count,
            layer_count,
        );

        node.update_transformed_center(world_view_matrix);

        // Insert into sorted list based on Z depth
        self.insert_sorted_node(node);
    }

    /// Insert a node into the sorted list
    fn insert_sorted_node(&mut self, node: SortingNode) {
        // Find the insertion point based on Z depth (front to back)
        let insert_pos = self
            .sorted_nodes
            .iter()
            .position(|existing| node.transformed_center.z > existing.transformed_center.z)
            .unwrap_or(self.sorted_nodes.len());

        self.sorted_nodes.insert(insert_pos, node);
    }

    /// Flush all sorted geometry
    pub fn flush(&mut self) {
        if self.sorted_nodes.is_empty() {
            return;
        }

        // Process nodes and collect overlapping ones
        self.process_sorted_nodes();

        // Render overlapping nodes in sorted order
        self.render_overlapping_nodes();

        // Clear for next frame
        self.sorted_nodes.clear();
        self.overlapping_nodes.clear();
        self.overlapping_indices.clear();
    }

    /// Process sorted nodes and collect overlapping geometry
    fn process_sorted_nodes(&mut self) {
        for node in &self.sorted_nodes {
            // For now, treat all nodes as potentially overlapping
            // In a full implementation, you'd check for actual overlaps
            self.overlapping_nodes.push(node.clone());
        }

        // Generate sorted triangle indices
        self.generate_sorted_indices();
    }

    /// Generate sorted triangle indices for overlapping nodes
    fn generate_sorted_indices(&mut self) {
        self.temp_indices.clear();

        let mut vertex_offset = 0u16;
        let mut polygon_offset = 0u16;

        for (node_id, node) in self.overlapping_nodes.iter().enumerate() {
            // For each triangle in this node, calculate its center Z and add to temp indices
            for i in 0..node.polygon_count {
                // In a real implementation, you'd access the actual vertex data
                // For now, we'll use a simplified approach
                let triangle_z = self.calculate_triangle_z(node, i as usize, vertex_offset);
                let tri_indices = TriangleIndices {
                    i: vertex_offset + (i * 3),
                    j: vertex_offset + (i * 3) + 1,
                    k: vertex_offset + (i * 3) + 2,
                };

                self.temp_indices
                    .push(TempIndexStruct::new(tri_indices, node_id, triangle_z));
            }

            vertex_offset = vertex_offset.saturating_add(node.vertex_count);
            polygon_offset = polygon_offset.saturating_add(node.polygon_count);
        }

        // Sort triangles by depth (back to front for transparency)
        self.temp_indices.sort_by(|a, b| {
            b.z_depth
                .partial_cmp(&a.z_depth)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Calculate the Z depth of a triangle (simplified)
    fn calculate_triangle_z(
        &self,
        node: &SortingNode,
        _triangle_index: usize,
        _vertex_offset: u16,
    ) -> f32 {
        // In a real implementation, you'd access the actual vertex positions
        // For now, use the node's transformed center
        node.transformed_center.z
    }

    /// Render overlapping nodes in sorted order
    fn render_overlapping_nodes(&mut self) {
        if self.temp_indices.is_empty() {
            return;
        }

        // Group triangles by node for efficient rendering
        let mut current_node_id = self.temp_indices[0].node_id;
        let mut start_triangle = 0;

        for (i, temp_index) in self.temp_indices.iter().enumerate() {
            if temp_index.node_id != current_node_id || i == self.temp_indices.len() - 1 {
                // Render the batch of triangles from the current node
                let triangle_count = if i == self.temp_indices.len() - 1 {
                    i - start_triangle + 1
                } else {
                    i - start_triangle
                };

                self.render_node_triangles(current_node_id, start_triangle, triangle_count);

                current_node_id = temp_index.node_id;
                start_triangle = i;
            }
        }
    }

    /// Render triangles from a specific node
    fn render_node_triangles(&self, node_id: usize, _start_triangle: usize, triangle_count: usize) {
        if node_id >= self.overlapping_nodes.len() || !self.enable_triangle_draw {
            return;
        }

        let node = &self.overlapping_nodes[node_id];

        // In a real implementation, you'd set up the render state and draw the triangles
        // For now, this is a placeholder for the actual rendering logic

        println!(
            "Rendering {} triangles from node {} (start: {}, min_vertex: {}, vertex_count: {})",
            triangle_count, node_id, node.start_index, node.min_vertex_index, node.vertex_count
        );
    }

    /// Deinitialize the sorting renderer
    pub fn deinit(&mut self) {
        self.sorted_nodes.clear();
        self.temp_indices.clear();
        self.overlapping_nodes.clear();
        self.overlapping_indices.clear();
    }

    /// Get the number of active sorted nodes
    pub fn get_sorted_node_count(&self) -> usize {
        self.sorted_nodes.len()
    }

    /// Get the total number of polygons being sorted
    pub fn get_total_polygon_count(&self) -> usize {
        self.sorted_nodes
            .iter()
            .map(|node| node.polygon_count as usize)
            .sum()
    }

    /// Get the total number of vertices being sorted
    pub fn get_total_vertex_count(&self) -> usize {
        self.sorted_nodes
            .iter()
            .map(|node| node.vertex_count as usize)
            .sum()
    }

    /// Check if sorting is enabled (always true for now)
    pub fn is_sorting_enabled() -> bool {
        true
    }
}

/// Simple insertion sort for small arrays (used in original C++ code)
pub fn insertion_sort(indices: &mut [TempIndexStruct]) {
    for i in 1..indices.len() {
        let mut j = i;
        while j > 0 && indices[j - 1].z_depth < indices[j].z_depth {
            indices.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// Quick sort with median-of-three partitioning (used in original C++ code)
pub fn quick_sort(indices: &mut [TempIndexStruct]) {
    if indices.len() <= 16 {
        insertion_sort(indices);
        return;
    }

    // Median-of-three partitioning
    let mid = indices.len() / 2;
    let last = indices.len() - 1;

    // Sort the first, middle, and last elements
    if indices[0].z_depth < indices[mid].z_depth {
        indices.swap(0, mid);
    }
    if indices[mid].z_depth < indices[last].z_depth {
        indices.swap(mid, last);
    }
    if indices[0].z_depth < indices[mid].z_depth {
        indices.swap(0, mid);
    }

    let pivot = indices[mid].z_depth;
    let mut i = 0;
    let mut j = last;

    loop {
        while indices[i].z_depth > pivot {
            i += 1;
        }
        while indices[j].z_depth < pivot {
            j -= 1;
        }
        if i >= j {
            break;
        }
        indices.swap(i, j);
        i += 1;
        j -= 1;
    }

    // Recursively sort partitions
    if j > 0 {
        quick_sort(&mut indices[0..j + 1]);
    }
    if j + 1 < indices.len() {
        quick_sort(&mut indices[j + 1..]);
    }
}
