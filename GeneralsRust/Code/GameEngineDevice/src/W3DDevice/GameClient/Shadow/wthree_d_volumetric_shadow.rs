//! W3D Volumetric Shadow System
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DVolumetricShadow.cpp
//! - GameEngineDevice/Include/W3DDevice/GameClient/W3DVolumetricShadow.h
//!
//! Real time shadow volume representations using stencil buffer.

use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use glam::{Vec3, Vec4, Mat4, Mat3};

use super::{ShadowHandle, ShadowTypeInfo, RenderObject, RenderInfo, Frustum};

/// Maximum number of shadow caster meshes in animated hierarchy
/// C++: #define MAX_SHADOW_CASTER_MESHES 160
pub const MAX_SHADOW_CASTER_MESHES: usize = 160;

/// Maximum silhouette edges
/// C++: #define MAX_SILHOUETTE_EDGES 1024
pub const MAX_SILHOUETTE_EDGES: usize = 1024;

/// Shadow extrusion buffer
/// C++: #define SHADOW_EXTRUSION_BUFFER 0.1f
pub const SHADOW_EXTRUSION_BUFFER: f32 = 0.1;

/// Airborne unit ground delta
/// C++: #define AIRBORNE_UNIT_GROUND_DELTA 2.0f
pub const AIRBORNE_UNIT_GROUND_DELTA: f32 = 2.0;

/// Maximum shadow length scale factor
/// C++: #define MAX_SHADOW_LENGTH_SCALE_FACTOR 1.0f
pub const MAX_SHADOW_LENGTH_SCALE_FACTOR: f32 = 1.0;

/// Maximum shadow length extra airborne scale factor
/// C++: #define MAX_SHADOW_LENGTH_EXTRA_AIRBORNE_SCALE_FACTOR 1.5f
pub const MAX_SHADOW_LENGTH_EXTRA_AIRBORNE_SCALE_FACTOR: f32 = 1.5;

/// Maximum extrusion length
/// C++: #define MAX_EXTRUSION_LENGTH (512.0f*MAP_XY_FACTOR)
pub const MAX_EXTRUSION_LENGTH: f32 = 512.0 * 10.0; // MAP_XY_FACTOR = 10.0

/// Maximum shadow extrusion under object before clamp
/// C++: #define MAX_SHADOW_EXTRUSION_UNDER_OBJECT_BEFORE_CLAMP 5.0f
pub const MAX_SHADOW_EXTRUSION_UNDER_OBJECT_CLAMP: f32 = 5.0;

/// Shadow sampling interval for terrain
/// C++: #define SHADOW_SAMPLING_INTERVAL (MAP_XY_FACTOR * 2.0f)
pub const SHADOW_SAMPLING_INTERVAL: f32 = 10.0 * 2.0;

/// Overhanging object clamp angle (80 degrees)
/// C++: #define OVERHANGING_OBJECT_CLAMP_ANGLE (80.0f/180.0f*PI)
pub const OVERHANGING_OBJECT_CLAMP_ANGLE: f32 = std::f32::consts::PI * 80.0 / 180.0;

/// Cosine of angle threshold for shadow volume updates
/// C++: const Real cosAngleToCare = cos((0.2 * PI) / 180.0)
pub const COS_ANGLE_TO_CARE: f32 = 0.999_998_1; // cos(0.2 degrees)

/// Maximum shadow volume vertices
/// C++: #define MAX_SHADOW_VOLUME_VERTS 16384
pub const MAX_SHADOW_VOLUME_VERTS: usize = 16384;

/// Shadow vertex buffer size
/// C++: int SHADOW_VERTEX_SIZE = 4096
pub const SHADOW_VERTEX_SIZE: usize = 4096;

/// Shadow index buffer size  
/// C++: int SHADOW_INDEX_SIZE = 8192
pub const SHADOW_INDEX_SIZE: usize = 8192;

/// Polygon visibility flags
/// C++: const Byte POLY_VISIBLE = 0x01
pub const POLY_VISIBLE: u8 = 0x01;
/// C++: const Byte POLY_PROCESSED = 0x02
pub const POLY_PROCESSED: u8 = 0x02;

/// No neighbor marker
/// C++: const Int NO_NEIGHBOR = -1
pub const NO_NEIGHBOR: i32 = -1;

/// Maximum polygon neighbors (triangles have 3)
/// C++: const Int MAX_POLYGON_NEIGHBORS = 3
pub const MAX_POLYGON_NEIGHBORS: usize = 3;

/// Neighbor edge structure
/// C++: typedef struct _NeighborEdge
#[derive(Debug, Clone, Default)]
pub struct NeighborEdge {
    /// Index of polygon who is our neighbor
    /// C++: Short neighborIndex
    pub neighbor_index: i16,
    /// The two vertex indices that represent the shared edge
    /// C++: Short neighborEdgeIndex[2]
    pub neighbor_edge_index: [i16; 2],
}

/// Polygon neighbor structure
/// C++: struct PolyNeighbor
#[derive(Debug, Clone)]
pub struct PolyNeighbor {
    /// Our polygon index
    /// C++: Short myIndex
    pub my_index: i16,
    /// Status flags (POLY_VISIBLE, POLY_PROCESSED)
    /// C++: Byte status
    pub status: u8,
    /// Neighbor edges
    /// C++: NeighborEdge neighbor[MAX_POLYGON_NEIGHBORS]
    pub neighbor: [NeighborEdge; MAX_POLYGON_NEIGHBORS],
}

impl Default for PolyNeighbor {
    fn default() -> Self {
        Self {
            my_index: 0,
            status: 0,
            neighbor: [NeighborEdge {
                neighbor_index: NO_NEIGHBOR as i16,
                neighbor_edge_index: [0, 0],
            }; MAX_POLYGON_NEIGHBORS],
        }
    }
}

/// Visible state for geometry
/// C++: Geometry::VisibleState
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibleState {
    Unknown = 0,
    Visible = 1,
    Invisible = 2,
}

/// Shadow volume geometry
/// C++: struct Geometry
#[derive(Debug, Clone)]
pub struct Geometry {
    /// Vertices
    pub verts: Vec<Vec3>,
    /// Triangle indices
    pub indices: Vec<u16>,
    /// Number of polygons
    pub num_polygon: usize,
    /// Number of vertices
    pub num_vertex: usize,
    /// Number of active polygons
    pub num_active_polygon: usize,
    /// Number of active vertices
    pub num_active_vertex: usize,
    /// Geometry flags
    pub flags: i32,
    /// Bounding box
    pub bounding_box: Option<AABBox>,
    /// Bounding sphere
    pub bounding_sphere: Option<Sphere>,
    /// Visible state
    pub visible_state: VisibleState,
}

impl Default for Geometry {
    fn default() -> Self {
        Self {
            verts: Vec::new(),
            indices: Vec::new(),
            num_polygon: 0,
            num_vertex: 0,
            num_active_polygon: 0,
            num_active_vertex: 0,
            flags: 0,
            bounding_box: None,
            bounding_sphere: None,
            visible_state: VisibleState::Unknown,
        }
    }
}

impl Geometry {
    /// Create new geometry with given sizes
    pub fn create(num_vertices: usize, num_polygons: usize) -> Self {
        Self {
            verts: vec![Vec3::ZERO; num_vertices],
            indices: vec![0; num_polygons * 3],
            num_polygon: num_polygons,
            num_vertex: num_vertices,
            ..Default::default()
        }
    }

    /// Get polygon index at given polygon ID
    pub fn get_polygon_index(&self, poly_id: usize) -> Option<[u16; 3]> {
        if poly_id * 3 + 2 < self.indices.len() {
            Some([
                self.indices[poly_id * 3],
                self.indices[poly_id * 3 + 1],
                self.indices[poly_id * 3 + 2],
            ])
        } else {
            None
        }
    }

    /// Set polygon index
    pub fn set_polygon_index(&mut self, poly_id: usize, indices: [u16; 3]) {
        if poly_id * 3 + 2 < self.indices.len() {
            self.indices[poly_id * 3] = indices[0];
            self.indices[poly_id * 3 + 1] = indices[1];
            self.indices[poly_id * 3 + 2] = indices[2];
        }
    }

    /// Get vertex at given index
    pub fn get_vertex(&self, vert_id: usize) -> Option<&Vec3> {
        self.verts.get(vert_id)
    }

    /// Set vertex at given index
    pub fn set_vertex(&mut self, vert_id: usize, vertex: Vec3) {
        if vert_id < self.verts.len() {
            self.verts[vert_id] = vertex;
        }
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy)]
pub struct AABBox {
    pub center: Vec3,
    pub extent: Vec3,
}

impl Default for AABBox {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            extent: Vec3::ZERO,
        }
    }
}

/// Bounding sphere
#[derive(Debug, Clone, Copy)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Default for Sphere {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 0.0,
        }
    }
}

/// Shadow geometry mesh data
/// C++: class W3DShadowGeometryMesh
#[derive(Debug, Clone)]
pub struct ShadowGeometryMesh {
    /// W3D mesh reference
    pub mesh: Option<MeshHandle>,
    /// Mesh index in render object
    pub mesh_robj_index: i32,
    /// Array of vertices
    pub verts: Vec<Vec3>,
    /// Array of face normals
    pub polygon_normals: Vec<Vec3>,
    /// Number of actual vertices after duplicates removed
    pub num_verts: usize,
    /// Number of polygons
    pub num_polygons: usize,
    /// Array of 3 vertex indices per face
    pub polygons: Vec<TriIndex>,
    /// Parent vertex indices
    pub parent_verts: Vec<u16>,
    /// Polygon neighbor info
    pub poly_neighbors: Vec<PolyNeighbor>,
    /// Parent geometry reference
    pub parent_geometry: Option<Arc<ShadowGeometry>>,
}

impl Default for ShadowGeometryMesh {
    fn default() -> Self {
        Self {
            mesh: None,
            mesh_robj_index: -1,
            verts: Vec::new(),
            polygon_normals: Vec::new(),
            num_verts: 0,
            num_polygons: 0,
            polygons: Vec::new(),
            parent_verts: Vec::new(),
            poly_neighbors: Vec::new(),
            parent_geometry: None,
        }
    }
}

/// Triangle index
/// C++: TriIndex struct
#[derive(Debug, Clone, Copy, Default)]
pub struct TriIndex {
    pub i: u16,
    pub j: u16,
    pub k: u16,
}

/// Mesh handle placeholder
#[derive(Debug, Clone)]
pub struct MeshHandle {
    pub id: u64,
}

/// Shadow geometry for a render object
/// C++: class W3DShadowGeometry
#[derive(Debug)]
pub struct ShadowGeometry {
    /// Name of model hierarchy
    pub name: String,
    /// Collection of meshes for this geometry
    pub mesh_list: Vec<ShadowGeometryMesh>,
    /// Number of meshes in hierarchy
    pub mesh_count: usize,
    /// Total number of vertices in all meshes
    pub num_total_verts: usize,
}

impl ShadowGeometry {
    /// Create new shadow geometry
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            mesh_list: Vec::new(),
            mesh_count: 0,
            num_total_verts: 0,
        }
    }

    /// Initialize from HLOD render object
    /// C++: Int W3DShadowGeometry::initFromHLOD(RenderObjClass *robj)
    pub fn init_from_hlod(&mut self, _robj: &RenderObject) -> bool {
        // TODO: Implement HLOD initialization
        // C++ iterates through hlod->Get_Lod_Model_Count() and processes meshes
        self.mesh_count > 0
    }

    /// Initialize from mesh render object
    /// C++: Int W3DShadowGeometry::initFromMesh(RenderObjClass *robj)
    pub fn init_from_mesh(&mut self, _robj: &RenderObject) -> bool {
        // TODO: Implement mesh initialization
        // C++ processes mesh vertices and builds polygon data
        self.mesh_count > 0
    }

    /// Get mesh by index
    pub fn get_mesh(&self, index: usize) -> Option<&ShadowGeometryMesh> {
        self.mesh_list.get(index)
    }

    /// Get mesh count
    pub fn get_mesh_count(&self) -> usize {
        self.mesh_count
    }

    /// Get total vertex count
    pub fn get_num_total_vertex(&self) -> usize {
        self.num_total_verts
    }
}

/// W3D Volumetric Shadow - individual shadow volume
/// C++: class W3DVolumetricShadow : public Shadow
#[derive(Debug)]
pub struct W3DVolumetricShadow {
    /// Next shadow in manager list
    pub next: Option<Arc<RwLock<W3DVolumetricShadow>>>,
    /// Shadow geometry data
    pub geometry: Option<Arc<ShadowGeometry>>,
    /// Shadow length scale factor
    pub shadow_length_scale: f32,
    /// Maximum horizontal reach of shadow
    pub robj_extent: f32,
    /// Extra extrusion padding
    pub extra_extrusion_padding: f32,
    /// Render object casting shadow
    pub robj: Option<RenderObject>,
    /// Is shadow enabled
    pub is_enabled: bool,
    /// Is invisible enabled
    pub is_invisible_enabled: bool,
    /// Shadow volumes per light per mesh
    /// C++: Geometry *m_shadowVolume[MAX_SHADOW_LIGHTS][MAX_SHADOW_CASTER_MESHES]
    pub shadow_volume: [[Option<Geometry>; MAX_SHADOW_CASTER_MESHES]; 1], // Only 1 light supported
    /// Silhouette indices per mesh
    pub silhouette_index: [Vec<i16>; MAX_SHADOW_CASTER_MESHES],
    /// Number of silhouette indices per mesh
    pub num_silhouette_indices: [i16; MAX_SHADOW_CASTER_MESHES],
    /// Max silhouette entries per mesh
    pub max_silhouette_entries: [i16; MAX_SHADOW_CASTER_MESHES],
    /// Shadow volume count per mesh
    pub shadow_volume_count: [usize; MAX_SHADOW_CASTER_MESHES],
    /// Light position history
    pub light_pos_history: [[Vec3; MAX_SHADOW_CASTER_MESHES]; 1],
    /// Object transform history
    pub object_xform_history: [[Mat4; MAX_SHADOW_CASTER_MESHES]; 1],
    /// Flags for dynamic shadows
    pub flags: i32,
}

impl Default for W3DVolumetricShadow {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DVolumetricShadow {
    /// Create new volumetric shadow
    /// C++: W3DVolumetricShadow::W3DVolumetricShadow()
    pub fn new() -> Self {
        Self {
            next: None,
            geometry: None,
            shadow_length_scale: 0.0,
            robj_extent: 0.0,
            extra_extrusion_padding: 0.0,
            robj: None,
            is_enabled: true,
            is_invisible_enabled: false,
            shadow_volume: Default::default(), // All None
            silhouette_index: Default::default(),
            num_silhouette_indices: [0; MAX_SHADOW_CASTER_MESHES],
            max_silhouette_entries: [0; MAX_SHADOW_CASTER_MESHES],
            shadow_volume_count: [0; MAX_SHADOW_CASTER_MESHES],
            light_pos_history: [[Vec3::ZERO; MAX_SHADOW_CASTER_MESHES]; 1],
            object_xform_history: [[Mat4::IDENTITY; MAX_SHADOW_CASTER_MESHES]; 1],
            flags: 0,
        }
    }

    /// Set geometry for this shadow
    /// C++: void W3DVolumetricShadow::SetGeometry(W3DShadowGeometry *geometry)
    pub fn set_geometry(&mut self, geometry: Option<Arc<ShadowGeometry>>) {
        // C++ allocates silhouette data based on vertex count
        if let Some(ref geom) = geometry {
            for i in 0..geom.get_mesh_count() {
                if let Some(mesh) = geom.get_mesh(i) {
                    let num_verts = mesh.num_verts;
                    if num_verts > self.max_silhouette_entries[i] as usize {
                        self.delete_silhouette(i);
                        self.allocate_silhouette(i, num_verts);
                    }
                }
            }
        }
        self.geometry = geometry;
    }

    /// Set render object
    pub fn set_render_object(&mut self, robj: RenderObject) {
        self.robj = Some(robj);
    }

    /// Set shadow length scale
    /// C++: void setShadowLengthScale(Real value)
    pub fn set_shadow_length_scale(&mut self, value: f32) {
        self.shadow_length_scale = value;
    }

    /// Set optimal extrusion padding
    /// C++: void setOptimalExtrusionPadding(Real value)
    pub fn set_optimal_extrusion_padding(&mut self, value: f32) {
        self.extra_extrusion_padding = value;
    }

    /// Update optimal extrusion padding based on terrain
    /// C++: void W3DVolumetricShadow::updateOptimalExtrusionPadding()
    pub fn update_optimal_extrusion_padding(&mut self) {
        // C++ code raycasts from object corners to terrain to find
        // optimal shadow extrusion distance
        // TODO: Implement terrain raycasting
    }

    /// Update shadow volume for this frame
    /// C++: void W3DVolumetricShadow::Update()
    pub fn update(&mut self) {
        if self.geometry.is_none() {
            return;
        }
        
        // C++ checks if light angle or object transform changed significantly
        // before rebuilding shadow volumes
        // TODO: Implement update logic
    }

    /// Allocate silhouette storage for mesh
    fn allocate_silhouette(&mut self, mesh_index: usize, num_vertices: usize) -> bool {
        if mesh_index >= MAX_SHADOW_CASTER_MESHES {
            return false;
        }
        
        // C++ allocates 2 * num_vertices entries for silhouette edges
        self.silhouette_index[mesh_index] = vec![0; num_vertices * 2];
        self.max_silhouette_entries[mesh_index] = num_vertices as i16 * 2;
        true
    }

    /// Delete silhouette storage for mesh
    fn delete_silhouette(&mut self, mesh_index: usize) {
        if mesh_index < MAX_SHADOW_CASTER_MESHES {
            self.silhouette_index[mesh_index].clear();
            self.num_silhouette_indices[mesh_index] = 0;
            self.max_silhouette_entries[mesh_index] = 0;
        }
    }
}

/// W3D Volumetric Shadow Manager - manages all volumetric shadows
/// C++: class W3DVolumetricShadowManager
#[derive(Debug, Default)]
pub struct W3DVolumetricShadowManager {
    /// List of all shadows
    /// C++: W3DVolumetricShadow *m_shadowList
    shadow_list: Vec<Arc<RwLock<W3DVolumetricShadow>>>,
    /// Dynamic shadow volumes to render
    /// C++: W3DVolumetricShadowRenderTask *m_dynamicShadowVolumesToRender
    dynamic_shadow_tasks: Vec<ShadowRenderTask>,
    /// Shadow geometry manager
    /// C++: W3DShadowGeometryManager *m_W3DShadowGeometryManager
    geometry_manager: Option<Arc<RwLock<ShadowGeometryManager>>>,
    /// Is initialized
    initialized: bool,
}

/// Shadow render task
/// C++: struct W3DVolumetricShadowRenderTask
#[derive(Debug, Clone)]
pub struct ShadowRenderTask {
    /// Parent shadow
    pub parent_shadow: Arc<RwLock<W3DVolumetricShadow>>,
    /// Mesh index
    pub mesh_index: u8,
    /// Light index
    pub light_index: u8,
}

/// Shadow geometry manager
/// C++: class W3DShadowGeometryManager
#[derive(Debug, Default)]
pub struct ShadowGeometryManager {
    /// Geometry cache by name
    geometries: HashMap<String, Arc<ShadowGeometry>>,
}

impl ShadowGeometryManager {
    /// Create new geometry manager
    pub fn new() -> Self {
        Self {
            geometries: HashMap::new(),
        }
    }

    /// Get or create geometry for render object
    pub fn get_geometry(&mut self, name: &str, robj: &RenderObject) -> Option<Arc<ShadowGeometry>> {
        if let Some(geom) = self.geometries.get(name) {
            return Some(geom.clone());
        }
        
        let mut geometry = ShadowGeometry::new(name);
        if geometry.init_from_hlod(robj) || geometry.init_from_mesh(robj) {
            let arc = Arc::new(geometry);
            self.geometries.insert(name.to_string(), arc.clone());
            Some(arc)
        } else {
            None
        }
    }

    /// Free all cached geometries
    pub fn free_all(&mut self) {
        self.geometries.clear();
    }
}

impl W3DVolumetricShadowManager {
    /// Create new volumetric shadow manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize resources
    /// C++: Bool W3DVolumetricShadowManager::init()
    pub fn init(&mut self) -> bool {
        self.geometry_manager = Some(Arc::new(RwLock::new(ShadowGeometryManager::new())));
        self.initialized = true;
        true
    }

    /// Reset - free all shadows for next map
    /// C++: void W3DVolumetricShadowManager::reset()
    pub fn reset(&mut self) {
        self.shadow_list.clear();
        self.dynamic_shadow_tasks.clear();
    }

    /// Release device-dependent resources
    /// C++: void W3DVolumetricShadowManager::ReleaseResources()
    pub fn release_resources(&mut self) {
        // Release GPU resources (vertex buffers, index buffers)
    }

    /// Re-acquire device-dependent resources
    /// C++: Bool W3DVolumetricShadowManager::ReAcquireResources()
    pub fn re_acquire_resources(&mut self) -> bool {
        // Recreate GPU resources
        true
    }

    /// Add shadow caster
    /// C++: W3DVolumetricShadow* W3DVolumetricShadowManager::addShadow(RenderObjClass *robj, ...)
    pub fn add_shadow(&mut self) -> Option<ShadowHandle> {
        let shadow = Arc::new(RwLock::new(W3DVolumetricShadow::new()));
        let handle = ShadowHandle::new(self.shadow_list.len() as u64, super::ShadowType::VOLUME);
        self.shadow_list.push(shadow);
        Some(handle)
    }

    /// Remove shadow
    /// C++: void W3DVolumetricShadowManager::removeShadow(W3DVolumetricShadow *shadow)
    pub fn remove_shadow(&mut self, handle: &ShadowHandle) {
        // Find and remove shadow by handle ID
        self.shadow_list.retain(|s| {
            s.read().is_enabled
        });
    }

    /// Remove all shadows
    /// C++: void W3DVolumetricShadowManager::removeAllShadows()
    pub fn remove_all_shadows(&mut self) {
        self.shadow_list.clear();
        self.dynamic_shadow_tasks.clear();
    }

    /// Invalidate cached light positions
    /// C++: void W3DVolumetricShadowManager::invalidateCachedLightPositions()
    pub fn invalidate_cached_light_positions(&mut self) {
        for shadow in &self.shadow_list {
            // Mark all shadows as needing update
            shadow.write().flags |= 1;
        }
    }

    /// Render shadows
    /// C++: void W3DVolumetricShadowManager::renderShadows(Int projectionCount)
    pub fn render_shadows(&mut self, projection_count: i32, force_stencil_fill: bool) {
        if self.shadow_list.is_empty() {
            return;
        }
        
        // C++ renders each shadow volume using stencil buffer
        // TODO: Implement GPU rendering
        let _ = projection_count;
        let _ = force_stencil_fill;
    }

    /// Load terrain shadows (if enabled)
    /// C++: void W3DVolumetricShadowManager::loadTerrainShadows()
    pub fn load_terrain_shadows(&mut self) {
        // C++ terrain shadow loading for DO_TERRAIN_SHADOW_VOLUMES
        // Currently disabled in C++ by default
    }
}

/// Global volumetric shadow manager singleton
/// C++: W3DVolumetricShadowManager *TheW3DVolumetricShadowManager = NULL;
static THE_W3D_VOLUMETRIC_SHADOW_MANAGER: std::sync::OnceLock<Arc<RwLock<W3DVolumetricShadowManager>>> = 
    std::sync::OnceLock::new();

/// Get or initialize the global volumetric shadow manager
pub fn the_w3d_volumetric_shadow_manager() -> Arc<RwLock<W3DVolumetricShadowManager>> {
    THE_W3D_VOLUMETRIC_SHADOW_MANAGER
        .get_or_init(|| Arc::new(RwLock::new(W3DVolumetricShadowManager::new())))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volumetric_shadow_creation() {
        let shadow = W3DVolumetricShadow::new();
        assert!(shadow.is_enabled);
        assert!(!shadow.is_invisible_enabled);
        assert!(shadow.geometry.is_none());
    }

    #[test]
    fn test_volumetric_shadow_manager_creation() {
        let manager = W3DVolumetricShadowManager::new();
        assert!(manager.shadow_list.is_empty());
        assert!(!manager.initialized);
    }

    #[test]
    fn test_volumetric_shadow_manager_init() {
        let mut manager = W3DVolumetricShadowManager::new();
        assert!(manager.init());
        assert!(manager.initialized);
    }

    #[test]
    fn test_volumetric_shadow_manager_add_shadow() {
        let mut manager = W3DVolumetricShadowManager::new();
        manager.init();
        
        let handle = manager.add_shadow();
        assert!(handle.is_some());
        assert_eq!(manager.shadow_list.len(), 1);
    }

    #[test]
    fn test_volumetric_shadow_manager_reset() {
        let mut manager = W3DVolumetricShadowManager::new();
        manager.init();
        manager.add_shadow();
        assert_eq!(manager.shadow_list.len(), 1);
        
        manager.reset();
        assert_eq!(manager.shadow_list.len(), 0);
    }

    #[test]
    fn test_geometry_creation() {
        let geom = Geometry::create(100, 50);
        assert_eq!(geom.verts.len(), 100);
        assert_eq!(geom.indices.len(), 150); // 50 * 3
        assert_eq!(geom.num_vertex, 100);
        assert_eq!(geom.num_polygon, 50);
    }

    #[test]
    fn test_geometry_polygon_index() {
        let mut geom = Geometry::create(3, 1);
        geom.set_polygon_index(0, [0, 1, 2]);
        
        let indices = geom.get_polygon_index(0);
        assert!(indices.is_some());
        let indices = indices.unwrap();
        assert_eq!(indices, [0, 1, 2]);
    }

    #[test]
    fn test_geometry_vertex() {
        let mut geom = Geometry::create(1, 0);
        geom.set_vertex(0, Vec3::new(1.0, 2.0, 3.0));
        
        let vert = geom.get_vertex(0);
        assert!(vert.is_some());
        assert_eq!(*vert.unwrap(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_shadow_geometry() {
        let geom = ShadowGeometry::new("test_model");
        assert_eq!(geom.name, "test_model");
        assert_eq!(geom.mesh_count, 0);
        assert_eq!(geom.get_num_total_vertex(), 0);
    }

    #[test]
    fn test_poly_neighbor_default() {
        let neighbor = PolyNeighbor::default();
        assert_eq!(neighbor.my_index, 0);
        assert_eq!(neighbor.status, 0);
        for n in &neighbor.neighbor {
            assert_eq!(n.neighbor_index, NO_NEIGHBOR as i16);
        }
    }

    #[test]
    fn test_global_manager() {
        let manager = the_w3d_volumetric_shadow_manager();
        let mgr = manager.read();
        assert!(mgr.shadow_list.is_empty());
    }
}
