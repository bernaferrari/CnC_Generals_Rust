//! Hierarchical Level of Detail (HLOD) System
//!
//! This module implements HLOD (Hierarchical LOD) which provides advanced
//! level-of-detail management with hierarchical transitions and mesh switching.

use crate::render_object_system::SphereClass;
use crate::rendering::lod_system::{LODLevel, LODManager};
use crate::rendering::mesh_system::{MeshClass, MeshModelClass};
use glam::{Mat4, Vec3};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use ww3d_core::w3d_format::{W3dTexCoordStruct, W3dTriangleStruct, W3dVectorStruct};

/// HLOD node representing a hierarchical LOD structure
#[derive(Debug)]
pub struct HLODNode {
    pub name: String,
    pub bounding_sphere: SphereClass,
    pub lod_levels: Vec<HLODLevel>,
    pub children: Vec<HLODNode>,
    pub parent: Option<usize>, // parent node index
    pub current_lod: LODLevel,
    pub transform: Mat4,
    pub visible: bool,
}

impl HLODNode {
    /// Create a new HLOD node
    pub fn new(name: &str, center: Vec3, radius: f32) -> Self {
        Self {
            name: name.to_string(),
            bounding_sphere: SphereClass::new(center, radius),
            lod_levels: Vec::new(),
            children: Vec::new(),
            parent: None,
            current_lod: LODLevel::Highest,
            transform: Mat4::IDENTITY,
            visible: true,
        }
    }

    /// Add an LOD level to this node
    pub fn add_lod_level(&mut self, lod_level: HLODLevel) {
        self.lod_levels.push(lod_level);
        // Sort by distance threshold (assuming levels are added in order)
        self.lod_levels.sort_by(|a, b| {
            a.distance_threshold
                .partial_cmp(&b.distance_threshold)
                .unwrap()
        });
    }

    /// Add a child node
    pub fn add_child(&mut self, mut child: HLODNode) {
        child.parent = Some(self.children.len());
        self.children.push(child);
    }

    /// Update the current LOD based on distance to camera
    pub fn update_lod(&mut self, camera_pos: Vec3, lod_manager: &LODManager) {
        let distance = (self.bounding_sphere.center - camera_pos).length();

        // Determine appropriate LOD level
        let mut best_lod = LODLevel::Lowest;
        for lod_level in &self.lod_levels {
            if distance <= lod_level.distance_threshold {
                best_lod = lod_level.lod_level;
                break;
            }
        }

        // Update transition if needed
        self.current_lod = best_lod;

        // Update children
        for child in &mut self.children {
            child.update_lod(camera_pos, lod_manager);
        }
    }

    /// Get the current mesh for rendering
    pub fn get_current_mesh(&self) -> Option<&MeshClass> {
        // Find the LOD level that matches current_lod
        for lod_level in &self.lod_levels {
            if lod_level.lod_level == self.current_lod {
                return Some(&lod_level.mesh);
            }
        }

        // Fallback to highest detail if no exact match
        self.lod_levels.first().map(|level| &level.mesh)
    }

    /// Get all visible meshes for rendering (including children)
    pub fn get_visible_meshes(&self) -> Vec<&MeshClass> {
        let mut meshes = Vec::new();

        if self.visible {
            if let Some(mesh) = self.get_current_mesh() {
                meshes.push(mesh);
            }
        }

        // Add children's meshes
        for child in &self.children {
            meshes.extend(child.get_visible_meshes());
        }

        meshes
    }

    /// Set visibility
    pub fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    /// Set transform
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    /// Get world-space bounding sphere
    pub fn get_world_bounding_sphere(&self) -> SphereClass {
        let center = self.transform.transform_point3(self.bounding_sphere.center);
        let radius = self.bounding_sphere.radius
            * self
                .transform
                .to_scale_rotation_translation()
                .0
                .max_element();
        SphereClass::new(center, radius)
    }
}

/// HLOD level containing mesh data for a specific LOD
#[derive(Debug)]
pub struct HLODLevel {
    pub lod_level: LODLevel,
    pub distance_threshold: f32,
    pub mesh: MeshClass,
    pub screen_size_threshold: f32,
}

impl HLODLevel {
    /// Create a new HLOD level
    pub fn new(lod_level: LODLevel, distance_threshold: f32, mesh: MeshClass) -> Self {
        Self {
            lod_level,
            distance_threshold,
            mesh,
            screen_size_threshold: 0.0, // Will be calculated based on mesh size
        }
    }

    /// Set screen size threshold for this LOD level
    pub fn with_screen_size_threshold(mut self, threshold: f32) -> Self {
        self.screen_size_threshold = threshold;
        self
    }
}

/// HLOD manager for handling hierarchical LOD systems
pub struct HLODManager {
    pub lod_manager: LODManager,
    pub root_nodes: Vec<HLODNode>,
    pub node_map: HashMap<String, usize>, // name to index mapping
}

impl HLODManager {
    /// Create a new HLOD manager
    pub fn new() -> Self {
        Self {
            lod_manager: LODManager::new(),
            root_nodes: Vec::new(),
            node_map: HashMap::new(),
        }
    }

    /// Add a root HLOD node
    pub fn add_root_node(&mut self, node: HLODNode) {
        let index = self.root_nodes.len();
        self.node_map.insert(node.name.clone(), index);
        self.root_nodes.push(node);
    }

    /// Get a node by name
    pub fn get_node(&self, name: &str) -> Option<&HLODNode> {
        self.node_map
            .get(name)
            .and_then(|&index| self.root_nodes.get(index))
    }

    /// Get a mutable node by name
    pub fn get_node_mut(&mut self, name: &str) -> Option<&mut HLODNode> {
        if let Some(&index) = self.node_map.get(name) {
            self.root_nodes.get_mut(index)
        } else {
            None
        }
    }

    /// Update all HLOD nodes based on camera position
    pub fn update_lods(&mut self, camera_pos: Vec3) {
        for node in &mut self.root_nodes {
            node.update_lod(camera_pos, &self.lod_manager);
        }
    }

    /// Get all visible meshes for rendering
    pub fn get_visible_meshes(&self) -> Vec<&MeshClass> {
        let mut meshes = Vec::new();
        for node in &self.root_nodes {
            meshes.extend(node.get_visible_meshes());
        }
        meshes
    }

    /// Create HLOD node from mesh with automatic LOD generation
    pub fn create_hlod_from_mesh(
        &mut self,
        name: &str,
        mesh: MeshClass,
        num_lod_levels: usize,
    ) -> HLODNode {
        let mut hlod_node = HLODNode::new(name, Vec3::ZERO, 10.0); // Default bounding sphere

        // Calculate bounding sphere from mesh
        if let Some(model) = &mesh.model {
            let sphere = self.calculate_mesh_bounding_sphere(model);
            hlod_node.bounding_sphere = sphere;
        }

        // Create LOD levels
        let base_distance = 25.0;
        for i in 0..num_lod_levels {
            let distance_threshold = base_distance * (i + 1) as f32;
            let lod_level = match i {
                0 => LODLevel::Highest,
                1 => LODLevel::High,
                2 => LODLevel::Medium,
                3 => LODLevel::Low,
                _ => LODLevel::Lowest,
            };

            // For simplicity, use the same mesh for all LOD levels
            // In practice, you'd generate simplified meshes
            let lod_mesh = mesh.clone();

            let hlod_level = HLODLevel::new(lod_level, distance_threshold, lod_mesh);
            hlod_node.add_lod_level(hlod_level);
        }

        hlod_node
    }

    /// Calculate bounding sphere for a mesh model
    fn calculate_mesh_bounding_sphere(&self, model: &MeshModelClass) -> SphereClass {
        if model.vertices.is_empty() {
            return SphereClass::new(Vec3::ZERO, 1.0);
        }

        // Calculate center as average of vertices
        let mut center = Vec3::ZERO;
        for vertex in &model.vertices {
            center += Vec3::new(vertex.x, vertex.y, vertex.z);
        }
        center /= model.vertices.len() as f32;

        // Calculate radius as maximum distance from center
        let mut max_distance: f32 = 0.0;
        for vertex in &model.vertices {
            let vertex_pos = Vec3::new(vertex.x, vertex.y, vertex.z);
            let distance = (vertex_pos - center).length();
            max_distance = max_distance.max(distance);
        }

        SphereClass::new(center, max_distance)
    }

    fn default_screen_size_threshold(lod_level: LODLevel) -> f32 {
        match lod_level {
            LODLevel::Highest => 1000.0,
            LODLevel::High => 500.0,
            LODLevel::Medium => 200.0,
            LODLevel::Low => 75.0,
            LODLevel::Lowest => 25.0,
        }
    }

    fn optimize_node_structure(node: &mut HLODNode) {
        let mut levels = std::mem::take(&mut node.lod_levels);
        levels.sort_by(|a, b| {
            a.distance_threshold
                .partial_cmp(&b.distance_threshold)
                .unwrap_or(Ordering::Equal)
        });

        let mut seen_lods = [false; 5];
        let mut normalized_levels = Vec::with_capacity(levels.len());
        for mut level in levels {
            let lod_index = level.lod_level as usize;
            if lod_index >= seen_lods.len() || seen_lods[lod_index] {
                continue;
            }
            seen_lods[lod_index] = true;
            if level.screen_size_threshold <= 0.0 {
                level.screen_size_threshold = Self::default_screen_size_threshold(level.lod_level);
            }
            normalized_levels.push(level);
        }

        let mut previous_distance = 0.0f32;
        for level in &mut normalized_levels {
            if level.distance_threshold <= previous_distance {
                level.distance_threshold = previous_distance + 0.01;
            }
            previous_distance = level.distance_threshold;
        }
        node.lod_levels = normalized_levels;

        for (child_index, child) in node.children.iter_mut().enumerate() {
            child.parent = Some(child_index);
            Self::optimize_node_structure(child);
        }
    }

    fn build_billboard_impostor_mesh(
        node_name: &str,
        bounds: SphereClass,
        source_mesh: &MeshClass,
    ) -> MeshClass {
        let mut impostor = source_mesh.clone();
        impostor.name = format!("{}_impostor", node_name);

        let mut model = source_mesh
            .model
            .as_ref()
            .map(|existing| (**existing).clone())
            .unwrap_or_else(|| MeshModelClass::new(&impostor.name));

        let radius = bounds.radius.max(0.01);
        let center = bounds.center;
        let normal = W3dVectorStruct {
            x: 0.0,
            y: 0.0,
            z: 1.0,
        };

        model.name = impostor.name.clone();
        model.vertices = vec![
            W3dVectorStruct {
                x: center.x - radius,
                y: center.y - radius,
                z: center.z,
            },
            W3dVectorStruct {
                x: center.x + radius,
                y: center.y - radius,
                z: center.z,
            },
            W3dVectorStruct {
                x: center.x + radius,
                y: center.y + radius,
                z: center.z,
            },
            W3dVectorStruct {
                x: center.x - radius,
                y: center.y + radius,
                z: center.z,
            },
        ];
        model.normals = vec![normal; 4];
        model.triangles = vec![
            W3dTriangleStruct {
                vindex: [0, 1, 2],
                attributes: 0,
                normal,
                distance: 0.0,
            },
            W3dTriangleStruct {
                vindex: [0, 2, 3],
                attributes: 0,
                normal,
                distance: 0.0,
            },
        ];
        model.texture_coords = vec![
            W3dTexCoordStruct { u: 0.0, v: 1.0 },
            W3dTexCoordStruct { u: 1.0, v: 1.0 },
            W3dTexCoordStruct { u: 1.0, v: 0.0 },
            W3dTexCoordStruct { u: 0.0, v: 0.0 },
        ];
        model.stage_texture_coords = vec![model.texture_coords.clone()];
        model.per_stage_face_texcoord_ids = vec![vec![[0, 1, 2], [0, 2, 3]]];
        model.vertex_count = model.vertices.len() as u32;
        model.index_count = (model.triangles.len() * 3) as u32;

        impostor.model = Some(Arc::new(model));
        impostor.update_cached_bounding_volumes();
        impostor
    }

    fn generate_node_impostors(node: &mut HLODNode) {
        if let Some(last_lod) = node.lod_levels.last_mut() {
            let has_complex_geometry = last_lod
                .mesh
                .model
                .as_ref()
                .map(|model| model.vertices.len() > 4 || model.triangles.len() > 2)
                .unwrap_or(true);

            if has_complex_geometry {
                let source_mesh = last_lod.mesh.clone();
                last_lod.mesh = Self::build_billboard_impostor_mesh(
                    &node.name,
                    node.bounding_sphere,
                    &source_mesh,
                );
                if last_lod.screen_size_threshold <= 0.0 {
                    last_lod.screen_size_threshold =
                        Self::default_screen_size_threshold(last_lod.lod_level);
                }
            }
        }

        for child in &mut node.children {
            Self::generate_node_impostors(child);
        }
    }

    /// Optimize HLOD tree structure and LOD thresholds.
    pub fn optimize_tree(&mut self) {
        for node in &mut self.root_nodes {
            Self::optimize_node_structure(node);
        }

        // Render larger nodes first to improve front-loaded culling.
        self.root_nodes.sort_by(|a, b| {
            b.bounding_sphere
                .radius
                .partial_cmp(&a.bounding_sphere.radius)
                .unwrap_or(Ordering::Equal)
        });

        self.node_map.clear();
        for (index, node) in self.root_nodes.iter().enumerate() {
            self.node_map.insert(node.name.clone(), index);
        }
    }

    /// Generate billboard impostor meshes for distant LOD levels.
    pub fn generate_impostors(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        let _ = (device, queue);
        for node in &mut self.root_nodes {
            Self::generate_node_impostors(node);
        }
    }
}

/// HLOD batch renderer for efficient rendering of multiple HLOD nodes
pub struct HLODBatchRenderer {
    pub hlod_manager: HLODManager,
    pub visible_nodes: Vec<usize>, // indices of visible root nodes
    pub render_stats: HLODRenderStats,
}

impl HLODBatchRenderer {
    /// Create a new HLOD batch renderer
    pub fn new(hlod_manager: HLODManager) -> Self {
        Self {
            hlod_manager,
            visible_nodes: Vec::new(),
            render_stats: HLODRenderStats::default(),
        }
    }

    /// Update visibility based on frustum culling
    pub fn update_visibility(
        &mut self,
        _camera_pos: Vec3,
        frustum: &crate::rendering::frustum::FrustumClass,
    ) {
        self.visible_nodes.clear();
        self.render_stats.reset();

        for (i, node) in self.hlod_manager.root_nodes.iter().enumerate() {
            let sphere = node.get_world_bounding_sphere();

            if frustum.intersects_sphere(sphere.center, sphere.radius) {
                self.visible_nodes.push(i);
                self.render_stats.total_visible_nodes += 1;
            }
        }
    }

    /// Update LOD levels for visible nodes
    pub fn update_lods(&mut self, camera_pos: Vec3) {
        for &node_index in &self.visible_nodes {
            if let Some(node) = self.hlod_manager.root_nodes.get_mut(node_index) {
                node.update_lod(camera_pos, &self.hlod_manager.lod_manager);
            }
        }
    }

    /// Render all visible HLOD nodes
    pub fn render(&mut self, _render_pass: &mut wgpu::RenderPass, _device: &wgpu::Device) {
        for &node_index in &self.visible_nodes {
            if let Some(node) = self.hlod_manager.root_nodes.get(node_index) {
                if let Some(mesh) = node.get_current_mesh() {
                    self.render_stats.total_rendered_meshes += 1;
                    self.render_stats.total_triangles_rendered += mesh.get_num_polys() as usize;
                }

                // Render children
                for child in &node.children {
                    if let Some(mesh) = child.get_current_mesh() {
                        self.render_stats.total_rendered_meshes += 1;
                        self.render_stats.total_triangles_rendered += mesh.get_num_polys() as usize;
                    }
                }
            }
        }
    }

    /// Get rendering statistics
    pub fn get_stats(&self) -> &HLODRenderStats {
        &self.render_stats
    }
}

/// Rendering statistics for HLOD system
#[derive(Debug, Default, Clone)]
pub struct HLODRenderStats {
    pub total_visible_nodes: usize,
    pub total_rendered_meshes: usize,
    pub total_triangles_rendered: usize,
    pub lod_transitions_this_frame: usize,
}

impl HLODRenderStats {
    /// Reset statistics
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// HLOD transition manager for smooth LOD changes
pub struct HLODTransitionManager {
    pub active_transitions: HashMap<String, HLODTransition>,
    pub transition_speed: f32,
}

impl HLODTransitionManager {
    /// Create a new transition manager
    pub fn new() -> Self {
        Self {
            active_transitions: HashMap::new(),
            transition_speed: 2.0, // transitions per second
        }
    }

    /// Start a transition for a node
    pub fn start_transition(&mut self, node_name: &str, from_lod: LODLevel, to_lod: LODLevel) {
        let transition = HLODTransition::new(node_name.to_string(), from_lod, to_lod);
        self.active_transitions
            .insert(node_name.to_string(), transition);
    }

    /// Update all active transitions
    pub fn update_transitions(&mut self, delta_time: f32) {
        let mut completed = Vec::new();

        for (name, transition) in &mut self.active_transitions {
            transition.update(delta_time * self.transition_speed);
            if transition.is_complete() {
                completed.push(name.clone());
            }
        }

        // Remove completed transitions
        for name in completed {
            self.active_transitions.remove(&name);
        }
    }

    /// Get current transition state for a node
    pub fn get_transition_state(&self, node_name: &str) -> Option<&HLODTransition> {
        self.active_transitions.get(node_name)
    }
}

/// Individual LOD transition
#[derive(Debug, Clone)]
pub struct HLODTransition {
    pub node_name: String,
    pub from_lod: LODLevel,
    pub to_lod: LODLevel,
    pub progress: f32, // 0.0 to 1.0
    pub duration: f32,
    pub elapsed: f32,
}

impl HLODTransition {
    /// Create a new transition
    pub fn new(node_name: String, from_lod: LODLevel, to_lod: LODLevel) -> Self {
        Self {
            node_name,
            from_lod,
            to_lod,
            progress: 0.0,
            duration: 1.0, // 1 second transition
            elapsed: 0.0,
        }
    }

    /// Update transition progress
    pub fn update(&mut self, delta_time: f32) {
        self.elapsed += delta_time;
        self.progress = (self.elapsed / self.duration).min(1.0);
    }

    /// Check if transition is complete
    pub fn is_complete(&self) -> bool {
        self.progress >= 1.0
    }

    /// Get interpolated LOD value
    pub fn get_interpolated_lod(&self) -> f32 {
        let from_value = self.from_lod as usize as f32;
        let to_value = self.to_lod as usize as f32;
        from_value + (to_value - from_value) * self.progress
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_mesh(name: &str) -> MeshClass {
        let mut mesh = MeshClass::new();
        mesh.name = name.to_string();

        let mut model = MeshModelClass::new(name);
        model.vertices = vec![
            W3dVectorStruct {
                x: -1.0,
                y: -1.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 1.0,
                y: -1.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 1.0,
                y: 1.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: -1.0,
                y: 1.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
        ];
        model.normals = vec![
            W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            };
            model.vertices.len()
        ];
        model.texture_coords = vec![
            W3dTexCoordStruct { u: 0.0, v: 1.0 },
            W3dTexCoordStruct { u: 1.0, v: 1.0 },
            W3dTexCoordStruct { u: 1.0, v: 0.0 },
            W3dTexCoordStruct { u: 0.0, v: 0.0 },
            W3dTexCoordStruct { u: 0.5, v: 0.5 },
        ];
        model.stage_texture_coords = vec![model.texture_coords.clone()];
        model.triangles = vec![
            W3dTriangleStruct {
                vindex: [0, 1, 4],
                attributes: 0,
                normal: W3dVectorStruct {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                distance: 0.0,
            },
            W3dTriangleStruct {
                vindex: [1, 2, 4],
                attributes: 0,
                normal: W3dVectorStruct {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                distance: 0.0,
            },
            W3dTriangleStruct {
                vindex: [2, 3, 4],
                attributes: 0,
                normal: W3dVectorStruct {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                distance: 0.0,
            },
            W3dTriangleStruct {
                vindex: [3, 0, 4],
                attributes: 0,
                normal: W3dVectorStruct {
                    x: 0.0,
                    y: 0.0,
                    z: 1.0,
                },
                distance: 0.0,
            },
        ];
        model.vertex_count = model.vertices.len() as u32;
        model.index_count = (model.triangles.len() * 3) as u32;
        mesh.model = Some(Arc::new(model));
        mesh.update_cached_bounding_volumes();
        mesh
    }

    #[test]
    fn optimize_tree_deduplicates_lods_and_populates_thresholds() {
        let mesh = make_test_mesh("optimize_mesh");
        let mut node = HLODNode::new("node_a", Vec3::ZERO, 5.0);
        node.add_lod_level(HLODLevel::new(LODLevel::Medium, 50.0, mesh.clone()));
        node.add_lod_level(HLODLevel::new(LODLevel::Highest, 10.0, mesh.clone()));
        node.add_lod_level(HLODLevel::new(LODLevel::Medium, 25.0, mesh.clone()));

        let mut manager = HLODManager::new();
        manager.add_root_node(node);
        manager.optimize_tree();

        let optimized = manager.get_node("node_a").expect("node must exist");
        assert_eq!(optimized.lod_levels.len(), 2);
        assert_eq!(optimized.lod_levels[0].lod_level, LODLevel::Highest);
        assert_eq!(optimized.lod_levels[1].lod_level, LODLevel::Medium);
        assert!(
            optimized.lod_levels[0].distance_threshold < optimized.lod_levels[1].distance_threshold
        );
        assert!(optimized
            .lod_levels
            .iter()
            .all(|level| level.screen_size_threshold > 0.0));
    }

    #[test]
    fn generate_node_impostors_replaces_farthest_lod_with_billboard_mesh() {
        let mesh = make_test_mesh("impostor_source");
        let mut node = HLODNode::new("node_impostor", Vec3::new(2.0, 3.0, 4.0), 6.0);
        node.add_lod_level(HLODLevel::new(LODLevel::High, 20.0, mesh.clone()));
        node.add_lod_level(HLODLevel::new(LODLevel::Lowest, 80.0, mesh));

        HLODManager::generate_node_impostors(&mut node);

        let far_lod = node.lod_levels.last().expect("farthest lod should exist");
        let model = far_lod
            .mesh
            .model
            .as_ref()
            .expect("impostor model should exist");
        assert_eq!(model.vertices.len(), 4);
        assert_eq!(model.triangles.len(), 2);
        assert_eq!(model.index_count, 6);
        assert!(far_lod.mesh.name.ends_with("_impostor"));
    }
}
