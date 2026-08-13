////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// W3D model loading system for real C&C 3D assets

use crate::assets::archive::ArchiveFileSystem;
use crate::assets::ini_parser::{
    AuthoredDrawPrimaryTurret, AuthoredDrawSubobjectVisibility, AuthoredDrawWeaponBoneBindings,
    AuthoredDrawWeaponBoneSlot,
};
use anyhow::{anyhow, Result};
use crc32fast::Hasher;
use glam::{Mat4, Vec3};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use ww3d_assets::prototypes::{MaterialPassInfo, VertexMapperConfig};
use ww3d_core::w3d_format::{
    w3d_string_from_bytes, W3dMeshHeader3Struct, W3dRGBAStruct, W3dShaderStruct, W3dVertInfStruct,
    W3dVertexMaterialStruct,
};
use ww3d_renderer_3d::rendering::mesh_system::MeshModelClass;

/// W3D file format constants based on C++ w3d_file.h
const W3D_CHUNK_MESH: u32 = 0x00000000;
const W3D_CHUNK_MESH_HEADER: u32 = 0x0000001F; // W3dMeshHeader3Struct
const W3D_CHUNK_VERTICES: u32 = 0x00000002;
const W3D_CHUNK_VERTEX_NORMALS: u32 = 0x00000003;
const W3D_CHUNK_MESH_USER_TEXT: u32 = 0x0000000C;
const W3D_CHUNK_VERTEX_INFLUENCES: u32 = 0x0000000E;
/// `sizeof(W3dVertInfStruct)` in the original MSVC W3D loader: one u16
/// `BoneIdx` followed by six opaque padding bytes, once per mesh vertex.
const W3D_VERTEX_INFLUENCE_RECORD_SIZE: usize = 8;
const W3D_CHUNK_TRIANGLES: u32 = 0x00000020;
const W3D_CHUNK_VERTEX_SHADE_INDICES: u32 = 0x00000022;
const W3D_CHUNK_MATERIAL_INFO: u32 = 0x00000028;
const W3D_CHUNK_SHADERS: u32 = 0x00000029;
const W3D_CHUNK_VERTEX_MATERIALS: u32 = 0x0000002A;
const W3D_CHUNK_VERTEX_MATERIAL: u32 = 0x0000002B;
const W3D_CHUNK_VERTEX_MATERIAL_NAME: u32 = 0x0000002C;
const W3D_CHUNK_VERTEX_MATERIAL_INFO: u32 = 0x0000002D;
const W3D_CHUNK_VERTEX_MAPPER_ARGS0: u32 = 0x0000002E;
const W3D_CHUNK_VERTEX_MAPPER_ARGS1: u32 = 0x0000002F;
// Obsolete v3 material chunks from w3d_obsolete.h (still used by shipped content).
const W3D_CHUNK_MATERIALS3: u32 = 0x00000015;
const W3D_CHUNK_MATERIAL3: u32 = 0x00000016;
const W3D_CHUNK_MATERIAL3_NAME: u32 = 0x00000017;
const W3D_CHUNK_MATERIAL3_INFO: u32 = 0x00000018;
const W3D_CHUNK_MATERIAL3_DC_MAP: u32 = 0x00000019;
const W3D_CHUNK_MAP3_FILENAME: u32 = 0x0000001A;
const W3D_CHUNK_MAP3_INFO: u32 = 0x0000001B;
const W3D_CHUNK_TEXTURES: u32 = 0x00000030; // FIXED: Was 0x32
const W3D_CHUNK_TEXTURE: u32 = 0x00000031; // FIXED: Was 0x33
const W3D_CHUNK_TEXTURE_NAME: u32 = 0x00000032; // FIXED: Was 0x34
const W3D_CHUNK_TEXTURE_INFO: u32 = 0x00000033; // FIXED: Was 0x35
const W3D_CHUNK_MATERIAL_PASS: u32 = 0x00000038;
const W3D_CHUNK_VERTEX_MATERIAL_IDS: u32 = 0x00000039;
const W3D_CHUNK_SHADER_IDS: u32 = 0x0000003A;
const W3D_CHUNK_DCG: u32 = 0x0000003B;
const W3D_CHUNK_DIG: u32 = 0x0000003C;
const W3D_CHUNK_TEXTURE_STAGE: u32 = 0x00000048;
const W3D_CHUNK_TEXTURE_IDS: u32 = 0x00000049; // NEW: Texture index array
const W3D_CHUNK_STAGE_TEXCOORDS: u32 = 0x0000004A;
const W3D_CHUNK_PER_FACE_TEXCOORD_IDS: u32 = 0x0000004B;

// Additional W3D chunks
const W3D_CHUNK_VERTEX_COLORS: u32 = 0x00000008;
const W3D_CHUNK_TEXCOORDS: u32 = 0x00000005;
const W3D_CHUNK_MATERIALS: u32 = 0x00000028;
const W3D_CHUNK_HIERARCHY: u32 = 0x00000100;
const W3D_CHUNK_ANIMATION: u32 = 0x00000200;
const W3D_CHUNK_HMODEL: u32 = 0x00000300;
const W3D_CHUNK_HMODEL_HEADER: u32 = 0x00000301;
const W3D_CHUNK_HMODEL_NODE: u32 = 0x00000302;
const W3D_CHUNK_HMODEL_COLLISION_NODE: u32 = 0x00000303;
const W3D_CHUNK_HMODEL_SKIN_NODE: u32 = 0x00000304;
const W3D_CHUNK_HMODEL_OBSOLETE_AUX_DATA: u32 = 0x00000305;
const W3D_CHUNK_HMODEL_OBSOLETE_SHADOW_NODE: u32 = 0x00000306;
const W3D_CHUNK_LODMODEL: u32 = 0x00000400;
const W3D_CHUNK_POINTS: u32 = 0x00000440;
const W3D_CHUNK_HLOD: u32 = 0x00000700;
const W3D_CHUNK_HLOD_HEADER: u32 = 0x00000701;
const W3D_CHUNK_HLOD_LOD_ARRAY: u32 = 0x00000702;
const W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER: u32 = 0x00000703;
const W3D_CHUNK_HLOD_SUB_OBJECT: u32 = 0x00000704;
const W3D_CHUNK_HLOD_AGGREGATE_ARRAY: u32 = 0x00000705;
const W3D_CHUNK_HLOD_PROXY_ARRAY: u32 = 0x00000706;

// Hierarchy sub-chunk types
const W3D_CHUNK_HIERARCHY_HEADER: u32 = 0x00000101;
const W3D_CHUNK_PIVOTS: u32 = 0x00000102;
const W3D_CHUNK_PIVOT_FIXUPS: u32 = 0x00000103;

// Animation sub-chunk types
const W3D_CHUNK_ANIMATION_HEADER: u32 = 0x00000201;
const W3D_CHUNK_ANIMATION_CHANNEL: u32 = 0x00000202;
const W3D_CHUNK_BIT_CHANNEL: u32 = 0x00000203;

// Compressed animation chunk types (timecoded and adaptive delta)
const W3D_CHUNK_COMPRESSED_ANIMATION: u32 = 0x00000280;
const W3D_CHUNK_COMPRESSED_ANIMATION_HEADER: u32 = 0x00000281;
const W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL: u32 = 0x00000282;
const W3D_CHUNK_COMPRESSED_BIT_CHANNEL: u32 = 0x00000283;

// Compressed animation flavor constants — C++ ANIM_FLAVOR_*
const ANIM_FLAVOR_TIMECODED: u16 = 0;
const ANIM_FLAVOR_ADAPTIVE_DELTA: u16 = 1;

/// W3D fixed-length name size (matches C++ W3D_NAME_LEN = 16)
const W3D_NAME_LEN: usize = 16;

/// C++ `W3D_MAKE_VERSION(3, 0)`.  WW3D introduced an explicit external
/// HTree root at this file-format boundary; older hierarchy/animation records
/// must be normalized by inserting and addressing that root during load.
const W3D_HTREE_ROOT_VERSION: u32 = 3 << 16;

/// C++ `W3D_CURRENT_HTREE_VERSION` / `W3D_CURRENT_HANIM_VERSION`.  These are
/// used only by source-shaped modern fixtures; production parsing accepts any
/// version and applies the pre-3.0 compatibility rule above where required.
const W3D_CURRENT_HTREE_VERSION: u32 = (4 << 16) | 1;
const W3D_CURRENT_HANIM_VERSION: u32 = (4 << 16) | 1;

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
    fn is_currently_rigid(self) -> bool {
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

    fn has_any_binding(self) -> bool {
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
struct W3dChunkRef<'a> {
    chunk_type: u32,
    is_container: bool,
    data: &'a [u8],
}

/// Read one chunk from a known-bounded W3D container.
///
/// The generic legacy parser tolerates malformed data to preserve its historical
/// diagnostics.  HLOD binding is authority for rigid transforms, so malformed
/// child boundaries must instead fail closed and suppress that HLOD's render path.
fn next_w3d_chunk<'a>(
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
fn hmodel_cxx_header_name(bytes: &[u8]) -> String {
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
    fn visible_at(&self, frame: i32) -> bool {
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
    fn visibility_for_pivot(&self, pivot: usize, frame: f32) -> Option<bool> {
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
    fn raw_frame_index(&self, frame: f32) -> Option<i32> {
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

    fn animation<'a>(&'a self, model: &'a W3DModel) -> Option<&'a W3dAnimation> {
        match self {
            Self::Local { index } => model.animations.get(*index),
            Self::Companion { animation, .. } => Some(animation.as_ref()),
        }
    }
}

#[derive(Debug, Default)]
struct ParsedTextureStage {
    texture_ids: Vec<u32>,
    texcoords: Vec<[f32; 2]>,
    per_face_texcoord_ids: Vec<[u32; 3]>,
}

#[derive(Debug, Default)]
struct ParsedMaterialPass {
    stage_texture_ids: Vec<Vec<u32>>,
    stage_texcoords: Vec<Vec<[f32; 2]>>,
    stage_per_face_texcoord_ids: Vec<Vec<[u32; 3]>>,
    vertex_material_ids: Vec<u32>,
    shader_ids: Vec<u32>,
    dcg_colors: Vec<W3dRGBAStruct>,
    dig_colors: Vec<W3dRGBAStruct>,
}

// Mesh types
const W3D_MESH_FLAG_NONE: u32 = 0;
const W3D_MESH_FLAG_HIDDEN: u32 = 0x00000001;
const W3D_MESH_FLAG_TWO_SIDED: u32 = 0x00000002;
const W3D_MESH_FLAG_CAST_SHADOW: u32 = 0x00000004;
const W3D_MESH_FLAG_GEOMETRY_TYPE_MASK: u32 = 0x00FF0000;
const W3D_MESH_FLAG_GEOMETRY_TYPE_NORMAL: u32 = 0x00000000;
const W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED: u32 = 0x00010000;
const W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN: u32 = 0x00020000;

/// C++ SAGE engine compatible vertex data - internal format for W3D loading
/// This gets converted to VertexXYZNDUV2 for rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct W3DVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl W3DVertex {
    /// Convert to C++ SAGE VertexFormatXYZNDUV2 format for rendering
    pub fn to_sage_vertex(&self, material_color: Vec3) -> crate::cnc_game_engine::VertexXYZNDUV2 {
        // Pack diffuse color as RGBA bytes (D3D8 style)
        let r = ((self.color[0] * material_color.x * 255.0) as u32).min(255);
        let g = ((self.color[1] * material_color.y * 255.0) as u32).min(255);
        let b = ((self.color[2] * material_color.z * 255.0) as u32).min(255);
        let a = ((self.color[3] * 255.0) as u32).min(255);
        let diffuse_packed = (a << 24) | (r << 16) | (g << 8) | b;

        crate::cnc_game_engine::VertexXYZNDUV2 {
            position: self.position,
            normal: self.normal,
            diffuse: diffuse_packed,
            tex_coords0: self.uv,    // Primary texture coordinates
            tex_coords1: [0.0, 0.0], // Secondary UV for multi-texturing
        }
    }
}

/// Map W3D shader blend factors to BlendMode — matches C++ W3DSHADER_SRCBLENDFUNC_*
/// and W3DSHADER_DESTBLENDFUNC_* constants from w3d_file.h.
///
/// C++ W3D src blend constants:
///   0 = ZERO, 1 = ONE (default), 2 = SRC_ALPHA, 3 = ONE_MINUS_SRC_ALPHA
/// C++ W3D dest blend constants:
///   0 = ZERO (default), 1 = ONE, 2 = SRC_COLOR, 3 = ONE_MINUS_SRC_COLOR,
///   4 = SRC_ALPHA, 5 = ONE_MINUS_SRC_ALPHA, 6 = SRC_COLOR_PREFOG
fn shader_blend_to_mode(src_blend: u8, dest_blend: u8, alpha_test: u8) -> (BlendMode, bool) {
    let alpha_test_enabled = alpha_test != 0;

    match (src_blend, dest_blend) {
        // Opaque (default shader state): src=ONE, dest=ZERO
        (1, 0) | (0, 0) => (BlendMode::Opaque, alpha_test_enabled),

        // Standard alpha blending: src=SRC_ALPHA, dest=ONE_MINUS_SRC_ALPHA
        (2, 5) => (BlendMode::Alpha, alpha_test_enabled),

        // Additive: src=ONE, dest=ONE (full additive)
        (1, 1) => (BlendMode::Additive, alpha_test_enabled),

        // Additive with alpha: src=SRC_ALPHA, dest=ONE
        (2, 1) => (BlendMode::Additive, alpha_test_enabled),

        // Modulate (multiply): src combined with dest=SRC_COLOR or ONE_MINUS_SRC_COLOR
        (_, 2) | (_, 3) => (BlendMode::Modulate, alpha_test_enabled),

        // Alpha-blended with dest=SRC_ALPHA
        (_, 4) => (BlendMode::Alpha, alpha_test_enabled),

        // Any other non-zero dest blend → treat as alpha blend
        (_, d) if d != 0 => (BlendMode::Alpha, alpha_test_enabled),

        // Fallback: opaque
        _ => (BlendMode::Opaque, alpha_test_enabled),
    }
}

fn w3d_position_to_world(position: [f32; 3]) -> [f32; 3] {
    // Legacy W3D content is authored in X/Y ground with Z-up. The active Rust world
    // uses X/Z ground with Y-up, so swap the vertical and depth axes on import.
    [position[0], position[2], position[1]]
}

fn w3d_normal_to_world(normal: [f32; 3]) -> [f32; 3] {
    [normal[0], normal[2], normal[1]]
}

fn push_world_space_triangle(indices: &mut Vec<u32>, a: u32, b: u32, c: u32) {
    // Swapping Y/Z to move legacy W3D content into Rust's Y-up world flips handedness.
    // Mirror the C++ visible winding by reversing triangle order at import time so
    // backface culling in the WW3D renderer keeps the same observable result.
    indices.push(a);
    indices.push(c);
    indices.push(b);
}

/// W3D material information - matches C++ VertexMaterialClass exactly
#[derive(Debug, Clone)]
pub struct W3DMaterial {
    pub name: String,
    pub diffuse_color: Vec3,  // Color reflected when illuminated by lighting
    pub specular_color: Vec3, // Sharp, concentrated reflective highlights
    pub emissive_color: Vec3, // Self-illumination color (glow)
    pub shininess: f32,       // Specular power (higher = sharper highlights)
    pub opacity: f32,         // Transparency: 1.0 = opaque, 0.0 = transparent
    pub texture_name: Option<String>,

    // C++ VertexMaterialClass multi-stage texture mapping properties
    pub stage0_mapping: TextureStageMapping,
    pub stage1_mapping: Option<TextureStageMapping>,
    pub stage2_mapping: Option<TextureStageMapping>,
    pub stage3_mapping: Option<TextureStageMapping>,

    // BumpEnv vertex material mapping (for normal/bump mapping)
    pub bump_rotation: f32, // Bump texture rotation
    pub bump_scale: f32,    // Bump effect intensity
    pub u_per_sec: f32,     // U coordinate animation speed
    pub v_per_sec: f32,     // V coordinate animation speed
    pub u_scale: f32,       // U coordinate scaling
    pub v_scale: f32,       // V coordinate scaling

    // Shader blending modes for transparency and alpha testing
    pub blend_mode: BlendMode,
    pub alpha_test_enabled: bool,
    pub alpha_reference: f32,
}

/// Texture stage mapping - matches C++ texture stage system
#[derive(Debug, Clone)]
pub struct TextureStageMapping {
    pub texture_name: Option<String>,
    pub uv_source: UVSource, // Which UV set to use
    pub blend_mode: TextureBlendMode,
    pub address_u: TextureAddressMode,
    pub address_v: TextureAddressMode,
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    pub mip_filter: TextureFilter,
}

/// UV coordinate source for multi-UV models
#[derive(Debug, Clone, Copy)]
pub enum UVSource {
    UV0, // Primary texture coordinates
    UV1, // Secondary texture coordinates
    UV2, // Tertiary texture coordinates
    UV3, // Quaternary texture coordinates
}

/// Texture blending modes - matches C++ shader blending
#[derive(Debug, Clone, Copy)]
pub enum TextureBlendMode {
    Replace,  // Replace previous stage
    Modulate, // Multiply with previous stage
    Add,      // Add to previous stage
    Subtract, // Subtract from previous stage
    Blend,    // Alpha blend with previous stage
}

/// Material blending modes for transparency
#[derive(Debug, Clone, Copy)]
pub enum BlendMode {
    Opaque,   // No blending (solid)
    Alpha,    // Standard alpha blending
    Additive, // Additive blending (for effects)
    Modulate, // Multiplicative blending
}

/// Texture addressing modes
#[derive(Debug, Clone, Copy)]
pub enum TextureAddressMode {
    Wrap,   // Repeat texture
    Clamp,  // Clamp to edge
    Mirror, // Mirror texture
}

/// Texture filtering modes
#[derive(Debug, Clone, Copy)]
pub enum TextureFilter {
    Point,       // Nearest neighbor
    Linear,      // Linear interpolation
    Anisotropic, // Anisotropic filtering
}

impl Default for W3DMaterial {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            diffuse_color: Vec3::new(1.0, 1.0, 1.0), // Pure white like C++ original
            specular_color: Vec3::new(0.0, 0.0, 0.0), // Black specular like C++ original
            emissive_color: Vec3::ZERO,
            shininess: 0.0, // C++ default shininess
            opacity: 1.0,
            texture_name: None,

            // Default texture stage 0 mapping
            stage0_mapping: TextureStageMapping::default(),
            stage1_mapping: None,
            stage2_mapping: None,
            stage3_mapping: None,

            // Default BumpEnv properties
            bump_rotation: 0.0,
            bump_scale: 1.0,
            u_per_sec: 0.0,
            v_per_sec: 0.0,
            u_scale: 1.0,
            v_scale: 1.0,

            // Default blending
            blend_mode: BlendMode::Opaque,
            alpha_test_enabled: false,
            alpha_reference: 0.5,
        }
    }
}

impl Default for TextureStageMapping {
    fn default() -> Self {
        Self {
            texture_name: None,
            uv_source: UVSource::UV0,
            blend_mode: TextureBlendMode::Replace,
            address_u: TextureAddressMode::Wrap,
            address_v: TextureAddressMode::Wrap,
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Linear,
            mip_filter: TextureFilter::Linear,
        }
    }
}

/// W3D mesh data
#[derive(Debug, Clone)]
pub struct W3DMesh {
    pub name: String,
    /// Exact source `W3dMeshHeader3Struct::ContainerName`.  HLOD child binding
    /// requires this authored identity; `name` alone is not authority.
    pub container_name: String,
    pub vertices: Vec<W3DVertex>,
    pub indices: Vec<u32>,
    pub material: W3DMaterial,
    pub transform: Mat4,
    pub header: Option<W3dMeshHeader3Struct>,
    pub stage_texcoords: Vec<Vec<[f32; 2]>>,
    pub passes: Vec<MaterialPassInfo>,
    pub per_pass_stage_texture_ids: Vec<Vec<Vec<u32>>>,
    pub per_pass_stage_texture_names: Vec<Vec<Vec<String>>>,
    pub per_pass_vertex_material_ids: Vec<Vec<u32>>,
    pub per_pass_shader_ids: Vec<Vec<u32>>,
    pub per_pass_dcg_colors: Vec<Vec<W3dRGBAStruct>>,
    pub per_pass_dig_colors: Vec<Vec<W3dRGBAStruct>>,
    pub vertex_materials: Vec<W3dVertexMaterialStruct>,
    pub shaders: Vec<W3dShaderStruct>,
    pub vertex_influences: Option<Vec<W3dVertInfStruct>>,
    pub vertex_shade_indices: Option<Vec<u32>>,
    pub per_stage_face_texcoord_ids: Vec<Vec<[u32; 3]>>,
    pub stage_uv_channels: Vec<u8>,
    pub texture_library: Vec<String>,
    pub vertex_mappers: Vec<VertexMapperConfig>,
    pub vertices_in_render_space: bool,
    pub has_explicit_vertex_colors: bool,
}

impl W3DMesh {
    pub fn new(name: String) -> Self {
        Self {
            name,
            container_name: String::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            material: W3DMaterial::default(),
            transform: Mat4::IDENTITY,
            header: None,
            stage_texcoords: Vec::new(),
            passes: Vec::new(),
            per_pass_stage_texture_ids: Vec::new(),
            per_pass_stage_texture_names: Vec::new(),
            per_pass_vertex_material_ids: Vec::new(),
            per_pass_shader_ids: Vec::new(),
            per_pass_dcg_colors: Vec::new(),
            per_pass_dig_colors: Vec::new(),
            vertex_materials: Vec::new(),
            shaders: Vec::new(),
            vertex_influences: None,
            vertex_shade_indices: None,
            per_stage_face_texcoord_ids: Vec::new(),
            stage_uv_channels: Vec::new(),
            texture_library: Vec::new(),
            vertex_mappers: Vec::new(),
            vertices_in_render_space: false,
            has_explicit_vertex_colors: false,
        }
    }

    pub fn texture_name_from_library(&self, texture_id: u32) -> Option<&str> {
        if texture_id == u32::MAX {
            return None;
        }
        self.texture_library
            .get(texture_id as usize)
            .map(|name| name.as_str())
            .filter(|name| !name.is_empty())
    }

    /// Whether this source mesh has a complete, safe skin declaration for an
    /// HMODEL palette of `palette_len` pivots.
    ///
    /// C++ `MeshGeometryClass::read_vertex_influences` reads exactly one
    /// `W3dVertInfStruct` per vertex and sets `SKIN` only after that succeeds.
    /// Until Main's importer retains that exact chunk, an HMODEL `SKIN_NODE`
    /// must stay absent rather than drawing the mesh with a guessed rigid
    /// transform or palette. Every influence must also address the HMODEL's
    /// own palette; a foreign/out-of-range bone is not recoverable safely.
    pub fn has_complete_skin_influences_for_palette(&self, palette_len: usize) -> bool {
        if palette_len == 0 || self.vertices.is_empty() {
            return false;
        }
        let Some(influences) = self.vertex_influences.as_ref() else {
            return false;
        };
        influences.len() == self.vertices.len()
            && influences
                .iter()
                .all(|influence| usize::from(influence.bone_idx) < palette_len)
    }

    pub fn stage_texture_names_from_ids(
        &self,
        pass_index: usize,
        stage_index: usize,
    ) -> Vec<String> {
        self.per_pass_stage_texture_ids
            .get(pass_index)
            .and_then(|stages| stages.get(stage_index))
            .map(|ids| {
                ids.iter()
                    .filter_map(|tex_id| self.texture_name_from_library(*tex_id))
                    .map(|name| name.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Complete W3D model
#[derive(Debug, Clone)]
pub struct W3DModel {
    pub name: String,
    pub meshes: Vec<W3DMesh>,
    pub materials: HashMap<String, W3DMaterial>,
    pub texture_names: Vec<String>,
    pub ww3d_mesh_models: HashMap<String, Arc<MeshModelClass>>,
    pub bounding_box_min: Vec3,
    pub bounding_box_max: Vec3,
    pub hierarchy: Option<W3dHierarchy>,
    /// Every source HTree retained from this exact W3D file in C++ load
    /// order. `hierarchy` remains the legacy whole-model selection used by
    /// existing draw paths; HMODEL definitions resolve their explicitly named
    /// HTree from this source set instead of borrowing that convenience field.
    pub hierarchies: Vec<W3dHierarchy>,
    /// Source-authored HLOD records. Main supports the C++ constructor-time
    /// static level for one rigid HLOD. Multiple independent HLODs remain
    /// non-rendering rather than flattening every group into one visible
    /// model; external aggregates resolve independently and proxies remain
    /// retained non-rendering application metadata.
    pub hlods: Vec<W3dHlod>,
    /// Source HMODEL definitions registered as their own C++ render-object
    /// prototypes. They must not be flattened into `meshes`: each instance
    /// owns the HTree named by its definition and attaches its node records at
    /// their authored pivots.
    pub hmodels: Vec<W3dHmodel>,
    /// A malformed HLOD must not silently fall back to generic mesh rendering:
    /// that would falsely claim a usable hierarchy/binding relationship.
    pub hlod_parse_failed: bool,
    pub animations: Vec<W3dAnimation>,
}

impl W3DModel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            meshes: Vec::new(),
            materials: HashMap::new(),
            texture_names: Vec::new(),
            ww3d_mesh_models: HashMap::new(),
            bounding_box_min: Vec3::splat(f32::MAX),
            bounding_box_max: Vec3::splat(f32::MIN),
            hierarchy: None,
            hierarchies: Vec::new(),
            hlods: Vec::new(),
            hmodels: Vec::new(),
            hlod_parse_failed: false,
            animations: Vec::new(),
        }
    }

    /// Retain one C++ `HTreeManager` source record without changing the
    /// legacy whole-model hierarchy selection. C++ preserves the first tree
    /// registered under an exact case-insensitive name, while existing Main
    /// rendering historically consults the most recently parsed `hierarchy`.
    /// Keep both contracts explicit rather than letting an HMODEL borrow an
    /// unrelated convenience value.
    fn retain_source_hierarchy(&mut self, hierarchy: W3dHierarchy) {
        let duplicate_source_name = self
            .hierarchies
            .iter()
            .any(|existing| existing.name.eq_ignore_ascii_case(&hierarchy.name));
        if !duplicate_source_name {
            self.hierarchies.push(hierarchy.clone());
        }
        self.hierarchy = Some(hierarchy);
    }

    /// Return immutable HMODEL-definition snap points in their authored W3D
    /// X/Y/Z basis.
    ///
    /// This is deliberately a source-definition query. Although
    /// `HModelDefClass::Load_W3D` retains `W3D_CHUNK_POINTS`, retail
    /// `HLodClass(HModelDefClass)` does not transfer that pointer to its own
    /// `SnapPoints` member. Consequently this API must not be used as a
    /// substitute for an active `RenderObjClass::Get_Snap_Point` call.
    /// Malformed HMODEL topology stays fail-closed, just as C++ refuses to
    /// register a prototype when its `Load_W3D` call fails.
    pub fn hmodel_source_snap_points(&self, hmodel_index: usize) -> Option<&[W3dHmodelSnapPoint]> {
        let hmodel = self.hmodels.get(hmodel_index)?;
        (!hmodel.has_invalid_records).then_some(hmodel.source_snap_points.as_slice())
    }

    /// Return one immutable source snap point, if the exact HMODEL and point
    /// index are valid. C++ indexes trusted source data directly; Main keeps
    /// this query safe rather than manufacturing an out-of-range point.
    pub fn hmodel_source_snap_point(
        &self,
        hmodel_index: usize,
        point_index: usize,
    ) -> Option<W3dHmodelSnapPoint> {
        self.hmodel_source_snap_points(hmodel_index)?
            .get(point_index)
            .copied()
    }

    /// Evaluate the constructor-selected bind-pose render topology for one
    /// exact C++ `HLodPrototypeClass` definition.
    ///
    /// `HLodLoaderClass` registers every `W3D_CHUNK_HLOD` independently and
    /// `HLodPrototypeClass::Create` passes that exact `HLodDefClass` to the
    /// constructor.  This API therefore accepts the immutable registry index
    /// instead of selecting the first HLOD or treating a source W3D file as a
    /// single aggregate.  The returned groups retain C++ render order:
    /// constructor-selected LOD children first, then `AdditionalModels`.
    ///
    /// A newly created HLOD owns its own HTree in bind pose.  As in
    /// `Animatable3DObjClass`, an empty or unavailable named HTree produces a
    /// one-pivot identity default tree; it must never borrow a different
    /// source HLOD's convenience hierarchy.  Malformed source topology,
    /// out-of-range prototype indices, invalid child identities, and invalid
    /// bone references fail closed at this isolated definition while valid
    /// sibling definitions remain usable.
    pub fn hlod_prototype_bind_pose(&self, hlod_index: usize) -> Option<W3dHlodPrototypeBindPose> {
        if self.hlod_parse_failed {
            return None;
        }

        let hlod = self.hlods.get(hlod_index)?;
        if hlod.has_invalid_trailing_records
            || hlod.name.is_empty()
            || hlod.name.as_bytes().contains(&0)
        {
            return None;
        }
        let selected_lod = hlod
            .lods
            .get(Self::cxx_constructor_selected_hlod_lod_index(hlod)?)?;

        let source_transforms = match self.source_hierarchy_for_hlod(hlod) {
            Some(hierarchy) => compute_bind_pose_global_transforms(hierarchy)?,
            // `Animatable3DObjClass::Animatable3DObjClass` calls
            // `HTreeClass::Init_Default` for an empty or unavailable named
            // hierarchy. Its single root is the render object's external
            // transform, represented by identity until the caller composes it.
            None => vec![Mat4::IDENTITY.to_cols_array()],
        };

        let pose_for_child = |child: &W3dHlodSubObject| {
            if child.name.is_empty() || child.name.as_bytes().contains(&0) {
                return None;
            }
            let bone_index = usize::try_from(child.bone_index)
                .ok()
                .filter(|index| *index < source_transforms.len())?;
            let source_transform = source_transforms.get(bone_index).copied()?;
            Some(W3dHlodAggregatePose {
                name: child.name.clone(),
                bone_index: child.bone_index,
                parent_transform: Self::w3d_transform_to_render_basis(Mat4::from_cols_array(
                    &source_transform,
                )),
                // Freshly constructed HLOD instances have no selected HAnim
                // and their bind-pose HTree pivots are visible. Root zero is
                // also forced visible by HTree.
                visible: true,
            })
        };

        let selected_lod_children = selected_lod
            .subobjects
            .iter()
            .filter_map(|child| pose_for_child(child))
            .collect();
        let additional_models = hlod
            .aggregates
            .as_ref()
            .map(|aggregates| {
                aggregates
                    .subobjects
                    .iter()
                    .filter_map(|child| pose_for_child(child))
                    .collect()
            })
            .unwrap_or_default();

        Some(W3dHlodPrototypeBindPose {
            selected_lod_children,
            additional_models,
        })
    }

    /// Return the local bind-pose palette for one independently constructed
    /// C++ HMODEL instance.
    ///
    /// `HModelPrototypeClass::Create` builds `HLodClass(HModelDef)`, whose
    /// `Animatable3DObjClass` owns the HTree named by this exact HMODEL. A
    /// missing or empty named tree becomes C++'s one-pivot identity
    /// `RootTransform`; it must never borrow another whole-file HTree or the
    /// parent Drawable animation. The external HMODEL attachment transform is
    /// intentionally not folded into this palette: `HTree::Base_Update` takes
    /// that root separately at runtime.
    pub fn hmodel_bind_pose_palette(&self, hmodel_index: usize) -> Option<Vec<Mat4>> {
        let hmodel = self.hmodels.get(hmodel_index)?;
        self.hmodel_bind_pose_source_transforms(hmodel)
            .map(|transforms| {
                transforms
                    .into_iter()
                    .map(|transform| {
                        Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&transform))
                    })
                    .collect()
            })
    }

    /// Return the valid `SKIN_NODE` connections for one exact HMODEL.
    ///
    /// The connection pivot is validated against the HMODEL's own named or
    /// default HTree, but is not returned as a mesh placement matrix. C++
    /// deforms a skin with `Container->Get_HTree()` and then applies the
    /// container's outer attachment only once. Invalid individual skin nodes
    /// therefore skip without changing valid rigid sibling behavior.
    pub fn hmodel_skin_node_bindings(
        &self,
        hmodel_index: usize,
    ) -> Option<Vec<W3dHmodelSkinNodeBinding>> {
        let palette_len = self.hmodel_bind_pose_palette(hmodel_index)?.len();
        let hmodel = self.hmodels.get(hmodel_index)?;

        Some(
            hmodel
                .nodes
                .iter()
                .filter(|node| {
                    node.kind == W3dHmodelNodeKind::SkinNode
                        && !node.name.is_empty()
                        && !node.name.as_bytes().contains(&0)
                        && usize::try_from(node.bone_index)
                            .ok()
                            .is_some_and(|bone_index| bone_index < palette_len)
                })
                .map(|node| W3dHmodelSkinNodeBinding {
                    name: node.name.clone(),
                    bone_index: node.bone_index,
                })
                .collect(),
        )
    }

    /// Evaluate the rigid NODE/COLLISION_NODE records of one C++ HMODEL in
    /// its independently instantiated default HTree pose.
    ///
    /// `HModelPrototypeClass::Create` constructs `HLodClass(HModelDef)`, and
    /// `Animatable3DObjClass` clones the HTree named by the definition. If
    /// that named tree cannot be found, C++ initializes a one-pivot identity
    /// `RootTransform`; keep that exact fallback rather than borrowing a
    /// different whole-file hierarchy. SKIN_NODE records use
    /// [`Self::hmodel_bind_pose_palette`] instead because their mesh placement
    /// is the outer HMODEL attachment, not their connection bone.
    pub fn hmodel_rigid_node_poses(&self, hmodel_index: usize) -> Option<Vec<W3dHmodelNodePose>> {
        let hmodel = self.hmodels.get(hmodel_index)?;
        let source_transforms = self.hmodel_bind_pose_source_transforms(hmodel)?;

        let mut poses = Vec::new();
        for node in &hmodel.nodes {
            if !node.kind.is_currently_rigid()
                || node.name.is_empty()
                || node.name.as_bytes().contains(&0)
            {
                continue;
            }
            let Some(bone_index) = usize::try_from(node.bone_index)
                .ok()
                .filter(|index| *index < source_transforms.len())
            else {
                // C++'s trusted-data implementation would later address this
                // HTree pivot. Keep valid sibling connections independent and
                // skip only this unsafe child.
                continue;
            };
            let Some(source_transform) = source_transforms.get(bone_index).copied() else {
                continue;
            };
            poses.push(W3dHmodelNodePose {
                name: node.name.clone(),
                bone_index: node.bone_index,
                parent_transform: Self::w3d_transform_to_render_basis(Mat4::from_cols_array(
                    &source_transform,
                )),
            });
        }
        Some(poses)
    }

    /// Resolve only the HTree explicitly named by one HLOD definition.
    ///
    /// `Animatable3DObjClass` falls back to its default one-root HTree when
    /// the source name is empty or cannot be found. Returning `None` here
    /// intentionally represents that exact fallback; callers must not select
    /// the legacy convenience hierarchy merely because it happens to be from
    /// another HLOD in the same source W3D file.
    fn source_hierarchy_for_hlod(&self, hlod: &W3dHlod) -> Option<&W3dHierarchy> {
        (!hlod.hierarchy_name.is_empty())
            .then(|| {
                self.hierarchies
                    .iter()
                    .find(|hierarchy| {
                        hierarchy
                            .name
                            .eq_ignore_ascii_case(hlod.hierarchy_name.as_str())
                    })
                    // Older hand-built fixtures may retain only the legacy
                    // field. It remains valid only when it names this exact
                    // HLOD's requested HTree.
                    .or_else(|| {
                        self.hierarchy.as_ref().filter(|hierarchy| {
                            hierarchy
                                .name
                                .eq_ignore_ascii_case(hlod.hierarchy_name.as_str())
                        })
                    })
            })
            .flatten()
    }

    /// Resolve only the HTree explicitly named by an HMODEL definition. The
    /// current source model can carry several HTree records; matching the
    /// convenience `hierarchy` field by position would change C++ ownership.
    fn source_hierarchy_for_hmodel(&self, hmodel: &W3dHmodel) -> Option<&W3dHierarchy> {
        (!hmodel.hierarchy_name.is_empty())
            .then(|| {
                self.hierarchies
                    .iter()
                    .find(|hierarchy| {
                        hierarchy
                            .name
                            .eq_ignore_ascii_case(hmodel.hierarchy_name.as_str())
                    })
                    // Hand-built source fixtures from older callers may only
                    // populate the legacy field. It is usable only when it
                    // names this exact HMODEL hierarchy.
                    .or_else(|| {
                        self.hierarchy.as_ref().filter(|hierarchy| {
                            hierarchy
                                .name
                                .eq_ignore_ascii_case(hmodel.hierarchy_name.as_str())
                        })
                    })
            })
            .flatten()
    }

    /// Produce the source-space local HTree bind pose for one valid HMODEL.
    /// Keep this shared by rigid placement and skin palette construction so a
    /// named/default hierarchy decision cannot drift between the two paths.
    fn hmodel_bind_pose_source_transforms(&self, hmodel: &W3dHmodel) -> Option<Vec<[f32; 16]>> {
        if hmodel.has_invalid_records
            || hmodel.name.is_empty()
            || hmodel.name.as_bytes().contains(&0)
        {
            return None;
        }

        match self.source_hierarchy_for_hmodel(hmodel) {
            Some(hierarchy) => compute_bind_pose_global_transforms(hierarchy),
            // `Animatable3DObjClass::Init_Default`: one visible identity root.
            None => Some(vec![Mat4::IDENTITY.to_cols_array()]),
        }
    }

    pub fn calculate_bounding_box(&mut self) {
        self.bounding_box_min = Vec3::splat(f32::MAX);
        self.bounding_box_max = Vec3::splat(f32::MIN);

        // W3D vertices are converted to the active Main render basis at import.
        // Rigid HLOD child transforms must therefore be applied exactly once here,
        // just as they are when creating a RenderItem.  Computing the transforms
        // before mutating bounds avoids borrowing `self.meshes` through both paths.
        let mesh_transforms: Vec<Option<Mat4>> = (0..self.meshes.len())
            .map(|mesh_index| self.mesh_bind_pose_local_transform(mesh_index))
            .collect();

        for (mesh, local_transform) in self.meshes.iter().zip(mesh_transforms) {
            let Some(local_transform) = local_transform else {
                continue;
            };
            for vertex in &mesh.vertices {
                let pos = local_transform.transform_point3(Vec3::from_array(vertex.position));
                self.bounding_box_min = self.bounding_box_min.min(pos);
                self.bounding_box_max = self.bounding_box_max.max(pos);
            }
        }

        // Unsupported HLODs intentionally emit no render items.  Keep their
        // bounds finite too, so downstream culling/debug paths cannot receive
        // sentinel infinities while that source feature remains fail-closed.
        if self.bounding_box_min == Vec3::splat(f32::MAX) {
            self.bounding_box_min = Vec3::ZERO;
            self.bounding_box_max = Vec3::ZERO;
        }
    }

    /// Return the render-basis local transform *and* source HTree visibility
    /// for one mesh at the requested animation frame.
    ///
    /// `None` means the mesh is not renderable through the source HLOD data:
    /// malformed HLOD, multiple independent HLODs, an unresolved selected-level
    /// child identity, an invalid bone, or an unsupported compressed visibility
    /// channel all fail closed. Aggregate children are resolved independently;
    /// their absence must not suppress valid parent geometry. Models without HLOD
    /// metadata preserve their existing local mesh transform and remain visible.
    ///
    /// An absent `animation_index` is deliberately a bind-pose request, not a
    /// request for animation zero. C++ W3DModelDraw only installs an animation
    /// explicitly selected by its current Draw state.
    pub fn mesh_local_transform_and_visibility_for_animation(
        &self,
        mesh_index: usize,
        animation_index: Option<usize>,
        animation_frame: f32,
    ) -> Option<(Mat4, bool)> {
        let binding = animation_index.map(W3dAnimationBinding::local);
        self.mesh_local_transform_and_visibility_for_binding(
            mesh_index,
            binding.as_ref(),
            animation_frame,
        )
    }

    /// As [`Self::mesh_local_transform_and_visibility_for_animation`], but
    /// retains the frozen local-or-companion HAnim selection all the way to
    /// the HLOD child. An absent binding is a bind-pose request; an invalid
    /// binding is *not* a request for local clip zero.
    pub fn mesh_local_transform_and_visibility_for_binding(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
    ) -> Option<(Mat4, bool)> {
        let mesh = self.meshes.get(mesh_index)?;

        if self.hlod_parse_failed {
            return None;
        }
        if self.hlods.is_empty() {
            return Some((mesh.transform, true));
        }

        let bone_index = self.rigid_hlod_bone_index_for_mesh(mesh_index)?;
        let hierarchy = self.hierarchy.as_ref()?;
        let (source_transform, visible) = if let Some(animation_binding) = animation_binding {
            // An HTree can only apply a motion authored for its exact source
            // hierarchy.  Do not reinterpret a same-named clip from a
            // different hierarchy as visibility for this HLOD child.
            if !self.animation_binding_is_compatible(animation_binding) {
                return None;
            }
            let animation = animation_binding.animation(self)?;
            let source_transform = self
                .sample_animation_binding(animation_binding, animation_frame)
                .and_then(|transforms| transforms.get(bone_index).copied())?;
            let visible = animation.visibility_for_pivot(bone_index, animation_frame)?;
            (source_transform, visible)
        } else {
            let source_transform = compute_bind_pose_global_transforms(hierarchy)?
                .get(bone_index)
                .copied()?;
            (source_transform, true)
        };

        Some((
            Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&source_transform)),
            visible,
        ))
    }

    /// Resolve each source aggregate's parent-bone pose for the frozen HAnim
    /// selection. This is the CPU-side equivalent of C++
    /// `HLodClass::Update_Sub_Object_Transforms` for `AdditionalModels`:
    /// each independently loaded child receives the exact parent HTree
    /// transform and animation-hidden state.
    ///
    /// This does not make aggregate models renderable by itself. The caller
    /// must resolve each `name` as an external render object, skip a missing
    /// asset or invalid bone individually, and compose `parent_transform`
    /// beneath the parent item world transform. Keeping that work separate
    /// prevents a source aggregate from being flattened into an unrelated
    /// parent mesh or substituted with a debug fallback.
    pub fn aggregate_attachment_poses_for_binding(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
    ) -> Option<Vec<W3dHlodAggregatePose>> {
        self.aggregate_attachment_poses_for_binding_and_capture_controls(
            animation_binding,
            animation_frame,
            &[],
        )
    }

    /// As [`Self::aggregate_attachment_poses_for_binding`], but applies the
    /// ordered source-space C++ `Capture_Bone`/`Control_Bone` transforms that
    /// the current Draw module installed before `HLodClass` updates its
    /// `AdditionalModels`. This is necessary for an aggregate on a turret,
    /// recoil, or wrapper-controlled pivot to inherit that exact current pose.
    pub fn aggregate_attachment_poses_for_binding_and_capture_controls(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        capture_bone_controls: &[(i32, Mat4)],
    ) -> Option<Vec<W3dHlodAggregatePose>> {
        let (hlod, _lod, hierarchy) = self.static_hlod_parent_context()?;
        let aggregates = hlod.aggregates.as_ref()?;
        let local_transforms =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)?;
        let mut controls = vec![None; hierarchy.pivots.len()];
        let mut captured_pivots = vec![false; hierarchy.pivots.len()];
        for (raw_index, transform) in capture_bone_controls {
            let index = usize::try_from(*raw_index)
                .ok()
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())?;
            if !Self::capture_control_transform_is_affine(*transform) {
                return None;
            }
            // C++ `Control_Bone` replaces prior controls for the same pivot.
            controls[index] = Some(transform.to_cols_array());
            captured_pivots[index] = true;
        }
        let source_transforms = compute_htree_global_transforms_from_locals_with_capture_controls(
            hierarchy,
            &local_transforms,
            &controls,
        )?;
        let animation = animation_binding.and_then(|binding| binding.animation(self));
        if animation_binding.is_some() && animation.is_none() {
            return None;
        }

        let mut poses = Vec::with_capacity(aggregates.subobjects.len());
        for aggregate in &aggregates.subobjects {
            let Some(bone_index) = usize::try_from(aggregate.bone_index)
                .ok()
                .filter(|index| *index < hierarchy.pivots.len())
            else {
                // C++ `Add_Sub_Object_To_Bone` skips an aggregate with an
                // invalid bone and keeps the remaining parent HLOD intact.
                continue;
            };
            let Some(source_transform) = source_transforms.get(bone_index).copied() else {
                continue;
            };
            // HTree forces a captured pivot visible after it applies the
            // control. Its root is also always visible; neither uses a raw
            // pivot-zero visibility channel.
            let visible = if bone_index == 0 || captured_pivots[bone_index] {
                true
            } else {
                match animation {
                    Some(animation) => {
                        animation.visibility_for_pivot(bone_index, animation_frame)?
                    }
                    None => true,
                }
            };
            poses.push(W3dHlodAggregatePose {
                name: aggregate.name.clone(),
                bone_index: aggregate.bone_index,
                parent_transform: Self::w3d_transform_to_render_basis(Mat4::from_cols_array(
                    &source_transform,
                )),
                visible,
            });
        }
        Some(poses)
    }

    /// As [`Self::aggregate_attachment_poses_for_binding`], with the same
    /// primary-turret and recoil `Control_Bone` sequence used by a rigid
    /// parent mesh. C++ `HLodClass::Update_Sub_Object_Transforms` reads the
    /// parent HTree *after* `W3DModelDraw` has installed those controls, so an
    /// `AdditionalModels` child on a turret or barrel must inherit them too.
    ///
    /// The bounded visual-control implementation remains restricted to the
    /// same exact single-HLOD topology as the parent mesh helper. When that
    /// control topology is unavailable or a recoil payload is malformed, use
    /// the already-valid selected HAnim/bind pose rather than moving an
    /// aggregate through guessed names or stale indices.
    pub fn aggregate_attachment_poses_for_primary_turret_and_weapon_controls(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle_degrees: f32,
        turret_pitch_degrees: f32,
        weapon_controls: &[W3dWeaponVisualControl],
    ) -> Option<Vec<W3dHlodAggregatePose>> {
        let fallback =
            self.aggregate_attachment_poses_for_binding(animation_binding, animation_frame)?;
        let Some((_hlod, hierarchy)) = self.rigid_hlod_context() else {
            return Some(fallback);
        };

        let mut capture_controls = Vec::new();
        if primary_turret.primary_fields_valid && !primary_turret.has_unsupported_alternate_turret()
        {
            if let Some((bone_index, transform)) = primary_turret
                .yaw_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_angle_degrees,
                        primary_turret.yaw_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_z(angle)))
                })
            {
                capture_controls.push((i32::try_from(bone_index).ok()?, transform));
            }
            if let Some((bone_index, transform)) = primary_turret
                .pitch_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_pitch_degrees,
                        primary_turret.pitch_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_y(-angle)))
                })
            {
                // `handleClientTurretPositioning` controls yaw before pitch.
                // Preserve that order, including a malformed same-pivot
                // source where pitch intentionally replaces yaw.
                capture_controls.push((i32::try_from(bone_index).ok()?, transform));
            }
        }

        for control in weapon_controls {
            let Some(pivot_index) = control
                .recoil_pivot_index
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())
            else {
                continue;
            };
            if !control.recoil_shift.is_finite() || control.recoil_shift < 0.0 {
                return Some(fallback);
            }
            // `handleClientRecoil` runs after turret positioning. Its later
            // slot/barrel control replaces an earlier control on the same
            // source pivot, exactly as `HTreeClass::Control_Bone` does.
            capture_controls.push((
                i32::try_from(pivot_index).ok()?,
                Mat4::from_translation(Vec3::new(-control.recoil_shift, 0.0, 0.0)),
            ));
        }

        if capture_controls.is_empty() {
            return Some(fallback);
        }
        self.aggregate_attachment_poses_for_binding_and_capture_controls(
            animation_binding,
            animation_frame,
            &capture_controls,
        )
        .or(Some(fallback))
    }

    /// As [`Self::mesh_local_transform_and_visibility_for_binding`], with an
    /// ordered C++ `HTreeClass::Capture_Bone` / `Control_Bone` control list
    /// applied in source pivot space after HAnim locals and before children
    /// inherit their parent's global transform.
    ///
    /// The controls originate in a frozen GameClient bridge submission. They
    /// are deliberately index-only: an index is valid only against this exact
    /// fresh hierarchy/HLOD, and malformed controls fail closed to the normal
    /// selected pose rather than guessing a bone by name.
    pub fn mesh_local_transform_and_visibility_for_binding_and_capture_controls(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        capture_bone_controls: &[(i32, Mat4)],
    ) -> Option<(Mat4, bool)> {
        let (fallback_transform, visible) = self.mesh_local_transform_and_visibility_for_binding(
            mesh_index,
            animation_binding,
            animation_frame,
        )?;
        if capture_bone_controls.is_empty() {
            return Some((fallback_transform, visible));
        }

        let Some(mesh_bone_index) = self.rigid_hlod_bone_index_for_mesh(mesh_index) else {
            return Some((fallback_transform, visible));
        };
        let Some((_hlod, hierarchy)) = self.rigid_hlod_context() else {
            return Some((fallback_transform, visible));
        };
        let Some(local_transforms) =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)
        else {
            return Some((fallback_transform, visible));
        };

        let mut controls = vec![None; hierarchy.pivots.len()];
        for (raw_index, transform) in capture_bone_controls {
            let Some(index) = usize::try_from(*raw_index)
                .ok()
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())
            else {
                return Some((fallback_transform, visible));
            };
            if !Self::capture_control_transform_is_affine(*transform) {
                return Some((fallback_transform, visible));
            }
            // `Control_Bone` replaces its captured pivot transform. Preserve
            // the bridge order so duplicate controls retain C++ last-write
            // wins semantics.
            controls[index] = Some(transform.to_cols_array());
        }

        let Some(source_transform) =
            compute_htree_global_transforms_from_locals_with_capture_controls(
                hierarchy,
                &local_transforms,
                &controls,
            )?
            .get(mesh_bone_index)
            .copied()
        else {
            return Some((fallback_transform, visible));
        };
        Some((
            Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&source_transform)),
            visible,
        ))
    }

    /// Produce a render-basis skin palette after the same validated C++ HTree
    /// capture controls used by rigid HLOD children. Keeping this paired with
    /// the mesh transform prevents a controlled rigid child and skinned
    /// vertices from disagreeing in the forward pass.
    pub fn animation_palette_for_binding_and_capture_controls(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        capture_bone_controls: &[(i32, Mat4)],
    ) -> Option<Vec<Mat4>> {
        if capture_bone_controls.is_empty() {
            // Preserve the old, deliberate contract for ordinary Draw state:
            // absent animation means bind pose and does not silently upload a
            // local clip or a synthetic skin palette. C++ recoil controls are
            // different: they operate on an HTree even with no HAnim, so the
            // non-empty-control path below constructs that bind-pose palette.
            let binding = animation_binding?;
            return Some(
                self.sample_animation_binding(binding, animation_frame)?
                    .into_iter()
                    .map(|transform| {
                        Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&transform))
                    })
                    .collect(),
            );
        }

        // Raw bridge controls are only meaningful against the exact rigid
        // HLOD/HTree topology that supplied their source pivot indices. A
        // hierarchy alone is insufficient: accepting a stale index against a
        // flattened or aggregate model can move unrelated geometry.
        let (_hlod, hierarchy) = self.rigid_hlod_context()?;
        let local_transforms =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)?;

        let mut controls = vec![None; hierarchy.pivots.len()];
        for (raw_index, transform) in capture_bone_controls {
            let index = usize::try_from(*raw_index)
                .ok()
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())?;
            if !Self::capture_control_transform_is_affine(*transform) {
                return None;
            }
            controls[index] = Some(transform.to_cols_array());
        }
        compute_htree_global_transforms_from_locals_with_capture_controls(
            hierarchy,
            &local_transforms,
            &controls,
        )
        .map(|transforms| {
            transforms
                .into_iter()
                .map(|transform| {
                    Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&transform))
                })
                .collect()
        })
    }

    /// A bridge control is an HTree relative affine transform in source W3D
    /// pivot space. Reject projective/non-finite payloads rather than letting
    /// a malformed client submission affect the final GPU transform.
    fn capture_control_transform_is_affine(transform: Mat4) -> bool {
        const AFFINE_EPSILON: f32 = 1.0e-4;
        transform.is_finite()
            && transform.x_axis.w.abs() <= AFFINE_EPSILON
            && transform.y_axis.w.abs() <= AFFINE_EPSILON
            && transform.z_axis.w.abs() <= AFFINE_EPSILON
            && (transform.w_axis.w - 1.0).abs() <= AFFINE_EPSILON
    }

    /// As [`Self::mesh_local_transform_and_visibility_for_binding`], with the
    /// bounded C++ `W3DModelDraw::handleClientTurretPositioning` primary-bone
    /// control applied after the frozen source HAnim has constructed its pose.
    ///
    /// The existing HLOD transform/visibility path remains the authority for
    /// whether a mesh can render. A missing, malformed, alternate-turret, or
    /// unresolved primary binding deliberately leaves that already selected
    /// pose alone: it must never rotate the entire vehicle hull or infer a
    /// turret from a mesh name. This helper only accepts Main's active
    /// single-HLOD topology and converts the final source pose to render basis
    /// exactly once.
    pub fn mesh_local_transform_and_visibility_for_primary_turret(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle_degrees: f32,
        turret_pitch_degrees: f32,
    ) -> Option<(Mat4, bool)> {
        let (fallback_transform, visible) = self.mesh_local_transform_and_visibility_for_binding(
            mesh_index,
            animation_binding,
            animation_frame,
        )?;

        let Some(source_transform) = self.primary_turret_source_transform_for_mesh(
            mesh_index,
            animation_binding,
            animation_frame,
            primary_turret,
            turret_angle_degrees,
            turret_pitch_degrees,
        ) else {
            return Some((fallback_transform, visible));
        };

        Some((
            Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&source_transform)),
            visible,
        ))
    }

    /// As [`Self::mesh_local_transform_and_visibility_for_primary_turret`],
    /// with C++ `W3DModelDraw::handleClientRecoil` controls applied after the
    /// selected HAnim and after primary-turret capture controls.  The controls
    /// are fresh W3D pivot identities only: callers must first validate their
    /// selected source Draw state against [`Self::weapon_barrel_topology_for_authored_bindings`].
    ///
    /// A malformed/unsupported control path deliberately falls back to the
    /// already-valid turret/animation pose rather than moving an arbitrary
    /// mesh.  Muzzle visibility is kept separate from HTree capture controls
    /// because C++ hides the exact subobject on that pivot, not every sibling
    /// sharing the same bone.
    pub fn mesh_local_transform_and_visibility_for_primary_turret_and_weapon_controls(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle_degrees: f32,
        turret_pitch_degrees: f32,
        weapon_controls: &[W3dWeaponVisualControl],
    ) -> Option<(Mat4, bool)> {
        let (fallback_transform, visible) = self
            .mesh_local_transform_and_visibility_for_primary_turret(
                mesh_index,
                animation_binding,
                animation_frame,
                primary_turret,
                turret_angle_degrees,
                turret_pitch_degrees,
            )?;
        if weapon_controls.is_empty() {
            return Some((fallback_transform, visible));
        }

        let Some(mesh_bone_index) = self.rigid_hlod_bone_index_for_mesh(mesh_index) else {
            return Some((fallback_transform, visible));
        };
        let Some((hlod, hierarchy)) = self.rigid_hlod_context() else {
            return Some((fallback_transform, visible));
        };
        let Some(local_transforms) =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)
        else {
            return Some((fallback_transform, visible));
        };

        let mut capture_controls = vec![None; hierarchy.pivots.len()];
        if primary_turret.primary_fields_valid && !primary_turret.has_unsupported_alternate_turret()
        {
            if let Some((bone_index, transform)) = primary_turret
                .yaw_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_angle_degrees,
                        primary_turret.yaw_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_z(angle).to_cols_array()))
                })
            {
                capture_controls[bone_index] = Some(transform);
            }
            if let Some((bone_index, transform)) = primary_turret
                .pitch_bone
                .as_deref()
                .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
                .and_then(|bone_index| {
                    Self::primary_turret_angle_radians(
                        turret_pitch_degrees,
                        primary_turret.pitch_art_angle_radians(),
                    )
                    .map(|angle| (bone_index, Mat4::from_rotation_y(-angle).to_cols_array()))
                })
            {
                // C++ calls yaw Control_Bone before pitch. If bad source data
                // aliases them, the later pitch control replaces yaw.
                capture_controls[bone_index] = Some(transform);
            }
        }

        for control in weapon_controls {
            let Some(pivot_index) = control
                .recoil_pivot_index
                .and_then(|index| usize::try_from(index).ok())
                .filter(|index| *index != 0 && *index < hierarchy.pivots.len())
            else {
                continue;
            };
            if !control.recoil_shift.is_finite() || control.recoil_shift < 0.0 {
                return Some((fallback_transform, visible));
            }
            // `handleClientRecoil` runs after turret positioning and calls
            // Capture_Bone/Control_Bone in slot/barrel order. A later control
            // on the same pivot replaces the earlier capture transform.
            capture_controls[pivot_index] = Some(
                Mat4::from_translation(Vec3::new(-control.recoil_shift, 0.0, 0.0)).to_cols_array(),
            );
        }

        let Some(source_transform) =
            compute_htree_global_transforms_from_locals_with_capture_controls(
                hierarchy,
                &local_transforms,
                &capture_controls,
            )?
            .get(mesh_bone_index)
            .copied()
        else {
            return Some((fallback_transform, visible));
        };
        Some((
            Self::w3d_transform_to_render_basis(Mat4::from_cols_array(&source_transform)),
            visible,
        ))
    }

    /// Return C++ `setMuzzleFlashHidden`'s direct visibility override for one
    /// rigid HLOD mesh.  `W3DModelDraw` changes only
    /// `Get_Sub_Object_On_Bone(0, muzzle_bone)`: the first child in the
    /// selected HLOD level on that exact pivot, not every sibling sharing the
    /// bone.  Controls are evaluated in slot/barrel order, so a malformed
    /// later alias intentionally wins just like sequential C++ calls.
    ///
    /// This is kept independent of HTree transforms.  The collector applies
    /// the override after authored Hide/Show directives, while preserving a
    /// selected HAnim's own invisible mesh state.
    pub fn muzzle_flash_visibility_override_for_mesh(
        &self,
        mesh_index: usize,
        weapon_controls: &[W3dWeaponVisualControl],
    ) -> Option<bool> {
        let (mesh_subobject, mesh_bone_index) = self.rigid_hlod_subobject_for_mesh(mesh_index)?;
        let (hlod, _hierarchy) = self.rigid_hlod_context()?;
        let mesh_bone_index = u32::try_from(mesh_bone_index).ok()?;
        let first_child_for_pivot = |pivot_index: u32| {
            hlod.lods.first().and_then(|lod| {
                lod.subobjects
                    .iter()
                    .find(|child| child.bone_index == pivot_index)
            })
        };

        let mut override_visibility = None;
        for control in weapon_controls {
            let Some(pivot_index) = control.muzzle_flash_pivot_index else {
                continue;
            };
            if mesh_bone_index != pivot_index {
                continue;
            }
            if first_child_for_pivot(pivot_index).is_some_and(|child| {
                child
                    .name
                    .eq_ignore_ascii_case(mesh_subobject.name.as_str())
            }) {
                override_visibility = Some(control.muzzle_flash_visible);
            }
        }
        override_visibility
    }

    /// Return the source-space HTree transform for one rigid HLOD mesh after
    /// C++-ordered primary turret capture controls. `None` means no safe
    /// primary control exists, not that the mesh itself is unavailable; the
    /// caller uses its already validated selected-animation/bind-pose value.
    fn primary_turret_source_transform_for_mesh(
        &self,
        mesh_index: usize,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
        primary_turret: &AuthoredDrawPrimaryTurret,
        turret_angle_degrees: f32,
        turret_pitch_degrees: f32,
    ) -> Option<[f32; 16]> {
        if !primary_turret.primary_fields_valid
            || primary_turret.has_unsupported_alternate_turret()
            || !primary_turret.has_primary_bone()
        {
            return None;
        }

        // This is intentionally the same one-HLOD/one-LOD/compatible-HTree
        // gate as the normal rigid-child transform path. A bare mesh or a
        // flattened multi-LOD name cannot become an implicit turret target.
        let mesh_bone_index = self.rigid_hlod_bone_index_for_mesh(mesh_index)?;
        let (_hlod, hierarchy) = self.rigid_hlod_context()?;

        let yaw_control = primary_turret
            .yaw_bone
            .as_deref()
            .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
            .and_then(|bone_index| {
                Self::primary_turret_angle_radians(
                    turret_angle_degrees,
                    primary_turret.yaw_art_angle_radians(),
                )
                .map(|angle| (bone_index, Mat4::from_rotation_z(angle).to_cols_array()))
            });
        let pitch_control = primary_turret
            .pitch_bone
            .as_deref()
            .and_then(|bone_name| Self::primary_turret_pivot_index(hierarchy, bone_name))
            .and_then(|bone_index| {
                Self::primary_turret_angle_radians(
                    turret_pitch_degrees,
                    primary_turret.pitch_art_angle_radians(),
                )
                // C++ uses Rotate_Y(-turretPitch) after adding its authored
                // `TurretArtPitch` offset.
                .map(|angle| (bone_index, Mat4::from_rotation_y(-angle).to_cols_array()))
            });

        if yaw_control.is_none() && pitch_control.is_none() {
            return None;
        }

        let local_transforms =
            self.local_transforms_for_animation_binding(animation_binding, animation_frame)?;
        let mut capture_controls = vec![None; hierarchy.pivots.len()];
        if let Some((bone_index, transform)) = yaw_control {
            capture_controls[bone_index] = Some(transform);
        }
        if let Some((bone_index, transform)) = pitch_control {
            // `handleClientTurretPositioning` calls yaw then pitch. If an
            // invalid source uses one exact bone for both, the latter
            // Control_Bone call replaces the former capture transform.
            capture_controls[bone_index] = Some(transform);
        }

        compute_htree_global_transforms_from_locals_with_capture_controls(
            hierarchy,
            &local_transforms,
            &capture_controls,
        )?
        .get(mesh_bone_index)
        .copied()
    }

    /// Resolve a C++ `NameKey` only against an exact HTree pivot. Pivot zero
    /// is C++'s "unresolved/no bone" sentinel in `validateTurretInfo`, so a
    /// root-name match may not turn into a whole-model rotation.
    fn primary_turret_pivot_index(hierarchy: &W3dHierarchy, bone_name: &str) -> Option<usize> {
        hierarchy
            .pivots
            .iter()
            .position(|pivot| pivot.name.eq_ignore_ascii_case(bone_name))
            .filter(|bone_index| *bone_index != 0)
    }

    fn primary_turret_angle_radians(gameplay_degrees: f32, art_radians: f32) -> Option<f32> {
        let angle = gameplay_degrees.to_radians() + art_radians;
        (gameplay_degrees.is_finite() && art_radians.is_finite() && angle.is_finite())
            .then_some(angle)
    }

    /// Rebuild C++ `ModelConditionInfo::m_weaponBarrelInfoVec` for one frozen
    /// selected Draw state using only its exact authored bases and this exact
    /// active W3D hierarchy.
    ///
    /// The bounded Main renderer supports only the same rigid single-HLOD
    /// topology used by its transform path. Missing/malformed Draw data,
    /// malformed HLODs, multi-LOD/aggregate content, or an incompatible
    /// hierarchy return `None`; callers must leave recoil idle instead of
    /// inferring a barrel from a mesh/model name. A valid source state with no
    /// retained bones returns `Some` with empty vectors, matching C++'s lack
    /// of `WeaponBarrelInfo` for that slot.
    pub fn weapon_barrel_topology_for_authored_bindings(
        &self,
        bindings: &AuthoredDrawWeaponBoneBindings,
    ) -> Option<W3dWeaponBarrelTopology> {
        if !bindings.source_fields_valid || self.hlod_parse_failed {
            return None;
        }
        let (_hlod, hierarchy) = self.rigid_hlod_context()?;
        Some(W3dWeaponBarrelTopology {
            slots: std::array::from_fn(|slot| {
                Self::weapon_barrels_for_authored_slot(hierarchy, &bindings.slots[slot])
            }),
        })
    }

    /// Mirror C++ `validateWeaponBarrelInfo`: scan every supplied base with
    /// `%02d` indices 01 through 99, stop at the first all-missing record,
    /// and use unadorned names only when no numbered record was found. A
    /// numbered muzzle flash may reuse the previous numbered FX pivot exactly
    /// like the retail multi-flash exception in C++.
    fn weapon_barrels_for_authored_slot(
        hierarchy: &W3dHierarchy,
        authored: &AuthoredDrawWeaponBoneSlot,
    ) -> Vec<W3dWeaponBarrelBinding> {
        let has_any_base = authored.fire_fx_bone_base.is_some()
            || authored.recoil_bone_base.is_some()
            || authored.muzzle_flash_bone_base.is_some()
            || authored.launch_bone_base.is_some();
        if !has_any_base {
            return Vec::new();
        }

        let mut numbered = Vec::new();
        let mut previous_fire_fx = None;
        for index in 1..=99u8 {
            let mut binding = W3dWeaponBarrelBinding {
                fire_fx_pivot_index: authored.fire_fx_bone_base.as_deref().and_then(|base| {
                    Self::pristine_pivot_index(hierarchy, &format!("{base}{index:02}"))
                }),
                recoil_pivot_index: authored.recoil_bone_base.as_deref().and_then(|base| {
                    Self::pristine_pivot_index(hierarchy, &format!("{base}{index:02}"))
                }),
                muzzle_flash_pivot_index: authored.muzzle_flash_bone_base.as_deref().and_then(
                    |base| Self::pristine_pivot_index(hierarchy, &format!("{base}{index:02}")),
                ),
                launch_pivot_index: authored.launch_bone_base.as_deref().and_then(|base| {
                    Self::pristine_pivot_index(hierarchy, &format!("{base}{index:02}"))
                }),
            };
            if binding.fire_fx_pivot_index.is_none() && binding.muzzle_flash_pivot_index.is_some() {
                binding.fire_fx_pivot_index = previous_fire_fx;
            }
            if !binding.has_any_binding() {
                break;
            }
            previous_fire_fx = binding.fire_fx_pivot_index;
            numbered.push(binding);
        }

        if !numbered.is_empty() {
            return numbered;
        }

        let unadorned = W3dWeaponBarrelBinding {
            fire_fx_pivot_index: authored
                .fire_fx_bone_base
                .as_deref()
                .and_then(|base| Self::pristine_pivot_index(hierarchy, base)),
            recoil_pivot_index: authored
                .recoil_bone_base
                .as_deref()
                .and_then(|base| Self::pristine_pivot_index(hierarchy, base)),
            muzzle_flash_pivot_index: authored
                .muzzle_flash_bone_base
                .as_deref()
                .and_then(|base| Self::pristine_pivot_index(hierarchy, base)),
            launch_pivot_index: authored
                .launch_bone_base
                .as_deref()
                .and_then(|base| Self::pristine_pivot_index(hierarchy, base)),
        };
        unadorned
            .has_any_binding()
            .then_some(unadorned)
            .into_iter()
            .collect()
    }

    /// C++ `findPristineBone` treats index zero as an unresolved/no-bone
    /// sentinel in the weapon and turret paths. Never let a matching root
    /// pivot become a whole-model recoil/launch binding.
    fn pristine_pivot_index(hierarchy: &W3dHierarchy, name: &str) -> Option<u32> {
        hierarchy
            .pivots
            .iter()
            .position(|pivot| pivot.name.eq_ignore_ascii_case(name))
            .filter(|index| *index != 0)
            .and_then(|index| u32::try_from(index).ok())
    }

    /// Build source-space local pivot matrices from either an explicitly
    /// frozen compatible HAnim or the HTree bind pose. Absent binding is
    /// deliberately bind pose; it must not select local animation zero.
    fn local_transforms_for_animation_binding(
        &self,
        animation_binding: Option<&W3dAnimationBinding>,
        animation_frame: f32,
    ) -> Option<Vec<[f32; 16]>> {
        let hierarchy = self.hierarchy.as_ref()?;
        let animation = match animation_binding {
            Some(binding) => {
                if !self.animation_binding_is_compatible(binding) {
                    return None;
                }
                binding.animation(self)?
            }
            None => return Some(hierarchy.pivots.iter().map(mat4_from_pivot).collect()),
        };
        sample_animation_local_transforms(hierarchy, animation, animation_frame)
    }

    /// Apply the frozen active `W3DModelDraw` `ShowSubObject`/`HideSubObject`
    /// state to one already-resolved rigid HLOD mesh.
    ///
    /// C++ first looks up a full subobject name, then the exact substring after
    /// its first dot, and applies the directive to that HTree bone plus all of
    /// its descendants.  Main keeps that lookup strictly inside one supported
    /// source HLOD's retained child records; it never guesses from an arbitrary
    /// mesh, template, or suffix.  Missing/unsupported metadata consequently
    /// leaves the mesh unchanged here (the transform path remains separately
    /// fail-closed for unsupported HLODs).
    pub fn mesh_visible_for_authored_subobject_directives(
        &self,
        mesh_index: usize,
        directives: &[AuthoredDrawSubobjectVisibility],
    ) -> bool {
        if directives.is_empty() || self.hlod_parse_failed || self.hlods.is_empty() {
            return true;
        }

        let Some((mesh_subobject, mesh_bone_index)) =
            self.rigid_hlod_subobject_for_mesh(mesh_index)
        else {
            return true;
        };
        let Some((hlod, lod, hierarchy)) = self.rigid_hlod_static_lod_context() else {
            return true;
        };

        // `ModelConditionInfo::m_hideShowVec` is iterated in its retained
        // declaration order.  A later directive affecting the same child or
        // an ancestor intentionally wins.
        let mut visible = true;
        for directive in directives {
            let Some(target_subobject) =
                Self::rigid_hlod_subobject_for_authored_directive(hlod, lod, &directive.name)
            else {
                continue;
            };
            let Some(target_bone_index) = usize::try_from(target_subobject.bone_index)
                .ok()
                .filter(|bone_index| *bone_index < hierarchy.pivots.len())
            else {
                continue;
            };
            // C++ hides the exact looked-up RenderObj directly, then visits
            // *strict* HTree descendants. A separate sibling on the same bone
            // is neither the named child nor a descendant and must stay intact.
            if mesh_subobject
                .name
                .eq_ignore_ascii_case(target_subobject.name.as_str())
                || Self::hierarchy_bone_is_strict_descendant(
                    hierarchy,
                    mesh_bone_index,
                    target_bone_index,
                )
            {
                visible = !directive.hidden;
            }
        }
        visible
    }

    /// Backwards-compatible transform-only facade for callers that have not
    /// yet retained a source Draw-state animation identity.  An out-of-range
    /// legacy index preserves the old bind-pose fallback rather than silently
    /// selecting another W3D animation.
    pub fn mesh_local_transform_for_animation(
        &self,
        mesh_index: usize,
        animation_index: usize,
        animation_frame: f32,
    ) -> Option<Mat4> {
        self.mesh_local_transform_and_visibility_for_animation(
            mesh_index,
            (animation_index < self.animations.len()).then_some(animation_index),
            animation_frame,
        )
        .map(|(transform, _visible)| transform)
    }

    /// Return the rigid mesh transform in its HTree bind pose.  This is used for
    /// model culling/bounds and has the same fail-closed identity checks as the
    /// animated render path above.
    fn mesh_bind_pose_local_transform(&self, mesh_index: usize) -> Option<Mat4> {
        let mesh = self.meshes.get(mesh_index)?;

        if self.hlod_parse_failed {
            return None;
        }
        if self.hlods.is_empty() {
            return Some(mesh.transform);
        }

        let bone_index = self.rigid_hlod_bone_index_for_mesh(mesh_index)?;
        let hierarchy = self.hierarchy.as_ref()?;
        let source_transform = compute_bind_pose_global_transforms(hierarchy)?
            .get(bone_index)
            .copied()?;
        Some(Self::w3d_transform_to_render_basis(Mat4::from_cols_array(
            &source_transform,
        )))
    }

    /// Resolve a flattened Main mesh back to the precise source HLOD record.
    ///
    /// C++ asks its asset manager for the exact `HLOD.Name.MeshName` render object
    /// and then assigns that object the source `BoneIndex`.  In Main, meshes reside
    /// in the same loaded W3D stream, so require the mesh header's own
    /// `ContainerName` plus that exact composed source identity.  Do not use a
    /// suffix, pivot-name, or template-name fallback here.
    fn rigid_hlod_bone_index_for_mesh(&self, mesh_index: usize) -> Option<usize> {
        self.rigid_hlod_subobject_for_mesh(mesh_index)
            .map(|(_subobject, bone_index)| bone_index)
    }

    /// Resolve one flattened Main mesh to its exact retained rigid HLOD child
    /// and source HTree bone.  Keeping the child identity is necessary for
    /// `ShowSubObject`/`HideSubObject`: C++ directly changes only the matched
    /// render object before it recursively changes descendant bones.
    fn rigid_hlod_subobject_for_mesh(
        &self,
        mesh_index: usize,
    ) -> Option<(&W3dHlodSubObject, usize)> {
        let (hlod, lod, hierarchy) = self.rigid_hlod_static_lod_context()?;

        let mesh = self.meshes.get(mesh_index)?;
        if mesh.container_name.is_empty()
            || !mesh.container_name.eq_ignore_ascii_case(hlod.name.as_str())
        {
            return None;
        }
        let source_identity = format!("{}.{}", mesh.container_name, mesh.name);
        let subobject = lod.subobjects.iter().find(|subobject| {
            subobject
                .name
                .eq_ignore_ascii_case(source_identity.as_str())
        })?;

        let bone_index = usize::try_from(subobject.bone_index).ok()?;
        (bone_index < hierarchy.pivots.len()).then_some((subobject, bone_index))
    }

    /// C++ `HLodClass` construction starts from `CurLod == 0`, calls
    /// `Calculate_Cost_Value_Arrays(1.0f, ...)`, then raises it to the returned
    /// minimum level.  It uses a strict `<` comparison against each ordered
    /// `MaxScreenSize`; if every level is below one pixel, the final (highest
    /// detail) level is selected.  The W3D exporter stores levels low-to-high.
    ///
    /// Generals' `RTS3DScene` explicitly disables its later dynamic
    /// `Prepare_LOD`/optimizer calls, so this frozen construction selection is
    /// the only HLOD selection Main may perform without inventing behavior.
    /// Malformed thresholds fail closed instead of making an arbitrary level
    /// visible.
    fn cxx_constructor_selected_hlod_lod_index(hlod: &W3dHlod) -> Option<usize> {
        let lods = &hlod.lods;
        if lods.is_empty() || lods.iter().any(|lod| !lod.max_screen_size.is_finite()) {
            return None;
        }

        let mut min_lod = 0;
        while min_lod < lods.len() && lods[min_lod].max_screen_size < 1.0 {
            min_lod += 1;
        }
        Some(min_lod.min(lods.len() - 1))
    }

    /// The source-valid parent HLOD/HTree topology shared by normal rigid
    /// children and C++ `AdditionalModels`. Aggregate entries are deliberately
    /// allowed here so their parent-bone poses can be prepared without
    /// pretending their external geometry is already rendered.
    fn static_hlod_parent_context(&self) -> Option<(&W3dHlod, &W3dHlodLod, &W3dHierarchy)> {
        if self.hlod_parse_failed || self.hlods.len() != 1 {
            return None;
        }
        let hlod = self.hlods.first()?;
        if hlod.has_invalid_trailing_records {
            return None;
        }
        let lod = hlod
            .lods
            .get(Self::cxx_constructor_selected_hlod_lod_index(hlod)?)?;

        let hierarchy = self.hierarchy.as_ref()?;
        if hlod.name.is_empty()
            || hlod.hierarchy_name.is_empty()
            || !hlod
                .hierarchy_name
                .eq_ignore_ascii_case(hierarchy.name.as_str())
        {
            return None;
        }
        Some((hlod, lod, hierarchy))
    }

    /// The bounded rigid geometry topology that can safely use the C++
    /// constructor-selected static level: one source HLOD, an exact matching
    /// source hierarchy, and an intact selected level.
    ///
    /// `HLodClass` creates every aggregate independently. A missing aggregate
    /// render object or an aggregate with an invalid parent bone does not
    /// invalidate the selected parent LOD, so retained aggregate metadata must
    /// never hide otherwise-valid parent geometry. Proxies likewise remain
    /// non-rendering application data.
    /// Every caller must preserve this gate rather than treating flattened
    /// mesh names as a substitute.
    fn rigid_hlod_static_lod_context(&self) -> Option<(&W3dHlod, &W3dHlodLod, &W3dHierarchy)> {
        self.static_hlod_parent_context()
    }

    /// The deliberately narrower topology required by current turret/recoil
    /// state.  Static multi-LOD geometry is safe above, but visual controls
    /// still require a separately validated one-level HLOD rather than being
    /// projected onto a selected level by inference.
    fn rigid_hlod_context(&self) -> Option<(&W3dHlod, &W3dHierarchy)> {
        let (hlod, _lod, hierarchy) = self.rigid_hlod_static_lod_context()?;
        (hlod.lods.len() == 1).then_some((hlod, hierarchy))
    }

    /// Resolve C++ `RenderObjClass::Get_Sub_Object_By_Name` only through a
    /// structurally valid retained HLOD child.  Its first pass compares the
    /// full source child name; its second pass compares the exact text after
    /// the first dot.  We require the child record to have this HLOD's exact
    /// prefix, so no unrelated mesh-name suffix can become visibility authority.
    fn rigid_hlod_subobject_for_authored_directive<'a>(
        hlod: &'a W3dHlod,
        lod: &'a W3dHlodLod,
        directive_name: &str,
    ) -> Option<&'a W3dHlodSubObject> {
        let directive_name = directive_name.trim();
        if directive_name.is_empty() {
            return None;
        }
        let subobjects = &lod.subobjects;
        subobjects
            .iter()
            .find(|subobject| {
                Self::rigid_hlod_child_leaf_name(hlod, subobject).is_some()
                    && subobject.name.eq_ignore_ascii_case(directive_name)
            })
            .or_else(|| {
                subobjects.iter().find(|subobject| {
                    Self::rigid_hlod_child_leaf_name(hlod, subobject)
                        .is_some_and(|leaf_name| leaf_name.eq_ignore_ascii_case(directive_name))
                })
            })
    }

    /// Return the C++ first-dot suffix only for a source record structurally
    /// owned by this exact HLOD.  A bare or differently-prefixed name is not a
    /// valid child mapping in Main's bounded rigid HLOD implementation.
    fn rigid_hlod_child_leaf_name<'a>(
        hlod: &W3dHlod,
        subobject: &'a W3dHlodSubObject,
    ) -> Option<&'a str> {
        let (prefix, leaf_name) = subobject.name.split_once('.')?;
        (!leaf_name.is_empty() && prefix.eq_ignore_ascii_case(hlod.name.as_str()))
            .then_some(leaf_name)
    }

    /// Whether `bone_index` lies strictly below `ancestor_bone_index` in the
    /// source HTree. The exact direct target is handled separately, matching
    /// C++ `doHideShowSubObjs` plus `doHideShowBoneSubObjs`. The bounded walk
    /// rejects malformed roots/cycles instead of treating invalid parent data
    /// as visible geometry.
    fn hierarchy_bone_is_strict_descendant(
        hierarchy: &W3dHierarchy,
        bone_index: usize,
        ancestor_bone_index: usize,
    ) -> bool {
        let mut current_bone_index = bone_index;
        for _ in 0..hierarchy.pivots.len() {
            let Some(pivot) = hierarchy.pivots.get(current_bone_index) else {
                return false;
            };
            if pivot.parent_idx == u32::MAX {
                return false;
            }
            let Ok(parent_bone_index) = usize::try_from(pivot.parent_idx) else {
                return false;
            };
            if parent_bone_index >= hierarchy.pivots.len()
                || parent_bone_index == current_bone_index
            {
                return false;
            }
            if parent_bone_index == ancestor_bone_index {
                return true;
            }
            current_bone_index = parent_bone_index;
        }
        false
    }

    /// Convert a source W3D Z-up matrix to the render basis used by imported
    /// `W3DVertex` payloads.  The axis swap is its own inverse.
    fn w3d_transform_to_render_basis(transform: Mat4) -> Mat4 {
        let axis = Mat4::from_cols_array(&[
            1.0, 0.0, 0.0, 0.0, // X stays X
            0.0, 0.0, 1.0, 0.0, // source Y becomes render Z
            0.0, 1.0, 0.0, 0.0, // source Z becomes render Y
            0.0, 0.0, 0.0, 1.0,
        ]);
        axis * transform * axis
    }

    /// Get the list of animation names available on this model.
    pub fn animation_names(&self) -> Vec<&str> {
        self.animations.iter().map(|a| a.name.as_str()).collect()
    }

    /// Find an animation index by name (case-insensitive).
    pub fn find_animation_index(&self, name: &str) -> Option<usize> {
        let lower = name.to_ascii_lowercase();
        self.animations
            .iter()
            .position(|a| a.name.to_ascii_lowercase() == lower)
    }

    /// Resolve a C++ `W3DModelDraw` animation identity against this exact W3D
    /// file.  Retail Object INIs commonly use the canonical
    /// `Hierarchy.Animation` spelling while a raw W3D animation header stores
    /// those two source records separately.  This is an exact qualified-record
    /// comparison, not a basename/suffix heuristic.
    pub fn find_animation_index_for_draw_identity(&self, identity: &str) -> Option<usize> {
        self.animations
            .iter()
            .position(|animation| animation.matches_draw_identity(identity))
    }

    /// Resolve an exact Draw identity only when this geometry file itself
    /// carries a compatible raw HAnim. The caller may then try C++'s companion
    /// `Animation.w3d` rule; it must never substitute a local clip by ordinal.
    pub fn local_animation_binding_for_draw_identity(
        &self,
        identity: &str,
    ) -> Option<W3dAnimationBinding> {
        let binding =
            W3dAnimationBinding::local(self.find_animation_index_for_draw_identity(identity)?);
        self.animation_binding_is_compatible(&binding)
            .then_some(binding)
    }

    /// Return whether a frozen Draw HAnim can be sampled against this model's
    /// actual hierarchy. Companion clips remain separate assets, but C++ binds
    /// them to the named HTree only; a matching clip name alone is insufficient.
    pub fn animation_binding_is_compatible(&self, binding: &W3dAnimationBinding) -> bool {
        let Some(hierarchy) = self.hierarchy.as_ref() else {
            return false;
        };
        let Some(animation) = binding.animation(self) else {
            return false;
        };
        if animation.hierarchy_name.trim().is_empty()
            || animation.name.trim().is_empty()
            || animation.num_frames == 0
            || animation.frame_rate == 0
            || !animation
                .hierarchy_name
                .eq_ignore_ascii_case(hierarchy.name.as_str())
        {
            return false;
        }

        match binding {
            W3dAnimationBinding::Local { .. } => true,
            W3dAnimationBinding::Companion { identity, .. } => {
                !animation.source_is_compressed && animation.matches_draw_identity(identity)
            }
        }
    }

    /// Get animation metadata: (num_frames, frame_rate) for the given animation.
    pub fn animation_metadata(&self, anim_index: usize) -> Option<(u32, u32)> {
        let anim = self.animations.get(anim_index)?;
        Some((anim.num_frames, anim.frame_rate))
    }

    /// Metadata for one exact frozen animation binding. An incompatible
    /// companion is treated as unavailable so callers stay in bind pose.
    pub fn animation_binding_metadata(&self, binding: &W3dAnimationBinding) -> Option<(u32, u32)> {
        self.animation_binding_is_compatible(binding)
            .then(|| binding.animation(self))
            .flatten()
            .map(|animation| (animation.num_frames, animation.frame_rate))
    }

    /// Sample an animation at the given frame, producing per-bone global transforms.
    ///
    /// Returns a Vec of column-major 4x4 matrices indexed by pivot (bone) index,
    /// or `None` if the animation or hierarchy is missing.
    ///
    /// The frame parameter is a continuous value; fractional parts interpolate
    /// between adjacent keyframes.
    pub fn sample_animation(&self, anim_index: usize, frame: f32) -> Option<Vec<[f32; 16]>> {
        let anim = self.animations.get(anim_index)?;
        self.sample_animation_data(anim, frame)
    }

    /// Sample a selected local or exact companion HAnim. This performs the
    /// hierarchy validation at the final palette boundary too, so an invalid
    /// companion cannot turn into a local animation-zero pose downstream.
    pub fn sample_animation_binding(
        &self,
        binding: &W3dAnimationBinding,
        frame: f32,
    ) -> Option<Vec<[f32; 16]>> {
        if !self.animation_binding_is_compatible(binding) {
            return None;
        }
        self.sample_animation_data(binding.animation(self)?, frame)
    }

    fn sample_animation_data(&self, anim: &W3dAnimation, frame: f32) -> Option<Vec<[f32; 16]>> {
        let hierarchy = self.hierarchy.as_ref()?;
        let local_transforms = sample_animation_local_transforms(hierarchy, anim, frame)?;
        compute_htree_global_transforms_from_locals(hierarchy, &local_transforms)
    }
}

/// Build a column-major 4x4 matrix from a pivot's translation + quaternion rotation.
/// Same logic as W3DLoader::mat4_from_tr_quat but operates on W3dPivot directly.
fn mat4_from_pivot(pivot: &W3dPivot) -> [f32; 16] {
    mat4_from_translation_and_quaternion(pivot.translation, pivot.rotation)
}

fn mat4_from_translation_and_quaternion(translation: [f32; 3], rotation: [f32; 4]) -> [f32; 16] {
    let x = rotation[0];
    let y = rotation[1];
    let z = rotation[2];
    let w = rotation[3];
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    let m00 = 1.0 - 2.0 * (yy + zz);
    let m01 = 2.0 * (xy - wz);
    let m02 = 2.0 * (xz + wy);
    let m10 = 2.0 * (xy + wz);
    let m11 = 1.0 - 2.0 * (xx + zz);
    let m12 = 2.0 * (yz - wx);
    let m20 = 2.0 * (xz - wy);
    let m21 = 2.0 * (yz + wx);
    let m22 = 1.0 - 2.0 * (xx + yy);
    let tx = translation[0];
    let ty = translation[1];
    let tz = translation[2];
    [
        m00, m10, m20, 0.0, m01, m11, m21, 0.0, m02, m12, m22, 0.0, tx, ty, tz, 1.0,
    ]
}

/// Build source-space HTree local transforms for the selected W3D animation.
///
/// Generals runs ordinary raw W3D animations through its specialized
/// `HTreeClass::Anim_Update(HRawAnimClass*, ...)`, not the generic interpolated
/// HAnim path.  Keep the existing compressed-channel implementation isolated:
/// that format takes the generic path and must not silently inherit raw-frame
/// behavior merely because both records share [`W3dAnimation`].
fn sample_animation_local_transforms(
    hierarchy: &W3dHierarchy,
    anim: &W3dAnimation,
    frame: f32,
) -> Option<Vec<[f32; 16]>> {
    if anim.source_is_compressed {
        return sample_compressed_animation_local_transforms(hierarchy, anim, frame);
    }

    if !frame.is_finite() {
        return None;
    }
    let raw_frame = anim.raw_frame_index(frame)?;
    let mut local_transforms: Vec<[f32; 16]> =
        hierarchy.pivots.iter().map(mat4_from_pivot).collect();
    let mut motion_channels: Vec<[Option<&W3dAnimChannel>; 4]> =
        vec![[None; 4]; hierarchy.pivots.len()];

    // `HRawAnimClass::add_channel` keeps one X/Y/Z/Q pointer per pivot and
    // replaces an earlier pointer for the same channel kind.  Preserve that
    // exact final-source-record authority rather than sequentially composing
    // duplicate W3D chunks.
    for channel in &anim.channels {
        let Some(slot) = raw_motion_channel_slot(channel.flags) else {
            continue;
        };
        let pivot_index = usize::from(channel.pivot);
        if pivot_index < motion_channels.len() {
            motion_channels[pivot_index][slot] = Some(channel);
        }
    }

    // C++ sets pivot zero to the external RenderObj root and begins the raw
    // node-motion walk at one. Source pivot-zero channels are intentionally
    // not sampled, including malformed ones.
    for pivot_index in 1..local_transforms.len() {
        let channels = motion_channels[pivot_index];
        let translation = [
            raw_scalar_channel_value(channels[0], raw_frame)?,
            raw_scalar_channel_value(channels[1], raw_frame)?,
            raw_scalar_channel_value(channels[2], raw_frame)?,
        ];
        let rotation = raw_quaternion_channel_value(channels[3], raw_frame)?;

        // `HTreeClass::Anim_Update(HRawAnimClass*)` first obtains
        // parent * BaseTransform, then Matrix3D::Translate and postMul(q).
        // Associativity permits the local equivalent here: Base * T * Q;
        // the shared HTree evaluator supplies the parent afterward.
        let with_translation = mat4_mul(
            &local_transforms[pivot_index],
            &mat4_from_translation(translation),
        );
        local_transforms[pivot_index] =
            mat4_mul(&with_translation, &mat4_from_quaternion(rotation));
    }

    Some(local_transforms)
}

/// The compact source `ANIM_CHANNEL_*` kinds that the Generals raw HAnim path
/// installs in one `NodeMotionStruct`. Other raw motion kinds are retained by
/// the parser but are not consumed by the specialized game update path.
fn raw_motion_channel_slot(flags: u16) -> Option<usize> {
    match flags {
        0 => Some(0),
        1 => Some(1),
        2 => Some(2),
        6 => Some(3),
        _ => None,
    }
}

/// `MotionChannelClass::Get_Vector` returns scalar zero outside an authored
/// range. A malformed scalar record is not source-usable, so fail the pose
/// rather than treating a truncated payload as a real zero channel.
fn raw_scalar_channel_value(channel: Option<&W3dAnimChannel>, frame: i32) -> Option<f32> {
    let Some(channel) = channel else {
        return Some(0.0);
    };
    if channel.vector_len != 1 || channel.last_frame < channel.first_frame {
        return None;
    }
    let first = i32::from(channel.first_frame);
    let last = i32::from(channel.last_frame);
    if frame < first || frame > last {
        return Some(0.0);
    }
    let index = usize::try_from(frame - first).ok()?;
    channel.data.get(index).copied()
}

/// `MotionChannelClass::Get_Vector_As_Quat` returns the identity quaternion
/// outside an authored range and does not normalize authored raw values.
fn raw_quaternion_channel_value(channel: Option<&W3dAnimChannel>, frame: i32) -> Option<[f32; 4]> {
    let Some(channel) = channel else {
        return Some([0.0, 0.0, 0.0, 1.0]);
    };
    if channel.vector_len != 4 || channel.last_frame < channel.first_frame {
        return None;
    }
    let first = i32::from(channel.first_frame);
    let last = i32::from(channel.last_frame);
    if frame < first || frame > last {
        return Some([0.0, 0.0, 0.0, 1.0]);
    }
    let index = usize::try_from(frame - first).ok()?.checked_mul(4)?;
    Some([
        *channel.data.get(index)?,
        *channel.data.get(index + 1)?,
        *channel.data.get(index + 2)?,
        *channel.data.get(index + 3)?,
    ])
}

fn mat4_from_translation(translation: [f32; 3]) -> [f32; 16] {
    [
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
        translation[0],
        translation[1],
        translation[2],
        1.0,
    ]
}

fn mat4_from_quaternion(rotation: [f32; 4]) -> [f32; 16] {
    mat4_from_translation_and_quaternion([0.0; 3], rotation)
}

/// Preserve the pre-existing generic compressed-HAnim behavior verbatim.
/// Raw HAnim deliberately uses local post-composition instead; this helper is
/// not part of Generals' specialized raw path.
fn replace_rotation_preserving_translation(m: &mut [f32; 16], qx: f32, qy: f32, qz: f32, qw: f32) {
    let xx = qx * qx;
    let yy = qy * qy;
    let zz = qz * qz;
    let xy = qx * qy;
    let xz = qx * qz;
    let yz = qy * qz;
    let wx = qw * qx;
    let wy = qw * qy;
    let wz = qw * qz;
    m[0] = 1.0 - 2.0 * (yy + zz);
    m[1] = 2.0 * (xy + wz);
    m[2] = 2.0 * (xz - wy);
    m[4] = 2.0 * (xy - wz);
    m[5] = 1.0 - 2.0 * (xx + zz);
    m[6] = 2.0 * (yz + wx);
    m[8] = 2.0 * (xz + wy);
    m[9] = 2.0 * (yz - wx);
    m[10] = 1.0 - 2.0 * (xx + yy);
}

/// Existing generic compressed-HAnim local-channel behavior. The source
/// compressed decoder is independently incomplete, but its current support
/// must not be reinterpreted through Generals' raw HAnim specialization.
fn sample_compressed_animation_local_transforms(
    hierarchy: &W3dHierarchy,
    anim: &W3dAnimation,
    frame: f32,
) -> Option<Vec<[f32; 16]>> {
    if !frame.is_finite() {
        return None;
    }
    let mut local_transforms: Vec<[f32; 16]> =
        hierarchy.pivots.iter().map(mat4_from_pivot).collect();

    for channel in &anim.channels {
        let pivot_idx = usize::from(channel.pivot);
        if pivot_idx >= local_transforms.len() {
            continue;
        }

        let values = sample_compressed_channel(channel, frame);

        match channel.flags {
            0 => {
                if let Some(v) = values.first() {
                    local_transforms[pivot_idx][12] = *v;
                }
            }
            1 => {
                if let Some(v) = values.first() {
                    local_transforms[pivot_idx][13] = *v;
                }
            }
            2 => {
                if let Some(v) = values.first() {
                    local_transforms[pivot_idx][14] = *v;
                }
            }
            6 if values.len() >= 4 => {
                replace_rotation_preserving_translation(
                    &mut local_transforms[pivot_idx],
                    values[0],
                    values[1],
                    values[2],
                    values[3],
                );
            }
            _ => {}
        }
    }

    Some(local_transforms)
}

/// Interpolate an animation channel at the given continuous frame value.
/// Returns the interpolated values (1 for scalar channels, 4 for quaternion).
fn sample_compressed_channel(channel: &W3dAnimChannel, frame: f32) -> Vec<f32> {
    let first = channel.first_frame as f32;
    let last = channel.last_frame as f32;

    // Clamp frame to channel range
    let t = (frame - first).max(0.0).min((last - first).max(0.0));

    let vl = channel.vector_len as usize;
    if vl == 0 || channel.data.is_empty() {
        return Vec::new();
    }

    // Number of keyframes in this channel
    let num_keys = channel.data.len() / vl;
    if num_keys == 0 {
        return vec![0.0; vl];
    }

    let frame_idx = (t as usize).min(num_keys - 1);
    let frac = t - frame_idx as f32;

    let idx0 = frame_idx * vl;
    let idx1 = if frame_idx + 1 < num_keys {
        (frame_idx + 1) * vl
    } else {
        idx0
    };

    if idx0 + vl > channel.data.len() {
        return vec![0.0; vl];
    }

    // Linear interpolation between adjacent keyframes
    let mut result = Vec::with_capacity(vl);
    for i in 0..vl {
        let a = channel.data[idx0 + i];
        let b = if idx1 + i < channel.data.len() {
            channel.data[idx1 + i]
        } else {
            a
        };
        result.push(a + (b - a) * frac);
    }

    // For quaternion channels (flags=6), normalize to unit quaternion
    if channel.flags == 6 && result.len() == 4 {
        let len = (result[0] * result[0]
            + result[1] * result[1]
            + result[2] * result[2]
            + result[3] * result[3])
            .sqrt();
        if len > 1e-10 {
            for v in result.iter_mut() {
                *v /= len;
            }
        }
    }

    result
}

/// Multiply two source W3D affine transforms in C++ `Matrix3D::Multiply`
/// order: `a * b`.
///
/// Source `Matrix3D` stores three logical rows and transforms column vectors;
/// Main retains that same transform in glam's column-major array layout.  Do
/// not use the old row/column-swapped loop here: it evaluated `b * a`, which
/// happens to pass translation-only fixtures but makes a child translation
/// ignore its rotated parent.  Keeping the three-by-four arithmetic explicit
/// also matches C++'s affine multiplication rather than accidentally granting
/// source HTree controls projective semantics.
fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut r = [0.0; 16];
    for column in 0..3 {
        for row in 0..3 {
            r[column * 4 + row] = a[row] * b[column * 4]
                + a[4 + row] * b[column * 4 + 1]
                + a[8 + row] * b[column * 4 + 2];
        }
    }
    for row in 0..3 {
        r[12 + row] = a[row] * b[12] + a[4 + row] * b[13] + a[8 + row] * b[14] + a[12 + row];
    }
    r[15] = 1.0;
    r
}

/// C++ `HTreeClass::{Base,Anim}_Update` source-space globals for a model
/// whose object/world transform is deliberately supplied by its caller.
///
/// `HTreeClass` overwrites pivot zero with that external object root and forces
/// it visible; it does not apply pivot-zero W3D bind/animation data. Main's
/// aggregate pose API leaves the object transform outside so it can compose
/// the parent RenderItem world matrix exactly once, therefore pivot zero is
/// the identity here. All non-root pivots retain the ordinary ordered
/// parent-local/capture update semantics.
fn compute_htree_global_transforms_from_locals_with_capture_controls(
    hierarchy: &W3dHierarchy,
    locals: &[[f32; 16]],
    capture_controls: &[Option<[f32; 16]>],
) -> Option<Vec<[f32; 16]>> {
    if locals.len() != hierarchy.pivots.len()
        || capture_controls.len() != hierarchy.pivots.len()
        || hierarchy.pivots.is_empty()
        || hierarchy.pivots[0].parent_idx != u32::MAX
    {
        return None;
    }

    let mut globals: Vec<[f32; 16]> = vec![[0.0; 16]; hierarchy.pivots.len()];
    globals[0] = Mat4::IDENTITY.to_cols_array();
    for (pivot_index, pivot) in hierarchy.pivots.iter().enumerate().skip(1) {
        let parent_index = usize::try_from(pivot.parent_idx).ok()?;
        // Source HTree stores parent pivots before children. Main has no
        // recursive malformed-order evaluator, so do not fabricate a parent.
        if parent_index >= pivot_index {
            return None;
        }
        let mut global = mat4_mul(&globals[parent_index], &locals[pivot_index]);
        if let Some(control) = capture_controls[pivot_index] {
            global = mat4_mul(&global, &control);
        }
        globals[pivot_index] = global;
    }
    Some(globals)
}

/// As [`compute_htree_global_transforms_from_locals_with_capture_controls`],
/// without C++ `Capture_Bone` controls.  Every runtime HTree caller goes
/// through this wrapper so pivot zero always remains the external object root
/// rather than leaking W3D bind or HAnim-local data into child transforms.
fn compute_htree_global_transforms_from_locals(
    hierarchy: &W3dHierarchy,
    locals: &[[f32; 16]],
) -> Option<Vec<[f32; 16]>> {
    compute_htree_global_transforms_from_locals_with_capture_controls(
        hierarchy,
        locals,
        &vec![None; hierarchy.pivots.len()],
    )
}

/// Compute the HTree bind-pose globals from source W3D pivot data.
///
/// Both static rigid HLOD children and animation sampling use the same hierarchy
/// convention.  Keeping this outside `W3DLoader` prevents a render-time HLOD
/// binding from accidentally depending on loader-only state.
fn compute_bind_pose_global_transforms(hierarchy: &W3dHierarchy) -> Option<Vec<[f32; 16]>> {
    let locals: Vec<[f32; 16]> = hierarchy.pivots.iter().map(mat4_from_pivot).collect();
    compute_htree_global_transforms_from_locals(hierarchy, &locals)
}

/// W3D model loader
pub struct W3DLoader;

impl Default for W3DLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip directory + extension from a W3D model request (keeps original casing).
fn w3d_model_basename(model_name: &str) -> &str {
    model_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(model_name)
        .trim()
        .trim_end_matches(".w3d")
        .trim_end_matches(".W3D")
}

/// Archive path candidates for `W3DLoader::load_model`.
///
/// Retail ZH BIG archives store mixed-case paths such as
/// `Art/W3D/ABBtCmdHQ.W3D`. Linux extract trees are case-sensitive, so
/// both `art/w3d/*.w3d` and `Art/W3D/*.W3D` (plus remapped / retail
/// basenames) must be tried. BIG lookup is already case-insensitive;
/// variants exist so a case-sensitive overlay / extract tree still hits.
pub fn w3d_archive_path_variants(model_name: &str) -> Vec<String> {
    use crate::assets::mesh_asset_resolve::{
        remap_model_key_alias, retail_w3d_basename_for_key, w3d_filename_variants,
    };

    let base = w3d_model_basename(model_name);
    let remapped = remap_model_key_alias(base);
    let retail = retail_w3d_basename_for_key(base);

    let mut stems = Vec::new();
    let mut push_stem = |stem: &str| {
        if stem.is_empty() {
            return;
        }
        if !stems.iter().any(|existing: &String| existing == stem) {
            stems.push(stem.to_string());
        }
    };
    // Requested basename first (preserves the original two load_model paths).
    push_stem(base);
    // Retail archive casing (AIRanger_S, ABBtCmdHQ, AvHummer, …).
    push_stem(&retail);
    // Remapped alias (AmericaCommandCenter → abbtcmdhq).
    push_stem(&remapped);

    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push_path = |path: String| {
        if seen.insert(path.clone()) {
            out.push(path);
        }
    };

    for stem in &stems {
        // Existing load_model candidates, then retail BIG / extract mixed-case.
        push_path(format!("art/w3d/{stem}.w3d"));
        push_path(format!("{stem}.w3d"));
        push_path(format!("Art/W3D/{stem}.w3d"));
        push_path(format!("Art/W3D/{stem}.W3D"));
        push_path(format!("art/w3d/{stem}.W3D"));

        // Extra filename casings from mesh residual tables.
        for name in w3d_filename_variants(stem) {
            push_path(format!("art/w3d/{name}"));
            push_path(name.clone());
            push_path(format!("Art/W3D/{name}"));
        }
    }

    out
}

/// Exact C++ companion-file spelling for a raw HAnim selected by
/// `W3DModelDraw`.
///
/// `WW3DAssetManager::Get_HAnim` resolves an HAnim by name first and, on a
/// miss, takes the substring after the first dot in `Hierarchy.Animation`,
/// then loads `Animation.w3d`. This intentionally does not use any model-key
/// alias or filename-family table.
pub fn w3d_companion_animation_filename(identity: &str) -> Option<String> {
    let (_, animation) = split_w3d_draw_animation_identity(identity)?;
    Some(format!("{animation}.w3d"))
}

/// Case-only archive/extract variants for one exact companion HAnim file.
///
/// BIG lookups are case-insensitive, while retail extracted trees may not be.
/// These paths vary only storage/path casing; they never remap a Draw identity
/// to a different model or scan an archive for similarly named motions.
pub fn w3d_companion_animation_archive_path_variants(identity: &str) -> Option<Vec<String>> {
    let (_, animation) = split_w3d_draw_animation_identity(identity)?;
    let mut stems = Vec::new();
    for stem in [
        animation.to_string(),
        animation.to_ascii_lowercase(),
        animation.to_ascii_uppercase(),
    ] {
        if !stems.iter().any(|existing: &String| existing == &stem) {
            stems.push(stem);
        }
    }

    let mut paths = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut push = |path: String| {
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    };
    for stem in stems {
        push(format!("art/w3d/{stem}.w3d"));
        push(format!("{stem}.w3d"));
        push(format!("Art/W3D/{stem}.w3d"));
        push(format!("Art/W3D/{stem}.W3D"));
        push(format!("art/w3d/{stem}.W3D"));
    }
    Some(paths)
}

impl W3DLoader {
    /// Create new W3D loader
    pub fn new() -> Self {
        Self
    }

    /// Load W3D model from BIG archive
    pub async fn load_model(
        &self,
        archive_system: &mut ArchiveFileSystem,
        model_name: &str,
    ) -> Result<W3DModel> {
        debug!("Loading W3D model: {}", model_name);

        // C++ parity: deterministic model lookup (requested file, remapped alias,
        // and retail Art/W3D mixed-case archive location).
        let base_name = w3d_model_basename(model_name);
        let w3d_filename = format!("{base_name}.w3d");
        let path_variations = w3d_archive_path_variants(model_name);

        let mut last_error = None;
        for path_variant in path_variations {
            debug!("Trying W3D path: {}", path_variant);
            match archive_system.open_file(&path_variant).await {
                Ok(model_data) => {
                    debug!("Found W3D file at path: {}", path_variant);
                    debug!("Loaded W3D file data: {} bytes", model_data.len());
                    return self.parse_w3d_data(&model_data, base_name.to_string());
                }
                Err(e) => {
                    debug!("Failed to find W3D at {}: {}", path_variant, e);
                    last_error = Some(e);
                }
            }
        }

        Err(anyhow!(
            "Failed to load W3D file {}: {}",
            w3d_filename,
            last_error.unwrap_or_else(|| anyhow!("file not found"))
        ))
    }

    /// Load one exact raw-animation companion according to the C++ HAnim
    /// fallback rule. The returned clip remains separate from a geometry
    /// model; callers must still validate its hierarchy before binding it.
    pub async fn load_companion_animation(
        &self,
        archive_system: &mut ArchiveFileSystem,
        identity: &str,
    ) -> Result<W3dAnimation> {
        let filename = w3d_companion_animation_filename(identity)
            .ok_or_else(|| anyhow!("invalid W3D Draw animation identity '{identity}'"))?;
        let candidates = w3d_companion_animation_archive_path_variants(identity)
            .expect("validated companion identity must yield paths");
        let mut last_error = None;

        for candidate in candidates {
            match archive_system.open_file(&candidate).await {
                Ok(data) => match self.load_companion_animation_from_bytes(&data, identity) {
                    Ok(animation) => return Ok(animation),
                    Err(error) => {
                        last_error = Some(error);
                        debug!(
                            "W3D companion '{}' at '{}' did not contain its exact HAnim: {}",
                            identity,
                            candidate,
                            last_error.as_ref().expect("just assigned error")
                        );
                    }
                },
                Err(error) => last_error = Some(error),
            }
        }

        Err(anyhow!(
            "failed to load exact W3D companion '{}' for Draw animation '{}': {}",
            filename,
            identity,
            last_error.unwrap_or_else(|| anyhow!("file not found"))
        ))
    }

    /// Parse W3D binary data using the legacy chunk parser path for strict C++ parity.
    fn parse_w3d_data(&self, data: &[u8], model_name: String) -> Result<W3DModel> {
        self.parse_w3d_data_legacy(data, model_name, false)
    }

    /// Parse an HAnim companion stream. Retail raw-animation W3Ds commonly
    /// contain no mesh chunks at all, unlike a geometry model, so the normal
    /// model parser's no-mesh rejection is not applicable here.
    fn parse_w3d_animation_data(&self, data: &[u8], model_name: String) -> Result<W3DModel> {
        self.parse_w3d_data_legacy(data, model_name, true)
    }

    /// Parse a W3D model from already-loaded bytes (filesystem / archive residual path).
    ///
    /// Used by mesh asset resolve when assets are present without a full GPU/AssetManager
    /// init. Fail-closed: not full material/animation retail parity.
    pub fn load_model_from_bytes(&self, data: &[u8], model_name: &str) -> Result<W3DModel> {
        let base_name = model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D");
        if data.is_empty() {
            return Err(anyhow!("empty W3D payload for '{}'", base_name));
        }
        self.parse_w3d_data(data, base_name.to_string())
    }

    /// Parse a raw-animation W3D payload and select only the exact full
    /// `Hierarchy.Animation` record requested by the frozen Draw state.
    pub fn load_companion_animation_from_bytes(
        &self,
        data: &[u8],
        identity: &str,
    ) -> Result<W3dAnimation> {
        let filename = w3d_companion_animation_filename(identity)
            .ok_or_else(|| anyhow!("invalid W3D Draw animation identity '{identity}'"))?;
        if data.is_empty() {
            return Err(anyhow!("empty W3D companion payload for '{filename}'"));
        }
        let model = self.parse_w3d_animation_data(data, filename.clone())?;
        model
            .animations
            .into_iter()
            .find(|animation| animation.matches_draw_identity(identity))
            .ok_or_else(|| {
                anyhow!(
                    "W3D companion '{}' contains no exact HAnim '{}'",
                    filename,
                    identity
                )
            })
    }

    /// Load a W3D model from a filesystem path when present (tests / residual resolve).
    pub fn load_model_from_path(&self, path: &std::path::Path) -> Result<W3DModel> {
        let data = std::fs::read(path)
            .map_err(|e| anyhow!("failed to read W3D '{}': {e}", path.display()))?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("model");
        self.load_model_from_bytes(&data, name)
    }

    // Non-parity companion/heuristic model-family merge path removed.
    // The active parser path is strict legacy chunk parsing (`parse_w3d_data_legacy`).

    // Non-parity companion source loading and alternate ww3d-assets parsing entrypoints removed.

    fn stage_channel_to_uv_source(channel: u8) -> UVSource {
        match channel {
            0 => UVSource::UV0,
            1 => UVSource::UV1,
            2 => UVSource::UV2,
            _ => UVSource::UV3,
        }
    }

    fn stage_mapping_mut(
        material: &mut W3DMaterial,
        stage: usize,
        create: bool,
    ) -> Option<&mut TextureStageMapping> {
        match stage {
            0 => Some(&mut material.stage0_mapping),
            1 => {
                if material.stage1_mapping.is_none() && create {
                    material.stage1_mapping = Some(TextureStageMapping::default());
                }
                material.stage1_mapping.as_mut()
            }
            2 => {
                if material.stage2_mapping.is_none() && create {
                    material.stage2_mapping = Some(TextureStageMapping::default());
                }
                material.stage2_mapping.as_mut()
            }
            3 => {
                if material.stage3_mapping.is_none() && create {
                    material.stage3_mapping = Some(TextureStageMapping::default());
                }
                material.stage3_mapping.as_mut()
            }
            _ => None,
        }
    }

    fn apply_material_stage_mappings(material: &mut W3DMaterial, mesh: &W3DMesh) {
        for stage_idx in 0..4 {
            let create = stage_idx == 0
                || mesh.stage_uv_channels.get(stage_idx).is_some()
                || Self::stage_texture_from_mesh(mesh, 0, stage_idx).is_some();

            if let Some(mapping) = Self::stage_mapping_mut(material, stage_idx, create) {
                if mapping.texture_name.is_none() {
                    if let Some(name) = Self::stage_texture_from_mesh(mesh, 0, stage_idx) {
                        mapping.texture_name = Some(name);
                    }
                }
            }
        }

        for (stage_idx, &channel) in mesh.stage_uv_channels.iter().enumerate().take(4) {
            if let Some(mapping) = Self::stage_mapping_mut(material, stage_idx, true) {
                mapping.uv_source = Self::stage_channel_to_uv_source(channel);
            }
        }

        // Match the common C++ material path more closely: once stage 0 resolves to a texture,
        // expose that as the primary material texture too so caches/debugging/legacy consumers
        // don't diverge from the active pass state.
        if material.texture_name.is_none() {
            material.texture_name = material.stage0_mapping.texture_name.clone();
        }
    }

    fn stage_texture_from_mesh(
        mesh: &W3DMesh,
        pass_index: usize,
        stage_index: usize,
    ) -> Option<String> {
        if let Some(stage_sets) = mesh.per_pass_stage_texture_names.get(pass_index) {
            if let Some(names) = stage_sets.get(stage_index) {
                if let Some(name) = names.iter().find(|n| !n.is_empty()) {
                    return Some(name.clone());
                }
            }
        }

        mesh.stage_texture_names_from_ids(pass_index, stage_index)
            .into_iter()
            .find(|name| !name.is_empty())
    }

    /// Parse W3D binary data using the legacy chunk parser (fallback path)
    fn parse_w3d_data_legacy(
        &self,
        data: &[u8],
        model_name: String,
        allow_animation_only: bool,
    ) -> Result<W3DModel> {
        if data.len() < 8 {
            return Err(anyhow!("W3D file too small: {} bytes", data.len()));
        }

        let mut model = W3DModel::new(model_name);
        let mut offset = 0usize;

        // Parse W3D chunks with safety counter to prevent infinite loops
        let mut chunk_counter = 0;
        const MAX_CHUNKS: usize = 10000; // Safety limit to prevent infinite loops

        while offset + 8 <= data.len() && chunk_counter < MAX_CHUNKS {
            chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // Handle W3D chunk size format: MSB indicates container chunk
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize; // Clear MSB to get actual size

            debug!(
                "W3D chunk: type=0x{:08X}, raw_size=0x{:08X}, size={}, container={}",
                chunk_type, raw_chunk_size, chunk_size, is_container_chunk
            );

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "W3D chunk extends beyond file: type 0x{:08X}, size {} (raw: 0x{:08X})",
                    chunk_type, chunk_size, raw_chunk_size
                );
                break;
            }

            // Additional safety checks to prevent infinite loops
            if chunk_size == 0 {
                warn!(
                    "Zero-sized chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8; // Skip just the header
                continue;
            }

            if chunk_size > data.len() {
                warn!(
                    "Chunk size {} exceeds total file size {} - aborting parsing",
                    chunk_size,
                    data.len()
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH => {
                    debug!("Parsing W3D mesh chunk, size: {}", chunk_size);
                    if let Ok(mut mesh) = self.parse_mesh_chunk(chunk_data) {
                        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
                            mesh.texture_library = model.texture_names.clone();
                        }
                        model.meshes.push(mesh);
                    } else {
                        warn!("Failed to parse W3D mesh chunk");
                    }
                }
                W3D_CHUNK_HIERARCHY => {
                    debug!("Parsing W3D hierarchy chunk, size: {}", chunk_size);
                    match self.parse_hierarchy_chunk(chunk_data) {
                        Ok(hierarchy) => {
                            debug!(
                                "Parsed hierarchy '{}' with {} pivots",
                                hierarchy.name,
                                hierarchy.pivots.len()
                            );
                            model.retain_source_hierarchy(hierarchy);
                        }
                        Err(e) => warn!("Failed to parse hierarchy chunk: {}", e),
                    }
                    if is_container_chunk {
                        self.parse_container_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_MATERIALS3 => {
                    debug!("Parsing W3D materials3 container, size: {}", chunk_size);
                    // Parse materials3 container - contains material definitions with texture names
                    if is_container_chunk {
                        self.parse_materials3_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_TEXTURES => {
                    debug!("Parsing W3D textures container, size: {}", chunk_size);
                    // Parse textures container - contains individual texture definitions
                    if is_container_chunk {
                        self.parse_textures_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_ANIMATION => {
                    debug!("Parsing W3D animation chunk, size: {}", chunk_size);
                    match self.parse_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            debug!(
                                "Parsed animation '{}' ({} frames @ {}fps)",
                                animation.name, animation.num_frames, animation.frame_rate
                            );
                            model.animations.push(animation);
                        }
                        Err(e) => warn!("Failed to parse animation chunk: {}", e),
                    }
                }
                W3D_CHUNK_COMPRESSED_ANIMATION => {
                    debug!(
                        "Parsing W3D compressed animation chunk (timecoded/adaptive delta), size: {}",
                        chunk_size
                    );
                    match self.parse_compressed_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            debug!(
                                "Parsed compressed animation '{}' ({} frames @ {}fps)",
                                animation.name, animation.num_frames, animation.frame_rate
                            );
                            model.animations.push(animation);
                        }
                        Err(e) => warn!("Failed to parse compressed animation chunk: {}", e),
                    }
                }
                W3D_CHUNK_HMODEL => {
                    debug!("Found W3D hierarchical model chunk, size: {}", chunk_size);
                    if !is_container_chunk {
                        warn!("HMODEL chunk is not a container; skipping unsafe prototype");
                    } else {
                        match self.parse_hmodel_chunk(chunk_data) {
                            Ok(hmodel) => model.hmodels.push(hmodel),
                            Err(error) => {
                                warn!("Failed to parse HMODEL definition: {}", error);
                            }
                        }
                    }
                }
                W3D_CHUNK_LODMODEL => {
                    debug!("Found W3D LOD model chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!("Failed to parse LOD model container: {}", e);
                        }
                    }
                }
                W3D_CHUNK_HLOD => {
                    debug!(
                        "Found W3D HLOD (Hierarchical LOD) chunk, size: {}",
                        chunk_size
                    );
                    if !is_container_chunk {
                        model.hlod_parse_failed = true;
                        warn!("HLOD chunk is not marked as a container; suppressing unsafe mesh fallback");
                    } else {
                        match self.parse_hlod_chunk(chunk_data) {
                            Ok(hlod) => model.hlods.push(hlod),
                            Err(e) => {
                                model.hlod_parse_failed = true;
                                warn!("Failed to parse HLOD container: {}", e);
                            }
                        }
                    }
                }
                _ => {
                    debug!("Unknown W3D chunk type: 0x{:08X}", chunk_type);
                    // If it's a container chunk, try to parse it recursively
                    if is_container_chunk && chunk_size > 0 {
                        debug!("  -> Container chunk, parsing recursively");
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!(
                                "Failed to parse container chunk 0x{:08X}: {}",
                                chunk_type, e
                            );
                        }
                    }
                }
            }

            offset += 8 + chunk_size;
        }

        if chunk_counter >= MAX_CHUNKS {
            warn!(
                "⚠️  W3D chunk parsing hit safety limit ({} chunks) - possible malformed file",
                MAX_CHUNKS
            );
        }

        if model.meshes.is_empty() && !allow_animation_only {
            return Err(anyhow!(
                "legacy parser: no valid meshes found in '{}'",
                model.name
            ));
        }

        // Post-process: Resolve texture indices to actual texture names from W3D_CHUNK_TEXTURES
        // This matches C++ behavior where W3D_CHUNK_MAP3_FILENAME contains texture indices
        // that need to be resolved against the texture_names array
        self.resolve_texture_indices(&mut model);

        // HMODEL has separate source binding metadata that is not part of this HLOD
        // correction.  Preserve its existing narrow pivot-name residual, but apply
        // the resulting matrix only at render time.  In particular, never bake it
        // into already converted vertices and then retain it for a second render
        // transform.  HLOD uses its own exact `HLOD.Name.MeshName -> BoneIndex`
        // records below and must never take this generic name path.
        if model.hlods.is_empty() && !model.hlod_parse_failed {
            if let Some(ref hierarchy) = model.hierarchy {
                if !hierarchy.pivots.is_empty() {
                    if let Some(globals) = Self::compute_global_transforms(hierarchy) {
                        for mesh in &mut model.meshes {
                            if mesh.transform != Mat4::IDENTITY {
                                continue;
                            }
                            if let Some(pivot_idx) =
                                hierarchy.pivots.iter().position(|p| p.name == mesh.name)
                            {
                                let global = &globals[pivot_idx];
                                mesh.transform = W3DModel::w3d_transform_to_render_basis(
                                    Mat4::from_cols_array(global),
                                );
                            }
                        }
                    }
                }
            }
        }

        model.calculate_bounding_box();
        Ok(model)
    }

    /// Parse a W3D container chunk recursively
    fn parse_container_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut chunk_counter = 0;
        const MAX_CONTAINER_CHUNKS: usize = 5000; // Safety limit for container chunks

        while offset + 8 <= data.len() && chunk_counter < MAX_CONTAINER_CHUNKS {
            chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Container sub-chunk extends beyond container: type 0x{:08X}, size {}",
                    chunk_type, chunk_size
                );
                break;
            }

            // Safety checks for container chunks
            if chunk_size == 0 {
                warn!(
                    "Zero-sized container chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8;
                continue;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH => {
                    debug!("Found mesh chunk in container, size: {}", chunk_size);
                    if let Ok(mut mesh) = self.parse_mesh_chunk(chunk_data) {
                        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
                            mesh.texture_library = model.texture_names.clone();
                        }
                        model.meshes.push(mesh);
                    } else {
                        warn!("Failed to parse mesh chunk in container");
                    }
                }
                W3D_CHUNK_TEXTURES => {
                    debug!("Found textures chunk in container, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_textures_chunk(chunk_data, model) {
                            warn!("Failed to parse textures chunk: {}", e);
                        }
                    }
                }
                W3D_CHUNK_ANIMATION => {
                    debug!("Found animation chunk in container, size: {}", chunk_size);
                    match self.parse_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            model.animations.push(animation);
                        }
                        Err(e) => warn!("Failed to parse animation chunk in container: {}", e),
                    }
                }
                W3D_CHUNK_COMPRESSED_ANIMATION => {
                    debug!(
                        "Found compressed animation chunk in container, size: {}",
                        chunk_size
                    );
                    match self.parse_compressed_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            model.animations.push(animation);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse compressed animation chunk in container: {}",
                                e
                            )
                        }
                    }
                }
                W3D_CHUNK_HIERARCHY => {
                    debug!("Found hierarchy chunk in container, size: {}", chunk_size);
                    match self.parse_hierarchy_chunk(chunk_data) {
                        Ok(hierarchy) => {
                            model.retain_source_hierarchy(hierarchy);
                        }
                        Err(e) => {
                            warn!("Failed to parse hierarchy chunk in container: {}", e)
                        }
                    }
                }
                W3D_CHUNK_HMODEL => {
                    if !is_container_chunk {
                        warn!("Nested HMODEL chunk is not a container; skipping unsafe prototype");
                    } else {
                        match self.parse_hmodel_chunk(chunk_data) {
                            Ok(hmodel) => model.hmodels.push(hmodel),
                            Err(error) => {
                                warn!("Failed to parse nested HMODEL definition: {}", error)
                            }
                        }
                    }
                }
                _ => {
                    debug!(
                        "Container sub-chunk: type 0x{:08X}, size {}, container: {}",
                        chunk_type, chunk_size, is_container_chunk
                    );
                    // Recursively parse nested containers
                    if is_container_chunk && chunk_size > 0 {
                        if let Err(e) = self.parse_container_chunk(chunk_data, model) {
                            warn!(
                                "Failed to parse nested container 0x{:08X}: {}",
                                chunk_type, e
                            );
                        }
                    }
                }
            }

            offset += 8 + chunk_size;
        }

        if chunk_counter >= MAX_CONTAINER_CHUNKS {
            warn!(
                "⚠️  Container chunk parsing hit safety limit ({} chunks)",
                MAX_CONTAINER_CHUNKS
            );
        }

        Ok(())
    }

    /// Parse one C++ `HModelDefClass` source record.
    ///
    /// `HModelDefClass::Load_W3D` requires a header first, allocates exactly
    /// `NumConnections` node records, and reads NODE, COLLISION_NODE, and
    /// SKIN_NODE in source order. Under the original 32-bit MSVC ABI,
    /// `sizeof(W3dHModelHeaderStruct)` is 40 (four-byte tail alignment) while
    /// `sizeof(W3dHModelNodeStruct)` is exactly 18 (its largest member has
    /// two-byte alignment). Preserve that asymmetrical wire shape instead of
    /// accepting the unrelated u32 helper structs from other compatibility
    /// crates.
    fn parse_hmodel_chunk(&self, data: &[u8]) -> Result<W3dHmodel> {
        const HMODEL_HEADER_SIZE: usize = 40;
        const HMODEL_NODE_SIZE: usize = 18;
        const MAX_HMODEL_CONNECTIONS: usize = 4096;

        let mut offset = 0usize;
        let header = next_w3d_chunk(data, &mut offset, "HMODEL header")?
            .ok_or_else(|| anyhow!("HMODEL has no header"))?;
        if header.chunk_type != W3D_CHUNK_HMODEL_HEADER || header.is_container {
            return Err(anyhow!(
                "HMODEL expected raw header 0x{W3D_CHUNK_HMODEL_HEADER:08X}, got 0x{:08X}, container={}",
                header.chunk_type,
                header.is_container
            ));
        }
        if header.data.len() != HMODEL_HEADER_SIZE {
            return Err(anyhow!(
                "HMODEL header has wrong size: {} != {}",
                header.data.len(),
                HMODEL_HEADER_SIZE
            ));
        }

        let version = u32::from_le_bytes(header.data[0..4].try_into().unwrap());
        let name = hmodel_cxx_header_name(&header.data[4..4 + W3D_NAME_LEN]);
        let hierarchy_name = hmodel_cxx_header_name(&header.data[4 + W3D_NAME_LEN..36]);
        let declared_connection_count =
            usize::from(u16::from_le_bytes(header.data[36..38].try_into().unwrap()));
        if declared_connection_count > MAX_HMODEL_CONNECTIONS {
            return Err(anyhow!(
                "HMODEL '{}' declares {} connections, exceeding safe limit {}",
                name,
                declared_connection_count,
                MAX_HMODEL_CONNECTIONS
            ));
        }

        let mut nodes = Vec::with_capacity(declared_connection_count);
        let mut source_snap_points = Vec::new();
        let mut has_invalid_records = name.is_empty()
            || name.as_bytes().contains(&0)
            || hierarchy_name.as_bytes().contains(&0);

        while let Some(record) = next_w3d_chunk(data, &mut offset, "HMODEL connection")? {
            if record.chunk_type == W3D_CHUNK_POINTS {
                // `SnapPointsClass::Load_W3D` uses `Cur_Chunk_Length() / 12`
                // and reads that many raw W3dVectorStruct values. It does
                // not consume a declared HMODEL connection, rejects no
                // trailing byte remainder, and a later POINTS record replaces
                // the definition-owned pointer. Keep all three properties.
                source_snap_points = Self::parse_hmodel_source_snap_points(record.data);
                continue;
            }
            let kind = match record.chunk_type {
                W3D_CHUNK_HMODEL_NODE => W3dHmodelNodeKind::Node,
                W3D_CHUNK_HMODEL_COLLISION_NODE => W3dHmodelNodeKind::CollisionNode,
                W3D_CHUNK_HMODEL_SKIN_NODE => W3dHmodelNodeKind::SkinNode,
                // Obsolete extension chunks do not define a child render
                // object and must never become aliases.
                _ => continue,
            };
            if record.is_container || record.data.len() != HMODEL_NODE_SIZE {
                has_invalid_records = true;
                continue;
            }
            if nodes.len() >= declared_connection_count {
                has_invalid_records = true;
                continue;
            }

            let Some(render_object_name_end) = record.data[..W3D_NAME_LEN]
                .iter()
                .position(|byte| *byte == 0)
            else {
                // `read_connection` uses `strcat` on this source field. A
                // non-terminated sixteen-byte leaf is undefined behavior in
                // C++; safe Main must not manufacture a wider alias from it.
                has_invalid_records = true;
                continue;
            };
            let render_object_name = w3d_string_from_bytes(&record.data[..render_object_name_end]);
            let bone_index = u32::from(u16::from_le_bytes(
                record.data[W3D_NAME_LEN..W3D_NAME_LEN + 2]
                    .try_into()
                    .unwrap(),
            ));
            if render_object_name.is_empty() || render_object_name.as_bytes().contains(&0) {
                has_invalid_records = true;
                continue;
            }

            // C++ `read_connection` always constructs the complete identity
            // from the HMODEL header name and the node leaf, even for a
            // collision or skin connection.
            nodes.push(W3dHmodelNode {
                name: format!("{}.{}", name, render_object_name),
                bone_index: if version < W3D_HTREE_ROOT_VERSION {
                    // Pre-3.0 source uses 0xffff for the newly synthesized
                    // external root; every other source pivot shifts by one.
                    if bone_index == u32::from(u16::MAX) {
                        0
                    } else {
                        bone_index.checked_add(1).ok_or_else(|| {
                            anyhow!("HMODEL '{}' pre-3.0 pivot index overflow", name)
                        })?
                    }
                } else {
                    if bone_index == u32::from(u16::MAX) {
                        has_invalid_records = true;
                    }
                    bone_index
                },
                kind,
            });
        }

        if nodes.len() != declared_connection_count {
            has_invalid_records = true;
        }

        Ok(W3dHmodel {
            version,
            name,
            hierarchy_name,
            nodes,
            source_snap_points,
            has_invalid_records,
        })
    }

    /// Read the source payload exactly as C++ `SnapPointsClass::Load_W3D`.
    /// A non-vector remainder is skipped by the C++ integer division, and
    /// points remain in source W3D coordinates rather than the active render
    /// basis.
    fn parse_hmodel_source_snap_points(data: &[u8]) -> Vec<W3dHmodelSnapPoint> {
        const W3D_VECTOR_SIZE: usize = 12;

        data.chunks_exact(W3D_VECTOR_SIZE)
            .map(|record| W3dHmodelSnapPoint {
                source_position: [
                    f32::from_le_bytes(record[0..4].try_into().unwrap()),
                    f32::from_le_bytes(record[4..8].try_into().unwrap()),
                    f32::from_le_bytes(record[8..12].try_into().unwrap()),
                ],
            })
            .collect()
    }

    /// Parse one C++ `W3dHLodArrayHeaderStruct` plus its exact ordered
    /// `W3dHLodSubObjectStruct` records. The same wire shape is used for an
    /// ordinary LOD, aggregate render objects, and application proxies.
    fn parse_hlod_subobject_array(
        &self,
        data: &[u8],
        hlod_name: &str,
        array_label: &str,
    ) -> Result<W3dHlodAttachmentArray> {
        const HLOD_ARRAY_HEADER_SIZE: usize = 8;
        const HLOD_SUBOBJECT_SIZE: usize = 36;
        const MAX_HLOD_SUBOBJECTS: usize = 4096;

        let mut offset = 0usize;
        let array_header = next_w3d_chunk(data, &mut offset, "HLOD subobject array header")?
            .ok_or_else(|| {
                anyhow!(
                    "HLOD '{}' {} has no subobject array header",
                    hlod_name,
                    array_label
                )
            })?;
        if array_header.chunk_type != W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER
            || array_header.is_container
        {
            return Err(anyhow!(
                "HLOD '{}' {} expected raw subobject array header 0x{W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER:08X}, got 0x{:08X}, container={}",
                hlod_name,
                array_label,
                array_header.chunk_type,
                array_header.is_container
            ));
        }
        if array_header.data.len() != HLOD_ARRAY_HEADER_SIZE {
            return Err(anyhow!(
                "HLOD '{}' {} array header has wrong size: {} != {}",
                hlod_name,
                array_label,
                array_header.data.len(),
                HLOD_ARRAY_HEADER_SIZE
            ));
        }

        let model_count = u32::from_le_bytes(array_header.data[0..4].try_into().unwrap()) as usize;
        if model_count > MAX_HLOD_SUBOBJECTS {
            return Err(anyhow!(
                "HLOD '{}' {} declares {} subobjects, exceeding safe limit {}",
                hlod_name,
                array_label,
                model_count,
                MAX_HLOD_SUBOBJECTS
            ));
        }
        let max_screen_size = f32::from_le_bytes(array_header.data[4..8].try_into().unwrap());

        let mut subobjects = Vec::with_capacity(model_count);
        for subobject_index in 0..model_count {
            let subobject =
                next_w3d_chunk(data, &mut offset, "HLOD subobject")?.ok_or_else(|| {
                    anyhow!(
                        "HLOD '{}' {} is missing subobject {}",
                        hlod_name,
                        array_label,
                        subobject_index
                    )
                })?;
            if subobject.chunk_type != W3D_CHUNK_HLOD_SUB_OBJECT || subobject.is_container {
                return Err(anyhow!(
                    "HLOD '{}' {} expected raw subobject 0x{W3D_CHUNK_HLOD_SUB_OBJECT:08X}, got 0x{:08X}, container={}",
                    hlod_name,
                    array_label,
                    subobject.chunk_type,
                    subobject.is_container
                ));
            }
            if subobject.data.len() != HLOD_SUBOBJECT_SIZE {
                return Err(anyhow!(
                    "HLOD '{}' {} subobject {} has wrong size: {} != {}",
                    hlod_name,
                    array_label,
                    subobject_index,
                    subobject.data.len(),
                    HLOD_SUBOBJECT_SIZE
                ));
            }
            subobjects.push(W3dHlodSubObject {
                bone_index: u32::from_le_bytes(subobject.data[0..4].try_into().unwrap()),
                name: w3d_string_from_bytes(&subobject.data[4..HLOD_SUBOBJECT_SIZE]),
            });
        }
        if next_w3d_chunk(data, &mut offset, "HLOD subobject array trailing data")?.is_some() {
            return Err(anyhow!(
                "HLOD '{}' {} has trailing chunks after its declared subobjects",
                hlod_name,
                array_label
            ));
        }

        Ok(W3dHlodAttachmentArray {
            max_screen_size,
            subobjects,
        })
    }

    /// Parse the exact rigid-child metadata from a `W3D_CHUNK_HLOD` container.
    ///
    /// C++ `HLodDefClass::Load_W3D` reads the HLOD header, then exactly
    /// `LodCount` `W3D_CHUNK_HLOD_LOD_ARRAY` records.  Each array contains an
    /// array header followed by exact `W3dHLodSubObjectStruct` records.  Keep that
    /// structure instead of recursively flattening it into anonymous meshes.
    fn parse_hlod_chunk(&self, data: &[u8]) -> Result<W3dHlod> {
        const HLOD_HEADER_SIZE: usize = 40;
        const MAX_HLOD_LODS: usize = 64;

        let mut offset = 0usize;
        let header = next_w3d_chunk(data, &mut offset, "HLOD header")?
            .ok_or_else(|| anyhow!("HLOD has no header"))?;
        if header.chunk_type != W3D_CHUNK_HLOD_HEADER {
            return Err(anyhow!(
                "HLOD expected header 0x{W3D_CHUNK_HLOD_HEADER:08X}, got 0x{:08X}",
                header.chunk_type
            ));
        }
        if header.data.len() < HLOD_HEADER_SIZE {
            return Err(anyhow!(
                "HLOD header too small: {} < {}",
                header.data.len(),
                HLOD_HEADER_SIZE
            ));
        }

        let version = u32::from_le_bytes(header.data[0..4].try_into().unwrap());
        let lod_count = u32::from_le_bytes(header.data[4..8].try_into().unwrap()) as usize;
        if lod_count > MAX_HLOD_LODS {
            return Err(anyhow!(
                "HLOD declares {} LODs, exceeding safe limit {}",
                lod_count,
                MAX_HLOD_LODS
            ));
        }
        let name = w3d_string_from_bytes(&header.data[8..8 + W3D_NAME_LEN]);
        let hierarchy_name =
            w3d_string_from_bytes(&header.data[8 + W3D_NAME_LEN..HLOD_HEADER_SIZE]);

        let mut lods = Vec::with_capacity(lod_count);
        for lod_index in 0..lod_count {
            let lod_chunk = next_w3d_chunk(data, &mut offset, "HLOD LOD array")?
                .ok_or_else(|| anyhow!("HLOD '{}' is missing LOD {}", name, lod_index))?;
            if lod_chunk.chunk_type != W3D_CHUNK_HLOD_LOD_ARRAY || !lod_chunk.is_container {
                return Err(anyhow!(
                    "HLOD '{}' expected container LOD array 0x{W3D_CHUNK_HLOD_LOD_ARRAY:08X}, got type 0x{:08X}, container={}",
                    name,
                    lod_chunk.chunk_type,
                    lod_chunk.is_container
                ));
            }

            let array = self.parse_hlod_subobject_array(
                lod_chunk.data,
                &name,
                &format!("LOD {lod_index}"),
            )?;

            lods.push(W3dHlodLod {
                max_screen_size: array.max_screen_size,
                subobjects: array.subobjects,
            });
        }

        // Retain the two C++ trailing arrays even before Main can recursively
        // render aggregates. Only aggregates affect model output; proxies are
        // intentionally non-rendering application metadata in C++.
        let mut aggregates = None;
        let mut proxies = None;
        let mut has_invalid_trailing_records = false;
        while let Some(remaining) = next_w3d_chunk(data, &mut offset, "HLOD trailing record")? {
            match remaining.chunk_type {
                W3D_CHUNK_HLOD_AGGREGATE_ARRAY => {
                    if !remaining.is_container || aggregates.is_some() {
                        has_invalid_trailing_records = true;
                        warn!(
                            "HLOD '{}' has a non-container or repeated aggregate array; suppressing ambiguous render",
                            name
                        );
                        continue;
                    }
                    match self.parse_hlod_subobject_array(remaining.data, &name, "aggregate array")
                    {
                        Ok(array) => aggregates = Some(array),
                        Err(error) => {
                            has_invalid_trailing_records = true;
                            warn!(
                                "HLOD '{}' has malformed aggregate data; suppressing ambiguous render: {}",
                                name, error
                            );
                        }
                    }
                }
                W3D_CHUNK_HLOD_PROXY_ARRAY => {
                    if !remaining.is_container || proxies.is_some() {
                        has_invalid_trailing_records = true;
                        warn!(
                            "HLOD '{}' has a non-container or repeated proxy array; suppressing ambiguous render",
                            name
                        );
                        continue;
                    }
                    match self.parse_hlod_subobject_array(remaining.data, &name, "proxy array") {
                        Ok(array) => proxies = Some(array),
                        Err(error) => {
                            has_invalid_trailing_records = true;
                            warn!(
                                "HLOD '{}' has malformed proxy data; suppressing ambiguous render: {}",
                                name, error
                            );
                        }
                    }
                }
                unknown => {
                    has_invalid_trailing_records = true;
                    warn!(
                        "HLOD '{}' has unsupported trailing chunk 0x{:08X}; suppressing ambiguous render",
                        name, unknown
                    );
                }
            }
        }

        let has_unrendered_aggregates = aggregates
            .as_ref()
            .is_some_and(|array| !array.subobjects.is_empty());

        Ok(W3dHlod {
            version,
            name,
            hierarchy_name,
            lods,
            aggregates,
            proxies,
            has_unrendered_aggregates,
            has_invalid_trailing_records,
        })
    }

    /// Parse W3D hierarchy chunk (0x100) — ported from standalone parser's parse_hierarchy_chunk
    fn parse_hierarchy_chunk(&self, data: &[u8]) -> Result<W3dHierarchy> {
        let mut name = String::new();
        let mut hierarchy_version = None;
        let mut num_pivots: u32 = 0;
        let mut pivots: Vec<W3dPivot> = Vec::new();
        let mut pivot_fixups: Vec<[[f32; 3]; 4]> = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_HIERARCHY_HEADER => {
                    // W3dHierarchyHeader: version(u32) + name[16] + num_pivots(u32) + center[3]
                    if chunk_data.len() < 32 {
                        return Err(anyhow!("hierarchy header too small: {}", chunk_data.len()));
                    }
                    let version = u32::from_le_bytes([
                        chunk_data[0],
                        chunk_data[1],
                        chunk_data[2],
                        chunk_data[3],
                    ]);
                    hierarchy_version = Some(version);
                    let mut n = String::new();
                    for i in 4..4 + W3D_NAME_LEN {
                        if i >= chunk_data.len() || chunk_data[i] == 0 {
                            break;
                        }
                        n.push(chunk_data[i] as char);
                    }
                    name = n;
                    num_pivots = u32::from_le_bytes([
                        chunk_data[20],
                        chunk_data[21],
                        chunk_data[22],
                        chunk_data[23],
                    ]);
                    debug!(
                        "Hierarchy header: name='{}', version=0x{:08X}, num_pivots={}",
                        name, version, num_pivots
                    );
                }
                W3D_CHUNK_PIVOTS => {
                    // Each pivot: name[16] + parent_idx(u32) + translation[3] + euler[3] + quat[4]
                    // = 16 + 4 + 12 + 12 + 16 = 60 bytes
                    const PIVOT_SIZE: usize = 60;
                    let count = if num_pivots > 0 {
                        num_pivots as usize
                    } else {
                        chunk_size / PIVOT_SIZE
                    };
                    pivots.reserve(count);
                    for i in 0..count {
                        let base = i * PIVOT_SIZE;
                        if base + PIVOT_SIZE > chunk_data.len() {
                            break;
                        }
                        let d = &chunk_data[base..base + PIVOT_SIZE];
                        let mut pname = String::new();
                        for j in 0..W3D_NAME_LEN {
                            if d[j] == 0 {
                                break;
                            }
                            pname.push(d[j] as char);
                        }
                        let parent_idx = u32::from_le_bytes([d[16], d[17], d[18], d[19]]);
                        let tx = f32::from_le_bytes([d[20], d[21], d[22], d[23]]);
                        let ty = f32::from_le_bytes([d[24], d[25], d[26], d[27]]);
                        let tz = f32::from_le_bytes([d[28], d[29], d[30], d[31]]);
                        let ex = f32::from_le_bytes([d[32], d[33], d[34], d[35]]);
                        let ey = f32::from_le_bytes([d[36], d[37], d[38], d[39]]);
                        let ez = f32::from_le_bytes([d[40], d[41], d[42], d[43]]);
                        let qx = f32::from_le_bytes([d[44], d[45], d[46], d[47]]);
                        let qy = f32::from_le_bytes([d[48], d[49], d[50], d[51]]);
                        let qz = f32::from_le_bytes([d[52], d[53], d[54], d[55]]);
                        let qw = f32::from_le_bytes([d[56], d[57], d[58], d[59]]);
                        pivots.push(W3dPivot {
                            name: pname,
                            parent_idx,
                            translation: [tx, ty, tz],
                            euler_angles: [ex, ey, ez],
                            rotation: [qx, qy, qz, qw],
                        });
                    }
                }
                W3D_CHUNK_PIVOT_FIXUPS => {
                    // Each fixup: [[f32;3];4] = 48 bytes
                    const FIXUP_SIZE: usize = 48;
                    let count = if num_pivots > 0 {
                        num_pivots as usize
                    } else {
                        chunk_size / FIXUP_SIZE
                    };
                    pivot_fixups.reserve(count);
                    for i in 0..count {
                        let base = i * FIXUP_SIZE;
                        if base + FIXUP_SIZE > chunk_data.len() {
                            break;
                        }
                        let d = &chunk_data[base..base + FIXUP_SIZE];
                        let mut tm = [[0.0f32; 3]; 4];
                        for row in 0..4 {
                            for col in 0..3 {
                                let off = (row * 3 + col) * 4;
                                tm[row][col] = f32::from_le_bytes([
                                    d[off],
                                    d[off + 1],
                                    d[off + 2],
                                    d[off + 3],
                                ]);
                            }
                        }
                        pivot_fixups.push(tm);
                    }
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        if hierarchy_version.is_some_and(|version| version < W3D_HTREE_ROOT_VERSION) {
            // C++ `HTreeClass::read_pivots(pre30)` injects this exact node,
            // then increments every source parent index (including
            // `0xFFFF_FFFF`, which wraps to the synthetic root).  Preserve
            // that normalized index space so the shared runtime evaluator can
            // always reserve pivot zero for the external object transform.
            for pivot in &mut pivots {
                pivot.parent_idx = pivot.parent_idx.wrapping_add(1);
            }
            pivots.insert(
                0,
                W3dPivot {
                    name: "RootTransform".to_string(),
                    parent_idx: u32::MAX,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
            );
            if !pivot_fixups.is_empty() {
                // The Main loader retains fixups as source metadata. Keep the
                // same pivot indexing even though no active runtime consumer
                // currently evaluates them.
                pivot_fixups.insert(
                    0,
                    [
                        [1.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0],
                        [0.0, 0.0, 1.0],
                        [0.0, 0.0, 0.0],
                    ],
                );
            }
        }

        Ok(W3dHierarchy {
            name,
            pivots,
            pivot_fixups,
        })
    }

    /// Parse W3D animation chunk (0x200) — ported from standalone parser's parse_animation_chunk
    fn parse_animation_chunk(&self, data: &[u8]) -> Result<W3dAnimation> {
        let mut name = String::new();
        let mut hierarchy_name = String::new();
        let mut animation_version = None;
        let mut num_frames: u32 = 0;
        let mut frame_rate: u32 = 0;
        let mut channels: Vec<W3dAnimChannel> = Vec::new();
        let mut raw_visibility_channels: Vec<W3dRawVisibilityChannel> = Vec::new();
        let mut unsupported_visibility_pivots: Vec<Option<u16>> = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_ANIMATION_HEADER => {
                    // version(u32) + name[16] + hierarchy_name[16] + num_frames(u32) + frame_rate(u32)
                    // = 4 + 16 + 16 + 4 + 4 = 44 bytes
                    if chunk_data.len() < 44 {
                        return Err(anyhow!("animation header too small: {}", chunk_data.len()));
                    }
                    let version = u32::from_le_bytes([
                        chunk_data[0],
                        chunk_data[1],
                        chunk_data[2],
                        chunk_data[3],
                    ]);
                    animation_version = Some(version);
                    let mut n = String::new();
                    for i in 4..4 + W3D_NAME_LEN {
                        if chunk_data[i] == 0 {
                            break;
                        }
                        n.push(chunk_data[i] as char);
                    }
                    name = n;
                    let mut hn = String::new();
                    for i in 20..20 + W3D_NAME_LEN {
                        if i >= chunk_data.len() || chunk_data[i] == 0 {
                            break;
                        }
                        hn.push(chunk_data[i] as char);
                    }
                    hierarchy_name = hn;
                    num_frames = u32::from_le_bytes([
                        chunk_data[36],
                        chunk_data[37],
                        chunk_data[38],
                        chunk_data[39],
                    ]);
                    frame_rate = u32::from_le_bytes([
                        chunk_data[40],
                        chunk_data[41],
                        chunk_data[42],
                        chunk_data[43],
                    ]);
                    debug!(
                        "Animation header: name='{}', hierarchy='{}', version=0x{:08X}, frames={}, fps={}",
                        name, hierarchy_name, version, num_frames, frame_rate
                    );
                }
                W3D_CHUNK_ANIMATION_CHANNEL => {
                    // first_frame(u16) + last_frame(u16) + vector_len(u16) + flags(u16) + pivot(u16) + pad(u16)
                    // = 12 bytes header, rest is f32 data
                    if chunk_data.len() < 12 {
                        offset += 8 + chunk_size;
                        continue;
                    }
                    let first_frame = u16::from_le_bytes([chunk_data[0], chunk_data[1]]);
                    let last_frame = u16::from_le_bytes([chunk_data[2], chunk_data[3]]);
                    let vector_len = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
                    let flags = u16::from_le_bytes([chunk_data[6], chunk_data[7]]);
                    let pivot = u16::from_le_bytes([chunk_data[8], chunk_data[9]]);
                    let remaining = chunk_size.saturating_sub(12);
                    let count_f32 = remaining / 4;
                    let mut chan_data = Vec::with_capacity(count_f32);
                    for i in 0..count_f32 {
                        let off = 12 + i * 4;
                        if off + 4 > chunk_data.len() {
                            break;
                        }
                        chan_data.push(f32::from_le_bytes([
                            chunk_data[off],
                            chunk_data[off + 1],
                            chunk_data[off + 2],
                            chunk_data[off + 3],
                        ]));
                    }
                    channels.push(W3dAnimChannel {
                        first_frame,
                        last_frame,
                        vector_len,
                        flags,
                        pivot,
                        data: chan_data,
                    });
                }
                W3D_CHUNK_BIT_CHANNEL => {
                    // W3dBitChannelStruct has nine fixed bytes before the
                    // packed bit stream: first/last/flags/pivot/default. C++
                    // accepts this channel only for BIT_CHANNEL_VIS (0).
                    if chunk_data.len() < 9 {
                        unsupported_visibility_pivots.push(None);
                        debug!(
                            "Malformed raw W3D bit channel has no recoverable pivot, size: {}",
                            chunk_size
                        );
                        offset += 8 + chunk_size;
                        continue;
                    }
                    let first_frame = u16::from_le_bytes([chunk_data[0], chunk_data[1]]);
                    let last_frame = u16::from_le_bytes([chunk_data[2], chunk_data[3]]);
                    let flags = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
                    let pivot = u16::from_le_bytes([chunk_data[6], chunk_data[7]]);
                    if flags != 0 {
                        // `HRawAnimClass::add_bit_channel` ignores types other
                        // than BIT_CHANNEL_VIS, so they are not an HLOD
                        // visibility fallback to guess at.
                        debug!(
                            "Ignoring non-visibility raw W3D bit channel type {} for pivot {}",
                            flags, pivot
                        );
                    } else if last_frame < first_frame {
                        unsupported_visibility_pivots.push(Some(pivot));
                        debug!(
                            "Malformed raw W3D visibility channel has inverted range {}..{} for pivot {}",
                            first_frame, last_frame, pivot
                        );
                    } else {
                        let bit_count = usize::from(last_frame - first_frame) + 1;
                        let byte_count = (bit_count + 7) / 8;
                        let expected_len = 9 + byte_count;
                        if chunk_data.len() != expected_len {
                            unsupported_visibility_pivots.push(Some(pivot));
                            debug!(
                                "Malformed raw W3D visibility channel for pivot {}: expected {} bytes, got {}",
                                pivot,
                                expected_len,
                                chunk_data.len()
                            );
                        } else {
                            raw_visibility_channels.push(W3dRawVisibilityChannel {
                                first_frame,
                                last_frame,
                                flags,
                                pivot,
                                default_visible: chunk_data[8] != 0,
                                bits: chunk_data[9..].to_vec(),
                            });
                        }
                    }
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        if animation_version.is_some_and(|version| version < W3D_HTREE_ROOT_VERSION) {
            // C++ `HRawAnimClass::{read_channel,read_bit_channel}` shifts
            // every pre-3.0 source pivot after `HTreeClass` inserts its
            // synthetic root.  `u16` wrapping matches the raw C++ field
            // operation; a resulting out-of-range channel is safely ignored
            // later by the same bounded pose sampler.
            for channel in &mut channels {
                channel.pivot = channel.pivot.wrapping_add(1);
            }
            for channel in &mut raw_visibility_channels {
                channel.pivot = channel.pivot.wrapping_add(1);
            }
            for pivot in &mut unsupported_visibility_pivots {
                if let Some(pivot) = pivot {
                    *pivot = pivot.wrapping_add(1);
                }
            }
        }

        Ok(W3dAnimation {
            name,
            hierarchy_name,
            num_frames,
            frame_rate,
            source_is_compressed: false,
            channels,
            raw_visibility_channels,
            unsupported_visibility_pivots,
        })
    }

    /// Parse W3D compressed animation chunk (0x280) — handles both timecoded (flavor 0)
    /// and adaptive delta (flavor 1) sub-formats.
    /// C++ parity: W3D_CHUNK_COMPRESSED_ANIMATION container with CompressedAnimationHeader,
    /// CompressedAnimationChannel, and CompressedBitChannel sub-chunks.
    fn parse_compressed_animation_chunk(&self, data: &[u8]) -> Result<W3dAnimation> {
        let mut name = String::new();
        let mut hierarchy_name = String::new();
        let mut num_frames: u32 = 0;
        let mut frame_rate: u32 = 0;
        let mut flavor: u16 = ANIM_FLAVOR_TIMECODED;
        let mut channels: Vec<W3dAnimChannel> = Vec::new();
        let raw_visibility_channels: Vec<W3dRawVisibilityChannel> = Vec::new();
        let mut unsupported_visibility_pivots: Vec<Option<u16>> = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_COMPRESSED_ANIMATION_HEADER => {
                    if chunk_data.len() < 44 {
                        return Err(anyhow!(
                            "compressed animation header too small: {}",
                            chunk_data.len()
                        ));
                    }
                    let mut n = String::new();
                    for i in 4..4 + W3D_NAME_LEN {
                        if chunk_data[i] == 0 {
                            break;
                        }
                        n.push(chunk_data[i] as char);
                    }
                    name = n;
                    let mut hn = String::new();
                    for i in 20..20 + W3D_NAME_LEN {
                        if i >= chunk_data.len() || chunk_data[i] == 0 {
                            break;
                        }
                        hn.push(chunk_data[i] as char);
                    }
                    hierarchy_name = hn;
                    num_frames = u32::from_le_bytes([
                        chunk_data[36],
                        chunk_data[37],
                        chunk_data[38],
                        chunk_data[39],
                    ]);
                    frame_rate = u16::from_le_bytes([chunk_data[40], chunk_data[41]]) as u32;
                    flavor = u16::from_le_bytes([chunk_data[42], chunk_data[43]]);
                    debug!(
                        "Compressed animation header: name='{}', hierarchy='{}', frames={}, fps={}, flavor={}",
                        name, hierarchy_name, num_frames, frame_rate, flavor
                    );
                }
                W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL => {
                    if chunk_data.len() < 8 {
                        offset += 8 + chunk_size;
                        continue;
                    }
                    if flavor == ANIM_FLAVOR_TIMECODED {
                        if let Some(ch) = Self::parse_timecoded_channel(chunk_data, num_frames) {
                            channels.push(ch);
                        }
                    } else {
                        let chs = Self::parse_adaptive_delta_channel(chunk_data, num_frames);
                        channels.extend(chs);
                    }
                }
                W3D_CHUNK_COMPRESSED_BIT_CHANNEL => {
                    // W3dTimeCodedBitChannelStruct starts with
                    // NumTimeCodes(u32), Pivot(u16), Flags(u8), Default(u8).
                    // The C++ HCompressedAnim class installs flags==0 as an
                    // HTree visibility source. Keep its pivot and make that
                    // child non-rendering until time-coded sampling is ported.
                    if chunk_data.len() < 8 {
                        unsupported_visibility_pivots.push(None);
                        debug!(
                            "Malformed compressed W3D bit channel has no recoverable pivot, size: {}",
                            chunk_size
                        );
                    } else {
                        let pivot = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
                        let flags = chunk_data[6];
                        if flags == 0 {
                            unsupported_visibility_pivots.push(Some(pivot));
                            debug!(
                                "Compressed W3D visibility channel for pivot {} retained as unsupported",
                                pivot
                            );
                        }
                    }
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok(W3dAnimation {
            name,
            hierarchy_name,
            num_frames,
            frame_rate,
            source_is_compressed: true,
            channels,
            raw_visibility_channels,
            unsupported_visibility_pivots,
        })
    }

    /// Parse a single timecoded animation channel (ANIM_FLAVOR_TIMECODED, flavor=0).
    /// Format: num_timecodes(u32) + pivot(u16) + vector_len(u8) + flags(u8)
    ///         then [timecode(u32) + vector_len * f32] per timecode.
    /// Timecode MSB = binary (step) flag; lower 31 bits = frame index.
    /// Produces a densified per-frame W3dAnimChannel matching the uncompressed layout.
    fn parse_timecoded_channel(chunk_data: &[u8], num_frames: u32) -> Option<W3dAnimChannel> {
        if chunk_data.len() < 8 {
            return None;
        }
        let num_timecodes =
            u32::from_le_bytes([chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3]])
                as usize;
        let pivot = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
        let vector_len = chunk_data[6] as u16;
        let flags_tc = chunk_data[7] as u16;

        if vector_len == 0 {
            return None;
        }

        let vl = vector_len as usize;
        let entry_size = 4 + vl * 4;
        if chunk_data.len() < 8 + num_timecodes * entry_size {
            return None;
        }

        let mut timecodes: Vec<(u32, bool, Vec<f32>)> = Vec::with_capacity(num_timecodes);
        for i in 0..num_timecodes {
            let base = 8 + i * entry_size;
            let tc = u32::from_le_bytes([
                chunk_data[base],
                chunk_data[base + 1],
                chunk_data[base + 2],
                chunk_data[base + 3],
            ]);
            let binary = (tc & 0x8000_0000) != 0;
            let frame = tc & 0x7FFF_FFFF;
            let mut vals = Vec::with_capacity(vl);
            for c in 0..vl {
                let off = base + 4 + c * 4;
                vals.push(f32::from_le_bytes([
                    chunk_data[off],
                    chunk_data[off + 1],
                    chunk_data[off + 2],
                    chunk_data[off + 3],
                ]));
            }
            timecodes.push((frame, binary, vals));
        }

        let total_frames = num_frames as usize;
        if total_frames == 0 {
            return Some(W3dAnimChannel {
                first_frame: 0,
                last_frame: 0,
                vector_len,
                flags: Self::map_timecoded_flag(flags_tc),
                pivot,
                data: Vec::new(),
            });
        }

        let mut data = vec![0.0f32; total_frames * vl];

        if timecodes.is_empty() {
            return Some(W3dAnimChannel {
                first_frame: 0,
                last_frame: (num_frames.max(1) - 1) as u16,
                vector_len,
                flags: Self::map_timecoded_flag(flags_tc),
                pivot,
                data,
            });
        }

        timecodes.sort_by_key(|t| t.0);

        let (first_tc, _, ref v0) = timecodes[0];
        for f in 0..(first_tc.min(num_frames)) as usize {
            let base = f * vl;
            for c in 0..vl {
                data[base + c] = v0[c];
            }
        }

        for seg in 0..timecodes.len() {
            let (f0, bin0, ref v0_vals) = timecodes[seg];
            let f1 = if seg + 1 < timecodes.len() {
                timecodes[seg + 1].0
            } else {
                num_frames - 1
            };
            let v1_vals = if seg + 1 < timecodes.len() {
                &timecodes[seg + 1].2
            } else {
                v0_vals
            };
            let start_f = f0 as usize;
            let end_f = f1 as usize;
            if start_f > end_f || start_f as u32 >= num_frames {
                continue;
            }
            let clamped_end = end_f.min(total_frames - 1);
            for f in start_f..=clamped_end {
                let t = if end_f == start_f {
                    0.0
                } else {
                    (f as f32 - start_f as f32) / (end_f as f32 - start_f as f32)
                };
                let base = f * vl;
                for c in 0..vl {
                    data[base + c] = if bin0 {
                        v0_vals[c]
                    } else {
                        v0_vals[c] * (1.0 - t) + v1_vals[c] * t
                    };
                }
            }
        }

        Some(W3dAnimChannel {
            first_frame: 0,
            last_frame: (num_frames.max(1) - 1) as u16,
            vector_len,
            flags: Self::map_timecoded_flag(flags_tc),
            pivot,
            data,
        })
    }

    /// Parse a single adaptive delta animation channel (ANIM_FLAVOR_ADAPTIVE_DELTA, flavor=1).
    /// Format: num_frames(u32) + pivot(u16) + vector_len(u8) + flags(u8) + scale(f32)
    ///         then blocks of 16 frames each, where each block contains vector_len packets.
    ///         Each packet: 1 byte (filter_idx in lower 7 bits) + 8 bytes (16 4-bit nibbles).
    ///         Delta decoding with adaptive filter table.
    fn parse_adaptive_delta_channel(chunk_data: &[u8], hdr_num_frames: u32) -> Vec<W3dAnimChannel> {
        if chunk_data.len() < 12 {
            return Vec::new();
        }
        let num_frames =
            u32::from_le_bytes([chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3]])
                as usize;
        let pivot = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
        let vector_len = chunk_data[6] as usize;
        let _flags_raw = chunk_data[7] as u16;
        let scale =
            f32::from_le_bytes([chunk_data[8], chunk_data[9], chunk_data[10], chunk_data[11]]);

        if vector_len == 0 || num_frames == 0 {
            return Vec::new();
        }

        let blocks = num_frames.div_ceil(16);
        let packet_count = blocks * vector_len;
        let bytes_needed = 8 + packet_count * 9;
        if chunk_data.len() < bytes_needed {
            return Vec::new();
        }

        let filter_table = Self::build_adaptive_delta_filter_table();
        let mut data = vec![0.0f32; num_frames * vector_len];
        let mut last_vals = vec![0.0f32; vector_len];
        if vector_len == 4 {
            last_vals[3] = 1.0;
        }

        let mut read_pos = 12usize;
        for b in 0..blocks {
            for vi in 0..vector_len {
                let b0 = chunk_data[read_pos];
                read_pos += 1;
                let filter_idx = (b0 & 0x7F) as usize;
                let mut nibbles = [0u8; 16];
                for byte_i in 0..8 {
                    let byte = chunk_data[read_pos];
                    read_pos += 1;
                    nibbles[byte_i * 2] = byte & 0x0F;
                    nibbles[byte_i * 2 + 1] = (byte >> 4) & 0x0F;
                }
                let filter = filter_table.get(filter_idx).copied().unwrap_or(1.0) * scale;
                for fi in 0..16 {
                    let frame = b * 16 + fi;
                    if frame >= num_frames {
                        break;
                    }
                    let raw = nibbles[fi] as i32;
                    let factor = (raw - 8) as f32;
                    let value = last_vals[vi] + factor * filter;
                    data[frame * vector_len + vi] = value;
                    last_vals[vi] = value;
                }
            }
        }

        let mut out = Vec::new();
        if vector_len == 3 {
            for axis in 0..3 {
                let mut axis_data = Vec::with_capacity(num_frames);
                for f in 0..num_frames {
                    axis_data.push(data[f * 3 + axis]);
                }
                out.push(W3dAnimChannel {
                    first_frame: 0,
                    last_frame: (hdr_num_frames.max(1) - 1) as u16,
                    vector_len: 1,
                    flags: axis as u16,
                    pivot,
                    data: axis_data,
                });
            }
        } else if vector_len == 4 {
            for f in 0..num_frames {
                let i = f * 4;
                let x = data[i];
                let y = data[i + 1];
                let z = data[i + 2];
                let w = data[i + 3];
                let len = (x * x + y * y + z * z + w * w).sqrt();
                if len > 1e-5 {
                    data[i] = x / len;
                    data[i + 1] = y / len;
                    data[i + 2] = z / len;
                    data[i + 3] = w / len;
                }
            }
            out.push(W3dAnimChannel {
                first_frame: 0,
                last_frame: (hdr_num_frames.max(1) - 1) as u16,
                vector_len: 4,
                flags: 6,
                pivot,
                data,
            });
        } else {
            let mut axis_data = Vec::with_capacity(num_frames);
            for f in 0..num_frames {
                axis_data.push(data[f * vector_len]);
            }
            out.push(W3dAnimChannel {
                first_frame: 0,
                last_frame: (hdr_num_frames.max(1) - 1) as u16,
                vector_len: 1,
                flags: 0,
                pivot,
                data: axis_data,
            });
        }
        out
    }

    fn map_timecoded_flag(flag: u16) -> u16 {
        match flag {
            8 => 0,
            9 => 1,
            10 => 2,
            11 => 6,
            _ => flag,
        }
    }

    fn build_adaptive_delta_filter_table() -> [f32; 256] {
        let mut table = [0.0f32; 256];
        let base: [f32; 16] = [
            1e-8, 1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1, 1.0, 10.0, 100.0, 1000.0, 10000.0,
            100000.0, 1000000.0, 10000000.0,
        ];
        table[..16].copy_from_slice(&base);
        let gen_start = 16usize;
        let gen_size = 256 - gen_start;
        for i in 0..gen_size {
            let ratio = i as f32 / gen_size as f32;
            table[gen_start + i] = 1.0 - (std::f32::consts::FRAC_PI_2 * ratio).sin();
        }
        table
    }

    /// Compute global bone transforms from hierarchy pivots.
    /// Ported from standalone writer.rs compute_global_transforms + mat4_from_tr_quat.
    fn compute_global_transforms(hierarchy: &W3dHierarchy) -> Option<Vec<[f32; 16]>> {
        let locals = hierarchy
            .pivots
            .iter()
            .map(Self::mat4_from_tr_quat)
            .collect::<Vec<_>>();
        compute_htree_global_transforms_from_locals(hierarchy, &locals)
    }

    /// Build the exact source-space affine matrix used by the shared HTree
    /// evaluator.  Keep the legacy HMODEL residual on the same representation
    /// as rigid HLOD/palette paths so parent-child order cannot diverge.
    fn mat4_from_tr_quat(pivot: &W3dPivot) -> [f32; 16] {
        mat4_from_pivot(pivot)
    }

    /// Resolve texture indices to actual texture names - matches C++ behavior
    /// W3D_CHUNK_MAP3_FILENAME may contain texture indices (e.g., "1", "2", "3")
    /// which need to be resolved against the model.texture_names array
    ///
    /// Special case: If texture_names is empty but materials have numeric texture references,
    /// we need to build a texture array from materials in order (C++ behavior when W3D_CHUNK_TEXTURES is missing)
    fn resolve_texture_indices(&self, model: &mut W3DModel) {
        // Check if any texture references are numeric indices
        let has_numeric_indices = model.materials.values().any(|mat| {
            if let Some(ref tex_ref) = mat.texture_name {
                tex_ref.parse::<usize>().is_ok()
            } else {
                false
            }
        }) || model.meshes.iter().any(|mesh| {
            if let Some(ref tex_ref) = mesh.material.texture_name {
                tex_ref.parse::<usize>().is_ok()
            } else {
                false
            }
        });

        // If we have numeric indices but no texture_names array, build one from materials
        if has_numeric_indices && model.texture_names.is_empty() {
            debug!("Building texture array from materials (W3D_CHUNK_TEXTURES missing)");

            // Collect all actual texture filenames from materials in order they appear
            let mut collected_textures: Vec<String> = Vec::new();

            for material in model.materials.values() {
                // Some materials might point to actual filenames (from DC_MAP chunks)
                if let Some(ref tex_name) = material.texture_name {
                    // Only add non-numeric filenames - these are actual texture names
                    if tex_name.parse::<usize>().is_err() && !collected_textures.contains(tex_name)
                    {
                        debug!("  Added texture from material: {}", tex_name);
                        collected_textures.push(tex_name.clone());
                    }
                }
            }

            // If we collected any textures, use them as the texture_names array
            if !collected_textures.is_empty() {
                debug!(
                    "Collected {} textures from materials",
                    collected_textures.len()
                );
                model.texture_names = collected_textures;
            } else {
                // No actual filenames found - this might be a pure index-based model
                debug!("No texture filenames in materials, cannot resolve indices");
                return;
            }
        }

        if model.texture_names.is_empty() {
            debug!("No texture names loaded from W3D_CHUNK_TEXTURES, skipping texture index resolution");
            return;
        }

        debug!("Resolving texture indices for model: {}", model.name);
        debug!("  Available textures: {:?}", model.texture_names);

        // Go through each mesh and resolve texture indices
        for mesh in &mut model.meshes {
            if mesh.texture_library.is_empty() {
                mesh.texture_library = model.texture_names.clone();
            }

            if let Some(ref texture_ref) = mesh.material.texture_name {
                // Try to parse texture_ref as an index
                if let Ok(index) = texture_ref.parse::<usize>() {
                    // It's an index - resolve it
                    if index < model.texture_names.len() {
                        let resolved_name = model.texture_names[index].clone();
                        debug!(
                            "Resolved texture index {} to texture name: {}",
                            index, resolved_name
                        );
                        mesh.material.texture_name = Some(resolved_name);
                    } else {
                        warn!(
                            "Texture index {} out of bounds (only {} textures available)",
                            index,
                            model.texture_names.len()
                        );
                    }
                } else {
                    // It's a filename, keep as-is
                    debug!(
                        "Texture reference '{}' is not an index, keeping as filename",
                        texture_ref
                    );
                }
            }

            if !mesh.per_pass_stage_texture_ids.is_empty() {
                let mut per_pass_names = Vec::with_capacity(mesh.per_pass_stage_texture_ids.len());
                for stages in &mesh.per_pass_stage_texture_ids {
                    let mut stage_names = Vec::with_capacity(stages.len());
                    for ids in stages {
                        let names = ids
                            .iter()
                            .filter_map(|texture_id| {
                                if *texture_id == u32::MAX {
                                    None
                                } else {
                                    mesh.texture_name_from_library(*texture_id)
                                        .map(|name| name.to_string())
                                }
                            })
                            .collect::<Vec<_>>();
                        stage_names.push(names);
                    }
                    per_pass_names.push(stage_names);
                }
                mesh.per_pass_stage_texture_names = per_pass_names;

                if mesh.material.texture_name.is_none() {
                    mesh.material.texture_name = Self::stage_texture_from_mesh(mesh, 0, 0);
                }
            }
        }

        // Also update materials map if they have texture references
        for (name, material) in &mut model.materials {
            if let Some(ref texture_ref) = material.texture_name {
                if let Ok(index) = texture_ref.parse::<usize>() {
                    if index < model.texture_names.len() {
                        let resolved_name = model.texture_names[index].clone();
                        debug!(
                            "Resolved material '{}' texture index {} to: {}",
                            name, index, resolved_name
                        );
                        let mut updated_material = material.clone();
                        updated_material.texture_name = Some(resolved_name);
                        *material = updated_material;
                    }
                }
            }
        }
    }

    fn parse_u32_array(&self, data: &[u8]) -> Result<Vec<u32>> {
        if !data.len().is_multiple_of(4) {
            return Err(anyhow!("invalid u32 array length {}", data.len()));
        }
        let mut values = Vec::with_capacity(data.len() / 4);
        let mut offset = 0usize;
        while offset + 4 <= data.len() {
            values.push(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
        }
        Ok(values)
    }

    fn parse_rgba_colors(&self, data: &[u8]) -> Result<Vec<W3dRGBAStruct>> {
        if !data.len().is_multiple_of(4) {
            return Err(anyhow!("invalid RGBA array length {}", data.len()));
        }
        let mut colors = Vec::with_capacity(data.len() / 4);
        let mut offset = 0usize;
        while offset + 4 <= data.len() {
            colors.push(W3dRGBAStruct {
                r: data[offset],
                g: data[offset + 1],
                b: data[offset + 2],
                a: data[offset + 3],
            });
            offset += 4;
        }
        Ok(colors)
    }

    fn parse_per_face_texcoord_ids(&self, data: &[u8]) -> Result<Vec<[u32; 3]>> {
        if !data.len().is_multiple_of(12) {
            return Err(anyhow!(
                "invalid per-face texcoord id array length {}",
                data.len()
            ));
        }
        let mut values = Vec::with_capacity(data.len() / 12);
        let mut offset = 0usize;
        while offset + 12 <= data.len() {
            values.push([
                u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]),
                u32::from_le_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]),
                u32::from_le_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]),
            ]);
            offset += 12;
        }
        Ok(values)
    }

    fn parse_texture_stage_chunk(&self, data: &[u8]) -> Result<ParsedTextureStage> {
        let mut stage = ParsedTextureStage::default();
        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            match chunk_type {
                W3D_CHUNK_TEXTURE_IDS => {
                    stage.texture_ids = self.parse_u32_array(chunk_data)?;
                }
                W3D_CHUNK_STAGE_TEXCOORDS | W3D_CHUNK_TEXCOORDS => {
                    stage.texcoords = self.parse_texcoords(chunk_data)?;
                }
                W3D_CHUNK_PER_FACE_TEXCOORD_IDS => {
                    stage.per_face_texcoord_ids = self.parse_per_face_texcoord_ids(chunk_data)?;
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }
        Ok(stage)
    }

    fn parse_material_pass_chunk(&self, data: &[u8]) -> Result<ParsedMaterialPass> {
        let mut pass = ParsedMaterialPass::default();
        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            match chunk_type {
                W3D_CHUNK_VERTEX_MATERIAL_IDS => {
                    pass.vertex_material_ids = self.parse_u32_array(chunk_data)?;
                }
                W3D_CHUNK_SHADER_IDS => {
                    pass.shader_ids = self.parse_u32_array(chunk_data)?;
                }
                W3D_CHUNK_DCG => {
                    pass.dcg_colors = self.parse_rgba_colors(chunk_data)?;
                }
                W3D_CHUNK_DIG => {
                    // C++ reads DIG as W3dRGBAStruct and uses RGB channels.
                    pass.dig_colors = self.parse_rgba_colors(chunk_data)?;
                }
                W3D_CHUNK_TEXTURE_STAGE => {
                    let stage = self.parse_texture_stage_chunk(chunk_data)?;
                    pass.stage_texture_ids.push(stage.texture_ids);
                    pass.stage_texcoords.push(stage.texcoords);
                    pass.stage_per_face_texcoord_ids
                        .push(stage.per_face_texcoord_ids);
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok(pass)
    }

    fn parse_shaders_chunk(&self, data: &[u8]) -> Result<Vec<W3dShaderStruct>> {
        // C++ W3dShaderStruct is 16 bytes (15 data bytes + 1 pad byte).
        if !data.len().is_multiple_of(16) {
            return Err(anyhow!("invalid shader chunk length {}", data.len()));
        }

        let mut shaders = Vec::with_capacity(data.len() / 16);
        let mut offset = 0usize;
        while offset + 16 <= data.len() {
            shaders.push(W3dShaderStruct {
                depth_compare: data[offset],
                depth_mask: data[offset + 1],
                color_mask: data[offset + 2],
                dest_blend: data[offset + 3],
                fog_func: data[offset + 4],
                pri_gradient: data[offset + 5],
                sec_gradient: data[offset + 6],
                src_blend: data[offset + 7],
                texturing: data[offset + 8],
                detail_color_func: data[offset + 9],
                detail_alpha_func: data[offset + 10],
                shader_preset: data[offset + 11],
                alpha_test: data[offset + 12],
                post_detail_color_func: data[offset + 13],
                post_detail_alpha_func: data[offset + 14],
            });
            offset += 16;
        }
        Ok(shaders)
    }

    fn default_vertex_material() -> W3dVertexMaterialStruct {
        W3dVertexMaterialStruct {
            attributes: 0,
            ambient: W3dRGBAStruct {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            diffuse: W3dRGBAStruct {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            specular: W3dRGBAStruct {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            emissive: W3dRGBAStruct {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            shininess: 1.0,
            opacity: 1.0,
            translucency: 0.0,
        }
    }

    fn parse_vertex_material_info_chunk(&self, data: &[u8]) -> Result<W3dVertexMaterialStruct> {
        // C++ W3dVertexMaterialStruct uses 3-byte RGB triplets with 4-byte alignment.
        // Accept both canonical 28-byte layout and 32-byte RGBA-expanded variant.
        if data.len() < 28 {
            return Err(anyhow!(
                "vertex material info chunk too small: {} bytes",
                data.len()
            ));
        }

        let mut material = Self::default_vertex_material();
        material.attributes = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        if data.len() >= 32 {
            material.ambient = W3dRGBAStruct {
                r: data[4],
                g: data[5],
                b: data[6],
                a: data[7],
            };
            material.diffuse = W3dRGBAStruct {
                r: data[8],
                g: data[9],
                b: data[10],
                a: data[11],
            };
            material.specular = W3dRGBAStruct {
                r: data[12],
                g: data[13],
                b: data[14],
                a: data[15],
            };
            material.emissive = W3dRGBAStruct {
                r: data[16],
                g: data[17],
                b: data[18],
                a: data[19],
            };
            material.shininess = f32::from_le_bytes([data[20], data[21], data[22], data[23]]);
            material.opacity = f32::from_le_bytes([data[24], data[25], data[26], data[27]]);
            material.translucency = f32::from_le_bytes([data[28], data[29], data[30], data[31]]);
        } else {
            material.ambient = W3dRGBAStruct {
                r: data[4],
                g: data[5],
                b: data[6],
                a: 255,
            };
            material.diffuse = W3dRGBAStruct {
                r: data[7],
                g: data[8],
                b: data[9],
                a: 255,
            };
            material.specular = W3dRGBAStruct {
                r: data[10],
                g: data[11],
                b: data[12],
                a: 255,
            };
            material.emissive = W3dRGBAStruct {
                r: data[13],
                g: data[14],
                b: data[15],
                a: 255,
            };
            material.shininess = f32::from_le_bytes([data[16], data[17], data[18], data[19]]);
            material.opacity = f32::from_le_bytes([data[20], data[21], data[22], data[23]]);
            material.translucency = f32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        }

        Ok(material)
    }

    fn parse_single_vertex_material_chunk(
        &self,
        data: &[u8],
    ) -> Result<(W3dVertexMaterialStruct, VertexMapperConfig)> {
        let mut material = Self::default_vertex_material();
        let mapper = VertexMapperConfig::default();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            match chunk_type {
                W3D_CHUNK_VERTEX_MATERIAL_INFO => {
                    material = self.parse_vertex_material_info_chunk(chunk_data)?;
                }
                W3D_CHUNK_VERTEX_MATERIAL_NAME
                | W3D_CHUNK_VERTEX_MAPPER_ARGS0
                | W3D_CHUNK_VERTEX_MAPPER_ARGS1 => {}
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok((material, mapper))
    }

    fn parse_vertex_materials_chunk(
        &self,
        data: &[u8],
    ) -> Result<(Vec<W3dVertexMaterialStruct>, Vec<VertexMapperConfig>)> {
        let mut materials = Vec::new();
        let mut mappers = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            if chunk_type == W3D_CHUNK_VERTEX_MATERIAL {
                let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
                let (material, mapper) = self.parse_single_vertex_material_chunk(chunk_data)?;
                materials.push(material);
                mappers.push(mapper);
            }

            offset += 8 + chunk_size;
        }

        Ok((materials, mappers))
    }

    /// Parse a W3D mesh chunk
    fn parse_mesh_chunk(&self, data: &[u8]) -> Result<W3DMesh> {
        debug!("parse_mesh_chunk called, data size: {} bytes", data.len());
        let mut mesh = W3DMesh::new("unknown_mesh".to_string());
        let mut offset = 0;
        let mut has_valid_mesh_header = false;

        let mut vertices: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut texcoords: Vec<[f32; 2]> = Vec::new();
        let mut vertex_colors: Vec<[f32; 4]> = Vec::new();
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        let mut expected_vertex_count: Option<u32> = None;
        let mut mesh_header_version: Option<u32> = None;
        // C++ `MeshGeometryClass::read_vertex_influences` writes its one
        // allocated link array on every occurrence. Retain only the last
        // complete chunk here for the same overwrite behavior; a short chunk
        // returns an error immediately, so Main fails the whole mesh closed
        // instead of retaining its partial trusted-data link array.
        let mut vertex_influences: Option<Vec<W3dVertInfStruct>> = None;
        let mut texture_names: Vec<String> = Vec::new(); // C++ MeshLoadContextClass texture array

        // Parse mesh sub-chunks with safety counter
        let mut mesh_chunk_counter = 0;
        const MAX_MESH_CHUNKS: usize = 1000; // Safety limit for mesh chunks

        while offset + 8 <= data.len() && mesh_chunk_counter < MAX_MESH_CHUNKS {
            mesh_chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            let _is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Mesh sub-chunk extends beyond mesh: type 0x{:08X}, size {}",
                    chunk_type, chunk_size
                );
                break;
            }

            // Safety checks for mesh chunks
            if chunk_size == 0 {
                warn!(
                    "Zero-sized mesh chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8;
                continue;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH_HEADER => {
                    debug!(
                        "Parsing mesh header (W3dMeshHeader3Struct), size: {}",
                        chunk_size
                    );
                    let header = self
                        .parse_mesh_header(chunk_data)
                        .map_err(|e| anyhow!("invalid mesh header in '{}': {}", mesh.name, e))?;
                    has_valid_mesh_header = true;
                    mesh.name = header.mesh_name;
                    mesh.container_name = header.container_name;
                    expected_vertex_count = Some(header.num_vertices);
                    mesh_header_version = Some(header.version);
                    debug!(
                        "Mesh name: '{}', expecting {} vertices, {} triangles",
                        mesh.name, header.num_vertices, header.num_triangles
                    );
                }
                W3D_CHUNK_VERTICES => {
                    vertices = self.parse_vertices_with_count(chunk_data, expected_vertex_count)?;
                    debug!("Parsed {} vertices", vertices.len());
                }
                W3D_CHUNK_VERTEX_NORMALS => {
                    normals = self.parse_normals(chunk_data)?;
                    debug!("Parsed {} normals", normals.len());
                }
                W3D_CHUNK_TEXCOORDS => {
                    texcoords = self.parse_texcoords(chunk_data)?;
                    debug!("Parsed {} texture coordinates", texcoords.len());
                }
                W3D_CHUNK_VERTEX_COLORS => {
                    vertex_colors = self.parse_vertex_colors(chunk_data)?;
                    debug!("Parsed {} vertex colors", vertex_colors.len());
                }
                W3D_CHUNK_VERTEX_INFLUENCES => {
                    vertex_influences = Some(
                        self.parse_vertex_influences_with_count(chunk_data, expected_vertex_count)?,
                    );
                    debug!(
                        "Parsed {} exact W3dVertInfStruct records",
                        vertex_influences
                            .as_ref()
                            .map_or(0, |influences| influences.len())
                    );
                }
                W3D_CHUNK_TRIANGLES => {
                    triangles = self.parse_triangles(chunk_data)?;
                    debug!("Parsed {} triangles", triangles.len());
                }
                W3D_CHUNK_MATERIAL_INFO => {
                    debug!("Parsing material info chunk, size: {}", chunk_size);
                    if let Ok(material) = self.parse_material_info(chunk_data) {
                        mesh.material = material;
                        debug!(
                            "Parsed material: {} (texture: {:?})",
                            mesh.material.name, mesh.material.texture_name
                        );
                    } else {
                        warn!("Failed to parse material info chunk");
                    }
                }
                W3D_CHUNK_MAP3_FILENAME => {
                    // Extract texture filename from MAP3_FILENAME chunk
                    // Read null-terminated string directly from chunk data
                    let mut filename = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            filename.push(byte as char);
                        }
                    }
                    if !filename.is_empty() {
                        debug!(
                            "Found texture filename in W3D_CHUNK_MAP3_FILENAME: {}",
                            filename
                        );
                        mesh.material.texture_name = Some(filename);
                    }
                }
                W3D_CHUNK_VERTEX_SHADE_INDICES => {
                    // Shade indices for vertex coloring - skip for now
                    debug!(
                        "Skipping W3D_CHUNK_VERTEX_SHADE_INDICES ({} bytes)",
                        chunk_size
                    );
                }
                W3D_CHUNK_SHADERS => match self.parse_shaders_chunk(chunk_data) {
                    Ok(shaders) => {
                        debug!("Parsed {} shaders", shaders.len());
                        mesh.shaders = shaders;
                    }
                    Err(err) => {
                        warn!("Failed to parse W3D_CHUNK_SHADERS: {}", err);
                    }
                },
                W3D_CHUNK_VERTEX_MATERIALS => match self.parse_vertex_materials_chunk(chunk_data) {
                    Ok((materials, mappers)) => {
                        debug!(
                            "Parsed {} vertex materials and {} mapper configs",
                            materials.len(),
                            mappers.len()
                        );
                        mesh.vertex_materials = materials;
                        mesh.vertex_mappers = mappers;
                    }
                    Err(err) => {
                        warn!("Failed to parse W3D_CHUNK_VERTEX_MATERIALS: {}", err);
                    }
                },
                W3D_CHUNK_MATERIAL_PASS => match self.parse_material_pass_chunk(chunk_data) {
                    Ok(pass_data) => {
                        let mut stage_texture_names = Vec::new();
                        for texture_ids in &pass_data.stage_texture_ids {
                            let names = texture_ids
                                .iter()
                                .filter_map(|texture_id| {
                                    if *texture_id == u32::MAX {
                                        return None;
                                    }
                                    texture_names.get(*texture_id as usize).cloned()
                                })
                                .collect::<Vec<_>>();
                            stage_texture_names.push(names);
                        }

                        mesh.passes.push(MaterialPassInfo {
                            vm_id: pass_data.vertex_material_ids.first().copied().unwrap_or(0),
                            shader_id: pass_data.shader_ids.first().copied().unwrap_or(0),
                            texture_count: pass_data.stage_texture_ids.len() as u32,
                        });
                        mesh.per_pass_vertex_material_ids
                            .push(pass_data.vertex_material_ids.clone());
                        mesh.per_pass_shader_ids.push(pass_data.shader_ids.clone());
                        mesh.per_pass_dcg_colors.push(pass_data.dcg_colors.clone());
                        mesh.per_pass_dig_colors.push(pass_data.dig_colors.clone());
                        mesh.per_pass_stage_texture_ids
                            .push(pass_data.stage_texture_ids.clone());
                        mesh.per_pass_stage_texture_names.push(stage_texture_names);

                        for (stage_index, stage_uvs) in pass_data.stage_texcoords.iter().enumerate()
                        {
                            mesh.stage_texcoords.push(stage_uvs.clone());
                            mesh.per_stage_face_texcoord_ids.push(
                                pass_data
                                    .stage_per_face_texcoord_ids
                                    .get(stage_index)
                                    .cloned()
                                    .unwrap_or_default(),
                            );
                        }
                    }
                    Err(err) => {
                        warn!("Failed to parse W3D_CHUNK_MATERIAL_PASS: {}", err);
                    }
                },
                W3D_CHUNK_TEXTURES => {
                    // Parse textures container - C++ read_textures() equivalent
                    debug!(
                        "Found W3D_CHUNK_TEXTURES inside mesh, size: {} bytes",
                        chunk_size
                    );
                    // Parse texture names from W3D_CHUNK_TEXTURE/W3D_CHUNK_TEXTURE_NAME
                    if let Ok(names) = self.parse_textures_chunk_into_array(chunk_data) {
                        debug!("Loaded {} texture(s) for mesh: {:?}", names.len(), names);
                        texture_names.extend(names);
                    }
                }
                _ => {
                    debug!("Unknown mesh sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        if mesh_chunk_counter >= MAX_MESH_CHUNKS {
            warn!(
                "⚠️  Mesh chunk parsing hit safety limit ({} chunks)",
                MAX_MESH_CHUNKS
            );
        }

        if !has_valid_mesh_header {
            return Err(anyhow!("mesh chunk missing required W3D mesh header"));
        }

        let stage0_fallback_texcoords = texcoords.clone();

        // Build final mesh (logging disabled)
        self.build_mesh_from_data(
            &mut mesh,
            vertices,
            normals,
            texcoords,
            vertex_colors,
            triangles,
        )?;

        if let Some(mut influences) = vertex_influences {
            let expected_count = expected_vertex_count
                .and_then(|count| usize::try_from(count).ok())
                .ok_or_else(|| anyhow!("vertex influences require a valid mesh header"))?;
            if influences.len() != expected_count || influences.len() != mesh.vertices.len() {
                return Err(anyhow!(
                    "mesh '{}' vertex influences do not match its exact vertex count",
                    mesh.name
                ));
            }

            // `MeshModelClass::Load_W3D` adjusts only successfully loaded
            // pre-3.0 skin links after all mesh chunks have been read. C++
            // stores those links as uint16, so the increment wraps at u16::MAX.
            if mesh_header_version.is_some_and(|version| version < W3D_HTREE_ROOT_VERSION) {
                for influence in &mut influences {
                    influence.bone_idx = influence.bone_idx.wrapping_add(1);
                }
            }
            mesh.vertex_influences = Some(influences);
        }

        if !texture_names.is_empty() {
            mesh.texture_library = texture_names.clone();
        }

        if mesh.stage_texcoords.is_empty() && !stage0_fallback_texcoords.is_empty() {
            mesh.stage_texcoords.push(stage0_fallback_texcoords);
            mesh.stage_uv_channels = vec![0];
            if mesh.per_stage_face_texcoord_ids.is_empty() {
                mesh.per_stage_face_texcoord_ids.push(Vec::new());
            }
        } else if !mesh.stage_texcoords.is_empty() {
            let (unique_layers, stage_channels) =
                deduplicate_stage_uv_layers(mesh.stage_texcoords.clone());
            mesh.stage_texcoords = unique_layers;
            mesh.stage_uv_channels = stage_channels;
            if mesh.per_stage_face_texcoord_ids.is_empty() {
                mesh.per_stage_face_texcoord_ids = vec![Vec::new(); mesh.stage_texcoords.len()];
            }
        }

        if !mesh.per_pass_stage_texture_ids.is_empty() {
            let mut per_pass_names = Vec::with_capacity(mesh.per_pass_stage_texture_ids.len());
            for stage_set in &mesh.per_pass_stage_texture_ids {
                let mut stage_names = Vec::with_capacity(stage_set.len());
                for ids in stage_set {
                    let names = ids
                        .iter()
                        .filter_map(|texture_id| {
                            if *texture_id == u32::MAX {
                                None
                            } else {
                                mesh.texture_name_from_library(*texture_id)
                                    .map(|name| name.to_string())
                            }
                        })
                        .collect::<Vec<_>>();
                    stage_names.push(names);
                }
                per_pass_names.push(stage_names);
            }
            mesh.per_pass_stage_texture_names = per_pass_names;
        }

        // C++ behavior: single-material fallback uses first texture if pass data does not bind one.
        if mesh.material.texture_name.is_none() && !texture_names.is_empty() {
            mesh.material.texture_name = Some(texture_names[0].clone());
        }
        if mesh.material.texture_name.is_none() {
            mesh.material.texture_name = Self::stage_texture_from_mesh(&mesh, 0, 0);
        }

        if let Some(texture_name) = &mesh.material.texture_name {
            debug!("Mesh '{}' will use texture: '{}'", mesh.name, texture_name);
        }

        // Map W3D shader blend factors to material blend_mode for C++ parity.
        // Uses the first shader, or the shader referenced by the first material pass.
        let shader_idx = mesh
            .passes
            .first()
            .map(|p| p.shader_id as usize)
            .unwrap_or(0);
        if let Some(shader) = mesh.shaders.get(shader_idx) {
            let (mode, alpha_test) =
                shader_blend_to_mode(shader.src_blend, shader.dest_blend, shader.alpha_test);
            mesh.material.blend_mode = mode;
            mesh.material.alpha_test_enabled = alpha_test;
            debug!(
                "Mesh '{}' blend_mode={:?}, alpha_test={} (src={}, dest={})",
                mesh.name,
                mesh.material.blend_mode,
                mesh.material.alpha_test_enabled,
                shader.src_blend,
                shader.dest_blend
            );
        }

        Ok(mesh)
    }

    /// Parse mesh header - C++ compatible W3dMeshHeader3Struct format
    fn parse_mesh_header(&self, data: &[u8]) -> Result<MeshHeader> {
        // W3dMeshHeader3Struct layout:
        // 0: uint32 Version
        // 4: uint32 Attributes
        // 8: char MeshName[16]
        // 24: char ContainerName[16]
        // 40: uint32 NumTris
        // 44: uint32 NumVertices
        // 48: uint32 NumMaterials
        // 52: uint32 NumDamageStages
        // 56: sint32 SortLevel
        // 60: uint32 PrelitVersion
        // 64: uint32 FutureCounts[1]
        // 68: uint32 VertexChannels
        // 72: uint32 FaceChannels
        // Plus bounding box, sphere data...

        if data.len() < 76 {
            // Minimum size for core header fields
            return Err(anyhow!("Mesh header too small: {} bytes", data.len()));
        }

        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let attributes = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let num_triangles = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
        let num_vertices = u32::from_le_bytes([data[44], data[45], data[46], data[47]]);

        // Extract mesh name (null-terminated string at offset 8, max 16 chars)
        let mut mesh_name = String::new();
        for i in 8..24 {
            if i >= data.len() || data[i] == 0 {
                break;
            }
            mesh_name.push(data[i] as char);
        }

        // Extract container name (null-terminated string at offset 24, max 16 chars)
        let mut container_name = String::new();
        for i in 24..40 {
            if i >= data.len() || data[i] == 0 {
                break;
            }
            container_name.push(data[i] as char);
        }

        debug!("Mesh header - version: 0x{:08X}, attributes: 0x{:08X}, triangles: {}, vertices: {}, mesh_name: '{}', container: '{}'", 
               version, attributes, num_triangles, num_vertices, mesh_name, container_name);

        Ok(MeshHeader {
            version,
            flags: attributes, // attributes field is what was called flags in the old structure
            num_triangles,
            num_vertices,
            mesh_name: if mesh_name.is_empty() {
                "unnamed_mesh".to_string()
            } else {
                mesh_name
            },
            container_name,
        })
    }

    /// Parse one exact source `W3dVertInfStruct` per declared mesh vertex.
    ///
    /// `MeshGeometryClass::read_vertex_influences` reads `sizeof` bytes for
    /// every `Get_Vertex_Count()` entry and returns `WW3D_ERROR_LOAD_FAILED`
    /// on the first short read. It does not validate or reinterpret the pad
    /// bytes, and `Close_Chunk` discards any remaining trailing data. Keeping
    /// that shape matters: an arbitrary extra byte is not a malformed record,
    /// while even one missing byte from the required records makes the C++
    /// reader fail. Main rejects that mesh rather than leaving a partial skin
    /// array behind.
    fn parse_vertex_influences_with_count(
        &self,
        data: &[u8],
        expected_count: Option<u32>,
    ) -> Result<Vec<W3dVertInfStruct>> {
        let vertex_count = usize::try_from(
            expected_count.ok_or_else(|| anyhow!("vertex influences precede mesh header"))?,
        )
        .map_err(|_| anyhow!("mesh vertex count does not fit usize"))?;
        let required_size = vertex_count
            .checked_mul(W3D_VERTEX_INFLUENCE_RECORD_SIZE)
            .ok_or_else(|| anyhow!("vertex influence byte count overflow"))?;
        if data.len() < required_size {
            return Err(anyhow!(
                "insufficient vertex influence data: need {} bytes, have {} (for {} vertices)",
                required_size,
                data.len(),
                vertex_count
            ));
        }

        let mut influences = Vec::with_capacity(vertex_count);
        for record in data[..required_size].chunks_exact(W3D_VERTEX_INFLUENCE_RECORD_SIZE) {
            let bone_idx = u16::from_le_bytes([record[0], record[1]]);
            let mut pad = [0u8; 6];
            pad.copy_from_slice(&record[2..W3D_VERTEX_INFLUENCE_RECORD_SIZE]);
            influences.push(W3dVertInfStruct { bone_idx, pad });
        }
        Ok(influences)
    }

    /// Parse vertices array with expected count validation - C++ compatible version
    fn parse_vertices_with_count(
        &self,
        data: &[u8],
        expected_count: Option<u32>,
    ) -> Result<Vec<[f32; 3]>> {
        // In C++: reads vertex count from mesh header, then reads that many W3dVectorStruct (12 bytes each)
        // No headers or padding in vertex chunk data itself - just raw vertex data

        let vertex_count = if let Some(expected) = expected_count {
            expected as usize
        } else {
            // Fallback: assume data contains only vertices (12 bytes each)
            data.len() / 12
        };

        debug!(
            "parse_vertices_with_count: data.len()={}, expected_count={:?}, using vertex_count={}",
            data.len(),
            expected_count,
            vertex_count
        );

        // Verify we have enough data for the expected vertices
        let required_size = vertex_count * 12; // 12 bytes per W3dVectorStruct
        if data.len() < required_size {
            return Err(anyhow!(
                "Insufficient vertex data: need {} bytes, have {} (for {} vertices)",
                required_size,
                data.len(),
                vertex_count
            ));
        }

        let mut vertices = Vec::with_capacity(vertex_count);

        // Read vertices directly as W3dVectorStruct (float32 X, Y, Z)
        for i in 0..vertex_count {
            let offset = i * 12;
            if offset + 12 > data.len() {
                warn!(
                    "Vertex {} would exceed data bounds, stopping at {} vertices",
                    i,
                    vertices.len()
                );
                break;
            }

            let x = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            // Validate vertices are reasonable (not NaN, not infinite)
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                warn!(
                    "Vertex {} has non-finite coordinates: ({}, {}, {})",
                    i, x, y, z
                );
                continue;
            }

            vertices.push([x, y, z]);

            // Log first few vertices for debugging
            if i < 3 {
                debug!("Vertex {}: ({:.3}, {:.3}, {:.3})", i, x, y, z);
            }
        }

        if vertices.is_empty() {
            return Err(anyhow!("No valid vertices parsed from data"));
        }

        debug!("Successfully parsed {} vertices", vertices.len());
        Ok(vertices)
    }

    /// Legacy parse vertices for backward compatibility
    fn parse_vertices(&self, data: &[u8]) -> Result<Vec<[f32; 3]>> {
        self.parse_vertices_with_count(data, None)
    }

    /// Parse normals array
    fn parse_normals(&self, data: &[u8]) -> Result<Vec<[f32; 3]>> {
        if !data.len().is_multiple_of(12) {
            return Err(anyhow!("Invalid normals data size: {}", data.len()));
        }

        let normal_count = data.len() / 12;
        let mut normals = Vec::with_capacity(normal_count);

        for i in 0..normal_count {
            let offset = i * 12;
            let x = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);
            normals.push([x, y, z]);
        }

        Ok(normals)
    }

    /// Parse texture coordinates array
    fn parse_texcoords(&self, data: &[u8]) -> Result<Vec<[f32; 2]>> {
        if !data.len().is_multiple_of(8) {
            return Err(anyhow!("Invalid texcoords data size: {}", data.len()));
        }

        let texcoord_count = data.len() / 8;
        let mut texcoords = Vec::with_capacity(texcoord_count);

        for i in 0..texcoord_count {
            let offset = i * 8;
            let u = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let v = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            // C++ parity: WW3D stores V upside-down in chunk payload and flips on load.
            texcoords.push([u, 1.0 - v]);
        }

        Ok(texcoords)
    }

    /// Parse vertex colors array
    fn parse_vertex_colors(&self, data: &[u8]) -> Result<Vec<[f32; 4]>> {
        let mut colors = Vec::new();

        if data.len().is_multiple_of(3) {
            let color_count = data.len() / 3;
            colors.reserve(color_count);
            for i in 0..color_count {
                let offset = i * 3;
                colors.push([
                    data[offset] as f32 / 255.0,
                    data[offset + 1] as f32 / 255.0,
                    data[offset + 2] as f32 / 255.0,
                    1.0,
                ]);
            }
            return Ok(colors);
        }

        if data.len().is_multiple_of(4) {
            let color_count = data.len() / 4;
            colors.reserve(color_count);
            for i in 0..color_count {
                let offset = i * 4;
                colors.push([
                    data[offset] as f32 / 255.0,
                    data[offset + 1] as f32 / 255.0,
                    data[offset + 2] as f32 / 255.0,
                    data[offset + 3] as f32 / 255.0,
                ]);
            }
            return Ok(colors);
        }

        Err(anyhow!("Invalid vertex colors data size: {}", data.len()))
    }

    /// Parse material info
    fn parse_material_info(&self, data: &[u8]) -> Result<W3DMaterial> {
        if data.len() < 4 {
            // Need at least 4 bytes for basic parsing
            return Err(anyhow!("Material info chunk too small: {}", data.len()));
        }

        // Material info structure is complex, let's extract basic information
        let mut material = W3DMaterial::default();

        // For small material info chunks (16 bytes), extract basic properties
        // For larger chunks, try to extract more detailed information

        if data.len() >= 48 {
            // Extract C++ VertexMaterialClass compatible color values for larger chunks
            let diffuse_r = f32::from_le_bytes(data[32..36].try_into().unwrap_or([0; 4]));
            let diffuse_g = f32::from_le_bytes(data[36..40].try_into().unwrap_or([0; 4]));
            let diffuse_b = f32::from_le_bytes(data[40..44].try_into().unwrap_or([0; 4]));

            if diffuse_r.is_finite() && diffuse_g.is_finite() && diffuse_b.is_finite() {
                material.diffuse_color = Vec3::new(diffuse_r, diffuse_g, diffuse_b);
            }
        }

        if data.len() >= 32 {
            // Try to extract material name for larger chunks
            let mut name = String::new();
            for i in 0..std::cmp::min(32, data.len()) {
                if data[i] == 0 {
                    break;
                }
                if data[i].is_ascii() && data[i] >= 32 {
                    name.push(data[i] as char);
                }
            }
            if !name.is_empty() {
                material.name = name;
            }
        } else if data.len() >= 16 {
            // For small material info chunks (16 bytes), extract basic properties
            debug!("Parsing 16-byte material info chunk - basic material properties");

            // Try to extract some basic properties from the first few bytes
            // Material index or type might be at the beginning
            let material_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            debug!("Material type/index: 0x{:08X}", material_type);

            // Set basic properties for small chunks
            material.name = format!("material_{:08X}", material_type);
            material.diffuse_color = Vec3::new(0.8, 0.8, 0.8); // Default gray
        }

        // Note: Texture names are now loaded separately from W3D_CHUNK_TEXTURES
        // They will be associated with materials through material passes

        // Set C++ compatible material properties
        material.stage0_mapping.uv_source = UVSource::UV0;
        material.stage0_mapping.blend_mode = TextureBlendMode::Modulate;
        material.blend_mode = BlendMode::Opaque;

        // Set texture name in stage 0 if found
        if let Some(ref texture_name) = material.texture_name {
            material.stage0_mapping.texture_name = Some(texture_name.clone());
        }

        debug!(
            "Parsed material: name='{}', diffuse={:?}, texture={:?}",
            material.name, material.diffuse_color, material.texture_name
        );

        Ok(material)
    }

    /// Parse W3D textures container chunk - contains individual texture definitions
    fn parse_textures_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut texture_count = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Invalid texture chunk size: {} at offset {}",
                    chunk_size, offset
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_TEXTURE => {
                    debug!("Parsing individual texture chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Ok(texture_name) = self.parse_single_texture_chunk(chunk_data) {
                            debug!("Found texture: {}", texture_name);
                            model.texture_names.push(texture_name);
                            texture_count += 1;
                        }
                    }
                }
                _ => {
                    debug!(
                        "Unknown texture sub-chunk: 0x{:08X}, size: {}",
                        chunk_type, chunk_size
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        debug!("Loaded {} textures from W3D_CHUNK_TEXTURES", texture_count);
        Ok(())
    }

    /// Parse W3D_CHUNK_TEXTURES and return array of texture names - C++ read_textures() equivalent
    fn parse_textures_chunk_into_array(&self, data: &[u8]) -> Result<Vec<String>> {
        debug!("parse_textures_chunk_into_array: data.len()={}", data.len());
        let mut textures = Vec::new();
        let mut offset = 0;

        // C++ code: for (TextureClass *newtex = ::Load_Texture(cload); newtex != NULL; newtex = ::Load_Texture(cload))
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // Check for container chunk flag (bit 31 set on chunk size - C++ behavior)
            let is_container = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            // C++ Load_Texture checks for W3D_CHUNK_TEXTURE
            if chunk_type == W3D_CHUNK_TEXTURE && is_container {
                if let Ok(texture_name) = self.parse_single_texture_chunk(chunk_data) {
                    textures.push(texture_name);
                }
            }

            offset += 8 + chunk_size;
        }

        debug!("Returning {} textures", textures.len());
        Ok(textures)
    }

    /// Parse a single W3D_CHUNK_TEXTURE and extract the texture name
    fn parse_single_texture_chunk(&self, data: &[u8]) -> Result<String> {
        let mut offset = 0;
        let mut texture_name: Option<String> = None;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_TEXTURE_NAME => {
                    // Read null-terminated string directly from chunk data
                    let mut name = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            name.push(byte as char);
                        }
                    }

                    if !name.is_empty() {
                        debug!("Found texture name in W3D_CHUNK_TEXTURE_NAME: {}", name);
                        texture_name = Some(name);
                    }
                }
                W3D_CHUNK_TEXTURE_INFO => {
                    debug!("Found W3D_CHUNK_TEXTURE_INFO (not parsing texture properties yet)");
                    // W3dTextureInfoStruct parsing can be added here later if needed
                }
                _ => {
                    debug!(
                        "Unknown texture sub-chunk in W3D_CHUNK_TEXTURE: 0x{:08X}",
                        chunk_type
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        texture_name.ok_or_else(|| anyhow!("No texture name found in W3D_CHUNK_TEXTURE"))
    }

    /// Parse W3D MATERIALS3 container chunk - contains material definitions with texture filenames
    /// This matches the C++ approach: create materials and directly assign texture names
    fn parse_materials3_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut material_count = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Invalid materials3 chunk size: {} at offset {}",
                    chunk_size, offset
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3 => {
                    debug!("Parsing individual material3 chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        // Parse the complete material (name + properties + texture) like C++ does
                        if let Ok(material) = self.parse_complete_material3_chunk(chunk_data) {
                            debug!(
                                "Found material3: '{}' with texture: {:?}",
                                material.name, material.texture_name
                            );

                            // Store the material in the model's materials HashMap
                            model
                                .materials
                                .insert(material.name.clone(), material.clone());

                            // Also add texture name to the model's texture list for loading
                            if let Some(ref texture_name) = material.texture_name {
                                model.texture_names.push(texture_name.clone());
                            }
                            material_count += 1;
                        }
                    }
                }
                _ => {
                    debug!(
                        "Unknown materials3 sub-chunk: 0x{:08X}, size: {}",
                        chunk_type, chunk_size
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        debug!(
            "Loaded {} complete materials from W3D_CHUNK_MATERIALS3",
            material_count
        );
        Ok(())
    }

    /// Parse a complete W3D_CHUNK_MATERIAL3 exactly like C++ does:
    /// 1. Read W3D_CHUNK_MATERIAL3_NAME
    /// 2. Read W3D_CHUNK_MATERIAL3_INFO (material properties)
    /// 3. Read W3D_CHUNK_MATERIAL3_DC_MAP -> W3D_CHUNK_MAP3_FILENAME (texture)
    fn parse_complete_material3_chunk(&self, data: &[u8]) -> Result<W3DMaterial> {
        let mut offset = 0;
        let mut material = W3DMaterial::default();
        let mut material_name: Option<String> = None;

        // Parse chunks inside W3D_CHUNK_MATERIAL3 container like C++ does
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3_NAME => {
                    // 0x0000002D
                    // Read material name exactly like C++: cload.Read(name,cload.Cur_Chunk_Length());
                    let mut name = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            name.push(byte as char);
                        }
                    }

                    if !name.is_empty() {
                        material_name = Some(name);
                        debug!("Found material3 name: {}", material_name.as_ref().unwrap());
                    }
                }
                W3D_CHUNK_MATERIAL3_INFO => {
                    // 0x0000002E
                    // Read W3dMaterial3Struct like C++: cload.Read(&mat,sizeof(W3dMaterial3Struct))
                    debug!("Parsing W3D_CHUNK_MATERIAL3_INFO, size: {}", chunk_size);
                    // For now, set basic material properties - we can expand this later
                    material.diffuse_color = Vec3::new(0.8, 0.8, 0.8);
                    material.specular_color = Vec3::new(0.2, 0.2, 0.2);
                    material.shininess = 16.0;
                    material.opacity = 1.0;
                }
                W3D_CHUNK_MATERIAL3_DC_MAP => {
                    // 0x0000002F - Diffuse Color Map
                    debug!(
                        "Found W3D_CHUNK_MATERIAL3_DC_MAP, extracting texture filename like C++"
                    );
                    let _is_container_chunk = (chunk_type & 0x80000000) != 0 || chunk_size > 256; // DC_MAP is a container

                    if let Ok(texture_filename) = self.parse_material3_dc_map_chunk(chunk_data) {
                        debug!(
                            "C++ style: Found texture filename from DC_MAP: {}",
                            texture_filename
                        );
                        material.texture_name = Some(texture_filename);
                        material.stage0_mapping.texture_name = material.texture_name.clone();
                    }
                }
                _ => {
                    debug!("Unknown material3 sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        // Set material name like C++: vmat->Set_Name(name);
        if let Some(name) = material_name {
            material.name = name;
        } else {
            material.name = "unnamed_material3".to_string();
        }

        // Set C++ compatible material properties
        material.stage0_mapping.uv_source = UVSource::UV0;
        material.stage0_mapping.blend_mode = TextureBlendMode::Modulate;
        material.blend_mode = BlendMode::Opaque;

        debug!(
            "Completed material3 parsing: '{}' with texture: {:?}",
            material.name, material.texture_name
        );

        Ok(material)
    }

    /// Parse a single W3D_CHUNK_MATERIAL3 and extract texture filenames from DC_MAP chunks
    fn parse_single_material3_chunk(&self, data: &[u8]) -> Result<Vec<String>> {
        let mut offset = 0;
        let mut texture_names: Vec<String> = Vec::new();

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            let is_container_chunk = (chunk_type & 0x80000000) != 0;
            let chunk_type = chunk_type & 0x7FFFFFFF;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3_DC_MAP => {
                    debug!("Found W3D_CHUNK_MATERIAL3_DC_MAP, extracting texture filename");
                    if is_container_chunk {
                        // Parse the DC_MAP container to find the filename
                        if let Ok(filename) = self.parse_material3_dc_map_chunk(chunk_data) {
                            debug!("Found texture filename from material3 DC_MAP: {}", filename);
                            texture_names.push(filename);
                        }
                    }
                }
                _ => {
                    debug!("Unknown material3 sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        Ok(texture_names)
    }

    /// Parse W3D_CHUNK_MATERIAL3_DC_MAP to extract texture filename
    fn parse_material3_dc_map_chunk(&self, data: &[u8]) -> Result<String> {
        let mut offset = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MAP3_FILENAME => {
                    // 0x00000030
                    // Read null-terminated string directly from chunk data
                    let mut filename = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            filename.push(byte as char);
                        }
                    }

                    if !filename.is_empty() {
                        debug!(
                            "Found texture filename in W3D_CHUNK_MAP3_FILENAME: {}",
                            filename
                        );
                        return Ok(filename);
                    }
                }
                _ => {
                    debug!("Unknown DC_MAP sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        Err(anyhow!(
            "No texture filename found in W3D_CHUNK_MATERIAL3_DC_MAP"
        ))
    }

    /// Parse triangles array - C++ compatible W3dTriStruct format
    fn parse_triangles(&self, data: &[u8]) -> Result<Vec<[u32; 3]>> {
        // W3dTriStruct format: 3 x uint32 vertex indices, uint32 attributes, W3dVectorStruct normal, float32 distance
        // Total size: 3*4 + 4 + 3*4 + 4 = 32 bytes per triangle
        const TRI_STRUCT_SIZE: usize = 32;

        if !data.len().is_multiple_of(TRI_STRUCT_SIZE) {
            return Err(anyhow!(
                "Invalid triangles data size: {} (expected multiple of {})",
                data.len(),
                TRI_STRUCT_SIZE
            ));
        }

        let triangle_count = data.len() / TRI_STRUCT_SIZE;
        let mut triangles = Vec::with_capacity(triangle_count);

        for i in 0..triangle_count {
            let offset = i * TRI_STRUCT_SIZE;

            // Read the 3 vertex indices (first 12 bytes of W3dTriStruct)
            let v0 = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let v1 = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let v2 = u32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            // Skip attributes (4 bytes), normal (12 bytes), and distance (4 bytes) for now
            // We only need the vertex indices for basic rendering

            triangles.push([v0, v1, v2]);

            // Log first few triangles for debugging
            if i < 3 {
                debug!("Triangle {}: [{}, {}, {}]", i, v0, v1, v2);
            }
        }

        debug!("Successfully parsed {} triangles", triangles.len());
        Ok(triangles)
    }

    /// Build final mesh from parsed data
    fn build_mesh_from_data(
        &self,
        mesh: &mut W3DMesh,
        vertices: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        texcoords: Vec<[f32; 2]>,
        vertex_colors: Vec<[f32; 4]>,
        triangles: Vec<[u32; 3]>,
    ) -> Result<()> {
        if vertices.is_empty() {
            return Err(anyhow!("No vertices in mesh"));
        }

        let vertex_count = vertices.len();
        mesh.vertices.clear();
        mesh.vertices.reserve(vertex_count);
        mesh.indices.clear();

        // Build vertices with available data
        for i in 0..vertex_count {
            let position = w3d_position_to_world(vertices[i]);
            let normal = if i < normals.len() {
                w3d_normal_to_world(normals[i])
            } else {
                [0.0, 1.0, 0.0]
            };
            let uv = if i < texcoords.len() {
                texcoords[i]
            } else {
                [0.0, 0.0]
            };
            let color = if i < vertex_colors.len() {
                vertex_colors[i]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };

            mesh.vertices.push(W3DVertex {
                position,
                normal,
                uv,
                color,
            });
        }
        mesh.vertices_in_render_space = true;
        mesh.has_explicit_vertex_colors = !vertex_colors.is_empty();

        // Build indices from triangles
        for triangle in triangles {
            if triangle[0] < vertex_count as u32
                && triangle[1] < vertex_count as u32
                && triangle[2] < vertex_count as u32
            {
                push_world_space_triangle(&mut mesh.indices, triangle[0], triangle[1], triangle[2]);
            }
        }

        // C++ parity: never synthesize triangle lists when triangle chunks are missing/invalid.
        if mesh.indices.is_empty() {
            return Err(anyhow!("mesh '{}' has no valid triangles", mesh.name));
        }

        debug!(
            "Built mesh with {} vertices and {} indices",
            mesh.vertices.len(),
            mesh.indices.len()
        );
        Ok(())
    }

    /// Load C&C model by exact asset name.
    pub async fn load_cnc_model(
        &self,
        archive_system: &mut ArchiveFileSystem,
        unit_name: &str,
    ) -> Result<W3DModel> {
        self.load_model(archive_system, unit_name).await
    }

    /// List available W3D models in archives
    pub fn list_available_models(&self, archive_system: &ArchiveFileSystem) -> Vec<String> {
        let mut models = Vec::new();
        let all_files = archive_system.list_all_files();

        for file in all_files {
            if file.to_lowercase().ends_with(".w3d") {
                models.push(file);
            }
        }

        models.sort();
        models
    }
}

/// Mesh header structure
#[derive(Debug)]
struct MeshHeader {
    pub version: u32,
    pub flags: u32,
    pub num_triangles: u32,
    pub num_vertices: u32,
    pub mesh_name: String,
    pub container_name: String,
}

/// Get common C&C unit models - updated with actual units found in archives
pub fn get_common_cnc_units() -> Vec<&'static str> {
    vec![
        // USA Units
        "humvee",   // avhummer - Confirmed exists
        "crusader", // avcrusader - Confirmed exists
        "chinook",  // avchinook - Confirmed exists
        "comanche", // avcomanche - Attack helicopter
        "abrams",   // Maps to crusader (main US tank)
        // China Units
        "mig",          // nvmign - Confirmed exists
        "helix",        // nvhelix - Confirmed exists
        "gattling",     // nvgatttank - Confirmed exists
        "battlemaster", // Chinese main battle tank
        "dragon",       // Dragon tank
        // GLA Units
        "scorpion",  // uvscorpion - Confirmed exists
        "toxin",     // uvtoxintrk - Confirmed exists
        "scud",      // SCUD launcher
        "technical", // Technical truck
        "marauder",  // GLA tank
        // Test units with confirmed models
        "test_tank",    // Uses uvscorpion
        "test_vehicle", // Uses avhummer
        "test_air",     // Uses nvhelix
    ]
}

fn deduplicate_stage_uv_layers(layers: Vec<Vec<[f32; 2]>>) -> (Vec<Vec<[f32; 2]>>, Vec<u8>) {
    const MAX_CHANNELS: usize = 4;
    let mut unique_layers: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut stage_channels: Vec<u8> = Vec::new();
    let mut crc_map = HashMap::new();

    for coords in layers {
        if coords.is_empty() {
            if unique_layers.is_empty() {
                unique_layers.push(Vec::new());
            }
            stage_channels.push(0);
            continue;
        }

        let mut hasher = Hasher::new();
        hasher.update(bytemuck::cast_slice(&coords));
        let crc = hasher.finalize();

        let channel = if let Some(&existing) = crc_map.get(&crc) {
            existing
        } else {
            let assigned = if unique_layers.len() < MAX_CHANNELS {
                let ch = unique_layers.len() as u8;
                unique_layers.push(coords.clone());
                ch
            } else {
                (MAX_CHANNELS.saturating_sub(1)) as u8
            };
            crc_map.insert(crc, assigned);
            assigned
        };

        stage_channels.push(channel);
    }

    if unique_layers.is_empty() {
        unique_layers.push(Vec::new());
    }

    (unique_layers, stage_channels)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn chunk(chunk_type: u32, payload: Vec<u8>, container: bool) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + payload.len());
        out.extend_from_slice(&chunk_type.to_le_bytes());
        let raw_size = (payload.len() as u32) | if container { 0x8000_0000 } else { 0 };
        out.extend_from_slice(&raw_size.to_le_bytes());
        out.extend_from_slice(&payload);
        out
    }

    fn fixed_name(name: &str, len: usize) -> Vec<u8> {
        let mut out = vec![0; len];
        let bytes = name.as_bytes();
        let copy_len = bytes.len().min(len);
        out[..copy_len].copy_from_slice(&bytes[..copy_len]);
        out
    }

    fn vertex_influence_payload(records: &[(u16, [u8; 6])], trailing: &[u8]) -> Vec<u8> {
        let mut payload =
            Vec::with_capacity(records.len() * W3D_VERTEX_INFLUENCE_RECORD_SIZE + trailing.len());
        for (bone_idx, pad) in records {
            payload.extend_from_slice(&bone_idx.to_le_bytes());
            payload.extend_from_slice(pad);
        }
        payload.extend_from_slice(trailing);
        payload
    }

    /// Small source-shaped mesh container for the exact C++ skin-link loader.
    /// It deliberately has three vertices because the chunk reader must use
    /// `NumVertices`, not derive a record count from the payload length.
    fn mesh_with_vertex_influence_chunks(version: u32, influence_chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut mesh_header = vec![0; 116];
        mesh_header[0..4].copy_from_slice(&version.to_le_bytes());
        mesh_header[8..24].copy_from_slice(&fixed_name("SKIN", W3D_NAME_LEN));
        mesh_header[40..44].copy_from_slice(&1u32.to_le_bytes());
        mesh_header[44..48].copy_from_slice(&3u32.to_le_bytes());

        let mut vertices = Vec::new();
        for vertex in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for value in vertex {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut triangle = Vec::with_capacity(32);
        triangle.extend_from_slice(&0u32.to_le_bytes());
        triangle.extend_from_slice(&1u32.to_le_bytes());
        triangle.extend_from_slice(&2u32.to_le_bytes());
        triangle.extend_from_slice(&[0u8; 20]);

        let mut mesh_payload = [
            chunk(W3D_CHUNK_MESH_HEADER, mesh_header, false),
            chunk(W3D_CHUNK_VERTICES, vertices, false),
            chunk(W3D_CHUNK_TRIANGLES, triangle, false),
        ]
        .concat();
        for influence_chunk in influence_chunks {
            mesh_payload.extend_from_slice(&chunk(
                W3D_CHUNK_VERTEX_INFLUENCES,
                influence_chunk.clone(),
                false,
            ));
        }
        chunk(W3D_CHUNK_MESH, mesh_payload, true)
    }

    fn pivot(name: &str, parent: u32, translation: [f32; 3]) -> Vec<u8> {
        let mut out = fixed_name(name, W3D_NAME_LEN);
        out.extend_from_slice(&parent.to_le_bytes());
        for value in translation {
            out.extend_from_slice(&value.to_le_bytes());
        }
        // Euler angles.
        out.extend_from_slice(&[0u8; 12]);
        // Identity quaternion [x, y, z, w].
        out.extend_from_slice(&0.0f32.to_le_bytes());
        out.extend_from_slice(&0.0f32.to_le_bytes());
        out.extend_from_slice(&0.0f32.to_le_bytes());
        out.extend_from_slice(&1.0f32.to_le_bytes());
        assert_eq!(out.len(), 60);
        out
    }

    fn hlod_attachment_array(chunk_type: u32, entries: &[(&str, u32)]) -> Vec<u8> {
        assert!(
            matches!(
                chunk_type,
                W3D_CHUNK_HLOD_AGGREGATE_ARRAY | W3D_CHUNK_HLOD_PROXY_ARRAY
            ),
            "synthetic attachment must be an aggregate or proxy array"
        );
        assert!(
            !entries.is_empty(),
            "C++ HLOD exporter omits empty aggregate/proxy arrays"
        );

        let mut array_header = Vec::with_capacity(8);
        array_header.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        // C++ `HLodSaveClass` writes zero here for aggregate/proxy arrays.
        // It is metadata, not an attachment-LOD threshold.
        array_header.extend_from_slice(&0.0f32.to_le_bytes());
        let mut payload = chunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, array_header, false);
        for (name, bone_index) in entries {
            let mut subobject = Vec::with_capacity(36);
            subobject.extend_from_slice(&bone_index.to_le_bytes());
            subobject.extend_from_slice(&fixed_name(name, 32));
            payload.extend_from_slice(&chunk(W3D_CHUNK_HLOD_SUB_OBJECT, subobject, false));
        }
        chunk(chunk_type, payload, true)
    }

    fn rigid_hlod_fixture(
        lod_count: usize,
        aggregate_entries: &[(&str, u32)],
        proxy_entries: &[(&str, u32)],
    ) -> Vec<u8> {
        let mut hierarchy_header = Vec::with_capacity(36);
        hierarchy_header.extend_from_slice(&W3D_CURRENT_HTREE_VERSION.to_le_bytes());
        hierarchy_header.extend_from_slice(&fixed_name("RIG_HIER", W3D_NAME_LEN));
        hierarchy_header.extend_from_slice(&2u32.to_le_bytes());
        hierarchy_header.extend_from_slice(&[0u8; 12]);
        let mut pivots = pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]);
        // Deliberately does not match the mesh name.  HLOD BoneIndex, not a
        // pivot-name heuristic, must produce this child transform.
        pivots.extend_from_slice(&pivot("AUTHORED_BONE", 0, [10.0, 20.0, 30.0]));
        let hierarchy = chunk(
            W3D_CHUNK_HIERARCHY,
            [
                chunk(W3D_CHUNK_HIERARCHY_HEADER, hierarchy_header, false),
                chunk(W3D_CHUNK_PIVOTS, pivots, false),
            ]
            .concat(),
            true,
        );

        let mut mesh_header = vec![0; 116];
        mesh_header[0..4].copy_from_slice(&1u32.to_le_bytes());
        mesh_header[8..24].copy_from_slice(&fixed_name("RIGID", W3D_NAME_LEN));
        mesh_header[24..40].copy_from_slice(&fixed_name("HLODROOT", W3D_NAME_LEN));
        mesh_header[40..44].copy_from_slice(&1u32.to_le_bytes());
        mesh_header[44..48].copy_from_slice(&3u32.to_le_bytes());
        let mut vertices = Vec::new();
        for vertex in [[1.0f32, 2.0, 3.0], [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for value in vertex {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut normals = Vec::new();
        for _ in 0..3 {
            normals.extend_from_slice(&0.0f32.to_le_bytes());
            normals.extend_from_slice(&0.0f32.to_le_bytes());
            normals.extend_from_slice(&1.0f32.to_le_bytes());
        }
        let mut triangle = Vec::with_capacity(32);
        triangle.extend_from_slice(&0u32.to_le_bytes());
        triangle.extend_from_slice(&1u32.to_le_bytes());
        triangle.extend_from_slice(&2u32.to_le_bytes());
        triangle.extend_from_slice(&[0u8; 20]);
        let mesh = chunk(
            W3D_CHUNK_MESH,
            [
                chunk(W3D_CHUNK_MESH_HEADER, mesh_header, false),
                chunk(W3D_CHUNK_VERTICES, vertices, false),
                chunk(W3D_CHUNK_VERTEX_NORMALS, normals, false),
                chunk(W3D_CHUNK_TRIANGLES, triangle, false),
            ]
            .concat(),
            true,
        );

        let mut hlod_header = Vec::with_capacity(40);
        hlod_header.extend_from_slice(&0x0001_0000u32.to_le_bytes());
        hlod_header.extend_from_slice(&(lod_count as u32).to_le_bytes());
        hlod_header.extend_from_slice(&fixed_name("HLODROOT", W3D_NAME_LEN));
        hlod_header.extend_from_slice(&fixed_name("RIG_HIER", W3D_NAME_LEN));
        let mut hlod_payload = chunk(W3D_CHUNK_HLOD_HEADER, hlod_header, false);
        for _ in 0..lod_count {
            let mut array_header = Vec::with_capacity(8);
            array_header.extend_from_slice(&1u32.to_le_bytes());
            array_header.extend_from_slice(&f32::MAX.to_le_bytes());
            let mut subobject = Vec::with_capacity(36);
            subobject.extend_from_slice(&1u32.to_le_bytes());
            subobject.extend_from_slice(&fixed_name("HLODROOT.RIGID", 32));
            let lod_payload = [
                chunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, array_header, false),
                chunk(W3D_CHUNK_HLOD_SUB_OBJECT, subobject, false),
            ]
            .concat();
            hlod_payload.extend_from_slice(&chunk(W3D_CHUNK_HLOD_LOD_ARRAY, lod_payload, true));
        }
        if !aggregate_entries.is_empty() {
            hlod_payload.extend_from_slice(&hlod_attachment_array(
                W3D_CHUNK_HLOD_AGGREGATE_ARRAY,
                aggregate_entries,
            ));
        }
        if !proxy_entries.is_empty() {
            hlod_payload.extend_from_slice(&hlod_attachment_array(
                W3D_CHUNK_HLOD_PROXY_ARRAY,
                proxy_entries,
            ));
        }

        [hierarchy, mesh, chunk(W3D_CHUNK_HLOD, hlod_payload, true)].concat()
    }

    /// Build one source-shaped C++ `HModelDefClass` container. The HMODEL
    /// structs use the original MSVC `sizeof` payloads (40-byte header,
    /// 18-byte connection) because `hmdldef.cpp` reads them directly.
    fn hmodel_fixture_chunk(
        version: u32,
        model_name: &str,
        hierarchy_name: &str,
        nodes: &[(u32, &str, u16)],
    ) -> Vec<u8> {
        hmodel_fixture_chunk_with_trailing(version, model_name, hierarchy_name, nodes, &[])
    }

    fn hmodel_fixture_chunk_with_trailing(
        version: u32,
        model_name: &str,
        hierarchy_name: &str,
        nodes: &[(u32, &str, u16)],
        trailing_chunks: &[(u32, Vec<u8>)],
    ) -> Vec<u8> {
        let mut header = Vec::with_capacity(40);
        header.extend_from_slice(&version.to_le_bytes());
        header.extend_from_slice(&fixed_name(model_name, W3D_NAME_LEN));
        header.extend_from_slice(&fixed_name(hierarchy_name, W3D_NAME_LEN));
        header.extend_from_slice(&(nodes.len() as u16).to_le_bytes());
        header.extend_from_slice(&[0u8; 2]);
        assert_eq!(header.len(), 40);

        let mut payload = chunk(W3D_CHUNK_HMODEL_HEADER, header, false);
        for (chunk_type, leaf_name, pivot_index) in nodes {
            assert!(
                matches!(
                    *chunk_type,
                    W3D_CHUNK_HMODEL_NODE
                        | W3D_CHUNK_HMODEL_COLLISION_NODE
                        | W3D_CHUNK_HMODEL_SKIN_NODE
                ),
                "fixture may only create HMODEL connection chunks"
            );
            let mut node = fixed_name(leaf_name, W3D_NAME_LEN);
            node.extend_from_slice(&pivot_index.to_le_bytes());
            assert_eq!(node.len(), 18);
            payload.extend_from_slice(&chunk(*chunk_type, node, false));
        }
        for (chunk_type, trailing_payload) in trailing_chunks {
            payload.extend_from_slice(&chunk(*chunk_type, trailing_payload.clone(), false));
        }
        chunk(W3D_CHUNK_HMODEL, payload, true)
    }

    fn hmodel_snap_points_payload(points: &[[f32; 3]], trailing: &[u8]) -> Vec<u8> {
        let mut payload = Vec::with_capacity(points.len() * 12 + trailing.len());
        for point in points {
            for component in point {
                payload.extend_from_slice(&component.to_le_bytes());
            }
        }
        payload.extend_from_slice(trailing);
        payload
    }

    fn rigid_hmodel_fixture(version: u32, nodes: &[(u32, &str, u16)]) -> Vec<u8> {
        [
            rigid_hlod_fixture(1, &[], &[]),
            hmodel_fixture_chunk(version, "RIG_HMODEL", "RIG_HIER", nodes),
        ]
        .concat()
    }

    fn visibility_hlod_fixture() -> Vec<u8> {
        let mut hierarchy_header = Vec::with_capacity(36);
        hierarchy_header.extend_from_slice(&W3D_CURRENT_HTREE_VERSION.to_le_bytes());
        hierarchy_header.extend_from_slice(&fixed_name("VIS_HIER", W3D_NAME_LEN));
        hierarchy_header.extend_from_slice(&2u32.to_le_bytes());
        hierarchy_header.extend_from_slice(&[0u8; 12]);
        let mut pivots = pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]);
        pivots.extend_from_slice(&pivot("NOT_A_MESH_NAME", 0, [3.0, 4.0, 5.0]));
        let hierarchy = chunk(
            W3D_CHUNK_HIERARCHY,
            [
                chunk(W3D_CHUNK_HIERARCHY_HEADER, hierarchy_header, false),
                chunk(W3D_CHUNK_PIVOTS, pivots, false),
            ]
            .concat(),
            true,
        );

        let mut mesh_header = vec![0; 116];
        mesh_header[0..4].copy_from_slice(&1u32.to_le_bytes());
        mesh_header[8..24].copy_from_slice(&fixed_name("body_d", W3D_NAME_LEN));
        mesh_header[24..40].copy_from_slice(&fixed_name("VIS_HLOD", W3D_NAME_LEN));
        mesh_header[40..44].copy_from_slice(&1u32.to_le_bytes());
        mesh_header[44..48].copy_from_slice(&3u32.to_le_bytes());
        let mut vertices = Vec::new();
        for vertex in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for value in vertex {
                vertices.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut normals = Vec::new();
        for _ in 0..3 {
            normals.extend_from_slice(&0.0f32.to_le_bytes());
            normals.extend_from_slice(&0.0f32.to_le_bytes());
            normals.extend_from_slice(&1.0f32.to_le_bytes());
        }
        let mut triangle = Vec::with_capacity(32);
        triangle.extend_from_slice(&0u32.to_le_bytes());
        triangle.extend_from_slice(&1u32.to_le_bytes());
        triangle.extend_from_slice(&2u32.to_le_bytes());
        triangle.extend_from_slice(&[0u8; 20]);
        let mesh = chunk(
            W3D_CHUNK_MESH,
            [
                chunk(W3D_CHUNK_MESH_HEADER, mesh_header, false),
                chunk(W3D_CHUNK_VERTICES, vertices, false),
                chunk(W3D_CHUNK_VERTEX_NORMALS, normals, false),
                chunk(W3D_CHUNK_TRIANGLES, triangle, false),
            ]
            .concat(),
            true,
        );

        let mut animation_header = Vec::with_capacity(44);
        animation_header.extend_from_slice(&W3D_CURRENT_HANIM_VERSION.to_le_bytes());
        animation_header.extend_from_slice(&fixed_name("VIS_CLIP", W3D_NAME_LEN));
        animation_header.extend_from_slice(&fixed_name("VIS_HIER", W3D_NAME_LEN));
        animation_header.extend_from_slice(&4u32.to_le_bytes());
        animation_header.extend_from_slice(&30u32.to_le_bytes());
        // first=2, last=4, flags=BIT_CHANNEL_VIS, pivot=1, default=visible;
        // bits 0b0000_0101 make frames [2, 3, 4] => [true, false, true].
        let mut raw_bit_channel = Vec::new();
        raw_bit_channel.extend_from_slice(&2u16.to_le_bytes());
        raw_bit_channel.extend_from_slice(&4u16.to_le_bytes());
        raw_bit_channel.extend_from_slice(&0u16.to_le_bytes());
        raw_bit_channel.extend_from_slice(&1u16.to_le_bytes());
        raw_bit_channel.push(1);
        raw_bit_channel.push(0b0000_0101);
        let animation = chunk(
            W3D_CHUNK_ANIMATION,
            [
                chunk(W3D_CHUNK_ANIMATION_HEADER, animation_header, false),
                chunk(W3D_CHUNK_BIT_CHANNEL, raw_bit_channel, false),
            ]
            .concat(),
            true,
        );

        let mut hlod_header = Vec::with_capacity(40);
        hlod_header.extend_from_slice(&0x0001_0000u32.to_le_bytes());
        hlod_header.extend_from_slice(&1u32.to_le_bytes());
        hlod_header.extend_from_slice(&fixed_name("VIS_HLOD", W3D_NAME_LEN));
        hlod_header.extend_from_slice(&fixed_name("VIS_HIER", W3D_NAME_LEN));
        let mut array_header = Vec::with_capacity(8);
        array_header.extend_from_slice(&1u32.to_le_bytes());
        array_header.extend_from_slice(&f32::MAX.to_le_bytes());
        let mut subobject = Vec::with_capacity(36);
        subobject.extend_from_slice(&1u32.to_le_bytes());
        subobject.extend_from_slice(&fixed_name("VIS_HLOD.body_d", 32));
        let lod_payload = [
            chunk(W3D_CHUNK_HLOD_SUB_OBJECT_ARRAY_HEADER, array_header, false),
            chunk(W3D_CHUNK_HLOD_SUB_OBJECT, subobject, false),
        ]
        .concat();
        let hlod = chunk(
            W3D_CHUNK_HLOD,
            [
                chunk(W3D_CHUNK_HLOD_HEADER, hlod_header, false),
                chunk(W3D_CHUNK_HLOD_LOD_ARRAY, lod_payload, true),
            ]
            .concat(),
            true,
        );

        [hierarchy, mesh, animation, hlod].concat()
    }

    /// A source-shaped single-HLOD fixture with a parent, a descendant, and a
    /// sibling.  Mesh and pivot names intentionally differ; only retained
    /// `HLOD.Name.Child -> BoneIndex` records are legal visibility bindings.
    fn hide_show_subobjects_hlod_model() -> W3DModel {
        let mut model = W3DModel::new("hide_show_subobjects".to_string());
        model.hierarchy = Some(W3dHierarchy {
            name: "VIS_HIER".to_string(),
            pivots: vec![
                W3dPivot {
                    name: "ROOT".to_string(),
                    parent_idx: u32::MAX,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
                W3dPivot {
                    name: "PARENT_BONE".to_string(),
                    parent_idx: 0,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
                W3dPivot {
                    name: "CHILD_BONE".to_string(),
                    parent_idx: 1,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
                W3dPivot {
                    name: "SIBLING_BONE".to_string(),
                    parent_idx: 0,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
            ],
            pivot_fixups: Vec::new(),
        });
        model.hlods.push(W3dHlod {
            version: 0x0001_0000,
            name: "VIS_HLOD".to_string(),
            hierarchy_name: "VIS_HIER".to_string(),
            lods: vec![W3dHlodLod {
                max_screen_size: f32::MAX,
                subobjects: vec![
                    W3dHlodSubObject {
                        name: "VIS_HLOD.ParentMesh".to_string(),
                        bone_index: 1,
                    },
                    W3dHlodSubObject {
                        // C++ only hides the directly named RenderObj on a
                        // bone, not another direct sibling sharing that bone.
                        name: "VIS_HLOD.SameBoneMesh".to_string(),
                        bone_index: 1,
                    },
                    W3dHlodSubObject {
                        name: "VIS_HLOD.ChildMesh".to_string(),
                        bone_index: 2,
                    },
                    W3dHlodSubObject {
                        name: "VIS_HLOD.SiblingMesh".to_string(),
                        bone_index: 3,
                    },
                ],
            }],
            aggregates: None,
            proxies: None,
            has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        });
        model.meshes = ["ParentMesh", "SameBoneMesh", "ChildMesh", "SiblingMesh"]
            .into_iter()
            .map(|name| {
                let mut mesh = W3DMesh::new(name.to_string());
                mesh.container_name = "VIS_HLOD".to_string();
                mesh
            })
            .collect();
        model
    }

    /// One exact supported HLOD tree for primary-turret control tests. Mesh
    /// names deliberately differ from pivot names: only the source HLOD child
    /// records and exact source `Turret`/`TurretPitch` pivot names may bind.
    fn primary_turret_hlod_model() -> W3DModel {
        fn test_pivot(name: &str, parent_idx: u32, translation: [f32; 3]) -> W3dPivot {
            W3dPivot {
                name: name.to_string(),
                parent_idx,
                translation,
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            }
        }

        let mut model = W3DModel::new("primary_turret_hlod".to_string());
        model.hierarchy = Some(W3dHierarchy {
            name: "TURRET_HIER".to_string(),
            pivots: vec![
                test_pivot("ROOT", u32::MAX, [0.0, 0.0, 0.0]),
                test_pivot("HULL_PIVOT", 0, [-4.0, 0.0, 0.0]),
                test_pivot("YAW_PIVOT", 0, [0.0, 10.0, 0.0]),
                test_pivot("PITCH_PIVOT", 2, [5.0, 0.0, 0.0]),
                test_pivot("MUZZLE_PIVOT", 3, [3.0, 0.0, 0.0]),
            ],
            pivot_fixups: Vec::new(),
        });
        model.hlods.push(W3dHlod {
            version: 0x0001_0000,
            name: "TURRET_HLOD".to_string(),
            hierarchy_name: "TURRET_HIER".to_string(),
            lods: vec![W3dHlodLod {
                max_screen_size: f32::MAX,
                subobjects: vec![
                    W3dHlodSubObject {
                        name: "TURRET_HLOD.ChassisMesh".to_string(),
                        bone_index: 1,
                    },
                    W3dHlodSubObject {
                        name: "TURRET_HLOD.GunHousingMesh".to_string(),
                        bone_index: 2,
                    },
                    W3dHlodSubObject {
                        name: "TURRET_HLOD.BarrelMesh".to_string(),
                        bone_index: 3,
                    },
                    W3dHlodSubObject {
                        name: "TURRET_HLOD.FlashMesh".to_string(),
                        bone_index: 4,
                    },
                ],
            }],
            aggregates: None,
            proxies: None,
            has_unrendered_aggregates: false,
            has_invalid_trailing_records: false,
        });
        model.meshes = ["ChassisMesh", "GunHousingMesh", "BarrelMesh", "FlashMesh"]
            .into_iter()
            .map(|name| {
                let mut mesh = W3DMesh::new(name.to_string());
                mesh.container_name = "TURRET_HLOD".to_string();
                mesh
            })
            .collect();
        // A selected source HAnim changes YAW_PIVOT's X translation at frame
        // one. The turret control must apply to that sampled pose and then
        // propagate through PITCH_PIVOT and MUZZLE_PIVOT, not to a separate
        // global hull matrix.
        model.animations.push(W3dAnimation {
            name: "TURRET_POSE".to_string(),
            hierarchy_name: "TURRET_HIER".to_string(),
            num_frames: 2,
            frame_rate: 30,
            source_is_compressed: false,
            channels: vec![W3dAnimChannel {
                first_frame: 0,
                last_frame: 1,
                vector_len: 1,
                flags: 0,
                pivot: 2,
                data: vec![0.0, 2.0],
            }],
            raw_visibility_channels: Vec::new(),
            unsupported_visibility_pivots: Vec::new(),
        });
        model
    }

    /// The exact turret fixture with one external C++ `AdditionalModels`
    /// record attached to the recoil/barrel pivot. It lets the aggregate
    /// projection prove that parent mesh and external child share one HTree
    /// control sequence without involving a renderer or asset lookup.
    fn primary_turret_hlod_model_with_barrel_aggregate() -> W3DModel {
        let mut model = primary_turret_hlod_model();
        model.hlods[0].aggregates = Some(W3dHlodAttachmentArray {
            max_screen_size: 0.0,
            subobjects: vec![W3dHlodSubObject {
                name: "EXTERNAL_BARREL_ATTACHMENT".to_string(),
                bone_index: 3,
            }],
        });
        model.hlods[0].has_unrendered_aggregates = true;
        model
    }

    /// An animation-only companion W3D, shaped like the raw HAnim files C++
    /// loads on a `Get_HAnim(Hierarchy.Animation)` miss.
    fn companion_animation_fixture(
        hierarchy_name: &str,
        animation_name: &str,
        first_x: f32,
        last_x: f32,
    ) -> Vec<u8> {
        let mut animation_header = Vec::with_capacity(44);
        animation_header.extend_from_slice(&W3D_CURRENT_HANIM_VERSION.to_le_bytes());
        animation_header.extend_from_slice(&fixed_name(animation_name, W3D_NAME_LEN));
        animation_header.extend_from_slice(&fixed_name(hierarchy_name, W3D_NAME_LEN));
        animation_header.extend_from_slice(&2u32.to_le_bytes());
        animation_header.extend_from_slice(&30u32.to_le_bytes());

        // X translation, pivot 1, frames [0, 1]. The companion contains no
        // geometry or hierarchy chunk; the geometry W3D supplies the HTree.
        let mut channel = Vec::with_capacity(20);
        channel.extend_from_slice(&0u16.to_le_bytes());
        channel.extend_from_slice(&1u16.to_le_bytes());
        channel.extend_from_slice(&1u16.to_le_bytes());
        channel.extend_from_slice(&0u16.to_le_bytes());
        channel.extend_from_slice(&1u16.to_le_bytes());
        channel.extend_from_slice(&0u16.to_le_bytes());
        channel.extend_from_slice(&first_x.to_le_bytes());
        channel.extend_from_slice(&last_x.to_le_bytes());
        chunk(
            W3D_CHUNK_ANIMATION,
            [
                chunk(W3D_CHUNK_ANIMATION_HEADER, animation_header, false),
                chunk(W3D_CHUNK_ANIMATION_CHANNEL, channel, false),
            ]
            .concat(),
            true,
        )
    }

    #[test]
    fn deduplicate_stage_uv_layers_merges_duplicate_channels() {
        let stage0 = vec![[0.0, 0.0], [1.0, 0.0]];
        let stage1 = stage0.clone();
        let stage2 = vec![[0.5, 0.5], [0.75, 0.75]];
        let layers = vec![stage0.clone(), stage1, stage2.clone()];
        let (unique_layers, stage_channels) = deduplicate_stage_uv_layers(layers);

        assert_eq!(unique_layers.len(), 2);
        assert_eq!(unique_layers[0], stage0);
        assert_eq!(unique_layers[1], stage2);
        assert_eq!(stage_channels, vec![0, 0, 1]);
    }

    #[test]
    fn apply_material_stage_mappings_sets_texture_and_uv_source() {
        let mut material = W3DMaterial::default();
        let mut mesh = W3DMesh::new("TestMesh".to_string());
        mesh.stage_uv_channels = vec![0, 2];
        mesh.per_pass_stage_texture_names = vec![vec![
            vec!["base.dds".to_string()],
            vec!["detail.dds".to_string()],
        ]];

        W3DLoader::apply_material_stage_mappings(&mut material, &mesh);

        assert_eq!(
            material.stage0_mapping.texture_name.as_deref(),
            Some("base.dds")
        );
        assert!(matches!(material.stage0_mapping.uv_source, UVSource::UV0));
        let stage1 = material
            .stage1_mapping
            .as_ref()
            .expect("stage 1 mapping missing");
        assert_eq!(stage1.texture_name.as_deref(), Some("detail.dds"));
        assert!(matches!(stage1.uv_source, UVSource::UV2));
    }

    #[test]
    fn w3d_archive_path_variants_include_retail_art_w3d_casing() {
        for name in ["AmericaCommandCenter", "airanger_s"] {
            let paths = w3d_archive_path_variants(name);
            assert!(
                paths.iter().any(|p| p.contains("Art/W3D/")),
                "{name} must include Art/W3D/: {paths:?}"
            );
            assert!(
                paths.iter().any(|p| p.ends_with(".W3D")),
                "{name} must include .W3D: {paths:?}"
            );
        }

        let cmd = w3d_archive_path_variants("AmericaCommandCenter");
        assert_eq!(cmd[0], "art/w3d/AmericaCommandCenter.w3d");
        assert_eq!(cmd[1], "AmericaCommandCenter.w3d");
        assert!(
            cmd.iter().any(|p| p == "Art/W3D/ABBtCmdHQ.W3D"),
            "AmericaCommandCenter must include Art/W3D/ABBtCmdHQ.W3D: {cmd:?}"
        );
        assert!(
            cmd.iter().any(|p| p.contains("ABBtCmdHQ")),
            "AmericaCommandCenter must include retail ABBtCmdHQ: {cmd:?}"
        );

        let ranger = w3d_archive_path_variants("airanger_s");
        assert_eq!(ranger[0], "art/w3d/airanger_s.w3d");
        assert_eq!(ranger[1], "airanger_s.w3d");
        assert!(
            ranger.iter().any(|p| p == "Art/W3D/AIRanger_S.W3D"),
            "airanger_s must include Art/W3D/AIRanger_S.W3D: {ranger:?}"
        );
        assert!(
            ranger.iter().any(|p| p.contains("AIRanger_S")),
            "airanger_s must include retail AIRanger_S: {ranger:?}"
        );
    }

    #[test]
    fn load_model_from_bytes_rejects_empty() {
        let loader = W3DLoader::new();
        assert!(loader.load_model_from_bytes(&[], "empty").is_err());
        assert!(loader
            .load_model_from_bytes(&[], "AmericaCommandCenter")
            .is_err());
    }

    #[test]
    fn mesh_vertex_influences_retain_exact_eight_byte_records_and_ignore_trailing_data() {
        let records = [
            (2u16, [1, 2, 3, 4, 5, 6]),
            (0u16, [7, 8, 9, 10, 11, 12]),
            (u16::MAX, [13, 14, 15, 16, 17, 18]),
        ];
        let bytes = mesh_with_vertex_influence_chunks(
            0x0004_0002,
            &[vertex_influence_payload(&records, &[0xAA, 0xBB, 0xCC])],
        );
        let model = W3DLoader::new()
            .load_model_from_bytes(&bytes, "exact_vertex_influences")
            .expect("a complete W3dVertInfStruct array with trailing bytes is source-valid");
        let influences = model.meshes[0]
            .vertex_influences
            .as_ref()
            .expect("the source SKIN chunk must survive parsing");

        assert_eq!(influences.len(), 3);
        assert_eq!(influences[0].bone_idx, 2);
        assert_eq!(influences[0].pad, [1, 2, 3, 4, 5, 6]);
        assert_eq!(influences[1].bone_idx, 0);
        assert_eq!(influences[1].pad, [7, 8, 9, 10, 11, 12]);
        assert_eq!(influences[2].bone_idx, u16::MAX);
        assert_eq!(influences[2].pad, [13, 14, 15, 16, 17, 18]);
    }

    #[test]
    fn mesh_vertex_influences_use_last_complete_chunk_and_only_pre3_indices_wrap_forward() {
        let first = vertex_influence_payload(&[(9, [1; 6]), (9, [2; 6]), (9, [3; 6])], &[]);
        let last = vertex_influence_payload(&[(0, [4; 6]), (1, [5; 6]), (u16::MAX, [6; 6])], &[]);
        let repeated = W3DLoader::new()
            .load_model_from_bytes(
                &mesh_with_vertex_influence_chunks(0x0004_0002, &[first, last]),
                "repeated_vertex_influences",
            )
            .expect("each complete source influence chunk can overwrite the prior link array");
        let repeated = repeated.meshes[0]
            .vertex_influences
            .as_ref()
            .expect("the final complete chunk remains");
        assert_eq!(
            repeated
                .iter()
                .map(|influence| influence.bone_idx)
                .collect::<Vec<_>>(),
            vec![0, 1, u16::MAX],
            "modern W3D indices must stay exactly as written"
        );
        assert_eq!(repeated[0].pad, [4; 6]);
        assert_eq!(repeated[2].pad, [6; 6]);

        let legacy_records = [(0, [7; 6]), (1, [8; 6]), (u16::MAX, [9; 6])];
        let pre3 = W3DLoader::new()
            .load_model_from_bytes(
                &mesh_with_vertex_influence_chunks(
                    W3D_HTREE_ROOT_VERSION - 1,
                    &[vertex_influence_payload(&legacy_records, &[])],
                ),
                "pre3_vertex_influences",
            )
            .expect("a complete pre-3.0 source influence chunk parses");
        let pre3 = pre3.meshes[0]
            .vertex_influences
            .as_ref()
            .expect("pre-3.0 links are retained after C++ root fixup");
        assert_eq!(
            pre3.iter()
                .map(|influence| influence.bone_idx)
                .collect::<Vec<_>>(),
            vec![1, 2, 0],
            "the C++ uint16 root insertion fixup wraps after a successful load"
        );
        assert_eq!(pre3[0].pad, [7; 6]);
        assert_eq!(pre3[2].pad, [9; 6]);
    }

    #[test]
    fn mesh_vertex_influences_short_chunk_invalidates_the_mesh_without_partial_skin_data() {
        let only_two_records =
            vertex_influence_payload(&[(0, [0; 6]), (1, [0; 6])], &[0xFE, 0xED, 0xFA, 0xCE]);
        assert!(
            W3DLoader::new()
                .load_model_from_bytes(
                    &mesh_with_vertex_influence_chunks(0x0004_0002, &[only_two_records]),
                    "short_vertex_influences",
                )
                .is_err(),
            "a file whose only mesh has a short W3dVertInfStruct array must fail closed instead of preserving partial links"
        );
    }

    #[test]
    fn hlod_rigid_binding_uses_authored_bone_once_without_name_heuristics() {
        let model = W3DLoader::new()
            .load_model_from_bytes(&rigid_hlod_fixture(1, &[], &[]), "rigid_hlod")
            .expect("source-shaped HLOD fixture should parse");

        assert_eq!(model.hlods.len(), 1);
        assert_eq!(model.hlods[0].name, "HLODROOT");
        assert_eq!(model.hlods[0].hierarchy_name, "RIG_HIER");
        assert_eq!(model.hlods[0].lods.len(), 1);
        assert_eq!(model.hlods[0].lods[0].subobjects[0].name, "HLODROOT.RIGID");
        assert_eq!(model.hlods[0].lods[0].subobjects[0].bone_index, 1);
        assert_eq!(model.meshes[0].name, "RIGID");
        assert_eq!(model.meshes[0].container_name, "HLODROOT");
        assert_ne!(model.meshes[0].name, "AUTHORED_BONE");

        // Import converts source [1, 2, 3] to Main render [1, 3, 2], but it
        // must remain unbaked.  The HLOD local matrix applies source translation
        // [10, 20, 30] exactly once, becoming Main render [10, 30, 20].
        assert_eq!(model.meshes[0].vertices[0].position, [1.0, 3.0, 2.0]);
        let local = model
            .mesh_local_transform_for_animation(0, 0, 0.0)
            .expect("authored HLOD child should resolve through BoneIndex");
        let transformed =
            local.transform_point3(Vec3::from_array(model.meshes[0].vertices[0].position));
        assert!(
            (transformed - Vec3::new(11.0, 33.0, 22.0)).length() < 0.0001,
            "single HLOD transform produced {transformed:?}"
        );
    }

    #[test]
    fn hmodel_parser_retains_exact_typed_connections_and_its_named_htree_pose() {
        let model = W3DLoader::new()
            .load_model_from_bytes(
                &rigid_hmodel_fixture(
                    0x0004_0002,
                    &[
                        (W3D_CHUNK_HMODEL_NODE, "RigidBody", 1),
                        (W3D_CHUNK_HMODEL_COLLISION_NODE, "Collision", 0),
                        (W3D_CHUNK_HMODEL_SKIN_NODE, "Skin", 1),
                    ],
                ),
                "rigid_hmodel",
            )
            .expect("source-shaped HMODEL fixture should parse");

        assert_eq!(model.hmodels.len(), 1);
        assert_eq!(
            model.hierarchies.len(),
            1,
            "the named source HTree is retained"
        );
        let hmodel = &model.hmodels[0];
        assert_eq!(hmodel.name, "RIG_HMODEL");
        assert_eq!(hmodel.hierarchy_name, "RIG_HIER");
        assert!(!hmodel.has_invalid_records);
        assert_eq!(
            hmodel
                .nodes
                .iter()
                .map(|node| (node.name.as_str(), node.bone_index, node.kind))
                .collect::<Vec<_>>(),
            vec![
                ("RIG_HMODEL.RigidBody", 1, W3dHmodelNodeKind::Node),
                ("RIG_HMODEL.Collision", 0, W3dHmodelNodeKind::CollisionNode),
                ("RIG_HMODEL.Skin", 1, W3dHmodelNodeKind::SkinNode),
            ],
            "HModelDefClass::read_connection always forms <ModelName>.<RenderObjName>"
        );

        let poses = model
            .hmodel_rigid_node_poses(0)
            .expect("valid HMODEL uses only its explicitly named HTree");
        assert_eq!(poses.len(), 2, "SKIN_NODE must not enter the rigid path");
        assert_eq!(poses[0].name, "RIG_HMODEL.RigidBody");
        assert_eq!(poses[0].bone_index, 1);
        assert!(
            (poses[0].parent_transform.w_axis.truncate() - Vec3::new(10.0, 30.0, 20.0)).length()
                < 0.0001,
            "the HMODEL child uses its own source HTree pivot in render basis"
        );
        assert_eq!(poses[1].name, "RIG_HMODEL.Collision");
        assert_eq!(poses[1].parent_transform, Mat4::IDENTITY);

        let skin_palette = model
            .hmodel_bind_pose_palette(0)
            .expect("a valid HMODEL owns its named HTree bind palette");
        assert_eq!(skin_palette.len(), 2);
        assert_eq!(skin_palette[0], Mat4::IDENTITY);
        assert!(
            (skin_palette[1].w_axis.truncate() - Vec3::new(10.0, 30.0, 20.0)).length() < 0.0001,
            "SKIN_NODE must use the HMODEL's named HTree, not a whole-file palette"
        );
        assert_eq!(
            model
                .hmodel_skin_node_bindings(0)
                .expect("the valid skin connection has an HMODEL-local palette"),
            vec![W3dHmodelSkinNodeBinding {
                name: "RIG_HMODEL.Skin".to_string(),
                bone_index: 1,
            }]
        );
    }

    #[test]
    fn hmodel_pre30_pivots_normalize_exactly_like_hmdldef() {
        let model = W3DLoader::new()
            .load_model_from_bytes(
                &rigid_hmodel_fixture(
                    2 << 16,
                    &[
                        (W3D_CHUNK_HMODEL_NODE, "LegacyRoot", u16::MAX),
                        (W3D_CHUNK_HMODEL_NODE, "LegacyChild", 0),
                    ],
                ),
                "legacy_hmodel",
            )
            .expect("pre-3.0 HMODEL fixture should normalize safely");

        let hmodel = &model.hmodels[0];
        assert_eq!(hmodel.nodes[0].bone_index, 0);
        assert_eq!(hmodel.nodes[1].bone_index, 1);
        let poses = model
            .hmodel_rigid_node_poses(0)
            .expect("normalized HMODEL nodes use the corresponding HTree pivots");
        assert_eq!(poses[0].parent_transform, Mat4::IDENTITY);
        assert!(
            (poses[1].parent_transform.w_axis.truncate() - Vec3::new(10.0, 30.0, 20.0)).length()
                < 0.0001
        );
    }

    #[test]
    fn hmodel_header_names_follow_cxx_fifteen_byte_termination() {
        let bytes = [
            rigid_hlod_fixture(1, &[], &[]),
            hmodel_fixture_chunk(
                0x0004_0002,
                "0123456789ABCDEF",
                "ABCDEFGHIJKLMNOP",
                &[(W3D_CHUNK_HMODEL_NODE, "Body", 0)],
            ),
        ]
        .concat();
        let model = W3DLoader::new()
            .load_model_from_bytes(&bytes, "truncated_hmodel")
            .expect("HMODEL header remains source-shaped when its fixed fields fill 16 bytes");

        assert_eq!(model.hmodels[0].name, "0123456789ABCDE");
        assert_eq!(model.hmodels[0].hierarchy_name, "ABCDEFGHIJKLMNO");
        assert_eq!(model.hmodels[0].nodes[0].name, "0123456789ABCDE.Body");
    }

    #[test]
    fn hmodel_points_retain_source_vectors_without_consuming_connections() {
        let expected_points = [[1.25, -2.5, 3.75], [-4.0, 5.5, 6.25]];
        let bytes = [
            rigid_hlod_fixture(1, &[], &[]),
            hmodel_fixture_chunk_with_trailing(
                0x0004_0002,
                "TRAILING_HMODEL",
                "RIG_HIER",
                &[(W3D_CHUNK_HMODEL_NODE, "Body", 1)],
                &[
                    // C++ `HModelDefClass::Load_W3D` loads snap points but
                    // does not increment SubObjectCount for them.
                    (
                        W3D_CHUNK_POINTS,
                        hmodel_snap_points_payload(&expected_points, &[]),
                    ),
                    // Both old HMODEL extension records are skipped by the
                    // documented source parser and cannot become aliases.
                    (W3D_CHUNK_HMODEL_OBSOLETE_AUX_DATA, vec![1, 2, 3, 4]),
                    (W3D_CHUNK_HMODEL_OBSOLETE_SHADOW_NODE, vec![5, 6]),
                ],
            ),
        ]
        .concat();
        let model = W3DLoader::new()
            .load_model_from_bytes(&bytes, "trailing_hmodel")
            .expect("non-connection HMODEL chunks remain source-shaped metadata");

        assert_eq!(model.hmodels.len(), 1);
        assert!(!model.hmodels[0].has_invalid_records);
        assert_eq!(model.hmodels[0].nodes.len(), 1);
        assert_eq!(model.hmodels[0].nodes[0].name, "TRAILING_HMODEL.Body");
        assert_eq!(
            model
                .hmodel_source_snap_points(0)
                .expect("a valid HMODEL exposes its source definition points"),
            [
                W3dHmodelSnapPoint {
                    source_position: expected_points[0],
                },
                W3dHmodelSnapPoint {
                    source_position: expected_points[1],
                },
            ]
        );
        assert_eq!(
            model.hmodel_source_snap_point(0, 1),
            Some(W3dHmodelSnapPoint {
                source_position: expected_points[1],
            })
        );
        assert_eq!(model.hmodel_source_snap_point(0, 2), None);
    }

    #[test]
    fn hmodel_later_points_replaces_prior_points_and_ignores_remainder() {
        let first_points = [[1.0, 2.0, 3.0]];
        let replacement_points = [[-4.0, 5.0, -6.0], [7.0, -8.0, 9.0]];
        let bytes = [
            rigid_hlod_fixture(1, &[], &[]),
            hmodel_fixture_chunk_with_trailing(
                0x0004_0002,
                "REPLACED_POINTS",
                "RIG_HIER",
                &[(W3D_CHUNK_HMODEL_NODE, "Body", 1)],
                &[
                    (
                        W3D_CHUNK_POINTS,
                        hmodel_snap_points_payload(&first_points, &[]),
                    ),
                    (
                        W3D_CHUNK_POINTS,
                        hmodel_snap_points_payload(&replacement_points, &[0xDE, 0xAD, 0xBE]),
                    ),
                ],
            ),
        ]
        .concat();
        let model = W3DLoader::new()
            .load_model_from_bytes(&bytes, "replaced_hmodel_points")
            .expect("C++ source-shaped HMODEL points should parse");

        let hmodel = &model.hmodels[0];
        assert!(!hmodel.has_invalid_records);
        assert_eq!(
            hmodel.nodes.len(),
            1,
            "POINTS records cannot consume a declared HMODEL connection"
        );
        assert_eq!(
            model
                .hmodel_source_snap_points(0)
                .expect("valid HMODEL source points"),
            [
                W3dHmodelSnapPoint {
                    source_position: replacement_points[0],
                },
                W3dHmodelSnapPoint {
                    source_position: replacement_points[1],
                },
            ],
            "the later C++ SnapPointsClass allocation replaces the prior definition vector"
        );
    }

    #[test]
    fn hmodel_rejects_twenty_byte_connection_payloads_from_the_wrong_u32_layout() {
        let mut malformed_node = fixed_name("Body", W3D_NAME_LEN);
        malformed_node.extend_from_slice(&1u16.to_le_bytes());
        // This extra dword-tail shape comes from the incorrect u32 helper
        // struct, not `sizeof(W3dHModelNodeStruct)` in the original MSVC C++.
        malformed_node.extend_from_slice(&[0u8; 2]);
        assert_eq!(malformed_node.len(), 20);
        let bytes = [
            rigid_hlod_fixture(1, &[], &[]),
            hmodel_fixture_chunk_with_trailing(
                0x0004_0002,
                "BAD_NODE_LAYOUT",
                "RIG_HIER",
                &[],
                &[(W3D_CHUNK_HMODEL_NODE, malformed_node)],
            ),
        ]
        .concat();
        let model = W3DLoader::new()
            .load_model_from_bytes(&bytes, "bad_hmodel_node_layout")
            .expect("malformed HMODEL remains inspectable but cannot render");

        assert!(model.hmodels[0].has_invalid_records);
        assert!(model.hmodels[0].nodes.is_empty());
        assert!(model.hmodel_rigid_node_poses(0).is_none());
        assert!(model.hmodel_bind_pose_palette(0).is_none());
        assert!(model.hmodel_skin_node_bindings(0).is_none());
    }

    #[test]
    fn hmodel_missing_named_htree_uses_only_default_root_and_never_an_inferred_palette() {
        let mut model = W3DModel::new("hmodel_default_root".to_string());
        model.hierarchy = Some(W3dHierarchy {
            name: "UNRELATED_TREE".to_string(),
            pivots: vec![
                W3dPivot {
                    name: "ROOT".to_string(),
                    parent_idx: u32::MAX,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
                W3dPivot {
                    name: "UNRELATED_BONE".to_string(),
                    parent_idx: 0,
                    translation: [99.0, 88.0, 77.0],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
            ],
            pivot_fixups: Vec::new(),
        });
        model.hmodels.push(W3dHmodel {
            version: 0x0004_0002,
            name: "DEFAULT_ROOT_HMODEL".to_string(),
            hierarchy_name: "MISSING_TREE".to_string(),
            nodes: vec![
                W3dHmodelNode {
                    name: "DEFAULT_ROOT_HMODEL.RootChild".to_string(),
                    bone_index: 0,
                    kind: W3dHmodelNodeKind::Node,
                },
                W3dHmodelNode {
                    name: "DEFAULT_ROOT_HMODEL.InvalidChild".to_string(),
                    bone_index: 1,
                    kind: W3dHmodelNodeKind::Node,
                },
                W3dHmodelNode {
                    name: "DEFAULT_ROOT_HMODEL.RootSkin".to_string(),
                    bone_index: 0,
                    kind: W3dHmodelNodeKind::SkinNode,
                },
                W3dHmodelNode {
                    name: "DEFAULT_ROOT_HMODEL.InvalidSkin".to_string(),
                    bone_index: 1,
                    kind: W3dHmodelNodeKind::SkinNode,
                },
            ],
            source_snap_points: Vec::new(),
            has_invalid_records: false,
        });

        let poses = model
            .hmodel_rigid_node_poses(0)
            .expect("C++ Animatable3DObjClass creates a default HTree on a miss");
        assert_eq!(poses.len(), 1);
        assert_eq!(poses[0].name, "DEFAULT_ROOT_HMODEL.RootChild");
        assert_eq!(poses[0].parent_transform, Mat4::IDENTITY);
        assert_eq!(
            model
                .hmodel_bind_pose_palette(0)
                .expect("missing named HTree uses only C++ Init_Default"),
            vec![Mat4::IDENTITY],
            "an unrelated whole-file hierarchy must never become an HMODEL skin palette"
        );
        assert_eq!(
            model
                .hmodel_skin_node_bindings(0)
                .expect("valid root skin remains independent from invalid siblings"),
            vec![W3dHmodelSkinNodeBinding {
                name: "DEFAULT_ROOT_HMODEL.RootSkin".to_string(),
                bone_index: 0,
            }],
            "the out-of-range skin connection must fail closed without a root alias"
        );
    }

    #[test]
    fn hmodel_skin_mesh_requires_one_valid_influence_for_each_vertex() {
        let mut mesh = W3DMesh::new("strict_skin".to_string());
        mesh.vertices = vec![
            W3DVertex {
                position: [0.0; 3],
                normal: [0.0, 0.0, 1.0],
                uv: [0.0; 2],
                color: [1.0; 4],
            },
            W3DVertex {
                position: [1.0, 0.0, 0.0],
                normal: [0.0, 0.0, 1.0],
                uv: [1.0, 0.0],
                color: [1.0; 4],
            },
        ];
        mesh.vertex_influences = Some(vec![
            W3dVertInfStruct {
                bone_idx: 0,
                pad: [0; 6],
            },
            W3dVertInfStruct {
                bone_idx: 1,
                pad: [0; 6],
            },
        ]);

        assert!(mesh.has_complete_skin_influences_for_palette(2));
        assert!(
            !mesh.has_complete_skin_influences_for_palette(1),
            "an influence outside the owning HMODEL palette is not a root fallback"
        );

        mesh.vertex_influences
            .as_mut()
            .expect("fixture influences")
            .pop();
        assert!(
            !mesh.has_complete_skin_influences_for_palette(2),
            "C++ reads exactly one W3dVertInfStruct per source vertex"
        );
    }

    #[test]
    fn hmodel_modern_invalid_pivot_fails_closed_without_partial_rigid_rendering() {
        let model = W3DLoader::new()
            .load_model_from_bytes(
                &rigid_hmodel_fixture(3 << 16, &[(W3D_CHUNK_HMODEL_NODE, "BadPivot", u16::MAX)]),
                "invalid_hmodel",
            )
            .expect("invalid source topology remains inspectable");

        assert!(model.hmodels[0].has_invalid_records);
        assert!(
            model.hmodel_rigid_node_poses(0).is_none(),
            "modern 0xffff pivot is an assertion violation in C++; Main must not guess"
        );
        assert!(
            model.hmodel_bind_pose_palette(0).is_none(),
            "a malformed HMODEL cannot still donate a skin palette"
        );
    }

    #[test]
    fn w3d_hlod_visibility_raw_bit_channel_uses_lsb_frames_and_default_outside_range() {
        let model = W3DLoader::new()
            .load_model_from_bytes(&visibility_hlod_fixture(), "visibility_hlod")
            .expect("source-shaped raw visibility HLOD should parse");
        assert_eq!(model.animations.len(), 1);
        let animation = &model.animations[0];
        assert_eq!(animation.raw_visibility_channels.len(), 1);
        assert_eq!(animation.raw_visibility_channels[0].pivot, 1);
        assert!(animation.raw_visibility_channels[0].visible_at(0));
        assert!(animation.raw_visibility_channels[0].visible_at(2));
        assert!(
            !animation.raw_visibility_channels[0].visible_at(3),
            "bit one is the second low-order bit, not MSB-first"
        );
        assert!(animation.raw_visibility_channels[0].visible_at(4));
        assert!(
            animation.raw_visibility_channels[0].visible_at(5),
            "frames outside [FirstFrame, LastFrame] use DefaultVal"
        );

        let at_default = model
            .mesh_local_transform_and_visibility_for_animation(0, Some(0), 0.0)
            .expect("exact HLOD child should resolve at default frame");
        assert!(at_default.1);
        let at_hidden = model
            .mesh_local_transform_and_visibility_for_animation(0, Some(0), 3.0)
            .expect("exact HLOD child should resolve at authored hidden frame");
        assert!(
            !at_hidden.1,
            "visibility follows the source BoneIndex, even though mesh name has _d"
        );
        let at_visible = model
            .mesh_local_transform_and_visibility_for_animation(0, Some(0), 4.0)
            .expect("exact HLOD child should resolve at authored visible frame");
        assert!(at_visible.1);
    }

    #[test]
    fn w3d_hlod_visibility_hide_show_subobjects_apply_exact_child_bones_in_order() {
        let model = hide_show_subobjects_hlod_model();
        let directives = vec![
            AuthoredDrawSubobjectVisibility {
                // C++ first tries this complete retained HLOD child identity.
                name: "VIS_HLOD.ParentMesh".to_string(),
                hidden: true,
            },
            AuthoredDrawSubobjectVisibility {
                // This case-insensitive leaf form uses C++'s first-dot pass,
                // but only after Main has matched a retained HLOD record.
                name: "siblingmesh".to_string(),
                hidden: true,
            },
            AuthoredDrawSubobjectVisibility {
                // The descendant's later show must override the parent's hide.
                name: "CHILDMESH".to_string(),
                hidden: false,
            },
            AuthoredDrawSubobjectVisibility {
                // Unknown directives must not become a broad mesh-name rule.
                name: "not_a_retained_hlod_child".to_string(),
                hidden: true,
            },
        ];

        assert!(
            !model.mesh_visible_for_authored_subobject_directives(0, &directives),
            "the full HLOD child directive hides its direct target"
        );
        assert!(
            model.mesh_visible_for_authored_subobject_directives(1, &directives),
            "a same-bone sibling is not the directly named C++ RenderObj and must stay visible"
        );
        assert!(
            model.mesh_visible_for_authored_subobject_directives(2, &directives),
            "a later descendant ShowSubObject wins over its hidden ancestor"
        );
        assert!(
            !model.mesh_visible_for_authored_subobject_directives(3, &directives),
            "the exact leaf directive resolves only its retained sibling record"
        );
    }

    #[test]
    fn w3d_hlod_turret_primary_controls_exact_bones_after_selected_animation() {
        let model = primary_turret_hlod_model();
        let binding = W3dAnimationBinding::local(0);
        let primary_turret = AuthoredDrawPrimaryTurret {
            // The synthetic HLOD child names are deliberately unrelated to
            // these exact source pivot names.
            yaw_bone: Some("yaw_pivot".to_string()),
            pitch_bone: Some("pitch_pivot".to_string()),
            yaw_art_angle_radians_bits: std::f32::consts::FRAC_PI_2.to_bits(),
            pitch_art_angle_radians_bits: 0.0f32.to_bits(),
            ..Default::default()
        };

        let normal_hull = model
            .mesh_local_transform_and_visibility_for_binding(0, Some(&binding), 1.0)
            .expect("selected raw HAnim should resolve the chassis");
        let normal_muzzle = model
            .mesh_local_transform_and_visibility_for_binding(3, Some(&binding), 1.0)
            .expect("selected raw HAnim should resolve the muzzle");
        let controlled_hull = model
            .mesh_local_transform_and_visibility_for_primary_turret(
                0,
                Some(&binding),
                1.0,
                &primary_turret,
                0.0,
                90.0,
            )
            .expect("exact HLOD chassis remains renderable");
        let controlled_barrel = model
            .mesh_local_transform_and_visibility_for_primary_turret(
                2,
                Some(&binding),
                1.0,
                &primary_turret,
                0.0,
                90.0,
            )
            .expect("exact HLOD pitch child remains renderable");
        let controlled_muzzle = model
            .mesh_local_transform_and_visibility_for_primary_turret(
                3,
                Some(&binding),
                1.0,
                &primary_turret,
                0.0,
                90.0,
            )
            .expect("exact HLOD pitch descendant remains renderable");
        let bind_controlled_hull = model
            .mesh_local_transform_and_visibility_for_primary_turret(
                0,
                None,
                1.0,
                &primary_turret,
                0.0,
                90.0,
            )
            .expect("bind-pose chassis remains renderable under the same control");
        let bind_controlled_muzzle = model
            .mesh_local_transform_and_visibility_for_primary_turret(
                3,
                None,
                1.0,
                &primary_turret,
                0.0,
                90.0,
            )
            .expect("bind-pose muzzle remains renderable under the same control");

        assert_eq!(normal_hull.1, controlled_hull.1);
        assert!(
            normal_hull
                .0
                .to_cols_array()
                .iter()
                .zip(controlled_hull.0.to_cols_array())
                .all(|(before, after)| (*before - after).abs() < 1.0e-5),
            "a source turret binding must not rotate the chassis/hull"
        );
        assert!(
            (normal_muzzle.0.w_axis.truncate() - controlled_muzzle.0.w_axis.truncate()).length()
                > 1.0e-3,
            "yaw/pitch controls must affect only their exact HTree descendant path"
        );
        assert!(
            (controlled_barrel.0.w_axis.truncate() - controlled_muzzle.0.w_axis.truncate())
                .length()
                > 1.0e-3,
            "the pitch control is installed before the next source descendant is evaluated"
        );
        assert!(
            (bind_controlled_hull.0.to_cols_array()
                .iter()
                .zip(controlled_hull.0.to_cols_array())
                .map(|(bind, selected)| (bind - selected).abs())
                .fold(0.0f32, f32::max))
                < 1.0e-5,
            "a selected HAnim that animates the yaw subtree must still leave its sibling hull equal to bind pose"
        );
        assert!(
            (bind_controlled_muzzle.0.to_cols_array()
                .iter()
                .zip(controlled_muzzle.0.to_cols_array())
                .map(|(bind, selected)| (bind - selected).abs())
                .fold(0.0f32, f32::max))
                > 1.0e-3,
            "the controlled descendant must retain the selected frame-one HAnim pose rather than reverting to bind pose"
        );

        let alternate_turret = AuthoredDrawPrimaryTurret {
            alternate_yaw_bone_present: true,
            ..primary_turret.clone()
        };
        let alternate_fallback = model
            .mesh_local_transform_and_visibility_for_primary_turret(
                3,
                Some(&binding),
                1.0,
                &alternate_turret,
                0.0,
                90.0,
            )
            .expect("unsupported alternate source retains normal HLOD pose");
        assert!(
            alternate_fallback
                .0
                .to_cols_array()
                .iter()
                .zip(normal_muzzle.0.to_cols_array())
                .all(|(fallback, normal)| (*fallback - normal).abs() < 1.0e-5),
            "an authored alternate turret must fail closed instead of borrowing the primary angle"
        );
    }

    #[test]
    fn aggregate_poses_inherit_primary_turret_and_recoil_controls_in_cxx_order() {
        let model = primary_turret_hlod_model_with_barrel_aggregate();
        let binding = W3dAnimationBinding::local(0);
        let primary_turret = AuthoredDrawPrimaryTurret {
            yaw_bone: Some("yaw_pivot".to_string()),
            pitch_bone: Some("pitch_pivot".to_string()),
            yaw_art_angle_radians_bits: std::f32::consts::FRAC_PI_2.to_bits(),
            pitch_art_angle_radians_bits: 0.0f32.to_bits(),
            ..Default::default()
        };
        let weapon_controls = [W3dWeaponVisualControl {
            recoil_pivot_index: Some(3),
            recoil_shift: 2.5,
            muzzle_flash_pivot_index: None,
            muzzle_flash_visible: false,
        }];

        let parent_barrel = model
            .mesh_local_transform_and_visibility_for_primary_turret_and_weapon_controls(
                2,
                Some(&binding),
                1.0,
                &primary_turret,
                0.0,
                90.0,
                &weapon_controls,
            )
            .expect("the rigid barrel parent must retain its exact controlled pose");
        let aggregate = model
            .aggregate_attachment_poses_for_primary_turret_and_weapon_controls(
                Some(&binding),
                1.0,
                &primary_turret,
                0.0,
                90.0,
                &weapon_controls,
            )
            .expect("an aggregate on a valid parent pivot must inherit controls");
        assert_eq!(aggregate.len(), 1);
        assert_eq!(aggregate[0].name, "EXTERNAL_BARREL_ATTACHMENT");
        assert!(aggregate[0].visible);
        assert!(
            aggregate[0]
                .parent_transform
                .to_cols_array()
                .iter()
                .zip(parent_barrel.0.to_cols_array())
                .all(|(aggregate, parent)| (*aggregate - parent).abs() < 1.0e-5),
            "C++ AdditionalModels use the same final HTree transform as their parent barrel"
        );

        let bind_pose = model
            .aggregate_attachment_poses_for_binding(Some(&binding), 1.0)
            .expect("source-valid aggregate bind/HAnim pose");
        assert!(
            bind_pose[0]
                .parent_transform
                .to_cols_array()
                .iter()
                .zip(aggregate[0].parent_transform.to_cols_array())
                .any(|(before, controlled)| (*before - controlled).abs() > 1.0e-4),
            "turret/recoil controls must not be dropped from the aggregate pose"
        );
    }

    #[test]
    fn w3d_hlod_weapon_barrel_topology_uses_all_four_bases_and_cxx_numbered_order() {
        let mut model = primary_turret_hlod_model();
        {
            let hierarchy = model
                .hierarchy
                .as_mut()
                .expect("single-HLOD fixture has an HTree");
            hierarchy.pivots[1].name = "Fx01".to_string();
            hierarchy.pivots[2].name = "Recoil01".to_string();
            hierarchy.pivots[3].name = "Muzzle01".to_string();
            hierarchy.pivots[4].name = "Launch01".to_string();
            hierarchy.pivots.push(W3dPivot {
                // A second muzzle but no second FX is the explicit C++ exception:
                // it reuses the first exact FX pivot rather than abandoning the
                // numbered barrel or inventing a bare-name lookup.
                name: "Muzzle02".to_string(),
                parent_idx: 0,
                translation: [0.0; 3],
                euler_angles: [0.0; 3],
                rotation: [0.0, 0.0, 0.0, 1.0],
            });
        }

        let bindings = AuthoredDrawWeaponBoneBindings {
            slots: [
                AuthoredDrawWeaponBoneSlot {
                    fire_fx_bone_base: Some("fx".to_string()),
                    recoil_bone_base: Some("recoil".to_string()),
                    muzzle_flash_bone_base: Some("muzzle".to_string()),
                    launch_bone_base: Some("launch".to_string()),
                    projectile_hide_show_bone: None,
                },
                AuthoredDrawWeaponBoneSlot::default(),
                AuthoredDrawWeaponBoneSlot::default(),
            ],
            source_fields_valid: true,
        };
        let topology = model
            .weapon_barrel_topology_for_authored_bindings(&bindings)
            .expect("one exact HLOD/hierarchy accepts frozen valid source bases");
        let primary = topology.slot(0).expect("PRIMARY topology");
        assert_eq!(primary.len(), 2, "01 then 02 stop at first all-missing 03");
        assert_eq!(topology.barrel_count(0), Some(2));
        assert_eq!(primary[0].fire_fx_pivot_index, Some(1));
        assert_eq!(primary[0].recoil_pivot_index, Some(2));
        assert_eq!(primary[0].muzzle_flash_pivot_index, Some(3));
        assert_eq!(primary[0].launch_pivot_index, Some(4));
        assert!(primary[0].has_recoil_or_muzzle());
        assert_eq!(
            primary[1].fire_fx_pivot_index,
            Some(1),
            "C++ reuses the previous numbered FX bone for a later muzzle-only barrel"
        );
        assert_eq!(primary[1].muzzle_flash_pivot_index, Some(5));
        assert_eq!(primary[1].recoil_pivot_index, None);
        assert_eq!(primary[1].launch_pivot_index, None);
        assert!(primary[1].has_recoil_or_muzzle());

        // No numbered `Bare*01` pivots exist, so C++ tries unadorned names
        // exactly once. It must not mix those with the numbered sequence.
        {
            let hierarchy = model
                .hierarchy
                .as_mut()
                .expect("single-HLOD fixture retains its HTree");
            hierarchy.pivots[1].name = "BareFx".to_string();
            hierarchy.pivots[2].name = "BareRecoil".to_string();
            hierarchy.pivots[3].name = "BareMuzzle".to_string();
            hierarchy.pivots[4].name = "BareLaunch".to_string();
            hierarchy.pivots.truncate(5);
        }
        let bare_bindings = AuthoredDrawWeaponBoneBindings {
            slots: [
                AuthoredDrawWeaponBoneSlot {
                    fire_fx_bone_base: Some("barefx".to_string()),
                    recoil_bone_base: Some("barerecoil".to_string()),
                    muzzle_flash_bone_base: Some("baremuzzle".to_string()),
                    launch_bone_base: Some("barelaunch".to_string()),
                    projectile_hide_show_bone: None,
                },
                AuthoredDrawWeaponBoneSlot::default(),
                AuthoredDrawWeaponBoneSlot::default(),
            ],
            source_fields_valid: true,
        };
        let bare = model
            .weapon_barrel_topology_for_authored_bindings(&bare_bindings)
            .expect("same valid single-HLOD accepts bare source bases");
        assert_eq!(bare.barrel_count(0), Some(1));
        assert_eq!(
            bare.slot(0).expect("PRIMARY bare topology")[0],
            W3dWeaponBarrelBinding {
                fire_fx_pivot_index: Some(1),
                recoil_pivot_index: Some(2),
                muzzle_flash_pivot_index: Some(3),
                launch_pivot_index: Some(4),
            }
        );
    }

    #[test]
    fn w3d_hlod_weapon_barrel_topology_rejects_invalid_source_and_unsupported_hlods() {
        let model = primary_turret_hlod_model();
        let invalid_source = AuthoredDrawWeaponBoneBindings {
            source_fields_valid: false,
            ..Default::default()
        };
        assert!(
            model
                .weapon_barrel_topology_for_authored_bindings(&invalid_source)
                .is_none(),
            "malformed source WeaponSlotType data cannot fall through to an empty/guessed topology"
        );

        let mut multi_lod = model.clone();
        let second_lod = multi_lod.hlods[0].lods[0].clone();
        multi_lod.hlods[0].lods.push(second_lod);
        assert!(
            multi_lod
                .weapon_barrel_topology_for_authored_bindings(&AuthoredDrawWeaponBoneBindings::default())
                .is_none(),
            "until C++ LOD selection exists, even empty valid bindings may not authorize a multi-LOD model"
        );
    }

    #[test]
    fn w3d_hlod_visibility_compressed_channel_fails_closed_for_its_authored_pivot() {
        let mut model = W3DLoader::new()
            .load_model_from_bytes(&visibility_hlod_fixture(), "visibility_hlod")
            .expect("source-shaped raw visibility HLOD should parse");
        model.animations[0]
            .unsupported_visibility_pivots
            .push(Some(1));
        assert!(
            model
                .mesh_local_transform_and_visibility_for_animation(0, Some(0), 0.0)
                .is_none(),
            "a compressed visibility source must not be rendered using a guessed value"
        );
    }

    #[test]
    fn w3d_companion_animation_external_binding_wins_over_local_clip() {
        let loader = W3DLoader::new();
        let geometry = loader
            .load_model_from_bytes(&visibility_hlod_fixture(), "geometry")
            .expect("source-shaped geometry fixture should parse");
        let companion = loader
            .load_companion_animation_from_bytes(
                &companion_animation_fixture("VIS_HIER", "EXTERNAL", 4.0, 12.0),
                "VIS_HIER.EXTERNAL",
            )
            .expect("animation-only companion should parse through HAnim path");
        let mut compressed_companion = companion.clone();
        compressed_companion.source_is_compressed = true;
        assert!(companion.matches_draw_identity("vis_hier.external"));
        assert_eq!(
            w3d_companion_animation_filename("VIS_HIER.EXTERNAL"),
            Some("EXTERNAL.w3d".to_string())
        );

        let binding = W3dAnimationBinding::companion("VIS_HIER.EXTERNAL", Arc::new(companion));
        assert!(geometry.animation_binding_is_compatible(&binding));
        let compressed_binding =
            W3dAnimationBinding::companion("VIS_HIER.EXTERNAL", Arc::new(compressed_companion));
        assert!(
            !geometry.animation_binding_is_compatible(&compressed_binding),
            "external compressed companion channels stay fail-closed until their path is ported"
        );

        let local_x = geometry
            .sample_animation(0, 1.0)
            .expect("local source clip should sample")[1][12];
        let external_x = geometry
            .sample_animation_binding(&binding, 1.0)
            .expect("exact external binding should sample")[1][12];
        assert_eq!(local_x, 3.0, "fixture local clip is only the bind pose");
        assert_eq!(
            external_x, 12.0,
            "companion motion must override local clip"
        );

        let (transform, visible) = geometry
            .mesh_local_transform_and_visibility_for_binding(0, Some(&binding), 1.0)
            .expect("external companion must carry through the exact HLOD record");
        assert!(visible, "companion with no bit channel defaults to visible");
        assert!(
            (transform.w_axis.x - 12.0).abs() < 0.0001,
            "HLOD transform must use the external companion, got {transform:?}"
        );

        // A missing external binding becomes the source bind pose at the
        // collector boundary, not the geometry file's local animation zero.
        let bind_pose = geometry
            .mesh_local_transform_and_visibility_for_binding(0, None, 1.0)
            .expect("absent selected HAnim is a bind-pose request");
        assert!((bind_pose.0.w_axis.x - 3.0).abs() < 0.0001);
    }

    #[test]
    fn w3d_companion_animation_retail_china_agent_asset_identity_when_available() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let candidates = [
            root.join("windows_game/extracted_big_files/W3DZH/Art/W3D/AIRNGR_ATB.W3D"),
            root.join("windows_game/extracted_big_files/W3DZH/art/w3d/AIRNGR_ATB.W3D"),
        ];
        let Some(path) = candidates.into_iter().find(|path| path.is_file()) else {
            eprintln!("skip: retail AIRNGR_ATB.W3D companion is not available on disk");
            return;
        };
        let paths = w3d_companion_animation_archive_path_variants("AIRngr_SKL.AIRngr_ATB")
            .expect("qualified retail Draw identity must yield exact companion paths");
        assert_eq!(
            w3d_companion_animation_filename("AIRngr_SKL.AIRngr_ATB"),
            Some("AIRngr_ATB.w3d".to_string())
        );
        assert!(
            paths.iter().any(|path| path == "Art/W3D/AIRNGR_ATB.W3D"),
            "case-only exact companion candidates must retain retail archive spelling: {paths:?}"
        );

        let bytes = std::fs::read(&path).expect("retail companion W3D bytes");
        let animation = W3DLoader::new()
            .load_companion_animation_from_bytes(&bytes, "AIRngr_SKL.AIRngr_ATB")
            .expect("retail China Agent companion must contain its exact HAnim");
        assert!(animation.matches_draw_identity("airngr_skl.airngr_atb"));
    }

    #[test]
    fn htree_source_matrix_multiply_keeps_cxx_parent_local_and_capture_order() {
        let quarter_turn = std::f32::consts::FRAC_1_SQRT_2;
        let hierarchy = W3dHierarchy {
            name: "MATRIX_ORDER".to_string(),
            pivots: vec![
                W3dPivot {
                    name: "ROOT".to_string(),
                    parent_idx: u32::MAX,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
                W3dPivot {
                    name: "ROTATED_PARENT".to_string(),
                    parent_idx: 0,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, quarter_turn, quarter_turn],
                },
                W3dPivot {
                    name: "LOCAL_CHILD".to_string(),
                    parent_idx: 1,
                    translation: [2.0, 0.0, 0.0],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
            ],
            pivot_fixups: Vec::new(),
        };
        let locals: Vec<_> = hierarchy.pivots.iter().map(mat4_from_pivot).collect();

        let globals = compute_htree_global_transforms_from_locals(&hierarchy, &locals)
            .expect("ordered affine HTree must evaluate");
        assert!(
            (Mat4::from_cols_array(&globals[2]).w_axis.truncate() - Vec3::new(0.0, 2.0, 0.0))
                .length()
                < 0.0001,
            "C++ Matrix3D::Multiply(parent, local) rotates the child's local translation"
        );

        let mut controls = vec![None; hierarchy.pivots.len()];
        controls[1] = Some(Mat4::from_translation(Vec3::X * 3.0).to_cols_array());
        let captured = compute_htree_global_transforms_from_locals_with_capture_controls(
            &hierarchy, &locals, &controls,
        )
        .expect("a valid C++ Capture_Bone control must evaluate");
        assert!(
            (Mat4::from_cols_array(&captured[2]).w_axis.truncate() - Vec3::new(0.0, 5.0, 0.0))
                .length()
                < 0.0001,
            "C++ Control_Bone post-multiplies the parent before descendants inherit it"
        );

        let loader_globals = W3DLoader::compute_global_transforms(&hierarchy)
            .expect("legacy HMODEL residual must use the same source matrix convention");
        assert!(
            (Mat4::from_cols_array(&loader_globals[2]).w_axis.truncate()
                - Vec3::new(0.0, 2.0, 0.0))
            .length()
                < 0.0001
        );
    }

    #[test]
    fn raw_hanim_uses_cxx_integer_delta_postcomposition_and_duplicate_channel_rules() {
        let quarter_turn = std::f32::consts::FRAC_1_SQRT_2;
        let hierarchy = W3dHierarchy {
            name: "RAW_DELTA".to_string(),
            pivots: vec![
                W3dPivot {
                    name: "ROOT".to_string(),
                    parent_idx: u32::MAX,
                    translation: [0.0; 3],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
                W3dPivot {
                    name: "ANIMATED_PARENT".to_string(),
                    parent_idx: 0,
                    translation: [10.0, 0.0, 0.0],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, quarter_turn, quarter_turn],
                },
                W3dPivot {
                    name: "CHILD".to_string(),
                    parent_idx: 1,
                    translation: [1.0, 0.0, 0.0],
                    euler_angles: [0.0; 3],
                    rotation: [0.0, 0.0, 0.0, 1.0],
                },
            ],
            pivot_fixups: Vec::new(),
        };
        let animation = W3dAnimation {
            name: "RAW_DELTA_ANIM".to_string(),
            hierarchy_name: "RAW_DELTA".to_string(),
            num_frames: 3,
            frame_rate: 30,
            source_is_compressed: false,
            channels: vec![
                // C++ starts its raw node-motion loop at pivot one, so this
                // malformed root channel must not poison the usable child pose.
                W3dAnimChannel {
                    first_frame: 0,
                    last_frame: 2,
                    vector_len: 0,
                    flags: 0,
                    pivot: 0,
                    data: Vec::new(),
                },
                W3dAnimChannel {
                    first_frame: 1,
                    last_frame: 1,
                    vector_len: 1,
                    flags: 0,
                    pivot: 1,
                    data: vec![2.0],
                },
                // `HRawAnimClass::add_channel` overwrites the prior X
                // pointer; the final source record is authoritative.
                W3dAnimChannel {
                    first_frame: 1,
                    last_frame: 1,
                    vector_len: 1,
                    flags: 0,
                    pivot: 1,
                    data: vec![3.0],
                },
                W3dAnimChannel {
                    first_frame: 1,
                    last_frame: 1,
                    vector_len: 4,
                    flags: 6,
                    pivot: 1,
                    data: vec![0.0, 0.0, quarter_turn, quarter_turn],
                },
            ],
            raw_visibility_channels: vec![W3dRawVisibilityChannel {
                first_frame: 1,
                last_frame: 1,
                flags: 0,
                pivot: 1,
                default_visible: true,
                bits: vec![0],
            }],
            unsupported_visibility_pivots: Vec::new(),
        };

        let raw_one = sample_animation_local_transforms(&hierarchy, &animation, 1.25)
            .expect("fractional raw frame must use the C++ rounded integer frame");
        let raw_one_globals = compute_htree_global_transforms_from_locals(&hierarchy, &raw_one)
            .expect("valid raw HAnim hierarchy");
        assert!(
            (Mat4::from_cols_array(&raw_one_globals[2]).w_axis.truncate()
                - Vec3::new(9.0, 3.0, 0.0))
                .length()
                < 0.0001,
            "base Rz90 * delta Tx3 * delta Rz90 must rotate the child after preserving its bind pose"
        );
        assert_eq!(
            animation.visibility_for_pivot(1, 1.25),
            Some(false),
            "specialized Generals raw update uses the same rounded frame for visibility"
        );

        let raw_two = sample_animation_local_transforms(&hierarchy, &animation, 1.5)
            .expect("half frame rounds to the even raw frame under C++ _RC_NEAR");
        let raw_two_globals = compute_htree_global_transforms_from_locals(&hierarchy, &raw_two)
            .expect("valid raw HAnim hierarchy");
        assert!(
            (Mat4::from_cols_array(&raw_two_globals[2]).w_axis.truncate()
                - Vec3::new(10.0, 1.0, 0.0))
                .length()
                < 0.0001,
            "outside a raw channel range C++ supplies identity deltas rather than clamping/interpolating"
        );
        assert_eq!(animation.visibility_for_pivot(1, 1.5), Some(true));

        let wrapped = sample_animation_local_transforms(&hierarchy, &animation, 2.6)
            .expect("a raw frame at NumFrames wraps to zero");
        let wrapped_globals = compute_htree_global_transforms_from_locals(&hierarchy, &wrapped)
            .expect("valid wrapped raw HAnim hierarchy");
        assert!(
            (Mat4::from_cols_array(&wrapped_globals[2]).w_axis.truncate()
                - Vec3::new(10.0, 1.0, 0.0))
            .length()
                < 0.0001
        );
    }

    #[test]
    fn htree_runtime_pose_ignores_authored_pivot_zero_pose_channels_and_visibility() {
        let mut model = W3DLoader::new()
            .load_model_from_bytes(&rigid_hlod_fixture(1, &[], &[]), "root_pose")
            .expect("source-shaped rigid HLOD fixture should parse");
        let original_bounds = (model.bounding_box_min, model.bounding_box_max);
        let hierarchy = model
            .hierarchy
            .as_mut()
            .expect("rigid HLOD fixture has an HTree");
        hierarchy.pivots[0].translation = [99.0, 88.0, 77.0];
        hierarchy.pivots[0].rotation = [0.0, 0.0, 1.0, 0.0];
        model.animations.push(W3dAnimation {
            name: "ROOT_ONLY".to_string(),
            hierarchy_name: "RIG_HIER".to_string(),
            num_frames: 2,
            frame_rate: 30,
            source_is_compressed: false,
            channels: vec![
                W3dAnimChannel {
                    first_frame: 0,
                    last_frame: 1,
                    vector_len: 1,
                    flags: 0,
                    pivot: 0,
                    data: vec![0.0, 500.0],
                },
                W3dAnimChannel {
                    first_frame: 0,
                    last_frame: 1,
                    vector_len: 4,
                    flags: 6,
                    pivot: 0,
                    data: vec![0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0],
                },
            ],
            raw_visibility_channels: vec![W3dRawVisibilityChannel {
                first_frame: 0,
                last_frame: 1,
                flags: 0,
                pivot: 0,
                default_visible: false,
                bits: vec![0],
            }],
            // A compressed pivot-zero source is likewise ignored because C++
            // replaces pivot zero before it queries visibility.
            unsupported_visibility_pivots: vec![Some(0)],
        });

        let binding = W3dAnimationBinding::local(0);
        let sampled = model
            .sample_animation_binding(&binding, 1.0)
            .expect("root-local source data must not invalidate the HTree pose");
        assert_eq!(sampled[0], Mat4::IDENTITY.to_cols_array());
        assert_eq!(
            Mat4::from_cols_array(&sampled[1]).w_axis.truncate(),
            Vec3::new(10.0, 20.0, 30.0),
            "a child must inherit the external object root, not authored pivot-zero data"
        );
        assert_eq!(
            model.animations[0].visibility_for_pivot(0, 1.0),
            Some(true),
            "pivot zero is always visible even when its raw source channel is hidden or unsupported"
        );

        let (bind_transform, bind_visible) = model
            .mesh_local_transform_and_visibility_for_binding(0, None, 0.0)
            .expect("bind-pose rigid HLOD child");
        let (animated_transform, animated_visible) = model
            .mesh_local_transform_and_visibility_for_binding(0, Some(&binding), 1.0)
            .expect("animated rigid HLOD child");
        assert!(bind_visible && animated_visible);
        assert_eq!(animated_transform, bind_transform);

        let palette = model
            .animation_palette_for_binding_and_capture_controls(Some(&binding), 1.0, &[])
            .expect("selected HAnim palette must use the same HTree root rule");
        assert_eq!(palette[0], Mat4::IDENTITY);
        assert_eq!(palette[1], animated_transform);

        model.calculate_bounding_box();
        assert_eq!(
            (model.bounding_box_min, model.bounding_box_max),
            original_bounds,
            "bind-pose bounds must ignore authored pivot-zero data"
        );

        let mut root_mesh_model = model.clone();
        root_mesh_model.hlods[0].lods[0].subobjects[0].bone_index = 0;
        let (root_transform, root_visible) = root_mesh_model
            .mesh_local_transform_and_visibility_for_binding(0, Some(&binding), 1.0)
            .expect("a valid root-bound HLOD child must receive the external root");
        assert_eq!(root_transform, Mat4::IDENTITY);
        assert!(root_visible);

        let loader_globals = W3DLoader::compute_global_transforms(
            root_mesh_model
                .hierarchy
                .as_ref()
                .expect("fixture hierarchy"),
        )
        .expect("loader residual HModel path must share the HTree root rule");
        assert_eq!(loader_globals[0], Mat4::IDENTITY.to_cols_array());
        assert_eq!(
            Mat4::from_cols_array(&loader_globals[1]).w_axis.truncate(),
            Vec3::new(10.0, 20.0, 30.0)
        );
    }

    #[test]
    fn pre30_hierarchy_and_raw_animation_insert_and_address_the_synthetic_root() {
        const PRE30_VERSION: u32 = (2 << 16) | 1;

        let mut hierarchy_header = Vec::with_capacity(36);
        hierarchy_header.extend_from_slice(&PRE30_VERSION.to_le_bytes());
        hierarchy_header.extend_from_slice(&fixed_name("LEGACY_HIER", W3D_NAME_LEN));
        hierarchy_header.extend_from_slice(&2u32.to_le_bytes());
        hierarchy_header.extend_from_slice(&[0u8; 12]);
        let hierarchy_data = [
            chunk(W3D_CHUNK_HIERARCHY_HEADER, hierarchy_header, false),
            chunk(
                W3D_CHUNK_PIVOTS,
                [
                    pivot("LEGACY_ROOT", u32::MAX, [0.0, 0.0, 0.0]),
                    pivot("LEGACY_CHILD", 0, [2.0, 0.0, 0.0]),
                ]
                .concat(),
                false,
            ),
        ]
        .concat();

        let mut animation_header = Vec::with_capacity(44);
        animation_header.extend_from_slice(&PRE30_VERSION.to_le_bytes());
        animation_header.extend_from_slice(&fixed_name("LEGACY_ANIM", W3D_NAME_LEN));
        animation_header.extend_from_slice(&fixed_name("LEGACY_HIER", W3D_NAME_LEN));
        animation_header.extend_from_slice(&2u32.to_le_bytes());
        animation_header.extend_from_slice(&30u32.to_le_bytes());
        let mut root_x_channel = Vec::with_capacity(20);
        root_x_channel.extend_from_slice(&0u16.to_le_bytes());
        root_x_channel.extend_from_slice(&1u16.to_le_bytes());
        root_x_channel.extend_from_slice(&1u16.to_le_bytes());
        root_x_channel.extend_from_slice(&0u16.to_le_bytes());
        root_x_channel.extend_from_slice(&0u16.to_le_bytes());
        root_x_channel.extend_from_slice(&0u16.to_le_bytes());
        root_x_channel.extend_from_slice(&0.0f32.to_le_bytes());
        root_x_channel.extend_from_slice(&7.0f32.to_le_bytes());
        let mut child_visibility = Vec::with_capacity(10);
        child_visibility.extend_from_slice(&0u16.to_le_bytes());
        child_visibility.extend_from_slice(&1u16.to_le_bytes());
        child_visibility.extend_from_slice(&0u16.to_le_bytes());
        child_visibility.extend_from_slice(&1u16.to_le_bytes());
        child_visibility.push(1);
        // Source child pivot one is visible at frame zero and hidden at one.
        child_visibility.push(0b0000_0001);
        let animation_data = [
            chunk(W3D_CHUNK_ANIMATION_HEADER, animation_header, false),
            chunk(W3D_CHUNK_ANIMATION_CHANNEL, root_x_channel, false),
            chunk(W3D_CHUNK_BIT_CHANNEL, child_visibility, false),
        ]
        .concat();

        let loader = W3DLoader::new();
        let hierarchy = loader
            .parse_hierarchy_chunk(&hierarchy_data)
            .expect("pre-3.0 hierarchy source must normalize safely");
        assert_eq!(hierarchy.pivots.len(), 3);
        assert_eq!(hierarchy.pivots[0].name, "RootTransform");
        assert_eq!(hierarchy.pivots[0].parent_idx, u32::MAX);
        assert_eq!(hierarchy.pivots[1].name, "LEGACY_ROOT");
        assert_eq!(hierarchy.pivots[1].parent_idx, 0);
        assert_eq!(hierarchy.pivots[2].name, "LEGACY_CHILD");
        assert_eq!(hierarchy.pivots[2].parent_idx, 1);

        let animation = loader
            .parse_animation_chunk(&animation_data)
            .expect("pre-3.0 raw HAnim source must normalize safely");
        assert_eq!(animation.channels[0].pivot, 1);
        assert_eq!(animation.raw_visibility_channels[0].pivot, 2);
        assert_eq!(animation.visibility_for_pivot(2, 0.0), Some(true));
        assert_eq!(animation.visibility_for_pivot(2, 1.0), Some(false));

        let mut model = W3DModel::new("pre30_pose".to_string());
        model.hierarchy = Some(hierarchy);
        model.animations.push(animation);
        let sampled = model
            .sample_animation_binding(&W3dAnimationBinding::local(0), 1.0)
            .expect("shifted raw HAnim channel must reach the normalized hierarchy");
        assert_eq!(sampled[0], Mat4::IDENTITY.to_cols_array());
        assert_eq!(
            sampled[1][12], 7.0,
            "source root channel shifts to pivot one"
        );
        assert_eq!(
            sampled[2][12],
            9.0,
            "the source child must inherit the shifted original root, not the synthetic external root"
        );
    }

    #[test]
    fn multi_lod_rigid_hlod_uses_cxx_constructor_selection_and_retains_attachment_metadata() {
        let mut multi_lod = W3DLoader::new()
            .load_model_from_bytes(&rigid_hlod_fixture(2, &[], &[]), "multi_lod")
            .expect("multi-LOD source fixture should parse and retain metadata");
        assert_eq!(multi_lod.hlods[0].lods.len(), 2);

        // C++ `Calculate_Cost_Value_Arrays(1.0f, ...)` uses a strict `<`.
        // A level whose authored maximum is exactly one pixel remains the
        // constructor-selected minimum level.
        multi_lod.hlods[0].lods[0].max_screen_size = 1.0;
        multi_lod.hlods[0].lods[1].max_screen_size = f32::MAX;
        assert_eq!(
            W3DModel::cxx_constructor_selected_hlod_lod_index(&multi_lod.hlods[0]),
            Some(0)
        );

        // Once the first level is strictly below one, C++ raises CurLod to the
        // second level. Make only that selected level name the flattened mesh
        // so the transform proof cannot accidentally draw both source groups.
        multi_lod.hlods[0].lods[0].max_screen_size = 0.999_999;
        multi_lod.hlods[0].lods[0].subobjects[0].name = "HLODROOT.LOW_ONLY".to_string();
        assert_eq!(
            W3DModel::cxx_constructor_selected_hlod_lod_index(&multi_lod.hlods[0]),
            Some(1)
        );
        assert!(
            multi_lod
                .mesh_local_transform_for_animation(0, 0, 0.0)
                .is_some(),
            "only the C++ constructor-selected HLOD level may resolve a rigid child"
        );
        assert!(
            !multi_lod.mesh_visible_for_authored_subobject_directives(
                0,
                &[AuthoredDrawSubobjectVisibility {
                    name: "RIGID".to_string(),
                    hidden: true,
                }],
            ),
            "HideSubObject must resolve only within that same selected source level"
        );

        // If all levels are below one pixel, C++ clamps to the final (highest
        // detail) level rather than wrapping or choosing a guessed default.
        multi_lod.hlods[0].lods[1].max_screen_size = 0.5;
        assert_eq!(
            W3DModel::cxx_constructor_selected_hlod_lod_index(&multi_lod.hlods[0]),
            Some(1)
        );

        // A malformed threshold is not a license to render an arbitrary
        // source level in Main's bounded implementation.
        multi_lod.hlods[0].lods[0].max_screen_size = f32::NAN;
        assert!(
            multi_lod
                .mesh_local_transform_for_animation(0, 0, 0.0)
                .is_none(),
            "malformed HLOD thresholds must fail closed"
        );

        let aggregate = W3DLoader::new()
            .load_model_from_bytes(
                &rigid_hlod_fixture(
                    1,
                    &[("ATTACHED_MODEL", 1), ("SECOND_ATTACHMENT", 0)],
                    &[("SPAWN_POINT", 1), ("RALLY_PROXY", 0)],
                ),
                "aggregate_hlod",
            )
            .expect("source-shaped attachment arrays should parse");
        let aggregate_hlod = &aggregate.hlods[0];
        assert_eq!(
            aggregate_hlod.aggregates.as_ref(),
            Some(&W3dHlodAttachmentArray {
                max_screen_size: 0.0,
                subobjects: vec![
                    W3dHlodSubObject {
                        name: "ATTACHED_MODEL".to_string(),
                        bone_index: 1,
                    },
                    W3dHlodSubObject {
                        name: "SECOND_ATTACHMENT".to_string(),
                        bone_index: 0,
                    }
                ],
            })
        );
        assert_eq!(
            aggregate_hlod.proxies.as_ref(),
            Some(&W3dHlodAttachmentArray {
                max_screen_size: 0.0,
                subobjects: vec![
                    W3dHlodSubObject {
                        name: "SPAWN_POINT".to_string(),
                        bone_index: 1,
                    },
                    W3dHlodSubObject {
                        name: "RALLY_PROXY".to_string(),
                        bone_index: 0,
                    }
                ],
            })
        );
        assert!(aggregate_hlod.has_unrendered_aggregates);
        assert!(!aggregate_hlod.has_invalid_trailing_records);
        let aggregate_poses = aggregate
            .aggregate_attachment_poses_for_binding(None, 0.0)
            .expect("a source-valid aggregate HLOD must expose exact parent-bone poses");
        assert_eq!(aggregate_poses.len(), 2);
        assert_eq!(aggregate_poses[0].name, "ATTACHED_MODEL");
        assert_eq!(aggregate_poses[0].bone_index, 1);
        assert!(aggregate_poses[0].visible);
        assert_eq!(
            aggregate_poses[0].parent_transform.w_axis.truncate(),
            Vec3::new(10.0, 30.0, 20.0),
            "aggregate pose must use the parent HTree bone in the same render basis as rigid children"
        );
        assert_eq!(aggregate_poses[1].name, "SECOND_ATTACHMENT");
        assert_eq!(aggregate_poses[1].bone_index, 0);
        let controlled_poses = aggregate
            .aggregate_attachment_poses_for_binding_and_capture_controls(
                None,
                0.0,
                &[(1, Mat4::from_translation(Vec3::new(-2.0, 0.0, 0.0)))],
            )
            .expect("valid source capture controls must update aggregate parent poses");
        assert_eq!(
            controlled_poses[0].parent_transform.w_axis.truncate(),
            Vec3::new(8.0, 30.0, 20.0),
            "C++ controls post-multiply the aggregate's HTree parent pivot before child rendering"
        );
        let mut authored_root_offset = aggregate.clone();
        authored_root_offset
            .hierarchy
            .as_mut()
            .expect("synthetic aggregate hierarchy")
            .pivots[0]
            .translation = [99.0, 88.0, 77.0];
        let root_ignored_poses = authored_root_offset
            .aggregate_attachment_poses_for_binding(None, 0.0)
            .expect("HTree root handling must remain valid");
        assert_eq!(
            root_ignored_poses[0].parent_transform.w_axis.truncate(),
            Vec3::new(10.0, 30.0, 20.0),
            "HTree overwrites pivot zero with the parent object root; W3D pivot-zero bind data must not leak into the child attachment pose"
        );
        assert!(
            aggregate
                .mesh_local_transform_for_animation(0, 0, 0.0)
                .is_some(),
            "C++ keeps the selected parent LOD visible while each aggregate is resolved or skipped independently"
        );

        let proxy_only = W3DLoader::new()
            .load_model_from_bytes(
                &rigid_hlod_fixture(1, &[], &[("SPAWN_POINT", 1)]),
                "proxy_hlod",
            )
            .expect("source-shaped proxy array should parse");
        assert!(!proxy_only.hlods[0].has_unrendered_aggregates);
        assert!(!proxy_only.hlods[0].has_invalid_trailing_records);
        assert!(
            proxy_only
                .aggregate_attachment_poses_for_binding(None, 0.0)
                .is_none(),
            "proxies remain source application records, not implicit aggregate render objects"
        );
        assert!(
            proxy_only
                .mesh_local_transform_for_animation(0, 0, 0.0)
                .is_some(),
            "C++ proxies are non-rendering application records and must not hide the parent HLOD"
        );

        let mut malformed_proxy = rigid_hlod_fixture(1, &[], &[("SPAWN_POINT", 1)]);
        let proxy_chunk_type = W3D_CHUNK_HLOD_PROXY_ARRAY.to_le_bytes();
        let proxy_offset = malformed_proxy
            .windows(proxy_chunk_type.len())
            .position(|window| window == proxy_chunk_type)
            .expect("synthetic fixture must contain its proxy chunk");
        let raw_size_offset = proxy_offset + proxy_chunk_type.len();
        let raw_size = u32::from_le_bytes(
            malformed_proxy[raw_size_offset..raw_size_offset + 4]
                .try_into()
                .expect("proxy chunk must carry a raw size"),
        );
        malformed_proxy[raw_size_offset..raw_size_offset + 4]
            .copy_from_slice(&(raw_size & 0x7FFF_FFFF).to_le_bytes());
        let malformed_proxy = W3DLoader::new()
            .load_model_from_bytes(&malformed_proxy, "malformed_proxy_hlod")
            .expect("malformed trailing metadata must safely retain the outer HLOD record");
        assert!(malformed_proxy.hlods[0].has_invalid_trailing_records);
        assert!(
            malformed_proxy
                .mesh_local_transform_for_animation(0, 0, 0.0)
                .is_none(),
            "a malformed source attachment record must not be treated as safe rigid topology"
        );
    }

    #[test]
    fn retail_america_command_center_hlod_retains_rigid_bone_records_when_available() {
        let Some(path) = crate::assets::mesh_asset_resolve::find_filesystem_w3d("ABBtCmdHQ") else {
            eprintln!("skip: retail ABBtCmdHQ.W3D is not available on disk");
            return;
        };
        let model = W3DLoader::new()
            .load_model_from_path(&path)
            .expect("retail AmericaCommandCenter W3D should parse");
        let hlod = model
            .hlods
            .iter()
            .find(|hlod| hlod.name.eq_ignore_ascii_case("ABBTCMDHQ"))
            .expect("ABBtCmdHQ must retain its HLOD header");
        let fan = hlod.lods[0]
            .subobjects
            .iter()
            .find(|subobject| subobject.name.eq_ignore_ascii_case("ABBTCMDHQ.FAN03"))
            .expect("retail Command Center HLOD must retain FAN03 source identity");
        assert_eq!(fan.bone_index, 2);

        let fan_mesh_index = model
            .meshes
            .iter()
            .position(|mesh| {
                mesh.name.eq_ignore_ascii_case("FAN03")
                    && mesh.container_name.eq_ignore_ascii_case("ABBTCMDHQ")
            })
            .expect("retail Command Center must include FAN03 mesh with source ContainerName");
        assert!(
            model
                .mesh_local_transform_for_animation(fan_mesh_index, 0, 0.0)
                .is_some(),
            "retail FAN03 must bind through the authored HLOD record"
        );
    }

    #[test]
    fn w3d_hlod_visibility_hide_show_subobjects_retail_scorpion_maps_exact_hlod_child_when_available(
    ) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let ini_path = root
            .join("windows_game/extracted_big_files/INIZH/Data/INI/Object/GC_Chem_GLAUnits.ini");
        let Some(w3d_path) = crate::assets::mesh_asset_resolve::find_filesystem_w3d("UVLiteTank")
        else {
            eprintln!("skip: retail UVLiteTank.W3D is not available on disk");
            return;
        };
        let Ok(ini_content) = std::fs::read_to_string(&ini_path) else {
            eprintln!(
                "skip: retail Scorpion Object INI is not available at {}",
                ini_path.display()
            );
            return;
        };

        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&ini_content, "GC_Chem_GLAUnits.ini")
            .expect("parse retail Scorpion source Draw state");
        let scorpion = parser
            .get_definition("GC_Chem_GLATankScorpion")
            .expect("retail Chem Scorpion definition");
        let upgrade_bit_index =
            crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                "WEAPONSET_PLAYER_UPGRADE",
            )
            .expect("retail player-upgrade condition bit");
        let upgrade_bits = 1u128
            .checked_shl(u32::try_from(upgrade_bit_index).expect("condition bit index fits u32"))
            .expect("condition bit fits retained bank");
        let default_draw = scorpion
            .select_draw_models_for_conditions(0)
            .expect("retail pristine Scorpion draw state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
            .expect("retail pristine Scorpion selects UVLiteTank");
        let upgraded_draw = scorpion
            .select_draw_models_for_conditions(upgrade_bits)
            .expect("retail upgraded Scorpion draw state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
            .expect("retail upgraded Scorpion retains UVLiteTank");

        assert!(
            default_draw
                .subobject_visibility
                .iter()
                .any(|directive| directive.name == "misslerack01" && directive.hidden),
            "retail DefaultConditionState hides the misspelled source rack leaf"
        );
        assert!(
            upgraded_draw
                .subobject_visibility
                .iter()
                .any(|directive| directive.name == "misslerack01" && !directive.hidden),
            "retail upgrade state overwrites the inherited rack directive in place"
        );

        let model = W3DLoader::new()
            .load_model_from_path(&w3d_path)
            .expect("retail UVLiteTank W3D should parse");
        let rack_mesh_index = model
            .meshes
            .iter()
            .position(|mesh| {
                mesh.name.eq_ignore_ascii_case("MISSLERACK01")
                    && mesh.container_name.eq_ignore_ascii_case("UVLITETANK")
            })
            .expect("retail UVLiteTank mesh must retain exact HLOD child identity");
        assert!(
            !model.mesh_visible_for_authored_subobject_directives(
                rack_mesh_index,
                &default_draw.subobject_visibility,
            ),
            "pristine Scorpion hides only the source-authored exact rack child"
        );
        assert!(
            model.mesh_visible_for_authored_subobject_directives(
                rack_mesh_index,
                &upgraded_draw.subobject_visibility,
            ),
            "upgrade state shows that same source-authored rack child"
        );
    }

    #[test]
    fn w3d_hlod_turret_retail_scorpion_retains_exact_primary_turret_binding_when_available() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let ini_path = root
            .join("windows_game/extracted_big_files/INIZH/Data/INI/Object/GC_Chem_GLAUnits.ini");
        let Some(w3d_path) = crate::assets::mesh_asset_resolve::find_filesystem_w3d("UVLiteTank")
        else {
            eprintln!("skip: retail UVLiteTank.W3D is not available on disk");
            return;
        };
        let Ok(ini_content) = std::fs::read_to_string(&ini_path) else {
            eprintln!(
                "skip: retail Scorpion Object INI is not available at {}",
                ini_path.display()
            );
            return;
        };

        let mut parser = crate::assets::IniParser::new();
        parser
            .parse_ini_content(&ini_content, "GC_Chem_GLAUnits.ini")
            .expect("parse retail Chem Scorpion Draw states");
        let scorpion = parser
            .get_definition("GC_Chem_GLATankScorpion")
            .expect("retail Chem Scorpion definition");
        let draw = scorpion
            .select_draw_models_for_conditions(0)
            .expect("retail pristine Scorpion draw state")
            .into_iter()
            .find(|draw| draw.model_key.eq_ignore_ascii_case("UVLiteTank"))
            .expect("retail pristine Scorpion selects UVLiteTank");
        assert_eq!(draw.primary_turret.yaw_bone.as_deref(), Some("turret01"));
        assert_eq!(draw.primary_turret.pitch_bone, None);
        assert!(!draw.primary_turret.has_unsupported_alternate_turret());

        let model = W3DLoader::new()
            .load_model_from_path(&w3d_path)
            .expect("retail UVLiteTank W3D should parse");
        let (hlod, hierarchy) = model
            .rigid_hlod_context()
            .expect("retail Scorpion body must use the currently supported single HLOD path");
        let turret_pivot_index = W3DModel::primary_turret_pivot_index(hierarchy, "turret01")
            .expect("retail source Turret01 must resolve to a non-root exact HTree pivot");
        let turret_child = hlod.lods[0]
            .subobjects
            .iter()
            .find(|child| child.bone_index == turret_pivot_index as u32)
            .expect("retail source HLOD must retain a child owned by Turret01");
        let turret_mesh_index = model
            .meshes
            .iter()
            .position(|mesh| {
                mesh.container_name.eq_ignore_ascii_case(hlod.name.as_str())
                    && format!("{}.{}", mesh.container_name, mesh.name)
                        .eq_ignore_ascii_case(turret_child.name.as_str())
            })
            .expect("retail Turret01 HLOD child must map to an exact flattened Main mesh");
        assert!(
            model
                .mesh_local_transform_and_visibility_for_primary_turret(
                    turret_mesh_index,
                    None,
                    0.0,
                    &draw.primary_turret,
                    0.0,
                    0.0,
                )
                .is_some(),
            "retail Scorpion's exact Turret01 binding must carry through the active rigid-HLOD path"
        );
    }

    #[test]
    fn w3d_hlod_visibility_retail_boss_airfield_binds_redlight_bone() {
        let Some(path) = crate::assets::mesh_asset_resolve::find_filesystem_w3d("NBAirfield_DS")
        else {
            eprintln!("skip: retail NBAirfield_DS.W3D is not available on disk");
            return;
        };
        let model = W3DLoader::new()
            .load_model_from_path(&path)
            .expect("retail Boss airfield W3D should parse");
        let animation_index = model
            .find_animation_index_for_draw_identity("nbairfield_ds.nbairfield_ds")
            .expect("retail Draw Animation identity must resolve exactly");
        let animation = &model.animations[animation_index];
        assert!(
            animation
                .raw_visibility_channels
                .iter()
                .any(|channel| channel.pivot == 15
                    && channel.first_frame == 20
                    && channel.last_frame == 79),
            "retail airfield retains raw pivot-15 visibility source"
        );
        let mesh_index = model
            .meshes
            .iter()
            .position(|mesh| {
                mesh.name.eq_ignore_ascii_case("REDLIGHT06")
                    && mesh.container_name.eq_ignore_ascii_case("NBAIRFIELD_DS")
            })
            .expect("retail airfield red light retains exact source mesh identity");
        let before = model
            .mesh_local_transform_and_visibility_for_animation(
                mesh_index,
                Some(animation_index),
                0.0,
            )
            .expect("retail red light should resolve through source HLOD bone");
        let hidden = model
            .mesh_local_transform_and_visibility_for_animation(
                mesh_index,
                Some(animation_index),
                20.0,
            )
            .expect("retail red light should sample authored bit channel");
        assert!(before.1, "before FirstFrame, DefaultVal is visible");
        assert!(!hidden.1, "retail frame 20 is authored hidden for pivot 15");
    }

    #[test]
    fn sample_w3d_still_loads_via_from_path_when_present() {
        let path = crate::assets::mesh_asset_resolve::find_filesystem_w3d("AmericaCommandCenter")
            .or_else(|| crate::assets::mesh_asset_resolve::find_filesystem_w3d("ABBtCmdHQ"))
            .or_else(|| crate::assets::mesh_asset_resolve::find_filesystem_w3d("airanger_s"));
        let Some(path) = path else {
            eprintln!("skip: no sample W3D on disk");
            // Candidate list is still the archive contract when bytes are absent.
            let paths = w3d_archive_path_variants("AmericaCommandCenter");
            assert!(paths.iter().any(|p| p == "Art/W3D/ABBtCmdHQ.W3D"));
            return;
        };
        let model = W3DLoader::new()
            .load_model_from_path(&path)
            .expect("sample W3D should parse");
        assert!(
            !model.meshes.is_empty(),
            "sample W3D at {} parsed with zero meshes",
            path.display()
        );
    }
}
