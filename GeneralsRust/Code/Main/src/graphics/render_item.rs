use crate::assets::{W3DMaterial, W3dAnimationBinding};
use crate::fow_rendering::ObjectVisibility;
use crate::game_logic::ObjectId;
use gamelogic::common::types::ObjectShroudStatus;
use glam::{Mat4, Vec2, Vec3};

use super::render_pipeline::RenderPass;

/// Stable identity for a frozen W3D ghost snapshot.
///
/// Ghost IDs are pooled by the C++ manager, so the pool identity alone is not
/// enough when a frame straddles a remove/reuse event.  The immutable scene
/// revision and source snapshot coordinates are retained in the render item
/// owner to prevent an old ghost payload from being attached to a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GhostRenderKey {
    pub ghost_id: u64,
    pub player_index: usize,
    pub snapshot_index: usize,
    pub scene_revision: u64,
}

/// C++'s ghost branch uses a dedicated fogged light environment and does not
/// derive its appearance from object FOW alpha.  Keeping the route explicit
/// makes a future WGPU consumer choose the correct pass instead of silently
/// reusing the ordinary object material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GhostLightingRoute {
    AlwaysFogged,
}

/// Immutable renderer contract for one exact W3D ghost RenderObj.
///
/// This is intentionally a data contract only.  It is not populated from a
/// model name or a normal Drawable and it is not consumed by the ordinary
/// FOW/lighting path.  A consumer must validate the exact asset and use the
/// dedicated lighting route before submitting it to WGPU.
#[derive(Debug, Clone, PartialEq)]
pub struct GhostRenderState {
    pub key: GhostRenderKey,
    pub parent_object_id: Option<ObjectId>,
    pub model_name: String,
    pub world_transform: Mat4,
    pub object_scale: f32,
    pub argb_color: [f32; 4],
    pub lighting_route: GhostLightingRoute,
    pub suppress_parent: bool,
}

impl GhostRenderState {
    pub fn new(
        key: GhostRenderKey,
        parent_object_id: Option<ObjectId>,
        model_name: String,
        world_transform: Mat4,
        object_scale: f32,
        argb_color: [f32; 4],
        suppress_parent: bool,
    ) -> Option<Self> {
        let valid_name = !model_name.trim().is_empty();
        let finite_transform = world_transform
            .to_cols_array()
            .into_iter()
            .all(|value| value.is_finite());
        let valid_scale = object_scale.is_finite() && object_scale > 0.0;
        let finite_color = argb_color.into_iter().all(|value| value.is_finite());
        (valid_name && finite_transform && valid_scale && finite_color).then_some(Self {
            key,
            parent_object_id,
            model_name,
            world_transform,
            object_scale,
            argb_color,
            lighting_route: GhostLightingRoute::AlwaysFogged,
            suppress_parent,
        })
    }
}

/// Identity of the client-side source that produced a render item.
///
/// C++ `DrawableID` and gameplay `ObjectID` are independent domains.  Effects,
/// placement previews, and other standalone client drawables intentionally do
/// not have a gameplay object, so they must not be smuggled into the object
/// domain merely to satisfy the renderer's bookkeeping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderItemOwner {
    Object(ObjectId),
    /// Snapshot-owned projectile mesh. Projectile IDs share the gameplay
    /// ObjectID type, but are not GameClient direct-drawable bindings.
    PresentationProjectile(ObjectId),
    UnboundClientDrawable(u32),
    /// Frozen W3D ghost RenderObj.  This is a separate identity domain from
    /// gameplay objects and standalone Drawable IDs.
    W3dGhost(GhostRenderKey),
}

/// Frozen direct-drawable scene outcome retained on an eligible object-owned
/// render item.
///
/// This is deliberately separate from `fow_visibility`: C++ W3D chooses the
/// projected-shroud route from `ObjectShroudStatus` after Drawable-owned
/// clear-grace handling, not from a scalar FOW alpha.  hq-1a1 will consume
/// this exact status/pass pair when it adds the material route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenDirectSceneShroudRenderState {
    /// Scene-local status after the Drawable's clear-grace evaluation.
    pub final_status: ObjectShroudStatus,
    /// Exact C++ `ss > OBJECTSHROUD_CLEAR` material-pass eligibility.
    pub pushes_projected_shroud_pass: bool,
}

/// Frozen C++ shroud result for a standalone client Drawable whose
/// `DrawableInfo::m_shroudStatusObjectID` points at an unrelated controller.
///
/// This is intentionally separate from both `fow_visibility` and the direct
/// object binding state above: objectless drawables have no ObjectID owner,
/// no Drawable clear-frame timer, and no direct scene lifetime. The projected
/// material route can consume this exact status later without deriving it from
/// alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrozenObjectlessDrawableShroudRenderState {
    /// Runtime GameClient Drawable identity which owns this fact.
    pub drawable_id: u32,
    /// Optional frozen controller identity. `None` means the source had no
    /// controller; `Some` with `controller_found == false` is a stale/missing
    /// controller and still fails open to C++'s clear status.
    pub controller_object_id: Option<ObjectId>,
    pub controller_found: bool,
    /// C++ W3DScene result: clear unless the controller is Fogged or worse,
    /// in which case an objectless Drawable is forced to Shrouded.
    pub final_status: ObjectShroudStatus,
    pub pushes_projected_shroud_pass: bool,
}

/// The only authority permitted to supply a skinned RenderItem's bone palette.
///
/// Ordinary Drawable geometry keeps its frozen Draw-state HAnim path. An
/// HMODEL `SKIN_NODE`, however, is a `MeshClass` whose C++ container is the
/// independently constructed HMODEL `HLodClass`; it must sample that exact
/// HMODEL's named/default bind-pose HTree instead of a parent Drawable or a
/// whole-file convenience hierarchy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderItemBonePaletteSource {
    /// Existing Draw-state binding, animation frame, and capture controls on
    /// this item provide the palette when one is explicitly selected.
    FrozenDrawState,
    /// The strict private graphics-cache source model and exact HMODEL
    /// definition that own this skin's bind-pose palette.
    HmodelBindPose {
        source_model_cache_key: String,
        hmodel_index: usize,
    },
}

/// Render item abstraction - equivalent to C++ SAGE RenderItem
#[derive(Debug, Clone)]
pub struct RenderItem {
    /// Object ID for debugging and tracking
    ///
    /// `INVALID_OBJECT_ID` is retained for legacy render-item consumers when
    /// `owner` is `UnboundClientDrawable`; new code must inspect `owner` when
    /// it needs to distinguish direct gameplay objects, presentation
    /// projectiles, and standalone client visuals.
    pub object_id: ObjectId,

    /// Exact ownership domain for this item.
    pub owner: RenderItemOwner,

    /// Exact ghost-only state, present only for `RenderItemOwner::W3dGhost`.
    /// Ordinary collection and FOW modifiers must leave this unset.
    pub ghost_render_state: Option<GhostRenderState>,

    /// Exact C++ W3DModelDraw source identity carried by a GameClient
    /// RenderBridge submission.  This is intentionally renderer metadata,
    /// not an ObjectID alias: the runtime Draw-module ordinal and tag remain
    /// the only stable way to distinguish equal model names from separate
    /// C++ draw modules.
    pub legacy_model_draw_source: Option<gamelogic::helpers::ModelDrawSourceIdentity>,

    /// Source-authored WeaponFireFXBone/WeaponRecoilBone/
    /// WeaponMuzzleFlash/WeaponLaunchBone bases for the selected Draw state.
    /// Keep the names intact until the receiving W3D asset validates them;
    /// this item-level bridge must never infer recoil from model names, mesh
    /// suffixes, or a broadcast WeaponDischarged marker.
    pub legacy_weapon_bone_bindings: Option<gamelogic::helpers::ModelDrawWeaponBoneBindings>,

    /// Debug name for render item
    pub debug_name: String,

    /// Source model name (used for WW3D renderer integration)
    pub model_name: String,

    /// Mesh index inside the source model
    pub mesh_index: usize,

    /// Material definition for this mesh
    pub material: W3DMaterial,

    /// World transform matrix
    pub world_matrix: Mat4,

    /// Mesh-local transform matrix
    pub mesh_local_transform: Mat4,

    /// World position (for sorting)
    pub world_position: Vec3,

    /// Distance from camera (for sorting)
    pub distance: f32,

    /// Material key for batching
    pub material_key: String,

    /// Render pass this item belongs to
    pub render_pass: RenderPass,

    /// Mesh resource key
    pub mesh_key: String,

    /// Vertex buffer range
    pub vertex_buffer_range: Option<(u32, u32)>, // (start, count)

    /// Index buffer range
    pub index_buffer_range: Option<(u32, u32)>, // (start, count)

    /// Sorting key for efficient rendering - equivalent to C++ RenderItem::SortingKey
    pub sorting_key: u64,

    /// FOW visibility data for this render item
    pub fow_visibility: ObjectVisibility,

    /// Frozen Drawable StealthLook opacity, kept separate from FOW alpha.
    /// Values below one force the renderer onto its alpha-blended path.
    pub presentation_opacity: f32,

    /// C++ direct-Drawing scene result for this item, if its full binding was
    /// accepted at the frozen Main boundary.  Objectless client drawables and
    /// ordinary GameWorld items intentionally retain `None`.
    pub frozen_direct_scene_shroud: Option<FrozenDirectSceneShroudRenderState>,

    /// C++ objectless Drawable shroud-controller result. This has no direct
    /// object owner and must not participate in direct-object culling or
    /// clear-frame history.
    pub frozen_objectless_drawable_shroud: Option<FrozenObjectlessDrawableShroudRenderState>,

    /// Per-instance UV offset override for submeshes such as W3D tread meshes.
    pub uv_offset_override: Option<Vec2>,

    pub animation_frame: f32,

    /// Exact local-or-companion HAnim frozen from the selected W3DModelDraw
    /// state. `None` means bind pose; it must never become local clip zero in
    /// the final palette path.
    pub animation_binding: Option<W3dAnimationBinding>,

    /// Ordered C++ HTree `Capture_Bone`/`Control_Bone` deltas in source W3D
    /// pivot space. These remain separate from the root world transform and
    /// are validated against the freshly loaded hierarchy before either rigid
    /// HLOD transforms or the GPU skin palette consume them.
    pub capture_bone_controls: Vec<(i32, Mat4)>,

    /// Explicit owner for GPU skinning. This is deliberately separate from
    /// `model_name`: an HMODEL skin mesh may be a strict child prototype from
    /// another source file while its palette still belongs to its HMODEL.
    pub bone_palette_source: RenderItemBonePaletteSource,

    /// C++ selection flash envelope residual intensity 0..1 (presentation-owned).
    pub selection_flash_intensity: f32,

    /// Frozen source color for the selection-flash envelope.  The current
    /// SAGE-default path is white, but retain the authored team color so a
    /// render-object child can replay the exact same frozen modifier on its
    /// own material.
    selection_flash_team_color: [f32; 4],

    /// C++ `TINT_STATUS_POISONED` presentation state.  This remains separate
    /// from the material colors because HLOD AdditionalModels have independent
    /// source materials and must replay the same frozen tint once each.
    pub poison_tinted: bool,
}

impl RenderItem {
    /// Create new render item - equivalent to C++ RenderItem constructor
    pub fn new(
        object_id: ObjectId,
        model_name: String,
        mesh_index: usize,
        world_position: Vec3,
        world_matrix: Mat4,
        material: &W3DMaterial,
        render_pass: RenderPass,
    ) -> Self {
        let mesh_key = format!("{}_{}", model_name, mesh_index);
        let distance = world_position.length();
        let texture_tag = material
            .texture_name
            .clone()
            .unwrap_or_else(|| "default".to_string());
        let material_key = format!("{}::{}", material.name, texture_tag);
        let sorting_key = Self::generate_sorting_key(render_pass, &material_key, distance);

        Self {
            object_id,
            owner: RenderItemOwner::Object(object_id),
            ghost_render_state: None,
            legacy_model_draw_source: None,
            legacy_weapon_bone_bindings: None,
            debug_name: format!("{}_{}", object_id.0, mesh_key),
            model_name,
            mesh_index,
            material: material.clone(),
            world_matrix,
            mesh_local_transform: Mat4::IDENTITY,
            world_position,
            distance,
            material_key,
            render_pass,
            mesh_key,
            vertex_buffer_range: None,
            index_buffer_range: None,
            sorting_key,
            fow_visibility: ObjectVisibility::default(),
            presentation_opacity: 1.0,
            frozen_direct_scene_shroud: None,
            frozen_objectless_drawable_shroud: None,
            uv_offset_override: None,
            animation_frame: 0.0,
            animation_binding: None,
            capture_bone_controls: Vec::new(),
            bone_palette_source: RenderItemBonePaletteSource::FrozenDrawState,
            selection_flash_intensity: 0.0,
            selection_flash_team_color: [1.0, 1.0, 1.0, 1.0],
            poison_tinted: false,
        }
    }

    /// Construct an item submitted by a standalone C++ GameClient drawable.
    ///
    /// These drawables have no gameplay object and therefore no object FOW
    /// channel.  Keeping their client ID as a first-class owner avoids the
    /// historical, incorrect DrawableID-to-ObjectID cast.
    pub fn new_unbound_client_drawable(
        drawable_id: u32,
        model_name: String,
        mesh_index: usize,
        world_position: Vec3,
        world_matrix: Mat4,
        material: &W3DMaterial,
        render_pass: RenderPass,
    ) -> Self {
        let mut item = Self::new(
            crate::game_logic::INVALID_OBJECT_ID,
            model_name,
            mesh_index,
            world_position,
            world_matrix,
            material,
            render_pass,
        );
        item.owner = RenderItemOwner::UnboundClientDrawable(drawable_id);
        item.debug_name = format!("client_drawable_{}_{}", drawable_id, item.mesh_key);
        item
    }

    /// Construct an item from a snapshot-owned projectile mesh.
    ///
    /// Presentation projectiles retain their source ID for diagnostics and
    /// normal render bookkeeping, but that ID is deliberately not an object
    /// owner: it can collide with a direct GameClient Drawable's ObjectID and
    /// must never receive that drawable's scene-shroud decision.
    pub fn new_presentation_projectile(
        projectile_id: ObjectId,
        model_name: String,
        mesh_index: usize,
        world_position: Vec3,
        world_matrix: Mat4,
        material: &W3DMaterial,
        render_pass: RenderPass,
    ) -> Self {
        let mut item = Self::new(
            projectile_id,
            model_name,
            mesh_index,
            world_position,
            world_matrix,
            material,
            render_pass,
        );
        item.owner = RenderItemOwner::PresentationProjectile(projectile_id);
        item.debug_name = format!(
            "presentation_projectile_{}_{}",
            projectile_id.0, item.mesh_key
        );
        item
    }

    /// Construct a typed ghost item from an already validated immutable
    /// snapshot.  Callers should only pass state returned by
    /// [`GhostRenderState::new`]; this option keeps the boundary fail-closed
    /// if validation is extended later.
    pub fn new_w3d_ghost(
        state: GhostRenderState,
        mesh_index: usize,
        material: &W3DMaterial,
        render_pass: RenderPass,
    ) -> Option<Self> {
        let object_id = state
            .parent_object_id
            .unwrap_or(crate::game_logic::INVALID_OBJECT_ID);
        let world_position = state.world_transform.w_axis.truncate();
        let mut item = Self::new(
            object_id,
            state.model_name.clone(),
            mesh_index,
            world_position,
            state.world_transform,
            material,
            render_pass,
        );
        item.owner = RenderItemOwner::W3dGhost(state.key);
        item.ghost_render_state = Some(state);
        item.debug_name = format!("w3d_ghost_{}_{}", item.object_id.0, item.mesh_key);
        Some(item)
    }

    /// Generate sorting key for render ordering - equivalent to C++ RenderItem::GenerateSortingKey()

    /// Apply C++ flashAsSelected residual as emissive boost (white flash default).
    pub fn apply_selection_flash(&mut self, intensity: f32, team_color: [f32; 4]) {
        let i = intensity.clamp(0.0, 1.0);
        self.selection_flash_team_color = team_color;
        if i <= 0.0 {
            self.selection_flash_intensity = 0.0;
            return;
        }
        self.selection_flash_intensity = i;
        // C++ default SelectionFlashHouseColor=false → white flash; house color optional.
        // Mix white with a touch of team color for residual house-tint option.
        let r = 1.0 * i + team_color[0] * 0.0;
        let g = 1.0 * i + team_color[1] * 0.0;
        let b = 1.0 * i + team_color[2] * 0.0;
        self.material.emissive_color.x = (self.material.emissive_color.x + r).min(2.0);
        self.material.emissive_color.y = (self.material.emissive_color.y + g).min(2.0);
        self.material.emissive_color.z = (self.material.emissive_color.z + b).min(2.0);
        // Slight diffuse lift so unlit paths still show flash residual.
        self.material.diffuse_color.x =
            (self.material.diffuse_color.x * (1.0 - 0.35 * i) + 1.0 * 0.35 * i).min(1.5);
        self.material.diffuse_color.y =
            (self.material.diffuse_color.y * (1.0 - 0.35 * i) + 1.0 * 0.35 * i).min(1.5);
        self.material.diffuse_color.z =
            (self.material.diffuse_color.z * (1.0 - 0.35 * i) + 1.0 * 0.35 * i).min(1.5);
    }

    fn generate_sorting_key(render_pass: RenderPass, material_key: &str, distance: f32) -> u64 {
        // Sorting key format (64-bit):
        // Bits 56-63: Render pass (8 bits)
        // Bits 32-55: Material hash (24 bits)
        // Bits 0-31:  Distance (32 bits, inverted for front-to-back)

        let pass_bits = (render_pass as u64) << 56;

        // Simple hash of material key
        let mut material_hash = 0u64;
        for byte in material_key.bytes() {
            material_hash = material_hash.wrapping_mul(31).wrapping_add(byte as u64);
        }
        let material_bits = (material_hash & 0xFFFFFF) << 32;

        // Distance bits (inverted for front-to-back sorting)
        let distance_u32 = (distance * 1000.0) as u32;
        let distance_bits = (!distance_u32) as u64;

        pass_bits | material_bits | distance_bits
    }

    /// Update world matrix - equivalent to C++ RenderItem::SetWorldMatrix()
    /// Wave 499: C++ TINT_STATUS_POISONED residual — greenish diffuse bias.
    pub fn apply_poison_tint(&mut self) {
        self.poison_tinted = true;
        const POISON: [f32; 3] = [0.15, 0.85, 0.20];
        const BLEND: f32 = 0.45;
        let d = &mut self.material.diffuse_color;
        d.x = d.x * (1.0 - BLEND) + POISON[0] * BLEND;
        d.y = d.y * (1.0 - BLEND) + POISON[1] * BLEND;
        d.z = d.z * (1.0 - BLEND) + POISON[2] * BLEND;
        let e = &mut self.material.emissive_color;
        e.x = (e.x + POISON[0] * 0.15).min(2.0);
        e.y = (e.y + POISON[1] * 0.25).min(2.0);
        e.z = (e.z + POISON[2] * 0.10).min(2.0);
    }

    /// Apply the frozen Drawable-level visual state shared by all render
    /// objects produced for one presentation unit.  Call this once while
    /// constructing each fresh source mesh or its synthetic HLOD parent; the
    /// latter lets independently materialled AdditionalModels replay the same
    /// state without re-reading GameLogic during rendering.
    pub fn apply_frozen_presentation_visuals(
        &mut self,
        fow_visibility: ObjectVisibility,
        selection_flash_intensity: f32,
        selection_flash_team_color: [f32; 4],
        poison_tinted: bool,
    ) {
        self.set_fow_visibility(fow_visibility);
        if selection_flash_intensity > 0.0 {
            self.apply_selection_flash(selection_flash_intensity, selection_flash_team_color);
        }
        if poison_tinted {
            self.apply_poison_tint();
        }
    }

    #[inline]
    pub fn set_presentation_opacity(&mut self, opacity: f32) {
        self.presentation_opacity = if opacity.is_finite() {
            opacity.clamp(0.0, 1.0)
        } else {
            1.0
        };
    }

    /// Replay the parent Drawable's already-frozen visual state on a new
    /// render-object child.  HLOD AdditionalModels have their own source
    /// material, so copying the parent's material itself would be incorrect.
    pub fn copy_frozen_presentation_visuals_from(&mut self, parent: &Self) {
        self.apply_frozen_presentation_visuals(
            parent.fow_visibility,
            parent.selection_flash_intensity,
            parent.selection_flash_team_color,
            parent.poison_tinted,
        );
        self.presentation_opacity = parent.presentation_opacity;
        self.frozen_objectless_drawable_shroud = parent.frozen_objectless_drawable_shroud;
        // AdditionalModels are C++ render-object children of the same
        // DrawModule submission. Carry the exact source stamp and authored
        // bases for diagnostics/future validated topology consumption, while
        // leaving all recoil dispatch inert until an exact event route exists.
        self.legacy_model_draw_source = parent.legacy_model_draw_source.clone();
        self.legacy_weapon_bone_bindings = parent.legacy_weapon_bone_bindings.clone();
    }

    pub fn set_world_matrix(&mut self, matrix: Mat4) {
        self.world_matrix = matrix;
        // Extract position from matrix
        self.world_position = Vec3::new(matrix.w_axis.x, matrix.w_axis.y, matrix.w_axis.z);
        self.distance = self.world_position.length();

        // Regenerate sorting key
        self.sorting_key =
            Self::generate_sorting_key(self.render_pass, &self.material_key, self.distance);
    }

    pub fn set_mesh_local_transform(&mut self, matrix: Mat4) {
        self.mesh_local_transform = matrix;
    }

    /// Set vertex buffer range - equivalent to C++ RenderItem::SetVertexRange()
    pub fn set_vertex_range(&mut self, start: u32, count: u32) {
        self.vertex_buffer_range = Some((start, count));
    }

    /// Set index buffer range - equivalent to C++ RenderItem::SetIndexRange()
    pub fn set_index_range(&mut self, start: u32, count: u32) {
        self.index_buffer_range = Some((start, count));
    }

    /// Get render pass
    pub fn get_render_pass(&self) -> RenderPass {
        self.render_pass
    }

    /// Get material key
    pub fn get_material_key(&self) -> &str {
        &self.material_key
    }

    /// Get mesh key
    pub fn get_mesh_key(&self) -> &str {
        &self.mesh_key
    }

    /// Set FOW visibility for this render item
    pub fn set_fow_visibility(&mut self, visibility: ObjectVisibility) {
        self.fow_visibility = visibility;
    }

    /// Retain the exact C++ direct-scene shroud result selected for this
    /// object-owned item.  The caller has already checked the full
    /// host-epoch/object/drawable/generation identity.
    pub fn set_frozen_direct_scene_shroud(&mut self, state: FrozenDirectSceneShroudRenderState) {
        self.frozen_direct_scene_shroud = Some(state);
    }

    /// Retain the frozen C++ controller result for one standalone client
    /// Drawable. The caller owns the identity validation at the Main bridge
    /// boundary; this setter never consults GameLogic or FOW.
    pub fn set_frozen_objectless_drawable_shroud(
        &mut self,
        state: FrozenObjectlessDrawableShroudRenderState,
    ) {
        self.frozen_objectless_drawable_shroud = Some(state);
    }

    /// Exact per-mesh projected-pass decision frozen by the W3D scene seam.
    /// This deliberately ignores scalar `fow_visibility`.
    #[inline]
    pub fn pushes_projected_shroud_pass(&self) -> bool {
        self.frozen_direct_scene_shroud
            .is_some_and(|state| state.pushes_projected_shroud_pass)
            || self
                .frozen_objectless_drawable_shroud
                .is_some_and(|state| state.pushes_projected_shroud_pass)
    }

    /// Get FOW visibility for this render item
    pub fn get_fow_visibility(&self) -> ObjectVisibility {
        self.fow_visibility
    }
}

/// Implement ordering for render items - equivalent to C++ RenderItem::operator<
impl PartialOrd for RenderItem {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RenderItem {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.sorting_key.cmp(&other.sorting_key)
    }
}

impl PartialEq for RenderItem {
    fn eq(&self, other: &Self) -> bool {
        self.sorting_key == other.sorting_key
    }
}

impl Eq for RenderItem {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unbound_client_drawable_keeps_its_identity_out_of_object_id_space() {
        let item = RenderItem::new_unbound_client_drawable(
            77,
            "StandaloneTracer".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );

        assert_eq!(item.object_id, crate::game_logic::INVALID_OBJECT_ID);
        assert_eq!(item.owner, RenderItemOwner::UnboundClientDrawable(77));
        assert!(item.debug_name.starts_with("client_drawable_77_"));
        assert_eq!(item.fow_visibility, ObjectVisibility::default());
        assert_eq!(item.frozen_direct_scene_shroud, None);
    }

    #[test]
    fn presentation_projectile_keeps_its_object_id_out_of_direct_drawable_ownership() {
        let projectile_id = ObjectId(77);
        let item = RenderItem::new_presentation_projectile(
            projectile_id,
            "ProjectileMesh".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );

        assert_eq!(item.object_id, projectile_id);
        assert_eq!(
            item.owner,
            RenderItemOwner::PresentationProjectile(projectile_id)
        );
        assert!(item.debug_name.starts_with("presentation_projectile_77_"));
        assert_eq!(item.fow_visibility, ObjectVisibility::default());
        assert_eq!(item.frozen_direct_scene_shroud, None);
    }

    #[test]
    fn w3d_ghost_item_keeps_stable_identity_and_dedicated_fog_route() {
        let key = GhostRenderKey {
            ghost_id: 11,
            player_index: 2,
            snapshot_index: 4,
            scene_revision: 9,
        };
        let state = GhostRenderState::new(
            key,
            Some(ObjectId(33)),
            "GhostTank".to_string(),
            Mat4::from_translation(Vec3::new(4.0, 5.0, 6.0)),
            0.75,
            [0.2, 0.3, 0.4, 0.5],
            true,
        )
        .expect("finite exact ghost state");
        let item = RenderItem::new_w3d_ghost(
            state.clone(),
            0,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        )
        .expect("validated ghost state");

        assert_eq!(item.owner, RenderItemOwner::W3dGhost(key));
        assert_eq!(item.ghost_render_state, Some(state));
        assert_eq!(item.fow_visibility, ObjectVisibility::default());
        assert_eq!(
            item.ghost_render_state
                .as_ref()
                .map(|state| state.lighting_route),
            Some(GhostLightingRoute::AlwaysFogged)
        );
    }

    #[test]
    fn ghost_contract_rejects_incomplete_or_nonfinite_payloads() {
        let key = GhostRenderKey {
            ghost_id: 1,
            player_index: 0,
            snapshot_index: 0,
            scene_revision: 1,
        };
        assert!(GhostRenderState::new(
            key,
            None,
            "".to_string(),
            Mat4::IDENTITY,
            1.0,
            [1.0; 4],
            false,
        )
        .is_none());
        assert!(GhostRenderState::new(
            key,
            None,
            "Ghost".to_string(),
            Mat4::IDENTITY,
            0.0,
            [1.0; 4],
            false,
        )
        .is_none());
        assert!(GhostRenderState::new(
            key,
            None,
            "Ghost".to_string(),
            Mat4::IDENTITY,
            1.0,
            [f32::NAN; 4],
            false,
        )
        .is_none());
    }

    #[test]
    fn ordinary_render_items_keep_the_existing_draw_state_palette_owner() {
        let item = RenderItem::new(
            ObjectId(9),
            "ordinary".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );

        assert_eq!(
            item.bone_palette_source,
            RenderItemBonePaletteSource::FrozenDrawState,
            "HMODEL palette ownership must not change ordinary rigid or animated items"
        );
    }

    #[test]
    fn direct_scene_status_is_retained_independently_from_fow_visibility() {
        let mut item = RenderItem::new(
            ObjectId(12),
            "direct".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        item.set_fow_visibility(ObjectVisibility::HIDDEN);
        item.set_frozen_direct_scene_shroud(FrozenDirectSceneShroudRenderState {
            final_status: ObjectShroudStatus::PartialClear,
            pushes_projected_shroud_pass: true,
        });

        assert_eq!(
            item.frozen_direct_scene_shroud,
            Some(FrozenDirectSceneShroudRenderState {
                final_status: ObjectShroudStatus::PartialClear,
                pushes_projected_shroud_pass: true,
            }),
            "the retained C++ scene decision must not be reconstructed from FOW alpha"
        );
        assert!(item.pushes_projected_shroud_pass());
    }

    #[test]
    fn objectless_drawable_shroud_is_keyed_and_separate_from_fow() {
        let mut item = RenderItem::new_unbound_client_drawable(
            77,
            "prisoner".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        item.set_fow_visibility(ObjectVisibility {
            visibility_alpha: 0.13,
            is_explored: 1.0,
            visibility_falloff: 1.0,
        });
        item.set_frozen_objectless_drawable_shroud(FrozenObjectlessDrawableShroudRenderState {
            drawable_id: 77,
            controller_object_id: Some(ObjectId(14)),
            controller_found: true,
            final_status: ObjectShroudStatus::Shrouded,
            pushes_projected_shroud_pass: true,
        });

        let state = item
            .frozen_objectless_drawable_shroud
            .expect("controller result retained on the standalone Drawable item");
        assert_eq!(state.drawable_id, 77);
        assert_eq!(state.controller_object_id, Some(ObjectId(14)));
        assert_eq!(state.final_status, ObjectShroudStatus::Shrouded);
        assert!(state.pushes_projected_shroud_pass);
        assert_eq!(item.fow_visibility.visibility_alpha, 0.13);
    }

    #[test]
    fn objectless_drawable_missing_controller_is_explicitly_clear() {
        let mut item = RenderItem::new_unbound_client_drawable(
            78,
            "stale_prisoner".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        item.set_frozen_objectless_drawable_shroud(FrozenObjectlessDrawableShroudRenderState {
            drawable_id: 78,
            controller_object_id: Some(ObjectId(9999)),
            controller_found: false,
            final_status: ObjectShroudStatus::Clear,
            pushes_projected_shroud_pass: false,
        });

        let state = item
            .frozen_objectless_drawable_shroud
            .expect("missing controller still yields a keyed clear fact");
        assert_eq!(state.controller_object_id, Some(ObjectId(9999)));
        assert!(!state.controller_found);
        assert_eq!(state.final_status, ObjectShroudStatus::Clear);
        assert!(!state.pushes_projected_shroud_pass);
    }
}
