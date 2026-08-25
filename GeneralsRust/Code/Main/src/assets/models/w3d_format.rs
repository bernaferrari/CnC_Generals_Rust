//! Mechanical split from `assets/models.rs`. No behavior change.
#![allow(dead_code, unused_imports)]
use super::prelude::*;
use super::w3d_anim::*;
use super::w3d_loader::*;
use super::w3d_loader_parse::*;
use super::w3d_mesh::*;
use super::w3d_mesh_build::*;
use super::w3d_model::*;
use super::*;

pub(super) const W3D_CHUNK_MESH: u32 = 0x00000000;
pub(super) const W3D_CHUNK_MESH_HEADER: u32 = 0x0000001F; // W3dMeshHeader3Struct
pub(super) const W3D_CHUNK_VERTICES: u32 = 0x00000002;
pub(super) const W3D_CHUNK_VERTEX_NORMALS: u32 = 0x00000003;
pub(super) const W3D_CHUNK_MESH_USER_TEXT: u32 = 0x0000000C;
pub(super) const W3D_CHUNK_VERTEX_INFLUENCES: u32 = 0x0000000E;
/// `sizeof(W3dVertInfStruct)` in the original MSVC W3D loader: one u16
/// `BoneIdx` followed by six opaque padding bytes, once per mesh vertex.
pub(super) const W3D_VERTEX_INFLUENCE_RECORD_SIZE: usize = 8;
pub(super) const W3D_CHUNK_TRIANGLES: u32 = 0x00000020;
pub(super) const W3D_CHUNK_VERTEX_SHADE_INDICES: u32 = 0x00000022;
pub(super) const W3D_CHUNK_MATERIAL_INFO: u32 = 0x00000028;
pub(super) const W3D_CHUNK_SHADERS: u32 = 0x00000029;
pub(super) const W3D_CHUNK_VERTEX_MATERIALS: u32 = 0x0000002A;
pub(super) const W3D_CHUNK_VERTEX_MATERIAL: u32 = 0x0000002B;
pub(super) const W3D_CHUNK_VERTEX_MATERIAL_NAME: u32 = 0x0000002C;
pub(super) const W3D_CHUNK_VERTEX_MATERIAL_INFO: u32 = 0x0000002D;
pub(super) const W3D_CHUNK_VERTEX_MAPPER_ARGS0: u32 = 0x0000002E;
pub(super) const W3D_CHUNK_VERTEX_MAPPER_ARGS1: u32 = 0x0000002F;
// Obsolete v3 material chunks from w3d_obsolete.h (still used by shipped content).
pub(super) const W3D_CHUNK_MATERIALS3: u32 = 0x00000015;
pub(super) const W3D_CHUNK_MATERIAL3: u32 = 0x00000016;
pub(super) const W3D_CHUNK_MATERIAL3_NAME: u32 = 0x00000017;
pub(super) const W3D_CHUNK_MATERIAL3_INFO: u32 = 0x00000018;
pub(super) const W3D_CHUNK_MATERIAL3_DC_MAP: u32 = 0x00000019;
pub(super) const W3D_CHUNK_MAP3_FILENAME: u32 = 0x0000001A;
pub(super) const W3D_CHUNK_MAP3_INFO: u32 = 0x0000001B;
pub(super) const W3D_CHUNK_TEXTURES: u32 = 0x00000030; // FIXED: Was 0x32
pub(super) const W3D_CHUNK_TEXTURE: u32 = 0x00000031; // FIXED: Was 0x33
pub(super) const W3D_CHUNK_TEXTURE_NAME: u32 = 0x00000032; // FIXED: Was 0x34
pub(super) const W3D_CHUNK_TEXTURE_INFO: u32 = 0x00000033; // FIXED: Was 0x35
pub(super) const W3D_CHUNK_MATERIAL_PASS: u32 = 0x00000038;
pub(super) const W3D_CHUNK_VERTEX_MATERIAL_IDS: u32 = 0x00000039;
pub(super) const W3D_CHUNK_SHADER_IDS: u32 = 0x0000003A;
pub(super) const W3D_CHUNK_DCG: u32 = 0x0000003B;
pub(super) const W3D_CHUNK_DIG: u32 = 0x0000003C;
pub(super) const W3D_CHUNK_TEXTURE_STAGE: u32 = 0x00000048;
pub(super) const W3D_CHUNK_TEXTURE_IDS: u32 = 0x00000049; // NEW: Texture index array
pub(super) const W3D_CHUNK_STAGE_TEXCOORDS: u32 = 0x0000004A;
pub(super) const W3D_CHUNK_PER_FACE_TEXCOORD_IDS: u32 = 0x0000004B;

// Additional W3D chunks
pub(super) const W3D_CHUNK_VERTEX_COLORS: u32 = 0x00000008;
pub(super) const W3D_CHUNK_TEXCOORDS: u32 = 0x00000005;
pub(super) const W3D_CHUNK_MATERIALS: u32 = 0x00000028;
pub(super) const W3D_CHUNK_HIERARCHY: u32 = 0x00000100;
pub(super) const W3D_CHUNK_ANIMATION: u32 = 0x00000200;
pub(super) const W3D_CHUNK_HMODEL: u32 = 0x00000300;
pub(super) const W3D_CHUNK_HMODEL_HEADER: u32 = 0x00000301;
pub(super) const W3D_CHUNK_HMODEL_NODE: u32 = 0x00000302;
pub(super) const W3D_CHUNK_HMODEL_COLLISION_NODE: u32 = 0x00000303;
pub(super) const W3D_CHUNK_HMODEL_SKIN_NODE: u32 = 0x00000304;
pub(super) const W3D_CHUNK_HMODEL_OBSOLETE_AUX_DATA: u32 = 0x00000305;
pub(super) const W3D_CHUNK_HMODEL_OBSOLETE_SHADOW_NODE: u32 = 0x00000306;
pub(super) const W3D_CHUNK_LODMODEL: u32 = 0x00000400;
pub(super) const W3D_CHUNK_LODMODEL_HEADER: u32 = 0x00000401;
pub(super) const W3D_CHUNK_LOD: u32 = 0x00000402;
pub(super) const W3D_CHUNK_COLLECTION: u32 = 0x00000420;
pub(super) const W3D_CHUNK_COLLECTION_HEADER: u32 = 0x00000421;
pub(super) const W3D_CHUNK_COLLECTION_OBJ_NAME: u32 = 0x00000422;
pub(super) const W3D_CHUNK_EMITTER: u32 = 0x00000500;
pub(super) const W3D_CHUNK_EMITTER_HEADER: u32 = 0x00000501;
pub(super) const W3D_CHUNK_EMITTER_INFO: u32 = 0x00000503;
pub(super) const W3D_CHUNK_EMITTER_INFOV2: u32 = 0x00000504;
pub(super) const W3D_CHUNK_AGGREGATE: u32 = 0x00000600;
pub(super) const W3D_CHUNK_AGGREGATE_HEADER: u32 = 0x00000601;
pub(super) const W3D_CHUNK_AGGREGATE_INFO: u32 = 0x00000602;
pub(super) const W3D_CHUNK_BOX: u32 = 0x00000740;
pub(super) const W3D_CHUNK_SPHERE: u32 = 0x00000741;
pub(super) const W3D_CHUNK_RING: u32 = 0x00000742;
pub(super) const W3D_CHUNK_NULL_OBJECT: u32 = 0x00000750;
pub(super) const W3D_CHUNK_DAZZLE: u32 = 0x00000900;
pub(super) const W3D_CHUNK_DAZZLE_NAME: u32 = 0x00000901;
pub(super) const W3D_CHUNK_DAZZLE_TYPENAME: u32 = 0x00000902;
pub(super) const W3D_CHUNK_POINTS: u32 = 0x00000440;
pub(super) const W3D_CHUNK_HLOD: u32 = 0x00000700;
pub(super) const W3D_CHUNK_HLOD_HEADER: u32 = 0x00000701;
pub(super) const W3D_CHUNK_HLOD_LOD_ARRAY: u32 = 0x00000702;
pub(super) const W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER: u32 = 0x00000703;
pub(super) const W3D_CHUNK_HLOD_SUB_OBJECT: u32 = 0x00000704;
pub(super) const W3D_CHUNK_HLOD_AGGREGATE_ARRAY: u32 = 0x00000705;
pub(super) const W3D_CHUNK_HLOD_PROXY_ARRAY: u32 = 0x00000706;

// Hierarchy sub-chunk types
pub(super) const W3D_CHUNK_HIERARCHY_HEADER: u32 = 0x00000101;
pub(super) const W3D_CHUNK_PIVOTS: u32 = 0x00000102;
pub(super) const W3D_CHUNK_PIVOT_FIXUPS: u32 = 0x00000103;

// Animation sub-chunk types
pub(super) const W3D_CHUNK_ANIMATION_HEADER: u32 = 0x00000201;
pub(super) const W3D_CHUNK_ANIMATION_CHANNEL: u32 = 0x00000202;
pub(super) const W3D_CHUNK_BIT_CHANNEL: u32 = 0x00000203;

// Compressed animation chunk types (timecoded and adaptive delta)
pub(super) const W3D_CHUNK_COMPRESSED_ANIMATION: u32 = 0x00000280;
pub(super) const W3D_CHUNK_COMPRESSED_ANIMATION_HEADER: u32 = 0x00000281;
pub(super) const W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL: u32 = 0x00000282;
pub(super) const W3D_CHUNK_COMPRESSED_BIT_CHANNEL: u32 = 0x00000283;

// Compressed animation flavor constants — C++ ANIM_FLAVOR_*
pub(super) const ANIM_FLAVOR_TIMECODED: u16 = 0;
pub(super) const ANIM_FLAVOR_ADAPTIVE_DELTA: u16 = 1;

/// W3D fixed-length name size (matches C++ W3D_NAME_LEN = 16)
pub(super) const W3D_NAME_LEN: usize = 16;

/// C++ `W3D_MAKE_VERSION(3, 0)`.  WW3D introduced an explicit external
/// HTree root at this file-format boundary; older hierarchy/animation records
/// must be normalized by inserting and addressing that root during load.
pub(super) const W3D_HTREE_ROOT_VERSION: u32 = 3 << 16;

/// C++ `W3D_CURRENT_HTREE_VERSION` / `W3D_CURRENT_HANIM_VERSION`.  These are
/// used only by source-shaped modern fixtures; production parsing accepts any
/// version and applies the pre-3.0 compatibility rule above where required.
pub(super) const W3D_CURRENT_HTREE_VERSION: u32 = (4 << 16) | 1;
pub(super) const W3D_CURRENT_HANIM_VERSION: u32 = (4 << 16) | 1;

/// Bone pivot from W3D hierarchy chunk. C++ parity: W3dPivotStruct (w3d_file.h:1322)
#[derive(Debug, Clone)]
pub struct W3dPivot {
    pub name: String,
    pub parent_idx: u32, // 0xFFFFFFFF = root
    pub translation: [f32; 3],
    pub euler_angles: [f32; 3],
    pub rotation: [f32; 4], // Quaternion [x,y,z,w]
}

/// W3D hierarchy data. C++ parity: W3dHierarchyStruct + W3dPivotStruct array
#[derive(Debug, Clone)]
pub struct W3dHierarchy {
    pub name: String,
    pub pivots: Vec<W3dPivot>,
    pub pivot_fixups: Vec<[[f32; 3]; 4]>,
}

/// One source-authored rigid child reference from a W3D HLOD LOD array.
///
/// C++ `W3dHLodSubObjectStruct` stores the render-object identity and the HTree
/// pivot that owns it.  The pivot is authoritative; the mesh name alone is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dHlodSubObject {
    pub name: String,
    pub bone_index: u32,
}

/// A single source-authored HLOD level.
///
/// `max_screen_size` is retained verbatim.  Main mirrors C++ construction by
/// selecting the minimum allowed level at a screen area of one pixel, but it
/// deliberately does not run C++'s normally per-view `Prepare_LOD` path: the
/// Generals RTS scene has that dynamic optimization disabled.
#[derive(Debug, Clone, PartialEq)]
pub struct W3dHlodLod {
    pub max_screen_size: f32,
    pub subobjects: Vec<W3dHlodSubObject>,
}

/// One source-authored HLOD attachment array.
///
/// C++ reuses `W3dHLodArrayHeaderStruct` and `W3dHLodSubObjectStruct` for
/// both aggregate render objects and non-rendering application proxies. Keep
/// the otherwise-unused header threshold intact rather than collapsing the
/// two array kinds into a generic boolean.
#[derive(Debug, Clone, PartialEq)]
pub struct W3dHlodAttachmentArray {
    pub max_screen_size: f32,
    pub subobjects: Vec<W3dHlodSubObject>,
}

/// One C++ `HLodClass::AdditionalModels` attachment resolved against the
/// current parent HTree pose.
///
/// `parent_transform` is in Main's render basis but remains local to the
/// parent model. A renderer must compose it with the parent's world transform
/// and draw the independently resolved child model; this type intentionally
/// does not flatten external aggregate geometry into the parent W3D file.
#[derive(Debug, Clone, PartialEq)]
pub struct W3dHlodAggregatePose {
    pub name: String,
    pub bone_index: u32,
    pub parent_transform: Mat4,
    pub visible: bool,
}

/// Bind-pose topology for one exact C++ `HLodPrototypeClass` definition.
///
/// A W3D file can register several independent `W3D_CHUNK_HLOD` records.
/// `HLodPrototypeClass::Create` constructs the definition selected by its
/// prototype token, not an inferred whole-file HLOD.  Preserve the two C++
/// render groups separately: the constructor-selected LOD children render
/// first, followed by `AdditionalModels` from the aggregate array.
///
/// Both vectors use [`W3dHlodAggregatePose`] because the render-object child
/// wire records have the same `name`/`bone_index` shape. Their ordering and
/// ownership remain distinct here so a caller cannot turn an aggregate into a
/// selected LOD child or borrow a different HLOD's hierarchy.
#[derive(Debug, Clone, PartialEq)]
pub struct W3dHlodPrototypeBindPose {
    pub selected_lod_children: Vec<W3dHlodAggregatePose>,
    pub additional_models: Vec<W3dHlodAggregatePose>,
}

/// Parsed W3D HLOD metadata.
///
/// HLOD aggregate and proxy arrays use the same source record shape as an LOD
/// array, but their semantics differ: aggregates are external render objects
/// attached to a parent HTree bone, while proxies are non-rendering
/// application-facing name/bone records.
#[derive(Debug, Clone, PartialEq)]
pub struct W3dHlod {
    pub version: u32,
    pub name: String,
    pub hierarchy_name: String,
    pub lods: Vec<W3dHlodLod>,
    pub aggregates: Option<W3dHlodAttachmentArray>,
    pub proxies: Option<W3dHlodAttachmentArray>,
    /// Whether this source HLOD still has external C++ `AdditionalModels` to
    /// resolve. The marker is presentation metadata only: C++ keeps the
    /// selected parent LOD visible while it independently creates or skips
    /// each aggregate render object. Empty aggregate arrays do not set it.
    pub has_unrendered_aggregates: bool,
    /// A malformed, repeated, or unknown trailing record makes this HLOD's
    /// authoritative source topology ambiguous. Keep its geometry disabled
    /// rather than guessing which partial array C++ happened to retain.
    pub has_invalid_trailing_records: bool,
}

/// The exact source connection kind stored in a C++ `W3D_CHUNK_HMODEL`.
///
/// `HModelDefClass` attaches all three kinds through `Create_Render_Obj`, but
/// a skin node needs its own HMODEL HTree palette at draw time.  Main retains
/// the distinction so the currently complete rigid path cannot accidentally
/// draw skinned geometry with an unrelated whole-file palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3dHmodelNodeKind {
    /// `W3D_CHUNK_NODE`: an ordinary rigid render object connection.
    Node,
    /// `W3D_CHUNK_COLLISION_NODE`: a collision render object connection.
    CollisionNode,
    /// `W3D_CHUNK_SKIN_NODE`: a skinned render object connection.
    SkinNode,
}

impl W3dHmodelNodeKind {
    pub(super) fn is_currently_rigid(self) -> bool {
        matches!(self, Self::Node | Self::CollisionNode)
    }
}

/// One C++ `HmdlNodeDefStruct` retained from an HMODEL definition.
///
/// `name` is already the exact `"<HModelName>.<RenderObjName>"` identity
/// assembled by `HModelDefClass::read_connection`; it is never a whole-file
/// fallback or a mesh-name suffix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dHmodelNode {
    pub name: String,
    pub bone_index: u32,
    pub kind: W3dHmodelNodeKind,
}

/// One source-space point retained from an HMODEL `W3D_CHUNK_POINTS` record.
///
/// `SnapPointsClass::Load_W3D` reads `W3dVectorStruct` records directly, so
/// this deliberately preserves the file's X/Y/Z basis. It is definition
/// metadata, not a render-basis vertex or an HMODEL child connection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct W3dHmodelSnapPoint {
    pub source_position: [f32; 3],
}

/// One exact C++ `HModelDefClass` source definition.
///
/// C++ creates an `HLodClass` with one LOD from this record and attaches each
/// node at its declared HTree pivot.  The definition is intentionally kept
/// separate from ordinary whole-file `W3DModel` mesh transforms.
#[derive(Debug, Clone, PartialEq)]
pub struct W3dHmodel {
    pub version: u32,
    pub name: String,
    pub hierarchy_name: String,
    pub nodes: Vec<W3dHmodelNode>,
    /// Immutable source metadata loaded by `HModelDefClass::Load_W3D` from
    /// its last `W3D_CHUNK_POINTS` record. C++ `HLodClass(HModelDefClass)`
    /// leaves its separate `SnapPoints` pointer null rather than copying this
    /// field, so this must not be exposed as active render-object ownership.
    pub source_snap_points: Vec<W3dHmodelSnapPoint>,
    /// A malformed header or node-count topology cannot safely be projected
    /// into a render tree.  Its prototype remains registered by exact name,
    /// but rendering and recursive prewarm fail closed.
    pub has_invalid_records: bool,
}

/// One rigid HMODEL node evaluated in its own default HTree pose.
///
/// The transform is local to the instantiated HMODEL.  The renderer composes
/// the parent render-object world transform before this pivot transform,
/// matching `HLodClass::Update_Sub_Object_Transforms`.
#[derive(Debug, Clone, PartialEq)]
pub struct W3dHmodelNodePose {
    pub name: String,
    pub bone_index: u32,
    pub parent_transform: Mat4,
}

/// One valid HMODEL `SKIN_NODE` connection bound to its owning HTree.
///
/// The connection's `bone_index` validates that the node belongs to the
/// HMODEL tree, but it is not an outer mesh transform. C++ skin deformation
/// reads every per-vertex bone from the container HTree; the caller must keep
/// the HMODEL attachment root separate from this local palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dHmodelSkinNodeBinding {
    pub name: String,
    pub bone_index: u32,
}

/// One exact validated C++ `WeaponBarrelInfo`-shaped hierarchy binding.
///
/// These indices remain renderer-local source topology, not save data. The
/// saved client Drawable record identifies its source Draw state; a fresh
/// `W3DModel` must rebuild and validate these indices before it can restore a
/// recoil phase against a newly loaded asset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct W3dWeaponBarrelBinding {
    /// C++ `m_fxBone`; a later numbered muzzle may reuse the previous exact
    /// FX pivot when its own indexed FX base is absent.
    pub fire_fx_pivot_index: Option<u32>,
    /// C++ `m_recoilBone`.
    pub recoil_pivot_index: Option<u32>,
    /// C++ `m_muzzleFlashBone`.
    pub muzzle_flash_pivot_index: Option<u32>,
    /// C++ `m_projectileOffsetMtx` source pivot, represented as its exact
    /// pristine HTree index until a future projectile-offset path needs the
    /// matrix itself.
    pub launch_pivot_index: Option<u32>,
}

impl W3dWeaponBarrelBinding {
    /// C++ starts visual recoil only when this barrel has a real recoil bone
    /// or muzzle flash. Fire/launch-only bindings still count as a concrete
    /// weapon barrel but must not invent recoil motion.
    pub fn has_recoil_or_muzzle(self) -> bool {
        self.recoil_pivot_index.is_some() || self.muzzle_flash_pivot_index.is_some()
    }

    pub(super) fn has_any_binding(self) -> bool {
        self.fire_fx_pivot_index.is_some()
            || self.recoil_pivot_index.is_some()
            || self.muzzle_flash_pivot_index.is_some()
            || self.launch_pivot_index.is_some()
    }
}

/// All exact W3DModelDraw barrel bindings for one currently selected Draw
/// state. The fixed three slots match C++ `WEAPONSLOT_COUNT`; each vector is
/// the ordered C++ `m_weaponBarrelInfoVec[slot]` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dWeaponBarrelTopology {
    pub slots: [Vec<W3dWeaponBarrelBinding>; 3],
}

impl W3dWeaponBarrelTopology {
    pub fn slot(&self, slot: u8) -> Option<&[W3dWeaponBarrelBinding]> {
        self.slots.get(usize::from(slot)).map(Vec::as_slice)
    }

    /// A count is meaningful only for a concrete retained C++ slot. It is
    /// capped at `u8` because the active host barrel cursor has that source
    /// width; W3DModelDraw itself scans at most 99 numbered entries.
    pub fn barrel_count(&self, slot: u8) -> Option<u8> {
        self.slot(slot)
            .and_then(|barrels| u8::try_from(barrels.len()).ok())
            .filter(|count| *count > 0)
    }
}

/// One renderer-local control reconstructed from a validated
/// [`W3dWeaponBarrelTopology`] record.  It deliberately carries only pristine
/// HTree pivot indices and already-evolved recoil state; authored names,
/// template names, and gameplay weapon identities remain outside the mesh
/// transform path.
///
/// The control vector is ordered by C++ weapon slot then barrel.  When a
/// malformed source aliases two controls to one pivot, later controls replace
/// earlier ones just like sequential `Control_Bone` calls in
/// `W3DModelDraw::handleClientRecoil`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct W3dWeaponVisualControl {
    pub recoil_pivot_index: Option<u32>,
    pub recoil_shift: f32,
    pub muzzle_flash_pivot_index: Option<u32>,
    /// C++ exposes a muzzle flash for the one visible frame in
    /// `RECOIL_START`, before it advances the recoil phase.
    pub muzzle_flash_visible: bool,
}

#[derive(Debug)]
pub(super) struct W3dChunkRef<'a> {
    pub(super) chunk_type: u32,
    pub(super) is_container: bool,
    pub(super) data: &'a [u8],
}

/// Read one chunk from a known-bounded W3D container.
///
/// The generic legacy parser tolerates malformed data to preserve its historical
/// diagnostics.  HLOD binding is authority for rigid transforms, so malformed
/// child boundaries must instead fail closed and suppress that HLOD's render path.
pub(super) fn next_w3d_chunk<'a>(
    data: &'a [u8],
    offset: &mut usize,
    context: &str,
) -> Result<Option<W3dChunkRef<'a>>> {
    if *offset == data.len() {
        return Ok(None);
    }
    if *offset + 8 > data.len() {
        return Err(anyhow!(
            "{} has {} trailing bytes without a chunk header",
            context,
            data.len().saturating_sub(*offset)
        ));
    }

    let chunk_type = u32::from_le_bytes(data[*offset..*offset + 4].try_into().unwrap());
    let raw_size = u32::from_le_bytes(data[*offset + 4..*offset + 8].try_into().unwrap());
    let is_container = (raw_size & 0x8000_0000) != 0;
    let size = (raw_size & 0x7FFF_FFFF) as usize;
    let payload_start = *offset + 8;
    let payload_end = payload_start
        .checked_add(size)
        .ok_or_else(|| anyhow!("{} chunk size overflow", context))?;
    if payload_end > data.len() {
        return Err(anyhow!(
            "{} chunk 0x{:08X} extends past its container: {} > {}",
            context,
            chunk_type,
            payload_end,
            data.len()
        ));
    }

    *offset = payload_end;
    Ok(Some(W3dChunkRef {
        chunk_type,
        is_container,
        data: &data[payload_start..payload_end],
    }))
}

/// Match `HModelDefClass::Load_W3D`'s `strncpy(..., W3D_NAME_LEN)` followed
/// by an unconditional `buffer[W3D_NAME_LEN - 1] = 0`. HMODEL header names
/// therefore retain at most fifteen source bytes even when a malformed fixed
/// field lacks a terminator; using the generic sixteen-byte W3D helper here
/// would create a prototype name C++ never registers.
pub(super) fn hmodel_cxx_header_name(bytes: &[u8]) -> String {
    let capped_len = bytes.len().min(W3D_NAME_LEN.saturating_sub(1));
    w3d_string_from_bytes(&bytes[..capped_len])
}

/// Animation channel targeting a specific bone. C++ parity: animation channel data
#[derive(Debug, Clone)]
pub struct W3dAnimChannel {
    pub first_frame: u16,
    pub last_frame: u16,
    pub vector_len: u16, // 1 for scalar (X/Y/Z), 4 for quaternion
    pub flags: u16,      // 0=X, 1=Y, 2=Z, 6=Q
    pub pivot: u16,      // Bone index
    pub data: Vec<f32>,
}

/// A raw `W3dBitChannelStruct` that drives an HTree pivot's visibility.
///
/// C++ `BitChannelClass::Get_Bit` uses the packed source bytes in LSB-first
/// order and returns `default_visible` outside the authored frame range.  The
/// type flag is retained because only `BIT_CHANNEL_VIS` (`0`) is installed by
/// `HRawAnimClass::add_bit_channel`; other raw bit-channel kinds are not a
/// visibility authority for an HLOD child.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dRawVisibilityChannel {
    pub first_frame: u16,
    pub last_frame: u16,
    pub flags: u16,
    pub pivot: u16,
    pub default_visible: bool,
    pub bits: Vec<u8>,
}

impl W3dRawVisibilityChannel {
    /// Match `BitChannelClass::Get_Bit` exactly: the caller supplies the
    /// integer raw-HAnim frame, then bit zero is the low bit of the first
    /// source byte.
    pub(super) fn visible_at(&self, frame: i32) -> bool {
        let first_frame = i32::from(self.first_frame);
        let last_frame = i32::from(self.last_frame);
        if frame < first_frame || frame > last_frame {
            return self.default_visible;
        }

        let bit =
            usize::try_from(frame - first_frame).expect("frame within a u16 W3D bit-channel range");
        let byte = self.bits.get(bit / 8).copied().unwrap_or_default();
        (byte & (1 << (bit % 8))) != 0
    }
}

/// W3D animation data. C++ parity: W3dAnimHeaderStruct + channels
#[derive(Debug, Clone)]
pub struct W3dAnimation {
    pub name: String,
    pub hierarchy_name: String,
    pub num_frames: u32,
    pub frame_rate: u32,
    /// True when this record came from a compressed animation container.
    /// Existing local support remains isolated, but hq-7x9 deliberately does
    /// not claim external companion compressed-channel parity.
    pub source_is_compressed: bool,
    pub channels: Vec<W3dAnimChannel>,
    /// Raw `BIT_CHANNEL_VIS` channels, in source chunk order.  If duplicate
    /// pivots occur, C++ overwrites the prior channel, so sampling walks this
    /// vector from the end.
    pub raw_visibility_channels: Vec<W3dRawVisibilityChannel>,
    /// A compressed/time-coded source visibility channel is intentionally not
    /// decoded yet.  `Some(pivot)` identifies a target which must not be
    /// rendered through a guessed visibility value; `None` means malformed
    /// channel metadata made the affected pivot unknowable, so every HLOD
    /// child in this animation fails closed.
    pub unsupported_visibility_pivots: Vec<Option<u16>>,
}

impl W3dAnimation {
    /// Match the name by which C++ `HAnimManagerClass` registers raw W3D
    /// motions: `<HierarchyName>.<AnimationName>`.  A Draw state carries this
    /// qualified identity, so accepting either half independently would turn a
    /// missing motion into an unrelated animation.
    pub fn matches_draw_identity(&self, identity: &str) -> bool {
        let Some((hierarchy, animation)) = split_w3d_draw_animation_identity(identity) else {
            return false;
        };
        self.hierarchy_name.eq_ignore_ascii_case(hierarchy)
            && self.name.eq_ignore_ascii_case(animation)
    }

    /// Return the source HTree visibility for one pivot, or `None` when a
    /// compressed/malformed `BIT_CHANNEL_VIS` means Main cannot safely decide
    /// whether that child should be drawn.
    pub(super) fn visibility_for_pivot(&self, pivot: usize, frame: f32) -> Option<bool> {
        // `HTreeClass::{Base,Anim}_Update` overwrites pivot zero with the
        // external RenderObj root and forces it visible before it considers
        // any source HAnim data.  Its raw W3D visibility channel (including a
        // compressed or malformed one) therefore cannot hide the object root.
        if pivot == 0 {
            return Some(true);
        }
        let frame = if self.source_is_compressed {
            if !frame.is_finite() || frame < i32::MIN as f32 || frame > i32::MAX as f32 {
                return None;
            }
            frame as i32
        } else {
            self.raw_frame_index(frame)?
        };
        let pivot = u16::try_from(pivot).ok()?;
        if self
            .unsupported_visibility_pivots
            .iter()
            .any(|unsupported| match unsupported {
                Some(value) => *value == pivot,
                None => true,
            })
        {
            return None;
        }

        self.raw_visibility_channels
            .iter()
            .rev()
            .find(|channel| channel.pivot == pivot && channel.flags == 0)
            .map(|channel| channel.visible_at(frame))
            // `HRawAnimClass::Get_Visibility` defaults to visible when this
            // animation has no visibility channel for the requested pivot.
            .or(Some(true))
    }

    /// Generals routes raw W3D HAnims through its specialized
    /// `HTreeClass::Anim_Update(HRawAnimClass*, frame)`: `Float_To_Long`
    /// obtains one integer frame, then a value at or beyond `NumFrames`
    /// wraps to zero.  The game sets x87 rounding to `_RC_NEAR`, so ties use
    /// nearest-even rather than Rust's truncation/cast behavior.  Negative
    /// frames intentionally remain negative; raw channels then provide their
    /// C++ identity/default values.
    pub(super) fn raw_frame_index(&self, frame: f32) -> Option<i32> {
        if !frame.is_finite() || self.num_frames == 0 {
            return None;
        }
        let rounded = frame.round_ties_even();
        if rounded < i32::MIN as f32 || rounded > i32::MAX as f32 {
            return None;
        }
        let frame = rounded as i32;
        (frame >= i32::try_from(self.num_frames).ok()?)
            .then_some(0)
            .or(Some(frame))
    }
}

/// Split the qualified raw-animation name C++ hands to
/// `WW3DAssetManager::Get_HAnim`.
///
/// The C++ fallback takes the substring after the *first* dot and appends
/// `.w3d`; the portion before that dot remains the exact hierarchy identity.
/// Do not accept a path here: Draw `Animation` is an HAnim identity, not an
/// archive filename escape hatch.
pub(crate) fn split_w3d_draw_animation_identity(identity: &str) -> Option<(&str, &str)> {
    let identity = identity.trim();
    let (hierarchy, animation) = identity.split_once('.')?;
    let hierarchy = hierarchy.trim();
    let animation = animation.trim();
    if hierarchy.is_empty()
        || animation.is_empty()
        || hierarchy.eq_ignore_ascii_case("none")
        || animation.eq_ignore_ascii_case("none")
        || hierarchy.contains(['/', '\\'])
        || animation.contains(['/', '\\'])
    {
        return None;
    }
    Some((hierarchy, animation))
}

/// A frozen `W3DModelDraw` animation selection carried from collection through
/// the final GPU palette upload.
///
/// C++ keeps model geometry and `HAnimClass` assets separate: on a miss,
/// `WW3DAssetManager::Get_HAnim` derives an exact companion `Animation.w3d`
/// file from `Hierarchy.Animation`.  Main therefore must not reduce a selected
/// companion to an index into the geometry file's local animation array.
#[derive(Debug, Clone)]
pub enum W3dAnimationBinding {
    /// A motion authored in the geometry W3D itself.
    Local { index: usize },
    /// A motion loaded from the exact companion W3D named by the frozen Draw
    /// identity.  The shared clip stays render-only and is not merged into the
    /// geometry model or selected through filename-family heuristics.
    Companion {
        identity: String,
        animation: Arc<W3dAnimation>,
    },
}

/// Stable state key for one selected W3D motion.  Animation playback state
/// needs identity equality without comparing raw floating-point channel data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum W3dAnimationBindingKey {
    Local(usize),
    Companion(String),
}

impl W3dAnimationBinding {
    pub fn local(index: usize) -> Self {
        Self::Local { index }
    }

    pub fn companion(identity: impl Into<String>, animation: Arc<W3dAnimation>) -> Self {
        Self::Companion {
            identity: identity.into(),
            animation,
        }
    }

    pub fn state_key(&self) -> W3dAnimationBindingKey {
        match self {
            Self::Local { index } => W3dAnimationBindingKey::Local(*index),
            Self::Companion { identity, .. } => {
                W3dAnimationBindingKey::Companion(identity.to_ascii_lowercase())
            }
        }
    }

    pub(super) fn animation<'a>(&'a self, model: &'a W3DModel) -> Option<&'a W3dAnimation> {
        match self {
            Self::Local { index } => model.animations.get(*index),
            Self::Companion { animation, .. } => Some(animation.as_ref()),
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ParsedTextureStage {
    pub(super) texture_ids: Vec<u32>,
    pub(super) texcoords: Vec<[f32; 2]>,
    pub(super) per_face_texcoord_ids: Vec<[u32; 3]>,
}

#[derive(Debug, Default)]
pub(super) struct ParsedMaterialPass {
    pub(super) stage_texture_ids: Vec<Vec<u32>>,
    pub(super) stage_texcoords: Vec<Vec<[f32; 2]>>,
    pub(super) stage_per_face_texcoord_ids: Vec<Vec<[u32; 3]>>,
    pub(super) vertex_material_ids: Vec<u32>,
    pub(super) shader_ids: Vec<u32>,
    pub(super) dcg_colors: Vec<W3dRGBAStruct>,
    pub(super) dig_colors: Vec<W3dRGBAStruct>,
}
