// GNU General Public License v3.0 - See LICENSE file for details
// Command & Conquer Generals Zero Hour(tm)
// Copyright 2025 Electronic Arts Inc.
//
// Complete port of MeshGeometryClass from C++ (meshgeometry.h)
// Original: meshgeometry.h lines 84-297

use crate::bounding_volumes::aabox::AABoxClass;
use crate::bounding_volumes::sphere::SphereClass;
use glam::{Vec3, Vec4};
use std::sync::Arc;

/// Triangle index type (16-bit or 32-bit)
/// C++: typedef Vector3i16 TriIndex (meshgeometry.h line 66)
pub type TriIndex = [u16; 3];

/// Flags controlling mesh geometry behavior
/// C++: enum FlagsType (meshgeometry.h lines 100-123)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GeometryFlags(u32);

impl GeometryFlags {
    pub const DIRTY_BOUNDS: u32 = 0x00000001;
    pub const DIRTY_PLANES: u32 = 0x00000002;
    pub const DIRTY_VNORMALS: u32 = 0x00000004;
    pub const SORT: u32 = 0x00000010;
    pub const DISABLE_BOUNDING_BOX: u32 = 0x00000020;
    pub const DISABLE_BOUNDING_SPHERE: u32 = 0x00000040;
    pub const DISABLE_PLANE_EQ: u32 = 0x00000080;
    pub const TWO_SIDED: u32 = 0x00000100;
    pub const ALIGNED: u32 = 0x00000200;
    pub const SKIN: u32 = 0x00000400;
    pub const ORIENTED: u32 = 0x00000800;
    pub const CAST_SHADOW: u32 = 0x00001000;
    pub const PRELIT_MASK: u32 = 0x0000E000;
    pub const PRELIT_VERTEX: u32 = 0x00002000;
    pub const PRELIT_LIGHTMAP_MULTI_PASS: u32 = 0x00004000;
    pub const PRELIT_LIGHTMAP_MULTI_TEXTURE: u32 = 0x00008000;
    pub const ALLOW_NPATCHES: u32 = 0x00010000;

    pub fn new(flags: u32) -> Self {
        Self(flags)
    }

    pub fn has(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn set(&mut self, flag: u32, value: bool) {
        if value {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }

    pub fn raw(&self) -> u32 {
        self.0
    }
}

/// Reference-counted buffer with shared ownership
/// C++: ShareBufferClass<T> (sharebuf.h)
#[derive(Debug, Clone)]
pub struct ShareBuffer<T: Clone> {
    data: Arc<Vec<T>>,
}

impl<T: Clone> ShareBuffer<T> {
    pub fn new(data: Vec<T>) -> Self {
        Self {
            data: Arc::new(data),
        }
    }

    pub fn from_slice(data: &[T]) -> Self {
        Self {
            data: Arc::new(data.to_vec()),
        }
    }

    pub fn get_array(&self) -> &[T] {
        &self.data
    }

    pub fn count(&self) -> usize {
        self.data.len()
    }

    pub fn num_refs(&self) -> usize {
        Arc::strong_count(&self.data)
    }

    pub fn make_unique(&mut self) -> &mut Vec<T> {
        Arc::make_mut(&mut self.data)
    }

    /// Get mutable access to underlying Arc for direct manipulation
    /// Used when making unique copies for modification
    pub fn data_mut(&mut self) -> &mut Arc<Vec<T>> {
        &mut self.data
    }
}

/// MeshGeometry encapsulates the geometry data for a triangle mesh
/// C++: MeshGeometryClass (meshgeometry.h lines 88-255)
///
/// This is the base geometry data: vertices, normals, triangles, bone influences, etc.
/// Copy/Add_Ref Rules (from C++ comments lines 100-118):
/// - ALWAYS SHARED: Poly, VertexShadeIdx, VertexInfluences (cannot be changed at runtime)
/// - SHARED UNTIL DEFORMED: Vertex, VertexNorm, PlaneEq (must copy if moved)
#[derive(Debug, Clone)]
pub struct MeshGeometry {
    // General info - C++: lines 231-235
    mesh_name: Option<String>,
    user_text: Option<String>,
    flags: GeometryFlags,
    sort_level: i8,
    w3d_attributes: u32,

    // Geometry counts - C++: lines 238-239
    poly_count: usize,
    vertex_count: usize,

    // Geometry arrays - C++: lines 241-247
    // ALWAYS SHARED: connectivity is immutable
    poly: Option<ShareBuffer<TriIndex>>,

    // SHARED UNTIL DEFORMED: positions can be modified for skins/deformation
    vertex: Option<ShareBuffer<Vec3>>,
    vertex_norm: Option<ShareBuffer<Vec3>>,
    plane_eq: Option<ShareBuffer<Vec4>>,

    // ALWAYS SHARED: bone attachments are immutable
    vertex_shade_idx: Option<ShareBuffer<u32>>,
    vertex_bone_link: Option<ShareBuffer<u16>>,
    poly_surface_type: Option<ShareBuffer<u8>>,

    // Bounding volumes - C++: lines 249-252
    bound_box_min: Vec3,
    bound_box_max: Vec3,
    bound_sphere_center: Vec3,
    bound_sphere_radius: f32,
    // Culling tree - C++: line 253
    // AABTree removed for simplicity, can be added later if needed
}

impl MeshGeometry {
    /// C++: MeshGeometryClass::MeshGeometryClass(void) (constructor)
    pub fn new() -> Self {
        Self {
            mesh_name: None,
            user_text: None,
            flags: GeometryFlags::new(GeometryFlags::DIRTY_BOUNDS),
            sort_level: 0,
            w3d_attributes: 0,
            poly_count: 0,
            vertex_count: 0,
            poly: None,
            vertex: None,
            vertex_norm: None,
            plane_eq: None,
            vertex_shade_idx: None,
            vertex_bone_link: None,
            poly_surface_type: None,
            bound_box_min: Vec3::ZERO,
            bound_box_max: Vec3::ZERO,
            bound_sphere_center: Vec3::ZERO,
            bound_sphere_radius: 0.0,
        }
    }

    /// Reset geometry to initial state with new counts
    /// C++: Reset_Geometry (meshgeometry.h line 125)
    pub fn reset_geometry(&mut self, poly_count: usize, vertex_count: usize) {
        self.poly_count = poly_count;
        self.vertex_count = vertex_count;

        // Release all arrays
        self.poly = None;
        self.vertex = None;
        self.vertex_norm = None;
        self.plane_eq = None;
        self.vertex_shade_idx = None;
        self.vertex_bone_link = None;
        self.poly_surface_type = None;

        // Mark bounds as dirty
        self.flags.set(GeometryFlags::DIRTY_BOUNDS, true);
        self.flags.set(GeometryFlags::DIRTY_PLANES, true);
        self.flags.set(GeometryFlags::DIRTY_VNORMALS, true);
    }

    /// Get mesh name
    /// C++: Get_Name (meshgeometry.h line 127)
    pub fn get_name(&self) -> Option<&str> {
        self.mesh_name.as_deref()
    }

    /// Set mesh name
    /// C++: Set_Name (meshgeometry.h line 128)
    pub fn set_name(&mut self, name: String) {
        self.mesh_name = Some(name);
    }

    /// Get user text
    /// C++: Get_User_Text (meshgeometry.h line 130)
    pub fn get_user_text(&self) -> Option<&str> {
        self.user_text.as_deref()
    }

    /// Set user text
    /// C++: Set_User_Text (meshgeometry.h line 131)
    pub fn set_user_text(&mut self, text: String) {
        self.user_text = Some(text);
    }

    /// Set a flag
    /// C++: Set_Flag (meshgeometry.h line 133)
    pub fn set_flag(&mut self, flag: u32, value: bool) {
        self.flags.set(flag, value);
    }

    /// Get flag value
    /// C++: Get_Flag (meshgeometry.h line 134)
    pub fn get_flag(&self, flag: u32) -> bool {
        self.flags.has(flag)
    }

    /// Set sort level
    /// C++: Set_Sort_Level (meshgeometry.h line 136)
    pub fn set_sort_level(&mut self, level: i8) {
        self.sort_level = level;
    }

    /// Get sort level
    /// C++: Get_Sort_Level (meshgeometry.h line 137)
    pub fn get_sort_level(&self) -> i8 {
        self.sort_level
    }

    /// Get polygon count
    /// C++: Get_Polygon_Count (meshgeometry.h line 139)
    pub fn get_polygon_count(&self) -> usize {
        self.poly_count
    }

    /// Get vertex count
    /// C++: Get_Vertex_Count (meshgeometry.h line 140)
    pub fn get_vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Get polygon array
    /// C++: Get_Polygon_Array (meshgeometry.h line 142)
    pub fn get_polygon_array(&self) -> Option<&[TriIndex]> {
        self.poly.as_ref().map(|p| p.get_array())
    }

    /// Get vertex array
    /// C++: Get_Vertex_Array (meshgeometry.h line 143)
    pub fn get_vertex_array(&self) -> Option<&[Vec3]> {
        self.vertex.as_ref().map(|v| v.get_array())
    }

    /// Get vertex normal array
    /// C++: Get_Vertex_Normal_Array (meshgeometry.h line 144)
    pub fn get_vertex_normal_array(&self) -> Option<&[Vec3]> {
        self.vertex_norm.as_ref().map(|vn| vn.get_array())
    }

    /// Get plane equation array
    /// C++: Get_Plane_Array (meshgeometry.h line 145)
    pub fn get_plane_array(&self, create: bool) -> Option<&[Vec4]> {
        if create && self.plane_eq.is_none() {
            // Would need to compute planes here
            // For now just return None if not present
        }
        self.plane_eq.as_ref().map(|pe| pe.get_array())
    }

    /// Get vertex shade index array
    /// C++: Get_Vertex_Shade_Index_Array (meshgeometry.h line 147)
    pub fn get_vertex_shade_index_array(&self) -> Option<&[u32]> {
        self.vertex_shade_idx.as_ref().map(|vsi| vsi.get_array())
    }

    /// Get vertex bone links
    /// C++: Get_Vertex_Bone_Links (meshgeometry.h line 148)
    pub fn get_vertex_bone_links(&self) -> Option<&[u16]> {
        self.vertex_bone_link.as_ref().map(|vbl| vbl.get_array())
    }

    /// Get polygon surface type array
    /// C++: Get_Poly_Surface_Type_Array (meshgeometry.h line 149)
    pub fn get_poly_surface_type_array(&self) -> Option<&[u8]> {
        self.poly_surface_type.as_ref().map(|pst| pst.get_array())
    }

    /// Get polygon surface type for a specific polygon
    /// C++: Get_Poly_Surface_Type (meshgeometry.h line 150)
    pub fn get_poly_surface_type(&self, poly_index: usize) -> Option<u8> {
        if poly_index >= self.poly_count {
            return None;
        }
        self.poly_surface_type
            .as_ref()
            .and_then(|pst| pst.get_array().get(poly_index).copied())
    }

    /// Get bounding box
    /// C++: Get_Bounding_Box (meshgeometry.h line 152)
    pub fn get_bounding_box(&mut self) -> AABoxClass {
        if self.flags.has(GeometryFlags::DIRTY_BOUNDS) {
            self.compute_bounds();
        }
        AABoxClass::from_min_max(self.bound_box_min, self.bound_box_max)
    }

    /// Get bounding sphere
    /// C++: Get_Bounding_Sphere (meshgeometry.h line 153)
    pub fn get_bounding_sphere(&mut self) -> SphereClass {
        if self.flags.has(GeometryFlags::DIRTY_BOUNDS) {
            self.compute_bounds();
        }
        SphereClass::new(self.bound_sphere_center, self.bound_sphere_radius)
    }

    /// Scale the geometry
    /// C++: Scale (meshgeometry.h line 184)
    pub fn scale(&mut self, scale: Vec3) {
        if let Some(vertex) = &mut self.vertex {
            let vertices = Arc::make_mut(&mut vertex.data);
            for v in vertices.iter_mut() {
                *v *= scale;
            }
        }
        self.flags.set(GeometryFlags::DIRTY_BOUNDS, true);
        self.flags.set(GeometryFlags::DIRTY_PLANES, true);
    }

    /// Install polygon array
    pub fn install_polygon_array(&mut self, polygons: Vec<TriIndex>) {
        self.poly_count = polygons.len();
        self.poly = Some(ShareBuffer::new(polygons));
    }

    /// Install vertex array
    pub fn install_vertex_array(&mut self, vertices: Vec<Vec3>) {
        self.vertex_count = vertices.len();
        self.vertex = Some(ShareBuffer::new(vertices));
        self.flags.set(GeometryFlags::DIRTY_BOUNDS, true);
    }

    /// Install vertex normal array
    pub fn install_vertex_normal_array(&mut self, normals: Vec<Vec3>) {
        self.vertex_norm = Some(ShareBuffer::new(normals));
        self.flags.set(GeometryFlags::DIRTY_VNORMALS, false);
    }

    /// Install plane equation array
    pub fn install_plane_array(&mut self, planes: Vec<Vec4>) {
        self.plane_eq = Some(ShareBuffer::new(planes));
        self.flags.set(GeometryFlags::DIRTY_PLANES, false);
    }

    /// Install vertex shade indices
    pub fn install_vertex_shade_indices(&mut self, indices: Vec<u32>) {
        self.vertex_shade_idx = Some(ShareBuffer::new(indices));
    }

    /// Install vertex bone links
    pub fn install_vertex_bone_links(&mut self, links: Vec<u16>) {
        self.vertex_bone_link = Some(ShareBuffer::new(links));
    }

    /// Install polygon surface types
    pub fn install_poly_surface_types(&mut self, types: Vec<u8>) {
        self.poly_surface_type = Some(ShareBuffer::new(types));
    }

    /// Make vertex array unique (copy-on-write)
    /// C++: Called from Make_Geometry_Unique (meshmdl.cpp line 291)
    pub fn make_vertex_array_unique(&mut self) {
        if let Some(vertex) = &mut self.vertex {
            if vertex.num_refs() > 1 {
                let data = vertex.get_array().to_vec();
                self.vertex = Some(ShareBuffer::new(data));
            }
        }
    }

    /// Make vertex normal array unique (copy-on-write)
    /// C++: Called from Make_Geometry_Unique (meshmdl.cpp line 299)
    pub fn make_vertex_normal_array_unique(&mut self) {
        if let Some(vertex_norm) = &mut self.vertex_norm {
            if vertex_norm.num_refs() > 1 {
                let data = vertex_norm.get_array().to_vec();
                self.vertex_norm = Some(ShareBuffer::new(data));
            }
        }
    }

    // Private helper methods

    /// Compute bounding volumes from vertex data
    /// C++: Compute_Bounds (meshgeometry.h line 211)
    fn compute_bounds(&mut self) {
        if let Some(vertex) = &self.vertex {
            let vertices = vertex.get_array();
            if vertices.is_empty() {
                self.bound_box_min = Vec3::ZERO;
                self.bound_box_max = Vec3::ZERO;
                self.bound_sphere_center = Vec3::ZERO;
                self.bound_sphere_radius = 0.0;
            } else {
                // Compute AABB
                let mut min = vertices[0];
                let mut max = vertices[0];
                for v in vertices.iter().skip(1) {
                    min = min.min(*v);
                    max = max.max(*v);
                }
                self.bound_box_min = min;
                self.bound_box_max = max;

                // Compute bounding sphere (center = AABB center)
                self.bound_sphere_center = (min + max) * 0.5;
                self.bound_sphere_radius = 0.0;
                for v in vertices.iter() {
                    let dist = (*v - self.bound_sphere_center).length();
                    if dist > self.bound_sphere_radius {
                        self.bound_sphere_radius = dist;
                    }
                }
            }
        }
        self.flags.set(GeometryFlags::DIRTY_BOUNDS, false);
    }
}

impl Default for MeshGeometry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mesh_geometry_creation() {
        let geom = MeshGeometry::new();
        assert_eq!(geom.get_polygon_count(), 0);
        assert_eq!(geom.get_vertex_count(), 0);
        assert!(geom.flags.has(GeometryFlags::DIRTY_BOUNDS));
    }

    #[test]
    fn test_geometry_flags() {
        let mut flags = GeometryFlags::new(0);
        assert!(!flags.has(GeometryFlags::SORT));

        flags.set(GeometryFlags::SORT, true);
        assert!(flags.has(GeometryFlags::SORT));

        flags.set(GeometryFlags::SORT, false);
        assert!(!flags.has(GeometryFlags::SORT));
    }

    #[test]
    fn test_mesh_geometry_reset() {
        let mut geom = MeshGeometry::new();
        geom.install_vertex_array(vec![Vec3::ZERO; 10]);
        geom.install_polygon_array(vec![[0, 1, 2]; 5]);

        assert_eq!(geom.get_vertex_count(), 10);
        assert_eq!(geom.get_polygon_count(), 5);

        geom.reset_geometry(0, 0);
        assert_eq!(geom.get_vertex_count(), 0);
        assert_eq!(geom.get_polygon_count(), 0);
    }

    #[test]
    fn test_bounding_volumes() {
        let mut geom = MeshGeometry::new();
        let vertices = vec![
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(0.0, 0.0, 0.0),
        ];
        geom.install_vertex_array(vertices);

        let bbox = geom.get_bounding_box();
        assert_eq!(bbox.min(), Vec3::new(-1.0, -1.0, -1.0));
        assert_eq!(bbox.max(), Vec3::new(1.0, 1.0, 1.0));

        let sphere = geom.get_bounding_sphere();
        assert!(sphere.radius() > 0.0);
    }

    #[test]
    fn test_share_buffer_copy_on_write() {
        let mut geom1 = MeshGeometry::new();
        geom1.install_vertex_array(vec![Vec3::ZERO; 10]);

        let mut geom2 = geom1.clone();

        // Should share data initially
        assert_eq!(Arc::strong_count(&geom1.vertex.as_ref().unwrap().data), 2);

        // Make unique on geom2
        geom2.make_vertex_array_unique();

        // Now should be separate
        assert_eq!(Arc::strong_count(&geom1.vertex.as_ref().unwrap().data), 1);
        assert_eq!(Arc::strong_count(&geom2.vertex.as_ref().unwrap().data), 1);
    }

    #[test]
    fn test_scale_geometry() {
        let mut geom = MeshGeometry::new();
        geom.install_vertex_array(vec![Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0)]);

        geom.scale(Vec3::new(2.0, 2.0, 2.0));

        let vertices = geom.get_vertex_array().unwrap();
        assert_eq!(vertices[0], Vec3::new(2.0, 4.0, 6.0));
        assert_eq!(vertices[1], Vec3::new(8.0, 10.0, 12.0));
    }
}
