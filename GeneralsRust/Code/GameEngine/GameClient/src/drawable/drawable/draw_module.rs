//! DrawModule trait, logic snapshot adapter, bone data, and terrain decals.

use super::*;
use game_engine::common::bit_flags::ModelConditionBitFlags;
use game_engine::common::system::Xfer;
use game_engine::common::system::game_common::WhichTurretType;
use game_engine::common::thing::module::Module;
use gamelogic::common::types::WeaponSlotType;
use std::collections::HashMap;

/// Terrain decal types (converted from C++ TerrainDecalType)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TerrainDecalType {
    Demoralized,
    Horde,
    HordeWithNationalism,
    HordeVehicle,
    HordeWithNationalismVehicle,
    Crate,
    HordeWithFanaticism,
    ChemSuit,
    None,
    ShadowTexture,
}

// ---------------------------------------------------------------------------
// DrawModule trait — draw dispatch interface for BasicDrawable
// ---------------------------------------------------------------------------
// PARITY_NOTE: C++ Drawable holds an array of DrawModule pointers per ThingTemplate
// and dispatches render/bone/FX queries through them via ObjectDrawInterface.
// The Rust BasicDrawable stores owned DrawModule trait objects and iterates them
// for the same queries. When the full W3D draw module system is ported,
// individual modules (W3DModelDraw, W3DTreeDraw, etc.) will implement this trait.

/// Trait for draw modules attached to a `BasicDrawable`.
///
/// C++ parity: `DrawModule` base class + `ObjectDrawInterface` for bone/FX queries.
/// Each method corresponds to a C++ dispatch loop inside `Drawable::methodName()`.
pub trait DrawModule: std::fmt::Debug + Send + Sync {
    /// Save-game tag for this module, matching C++ `Module::getModuleTagNameKey`.
    ///
    /// Modules that have not ported C++ snapshot state should leave this as `None`;
    /// `Drawable::xferDrawableModules` will omit them from the saved bucket.
    fn snapshot_module_identifier(&self) -> Option<&str> {
        None
    }

    /// Save/load this module's C++ snapshot block.
    fn xfer_snapshot(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    /// C++ drawable module bucket: 0 = draw, 1 = client update.
    fn drawable_module_type_index(&self) -> usize {
        0
    }

    /// Draw this module. C++ `DrawModule::doDrawModule(transformMtx)`.
    fn do_draw(&mut self, _transform: &Matrix4, _view: &Matrix4, _projection: &Matrix4) {}

    /// Enable/disable shadow rendering for this module.
    /// C++ `DrawModule::setShadowsEnabled(Bool)`.
    fn set_shadows_enabled(&mut self, _enable: bool) {}

    /// Allocate shadow resources if not present (Options screen).
    /// C++ `DrawModule::allocateShadows()`.
    ///
    /// Fail-closed residual: status/enable bookkeeping only — not full shadow mesh GPU alloc.
    fn allocate_shadows(&mut self) {}

    /// Release shadow resources (Options screen).
    /// C++ `DrawModule::releaseShadows()`.
    ///
    /// Fail-closed residual: does not free GPU shadow meshes that were never allocated.
    fn release_shadows(&mut self) {}

    /// C++ `DrawModule::setFullyObscuredByShroud(Bool)`.
    ///
    /// The base Drawable dispatches this only when the effective value changes.
    fn set_fully_obscured_by_shroud(&mut self, _fully_obscured: bool) {}

    /// C++ `DrawModule::setTerrainDecal`.
    fn set_terrain_decal(&mut self, _decal_type: TerrainDecalType) {}
    /// C++ `DrawModule::setTerrainDecalSize`.
    fn set_terrain_decal_size(&mut self, _x: f32, _y: f32) {}
    /// C++ `DrawModule::setTerrainDecalOpacity`.
    fn set_terrain_decal_opacity(&mut self, _opacity: f32) {}

    /// Replace the team indicator color.
    /// C++ `ObjectDrawInterface::replaceIndicatorColor(color)`.
    fn replace_indicator_color(&mut self, _color: Option<(u8, u8, u8)>) {}

    /// Hide or show this module's render object and shadow.
    /// C++ `ObjectDrawInterface::setHidden(Bool)`.
    fn set_hidden(&mut self, _hidden: bool) {}

    /// Swap condition-state models / hide-show lists / transition anims.
    /// C++ `ObjectDrawInterface::replaceModelConditionState`.
    fn replace_model_condition_state(&mut self, _flags: &ModelConditionBitFlags) {}
    /// Called after the drawable is bound to an object.
    /// C++ `DrawModule::onDrawableBoundToObject()`.
    fn on_drawable_bound_to_object(&mut self) {}

    /// Return barrel count for the given weapon slot.
    /// C++ `ObjectDrawInterface::getBarrelCount(wslot)`.
    fn get_barrel_count(&self, _wslot: WeaponSlotType) -> i32 {
        0
    }

    /// Handle weapon fire FX at the barrel position.
    /// C++ `ObjectDrawInterface::handleWeaponFireFX(wslot, barrel, fxl, speed, victimPos, radius)`.
    /// Returns true if the FX was consumed.
    fn handle_weapon_fire_fx(
        &mut self,
        _wslot: WeaponSlotType,
        _barrel: i32,
        _fx_list: Option<&FXListRef>,
        _weapon_speed: f32,
        _victim_pos: Option<&Vector3>,
        _damage_radius: f32,
    ) -> bool {
        false
    }

    /// Query pristine (unanimated) bone positions.
    /// C++ `ObjectDrawInterface::getPristineBonePositionsForConditionState(...)`.
    /// Returns number of bones found.
    fn get_pristine_bone_positions(
        &self,
        _bone_name_prefix: &str,
        _start_index: i32,
        _positions: &mut [Vector3],
        _transforms: &mut [Matrix4],
    ) -> i32 {
        0
    }

    /// Query current (animated) bone positions.
    /// C++ `ObjectDrawInterface::getCurrentBonePositions(...)`.
    /// Returns number of bones found.
    fn get_current_bone_positions(
        &self,
        _bone_name_prefix: &str,
        _start_index: i32,
        _positions: &mut [Vector3],
        _transforms: &mut [Matrix4],
    ) -> i32 {
        0
    }

    /// Query current world-space bone transform.
    /// C++ `ObjectDrawInterface::getCurrentWorldspaceClientBonePositions(...)`.
    fn get_current_worldspace_client_bone_positions(
        &self,
        _bone_name: &str,
        _transform: &mut Matrix4,
    ) -> bool {
        false
    }

    /// Get projectile launch offset from bone data.
    /// C++ `ObjectDrawInterface::getProjectileLaunchOffset(...)`.
    fn get_projectile_launch_offset(
        &self,
        _wslot: WeaponSlotType,
        _barrel: i32,
        _launch_pos: &mut Matrix4,
        _turret: WhichTurretType,
        _turret_rot_pos: &mut Vector3,
        _turret_pitch_pos: Option<&mut Vector3>,
    ) -> bool {
        false
    }
}

/// Adapts concrete GameLogic/GameEngine modules into GameClient drawable save buckets.
///
/// C++ saves drawable modules by drawable-module bucket and module tag name. The
/// Rust GameClient renderer is not yet fully backed by these logic modules,
/// so this adapter keeps the snapshot path concrete while draw dispatch remains
/// owned by the WGPU-facing client code.
pub struct LogicDrawModuleSnapshotAdapter {
    module_identifier: String,
    module_type_index: usize,
    module: Box<dyn Module>,
}

impl LogicDrawModuleSnapshotAdapter {
    pub const DRAW_MODULE_TYPE_INDEX: usize = 0;
    pub const CLIENT_UPDATE_MODULE_TYPE_INDEX: usize = 1;

    pub fn new(
        module_identifier: impl Into<String>,
        module_type_index: usize,
        module: Box<dyn Module>,
    ) -> Self {
        Self {
            module_identifier: module_identifier.into(),
            module_type_index,
            module,
        }
    }

    pub fn draw_module(module_identifier: impl Into<String>, module: Box<dyn Module>) -> Self {
        Self::new(module_identifier, Self::DRAW_MODULE_TYPE_INDEX, module)
    }

    pub fn client_update_module(
        module_identifier: impl Into<String>,
        module: Box<dyn Module>,
    ) -> Self {
        Self::new(
            module_identifier,
            Self::CLIENT_UPDATE_MODULE_TYPE_INDEX,
            module,
        )
    }
}

impl std::fmt::Debug for LogicDrawModuleSnapshotAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogicDrawModuleSnapshotAdapter")
            .field("module_identifier", &self.module_identifier)
            .field("module_type_index", &self.module_type_index)
            .finish()
    }
}

impl DrawModule for LogicDrawModuleSnapshotAdapter {
    fn snapshot_module_identifier(&self) -> Option<&str> {
        Some(&self.module_identifier)
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.module.xfer(xfer)
    }

    fn drawable_module_type_index(&self) -> usize {
        self.module_type_index
    }
}

// ---------------------------------------------------------------------------
// BoneData — stores bone positions for draw modules without W3D bone systems
// ---------------------------------------------------------------------------

/// Bone position storage for draw modules that lack a W3D HTreeClass.
///
/// PARITY_NOTE: In C++, bone data lives inside the W3D RenderObjClass → HTreeClass.
/// This struct provides the same query interface using pre-loaded data from INI.
/// When the full W3D system is ported, this will be replaced by actual HTree queries.
#[derive(Debug, Clone, Default)]
pub struct BoneData {
    /// Map from bone name prefix to ordered list of (position, transform) pairs.
    /// Index in the Vec corresponds to the bone suffix (01, 02, ...).
    pub pristine_bones: HashMap<String, Vec<(Vector3, Matrix4)>>,
    /// Animated bone positions — same layout, updated each frame.
    pub current_bones: HashMap<String, Vec<(Vector3, Matrix4)>>,
    /// World-space bone transforms — single transform per named bone.
    pub worldspace_bones: HashMap<String, Matrix4>,
    /// Per-slot barrel counts (Primary, Secondary, Tertiary).
    pub barrel_counts: [i32; 3],
}

impl BoneData {
    /// Create empty bone data with specified barrel counts.
    pub fn with_barrel_counts(primary: i32, secondary: i32, tertiary: i32) -> Self {
        Self {
            barrel_counts: [primary, secondary, tertiary],
            ..Default::default()
        }
    }

    /// Add a pristine bone entry.
    pub fn add_pristine_bone(&mut self, name: &str, position: Vector3, transform: Matrix4) {
        self.pristine_bones
            .entry(name.to_string())
            .or_default()
            .push((position, transform));
    }

    /// Add a current (animated) bone entry.
    pub fn add_current_bone(&mut self, name: &str, position: Vector3, transform: Matrix4) {
        self.current_bones
            .entry(name.to_string())
            .or_default()
            .push((position, transform));
    }

    /// Set a world-space bone transform.
    pub fn set_worldspace_bone(&mut self, name: &str, transform: Matrix4) {
        self.worldspace_bones.insert(name.to_string(), transform);
    }

    /// Query pristine bone positions matching `bone_name_prefix` starting at `start_index`.
    /// Returns count of bones written into `positions` and `transforms`.
    /// C++ parity: `ObjectDrawInterface::getPristineBonePositionsForConditionState`.
    pub fn query_pristine_bones(
        &self,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Vector3],
        transforms: &mut [Matrix4],
    ) -> i32 {
        let bones = match self.pristine_bones.get(bone_name_prefix) {
            Some(b) => b,
            None => return 0,
        };
        let start = start_index.max(0) as usize;
        if start >= bones.len() {
            return 0;
        }
        let max_write = positions.len().min(transforms.len());
        let available = &bones[start..];
        let count = available.len().min(max_write);
        for i in 0..count {
            positions[i] = available[i].0;
            transforms[i] = available[i].1;
        }
        count as i32
    }

    /// Query current (animated) bone positions matching `bone_name_prefix`.
    /// C++ parity: `ObjectDrawInterface::getCurrentBonePositions`.
    pub fn query_current_bones(
        &self,
        bone_name_prefix: &str,
        start_index: i32,
        positions: &mut [Vector3],
        transforms: &mut [Matrix4],
    ) -> i32 {
        let bones = match self.current_bones.get(bone_name_prefix) {
            Some(b) => b,
            None => return 0,
        };
        let start = start_index.max(0) as usize;
        if start >= bones.len() {
            return 0;
        }
        let max_write = positions.len().min(transforms.len());
        let available = &bones[start..];
        let count = available.len().min(max_write);
        for i in 0..count {
            positions[i] = available[i].0;
            transforms[i] = available[i].1;
        }
        count as i32
    }

    /// Query world-space bone transform.
    /// C++ parity: `ObjectDrawInterface::getCurrentWorldspaceClientBonePositions`.
    pub fn query_worldspace_bone(&self, bone_name: &str, transform: &mut Matrix4) -> bool {
        match self.worldspace_bones.get(bone_name) {
            Some(t) => {
                *transform = *t;
                true
            }
            None => false,
        }
    }

    /// Get barrel count for a weapon slot.
    pub fn barrel_count_for_slot(&self, wslot: WeaponSlotType) -> i32 {
        match wslot {
            WeaponSlotType::Primary => self.barrel_counts[0],
            WeaponSlotType::Secondary => self.barrel_counts[1],
            WeaponSlotType::Tertiary => self.barrel_counts[2],
        }
    }
}

/// Placeholder type for FXList references in draw module dispatch.
///
/// PARITY_NOTE: C++ passes `const FXList*` through `handleWeaponFireFX`.
/// The actual FXList system lives in `crate::fx_list`. This type alias
/// provides a named reference for the draw module trait without pulling
/// in the full FXList type (which depends on gamelogic Coord3D/Matrix3D).
/// When the W3D draw module system is fully ported, this will be replaced
/// by the real FXList reference.
pub type FXListRef = str;
