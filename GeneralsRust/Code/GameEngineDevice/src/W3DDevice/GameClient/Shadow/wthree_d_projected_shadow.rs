//! W3D Projected Shadow System
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DProjectedShadow.cpp  
//! - GameEngineDevice/Include/W3DDevice/GameClient/W3DProjectedShadow.h
//!
//! Texture based shadow projection and decal system.

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec2, Vec3};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use wgpu::{
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindingType, Buffer, BufferDescriptor, BufferUsages,
    CommandEncoder, CompareFunction, Device, Extent3d, FilterMode, FragmentState, FrontFace,
    LoadOp, Operations, Origin3d, PipelineLayoutDescriptor, PrimitiveState, PrimitiveTopology,
    Queue, RenderPass, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderStages,
    StoreOp, Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, VertexBufferLayout,
    VertexState,
};

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
#[derive(Debug, Clone, Copy, Default, Pod, Zeroable)]
pub struct ShadowDecalVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub diffuse: u32,
    pub u: f32,
    pub v: f32,
}

impl ShadowDecalVertex {
    fn buffer_layout<'a>() -> VertexBufferLayout<'a> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Uint32,
            2 => Float32x2,
        ];
        VertexBufferLayout {
            array_stride: std::mem::size_of::<ShadowDecalVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
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

/// Texture handle wrapping a wgpu Texture + TextureView for shadow textures.
/// Replaces the previous placeholder (id, width, height) with real GPU resources.
#[derive(Debug, Clone)]
pub struct TextureHandle {
    pub texture: Arc<Texture>,
    pub view: Arc<TextureView>,
    pub width: u32,
    pub height: u32,
}

impl TextureHandle {
    pub fn new(texture: Texture, view: TextureView, width: u32, height: u32) -> Self {
        Self {
            texture: Arc::new(texture),
            view: Arc::new(view),
            width,
            height,
        }
    }

    /// Create a 1x1 black shadow texture as fallback.
    pub fn new_placeholder(device: &Device, queue: &Queue) -> Self {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Shadow Placeholder Texture"),
            size: Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &[0, 0, 0, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        Self::new(texture, view, 1, 1)
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
        let light_pos = super::wthree_d_shadow::get_light_pos_world(0);

        if let Some(ref shadow_tex) = self.shadow_texture[0] {
            if shadow_tex.get_light_pos_history() != light_pos {
                self.update_texture(light_pos);
            }
        }

        if let Some(ref robj) = self.robj {
            if self.last_obj_position != robj.position {
                if self.shadow_type == ShadowType::PROJECTION {
                    if let Some(ref mut projector) = self.shadow_projector {
                        let obj_to_light = light_pos - robj.position;
                        let dist = obj_to_light.length();
                        let normalized = if dist > 0.0 {
                            obj_to_light / dist
                        } else {
                            Vec3::new(0.0, 0.0, 1.0)
                        };
                        let virtual_light_pos =
                            robj.position + normalized * 2000.0;
                        projector.compute_perspective_projection(
                            robj.position,
                            virtual_light_pos,
                            self.decal_size_x,
                            self.decal_size_y,
                        );
                    }
                }
                self.last_obj_position = robj.position;
            }
        }
    }

    /// Update shadow texture image
    /// C++: void updateTexture(Vector3 &lightPos)
    pub fn update_texture(&mut self, light_pos: Vec3) {
        if self.shadow_type == ShadowType::PROJECTION {
            if let Some(ref robj) = self.robj {
                if robj.position == Vec3::ZERO {
                    return;
                }
                if let Some(ref mut projector) = self.shadow_projector {
                    let obj_to_light = light_pos - robj.position;
                    let dist = obj_to_light.length();
                    let normalized = if dist > 0.0 {
                        obj_to_light / dist
                    } else {
                        Vec3::new(0.0, 0.0, 1.0)
                    };
                    let virtual_light_pos = robj.position + normalized * 2000.0;
                    projector.compute_perspective_projection(
                        robj.position,
                        virtual_light_pos,
                        self.decal_size_x,
                        self.decal_size_y,
                    );
                }
            }
        } else if self.shadow_type == ShadowType::DECAL {
            if let Some(ref robj) = self.robj {
                let obj_pos = robj.position;
                let object_to_light = if self.flags & 0x10 != 0 {
                    let mut dir = light_pos - obj_pos;
                    dir.z = 0.0;
                    let len = dir.length();
                    if len > 0.0 { dir / len } else { Vec3::new(1.0, 0.0, 0.0) }
                } else {
                    Vec3::new(1.0, 0.0, 0.0)
                };

                const DECAL_TEXELS_PER_WORLD_UNIT: f32 = 64.0 / 20.0;

                if let Some(ref shadow_tex_arc) = self.shadow_texture[0] {
                    let tex_width = 64.0f32;
                    let tex_height = 64.0f32;

                    let u_vec = object_to_light * DECAL_TEXELS_PER_WORLD_UNIT / tex_width;
                    let rotated = Vec3::new(object_to_light.y, -object_to_light.x, 0.0);
                    let v_vec = rotated * DECAL_TEXELS_PER_WORLD_UNIT / tex_height;

                    if let Some(tex) = Arc::get_mut(shadow_tex_arc) {
                        tex.set_decal_uv_axis(u_vec, v_vec);
                    }
                }
            }
        }

        if let Some(ref shadow_tex) = self.shadow_texture[0] {
            if let Some(tex) = Arc::get_mut(shadow_tex) {
                tex.set_light_pos_history(light_pos);
            }
        }
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

/// Texture projector holding the computed projection matrix.
/// C++ uses TexProjectClass::Compute_Perspective_Projection.
#[derive(Debug, Clone)]
pub struct TexProjectHandle {
    /// Projection matrix mapping world space to shadow texture UV space.
    pub projection_matrix: Mat4,
    /// Object-to-light direction (normalized), used for perspective projection.
    pub light_direction: Vec3,
    /// Distance of virtual light from object center.
    pub light_distance: f32,
}

impl TexProjectHandle {
    pub fn new() -> Self {
        Self {
            projection_matrix: Mat4::IDENTITY,
            light_direction: Vec3::new(0.0, 0.0, -1.0),
            light_distance: 2000.0,
        }
    }

    /// Compute perspective projection from object position toward light.
    /// C++ W3DProjectedShadow.cpp: normalizes objToLight, places light 2000 units
    /// from object, then calls TexProjectClass::Compute_Perspective_Projection.
    pub fn compute_perspective_projection(
        &mut self,
        object_position: Vec3,
        light_position: Vec3,
        decal_size_x: f32,
        decal_size_y: f32,
    ) {
        let to_light = light_position - object_position;
        let dist = to_light.length();
        self.light_direction = if dist > 0.0 { to_light / dist } else { Vec3::new(0.0, 0.0, 1.0) };
        self.light_distance = 2000.0;

        let virtual_light_pos = object_position + self.light_direction * self.light_distance;

        let half_x = decal_size_x * 0.5;
        let half_y = decal_size_y * 0.5;

        self.projection_matrix = Mat4::orthographic_rh_gl(-half_x, half_x, -half_y, half_y, 0.0, self.light_distance * 2.0);
    }
}

impl Default for TexProjectHandle {
    fn default() -> Self {
        Self::new()
    }
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
    pub fn re_acquire_resources(&mut self, device: &Device, queue: &Queue) -> bool {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Shadow Render Target"),
            size: Extent3d {
                width: DEFAULT_RENDER_TARGET_WIDTH,
                height: DEFAULT_RENDER_TARGET_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        self.dynamic_render_target = Some(TextureHandle::new(texture, view, DEFAULT_RENDER_TARGET_WIDTH, DEFAULT_RENDER_TARGET_HEIGHT));
        self.render_target_has_alpha = true;
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

    /// Render shadows with actual GPU draw calls.
    /// C++: Int W3DProjectedShadowManager::renderShadows(RenderInfoClass & rinfo)
    pub fn render_shadows(
        &mut self,
        device: &Device,
        queue: &Queue,
        render_pass: &mut RenderPass,
        view_proj: Mat4,
        surface_format: TextureFormat,
    ) -> i32 {
        let mut projection_count: i32 = 0;

        if self.shadow_list.is_none() && self.decal_list.is_none() {
            return projection_count;
        }

        self.decal_verts_in_buf = 0xffff;
        self.decal_indices_in_buf = 0xffff;

        let mut last_shadow_decal_texture: Option<Arc<W3DShadowTexture>> = None;
        let mut last_shadow_type = ShadowType::NONE;

        if let Some(ref shadow_head) = self.shadow_list {
            let mut current = Some(shadow_head.clone());
            while let Some(shadow_arc) = current {
                let shadow = shadow_arc.read();
                if shadow.is_enabled && !shadow.is_invisible_enabled {
                    if shadow.shadow_type.contains(ShadowType::DECAL) {
                        if let Some(ref tex) = shadow.shadow_texture[0] {
                            if last_shadow_decal_texture.is_none() {
                                last_shadow_decal_texture = Some(tex.clone());
                            }
                            if last_shadow_type == ShadowType::NONE {
                                last_shadow_type = shadow.shadow_type;
                            }

                            let should_flush = last_shadow_decal_texture
                                .as_ref()
                                .map_or(true, |t| !Arc::ptr_eq(t, tex))
                                || last_shadow_type != shadow.shadow_type;

                            if should_flush {
                                if let Some(ref last_tex) = last_shadow_decal_texture {
                                    self.flush_decals(device, queue, render_pass, last_tex, last_shadow_type, view_proj, surface_format);
                                }
                                last_shadow_decal_texture = Some(tex.clone());
                                last_shadow_type = shadow.shadow_type;
                            }

                            drop(shadow);
                            self.queue_decal(&shadow_arc.read());
                            projection_count += 1;
                            current = shadow_arc.read().next.clone();
                            continue;
                        }
                    }

                    if shadow.shadow_type == ShadowType::PROJECTION {
                        projection_count += 1;
                    }
                }
                current = shadow.next.clone();
            }

            if let Some(ref last_tex) = last_shadow_decal_texture {
                self.flush_decals(device, queue, render_pass, last_tex, last_shadow_type, view_proj, surface_format);
            }
        }

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
                                self.flush_decals(device, queue, render_pass, last_tex, last_shadow_type, view_proj, surface_format);
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

            if let Some(ref last_tex) = last_shadow_decal_texture {
                self.flush_decals(device, queue, render_pass, last_tex, last_shadow_type, view_proj, surface_format);
            }
        }

        projection_count
    }

    /// Queue decal for rendering by adding vertices to staging buffer.
    /// C++: void W3DProjectedShadowManager::queueDecal(W3DProjectedShadow *shadow)
    pub fn queue_decal(&mut self, shadow: &W3DProjectedShadow) {
        let context = match self.shadow_context.as_mut() {
            Some(ctx) => ctx,
            None => return,
        };

        let half_x = shadow.decal_size_x * 0.5;
        let half_y = shadow.decal_size_y * 0.5;
        let base_vertex = context.decal_vertices.len() as u16;

        let diffuse = shadow.diffuse;

        // Create a simple 4-vertex quad for the decal
        context.decal_vertices.push(ShadowDecalVertex {
            x: shadow.x - half_x,
            y: shadow.y - half_y,
            z: shadow.z,
            diffuse,
            u: 0.0 + shadow.decal_offset_u,
            v: 1.0 + shadow.decal_offset_v,
        });
        context.decal_vertices.push(ShadowDecalVertex {
            x: shadow.x + half_x,
            y: shadow.y - half_y,
            z: shadow.z,
            diffuse,
            u: 1.0 + shadow.decal_offset_u,
            v: 1.0 + shadow.decal_offset_v,
        });
        context.decal_vertices.push(ShadowDecalVertex {
            x: shadow.x + half_x,
            y: shadow.y + half_y,
            z: shadow.z,
            diffuse,
            u: 1.0 + shadow.decal_offset_u,
            v: 0.0 + shadow.decal_offset_v,
        });
        context.decal_vertices.push(ShadowDecalVertex {
            x: shadow.x - half_x,
            y: shadow.y + half_y,
            z: shadow.z,
            diffuse,
            u: 0.0 + shadow.decal_offset_u,
            v: 0.0 + shadow.decal_offset_v,
        });

        // Two triangles for the quad
        context.decal_indices.extend_from_slice(&[
            base_vertex, base_vertex + 1, base_vertex + 2,
            base_vertex, base_vertex + 2, base_vertex + 3,
        ]);

        self.decal_verts_in_batch += 4;
        self.decal_polys_in_batch += 2;
    }

    /// Queue simple decal (floating on terrain)
    /// C++: void W3DProjectedShadowManager::queueSimpleDecal(W3DProjectedShadow *shadow)
    pub fn queue_simple_decal(&mut self, _shadow: &W3DProjectedShadow) {
        // C++ creates simple 4-vertex quad for decal
    }

    /// Flush decals to GPU with actual draw calls and blend modes.
    /// C++: void W3DProjectedShadowManager::flushDecals(W3DShadowTexture *texture, ShadowType type)
    pub fn flush_decals(
        &mut self,
        device: &Device,
        queue: &Queue,
        render_pass: &mut RenderPass,
        texture: &W3DShadowTexture,
        shadow_type: ShadowType,
        view_proj: Mat4,
        surface_format: TextureFormat,
    ) {
        if self.decal_polys_in_batch == 0 && self.decal_verts_in_batch == 0 {
            return;
        }

        let context = match self.shadow_context.as_mut() {
            Some(ctx) => ctx,
            None => return,
        };

        context.ensure_pipelines(device, surface_format);

        let pipeline = match shadow_type {
            ShadowType::DECAL | ShadowType::DIRECTIONAL_PROJECTION => context.pipeline_decal.as_ref(),
            ShadowType::ALPHA_DECAL => context.pipeline_alpha.as_ref(),
            ShadowType::ADDITIVE_DECAL => context.pipeline_additive.as_ref(),
            _ => context.pipeline_decal.as_ref(),
        };

        let Some(pipeline) = pipeline else { return };

        // Upload uniform buffer with view-projection matrix
        if let Some(ref uniform_buffer) = context.uniform_buffer {
            let vp_bytes: [[f32; 4]; 4] = view_proj.to_cols_array_2d();
            queue.write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[vp_bytes]));
        }

        // Upload vertex data
        let vertices = &context.decal_vertices;
        let indices = &context.decal_indices;
        if vertices.is_empty() || indices.is_empty() {
            self.decal_polys_in_batch = 0;
            self.decal_verts_in_batch = 0;
            return;
        }

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Decal VB"),
            contents: bytemuck::cast_slice(vertices),
            usage: BufferUsages::VERTEX,
        });

        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Shadow Decal IB"),
            contents: bytemuck::cast_slice(indices),
            usage: BufferUsages::INDEX,
        });

        render_pass.set_pipeline(pipeline);

        // Set uniform bind group (group 0)
        if let (Some(ref uniform_buffer), Some(ref ubgl)) = (context.uniform_buffer.as_ref(), context.uniform_bind_group_layout.as_ref()) {
            let uniform_bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Shadow Decal Uniform BG"),
                layout: ubgl,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            });
            render_pass.set_bind_group(0, &uniform_bg, &[]);
        }

        // Set texture bind group (group 1)
        if let (Some(ref tex_handle), Some(ref tbgl)) = (texture.get_texture(), context.texture_bind_group_layout.as_ref()) {
            let sampler = device.create_sampler(&SamplerDescriptor {
                label: Some("Shadow Decal Sampler"),
                address_mode_u: AddressMode::ClampToEdge,
                address_mode_v: AddressMode::ClampToEdge,
                address_mode_w: AddressMode::ClampToEdge,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Nearest,
                ..Default::default()
            });
            let tex_bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Shadow Decal Texture BG"),
                layout: tbgl,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(tex_handle.view.as_ref()),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            render_pass.set_bind_group(1, &tex_bg, &[]);
        }

        render_pass.set_vertex_buffer(0, vb.slice(..));
        render_pass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);

        // Clear staging buffers and reset batch counters
        context.decal_vertices.clear();
        context.decal_indices.clear();
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

/// Camera handle for shadow rendering, holding GPU resources.
#[derive(Debug, Clone)]
pub struct CameraHandle {
    pub view_matrix: Mat4,
    pub projection_matrix: Mat4,
    pub view_projection: Mat4,
}

impl CameraHandle {
    pub fn new() -> Self {
        Self {
            view_matrix: Mat4::IDENTITY,
            projection_matrix: Mat4::IDENTITY,
            view_projection: Mat4::IDENTITY,
        }
    }

    pub fn update_matrices(&mut self, view: Mat4, projection: Mat4) {
        self.view_matrix = view;
        self.projection_matrix = projection;
        self.view_projection = projection * view;
    }
}

impl Default for CameraHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Render context holding decal vertex/index buffers and pipeline for shadow draws.
#[derive(Debug)]
pub struct RenderContextHandle {
    /// Staging vertex data for batched decal rendering
    pub decal_vertices: Vec<ShadowDecalVertex>,
    /// Staging index data for batched decal rendering
    pub decal_indices: Vec<u16>,
    /// GPU vertex buffer (recreated each flush)
    vertex_buffer: Option<Buffer>,
    /// GPU index buffer (recreated each flush)
    index_buffer: Option<Buffer>,
    /// Decal render pipeline for multiplicative blend
    pipeline_decal: Option<RenderPipeline>,
    /// Decal render pipeline for alpha blend
    pipeline_alpha: Option<RenderPipeline>,
    /// Decal render pipeline for additive blend
    pipeline_additive: Option<RenderPipeline>,
    /// Bind group layout for shadow texture
    texture_bind_group_layout: Option<BindGroupLayout>,
    /// Uniform buffer for projection matrix
    uniform_buffer: Option<Buffer>,
    /// Uniform bind group layout
    uniform_bind_group_layout: Option<BindGroupLayout>,
    /// Surface format for pipeline creation
    surface_format: Option<TextureFormat>,
}

impl RenderContextHandle {
    pub fn new() -> Self {
        Self {
            decal_vertices: Vec::with_capacity(SHADOW_DECAL_VERTEX_SIZE),
            decal_indices: Vec::with_capacity(SHADOW_DECAL_INDEX_SIZE),
            vertex_buffer: None,
            index_buffer: None,
            pipeline_decal: None,
            pipeline_alpha: None,
            pipeline_additive: None,
            texture_bind_group_layout: None,
            uniform_buffer: None,
            uniform_bind_group_layout: None,
            surface_format: None,
        }
    }

    fn ensure_pipelines(&mut self, device: &Device, surface_format: TextureFormat) {
        if self.surface_format == Some(surface_format) && self.pipeline_decal.is_some() {
            return;
        }
        self.surface_format = Some(surface_format);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Decal Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADOW_DECAL_SHADER.into()),
        });

        self.uniform_bind_group_layout = Some(device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Shadow Decal Uniform Layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        }));

        self.texture_bind_group_layout = Some(device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Shadow Decal Texture Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        }));

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Shadow Decal Uniforms"),
            size: 64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.uniform_buffer = Some(uniform_buffer);

        let decal_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::SrcColor,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let alpha_blend = wgpu::BlendState::ALPHA_BLENDING;

        let additive_blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };

        let layouts: Vec<&BindGroupLayout> = vec![
            self.uniform_bind_group_layout.as_ref().unwrap(),
            self.texture_bind_group_layout.as_ref().unwrap(),
        ];

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Shadow Decal Pipeline Layout"),
            bind_group_layouts: &layouts,
            push_constant_ranges: &[],
        });

        let vertex_layout = ShadowDecalVertex::buffer_layout();

        let create_pipeline = |blend: wgpu::BlendState, label: &str| -> RenderPipeline {
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[vertex_layout.clone()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: TextureFormat::Depth24PlusStencil8,
                    depth_write_enabled: false,
                    depth_compare: CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };

        self.pipeline_decal = Some(create_pipeline(decal_blend, "Shadow Decal Pipeline (Multiply)"));
        self.pipeline_alpha = Some(create_pipeline(alpha_blend, "Shadow Decal Pipeline (Alpha)"));
        self.pipeline_additive = Some(create_pipeline(additive_blend, "Shadow Decal Pipeline (Additive)"));
    }
}

impl Default for RenderContextHandle {
    fn default() -> Self {
        Self::new()
    }
}

const SHADOW_DECAL_SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) diffuse: u32,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

@vertex
fn vs(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    let r = f32(input.diffuse & 0xFFu) / 255.0;
    let g = f32((input.diffuse >> 8u) & 0xFFu) / 255.0;
    let b = f32((input.diffuse >> 16u) & 0xFFu) / 255.0;
    let a = f32((input.diffuse >> 24u) & 0xFFu) / 255.0;
    out.color = vec4<f32>(r, g, b, a);
    out.uv = input.uv;
    return out;
}

@group(1) @binding(0) var shadow_tex: texture_2d<f32>;
@group(1) @binding(1) var shadow_sampler: sampler;

@fragment
fn fs(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(shadow_tex, shadow_sampler, in.uv);
    return in.color * tex_color;
}
"#;

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
