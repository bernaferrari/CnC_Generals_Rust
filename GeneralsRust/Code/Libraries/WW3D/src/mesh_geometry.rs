// Mesh Geometry System
// Ported from meshgeometry.h and meshgeometry.cpp

use crate::math::*;
use crate::w3d_file::*;
use crate::{Result, W3DError};
use crate::collision::AABTree;
use std::io::Read;

// Geometry flags
bitflags::bitflags! {
    pub struct MeshGeometryFlags: u32 {
        const DIRTY_BOUNDS = 0x00000001;
        const DIRTY_PLANES = 0x00000002;
        const DIRTY_VNORMALS = 0x00000004;

        const SORT = 0x00000010;
        const DISABLE_BOUNDING_BOX = 0x00000020;
        const DISABLE_BOUNDING_SPHERE = 0x00000040;
        const DISABLE_PLANE_EQ = 0x00000080;
        const TWO_SIDED = 0x00000100;

        const ALIGNED = 0x00000200;
        const SKIN = 0x00000400;
        const ORIENTED = 0x00000800;
        const CAST_SHADOW = 0x00001000;

        const PRELIT_MASK = 0x0000E000;
        const PRELIT_VERTEX = 0x00002000;
        const PRELIT_LIGHTMAP_MULTI_PASS = 0x00004000;
        const PRELIT_LIGHTMAP_MULTI_TEXTURE = 0x00008000;

        const ALLOW_NPATCHES = 0x00010000;
    }
}

// Triangle index (using 16-bit indices for memory efficiency)
pub type TriIndex = Vector3i16;

// Mesh Geometry class - contains the raw geometry data
pub struct MeshGeometry {
    // General info
    pub mesh_name: String,
    pub user_text: String,
    pub flags: MeshGeometryFlags,
    pub sort_level: i8,
    pub w3d_attributes: u32,

    // Geometry data
    pub poly_count: usize,
    pub vertex_count: usize,

    // Arrays
    pub polygons: Vec<TriIndex>,
    pub vertices: Vec<Vec3>,
    pub vertex_normals: Option<Vec<Vec3>>,
    pub plane_equations: Option<Vec<Vec4>>,
    pub vertex_shade_indices: Option<Vec<u32>>,
    pub vertex_bone_links: Option<Vec<u16>>,
    pub poly_surface_types: Option<Vec<u8>>,

    // Bounding volumes
    pub bound_box_min: Vec3,
    pub bound_box_max: Vec3,
    pub bound_sphere_center: Vec3,
    pub bound_sphere_radius: f32,

    // Culling tree
    pub cull_tree: Option<AABTree>,
}

impl MeshGeometry {
    pub fn new() -> Self {
        Self {
            mesh_name: String::new(),
            user_text: String::new(),
            flags: MeshGeometryFlags::empty(),
            sort_level: 0,
            w3d_attributes: 0,
            poly_count: 0,
            vertex_count: 0,
            polygons: Vec::new(),
            vertices: Vec::new(),
            vertex_normals: None,
            plane_equations: None,
            vertex_shade_indices: None,
            vertex_bone_links: None,
            poly_surface_types: None,
            bound_box_min: Vec3::zeros(),
            bound_box_max: Vec3::zeros(),
            bound_sphere_center: Vec3::zeros(),
            bound_sphere_radius: 0.0,
            cull_tree: None,
        }
    }

    pub fn reset_geometry(&mut self, poly_count: usize, vertex_count: usize) {
        self.poly_count = poly_count;
        self.vertex_count = vertex_count;

        self.polygons = vec![TriIndex { x: 0, y: 0, z: 0 }; poly_count];
        self.vertices = vec![Vec3::zeros(); vertex_count];

        self.vertex_normals = None;
        self.plane_equations = None;
        self.vertex_shade_indices = None;
        self.vertex_bone_links = None;
        self.poly_surface_types = None;

        self.flags.insert(MeshGeometryFlags::DIRTY_BOUNDS);
        self.flags.insert(MeshGeometryFlags::DIRTY_PLANES);
        self.flags.insert(MeshGeometryFlags::DIRTY_VNORMALS);
    }

    pub fn get_name(&self) -> &str {
        &self.mesh_name
    }

    pub fn set_name(&mut self, name: String) {
        self.mesh_name = name;
    }

    pub fn get_user_text(&self) -> &str {
        &self.user_text
    }

    pub fn set_user_text(&mut self, text: String) {
        self.user_text = text;
    }

    pub fn get_polygon_count(&self) -> usize {
        self.poly_count
    }

    pub fn get_vertex_count(&self) -> usize {
        self.vertex_count
    }

    pub fn get_polygon_array(&self) -> &[TriIndex] {
        &self.polygons
    }

    pub fn get_vertex_array(&self) -> &[Vec3] {
        &self.vertices
    }

    pub fn get_vertex_array_mut(&mut self) -> &mut [Vec3] {
        &mut self.vertices
    }

    pub fn get_vertex_normal_array(&mut self) -> &[Vec3] {
        if self.vertex_normals.is_none() || self.flags.contains(MeshGeometryFlags::DIRTY_VNORMALS) {
            self.compute_vertex_normals();
        }
        self.vertex_normals.as_ref().unwrap()
    }

    pub fn get_plane_array(&mut self) -> &[Vec4] {
        if self.plane_equations.is_none() || self.flags.contains(MeshGeometryFlags::DIRTY_PLANES) {
            self.compute_plane_equations();
        }
        self.plane_equations.as_ref().unwrap()
    }

    pub fn compute_plane(&self, poly_idx: usize) -> Plane {
        let tri = &self.polygons[poly_idx];
        let v0 = &self.vertices[tri.x as usize];
        let v1 = &self.vertices[tri.y as usize];
        let v2 = &self.vertices[tri.z as usize];
        Plane::from_points(v0, v1, v2)
    }

    pub fn get_bounding_box(&mut self) -> AABox {
        if self.flags.contains(MeshGeometryFlags::DIRTY_BOUNDS) {
            self.compute_bounds();
        }
        AABox::new(self.bound_box_min, self.bound_box_max)
    }

    pub fn get_bounding_sphere(&mut self) -> Sphere {
        if self.flags.contains(MeshGeometryFlags::DIRTY_BOUNDS) {
            self.compute_bounds();
        }
        Sphere::new(self.bound_sphere_center, self.bound_sphere_radius)
    }

    pub fn has_cull_tree(&self) -> bool {
        self.cull_tree.is_some()
    }

    pub fn contains(&self, point: &Vec3) -> bool {
        // Ray casting algorithm for point-in-mesh test
        // Cast a ray from the point in an arbitrary direction and count intersections
        let ray = Ray::new(*point, Vec3::new(1.0, 0.0, 0.0));
        let mut intersection_count = 0;

        for i in 0..self.poly_count {
            let tri = &self.polygons[i];
            let v0 = &self.vertices[tri.x as usize];
            let v1 = &self.vertices[tri.y as usize];
            let v2 = &self.vertices[tri.z as usize];

            if self.ray_triangle_intersection(&ray, v0, v1, v2).is_some() {
                intersection_count += 1;
            }
        }

        // Odd number of intersections means inside
        (intersection_count % 2) == 1
    }

    pub fn scale(&mut self, scale: &Vec3) {
        for vertex in &mut self.vertices {
            vertex.x *= scale.x;
            vertex.y *= scale.y;
            vertex.z *= scale.z;
        }

        self.bound_box_min.component_mul_assign(scale);
        self.bound_box_max.component_mul_assign(scale);
        self.bound_sphere_center.component_mul_assign(scale);

        let max_scale = scale.x.max(scale.y).max(scale.z);
        self.bound_sphere_radius *= max_scale;

        self.flags.insert(MeshGeometryFlags::DIRTY_PLANES);
        self.flags.insert(MeshGeometryFlags::DIRTY_VNORMALS);
    }

    // W3D file loading
    pub fn load_w3d<R: Read>(&mut self, reader: &mut R) -> Result<()> {
        loop {
            let header = match W3DChunkHeader::read(reader) {
                Ok(h) => h,
                Err(_) => break,
            };

            match W3DChunkType::from_u32(header.chunk_type) {
                Some(W3DChunkType::Vertices) => self.read_vertices(reader, header.chunk_size)?,
                Some(W3DChunkType::VertexNormals) => self.read_vertex_normals(reader, header.chunk_size)?,
                Some(W3DChunkType::Triangles) => self.read_triangles(reader, header.chunk_size)?,
                Some(W3DChunkType::MeshUserText) => self.read_user_text(reader, header.chunk_size)?,
                Some(W3DChunkType::VertexInfluences) => self.read_vertex_influences(reader, header.chunk_size)?,
                Some(W3DChunkType::VertexShadeIndices) => self.read_vertex_shade_indices(reader, header.chunk_size)?,
                _ => {
                    // Skip unknown chunks
                    let mut buf = vec![0u8; header.chunk_size as usize];
                    reader.read_exact(&mut buf)?;
                }
            }
        }

        self.flags.insert(MeshGeometryFlags::DIRTY_BOUNDS);
        self.flags.insert(MeshGeometryFlags::DIRTY_PLANES);
        self.flags.insert(MeshGeometryFlags::DIRTY_VNORMALS);

        Ok(())
    }

    fn read_vertices<R: Read>(&mut self, reader: &mut R, size: u32) -> Result<()> {
        let count = size as usize / std::mem::size_of::<W3DVector3>();
        self.vertex_count = count;
        self.vertices = Vec::with_capacity(count);

        for _ in 0..count {
            let mut buf = [0u8; 12];
            reader.read_exact(&mut buf)?;
            let x = f32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
            let y = f32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
            let z = f32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
            self.vertices.push(Vec3::new(x, y, z));
        }

        Ok(())
    }

    fn read_vertex_normals<R: Read>(&mut self, reader: &mut R, size: u32) -> Result<()> {
        let count = size as usize / std::mem::size_of::<W3DVector3>();
        let mut normals = Vec::with_capacity(count);

        for _ in 0..count {
            let mut buf = [0u8; 12];
            reader.read_exact(&mut buf)?;
            let x = f32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
            let y = f32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
            let z = f32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
            normals.push(Vec3::new(x, y, z));
        }

        self.vertex_normals = Some(normals);
        self.flags.remove(MeshGeometryFlags::DIRTY_VNORMALS);

        Ok(())
    }

    fn read_triangles<R: Read>(&mut self, reader: &mut R, size: u32) -> Result<()> {
        let count = size as usize / std::mem::size_of::<W3DTriangle>();
        self.poly_count = count;
        self.polygons = Vec::with_capacity(count);

        for _ in 0..count {
            let mut buf = [0u8; std::mem::size_of::<W3DTriangle>()];
            reader.read_exact(&mut buf)?;

            let v0 = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as i16;
            let v1 = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as i16;
            let v2 = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]) as i16;

            self.polygons.push(TriIndex { x: v0, y: v1, z: v2 });
        }

        Ok(())
    }

    fn read_user_text<R: Read>(&mut self, reader: &mut R, size: u32) -> Result<()> {
        let mut buf = vec![0u8; size as usize];
        reader.read_exact(&mut buf)?;

        // Find null terminator
        if let Some(pos) = buf.iter().position(|&b| b == 0) {
            buf.truncate(pos);
        }

        self.user_text = String::from_utf8_lossy(&buf).to_string();
        Ok(())
    }

    fn read_vertex_influences<R: Read>(&mut self, reader: &mut R, size: u32) -> Result<()> {
        let count = size as usize / std::mem::size_of::<W3DVertexInfluence>();
        let mut bone_links = Vec::with_capacity(count);

        for _ in 0..count {
            let mut buf = [0u8; std::mem::size_of::<W3DVertexInfluence>()];
            reader.read_exact(&mut buf)?;
            let bone_idx = u16::from_le_bytes([buf[0], buf[1]]);
            bone_links.push(bone_idx);
        }

        self.vertex_bone_links = Some(bone_links);
        self.flags.insert(MeshGeometryFlags::SKIN);

        Ok(())
    }

    fn read_vertex_shade_indices<R: Read>(&mut self, reader: &mut R, size: u32) -> Result<()> {
        let count = size as usize / 4;
        let mut indices = Vec::with_capacity(count);

        for _ in 0..count {
            let mut buf = [0u8; 4];
            reader.read_exact(&mut buf)?;
            let idx = u32::from_le_bytes(buf);
            indices.push(idx);
        }

        self.vertex_shade_indices = Some(indices);
        Ok(())
    }

    // Internal computation methods
    fn compute_bounds(&mut self) {
        if self.vertices.is_empty() {
            self.bound_box_min = Vec3::zeros();
            self.bound_box_max = Vec3::zeros();
            self.bound_sphere_center = Vec3::zeros();
            self.bound_sphere_radius = 0.0;
            return;
        }

        // Compute bounding box
        let mut min = self.vertices[0];
        let mut max = self.vertices[0];

        for vertex in &self.vertices[1..] {
            min = Vec3::new(min.x.min(vertex.x), min.y.min(vertex.y), min.z.min(vertex.z));
            max = Vec3::new(max.x.max(vertex.x), max.y.max(vertex.y), max.z.max(vertex.z));
        }

        self.bound_box_min = min;
        self.bound_box_max = max;

        // Compute bounding sphere
        let center = (min + max) * 0.5;
        let mut radius = 0.0;

        for vertex in &self.vertices {
            let dist = (vertex - center).norm();
            if dist > radius {
                radius = dist;
            }
        }

        self.bound_sphere_center = center;
        self.bound_sphere_radius = radius;

        self.flags.remove(MeshGeometryFlags::DIRTY_BOUNDS);
    }

    fn compute_vertex_normals(&mut self) {
        let mut normals = vec![Vec3::zeros(); self.vertex_count];

        // Accumulate face normals
        for poly in &self.polygons {
            let v0 = &self.vertices[poly.x as usize];
            let v1 = &self.vertices[poly.y as usize];
            let v2 = &self.vertices[poly.z as usize];

            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            let normal = edge1.cross(&edge2);

            normals[poly.x as usize] += normal;
            normals[poly.y as usize] += normal;
            normals[poly.z as usize] += normal;
        }

        // Normalize
        for normal in &mut normals {
            let len = normal.norm();
            if len > 0.0001 {
                *normal /= len;
            }
        }

        self.vertex_normals = Some(normals);
        self.flags.remove(MeshGeometryFlags::DIRTY_VNORMALS);
    }

    fn compute_plane_equations(&mut self) {
        let mut planes = Vec::with_capacity(self.poly_count);

        for poly in &self.polygons {
            let v0 = &self.vertices[poly.x as usize];
            let v1 = &self.vertices[poly.y as usize];
            let v2 = &self.vertices[poly.z as usize];

            let plane = Plane::from_points(v0, v1, v2);
            planes.push(Vec4::new(plane.normal.x, plane.normal.y, plane.normal.z, plane.distance));
        }

        self.plane_equations = Some(planes);
        self.flags.remove(MeshGeometryFlags::DIRTY_PLANES);
    }

    // Ray-triangle intersection test (Möller-Trumbore algorithm)
    fn ray_triangle_intersection(&self, ray: &Ray, v0: &Vec3, v1: &Vec3, v2: &Vec3) -> Option<f32> {
        const EPSILON: f32 = 0.0000001;

        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let h = ray.direction.cross(&edge2);
        let a = edge1.dot(&h);

        if a > -EPSILON && a < EPSILON {
            return None; // Ray is parallel to triangle
        }

        let f = 1.0 / a;
        let s = ray.origin - v0;
        let u = f * s.dot(&h);

        if u < 0.0 || u > 1.0 {
            return None;
        }

        let q = s.cross(&edge1);
        let v = f * ray.direction.dot(&q);

        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = f * edge2.dot(&q);

        if t > EPSILON {
            Some(t)
        } else {
            None
        }
    }
}

impl Default for MeshGeometry {
    fn default() -> Self {
        Self::new()
    }
}
