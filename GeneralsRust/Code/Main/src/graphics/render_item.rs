#[cfg(feature = "game_client")]
use crate::assets::W3DModel;
use crate::assets::{W3DMaterial, W3dAnimationBinding};
use crate::fow_rendering::ObjectVisibility;
use crate::game_logic::ObjectId;
#[cfg(feature = "game_client")]
use game_client::render_bridge::FrozenGhostSceneFrame;
use gamelogic::common::types::ObjectShroudStatus;
use gamelogic::object::w3d_ghost_object::Matrix3x4;
#[cfg(feature = "game_client")]
use gamelogic::object::w3d_ghost_object::{
    FrozenW3DGhostSnapshot, RenderObjectClass, RenderObjectState, W3DGhostSnapshotKey,
    W3DRenderObjectSnapshot,
};
use glam::{Mat4, Vec2, Vec3, Vec4};
#[cfg(feature = "game_client")]
use std::sync::Arc;

use super::render_pipeline::RenderPass;

/// C++ `W3DAssetManager::houseColorScale` / leftover `HOUSE_COLOR_SCALE`.
/// Brightness ramp used when remapping HouseColor / ZHC livery.
pub const HOUSE_COLOR_SCALE: [u16; 16] = [
    255, 239, 223, 211, 195, 174, 167, 151, 135, 123, 107, 91, 79, 63, 47, 35,
];

/// C++ Recolor_Mesh name test: leaf after `.` starts with `HOUSECOLOR`.
pub fn mesh_uses_house_color_vertex_material(mesh_name: &str) -> bool {
    let leaf = mesh_name.rsplit('.').next().unwrap_or(mesh_name).trim();
    leaf.len() >= 10 && leaf[..10].eq_ignore_ascii_case("HOUSECOLOR")
}

/// C++ Recolor_Mesh texture test: name starts with `ZHC`.
pub fn texture_uses_house_color_remap(texture_name: &str) -> bool {
    let file = texture_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(texture_name)
        .trim();
    file.len() >= 3 && file[..3].eq_ignore_ascii_case("ZHC")
}

/// C++ `reallycolor = (color & 0xFFFFFF) != 0` — black is not a house color.
pub fn house_color_rgb(team_color: [f32; 4]) -> Option<Vec3> {
    let r = team_color[0];
    let g = team_color[1];
    let b = team_color[2];
    if !r.is_finite() || !g.is_finite() || !b.is_finite() {
        return None;
    }
    if r.abs() <= 1e-6 && g.abs() <= 1e-6 && b.abs() <= 1e-6 {
        return None;
    }
    Some(Vec3::new(
        r.clamp(0.0, 1.0),
        g.clamp(0.0, 1.0),
        b.clamp(0.0, 1.0),
    ))
}

/// Unpack C++ `m_hexColor` / `Create_Render_Obj` ARGB into 0..1 RGBA.
pub fn house_color_from_argb(color: u32) -> Option<[f32; 4]> {
    if color & 0x00FF_FFFF == 0 {
        return None;
    }
    Some([
        ((color >> 16) & 0xff) as f32 / 255.0,
        ((color >> 8) & 0xff) as f32 / 255.0,
        (color & 0xff) as f32 / 255.0,
        1.0,
    ])
}

/// Leftover `generate_team_color_palette_32bit` shade at `index`.
pub fn house_color_palette_shade(team_color: [f32; 4], index: usize) -> Option<Vec3> {
    let rgb = house_color_rgb(team_color)?;
    let scale = HOUSE_COLOR_SCALE.get(index).copied().unwrap_or(255) as f32 / 255.0;
    Some(rgb * scale)
}

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

/// Parent suppression captured with one immutable ghost scene revision.
///
/// The GameClient bridge sorts and de-duplicates this list before freezing it.
/// Requiring that canonical order here prevents a renderer consumer from
/// silently normalizing a stale or mixed-revision handoff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GhostParentSuppression {
    scene_revision: u64,
    parent_object_ids: Vec<ObjectId>,
}

impl GhostParentSuppression {
    #[cfg(feature = "game_client")]
    pub fn from_frozen_scene(frame: &FrozenGhostSceneFrame) -> Option<Self> {
        let ids = &frame.hidden_parent_object_ids;
        if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return None;
        }
        Some(Self {
            scene_revision: frame.revision,
            parent_object_ids: ids.iter().copied().map(ObjectId).collect(),
        })
    }

    #[inline]
    pub const fn scene_revision(&self) -> u64 {
        self.scene_revision
    }

    #[inline]
    pub fn suppresses(&self, object_id: ObjectId) -> bool {
        self.parent_object_ids.binary_search(&object_id).is_ok()
    }

    #[cfg(test)]
    fn from_ids(scene_revision: u64, ids: &[u32]) -> Option<Self> {
        if ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return None;
        }
        Some(Self {
            scene_revision,
            parent_object_ids: ids.iter().copied().map(ObjectId).collect(),
        })
    }
}

/// Convert the exact row-major affine ghost payload to Main's column-major
/// `glam::Mat4`. The final affine row is structural, not a render default.
pub fn ghost_matrix3x4_to_mat4(matrix: Matrix3x4) -> Option<Mat4> {
    if !matrix.rows.iter().flatten().all(|value| value.is_finite()) {
        return None;
    }
    Some(Mat4::from_cols(
        Vec4::new(matrix.rows[0][0], matrix.rows[1][0], matrix.rows[2][0], 0.0),
        Vec4::new(matrix.rows[0][1], matrix.rows[1][1], matrix.rows[2][1], 0.0),
        Vec4::new(matrix.rows[0][2], matrix.rows[1][2], matrix.rows[2][2], 0.0),
        Vec4::new(matrix.rows[0][3], matrix.rows[1][3], matrix.rows[2][3], 1.0),
    ))
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
    /// Exact packed C++ ARGB color. Keep the source integer until a shader
    /// material path explicitly consumes it; normalized floats lose the
    /// authoritative payload identity.
    pub argb_color: u32,
    pub lighting_route: GhostLightingRoute,
    pub suppress_parent: bool,
    /// Frozen HLOD children after muzzle-hide. Mesh snapshots leave this empty.
    pub sub_objects: Vec<GhostSubObjectState>,
    /// C++ `W3DGhostObject.cpp:64-94` `disableUVAnimations`: HLOD ghosts pin
    /// LinearOffset mappers at `customUVOffset(0,0)`. Must not mutate the
    /// shared `Arc<W3DModel>` other live items still tick.
    pub uv_animations_disabled: bool,
}

/// One frozen HLOD child retained for the dedicated ghost pass.
#[derive(Debug, Clone, PartialEq)]
pub struct GhostSubObjectState {
    pub name: String,
    pub visible: bool,
    pub local_transform: Mat4,
}

impl GhostRenderState {
    pub fn new(
        key: GhostRenderKey,
        parent_object_id: Option<ObjectId>,
        model_name: String,
        world_transform: Mat4,
        object_scale: f32,
        argb_color: u32,
        suppress_parent: bool,
    ) -> Option<Self> {
        let valid_name = !model_name.trim().is_empty();
        let finite_transform = world_transform
            .to_cols_array()
            .into_iter()
            .all(|value| value.is_finite());
        let valid_scale = object_scale.is_finite() && object_scale > 0.0;
        (valid_name && finite_transform && valid_scale).then_some(Self {
            key,
            parent_object_id,
            model_name,
            world_transform,
            object_scale,
            argb_color,
            lighting_route: GhostLightingRoute::AlwaysFogged,
            suppress_parent,
            sub_objects: Vec::new(),
            uv_animations_disabled: false,
        })
    }

    /// Decode only at the eventual material boundary. The packed ARGB value
    /// remains available for exact comparisons and snapshot diagnostics.
    #[inline]
    pub fn argb_color_rgba(&self) -> [f32; 4] {
        [
            ((self.argb_color >> 16) & 0xFF) as f32 / 255.0,
            ((self.argb_color >> 8) & 0xFF) as f32 / 255.0,
            (self.argb_color & 0xFF) as f32 / 255.0,
            ((self.argb_color >> 24) & 0xFF) as f32 / 255.0,
        ]
    }

    /// Materialize one frozen Mesh or HLOD snapshot. Other classes and
    /// non-finite child transforms fail closed for that snapshot only.
    #[cfg(feature = "game_client")]
    pub fn from_frozen_snapshot(
        frame: &FrozenGhostSceneFrame,
        snapshot: &FrozenW3DGhostSnapshot,
        parent_suppression: &GhostParentSuppression,
    ) -> Option<Self> {
        if parent_suppression.scene_revision() != frame.revision {
            return None;
        }
        match snapshot.render_object.render_object.class_id {
            RenderObjectClass::Mesh | RenderObjectClass::HLod => {}
            RenderObjectClass::Other => return None,
        }

        let world_transform =
            ghost_matrix3x4_to_mat4(snapshot.render_object.render_object.transform)?;
        let parent_object_id = snapshot.parent_object_id.map(ObjectId);
        let suppress_parent = parent_object_id
            .map(|object_id| parent_suppression.suppresses(object_id))
            .unwrap_or(false);
        if parent_object_id.is_some() && !suppress_parent {
            return None;
        }

        let mut state = Self::new(
            GhostRenderKey {
                ghost_id: snapshot.key.ghost_id,
                player_index: snapshot.key.player_index,
                snapshot_index: snapshot.key.snapshot_index,
                scene_revision: frame.revision,
            },
            parent_object_id,
            snapshot.render_object.render_object.name.clone(),
            world_transform,
            snapshot.render_object.render_object.scale,
            snapshot.render_object.render_object.color,
            suppress_parent,
        )?;
        let mut sub_objects =
            Vec::with_capacity(snapshot.render_object.render_object.sub_objects.len());
        for child in &snapshot.render_object.render_object.sub_objects {
            let local_transform = ghost_matrix3x4_to_mat4(child.transform)?;
            if child.name.trim().is_empty() {
                return None;
            }
            sub_objects.push(GhostSubObjectState {
                name: child.name.clone(),
                visible: child.visible,
                local_transform,
            });
        }
        state.sub_objects = sub_objects;
        // Capture already applied muzzle-hide. Copy the UV freeze so the
        // ghost pass can pin LinearOffset without touching the shared asset.
        state.uv_animations_disabled = snapshot.render_object.uv_animations_disabled;
        Some(state)
    }
}

/// Effective LinearOffset UV for one draw. Ghosts with
/// `uv_animations_disabled` stay at the C++ `customUVOffset(0,0)` pin.
pub fn effective_linear_offset_uv(item: &RenderItem, speed: Vec2, animation_time: f32) -> Vec2 {
    if item
        .ghost_render_state
        .as_ref()
        .is_some_and(|state| state.uv_animations_disabled)
    {
        return item.uv_offset_override.unwrap_or(Vec2::ZERO);
    }
    if let Some(offset) = item.uv_offset_override {
        return offset;
    }
    speed * animation_time
}

/// A ghost item with the exact cached W3D asset retained for the item's full
/// immutable frame lifetime. This is presentation infrastructure only; the
/// ordinary FOW-bound `RenderItem` path must not consume it.
#[cfg(feature = "game_client")]
#[derive(Debug, Clone)]
pub struct MaterializedW3DGhostRenderItem {
    pub state: GhostRenderState,
    pub asset: Arc<W3DModel>,
}

/// Per-snapshot ghost materialization for one frozen scene.
#[cfg(feature = "game_client")]
#[derive(Debug, Clone)]
pub struct MaterializedW3DGhostScene {
    pub revision: u64,
    pub parent_suppression: GhostParentSuppression,
    pub items: Vec<MaterializedW3DGhostRenderItem>,
}

/// Materialize each frozen snapshot independently. A missing HLOD/Other fact
/// or asset defers that snapshot only; Mesh siblings still emit.
#[cfg(feature = "game_client")]
pub fn materialize_frozen_w3d_ghost_scene<F>(
    frame: &FrozenGhostSceneFrame,
    mut resolve_asset: F,
) -> Option<MaterializedW3DGhostScene>
where
    F: FnMut(&str) -> Option<Arc<W3DModel>>,
{
    let parent_suppression = GhostParentSuppression::from_frozen_scene(frame)?;
    let mut keys = std::collections::HashSet::with_capacity(frame.snapshots.len());
    let mut items = Vec::with_capacity(frame.snapshots.len());

    for snapshot in &frame.snapshots {
        let key = GhostRenderKey {
            ghost_id: snapshot.key.ghost_id,
            player_index: snapshot.key.player_index,
            snapshot_index: snapshot.key.snapshot_index,
            scene_revision: frame.revision,
        };
        if !keys.insert(key) {
            log::warn!(
                "w3d ghost snapshot deferred: duplicate key ghost={} player={} index={}",
                key.ghost_id,
                key.player_index,
                key.snapshot_index
            );
            continue;
        }

        let Some(state) =
            GhostRenderState::from_frozen_snapshot(frame, snapshot, &parent_suppression)
        else {
            log::warn!(
                "w3d ghost snapshot deferred: fail-closed materialize ghost={} class={:?}",
                snapshot.key.ghost_id,
                snapshot.render_object.render_object.class_id
            );
            continue;
        };
        let Some(asset) = resolve_asset(&state.model_name) else {
            log::warn!(
                "w3d ghost snapshot deferred: missing asset '{}'",
                state.model_name
            );
            continue;
        };
        if !asset.name.eq_ignore_ascii_case(&state.model_name) {
            log::warn!(
                "w3d ghost snapshot deferred: asset name mismatch '{}' vs '{}'",
                asset.name,
                state.model_name
            );
            continue;
        }
        items.push(MaterializedW3DGhostRenderItem { state, asset });
    }

    Some(MaterializedW3DGhostScene {
        revision: frame.revision,
        parent_suppression,
        items,
    })
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
///
/// Generic HLOD SKIN meshes (influences / `GEOMETRY_TYPE_SKIN`) stamp
/// [`Self::HierarchyBindPose`] so a missing HAnim still uploads the resolved
/// HTree bind pose instead of the renderer's identity 64-mat pad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderItemBonePaletteSource {
    /// Existing Draw-state binding, animation frame, and capture controls on
    /// this item provide the palette when one is explicitly selected.
    FrozenDrawState,
    /// The constructor-selected HLOD's named HTree in bind pose (or the
    /// item's frozen HAnim when that sample is available). Stamped only for
    /// SKIN meshes on the generic collect path.
    HierarchyBindPose,
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
    /// C++ Drawable tint-status signed RGB (DISABLED/SUBDUAL/FRENZY).
    pub status_tint: [f32; 3],
    /// C++ `m_hexColor` / player indicator color applied to HOUSECOLOR / ZHC.
    pub house_color: [f32; 4],
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
            status_tint: [0.0; 3],
            house_color: [0.0; 4],
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
        // C++ `Set_User_Data(&animationDisableOverride)` — per-instance pin,
        // never a write into the shared W3DModel / MeshModelClass.
        if state.uv_animations_disabled {
            item.uv_offset_override = Some(Vec2::ZERO);
        }
        item.ghost_render_state = Some(state);
        item.debug_name = format!("w3d_ghost_{}_{}", item.object_id.0, item.mesh_key);
        Some(item)
    }

    /// Generate sorting key for render ordering - equivalent to C++ RenderItem::GenerateSortingKey()

    /// Apply C++ flashAsSelected residual as emissive boost.
    /// `flash_color` is white for default `flashAsSelected()`, or the already-
    /// saturated house color when C++ passed `&myHouseColor`.
    pub fn apply_selection_flash(&mut self, intensity: f32, flash_color: [f32; 4]) {
        let i = intensity.clamp(0.0, 1.0);
        self.selection_flash_team_color = flash_color;
        if i <= 0.0 {
            self.selection_flash_intensity = 0.0;
            return;
        }
        self.selection_flash_intensity = i;
        let r = flash_color[0] * i;
        let g = flash_color[1] * i;
        let b = flash_color[2] * i;
        self.material.emissive_color.x = (self.material.emissive_color.x + r).min(2.0);
        self.material.emissive_color.y = (self.material.emissive_color.y + g).min(2.0);
        self.material.emissive_color.z = (self.material.emissive_color.z + b).min(2.0);
        // Slight diffuse lift so unlit paths still show flash residual.
        self.material.diffuse_color.x =
            (self.material.diffuse_color.x * (1.0 - 0.35 * i) + flash_color[0] * 0.35 * i).min(1.5);
        self.material.diffuse_color.y =
            (self.material.diffuse_color.y * (1.0 - 0.35 * i) + flash_color[1] * 0.35 * i).min(1.5);
        self.material.diffuse_color.z =
            (self.material.diffuse_color.z * (1.0 - 0.35 * i) + flash_color[2] * 0.35 * i).min(1.5);
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

    /// Wave 13: C++ DISABLED / SUBDUAL / FRENZY signed additive envelope sample.
    pub fn apply_status_tint(&mut self, rgb: [f32; 3]) {
        self.status_tint = rgb;
        if rgb.iter().all(|c| c.abs() < 1e-5) {
            return;
        }
        let e = &mut self.material.emissive_color;
        e.x = (e.x + rgb[0]).clamp(-1.0, 2.0);
        e.y = (e.y + rgb[1]).clamp(-1.0, 2.0);
        e.z = (e.z + rgb[2]).clamp(-1.0, 2.0);
        let d = &mut self.material.diffuse_color;
        d.x = (d.x + rgb[0]).clamp(0.0, 2.0);
        d.y = (d.y + rgb[1]).clamp(0.0, 2.0);
        d.z = (d.z + rgb[2]).clamp(0.0, 2.0);
    }

    /// C++ W3DScene heat-vision second material pass (orange emissive overlay).
    pub fn apply_heat_vision_second_pass(&mut self, opacity: f32) {
        if !opacity.is_finite() || opacity <= 0.0 {
            return;
        }
        let o = opacity.clamp(0.0, 1.0);
        const HEAT: [f32; 3] = [1.0, 0.35, 0.05];
        let e = &mut self.material.emissive_color;
        e.x = (e.x + HEAT[0] * o).min(2.0);
        e.y = (e.y + HEAT[1] * o).min(2.0);
        e.z = (e.z + HEAT[2] * o).min(2.0);
        let d = &mut self.material.diffuse_color;
        d.x = d.x * (1.0 - 0.25 * o) + HEAT[0] * 0.25 * o;
        d.y = d.y * (1.0 - 0.25 * o) + HEAT[1] * 0.25 * o;
        d.z = d.z * (1.0 - 0.25 * o) + HEAT[2] * 0.25 * o;
    }

    /// C++ Recolor_Vertex_Material / Recolor_Texture on HOUSECOLOR meshes and ZHC.
    /// Capture/owner change recolors because the next collect uses the new house color.
    pub fn apply_house_color_livery(&mut self, mesh_name: &str) {
        self.apply_house_color_livery_with(self.house_color, mesh_name);
    }

    pub fn apply_house_color_livery_with(&mut self, team_color: [f32; 4], mesh_name: &str) {
        self.house_color = team_color;
        let Some(rgb) = house_color_rgb(team_color) else {
            return;
        };
        let house_mesh = mesh_uses_house_color_vertex_material(mesh_name);
        let house_tex = self
            .material
            .texture_name
            .as_deref()
            .is_some_and(texture_uses_house_color_remap)
            || self
                .material
                .stage0_mapping
                .texture_name
                .as_deref()
                .is_some_and(texture_uses_house_color_remap);
        if house_mesh {
            // C++ Recolor_Vertex_Material: Set_Ambient + Set_Diffuse to house RGB.
            self.material.diffuse_color = rgb;
        } else if house_tex {
            // Approximate Recolor_Texture via leftover HOUSE_COLOR_SCALE[0] (255).
            self.material.diffuse_color = house_color_palette_shade(team_color, 0).unwrap_or(rgb);
        }
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
        house_color: [f32; 4],
        poison_tinted: bool,
        selection_flash_color: [f32; 4],
    ) {
        self.house_color = house_color;
        self.set_fow_visibility(fow_visibility);
        if selection_flash_intensity > 0.0 {
            self.apply_selection_flash(selection_flash_intensity, selection_flash_color);
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
            parent.house_color,
            parent.poison_tinted,
            parent.selection_flash_team_color,
        );
        self.apply_status_tint(parent.status_tint);
        self.presentation_opacity = parent.presentation_opacity;
        self.frozen_objectless_drawable_shroud = parent.frozen_objectless_drawable_shroud;
        // AdditionalModels are C++ render-object children of the same
        // DrawModule submission. Carry the exact source stamp and authored
        // bases for diagnostics/future validated topology consumption, while
        // leaving all recoil dispatch inert until an exact event route exists.
        self.legacy_model_draw_source = parent.legacy_model_draw_source.clone();
        self.legacy_weapon_bone_bindings = parent.legacy_weapon_bone_bindings.clone();
        self.house_color = parent.house_color;
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

    #[cfg(feature = "game_client")]
    fn frozen_mesh_ghost_frame(
        class_id: RenderObjectClass,
        hidden_parent_object_ids: Vec<u32>,
    ) -> FrozenGhostSceneFrame {
        let parent_object_id = hidden_parent_object_ids.first().copied();
        let render_object = RenderObjectState {
            name: "GhostTank".to_string(),
            scale: 0.75,
            color: 0x804c6680,
            transform: Matrix3x4::IDENTITY,
            sub_objects: Vec::new(),
            class_id,
        };
        FrozenGhostSceneFrame {
            revision: 7,
            snapshots: vec![FrozenW3DGhostSnapshot {
                key: W3DGhostSnapshotKey {
                    ghost_id: 11,
                    player_index: 2,
                    snapshot_index: 0,
                },
                parent_object_id,
                drawable_info: gamelogic::object::w3d_ghost_object::W3DDrawableInfo::default(),
                parent_geometry: None,
                render_object: W3DRenderObjectSnapshot::new(render_object),
            }],
            hidden_parent_object_ids,
        }
    }

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
            0x804c6680,
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
        assert_eq!(
            item.ghost_render_state
                .as_ref()
                .map(|state| state.argb_color),
            Some(0x804c6680)
        );
        let rgba = item
            .ghost_render_state
            .as_ref()
            .map(|state| state.argb_color_rgba())
            .expect("packed ARGB retained at the render boundary");
        assert_eq!(
            rgba,
            [
                0x4c as f32 / 255.0,
                0x66 as f32 / 255.0,
                0x80 as f32 / 255.0,
                0x80 as f32 / 255.0
            ]
        );
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn frozen_mesh_ghost_materializes_exact_asset_and_parent_suppression() {
        let frame = frozen_mesh_ghost_frame(RenderObjectClass::Mesh, vec![33]);
        let scene = materialize_frozen_w3d_ghost_scene(&frame, |name| {
            Some(Arc::new(W3DModel::new(name.to_string())))
        })
        .expect("a complete cached Mesh snapshot should materialize");

        assert_eq!(scene.revision, 7);
        assert!(scene.parent_suppression.suppresses(ObjectId(33)));
        assert_eq!(scene.items.len(), 1);
        let item = &scene.items[0];
        assert_eq!(item.state.key.ghost_id, 11);
        assert_eq!(item.state.parent_object_id, Some(ObjectId(33)));
        assert!(item.state.suppress_parent);
        assert_eq!(item.state.argb_color, 0x804c6680);
        assert_eq!(item.asset.name, "GhostTank");
        assert!(!item.state.uv_animations_disabled);
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn ghost_uv_animations_disabled_does_not_advance_shared_model_uv() {
        let frame = frozen_mesh_ghost_frame(RenderObjectClass::HLod, vec![33]);
        assert!(frame.snapshots[0].render_object.uv_animations_disabled);

        let model = Arc::new(W3DModel::new("GhostTank".to_string()));
        let mapper_len_before = model.meshes.first().map(|mesh| mesh.vertex_mappers.len());

        let scene = materialize_frozen_w3d_ghost_scene(&frame, |_| Some(Arc::clone(&model)))
            .expect("hlod ghost");
        let state = &scene.items[0].state;
        assert!(state.uv_animations_disabled);
        assert!(Arc::ptr_eq(&scene.items[0].asset, &model));

        let ghost =
            RenderItem::new_w3d_ghost(state.clone(), 0, &W3DMaterial::default(), RenderPass::Ghost)
                .expect("ghost item");
        assert_eq!(ghost.uv_offset_override, Some(Vec2::ZERO));

        let live = RenderItem::new(
            ObjectId(1),
            "GhostTank".into(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        let speed = Vec2::new(0.5, 0.25);
        let ghost_t0 = effective_linear_offset_uv(&ghost, speed, 0.0);
        let ghost_t1 = effective_linear_offset_uv(&ghost, speed, 1.0);
        let live_t0 = effective_linear_offset_uv(&live, speed, 0.0);
        let live_t1 = effective_linear_offset_uv(&live, speed, 1.0);

        assert_eq!(ghost_t0, Vec2::ZERO);
        assert_eq!(ghost_t1, ghost_t0);
        assert_ne!(live_t0, live_t1);
        assert_eq!(live_t1, speed * 1.0);
        assert_eq!(
            model.meshes.first().map(|mesh| mesh.vertex_mappers.len()),
            mapper_len_before
        );
        assert!(Arc::ptr_eq(&scene.items[0].asset, &model));
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn frozen_ghost_materializer_keeps_mesh_when_hlod_sibling_defers() {
        let hlod = frozen_mesh_ghost_frame(RenderObjectClass::HLod, vec![33]);
        let scene = materialize_frozen_w3d_ghost_scene(&hlod, |name| {
            Some(Arc::new(W3DModel::new(name.to_string())))
        })
        .expect("valid HLOD snapshot materializes independently");
        assert_eq!(scene.items.len(), 1);

        let mut mixed = frozen_mesh_ghost_frame(RenderObjectClass::Mesh, vec![33]);
        mixed.snapshots.push(FrozenW3DGhostSnapshot {
            key: W3DGhostSnapshotKey {
                ghost_id: 12,
                player_index: 2,
                snapshot_index: 1,
            },
            parent_object_id: Some(33),
            drawable_info: gamelogic::object::w3d_ghost_object::W3DDrawableInfo::default(),
            parent_geometry: None,
            render_object: W3DRenderObjectSnapshot::new(RenderObjectState {
                name: "MissingHlod".to_string(),
                scale: 1.0,
                color: 0xffff_ffff,
                transform: Matrix3x4::IDENTITY,
                sub_objects: Vec::new(),
                class_id: RenderObjectClass::HLod,
            }),
        });
        let mixed_scene = materialize_frozen_w3d_ghost_scene(&mixed, |name| {
            name.eq_ignore_ascii_case("GhostTank")
                .then(|| Arc::new(W3DModel::new(name.to_string())))
        })
        .expect("Mesh sibling survives a deferred HLOD");
        assert_eq!(mixed_scene.items.len(), 1);
        assert_eq!(mixed_scene.items[0].state.model_name, "GhostTank");

        let mut unsorted = frozen_mesh_ghost_frame(RenderObjectClass::Mesh, vec![33, 12]);
        unsorted.snapshots[0].parent_object_id = Some(33);
        assert!(
            materialize_frozen_w3d_ghost_scene(&unsorted, |_| {
                Some(Arc::new(W3DModel::new("GhostTank".to_string())))
            })
            .is_none()
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
        assert!(
            GhostRenderState::new(
                key,
                None,
                "".to_string(),
                Mat4::IDENTITY,
                1.0,
                0xffff_ffff,
                false,
            )
            .is_none()
        );
        assert!(
            GhostRenderState::new(
                key,
                None,
                "Ghost".to_string(),
                Mat4::IDENTITY,
                f32::NAN,
                0xffff_ffff,
                false,
            )
            .is_none()
        );
        assert!(
            GhostRenderState::new(
                key,
                None,
                "Ghost".to_string(),
                Mat4::from_cols_array(&[f32::NAN; 16]),
                1.0,
                0xffff_ffff,
                false,
            )
            .is_none()
        );
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

    #[test]
    fn house_color_recolors_housecolor_mesh_not_hull() {
        let gold = [0.87, 0.89, 0.05, 1.0];
        let mut stripe = RenderItem::new(
            ObjectId(1),
            "AVTank".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        stripe.apply_house_color_livery_with(gold, "AVTank.HOUSECOLOR");
        assert!((stripe.material.diffuse_color.x - gold[0]).abs() < 1e-5);
        assert!((stripe.material.diffuse_color.y - gold[1]).abs() < 1e-5);
        assert!((stripe.material.diffuse_color.z - gold[2]).abs() < 1e-5);

        let mut hull = RenderItem::new(
            ObjectId(1),
            "AVTank".to_string(),
            1,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &W3DMaterial::default(),
            RenderPass::ForwardOpaque,
        );
        hull.apply_house_color_livery_with(gold, "AVTank.HULL");
        assert_eq!(hull.material.diffuse_color, Vec3::ONE);
    }

    #[test]
    fn house_color_tints_zhc_texture_and_capture_recolors() {
        let mut material = W3DMaterial::default();
        material.texture_name = Some("ZHCstripe.tga".to_string());
        let mut item = RenderItem::new(
            ObjectId(2),
            "CIRanger".to_string(),
            0,
            Vec3::ZERO,
            Mat4::IDENTITY,
            &material,
            RenderPass::ForwardOpaque,
        );
        let red = [0.8, 0.1, 0.1, 1.0];
        item.apply_house_color_livery_with(red, "CIRanger.MESH");
        assert!((item.material.diffuse_color.x - red[0]).abs() < 1e-5);
        assert!((item.material.diffuse_color.y - red[1]).abs() < 1e-5);

        let blue = [0.1, 0.25, 0.9, 1.0];
        item.apply_house_color_livery_with(blue, "CIRanger.MESH");
        assert!((item.material.diffuse_color.z - blue[2]).abs() < 1e-5);
        assert!(item.material.diffuse_color.x < 0.2);
    }

    #[test]
    fn house_color_scale_matches_cpp_and_black_is_invalid() {
        assert_eq!(HOUSE_COLOR_SCALE[0], 255);
        assert_eq!(HOUSE_COLOR_SCALE[15], 35);
        assert!(house_color_rgb([0.0, 0.0, 0.0, 1.0]).is_none());
        assert!(house_color_from_argb(0xFF00_0000).is_none());
        let packed = house_color_from_argb(0xFFCC_3311).expect("nonzero RGB");
        assert!((packed[0] - 204.0 / 255.0).abs() < 1e-5);
        assert!((packed[2] - 17.0 / 255.0).abs() < 1e-5);
    }
}
