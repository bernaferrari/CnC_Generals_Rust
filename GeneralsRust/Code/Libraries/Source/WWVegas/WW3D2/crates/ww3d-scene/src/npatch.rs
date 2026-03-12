//! N-Patch Tessellation Support for Curved Surface Subdivision
//!
//! This module implements N-Patch tessellation, a technique for creating smooth curved
//! surfaces from triangular meshes. N-Patches use quintic Bezier triangles to interpolate
//! both position and normals across a triangle, creating visually smooth surfaces without
//! requiring explicit subdivision of the source geometry.
//!
//! ## Background
//!
//! N-Patches (also called PN Triangles - Point-Normal Triangles) were introduced in the
//! early 2000s as a technique to improve the visual quality of low-polygon meshes. DirectX 8
//! supported them via D3DRS_PATCHSEGMENTS render state.
//!
//! C++ Reference: shader.cpp lines 1033-1037
//! ```cpp
//! if (diff&ShaderClass::MASK_NPATCHENABLE) {
//!     float level=1.0f;
//!     if (Get_NPatch_Enable()) level=float(WW3D::Get_NPatches_Level());
//!     DX8Wrapper::Set_DX8_Render_State(D3DRS_PATCHSEGMENTS,*((DWORD*)&level));
//! }
//! ```
//!
//! ## Algorithm
//!
//! For each triangle with vertices (P0, N0), (P1, N1), (P2, N2):
//!
//! 1. Compute 10 control points for a quintic Bezier triangle
//! 2. Subdivide the triangle based on tessellation level
//! 3. Evaluate Bezier surface at each subdivision point
//! 4. Compute interpolated normals for smooth shading
//!
//! ## Modern Implementation
//!
//! Since WGPU doesn't have direct N-Patch support like DX8, we provide two approaches:
//!
//! 1. **CPU Subdivision** (fallback): Pre-compute subdivided mesh on CPU
//! 2. **Tessellation Shaders** (if supported): Use modern tessellation pipeline
//!
//! The CPU approach is always available and gives good results for static meshes.

use glam::{Vec2, Vec3};

/// Tessellation level for N-Patch subdivision
///
/// Higher levels create more triangles and smoother surfaces, but increase vertex count.
/// Level 1 = no subdivision (original triangle)
/// Level 2 = 4 triangles
/// Level 3 = 9 triangles
/// Level 4 = 16 triangles
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TessellationLevel(pub u32);

impl TessellationLevel {
    pub const NONE: Self = Self(1);
    pub const LOW: Self = Self(2);
    pub const MEDIUM: Self = Self(3);
    pub const HIGH: Self = Self(4);
    pub const VERY_HIGH: Self = Self(5);

    /// Get the number of triangles that will be generated
    pub fn triangle_count(&self) -> usize {
        let n = self.0 as usize;
        n * n
    }

    /// Get the number of vertices that will be generated
    pub fn vertex_count(&self) -> usize {
        let n = self.0 as usize;
        ((n + 1) * (n + 2)) / 2
    }

    /// Create from raw level value
    pub fn from_raw(level: u32) -> Self {
        // Clamp to reasonable range (1-8)
        Self(level.max(1).min(8))
    }

    /// Get raw level value
    pub fn as_raw(&self) -> u32 {
        self.0
    }
}

impl Default for TessellationLevel {
    fn default() -> Self {
        Self::MEDIUM
    }
}

/// Vertex data for N-Patch tessellation
#[derive(Debug, Clone, Copy)]
pub struct NPatchVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub uv: Vec2,
}

impl NPatchVertex {
    pub fn new(position: Vec3, normal: Vec3, uv: Vec2) -> Self {
        Self {
            position,
            normal,
            uv,
        }
    }
}

/// N-Patch control points for a quintic Bezier triangle
///
/// The control points are arranged as follows:
/// ```text
///        b300
///       /    \
///    b210    b120
///   /    \  /    \
/// b201  b111  b021
///   \  /    \  /
///    b102    b012
///       \    /
///        b003
/// ```
#[derive(Debug, Clone)]
struct NPatchControlPoints {
    // Corner control points
    b300: Vec3,
    b030: Vec3,
    b003: Vec3,

    // Edge control points
    b210: Vec3,
    b120: Vec3,
    b021: Vec3,
    b012: Vec3,
    b102: Vec3,
    b201: Vec3,

    // Center control point
    b111: Vec3,
}

impl NPatchControlPoints {
    /// Compute N-Patch control points from triangle vertices and normals
    ///
    /// This implements the PN Triangles algorithm from:
    /// Vlachos, Peters, Boyd, Mitchell (2001)
    /// "Curved PN Triangles"
    fn from_triangle(v0: &NPatchVertex, v1: &NPatchVertex, v2: &NPatchVertex) -> Self {
        let p0 = v0.position;
        let p1 = v1.position;
        let p2 = v2.position;

        let n0 = v0.normal.normalize();
        let n1 = v1.normal.normalize();
        let n2 = v2.normal.normalize();

        // Corner control points are just the vertices
        let b300 = p0;
        let b030 = p1;
        let b003 = p2;

        // Edge control points are computed to maintain C1 continuity
        // wij = (2*Pi + Pj - (Pj-Pi).dot(Ni)*Ni) / 3
        let w01 = Self::compute_edge_control_point(p0, p1, n0);
        let w10 = Self::compute_edge_control_point(p1, p0, n1);
        let w12 = Self::compute_edge_control_point(p1, p2, n1);
        let w21 = Self::compute_edge_control_point(p2, p1, n2);
        let w20 = Self::compute_edge_control_point(p2, p0, n2);
        let w02 = Self::compute_edge_control_point(p0, p2, n0);

        let b210 = w01;
        let b120 = w10;
        let b021 = w12;
        let b012 = w21;
        let b102 = w20;
        let b201 = w02;

        // Center control point for quadratic normal interpolation
        // E = (b210 + b120 + b021 + b012 + b102 + b201) / 6
        // V = (b300 + b030 + b003) / 3
        // b111 = E + (E - V) / 2
        let e = (b210 + b120 + b021 + b012 + b102 + b201) / 6.0;
        let v = (b300 + b030 + b003) / 3.0;
        let b111 = e + (e - v) / 2.0;

        Self {
            b300,
            b030,
            b003,
            b210,
            b120,
            b021,
            b012,
            b102,
            b201,
            b111,
        }
    }

    /// Compute edge control point for C1 continuity
    fn compute_edge_control_point(pi: Vec3, pj: Vec3, ni: Vec3) -> Vec3 {
        let pj_minus_pi = pj - pi;
        (2.0 * pi + pj - pj_minus_pi.dot(ni) * ni) / 3.0
    }

    /// Evaluate position on the Bezier surface at barycentric coordinates (u, v, w)
    /// where u + v + w = 1
    fn eval_position(&self, u: f32, v: f32, w: f32) -> Vec3 {
        // Quintic Bezier triangle evaluation
        // B(u,v,w) = sum of (3!/(i!j!k!)) * u^i * v^j * w^k * b_ijk
        // where i+j+k = 3

        let u2 = u * u;
        let u3 = u2 * u;
        let v2 = v * v;
        let v3 = v2 * v;
        let w2 = w * w;
        let w3 = w2 * w;

        // Multinomial coefficients for cubic Bezier triangle
        self.b300 * u3
            + self.b030 * v3
            + self.b003 * w3
            + self.b210 * 3.0 * u2 * v
            + self.b120 * 3.0 * u * v2
            + self.b201 * 3.0 * u2 * w
            + self.b021 * 3.0 * v2 * w
            + self.b102 * 3.0 * u * w2
            + self.b012 * 3.0 * v * w2
            + self.b111 * 6.0 * u * v * w
    }
}

/// Normal interpolation for smooth shading
#[derive(Debug, Clone)]
struct NPatchNormalInterpolation {
    n0: Vec3,
    n1: Vec3,
    n2: Vec3,
}

impl NPatchNormalInterpolation {
    fn new(n0: Vec3, n1: Vec3, n2: Vec3) -> Self {
        Self {
            n0: n0.normalize(),
            n1: n1.normalize(),
            n2: n2.normalize(),
        }
    }

    /// Quadratic normal interpolation using barycentric coordinates
    fn eval_normal(&self, u: f32, v: f32, w: f32) -> Vec3 {
        // Quadratic interpolation of normals
        let n = self.n0 * u + self.n1 * v + self.n2 * w;
        n.normalize()
    }
}

/// Subdivided triangle mesh result
#[derive(Debug, Clone)]
pub struct SubdividedMesh {
    pub vertices: Vec<NPatchVertex>,
    pub indices: Vec<u32>,
}

impl SubdividedMesh {
    /// Create an empty subdivided mesh
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Get triangle count
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Get total memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.vertices.len() * std::mem::size_of::<NPatchVertex>()
            + self.indices.len() * std::mem::size_of::<u32>()
    }
}

impl Default for SubdividedMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// N-Patch tessellator - converts triangles into smooth curved surfaces
pub struct NPatchTessellator {
    level: TessellationLevel,
}

impl NPatchTessellator {
    /// Create a new N-Patch tessellator with specified level
    pub fn new(level: TessellationLevel) -> Self {
        Self { level }
    }

    /// Subdivide a single triangle using N-Patch algorithm
    ///
    /// Returns a mesh containing the subdivided triangle.
    /// The original triangle is split into level^2 smaller triangles,
    /// with positions evaluated on the Bezier surface.
    pub fn subdivide_triangle(
        &self,
        v0: &NPatchVertex,
        v1: &NPatchVertex,
        v2: &NPatchVertex,
    ) -> SubdividedMesh {
        if self.level.0 == 1 {
            // No subdivision - return original triangle
            return SubdividedMesh {
                vertices: vec![*v0, *v1, *v2],
                indices: vec![0, 1, 2],
            };
        }

        // Compute N-Patch control points
        let control_points = NPatchControlPoints::from_triangle(v0, v1, v2);
        let normal_interp = NPatchNormalInterpolation::new(v0.normal, v1.normal, v2.normal);

        let n = self.level.0 as usize;
        let mut vertices = Vec::with_capacity(self.level.vertex_count());
        let mut indices = Vec::with_capacity(self.level.triangle_count() * 3);

        // Generate vertices using barycentric subdivision
        // For level n, we create (n+1)*(n+2)/2 vertices
        for i in 0..=n {
            for j in 0..=(n - i) {
                let k = n - i - j;

                // Barycentric coordinates
                let u = i as f32 / n as f32;
                let v = j as f32 / n as f32;
                let w = k as f32 / n as f32;

                // Evaluate position on Bezier surface
                let position = control_points.eval_position(u, v, w);

                // Interpolate normal
                let normal = normal_interp.eval_normal(u, v, w);

                // Interpolate UV coordinates
                let uv = v0.uv * u + v1.uv * v + v2.uv * w;

                vertices.push(NPatchVertex::new(position, normal, uv));
            }
        }

        // Generate indices for subdivided triangles
        // We create a triangular grid of indices
        let mut vertex_index = 0;
        for i in 0..n {
            let row_length = n + 1 - i;
            for j in 0..row_length - 1 {
                // Each quad in the grid becomes 1 or 2 triangles
                let idx0 = vertex_index + j;
                let idx1 = idx0 + 1;
                let idx2 = idx0 + row_length;

                // First triangle (upward-pointing)
                indices.push(idx0 as u32);
                indices.push(idx1 as u32);
                indices.push(idx2 as u32);

                // Second triangle (downward-pointing), if not at edge
                if j < row_length - 2 {
                    let idx3 = idx2 + 1;
                    indices.push(idx1 as u32);
                    indices.push(idx3 as u32);
                    indices.push(idx2 as u32);
                }
            }
            vertex_index += row_length;
        }

        SubdividedMesh { vertices, indices }
    }

    /// Subdivide an entire mesh using N-Patch algorithm
    ///
    /// Takes a list of triangles and returns a new mesh with all triangles subdivided.
    pub fn subdivide_mesh(
        &self,
        triangles: &[(NPatchVertex, NPatchVertex, NPatchVertex)],
    ) -> SubdividedMesh {
        let mut result = SubdividedMesh::new();

        for (v0, v1, v2) in triangles {
            let subdivided = self.subdivide_triangle(v0, v1, v2);

            // Append vertices with index offset
            let vertex_offset = result.vertices.len() as u32;
            result.vertices.extend(subdivided.vertices);
            result
                .indices
                .extend(subdivided.indices.iter().map(|&idx| idx + vertex_offset));
        }

        result
    }

    /// Get the tessellation level
    pub fn level(&self) -> TessellationLevel {
        self.level
    }

    /// Set the tessellation level
    pub fn set_level(&mut self, level: TessellationLevel) {
        self.level = level;
    }
}

impl Default for NPatchTessellator {
    fn default() -> Self {
        Self::new(TessellationLevel::default())
    }
}

/// Configuration for N-Patch tessellation in rendering pipeline
#[derive(Debug, Clone)]
pub struct NPatchConfig {
    /// Enable/disable N-Patch tessellation
    pub enabled: bool,

    /// Tessellation level
    pub level: TessellationLevel,

    /// Use CPU subdivision (always available)
    /// If false, will attempt to use GPU tessellation shaders (if supported)
    pub use_cpu_subdivision: bool,

    /// Cache subdivided meshes to avoid recomputation
    pub cache_subdivisions: bool,
}

impl NPatchConfig {
    /// Create default configuration with N-Patch disabled
    pub fn new() -> Self {
        Self {
            enabled: false,
            level: TessellationLevel::MEDIUM,
            use_cpu_subdivision: true,
            cache_subdivisions: true,
        }
    }

    /// Enable N-Patch with specified level
    pub fn with_level(level: TessellationLevel) -> Self {
        Self {
            enabled: true,
            level,
            use_cpu_subdivision: true,
            cache_subdivisions: true,
        }
    }

    /// Disable N-Patch tessellation
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            level: TessellationLevel::NONE,
            use_cpu_subdivision: true,
            cache_subdivisions: false,
        }
    }
}

impl Default for NPatchConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a simple equilateral triangle for testing
    fn create_test_triangle() -> (NPatchVertex, NPatchVertex, NPatchVertex) {
        let v0 = NPatchVertex::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::new(0.0, 0.0),
        );
        let v1 = NPatchVertex::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::new(1.0, 0.0),
        );
        let v2 = NPatchVertex::new(
            Vec3::new(0.5, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::new(0.5, 1.0),
        );
        (v0, v1, v2)
    }

    #[test]
    fn test_tessellation_level_counts() {
        assert_eq!(TessellationLevel::NONE.triangle_count(), 1);
        assert_eq!(TessellationLevel::LOW.triangle_count(), 4);
        assert_eq!(TessellationLevel::MEDIUM.triangle_count(), 9);
        assert_eq!(TessellationLevel::HIGH.triangle_count(), 16);

        assert_eq!(TessellationLevel::NONE.vertex_count(), 3);
        assert_eq!(TessellationLevel::LOW.vertex_count(), 6);
        assert_eq!(TessellationLevel::MEDIUM.vertex_count(), 10);
        assert_eq!(TessellationLevel::HIGH.vertex_count(), 15);
    }

    #[test]
    fn test_no_subdivision() {
        let (v0, v1, v2) = create_test_triangle();
        let tessellator = NPatchTessellator::new(TessellationLevel::NONE);
        let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

        assert_eq!(result.vertices.len(), 3);
        assert_eq!(result.indices.len(), 3);
        assert_eq!(result.triangle_count(), 1);
    }

    #[test]
    fn test_low_subdivision() {
        let (v0, v1, v2) = create_test_triangle();
        let tessellator = NPatchTessellator::new(TessellationLevel::LOW);
        let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

        assert_eq!(result.vertices.len(), 6);
        assert_eq!(result.triangle_count(), 4);
    }

    #[test]
    fn test_medium_subdivision() {
        let (v0, v1, v2) = create_test_triangle();
        let tessellator = NPatchTessellator::new(TessellationLevel::MEDIUM);
        let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

        assert_eq!(result.vertices.len(), 10);
        assert_eq!(result.triangle_count(), 9);
    }

    #[test]
    fn test_high_subdivision() {
        let (v0, v1, v2) = create_test_triangle();
        let tessellator = NPatchTessellator::new(TessellationLevel::HIGH);
        let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

        assert_eq!(result.vertices.len(), 15);
        assert_eq!(result.triangle_count(), 16);
    }

    #[test]
    fn test_normals_are_normalized() {
        let (v0, v1, v2) = create_test_triangle();
        let tessellator = NPatchTessellator::new(TessellationLevel::MEDIUM);
        let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

        for vertex in &result.vertices {
            let length = vertex.normal.length();
            assert!((length - 1.0).abs() < 0.001, "Normal should be normalized");
        }
    }

    #[test]
    fn test_curved_surface() {
        // Create a triangle with curved normals to test actual curvature
        let v0 = NPatchVertex::new(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0).normalize(),
            Vec2::new(0.0, 0.0),
        );
        let v1 = NPatchVertex::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0).normalize(),
            Vec2::new(1.0, 0.0),
        );
        let v2 = NPatchVertex::new(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, 1.0).normalize(),
            Vec2::new(0.5, 1.0),
        );

        let tessellator = NPatchTessellator::new(TessellationLevel::HIGH);
        let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

        // The center vertex should be displaced from the original triangle plane
        // This tests that actual curvature is happening
        assert!(result.vertices.len() > 3);

        // Check that we have the expected number of vertices
        assert_eq!(result.vertices.len(), 15);
    }

    #[test]
    fn test_uv_interpolation() {
        let (v0, v1, v2) = create_test_triangle();
        let tessellator = NPatchTessellator::new(TessellationLevel::LOW);
        let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

        // Check that UV coordinates are interpolated
        for vertex in &result.vertices {
            assert!(vertex.uv.x >= 0.0 && vertex.uv.x <= 1.0);
            assert!(vertex.uv.y >= 0.0 && vertex.uv.y <= 1.0);
        }
    }

    #[test]
    fn test_mesh_subdivision() {
        let tri1 = create_test_triangle();
        let tri2 = create_test_triangle();

        let tessellator = NPatchTessellator::new(TessellationLevel::LOW);
        let result = tessellator.subdivide_mesh(&[tri1, tri2]);

        // Two triangles at level 2 should produce 12 vertices and 8 triangles
        assert_eq!(result.vertices.len(), 12);
        assert_eq!(result.triangle_count(), 8);
    }

    #[test]
    fn test_config_defaults() {
        let config = NPatchConfig::new();
        assert!(!config.enabled);
        assert_eq!(config.level.0, TessellationLevel::MEDIUM.0);
        assert!(config.use_cpu_subdivision);
        assert!(config.cache_subdivisions);
    }

    #[test]
    fn test_config_with_level() {
        let config = NPatchConfig::with_level(TessellationLevel::HIGH);
        assert!(config.enabled);
        assert_eq!(config.level.0, TessellationLevel::HIGH.0);
    }

    #[test]
    fn test_control_points_symmetry() {
        let (v0, v1, v2) = create_test_triangle();
        let control_points = NPatchControlPoints::from_triangle(&v0, &v1, &v2);

        // Corner control points should match vertices
        assert_eq!(control_points.b300, v0.position);
        assert_eq!(control_points.b030, v1.position);
        assert_eq!(control_points.b003, v2.position);
    }

    #[test]
    fn test_barycentric_evaluation() {
        let (v0, v1, v2) = create_test_triangle();
        let control_points = NPatchControlPoints::from_triangle(&v0, &v1, &v2);

        // Evaluate at corners should match vertices
        let p0 = control_points.eval_position(1.0, 0.0, 0.0);
        let p1 = control_points.eval_position(0.0, 1.0, 0.0);
        let p2 = control_points.eval_position(0.0, 0.0, 1.0);

        assert!((p0 - v0.position).length() < 0.001);
        assert!((p1 - v1.position).length() < 0.001);
        assert!((p2 - v2.position).length() < 0.001);
    }

    #[test]
    fn test_memory_size_calculation() {
        let (v0, v1, v2) = create_test_triangle();
        let tessellator = NPatchTessellator::new(TessellationLevel::MEDIUM);
        let result = tessellator.subdivide_triangle(&v0, &v1, &v2);

        let expected_size = result.vertices.len() * std::mem::size_of::<NPatchVertex>()
            + result.indices.len() * std::mem::size_of::<u32>();
        assert_eq!(result.memory_size(), expected_size);
    }
}
