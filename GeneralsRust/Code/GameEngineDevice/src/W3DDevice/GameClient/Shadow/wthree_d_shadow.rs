//! W3D Shadow System
//!
//! Corresponds to C++ files:
//! - GameEngineDevice/Source/W3DDevice/GameClient/Shadow/W3DShadow.cpp
//! - GameEngineDevice/Include/W3DDevice/GameClient/W3DShadow.h
//!
//! Real time shadow representations including shadow volume and projected shadow management.

use glam::{Mat4, Vec3, Vec4};
use parking_lot::RwLock;
use std::sync::{Arc, OnceLock};

use super::{W3DProjectedShadowManager, W3DVolumetricShadowManager};

/// Sun distance from ground for directional light shadows
/// C++: #define SUN_DISTANCE_FROM_GROUND 10000.0f
pub const SUN_DISTANCE_FROM_GROUND: f32 = 10000.0f;

/// Maximum number of shadow casting lights
/// C++: LightPosWorld[MAX_SHADOW_LIGHTS] with 1 element
pub const MAX_SHADOW_LIGHTS: usize = 1;

/// Shadow type bit flags matching C++ ShadowType
/// C++: enum ShadowType in GameClient/Shadow.h
/// These are bit flags, not sequential values!
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowType(pub u32);

impl ShadowType {
    /// No shadow
    /// C++: SHADOW_NONE = 0x00000000
    pub const NONE: ShadowType = ShadowType(0x00000000);

    /// Shadow decal applied via modulate blend
    /// C++: SHADOW_DECAL = 0x00000001
    pub const DECAL: ShadowType = ShadowType(0x00000001);

    /// Volume-based shadow (stencil)
    /// C++: SHADOW_VOLUME = 0x00000002
    pub const VOLUME: ShadowType = ShadowType(0x00000002);

    /// Projected shadow
    /// C++: SHADOW_PROJECTION = 0x00000004
    pub const PROJECTION: ShadowType = ShadowType(0x00000004);

    /// Extra setting for shadows which need dynamic updates
    /// C++: SHADOW_DYNAMIC_PROJECTION = 0x00000008
    pub const DYNAMIC_PROJECTION: ShadowType = ShadowType(0x00000008);

    /// Extra setting for shadow decals that rotate with sun direction
    /// C++: SHADOW_DIRECTIONAL_PROJECTION = 0x00000010
    pub const DIRECTIONAL_PROJECTION: ShadowType = ShadowType(0x00000010);

    /// Not really for shadows but for other decal uses. Alpha blended.
    /// C++: SHADOW_ALPHA_DECAL = 0x00000020
    pub const ALPHA_DECAL: ShadowType = ShadowType(0x00000020);

    /// Not really for shadows but for other decal uses. Additive blended.
    /// C++: SHADOW_ADDITIVE_DECAL = 0x00000040
    pub const ADDITIVE_DECAL: ShadowType = ShadowType(0x00000040);

    /// Check if this shadow type contains a specific flag
    pub fn contains(&self, other: ShadowType) -> bool {
        (self.0 & other.0) != 0
    }

    /// Check if this is a decal type (DECAL, ALPHA_DECAL, or ADDITIVE_DECAL)
    pub fn is_decal(&self) -> bool {
        self.contains(Self::DECAL)
            || self.contains(Self::ALPHA_DECAL)
            || self.contains(Self::ADDITIVE_DECAL)
    }

    /// Check if this is a projection type
    pub fn is_projection(&self) -> bool {
        self.contains(Self::PROJECTION) || self.contains(Self::DIRECTIONAL_PROJECTION)
    }

    /// Check if this is a volume shadow
    pub fn is_volume(&self) -> bool {
        self.contains(Self::VOLUME)
    }
}

impl Default for ShadowType {
    fn default() -> Self {
        ShadowType::VOLUME
    }
}

impl std::ops::BitOr for ShadowType {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self::Output {
        ShadowType(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for ShadowType {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl std::ops::BitAnd for ShadowType {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self::Output {
        ShadowType(self.0 & rhs.0)
    }
}

/// Shadow color type (ARGB format)
pub type ShadowColor = u32;

/// Light position in world space - mutable global state
/// C++: Vector3 LightPosWorld[MAX_SHADOW_LIGHTS]
use std::sync::Mutex;
static LIGHT_POS_WORLD: Mutex<[Vec3; MAX_SHADOW_LIGHTS]> =
    Mutex::new([Vec3::new(94.0161, 50.499, 200.0)]);

/// Get light position in world space for given light index
/// C++: W3DShadowManager::getLightPosWorld()
pub fn get_light_pos_world(light_index: usize) -> Vec3 {
    if let Ok(guard) = LIGHT_POS_WORLD.lock() {
        guard.get(light_index).copied().unwrap_or(Vec3::ZERO)
    } else {
        Vec3::ZERO
    }
}

/// Set light position in world space
/// C++: W3DShadowManager::setLightPosition()
pub fn set_light_pos_world(light_index: usize, pos: Vec3) {
    if light_index < MAX_SHADOW_LIGHTS {
        if let Ok(mut guard) = LIGHT_POS_WORLD.lock() {
            guard[light_index] = pos;
        }
    }
}

/// W3D Shadow Manager - manages all shadow systems
/// C++: class W3DShadowManager
pub struct W3DShadowManager {
    /// Flag if current scene needs shadows (m_isShadowScene)
    is_shadow_scene: bool,
    /// Color and alpha for all shadows in scene (m_shadowColor - ARGB format)
    shadow_color: ShadowColor,
    /// Stencil shadow mask (m_stencilShadowMask)
    stencil_shadow_mask: i32,
    /// Volumetric shadow manager
    volumetric_manager: Option<Arc<RwLock<W3DVolumetricShadowManager>>>,
    /// Projected shadow manager  
    projected_manager: Option<Arc<RwLock<W3DProjectedShadowManager>>>,
}

impl Default for W3DShadowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DShadowManager {
    /// Create new shadow manager
    /// C++: W3DShadowManager::W3DShadowManager()
    pub fn new() -> Self {
        let mut manager = Self {
            is_shadow_scene: false,
            shadow_color: 0x7fa0a0a0, // C++ default: m_shadowColor = 0x7fa0a0a0
            stencil_shadow_mask: 0,   // C++: m_stencilShadowMask = 0
            volumetric_manager: None,
            projected_manager: None,
        };

        manager.initialize_light_position();
        manager
    }

    /// Initialize light position from global terrain lighting
    /// C++: Constructor initializes light ray from TheGlobalData->m_terrainLightPos
    fn initialize_light_position(&mut self) {
        // C++ code:
        // Vector3 lightRay(-TheGlobalData->m_terrainLightPos[0].x,
        //     -TheGlobalData->m_terrainLightPos[0].y, -TheGlobalData->m_terrainLightPos[0].z);
        // lightRay.Normalize();
        // LightPosWorld[0] = lightRay * SUN_DISTANCE_FROM_GROUND;

        // Default light direction (normalized)
        let light_ray = Vec3::new(-94.0161, -50.499, -200.0).normalize();
        set_light_pos_world(0, light_ray * SUN_DISTANCE_FROM_GROUND);
    }

    /// One-time initialization of shadow systems
    /// C++: Bool W3DShadowManager::init()
    pub fn init(&mut self) -> bool {
        let mut result = true;

        // Initialize volumetric shadow manager
        if let Some(ref vol_manager) = self.volumetric_manager {
            let mut mgr = vol_manager.write();
            if mgr.init() && mgr.re_acquire_resources() {
                result = true;
            }
        }

        // Initialize projected shadow manager
        if let Some(ref proj_manager) = self.projected_manager {
            let mut mgr = proj_manager.write();
            if mgr.init() && mgr.re_acquire_resources() {
                result = true;
            }
        }

        result
    }

    /// Per-map reset - frees shadows from all objects
    /// C++: void W3DShadowManager::Reset()
    pub fn reset(&mut self) {
        if let Some(ref vol_manager) = self.volumetric_manager {
            vol_manager.write().reset();
        }
        if let Some(ref proj_manager) = self.projected_manager {
            proj_manager.write().reset();
        }
    }

    /// Re-acquire device-dependent resources
    /// C++: Bool W3DShadowManager::ReAcquireResources()
    pub fn re_acquire_resources(&mut self) -> bool {
        let mut result = true;

        if let Some(ref vol_manager) = self.volumetric_manager {
            if !vol_manager.write().re_acquire_resources() {
                result = false;
            }
        }
        if let Some(ref proj_manager) = self.projected_manager {
            if !proj_manager.write().re_acquire_resources() {
                result = false;
            }
        }

        result
    }

    /// Release device-dependent resources
    /// C++: void W3DShadowManager::ReleaseResources()
    pub fn release_resources(&mut self) {
        if let Some(ref vol_manager) = self.volumetric_manager {
            vol_manager.write().release_resources();
        }
        if let Some(ref proj_manager) = self.projected_manager {
            proj_manager.write().release_resources();
        }
    }

    /// Flag system to process shadows on next render call
    /// C++: void queueShadows(Bool state) { m_isShadowScene = state; }
    pub fn queue_shadows(&mut self, state: bool) {
        self.is_shadow_scene = state;
    }

    /// Check if current scene needs shadows
    /// C++: Bool isShadowScene() { return m_isShadowScene; }
    pub fn is_shadow_scene(&self) -> bool {
        self.is_shadow_scene
    }

    /// Set shadow color in ARGB format
    /// C++: void setShadowColor(UnsignedInt color) { m_shadowColor = color; }
    pub fn set_shadow_color(&mut self, color: ShadowColor) {
        self.shadow_color = color;
    }

    /// Get shadow color in ARGB format
    /// C++: UnsignedInt getShadowColor() { return m_shadowColor; }
    pub fn get_shadow_color(&self) -> ShadowColor {
        self.shadow_color
    }

    /// Set stencil shadow mask
    /// C++: inline void setStencilShadowMask(int mask) { m_stencilShadowMask = mask; }
    pub fn set_stencil_shadow_mask(&mut self, mask: i32) {
        self.stencil_shadow_mask = mask;
    }

    /// Get stencil shadow mask
    /// C++: inline Int getStencilShadowMask() { return m_stencilShadowMask; }
    pub fn get_stencil_shadow_mask(&self) -> i32 {
        self.stencil_shadow_mask
    }

    /// Set light position for given index
    /// C++: void W3DShadowManager::setLightPosition(Int lightIndex, Real x, Real y, Real z)
    pub fn set_light_position(&mut self, light_index: i32, x: f32, y: f32, z: f32) {
        if light_index != 0 {
            return; // C++: only supports light index 0
        }
        set_light_pos_world(light_index as usize, Vec3::new(x, y, z));
    }

    /// Get light position for given index
    /// C++: Vector3 &W3DShadowManager::getLightPosWorld(Int lightIndex)
    pub fn get_light_pos_world(&self, light_index: i32) -> Vec3 {
        get_light_pos_world(light_index as usize)
    }

    /// Set time of day - updates light position based on terrain lighting
    /// C++: void W3DShadowManager::setTimeOfDay(TimeOfDay tod)
    pub fn set_time_of_day(&mut self, tod: TimeOfDay) {
        // C++ code reads from TheGlobalData->m_terrainObjectsLighting[tod][0]
        // and calculates light ray direction:
        // const GlobalData::TerrainLighting *ol = &TheGlobalData->m_terrainObjectsLighting[tod][0];
        // Vector3 lightRay(-ol->lightPos.x, -ol->lightPos.y, -ol->lightPos.z);
        // lightRay.Normalize();
        // lightRay *= SUN_DISTANCE_FROM_GROUND;
        // setLightPosition(0, lightRay.X, lightRay.Y, lightRay.Z);

        // Default light directions for each time of day (normalized, then scaled)
        let light_ray = match tod {
            TimeOfDay::Morning => {
                Vec3::new(-0.4f32, -0.6f32, -0.7f32).normalize() * SUN_DISTANCE_FROM_GROUND
            }
            TimeOfDay::Afternoon => {
                Vec3::new(-0.3f32, -0.4f32, -0.9f32).normalize() * SUN_DISTANCE_FROM_GROUND
            }
            TimeOfDay::Evening => {
                Vec3::new(-0.5f32, -0.3f32, -0.6f32).normalize() * SUN_DISTANCE_FROM_GROUND
            }
            TimeOfDay::Night => {
                Vec3::new(-0.2f32, -0.2f32, -1.0f32).normalize() * SUN_DISTANCE_FROM_GROUND
            }
        };

        set_light_pos_world(0, light_ray);
    }

    /// Force update of all shadows even when light/object hasn't moved
    /// C++: void W3DShadowManager::invalidateCachedLightPositions()
    pub fn invalidate_cached_light_positions(&mut self) {
        if let Some(ref vol_manager) = self.volumetric_manager {
            vol_manager.write().invalidate_cached_light_positions();
        }
        if let Some(ref proj_manager) = self.projected_manager {
            proj_manager.write().invalidate_cached_light_positions();
        }
    }

    /// Add shadow caster to rendering system
    /// C++: Shadow* W3DShadowManager::addShadow(RenderObjClass *robj, Shadow::ShadowTypeInfo *shadowInfo, Drawable *draw)
    pub fn add_shadow(
        &mut self,
        _robj: &RenderObject,
        shadow_info: Option<&ShadowTypeInfo>,
    ) -> Option<ShadowHandle> {
        let shadow_type = shadow_info.map(|i| i.shadow_type).unwrap_or_default();

        if shadow_type.is_volume() {
            if let Some(ref vol_manager) = self.volumetric_manager {
                vol_manager.write().add_shadow()
            } else {
                None
            }
        } else if shadow_type.is_projection() || shadow_type.is_decal() {
            if let Some(ref proj_manager) = self.projected_manager {
                if let Some(info) = shadow_info {
                    proj_manager.write().add_decal(info)
                } else {
                    proj_manager.write().add_shadow()
                }
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Remove shadow from rendering system
    /// C++: void W3DShadowManager::removeShadow(Shadow *shadow)
    pub fn remove_shadow(&mut self, shadow: &ShadowHandle) {
        // C++ calls shadow->release() which delegates to appropriate manager
        if shadow.shadow_type.is_volume() {
            if let Some(ref vol_manager) = self.volumetric_manager {
                vol_manager.write().remove_shadow(shadow);
            }
        } else {
            if let Some(ref proj_manager) = self.projected_manager {
                proj_manager.write().remove_shadow(shadow);
            }
        }
    }

    /// Remove all shadows
    /// C++: void W3DShadowManager::removeAllShadows()
    pub fn remove_all_shadows(&mut self) {
        if let Some(ref vol_manager) = self.volumetric_manager {
            vol_manager.write().remove_all_shadows();
        }
        if let Some(ref proj_manager) = self.projected_manager {
            proj_manager.write().remove_all_shadows();
        }
    }

    /// Set the volumetric shadow manager
    pub fn set_volumetric_manager(&mut self, manager: Arc<RwLock<W3DVolumetricShadowManager>>) {
        self.volumetric_manager = Some(manager);
    }

    /// Set the projected shadow manager
    pub fn set_projected_manager(&mut self, manager: Arc<RwLock<W3DProjectedShadowManager>>) {
        self.projected_manager = Some(manager);
    }
}

/// Time of day enumeration for lighting
/// C++: enum TimeOfDay
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum TimeOfDay {
    Morning = 0,
    Afternoon = 1,
    Evening = 2,
    Night = 3,
}

/// Shadow type info for creating shadows
/// C++: Shadow::ShadowTypeInfo
#[derive(Debug, Clone, Default)]
pub struct ShadowTypeInfo {
    /// Type of shadow to create
    pub shadow_type: ShadowType,
    /// Allow dynamic updates to shadow
    pub allow_updates: bool,
    /// Allow world-aligned projection
    pub allow_world_align: bool,
    /// Shadow texture name
    pub shadow_name: String,
    /// Size in X direction
    pub size_x: f32,
    /// Size in Y direction
    pub size_y: f32,
    /// Offset in X direction
    pub offset_x: f32,
    /// Offset in Y direction
    pub offset_y: f32,
}

/// Render object abstraction (placeholder for RenderObjClass)
#[derive(Debug, Clone)]
pub struct RenderObject {
    /// Object transform
    pub transform: Mat4,
    /// Object position
    pub position: Vec3,
    /// Is object visible
    pub is_visible: bool,
}

impl Default for RenderObject {
    fn default() -> Self {
        Self {
            transform: Mat4::IDENTITY,
            position: Vec3::ZERO,
            is_visible: true,
        }
    }
}

/// Handle to a shadow in the system
#[derive(Debug, Clone)]
pub struct ShadowHandle {
    /// Unique ID
    pub id: u64,
    /// Shadow type
    pub shadow_type: ShadowType,
    /// Is shadow enabled
    pub is_enabled: bool,
    /// Is invisible enabled
    pub is_invisible_enabled: bool,
}

impl ShadowHandle {
    /// Create new shadow handle
    pub fn new(id: u64, shadow_type: ShadowType) -> Self {
        Self {
            id,
            shadow_type,
            is_enabled: true,
            is_invisible_enabled: false,
        }
    }
}

/// Global shadow manager singleton
/// C++: W3DShadowManager *TheW3DShadowManager = NULL;
static THE_W3D_SHADOW_MANAGER: OnceLock<Arc<RwLock<W3DShadowManager>>> = OnceLock::new();

/// Get or initialize the global shadow manager
pub fn the_w3d_shadow_manager() -> Arc<RwLock<W3DShadowManager>> {
    THE_W3D_SHADOW_MANAGER
        .get_or_init(|| Arc::new(RwLock::new(W3DShadowManager::new())))
        .clone()
}

/// Do shadows rendering entry point
/// C++: void DoShadows(RenderInfoClass & rinfo, Bool stencilPass)
pub fn do_shadows(rinfo: &mut RenderInfo, stencil_pass: bool) {
    let manager = the_w3d_shadow_manager();

    // Store the camera frustum for shadow culling
    // C++: shadowCameraFrustum = &rinfo.Camera.Get_Frustum();
    let camera_frustum = rinfo.camera_frustum.clone();
    let (is_shadow_scene, projected_manager, volumetric_manager) = {
        let mgr = manager.read();
        (
            mgr.is_shadow_scene(),
            mgr.projected_manager.clone(),
            mgr.volumetric_manager.clone(),
        )
    };

    let mut projection_count: i32 = 0;

    // Projected shadows render first because they may fill the stencil buffer
    // which will be used by the shadow volumes
    // C++: if (stencilPass == FALSE && TheW3DProjectedShadowManager)
    if !stencil_pass && is_shadow_scene {
        if let Some(proj_manager) = projected_manager {
            projection_count = proj_manager.write().render_shadows(rinfo);
        }
    }

    // C++: if (stencilPass == TRUE && TheW3DVolumetricShadowManager)
    if stencil_pass && is_shadow_scene {
        // Restore camera frustum for volumetric shadows
        rinfo.camera_frustum = camera_frustum;

        if let Some(vol_manager) = volumetric_manager {
            vol_manager.write().render_shadows(projection_count, false);
        }
    }

    // Reset shadow processing flag for this frame
    // C++: if (TheW3DShadowManager && stencilPass) TheW3DShadowManager->queueShadows(FALSE);
    if stencil_pass {
        manager.write().queue_shadows(false);
    }
}

/// Render info structure (abstraction for RenderInfoClass)
#[derive(Debug, Clone)]
pub struct RenderInfo {
    /// Camera frustum for culling
    pub camera_frustum: Option<Frustum>,
    /// Frame number
    pub frame_number: u64,
}

impl Default for RenderInfo {
    fn default() -> Self {
        Self {
            camera_frustum: None,
            frame_number: 0,
        }
    }
}

impl RenderInfo {
    /// Attach a frame number for downstream systems that need stable sequencing.
    pub fn with_frame_number(mut self, frame_number: u64) -> Self {
        self.frame_number = frame_number;
        self
    }

    /// Attach a camera frustum for downstream culling-sensitive systems.
    pub fn with_camera_frustum(mut self, camera_frustum: Option<Frustum>) -> Self {
        self.camera_frustum = camera_frustum;
        self
    }
}

/// Frustum for culling (abstraction for FrustumClass)
#[derive(Debug, Clone)]
pub struct Frustum {
    /// Planes defining the frustum
    pub planes: [Vec4; 6],
}

impl Default for Frustum {
    fn default() -> Self {
        Self {
            planes: [Vec4::ZERO; 6],
        }
    }
}

impl Frustum {
    /// Build a frustum from the available camera state.
    pub fn from_camera(
        camera_position: Vec3,
        camera_direction: Vec3,
        near_z: f32,
        far_z: f32,
        fov_degrees: f32,
    ) -> Self {
        fn plane_from_points(a: Vec3, b: Vec3, c: Vec3, inside_point: Vec3) -> Vec4 {
            let mut normal = (b - a).cross(c - a);
            if normal.length_squared() <= f32::EPSILON {
                return Vec4::ZERO;
            }

            normal = normal.normalize();
            let mut distance = -normal.dot(a);
            if normal.dot(inside_point) + distance < 0.0 {
                normal = -normal;
                distance = -distance;
            }

            Vec4::new(normal.x, normal.y, normal.z, distance)
        }

        let mut forward = if camera_direction.length_squared() > f32::EPSILON {
            camera_direction.normalize()
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };

        if forward.length_squared() <= f32::EPSILON {
            forward = Vec3::new(0.0, 0.0, 1.0);
        }

        let mut up_hint = Vec3::Y;
        if forward.dot(up_hint).abs() > 0.99 {
            up_hint = Vec3::Z;
        }

        let mut right = forward.cross(up_hint);
        if right.length_squared() <= f32::EPSILON {
            right = Vec3::X;
        } else {
            right = right.normalize();
        }

        let mut up = right.cross(forward);
        if up.length_squared() <= f32::EPSILON {
            up = Vec3::Y;
        } else {
            up = up.normalize();
        }

        let near_z = near_z.max(0.001);
        let far_z = far_z.max(near_z + 0.001);
        let fov_radians = fov_degrees
            .to_radians()
            .clamp(0.01, std::f32::consts::PI - 0.01);
        let half_tan = (fov_radians * 0.5).tan();
        let aspect_ratio = 1.0f32;

        let near_half_height = near_z * half_tan;
        let near_half_width = near_half_height * aspect_ratio;
        let far_half_height = far_z * half_tan;
        let far_half_width = far_half_height * aspect_ratio;

        let near_center = camera_position + forward * near_z;
        let far_center = camera_position + forward * far_z;

        let ntl = near_center + up * near_half_height - right * near_half_width;
        let ntr = near_center + up * near_half_height + right * near_half_width;
        let nbl = near_center - up * near_half_height - right * near_half_width;
        let nbr = near_center - up * near_half_height + right * near_half_width;

        let ftl = far_center + up * far_half_height - right * far_half_width;
        let ftr = far_center + up * far_half_height + right * far_half_width;
        let fbl = far_center - up * far_half_height - right * far_half_width;
        let fbr = far_center - up * far_half_height + right * far_half_width;

        let inside_point = camera_position + forward * ((near_z + far_z) * 0.5);

        Self {
            planes: [
                plane_from_points(ntl, ntr, nbr, inside_point),
                plane_from_points(ftl, fbl, fbr, inside_point),
                plane_from_points(camera_position, nbl, ntl, inside_point),
                plane_from_points(camera_position, ntr, nbr, inside_point),
                plane_from_points(camera_position, ntl, ntr, inside_point),
                plane_from_points(camera_position, nbr, nbl, inside_point),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shadow_manager_creation() {
        let manager = W3DShadowManager::new();
        assert!(!manager.is_shadow_scene());
        assert_eq!(manager.get_shadow_color(), 0x7fa0a0a0);
        assert_eq!(manager.get_stencil_shadow_mask(), 0);
    }

    #[test]
    fn test_shadow_color() {
        let mut manager = W3DShadowManager::new();
        manager.set_shadow_color(0xff000000);
        assert_eq!(manager.get_shadow_color(), 0xff000000);
    }

    #[test]
    fn test_queue_shadows() {
        let mut manager = W3DShadowManager::new();
        assert!(!manager.is_shadow_scene());

        manager.queue_shadows(true);
        assert!(manager.is_shadow_scene());

        manager.queue_shadows(false);
        assert!(!manager.is_shadow_scene());
    }

    #[test]
    fn test_stencil_shadow_mask() {
        let mut manager = W3DShadowManager::new();
        manager.set_stencil_shadow_mask(0xFF);
        assert_eq!(manager.get_stencil_shadow_mask(), 0xFF);
    }

    #[test]
    fn test_global_manager() {
        let manager = the_w3d_shadow_manager();
        let mgr = manager.read();
        assert!(!mgr.is_shadow_scene());
    }

    #[test]
    fn test_light_position() {
        let manager = the_w3d_shadow_manager();
        let mut mgr = manager.write();
        mgr.set_light_position(0, 100.0, 200.0, 300.0);

        let pos = mgr.get_light_pos_world(0);
        assert_eq!(pos.x, 100.0);
        assert_eq!(pos.y, 200.0);
        assert_eq!(pos.z, 300.0);
    }

    #[test]
    fn test_shadow_type_info() {
        let info = ShadowTypeInfo {
            shadow_type: ShadowType::VOLUME,
            allow_updates: true,
            ..Default::default()
        };
        assert!(info.shadow_type.contains(ShadowType::VOLUME));
        assert!(info.allow_updates);
    }
}
