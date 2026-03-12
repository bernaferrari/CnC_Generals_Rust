//! Mesh Damage System
//!
//! Implements progressive mesh deformation for damage visualization on units and buildings.
//! This system allows vertices to morph from their original positions to damaged positions,
//! and vertex colors to transition to damaged colors, providing visual feedback for unit health.
//!
//! The C++ equivalent is in `meshdam.cpp/h` (though it was disabled with `#if 0`).

use crate::{MeshGeometry, Vector3};
use std::collections::HashMap;

/// RGB color structure for damage color morphing
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGBColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RGBColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn white() -> Self {
        Self::new(255, 255, 255)
    }

    pub fn black() -> Self {
        Self::new(0, 0, 0)
    }

    /// Linear interpolation between two colors
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        Self {
            r: (self.r as f32 + (other.r as f32 - self.r as f32) * t) as u8,
            g: (self.g as f32 + (other.g as f32 - self.g as f32) * t) as u8,
            b: (self.b as f32 + (other.b as f32 - self.b as f32) * t) as u8,
        }
    }

    /// Convert to normalized Vec3 (0-1 range)
    pub fn to_vec3(&self) -> Vector3 {
        Vector3::new(
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        )
    }

    /// Create from normalized Vec3
    pub fn from_vec3(v: Vector3) -> Self {
        Self {
            r: (v.x.clamp(0.0, 1.0) * 255.0) as u8,
            g: (v.y.clamp(0.0, 1.0) * 255.0) as u8,
            b: (v.z.clamp(0.0, 1.0) * 255.0) as u8,
        }
    }
}

/// Vertex position morph information
#[derive(Debug, Clone)]
pub struct DamageVertex {
    /// Index of the vertex to damage
    pub vertex_index: usize,
    /// Original vertex position
    pub original_position: Vector3,
    /// Damaged (morphed) vertex position
    pub damaged_position: Vector3,
}

impl DamageVertex {
    pub fn new(vertex_index: usize, original_position: Vector3, damaged_position: Vector3) -> Self {
        Self {
            vertex_index,
            original_position,
            damaged_position,
        }
    }

    /// Interpolate between original and damaged position
    pub fn interpolate(&self, damage_ratio: f32) -> Vector3 {
        let t = damage_ratio.clamp(0.0, 1.0);
        self.original_position.lerp(self.damaged_position, t)
    }
}

/// Vertex color morph information
#[derive(Debug, Clone)]
pub struct DamageColor {
    /// Index of the vertex to recolor
    pub vertex_index: usize,
    /// Original vertex color
    pub original_color: RGBColor,
    /// Damaged vertex color
    pub damaged_color: RGBColor,
}

impl DamageColor {
    pub fn new(vertex_index: usize, original_color: RGBColor, damaged_color: RGBColor) -> Self {
        Self {
            vertex_index,
            original_color,
            damaged_color,
        }
    }

    /// Interpolate between original and damaged color
    pub fn interpolate(&self, damage_ratio: f32) -> RGBColor {
        self.original_color.lerp(&self.damaged_color, damage_ratio)
    }
}

/// Mesh damage state container
///
/// This class encapsulates the information needed to apply damage to meshes.
/// It contains replacement vertices, vertex colors, and material indices.
#[derive(Debug, Clone)]
pub struct MeshDamage {
    /// Damage stage identifier (0 = undamaged, higher = more damaged)
    pub damage_index: u32,
    /// Number of material passes affected by damage
    pub num_materials: usize,
    /// Vertex position morphs
    pub vertex_morphs: Vec<DamageVertex>,
    /// Vertex color morphs
    pub color_morphs: Vec<DamageColor>,
}

impl MeshDamage {
    /// Create a new empty damage state
    pub fn new() -> Self {
        Self {
            damage_index: 0,
            num_materials: 0,
            vertex_morphs: Vec::new(),
            color_morphs: Vec::new(),
        }
    }

    /// Create with specific capacity
    pub fn with_capacity(damage_index: u32, vertex_capacity: usize, color_capacity: usize) -> Self {
        Self {
            damage_index,
            num_materials: 0,
            vertex_morphs: Vec::with_capacity(vertex_capacity),
            color_morphs: Vec::with_capacity(color_capacity),
        }
    }

    /// Add a vertex position morph
    pub fn add_vertex_morph(
        &mut self,
        vertex_index: usize,
        original_position: Vector3,
        damaged_position: Vector3,
    ) {
        self.vertex_morphs.push(DamageVertex::new(
            vertex_index,
            original_position,
            damaged_position,
        ));
    }

    /// Add a vertex color morph
    pub fn add_color_morph(
        &mut self,
        vertex_index: usize,
        original_color: RGBColor,
        damaged_color: RGBColor,
    ) {
        self.color_morphs.push(DamageColor::new(
            vertex_index,
            original_color,
            damaged_color,
        ));
    }

    /// Apply damage to a mesh geometry
    ///
    /// # Arguments
    /// * `mesh` - The mesh to modify
    /// * `damage_ratio` - 0.0 = no damage, 1.0 = full damage
    ///
    /// # Example
    /// ```
    /// # use ww3d_geometry::*;
    /// let mut damage = MeshDamage::new();
    /// damage.add_vertex_morph(0, Vec3::ZERO, Vec3::new(0.5, 0.0, 0.0));
    ///
    /// let mut mesh = MeshGeometry::new();
    /// // ... populate mesh ...
    ///
    /// // Apply 50% damage
    /// damage.apply_damage(&mut mesh, 0.5);
    /// ```
    pub fn apply_damage(&self, mesh: &mut MeshGeometry, damage_ratio: f32) {
        let ratio = damage_ratio.clamp(0.0, 1.0);

        // Apply vertex position morphs
        for vertex_morph in &self.vertex_morphs {
            if vertex_morph.vertex_index < mesh.vertices.len() {
                let new_position = vertex_morph.interpolate(ratio);
                mesh.vertices[vertex_morph.vertex_index].position = new_position;
            }
        }

        // Mark mesh bounds as dirty after modifying vertices
        if !self.vertex_morphs.is_empty() {
            mesh.update_bounds();
        }

        // Note: Color morphing would require vertex colors in MeshVertex
        // For now, we store the color morphs but don't apply them
        // A future enhancement would add a color field to MeshVertex
    }

    /// Get the number of vertex morphs
    pub fn vertex_morph_count(&self) -> usize {
        self.vertex_morphs.len()
    }

    /// Get the number of color morphs
    pub fn color_morph_count(&self) -> usize {
        self.color_morphs.len()
    }

    /// Clear all damage data
    pub fn clear(&mut self) {
        self.vertex_morphs.clear();
        self.color_morphs.clear();
        self.num_materials = 0;
    }

    /// Create damage data from mesh analysis
    ///
    /// This is a utility function that can automatically generate damage morphs
    /// by randomly displacing vertices based on a damage pattern.
    pub fn generate_random_damage(
        mesh: &MeshGeometry,
        damage_index: u32,
        displacement_amount: f32,
        affected_vertex_ratio: f32,
    ) -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let mut damage = Self::with_capacity(
            damage_index,
            (mesh.vertices.len() as f32 * affected_vertex_ratio) as usize,
            0,
        );

        // Select random vertices to damage
        for (i, vertex) in mesh.vertices.iter().enumerate() {
            if rng.gen::<f32>() < affected_vertex_ratio {
                // Random displacement
                let displacement = Vector3::new(
                    rng.gen_range(-displacement_amount..displacement_amount),
                    rng.gen_range(-displacement_amount..displacement_amount),
                    rng.gen_range(-displacement_amount..displacement_amount),
                );

                damage.add_vertex_morph(i, vertex.position, vertex.position + displacement);
            }
        }

        damage
    }
}

impl Default for MeshDamage {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection of damage states for multiple damage levels
#[derive(Debug, Clone)]
pub struct DamageStageCollection {
    /// Map from damage index to damage state
    stages: HashMap<u32, MeshDamage>,
}

impl DamageStageCollection {
    pub fn new() -> Self {
        Self {
            stages: HashMap::new(),
        }
    }

    /// Add a damage stage
    pub fn add_stage(&mut self, damage: MeshDamage) {
        self.stages.insert(damage.damage_index, damage);
    }

    /// Get a damage stage by index
    pub fn get_stage(&self, damage_index: u32) -> Option<&MeshDamage> {
        self.stages.get(&damage_index)
    }

    /// Get a mutable damage stage by index
    pub fn get_stage_mut(&mut self, damage_index: u32) -> Option<&mut MeshDamage> {
        self.stages.get_mut(&damage_index)
    }

    /// Apply damage from the appropriate stage based on health percentage
    ///
    /// # Arguments
    /// * `mesh` - The mesh to modify
    /// * `health_percent` - 0.0 = destroyed, 1.0 = pristine
    pub fn apply_damage_for_health(&self, mesh: &mut MeshGeometry, health_percent: f32) {
        let health = health_percent.clamp(0.0, 1.0);

        // Determine which damage stage to use
        // For example: 100-75% = stage 0, 75-50% = stage 1, 50-25% = stage 2, etc.
        let damage_stage = if health > 0.75 {
            0
        } else if health > 0.5 {
            1
        } else if health > 0.25 {
            2
        } else {
            3
        };

        if let Some(damage) = self.get_stage(damage_stage) {
            // Within each stage, interpolate damage based on health
            let stage_health_range = 0.25;
            let stage_base = (3 - damage_stage) as f32 * stage_health_range;
            let damage_ratio = if stage_health_range > 0.0 {
                1.0 - ((health - stage_base) / stage_health_range)
            } else {
                1.0
            };

            damage.apply_damage(mesh, damage_ratio.clamp(0.0, 1.0));
        }
    }

    /// Get the number of damage stages
    pub fn stage_count(&self) -> usize {
        self.stages.len()
    }

    /// Clear all damage stages
    pub fn clear(&mut self) {
        self.stages.clear();
    }
}

impl Default for DamageStageCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MeshBuilder;

    #[test]
    fn test_rgb_color_lerp() {
        let white = RGBColor::white();
        let black = RGBColor::black();

        let mid = white.lerp(&black, 0.5);
        assert_eq!(mid.r, 127);
        assert_eq!(mid.g, 127);
        assert_eq!(mid.b, 127);

        let full_white = white.lerp(&black, 0.0);
        assert_eq!(full_white, white);

        let full_black = white.lerp(&black, 1.0);
        assert_eq!(full_black, black);
    }

    #[test]
    fn test_damage_vertex_interpolation() {
        let damage_vert =
            DamageVertex::new(0, Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 1.0, 1.0));

        let mid = damage_vert.interpolate(0.5);
        assert!((mid.x - 0.5).abs() < 1e-5);
        assert!((mid.y - 0.5).abs() < 1e-5);
        assert!((mid.z - 0.5).abs() < 1e-5);
    }

    #[test]
    fn test_mesh_damage_creation() {
        let mut damage = MeshDamage::new();
        assert_eq!(damage.damage_index, 0);
        assert_eq!(damage.vertex_morph_count(), 0);
        assert_eq!(damage.color_morph_count(), 0);

        damage.add_vertex_morph(0, Vector3::ZERO, Vector3::X);
        assert_eq!(damage.vertex_morph_count(), 1);

        damage.add_color_morph(0, RGBColor::white(), RGBColor::black());
        assert_eq!(damage.color_morph_count(), 1);
    }

    #[test]
    fn test_apply_damage_to_mesh() {
        let mut mesh = MeshBuilder::create_cube(1.0);

        // Store original positions
        let original_positions: Vec<Vector3> = mesh.vertices.iter().map(|v| v.position).collect();

        // Create damage that moves first vertex
        let mut damage = MeshDamage::new();
        damage.add_vertex_morph(
            0,
            original_positions[0],
            original_positions[0] + Vector3::new(1.0, 0.0, 0.0),
        );

        // Apply 50% damage
        damage.apply_damage(&mut mesh, 0.5);

        // Check that first vertex moved halfway
        let expected = original_positions[0] + Vector3::new(0.5, 0.0, 0.0);
        let actual = mesh.vertices[0].position;
        assert!((actual - expected).length() < 1e-5);

        // Apply full damage
        damage.apply_damage(&mut mesh, 1.0);
        let expected_full = original_positions[0] + Vector3::new(1.0, 0.0, 0.0);
        let actual_full = mesh.vertices[0].position;
        assert!((actual_full - expected_full).length() < 1e-5);
    }

    #[test]
    fn test_damage_stage_collection() {
        let mut collection = DamageStageCollection::new();

        let damage1 = MeshDamage::with_capacity(1, 10, 5);
        let damage2 = MeshDamage::with_capacity(2, 10, 5);

        collection.add_stage(damage1);
        collection.add_stage(damage2);

        assert_eq!(collection.stage_count(), 2);
        assert!(collection.get_stage(1).is_some());
        assert!(collection.get_stage(2).is_some());
        assert!(collection.get_stage(3).is_none());
    }

    #[test]
    fn test_damage_for_health() {
        let mut mesh = MeshBuilder::create_sphere(1.0, 8, 8);
        let mut collection = DamageStageCollection::new();

        // Create multiple damage stages
        for stage in 0..4 {
            let mut damage = MeshDamage::with_capacity(stage, 10, 0);
            // Add some sample damage (first 3 vertices)
            for i in 0..3.min(mesh.vertices.len()) {
                let displacement = (stage + 1) as f32 * 0.1;
                damage.add_vertex_morph(
                    i,
                    mesh.vertices[i].position,
                    mesh.vertices[i].position + Vector3::new(displacement, 0.0, 0.0),
                );
            }
            collection.add_stage(damage);
        }

        // Test different health levels
        collection.apply_damage_for_health(&mut mesh, 1.0); // Full health - minimal damage
        collection.apply_damage_for_health(&mut mesh, 0.5); // Half health - moderate damage
        collection.apply_damage_for_health(&mut mesh, 0.0); // Destroyed - full damage

        // Just verify it doesn't crash - actual damage values depend on staging
    }

    #[test]
    fn test_color_conversion() {
        let color = RGBColor::new(128, 64, 192);
        let vec = color.to_vec3();

        assert!((vec.x - 128.0 / 255.0).abs() < 1e-5);
        assert!((vec.y - 64.0 / 255.0).abs() < 1e-5);
        assert!((vec.z - 192.0 / 255.0).abs() < 1e-5);

        let roundtrip = RGBColor::from_vec3(vec);
        assert_eq!(roundtrip, color);
    }
}
