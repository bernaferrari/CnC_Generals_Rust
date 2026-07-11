//! Mesh Optimization Algorithms
//!
//! This module provides various mesh optimization techniques including
//! vertex cache optimization, mesh simplification, and performance improvements.

use crate::mesh_geometry::{MeshStats, MeshTriangle};
use crate::*;
use std::collections::{HashMap, HashSet};

/// Mesh optimization utilities
pub struct MeshOptimizer;

impl MeshOptimizer {
    /// Optimize mesh for vertex cache efficiency
    pub fn optimize_vertex_cache(mesh: &mut MeshGeometry) {
        let triangles = mesh.triangles.clone();
        let mut vertex_scores = vec![0.0; mesh.vertices.len()];

        // Calculate vertex scores based on triangle adjacency
        for triangle in &triangles {
            for &vertex_index in &triangle.indices {
                vertex_scores[vertex_index as usize] += 1.0;
            }
        }

        // Sort triangles by average vertex score (highest first)
        let mut triangle_indices: Vec<usize> = (0..triangles.len()).collect();
        triangle_indices.sort_by(|&a, &b| {
            let score_a = triangles[a]
                .indices
                .iter()
                .map(|&idx| vertex_scores[idx as usize])
                .sum::<f32>()
                / 3.0;
            let score_b = triangles[b]
                .indices
                .iter()
                .map(|&idx| vertex_scores[idx as usize])
                .sum::<f32>()
                / 3.0;
            score_b.partial_cmp(&score_a).unwrap()
        });

        // Reorder triangles
        let mut new_triangles = Vec::new();
        for &index in &triangle_indices {
            new_triangles.push(triangles[index]);
        }

        mesh.triangles = new_triangles;
    }

    /// Perform mesh simplification using quadratic error metrics
    pub fn simplify_mesh(mesh: &MeshGeometry, target_triangle_count: usize) -> MeshGeometry {
        if mesh.triangle_count() <= target_triangle_count {
            return mesh.clone();
        }

        let mut simplified_mesh = mesh.clone();
        let mut quadric_errors = Self::compute_quadric_errors(&simplified_mesh);

        while simplified_mesh.triangle_count() > target_triangle_count {
            if let Some((edge, _cost)) =
                Self::find_best_edge_collapse(&simplified_mesh, &quadric_errors)
            {
                Self::collapse_edge(&mut simplified_mesh, edge, &mut quadric_errors);
            } else {
                break;
            }
        }

        simplified_mesh.update_bounds();
        simplified_mesh.compute_normals();
        simplified_mesh.compute_plane_equations();
        simplified_mesh
    }

    /// Compute quadratic error metrics for vertices
    fn compute_quadric_errors(mesh: &MeshGeometry) -> Vec<QuadricError> {
        let mut quadrics = vec![Quadric::new(); mesh.vertices.len()];

        // Accumulate quadrics from triangles
        for triangle in &mesh.triangles {
            let v0 = mesh.vertices[triangle.indices[0] as usize];
            let v1 = mesh.vertices[triangle.indices[1] as usize];
            let v2 = mesh.vertices[triangle.indices[2] as usize];

            let plane = Plane::from_points(v0.position, v1.position, v2.position);
            let quadric = Quadric::from_plane(&plane);

            for &vertex_index in &triangle.indices {
                quadrics[vertex_index as usize].add_quadric(&quadric);
            }
        }

        quadrics.into_iter().map(|q| q.error()).collect()
    }

    /// Find the best edge to collapse
    fn find_best_edge_collapse(
        mesh: &MeshGeometry,
        errors: &[QuadricError],
    ) -> Option<(Edge, f32)> {
        let mut best_edge: Option<(Edge, f32)> = None;

        for triangle in &mesh.triangles {
            for i in 0..3 {
                let v0 = triangle.indices[i] as usize;
                let v1 = triangle.indices[(i + 1) % 3] as usize;
                let edge = Edge::new(v0.min(v1), v0.max(v1));

                let cost = errors[v0] + errors[v1];
                let total_cost = cost.error();

                if best_edge.is_none() || total_cost < best_edge.as_ref().unwrap().1 {
                    best_edge = Some((edge, total_cost));
                }
            }
        }

        best_edge
    }

    /// Collapse an edge
    fn collapse_edge(mesh: &mut MeshGeometry, edge: Edge, errors: &mut Vec<QuadricError>) {
        let target_vertex = edge.v0;
        let source_vertex = edge.v1;

        // Move source vertex to target position
        mesh.vertices[source_vertex] = mesh.vertices[target_vertex];

        // Update triangles to use target vertex instead of source
        mesh.triangles.retain_mut(|triangle| {
            let mut has_source = false;
            let mut has_target = false;

            for &vertex_index in &triangle.indices {
                if vertex_index as usize == source_vertex {
                    has_source = true;
                }
                if vertex_index as usize == target_vertex {
                    has_target = true;
                }
            }

            // Remove degenerate triangles
            if has_source && has_target {
                false // Triangle becomes degenerate
            } else if has_source {
                // Replace source with target
                for vertex_index in &mut triangle.indices {
                    if *vertex_index as usize == source_vertex {
                        *vertex_index = target_vertex as u32;
                    }
                }
                true
            } else {
                true
            }
        });

        // Update error for target vertex
        errors[target_vertex] = errors[target_vertex] + errors[source_vertex];
    }

    /// Remove duplicate vertices within a threshold
    pub fn remove_duplicate_vertices(mesh: &mut MeshGeometry, threshold: f32) {
        let mut vertex_map = HashMap::new();
        let mut new_vertices = Vec::new();
        let mut index_map = vec![0; mesh.vertices.len()];

        for (i, vertex) in mesh.vertices.iter().enumerate() {
            let position_key = (
                (vertex.position.x / threshold).round() as i32,
                (vertex.position.y / threshold).round() as i32,
                (vertex.position.z / threshold).round() as i32,
            );

            if let Some(&existing_index) = vertex_map.get(&position_key) {
                index_map[i] = existing_index;
            } else {
                let new_index = new_vertices.len();
                new_vertices.push(*vertex);
                vertex_map.insert(position_key, new_index);
                index_map[i] = new_index;
            }
        }

        // Update triangle indices
        for triangle in &mut mesh.triangles {
            triangle.indices[0] = index_map[triangle.indices[0] as usize] as u32;
            triangle.indices[1] = index_map[triangle.indices[1] as usize] as u32;
            triangle.indices[2] = index_map[triangle.indices[2] as usize] as u32;
        }

        mesh.vertices = new_vertices;
    }

    /// Optimize mesh for GPU rendering (stripify)
    pub fn create_triangle_strips(mesh: &MeshGeometry) -> Vec<Vec<u32>> {
        let mut strips = Vec::new();
        let mut used_triangles = HashSet::new();

        for start_triangle in 0..mesh.triangles.len() {
            if used_triangles.contains(&start_triangle) {
                continue;
            }

            let mut strip = Vec::new();
            let mut current_triangle = start_triangle;

            // Start with first triangle
            let triangle = &mesh.triangles[current_triangle];
            strip.extend_from_slice(&triangle.indices);
            used_triangles.insert(current_triangle);

            // Try to extend the strip
            loop {
                let mut found_next = false;

                for next_triangle in 0..mesh.triangles.len() {
                    if used_triangles.contains(&next_triangle) {
                        continue;
                    }

                    if let Some(shared_edge) = Self::find_shared_edge(
                        &mesh.triangles[current_triangle],
                        &mesh.triangles[next_triangle],
                    ) {
                        // Add the new vertex to continue the strip
                        if let Some(new_vertex) =
                            Self::find_unshared_vertex(&mesh.triangles[next_triangle], shared_edge)
                        {
                            strip.push(new_vertex);
                            used_triangles.insert(next_triangle);
                            current_triangle = next_triangle;
                            found_next = true;
                            break;
                        }
                    }
                }

                if !found_next {
                    break;
                }
            }

            if strip.len() >= 3 {
                strips.push(strip);
            }
        }

        strips
    }

    /// Find shared edge between two triangles
    fn find_shared_edge(triangle1: &MeshTriangle, triangle2: &MeshTriangle) -> Option<Edge> {
        let mut shared_vertices = Vec::new();

        for &v1 in &triangle1.indices {
            for &v2 in &triangle2.indices {
                if v1 == v2 {
                    shared_vertices.push(v1 as usize);
                }
            }
        }

        if shared_vertices.len() == 2 {
            Some(Edge::new(
                shared_vertices[0].min(shared_vertices[1]),
                shared_vertices[0].max(shared_vertices[1]),
            ))
        } else {
            None
        }
    }

    /// Find the unshared vertex in a triangle given a shared edge
    /// Returns None if the triangle doesn't have exactly one unshared vertex (invalid mesh)
    fn find_unshared_vertex(triangle: &MeshTriangle, shared_edge: Edge) -> Option<u32> {
        triangle
            .indices
            .iter()
            .find(|&&vertex| vertex as usize != shared_edge.v0 && vertex as usize != shared_edge.v1)
            .copied()
    }

    /// Generate level-of-detail (LOD) meshes
    pub fn generate_lod_meshes(mesh: &MeshGeometry, levels: usize) -> Vec<MeshGeometry> {
        let mut lod_meshes = Vec::new();
        let total_triangles = mesh.triangle_count();

        for i in 0..levels {
            let target_triangles = total_triangles * (levels - i) / levels;
            let lod_mesh = Self::simplify_mesh(mesh, target_triangles);
            lod_meshes.push(lod_mesh);
        }

        lod_meshes
    }

    /// Optimize mesh for memory layout (improve cache locality)
    pub fn optimize_memory_layout(mesh: &mut MeshGeometry) {
        // Reorder vertices based on usage in triangles
        let mut vertex_usage = vec![0; mesh.vertices.len()];

        for triangle in &mesh.triangles {
            for &vertex_index in &triangle.indices {
                vertex_usage[vertex_index as usize] += 1;
            }
        }

        // Sort vertices by usage (most used first)
        let mut vertex_indices: Vec<usize> = (0..mesh.vertices.len()).collect();
        vertex_indices.sort_by(|&a, &b| vertex_usage[b].cmp(&vertex_usage[a]));

        // Create new vertex array
        let mut new_vertices = Vec::new();
        let mut old_to_new_index = vec![0; mesh.vertices.len()];

        for &old_index in &vertex_indices {
            old_to_new_index[old_index] = new_vertices.len();
            new_vertices.push(mesh.vertices[old_index]);
        }

        // Update triangle indices
        for triangle in &mut mesh.triangles {
            triangle.indices[0] = old_to_new_index[triangle.indices[0] as usize] as u32;
            triangle.indices[1] = old_to_new_index[triangle.indices[1] as usize] as u32;
            triangle.indices[2] = old_to_new_index[triangle.indices[2] as usize] as u32;
        }

        mesh.vertices = new_vertices;
    }

    /// Compute mesh statistics
    pub fn compute_mesh_stats(mesh: &MeshGeometry) -> MeshStats {
        let vertex_count = mesh.vertex_count();
        let triangle_count = mesh.triangle_count();

        // Calculate bounding box volume
        let bb_volume = mesh.bounding_box.extent.x
            * mesh.bounding_box.extent.y
            * mesh.bounding_box.extent.z
            * 8.0;

        // Calculate surface area
        let surface_area = mesh
            .triangles
            .iter()
            .map(|triangle| {
                let v0 = mesh.vertices[triangle.indices[0] as usize].position;
                let v1 = mesh.vertices[triangle.indices[1] as usize].position;
                let v2 = mesh.vertices[triangle.indices[2] as usize].position;

                let edge1 = v1 - v0;
                let edge2 = v2 - v0;
                edge1.cross(edge2).length() * 0.5
            })
            .sum();

        MeshStats {
            vertex_count,
            triangle_count,
            material_count: mesh.materials.len(),
            bounding_box_volume: bb_volume,
            surface_area,
        }
    }

    /// Get the total number of indices in a set of triangle strips
    ///
    /// Strips are stored as: [length, index1, index2, ..., length, index1, ...]
    pub fn get_strip_index_count(strips: &[Vec<u32>]) -> usize {
        strips.iter().map(|strip| strip.len()).sum()
    }

    /// Combine multiple triangle strips into a single optimized strip
    ///
    /// This implements the C++ `StripOptimizerClass::Combine_Strips` algorithm.
    /// Uses degenerate triangles to connect separate strips.
    pub fn combine_strips(strips: &[Vec<u32>]) -> Vec<u32> {
        if strips.is_empty() {
            return Vec::new();
        }

        if strips.len() == 1 {
            return strips[0].clone();
        }

        // Calculate output size: sum of all strip lengths + 3 indices per join
        let total_indices: usize = strips.iter().map(|s| s.len()).sum();
        let join_count = strips.len() - 1;
        let mut combined = Vec::with_capacity(total_indices + join_count * 3);

        let mut prev_even = true;

        for (i, strip) in strips.iter().enumerate() {
            if strip.is_empty() {
                continue;
            }

            if i != 0 {
                // Duplicate first vertex of new strip to create degenerate triangle
                combined.push(strip[0]);

                // If previous strip had odd length, add another duplicate
                // to maintain proper winding order
                if !prev_even {
                    combined.push(strip[0]);
                }
            }

            // Copy the entire strip
            combined.extend_from_slice(strip);

            // If not the last strip, duplicate last vertex
            if i != strips.len() - 1 {
                if let Some(&last) = strip.last() {
                    combined.push(last);
                }
            }

            // Track if this strip has even length (affects winding)
            prev_even = strip.len() % 2 == 0;
        }

        combined
    }

    /// Optimize the order of triangle strips for better cache coherency
    ///
    /// Reorders strips so that strips with shared vertices are placed adjacently.
    /// This implements the C++ `StripOptimizerClass::Optimize_Strip_Order` algorithm.
    pub fn optimize_strip_order(strips: &mut Vec<Vec<u32>>) {
        if strips.len() <= 1 {
            return;
        }

        let mut optimized = Vec::with_capacity(strips.len());
        let mut used = vec![false; strips.len()];

        // Start with the first strip
        optimized.push(strips[0].clone());
        used[0] = true;

        // Greedily select the next strip that shares the most vertices
        for _ in 1..strips.len() {
            let prev_strip = optimized.last().unwrap();
            let mut best_index = None;
            let mut best_similarity = 0;

            for (j, strip) in strips.iter().enumerate() {
                if used[j] {
                    continue;
                }

                let similarity = Self::compute_strip_similarity(prev_strip, strip);
                if similarity > best_similarity {
                    best_similarity = similarity;
                    best_index = Some(j);
                }
            }

            if let Some(index) = best_index {
                optimized.push(strips[index].clone());
                used[index] = true;
            } else {
                // Find any unused strip
                for (j, strip) in strips.iter().enumerate() {
                    if !used[j] {
                        optimized.push(strip.clone());
                        used[j] = true;
                        break;
                    }
                }
            }
        }

        *strips = optimized;
    }

    /// Compute similarity between two strips (number of shared vertices)
    fn compute_strip_similarity(strip_a: &[u32], strip_b: &[u32]) -> usize {
        let mut count = 0;

        for &index_a in strip_a {
            for &index_b in strip_b {
                if index_a == index_b {
                    count += 1;
                    break; // Count each shared vertex only once
                }
            }
        }

        count
    }

    /// Optimize triangle order for better vertex cache performance
    ///
    /// Reorders triangles so triangles that share vertices are placed adjacently.
    /// This implements the C++ `StripOptimizerClass::Optimize_Triangle_Order` algorithm.
    pub fn optimize_triangle_order(mesh: &mut MeshGeometry) {
        if mesh.triangles.is_empty() {
            return;
        }

        let mut optimized = Vec::with_capacity(mesh.triangles.len());
        let mut used = vec![false; mesh.triangles.len()];

        // Start with the first triangle
        optimized.push(mesh.triangles[0]);
        used[0] = true;

        // Greedily select the next triangle that shares the most vertices
        for _ in 1..mesh.triangles.len() {
            let prev = optimized.last().unwrap();
            let mut best_index = None;
            let mut best_similarity = 0;

            for (j, triangle) in mesh.triangles.iter().enumerate() {
                if used[j] {
                    continue;
                }

                let similarity = Self::compute_triangle_similarity(prev, triangle);
                if similarity > best_similarity {
                    best_similarity = similarity;
                    best_index = Some(j);

                    // If we found 2 shared vertices (an edge), that's the best we can get
                    if similarity >= 2 {
                        break;
                    }
                }
            }

            if let Some(index) = best_index {
                optimized.push(mesh.triangles[index]);
                used[index] = true;
            } else {
                // Find any unused triangle
                for (j, triangle) in mesh.triangles.iter().enumerate() {
                    if !used[j] {
                        optimized.push(*triangle);
                        used[j] = true;
                        break;
                    }
                }
            }
        }

        mesh.triangles = optimized;
    }

    /// Compute similarity between two triangles (number of shared vertices)
    fn compute_triangle_similarity(tri_a: &MeshTriangle, tri_b: &MeshTriangle) -> usize {
        let mut count = 0;

        for &va in &tri_a.indices {
            for &vb in &tri_b.indices {
                if va == vb {
                    count += 1;
                    break;
                }
            }
        }

        count
    }
}

/// Edge representation for mesh operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Edge {
    pub v0: usize,
    pub v1: usize,
}

impl Edge {
    pub fn new(v0: usize, v1: usize) -> Self {
        Self { v0, v1 }
    }
}

/// Quadratic error metric for mesh simplification
#[derive(Debug, Clone, Copy)]
pub struct Quadric {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
    pub g: f32,
    pub h: f32,
    pub i: f32,
    pub j: f32,
}

impl Default for Quadric {
    fn default() -> Self {
        Self::new()
    }
}

impl Quadric {
    pub fn new() -> Self {
        Self {
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
            g: 0.0,
            h: 0.0,
            i: 0.0,
            j: 0.0,
        }
    }

    pub fn from_plane(plane: &Plane) -> Self {
        let n = plane.normal;
        let d = plane.distance;

        Self {
            a: n.x * n.x,
            b: n.x * n.y,
            c: n.x * n.z,
            d: n.x * d,
            e: n.y * n.y,
            f: n.y * n.z,
            g: n.y * d,
            h: n.z * n.z,
            i: n.z * d,
            j: d * d,
        }
    }

    pub fn add_quadric(&mut self, other: &Quadric) {
        self.a += other.a;
        self.b += other.b;
        self.c += other.c;
        self.d += other.d;
        self.e += other.e;
        self.f += other.f;
        self.g += other.g;
        self.h += other.h;
        self.i += other.i;
        self.j += other.j;
    }

    pub fn error(&self) -> QuadricError {
        QuadricError { quadric: *self }
    }
}

/// Quadric error representation
#[derive(Debug, Clone, Copy)]
pub struct QuadricError {
    pub quadric: Quadric,
}

impl QuadricError {
    pub fn error(&self) -> f32 {
        // For now, return a simple error metric
        // In a full implementation, this would solve for the minimum error
        self.quadric.j
    }
}

impl std::ops::Add for QuadricError {
    type Output = QuadricError;

    fn add(self, other: QuadricError) -> QuadricError {
        let mut result = QuadricError {
            quadric: Quadric::new(),
        };
        result.quadric.add_quadric(&self.quadric);
        result.quadric.add_quadric(&other.quadric);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh_geometry::MeshVertex;
    use glam::Vec2;

    #[test]
    fn test_vertex_cache_optimization() {
        let mut mesh = MeshGeometry::new();

        // Add some vertices and triangles
        mesh.add_vertex(MeshVertex::new(Vec3::ZERO, Vec3::Y, Vec2::ZERO));
        mesh.add_vertex(MeshVertex::new(Vec3::X, Vec3::Y, Vec2::new(1.0, 0.0)));
        mesh.add_vertex(MeshVertex::new(Vec3::Z, Vec3::Y, Vec2::new(0.0, 1.0)));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(1.0, 0.0, 1.0),
            Vec3::Y,
            Vec2::ONE,
        ));

        mesh.add_triangle(MeshTriangle::new(0, 1, 2, 0));
        mesh.add_triangle(MeshTriangle::new(1, 2, 3, 0));

        let original_triangles = mesh.triangles.clone();
        MeshOptimizer::optimize_vertex_cache(&mut mesh);

        // The triangles should be reordered (though in this simple case it might stay the same)
        assert_eq!(mesh.triangle_count(), original_triangles.len());
    }

    #[test]
    fn test_duplicate_vertex_removal() {
        let mut mesh = MeshGeometry::new();

        // Add duplicate vertices
        mesh.add_vertex(MeshVertex::new(Vec3::ZERO, Vec3::Y, Vec2::ZERO));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.001, 0.0, 0.0),
            Vec3::Y,
            Vec2::ZERO,
        ));

        mesh.add_triangle(MeshTriangle::new(0, 1, 0, 0)); // Degenerate triangle

        MeshOptimizer::remove_duplicate_vertices(&mut mesh, 0.01);

        // Should have welded the vertices
        assert_eq!(mesh.vertex_count(), 1);
    }

    #[test]
    fn test_mesh_stats() {
        let cube = MeshBuilder::create_cube(2.0);
        let stats = MeshOptimizer::compute_mesh_stats(&cube);

        assert_eq!(stats.vertex_count, 8);
        assert_eq!(stats.triangle_count, 12);
        assert!(stats.surface_area > 0.0);
    }

    #[test]
    fn test_combine_strips() {
        let strip1 = vec![0, 1, 2, 3, 4];
        let strip2 = vec![5, 6, 7, 8];
        let strip3 = vec![9, 10, 11];

        let strips = vec![strip1.clone(), strip2.clone(), strip3.clone()];
        let combined = MeshOptimizer::combine_strips(&strips);

        // Combined strip should be longer than individual strips
        // and contain all original indices plus degenerate triangles
        assert!(combined.len() > strip1.len() + strip2.len() + strip3.len());

        // Should contain all original indices
        for &idx in &strip1 {
            assert!(combined.contains(&idx));
        }
        for &idx in &strip2 {
            assert!(combined.contains(&idx));
        }
        for &idx in &strip3 {
            assert!(combined.contains(&idx));
        }
    }

    #[test]
    fn test_combine_empty_strips() {
        let empty: Vec<Vec<u32>> = Vec::new();
        let combined = MeshOptimizer::combine_strips(&empty);
        assert!(combined.is_empty());

        let single = vec![vec![0, 1, 2, 3]];
        let combined_single = MeshOptimizer::combine_strips(&single);
        assert_eq!(combined_single, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_optimize_strip_order() {
        // Create strips with some shared vertices
        let mut strips = vec![
            vec![0, 1, 2, 3], // Shares 0 and 1 with strip 1
            vec![0, 1, 4, 5], // Shares 0 and 1 with strip 0
            vec![6, 7, 8, 9], // No shared vertices
        ];

        MeshOptimizer::optimize_strip_order(&mut strips);

        // After optimization, strips 0 and 1 should be adjacent
        assert_eq!(strips.len(), 3);

        // First two strips should share vertices
        let similarity = MeshOptimizer::compute_strip_similarity(&strips[0], &strips[1]);
        assert!(similarity > 0, "First two strips should share vertices");
    }

    #[test]
    fn test_optimize_triangle_order() {
        let mut mesh = MeshGeometry::new();

        // Create vertices
        for i in 0..6 {
            mesh.add_vertex(MeshVertex::new(
                Vec3::new(i as f32, 0.0, 0.0),
                Vec3::Y,
                Vec2::ZERO,
            ));
        }

        // Add triangles with some shared edges
        mesh.add_triangle(MeshTriangle::new(0, 1, 2, 0)); // Shares edge 1-2 with tri 1
        mesh.add_triangle(MeshTriangle::new(1, 2, 3, 0)); // Shares edge 1-2 with tri 0
        mesh.add_triangle(MeshTriangle::new(4, 5, 3, 0)); // Less connected

        MeshOptimizer::optimize_triangle_order(&mut mesh);

        // After optimization, triangles 0 and 1 should be adjacent
        assert_eq!(mesh.triangle_count(), 3);

        // First two triangles should share vertices
        let similarity =
            MeshOptimizer::compute_triangle_similarity(&mesh.triangles[0], &mesh.triangles[1]);
        assert!(similarity >= 2, "First two triangles should share an edge");
    }

    #[test]
    fn test_get_strip_index_count() {
        let strips = vec![vec![0, 1, 2, 3], vec![4, 5, 6], vec![7, 8, 9, 10, 11]];

        let count = MeshOptimizer::get_strip_index_count(&strips);
        assert_eq!(count, 4 + 3 + 5);
    }
}
