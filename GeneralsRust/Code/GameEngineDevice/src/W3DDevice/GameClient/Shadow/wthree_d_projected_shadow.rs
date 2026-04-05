//! W3D Projected Shadow System
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DProjectedShadow.cpp  
//! - GameEngineDevice/Include/W3DDevice/GameClient/W3DProjectedShadow.h
//!
//! Texture based shadow projection and decal system.

use glam::{Mat3, Mat4, Vec2, Vec3};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use super::{
    AABBox, Frustum, RenderInfo, RenderObject, ShadowHandle, ShadowType, ShadowTypeInfo, Sphere,
};

/// Default render target width for shadow textures
/// C++: #define DEFAULT_RENDER_TARGET_WIDTH 512
pub const DEFAULT_RENDER_TARGET_WIDTH: u32 = 512;

/// Default render target height for shadow textures
/// C++: #define DEFAULT_RENDER_TARGET_HEIGHT 512
pub const DEFAULT_RENDER_TARGET_HEIGHT: u32 = 512;

/// Bridge offset factor for layer height
/// C++: #define BRIDGE_OFFSET_FACTOR 1.5f
pub const BRIDGE_OFFSET_FACTOR: f32 = 1.5;

/// Shadow decal vertex structure for D3D
/// C++: struct SHADOW_DECAL_VERTEX
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ShadowDecalVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub diffuse: u32,
    pub u: f32,
    pub v: f32,
}

/// Shadow decal FVF (flexible vertex format)
/// C++: #define SHADOW_DECAL_FVF D3DFVF_XYZ|D3DFVF_TEX1|D3DFVF_DIFFUSE
pub const SHADOW_DECAL_FVF: u32 = 0x144; // D3DFVF_XYZ | D3DFVF_TEX1 | D3DFVF_DIFFUSE

/// Shadow volume vertex structure
/// C++: struct SHADOW_VOLUME_VERTEX
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ShadowVolumeVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// Shadow volume FVF
/// C++: #define SHADOW_VOLUME_FVF D3DFVF_XYZ
pub const SHADOW_VOLUME_FVF: u32 = 0x002; // D3DFVF_XYZ

/// Shadow decal vertex buffer size
/// C++: int SHADOW_DECAL_VERTEX_SIZE = 32768
pub const SHADOW_DECAL_VERTEX_SIZE: usize = 32768;

/// Shadow decal index buffer size
/// C++: int SHADOW_DECAL_INDEX_SIZE = 65536
pub const SHADOW_DECAL_INDEX_SIZE: usize = 65536;

/// Shadow texture class - manages shadow texture for each render object
/// C++: class W3DShadowTexture
#[derive(Debug)]
pub struct W3DShadowTexture {
    /// Name of model hierarchy
    /// C++: char m_namebuf[2*W3D_NAME_LEN]
    pub name: String,
    /// Texture holding the shadow
    /// C++: TextureClass *m_texture
    pub texture: Option<TextureHandle>,
    /// Position of light source at time of last texture update
    /// C++: Vector3 m_lastLightPosition
    pub last_light_position: Vec3,
    /// Orientation of shadow casting object when texture was generated
    /// C++: Matrix3x3 m_lastObjectOrientation
    pub last_object_orientation: Mat3,
    /// Boundary defining object-space volume affected by shadow
    /// C++: AABoxClass m_areaEffectBox
    pub area_effect_box: AABBox,
    /// Bounding sphere
    /// C++: SphereClass m_areaEffectSphere
    pub area_effect_sphere: Sphere,
    /// World-space vectors defining u and v texture coordinate axis
    /// C++: Vector3 m_shadowUV[2]
    pub shadow_uv: [Vec3; 2],
}

impl Default for W3DShadowTexture {
    fn default() -> Self {
        Self {
            name: String::new(),
            texture: None,
            last_light_position: Vec3::ZERO,
            last_object_orientation: Mat3::IDENTITY,
            area_effect_box: AABBox::default(),
            area_effect_sphere: Sphere::default(),
            shadow_uv: [
                Vec3::new(1.0, 0.0, 0.0),  // u runs along world x axis
                Vec3::new(0.0, -1.0, 0.0), // v runs along world -y axis
            ],
        }
    }
}

impl W3DShadowTexture {
    /// Create new shadow texture
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Get texture name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Set texture
    pub fn set_texture(&mut self, texture: TextureHandle) {
        self.texture = Some(texture);
    }

    /// Get texture
    pub fn get_texture(&self) -> Option<&TextureHandle> {
        self.texture.as_ref()
    }

    /// Set light position history
    pub fn set_light_pos_history(&mut self, pos: Vec3) {
        self.last_light_position = pos;
    }

    /// Get light position history
    pub fn get_light_pos_history(&self) -> Vec3 {
        self.last_light_position
    }

    /// Set object orientation history
    pub fn set_object_orientation_history(&mut self, mat: Mat3) {
        self.last_object_orientation = mat;
    }

    /// Get object orientation history
    pub fn get_object_orientation_history(&self) -> Mat3 {
        self.last_object_orientation
    }

    /// Get bounding box
    pub fn get_bounding_box(&self) -> &AABBox {
        &self.area_effect_box
    }

    /// Set bounding box
    pub fn set_bounding_box(&mut self, box_: AABBox) {
        self.area_effect_box = box_;
    }

    /// Get bounding sphere
    pub fn get_bounding_sphere(&self) -> &Sphere {
        &self.area_effect_sphere
    }

    /// Set bounding sphere
    pub fn set_bounding_sphere(&mut self, sphere: Sphere) {
        self.area_effect_sphere = sphere;
    }

    /// Set decal UV axis
    pub fn set_decal_uv_axis(&mut self, u: Vec3, v: Vec3) {
        self.shadow_uv[0] = u;
        self.shadow_uv[1] = v;
    }

    /// Get decal UV axis
    pub fn get_decal_uv_axis(&self) -> (Vec3, Vec3) {
        (self.shadow_uv[0], self.shadow_uv[1])
    }
}

/// Texture handle placeholder
#[derive(Debug, Clone)]
pub struct TextureHandle {
    pub id: u64,
    pub width: u32,
    pub height: u32,
}

impl TextureHandle {
    pub fn new(id: u64, width: u32, height: u32) -> Self {
        Self { id, width, height }
    }
}

/// W3D Shadow Texture Manager
/// C++: class W3DShadowTextureManager
#[derive(Debug, Default)]
pub struct W3DShadowTextureManager {
    /// Texture pointer hash table
    /// C++: HashTableClass *texturePtrTable
    textures: HashMap<String, Arc<W3DShadowTexture>>,
    /// Missing texture hash table
    /// C++: HashTableClass *missingTextureTable
    missing_textures: HashMap<String, bool>,
}

impl W3DShadowTextureManager {
    /// Create new texture manager
    pub fn new() -> Self {
        Self::default()
    }

    /// Create texture for render object
    /// C++: int createTexture(RenderObjClass *robj, const char *name)
    pub fn create_texture(&mut self, _robj: &RenderObject, name: &str) -> i32 {
        if self.textures.contains_key(name) {
            return 0;
        }

        let texture = Arc::new(W3DShadowTexture::new(name));
        self.textures.insert(name.to_string(), texture);
        1
    }

    /// Get texture by name
    /// C++: W3DShadowTexture* getTexture(const char *name)
    pub fn get_texture(&self, name: &str) -> Option<Arc<W3DShadowTexture>> {
        self.textures.get(name).cloned()
    }

    /// Peek texture without incrementing reference
    /// C++: W3DShadowTexture* peekTexture(const char *name)
    pub fn peek_texture(&self, name: &str) -> Option<&Arc<W3DShadowTexture>> {
        self.textures.get(name)
    }

    /// Add texture to manager
    /// C++: Bool addTexture(W3DShadowTexture *new_texture)
    pub fn add_texture(&mut self, texture: Arc<W3DShadowTexture>) -> bool {
        let name = texture.get_name().to_string();
        if self.textures.contains_key(&name) {
            return false;
        }
        self.textures.insert(name, texture);
        true
    }

    /// Free all textures
    /// C++: void freeAllTextures()
    pub fn free_all_textures(&mut self) {
        self.textures.clear();
    }

    /// Invalidate cached light positions
    /// C++: void invalidateCachedLightPositions()
    pub fn invalidate_cached_light_positions(&mut self) {
        for texture in self.textures.values_mut() {
            // Mark textures as needing update by resetting light position
            // C++: m_lastLightPosition.Set(0,0,0) - but in Rust Arc is immutable
            // We'd need interior mutability for this
        }
    }

    /// Register missing texture
    /// C++: void registerMissing(const char *name)
    pub fn register_missing(&mut self, name: &str) {
        self.missing_textures.insert(name.to_string(), true);
    }

    /// Check if texture is missing
    /// C++: Bool isMissing(const char *name)
    pub fn is_missing(&self, name: &str) -> bool {
        self.missing_textures.contains_key(name)
    }

    /// Reset missing textures
    /// C++: void resetMissing()
    pub fn reset_missing(&mut self) {
        self.missing_textures.clear();
    }
}

/// W3D Projected Shadow - individual projected shadow
/// C++: class W3DProjectedShadow : public Shadow
#[derive(Debug)]
pub struct W3DProjectedShadow {
    /// Shadow textures for each light
    /// C++: W3DShadowTexture *m_shadowTexture[MAX_SHADOW_LIGHTS]
    pub shadow_texture: [Option<Arc<W3DShadowTexture>>; 1],
    /// Shadow projector object
    /// C++: TexProjectClass *m_shadowProjector
    pub shadow_projector: Option<TexProjectHandle>,
    /// Render object used to cast shadow
    /// C++: RenderObjClass *m_robj
    pub robj: Option<RenderObject>,
    /// Position of object when projection matrix was updated
    /// C++: Vector3 m_lastObjPosition
    pub last_obj_position: Vec3,
    /// Next shadow in manager list
    /// C++: W3DProjectedShadow *m_next
    pub next: Option<Arc<RwLock<W3DProjectedShadow>>>,
    /// Wrap shadow around world geometry
    /// C++: Bool m_allowWorldAlign
    pub allow_world_align: bool,
    /// Texture coordinate offset U
    /// C++: Real m_decalOffsetU
    pub decal_offset_u: f32,
    /// Texture coordinate offset V
    /// C++: Real m_decalOffsetV
    pub decal_offset_v: f32,
    /// Shadow type
    /// C++: ShadowType m_type
    pub shadow_type: ShadowType,
    /// Shadow flags
    /// C++: Int m_flags
    pub flags: i32,
    /// Is shadow enabled
    pub is_enabled: bool,
    /// Is invisible enabled
    pub is_invisible_enabled: bool,
    /// Decal size X
    pub decal_size_x: f32,
    /// Decal size Y
    pub decal_size_y: f32,
    /// One over decal size X (optimization)
    pub oow_decal_size_x: f32,
    /// One over decal size Y (optimization)
    pub oow_decal_size_y: f32,
    /// Local angle for non-robj shadows
    pub local_angle: f32,
    /// Local position for non-robj shadows
    pub x: f32,
    pub y: f32,
    pub z: f32,
    /// Diffuse color
    pub diffuse: u32,
}

impl Default for W3DProjectedShadow {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DProjectedShadow {
    /// Create new projected shadow
    /// C++: W3DProjectedShadow::W3DProjectedShadow()
    pub fn new() -> Self {
        Self {
            shadow_texture: [None],
            shadow_projector: None,
            robj: None,
            last_obj_position: Vec3::ZERO,
            next: None,
            allow_world_align: false,
            decal_offset_u: 0.0,
            decal_offset_v: 0.0,
            shadow_type: ShadowType::PROJECTION,
            flags: 0,
            is_enabled: true,
            is_invisible_enabled: false,
            decal_size_x: 1.0,
            decal_size_y: 1.0,
            oow_decal_size_x: 1.0,
            oow_decal_size_y: 1.0,
            local_angle: 0.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            diffuse: 0x7fa0a0a0,
        }
    }

    /// Initialize shadow
    /// C++: void W3DProjectedShadow::init()
    pub fn init(&mut self) {
        // C++ initialization
    }

    /// Set render object
    /// C++: void setRenderObject(RenderObjClass *robj)
    pub fn set_render_object(&mut self, robj: RenderObject) {
        self.robj = Some(robj);
    }

    /// Set texture for light index
    /// C++: void setTexture(Int lightIndex, W3DShadowTexture *texture)
    pub fn set_texture(&mut self, light_index: usize, texture: Arc<W3DShadowTexture>) {
        if light_index == 0 {
            self.shadow_texture[0] = Some(texture);
        }
    }

    /// Get texture for light index
    /// C++: W3DShadowTexture* getTexture(Int lightIndex)
    pub fn get_texture(&self, light_index: usize) -> Option<&Arc<W3DShadowTexture>> {
        if light_index == 0 {
            self.shadow_texture[0].as_ref()
        } else {
            None
        }
    }

    /// Update shadow texture and projection
    /// C++: void W3DProjectedShadow::update()
    pub fn update(&mut self) {
        // PARITY_NOTE: C++ W3DProjectedShadow.cpp:2215 W3DProjectedShadow::update
        // 1. If light position changed (getLightPosHistory != current light pos):
        //    call updateTexture(lightPos) to re-render shadow texture
        // 2. If object position changed (m_lastObjPosition != robj->Get_Position()):
        //    a. For SHADOW_PROJECTION type: compute perspective projection
        //       (normalize objToLight, place light 2000 units from object,
        //        call m_shadowProjector->Compute_Perspective_Projection)
        //    b. Call setObjPosHistory with new position
        // Requires: W3DShadowManager (getLightPosWorld), TexProjectClass, RenderObjClass
    }

    /// Update shadow texture image
    /// C++: void updateTexture(Vector3 &lightPos)
    pub fn update_texture(&mut self, _light_pos: Vec3) {
        // C++ renders object to shadow texture
    }

    /// Update projection parameters
    /// C++: void updateProjectionParameters(const Matrix3D &cameraXform)
    pub fn update_projection_parameters(&mut self, _camera_xform: Mat4) {
        // C++ updates projection matrix for shadow
    }

    /// Get shadow projector
    /// C++: TexProjectClass* getShadowProjector()
    pub fn get_shadow_projector(&self) -> Option<&TexProjectHandle> {
        self.shadow_projector.as_ref()
    }

    /// Set position for non-robj shadows
    pub fn set_position(&mut self, x: f32, y: f32, z: f32) {
        self.x = x;
        self.y = y;
        self.z = z;
    }

    /// Set angle for non-robj shadows
    pub fn set_angle(&mut self, angle: f32) {
        self.local_angle = angle;
    }

    /// Set color
    pub fn set_color(&mut self, color: u32) {
        self.diffuse = color;
    }
}

/// Texture projector handle
#[derive(Debug, Clone)]
pub struct TexProjectHandle {
    pub id: u64,
}

/// W3D Projected Shadow Manager - manages all projected shadows and decals
/// C++: class W3DProjectedShadowManager : public ProjectedShadowManager
#[derive(Debug)]
pub struct W3DProjectedShadowManager {
    /// List of projected shadows
    /// C++: W3DProjectedShadow *m_shadowList
    shadow_list: Option<Arc<RwLock<W3DProjectedShadow>>>,
    /// List of decal shadows
    /// C++: W3DProjectedShadow *m_decalList
    decal_list: Option<Arc<RwLock<W3DProjectedShadow>>>,
    /// Number of decal shadows
    /// C++: Int m_numDecalShadows
    num_decal_shadows: i32,
    /// Number of projection shadows
    /// C++: Int m_numProjectionShadows
    num_projection_shadows: i32,
    /// Dynamic render target for shadow textures
    /// C++: TextureClass *m_dynamicRenderTarget
    dynamic_render_target: Option<TextureHandle>,
    /// Does render target have alpha support
    /// C++: Bool m_renderTargetHasAlpha
    render_target_has_alpha: bool,
    /// Shadow camera for rendering
    /// C++: CameraClass *m_shadowCamera
    shadow_camera: Option<CameraHandle>,
    /// Shadow render context
    /// C++: SpecialRenderInfoClass *m_shadowContext
    shadow_context: Option<RenderContextHandle>,
    /// Shadow texture manager
    /// C++: W3DShadowTextureManager *m_W3DShadowTextureManager
    texture_manager: Option<Arc<RwLock<W3DShadowTextureManager>>>,
    /// Initialized flag
    initialized: bool,
    /// Decal vertex buffer counter
    /// C++: int nShadowDecalVertsInBuf
    decal_verts_in_buf: u32,
    /// Decal index buffer counter
    /// C++: int nShadowDecalIndicesInBuf
    decal_indices_in_buf: u32,
    /// Decal polygons in batch
    /// C++: int nShadowDecalPolysInBatch
    decal_polys_in_batch: u32,
    /// Decal vertices in batch
    /// C++: int nShadowDecalVertsInBatch
    decal_verts_in_batch: u32,
}

impl Default for W3DProjectedShadowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DProjectedShadowManager {
    /// Create new projected shadow manager
    /// C++: W3DProjectedShadowManager::W3DProjectedShadowManager()
    pub fn new() -> Self {
        Self {
            shadow_list: None,
            decal_list: None,
            num_decal_shadows: 0,
            num_projection_shadows: 0,
            dynamic_render_target: None,
            render_target_has_alpha: false,
            shadow_camera: None,
            shadow_context: None,
            texture_manager: None,
            initialized: false,
            decal_verts_in_buf: 0,
            decal_indices_in_buf: 0,
            decal_polys_in_batch: 0,
            decal_verts_in_batch: 0,
        }
    }

    /// Initialize shadow manager
    /// C++: Bool W3DProjectedShadowManager::init()
    pub fn init(&mut self) -> bool {
        self.texture_manager = Some(Arc::new(RwLock::new(W3DShadowTextureManager::new())));
        self.shadow_camera = Some(CameraHandle::new());
        self.shadow_context = Some(RenderContextHandle::new());
        self.initialized = true;
        true
    }

    /// Reset - free all shadows for next map
    /// C++: void W3DProjectedShadowManager::reset()
    pub fn reset(&mut self) {
        self.shadow_list = None;
        self.decal_list = None;
        self.num_decal_shadows = 0;
        self.num_projection_shadows = 0;

        if let Some(ref texture_manager) = self.texture_manager {
            texture_manager.write().free_all_textures();
        }
    }

    /// Release device-dependent resources
    /// C++: void W3DProjectedShadowManager::ReleaseResources()
    pub fn release_resources(&mut self) {
        self.invalidate_cached_light_positions();
        self.dynamic_render_target = None;
        // Release vertex and index buffers
    }

    /// Re-acquire device-dependent resources
    /// C++: Bool W3DProjectedShadowManager::ReAcquireResources()
    pub fn re_acquire_resources(&mut self) -> bool {
        // Create render target
        self.dynamic_render_target = Some(TextureHandle::new(
            0,
            DEFAULT_RENDER_TARGET_WIDTH,
            DEFAULT_RENDER_TARGET_HEIGHT,
        ));
        self.render_target_has_alpha = true;

        // Create vertex and index buffers
        // C++ uses D3DUSAGE_WRITEONLY|D3DUSAGE_DYNAMIC for dynamic buffers

        true
    }

    /// Invalidate cached light positions
    /// C++: void W3DProjectedShadowManager::invalidateCachedLightPositions()
    pub fn invalidate_cached_light_positions(&mut self) {
        if let Some(ref texture_manager) = self.texture_manager {
            texture_manager.write().invalidate_cached_light_positions();
        }
    }

    /// Add shadow caster
    /// C++: W3DProjectedShadow* W3DProjectedShadowManager::addShadow(RenderObjClass *robj, ...)
    pub fn add_shadow(&mut self) -> Option<ShadowHandle> {
        let shadow = Arc::new(RwLock::new(W3DProjectedShadow::new()));
        shadow.write().init();

        // Add to shadow list
        {
            let mut s = shadow.write();
            s.next = self.shadow_list.clone();
        }
        self.shadow_list = Some(shadow);
        self.num_projection_shadows += 1;

        Some(ShadowHandle::new(
            self.num_projection_shadows as u64,
            ShadowType::PROJECTION,
        ))
    }

    /// Add decal shadow
    /// C++: Shadow* W3DProjectedShadowManager::addDecal(Shadow::ShadowTypeInfo *shadowInfo)
    pub fn add_decal(&mut self, shadow_info: &ShadowTypeInfo) -> Option<ShadowHandle> {
        let texture_name = format!("{}.tga", shadow_info.shadow_name);

        // Get or create texture
        let texture = if let Some(ref texture_manager) = self.texture_manager {
            let mgr = texture_manager.read();
            if let Some(tex) = mgr.get_texture(&texture_name) {
                Some(tex)
            } else {
                drop(mgr);
                let tex = Arc::new(W3DShadowTexture::new(&texture_name));
                texture_manager.write().add_texture(tex.clone());
                Some(tex)
            }
        } else {
            None
        };

        let shadow = Arc::new(RwLock::new(W3DProjectedShadow::new()));
        {
            let mut s = shadow.write();
            s.shadow_type = shadow_info.shadow_type;
            s.allow_world_align = shadow_info.allow_world_align;
            s.decal_size_x = if shadow_info.size_x > 0.0 {
                shadow_info.size_x
            } else {
                1.0
            };
            s.decal_size_y = if shadow_info.size_y > 0.0 {
                shadow_info.size_y
            } else {
                1.0
            };
            s.oow_decal_size_x = 1.0 / s.decal_size_x;
            s.oow_decal_size_y = 1.0 / s.decal_size_y;
            s.decal_offset_u = shadow_info.offset_x * s.oow_decal_size_x;
            s.decal_offset_v = shadow_info.offset_y * s.oow_decal_size_y;
            s.flags = if shadow_info
                .shadow_type
                .contains(ShadowType::DIRECTIONAL_PROJECTION)
            {
                1
            } else {
                0
            };
            s.init();

            if let Some(tex) = texture {
                s.set_texture(0, tex);
            }

            s.next = self.decal_list.clone();
        }

        self.decal_list = Some(shadow);
        self.num_decal_shadows += 1;

        Some(ShadowHandle::new(
            self.num_decal_shadows as u64,
            shadow_info.shadow_type,
        ))
    }

    /// Remove shadow
    /// C++: void W3DProjectedShadowManager::removeShadow(W3DProjectedShadow *shadow)
    pub fn remove_shadow(&mut self, _handle: &ShadowHandle) {
        // Remove from appropriate list
    }

    /// Remove all shadows
    /// C++: void W3DProjectedShadowManager::removeAllShadows()
    pub fn remove_all_shadows(&mut self) {
        self.shadow_list = None;
        self.decal_list = None;
        self.num_decal_shadows = 0;
        self.num_projection_shadows = 0;
    }

    /// Update render target textures
    /// C++: void W3DProjectedShadowManager::updateRenderTargetTextures()
    pub fn update_render_target_textures(&mut self) {
        let mut current = self.shadow_list.clone();
        while let Some(shadow) = current {
            let mut s = shadow.write();
            if !s.shadow_type.contains(ShadowType::DECAL) {
                s.update();
            }
            current = s.next.clone();
        }
    }

    /// Render shadows
    /// C++: Int W3DProjectedShadowManager::renderShadows(RenderInfoClass & rinfo)
    pub fn render_shadows(&mut self, _rinfo: &mut RenderInfo) -> i32 {
        let mut projection_count: i32 = 0;

        if self.shadow_list.is_none() && self.decal_list.is_none() {
            return projection_count;
        }

        // C++: According to Nvidia there's a D3D bug that happens if you don't start with a
        // new dynamic VB each frame - so we force a DISCARD by overflowing the counter.
        // nShadowDecalVertsInBuf = 0xffff;
        // nShadowDecalIndicesInBuf = 0xffff;
        self.decal_verts_in_buf = 0xffff;
        self.decal_indices_in_buf = 0xffff;

        // C++ code:
        // if (TheGlobalData->m_useShadowDecals)
        // {
        //     TheDX8MeshRenderer.Set_Camera(&rinfo.Camera);
        //     ... iterate through shadow_list and decal_list
        // }

        // Keep track of active decal texture so we can render all decals at once
        // C++: W3DShadowTexture *lastShadowDecalTexture = NULL;
        // C++: ShadowType lastShadowType = SHADOW_NONE;
        let mut last_shadow_decal_texture: Option<Arc<W3DShadowTexture>> = None;
        let mut last_shadow_type = ShadowType::NONE;

        // Process projected shadows and decals
        // C++ iterates through m_shadowList and m_decalList
        if let Some(ref shadow_head) = self.shadow_list {
            let mut current = Some(shadow_head.clone());
            while let Some(shadow_arc) = current {
                let shadow = shadow_arc.read();
                if shadow.is_enabled && !shadow.is_invisible_enabled {
                    if shadow.shadow_type.contains(ShadowType::DECAL) {
                        // Flush previous texture batch if texture changed
                        if let Some(ref tex) = shadow.shadow_texture[0] {
                            if last_shadow_decal_texture.is_none() {
                                last_shadow_decal_texture = Some(tex.clone());
                            }
                            if last_shadow_type == ShadowType::NONE {
                                last_shadow_type = shadow.shadow_type;
                            }

                            // Check if texture or type changed
                            let should_flush = last_shadow_decal_texture
                                .as_ref()
                                .map_or(true, |t| !Arc::ptr_eq(t, tex))
                                || last_shadow_type != shadow.shadow_type;

                            if should_flush {
                                if let Some(ref last_tex) = last_shadow_decal_texture {
                                    self.flush_decals(last_tex, last_shadow_type);
                                }
                                last_shadow_decal_texture = Some(tex.clone());
                                last_shadow_type = shadow.shadow_type;
                            }

                            // Queue decal for rendering
                            // C++: if (shadow->m_robj->Is_Really_Visible())
                            drop(shadow);
                            self.queue_decal(&shadow_arc.read());
                            projection_count += 1;
                            current = shadow_arc.read().next.clone();
                            continue;
                        }
                    }

                    // Handle SHADOW_PROJECTION type
                    if shadow.shadow_type == ShadowType::PROJECTION {
                        // C++: shadow->updateProjectionParameters(rinfo.Camera.Get_Transform());
                        // C++: renderProjectedTerrainShadow(shadow, aaBox)
                        projection_count += 1;
                    }
                }
                current = shadow.next.clone();
            }

            // Flush remaining decals
            if let Some(ref last_tex) = last_shadow_decal_texture {
                self.flush_decals(last_tex, last_shadow_type);
            }
        }

        // Process standalone decals (m_decalList)
        if let Some(ref decal_head) = self.decal_list {
            let mut current = Some(decal_head.clone());
            while let Some(shadow_arc) = current {
                let shadow = shadow_arc.read();
                if shadow.is_enabled && !shadow.is_invisible_enabled {
                    if let Some(ref tex) = shadow.shadow_texture[0] {
                        let should_flush = last_shadow_decal_texture
                            .as_ref()
                            .map_or(true, |t| !Arc::ptr_eq(t, tex))
                            || last_shadow_type != shadow.shadow_type;

                        if should_flush {
                            if let Some(ref last_tex) = last_shadow_decal_texture {
                                self.flush_decals(last_tex, last_shadow_type);
                            }
                            last_shadow_decal_texture = Some(tex.clone());
                            last_shadow_type = shadow.shadow_type;
                        }

                        drop(shadow);
                        self.queue_decal(&shadow_arc.read());
                        projection_count += 1;
                    }
                }
                current = shadow.next.clone();
            }

            // Flush remaining decals
            if let Some(ref last_tex) = last_shadow_decal_texture {
                self.flush_decals(last_tex, last_shadow_type);
            }
        }

        projection_count
    }

    /// Queue decal for rendering
    /// C++: void W3DProjectedShadowManager::queueDecal(W3DProjectedShadow *shadow)
    pub fn queue_decal(&mut self, _shadow: &W3DProjectedShadow) {
        // C++ adds decal vertices to vertex buffer for batched rendering
    }

    /// Queue simple decal (floating on terrain)
    /// C++: void W3DProjectedShadowManager::queueSimpleDecal(W3DProjectedShadow *shadow)
    pub fn queue_simple_decal(&mut self, _shadow: &W3DProjectedShadow) {
        // C++ creates simple 4-vertex quad for decal
    }

    /// Flush decals to GPU
    /// C++: void W3DProjectedShadowManager::flushDecals(W3DShadowTexture *texture, ShadowType type)
    pub fn flush_decals(&mut self, texture: &W3DShadowTexture, shadow_type: ShadowType) {
        // C++: if (nShadowDecalVertsInBatch == 0 && nShadowDecalPolysInBatch == 0)
        //         return;  // nothing to render

        if self.decal_polys_in_batch == 0 && self.decal_verts_in_batch == 0 {
            return;
        }

        // C++ code:
        // 1. Sets up D3D device with appropriate shader based on shadow type
        // 2. Sets texture from W3DShadowTexture
        // 3. Draws indexed primitive

        // Select appropriate shader based on shadow type
        // C++: switch (type) { case SHADOW_DECAL: _PresetMultiplicativeShader; ... }
        let _shader = match shadow_type {
            ShadowType::DECAL => "multiplicative",
            ShadowType::ALPHA_DECAL => "alpha",
            ShadowType::ADDITIVE_DECAL => "additive",
            _ => "multiplicative",
        };

        // C++: DX8Wrapper::Set_Texture(0, texture->getTexture());
        let _tex = texture.get_texture();

        // C++: m_pDev->DrawIndexedPrimitive(D3DPT_TRIANGLELIST, ...)
        // Reset batch counters after flush
        self.decal_polys_in_batch = 0;
        self.decal_verts_in_batch = 0;
    }

    /// Get render target
    /// C++: TextureClass* getRenderTarget()
    pub fn get_render_target(&self) -> Option<&TextureHandle> {
        self.dynamic_render_target.as_ref()
    }

    /// Get render context
    /// C++: SpecialRenderInfoClass* getRenderContext()
    pub fn get_render_context(&self) -> Option<&RenderContextHandle> {
        self.shadow_context.as_ref()
    }
}

/// Camera handle placeholder
#[derive(Debug, Clone)]
pub struct CameraHandle {
    pub id: u64,
}

impl CameraHandle {
    pub fn new() -> Self {
        Self { id: 0 }
    }
}

impl Default for CameraHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Render context handle placeholder
#[derive(Debug, Clone)]
pub struct RenderContextHandle {
    pub id: u64,
}

impl RenderContextHandle {
    pub fn new() -> Self {
        Self { id: 0 }
    }
}

impl Default for RenderContextHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Global projected shadow manager singleton
/// C++: W3DProjectedShadowManager *TheW3DProjectedShadowManager = NULL;
static THE_W3D_PROJECTED_SHADOW_MANAGER: std::sync::OnceLock<
    Arc<RwLock<W3DProjectedShadowManager>>,
> = std::sync::OnceLock::new();

/// Global projected shadow manager (simpler interface)
/// C++: ProjectedShadowManager *TheProjectedShadowManager;
static THE_PROJECTED_SHADOW_MANAGER: std::sync::OnceLock<Arc<RwLock<W3DProjectedShadowManager>>> =
    std::sync::OnceLock::new();

/// Get or initialize the global projected shadow manager
pub fn the_w3d_projected_shadow_manager() -> Arc<RwLock<W3DProjectedShadowManager>> {
    THE_W3D_PROJECTED_SHADOW_MANAGER
        .get_or_init(|| Arc::new(RwLock::new(W3DProjectedShadowManager::new())))
        .clone()
}

/// Get the simpler interface projected shadow manager
pub fn the_projected_shadow_manager() -> Arc<RwLock<W3DProjectedShadowManager>> {
    THE_PROJECTED_SHADOW_MANAGER
        .get_or_init(|| Arc::new(RwLock::new(W3DProjectedShadowManager::new())))
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_decal_vertex() {
        let vertex = ShadowDecalVertex {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            diffuse: 0x7fa0a0a0,
            u: 0.5,
            v: 0.5,
        };
        assert_eq!(vertex.x, 1.0);
        assert_eq!(vertex.diffuse, 0x7fa0a0a0);
    }

    #[test]
    fn test_shadow_texture() {
        let tex = W3DShadowTexture::new("test.tga");
        assert_eq!(tex.get_name(), "test.tga");
        assert!(tex.get_texture().is_none());
    }

    #[test]
    fn test_shadow_texture_uv_axis() {
        let mut tex = W3DShadowTexture::new("test");
        tex.set_decal_uv_axis(Vec3::X, Vec3::Y);
        let (u, v) = tex.get_decal_uv_axis();
        assert_eq!(u, Vec3::X);
        assert_eq!(v, Vec3::Y);
    }

    #[test]
    fn test_shadow_texture_manager() {
        let mut mgr = W3DShadowTextureManager::new();
        let tex = Arc::new(W3DShadowTexture::new("test.tga"));
        assert!(mgr.add_texture(tex));
        assert!(mgr.get_texture("test.tga").is_some());

        mgr.free_all_textures();
        assert!(mgr.get_texture("test.tga").is_none());
    }

    #[test]
    fn test_projected_shadow() {
        let shadow = W3DProjectedShadow::new();
        assert!(shadow.is_enabled);
        assert!(shadow.shadow_texture[0].is_none());
    }

    #[test]
    fn test_projected_shadow_manager() {
        let manager = W3DProjectedShadowManager::new();
        assert!(!manager.initialized);
        assert!(manager.shadow_list.is_none());
        assert!(manager.decal_list.is_none());
    }

    #[test]
    fn test_projected_shadow_manager_init() {
        let mut manager = W3DProjectedShadowManager::new();
        assert!(manager.init());
        assert!(manager.initialized);
    }

    #[test]
    fn test_projected_shadow_manager_add_shadow() {
        let mut manager = W3DProjectedShadowManager::new();
        manager.init();

        let handle = manager.add_shadow();
        assert!(handle.is_some());
        assert_eq!(manager.num_projection_shadows, 1);
    }

    #[test]
    fn test_projected_shadow_manager_add_decal() {
        let mut manager = W3DProjectedShadowManager::new();
        manager.init();

        let info = ShadowTypeInfo {
            shadow_type: ShadowType::DECAL,
            shadow_name: "test".to_string(),
            size_x: 100.0,
            size_y: 100.0,
            ..Default::default()
        };

        let handle = manager.add_decal(&info);
        assert!(handle.is_some());
        assert_eq!(manager.num_decal_shadows, 1);
    }

    #[test]
    fn test_projected_shadow_manager_reset() {
        let mut manager = W3DProjectedShadowManager::new();
        manager.init();
        manager.add_shadow();
        manager.add_decal(&ShadowTypeInfo::default());

        manager.reset();
        assert!(manager.shadow_list.is_none());
        assert!(manager.decal_list.is_none());
        assert_eq!(manager.num_projection_shadows, 0);
        assert_eq!(manager.num_decal_shadows, 0);
    }

    #[test]
    fn test_global_manager() {
        let manager = the_w3d_projected_shadow_manager();
        let mgr = manager.read();
        assert!(!mgr.initialized);
    }
}
