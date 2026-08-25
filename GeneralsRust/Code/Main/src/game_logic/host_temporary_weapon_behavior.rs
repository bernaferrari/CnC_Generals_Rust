//! Typed source contracts for C++ `FireWeaponWhenDamagedBehavior` and
//! `FireWeaponWhenDeadBehavior`.
//!
//! This is intentionally a data/runtime-state foundation, not a second live
//! firing path.  The existing host residuals still own their explicitly
//! bounded heuristics.  C++ owns eight *independent* `Weapon` instances for
//! every `FireWeaponWhenDamagedBehavior`; those instances carry clip,
//! cooldown, barrel, projectile-stream, and SuspendFX state and are Xferred
//! independently.  Main must not activate these metadata records until an
//! Object snapshot tail can preserve that state in the exact C++ order.
//!
//! C++ references:
//! - `FireWeaponWhenDamagedBehavior.{h,cpp}`
//! - `FireWeaponWhenDeadBehavior.{h,cpp}`
//! - `Weapon::xfer` (`Weapon.cpp`, version 3)
//! - `UpgradeMux` / `DieMuxData`

use crate::assets::BehaviorModuleDefinition;
use crate::game_logic::ObjectId;
use crate::game_logic::host_enum_table_residual::{
    DAMAGE_NUM_TYPES, DEATH_NUM_TYPES, OBJECT_STATUS_COUNT, VETERANCY_LEVEL_COUNT,
    damage_type_bit_name_index, death_type_name_index, object_status_bit_name_index,
    veterancy_level_name_index,
};
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use serde::{Deserialize, Serialize};

/// C++ `FireWeaponWhenDamagedBehavior::xfer` version.
pub const FIRE_WEAPON_WHEN_DAMAGED_XFER_VERSION: u8 = 1;
/// C++ `FireWeaponWhenDeadBehavior::xfer` version.
pub const FIRE_WEAPON_WHEN_DEAD_XFER_VERSION: u8 = 1;
/// C++ `Weapon::xfer` version used by each retained damaged-behavior weapon.
pub const TEMPORARY_WEAPON_XFER_VERSION: u8 = 3;
/// C++ `NO_MAX_SHOTS_LIMIT` (`Weapon.h`).
pub const TEMPORARY_WEAPON_NO_MAX_SHOTS_LIMIT: i32 = i32::MAX;

const fn low_mask_u64(bits: u32) -> u64 {
    if bits >= 64 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

const fn low_mask_u32(bits: u32) -> u32 {
    if bits >= 32 {
        u32::MAX
    } else {
        (1u32 << bits) - 1
    }
}

const fn low_mask_u8(bits: u32) -> u8 {
    if bits >= 8 {
        u8::MAX
    } else {
        ((1u16 << bits) - 1) as u8
    }
}

/// C++ `DamageTypeFlags` as used only by this module data.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FireWeaponDamageTypeMask(pub u64);

impl FireWeaponDamageTypeMask {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(low_mask_u64(DAMAGE_NUM_TYPES));

    #[inline]
    pub const fn contains_ordinal(self, ordinal: u32) -> bool {
        ordinal < DAMAGE_NUM_TYPES && (self.0 & (1u64 << ordinal)) != 0
    }
}

impl Default for FireWeaponDamageTypeMask {
    fn default() -> Self {
        Self::ALL
    }
}

/// C++ `DeathTypeFlags` as used by `DieMuxData`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FireWeaponDeathTypeMask(pub u32);

impl FireWeaponDeathTypeMask {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(low_mask_u32(DEATH_NUM_TYPES));

    #[inline]
    pub const fn contains_ordinal(self, ordinal: u32) -> bool {
        ordinal < DEATH_NUM_TYPES && (self.0 & (1u32 << ordinal)) != 0
    }
}

impl Default for FireWeaponDeathTypeMask {
    fn default() -> Self {
        Self::ALL
    }
}

/// C++ `VeterancyLevelFlags` as used by `DieMuxData`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FireWeaponVeterancyLevelMask(pub u8);

impl FireWeaponVeterancyLevelMask {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(low_mask_u8(VETERANCY_LEVEL_COUNT));

    #[inline]
    pub const fn contains_ordinal(self, ordinal: u32) -> bool {
        ordinal < VETERANCY_LEVEL_COUNT && (self.0 & (1u8 << ordinal)) != 0
    }
}

impl Default for FireWeaponVeterancyLevelMask {
    fn default() -> Self {
        Self::ALL
    }
}

/// C++ `ObjectStatusMaskType` as used by `DieMuxData`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FireWeaponObjectStatusMask(pub u64);

impl FireWeaponObjectStatusMask {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(low_mask_u64(OBJECT_STATUS_COUNT));

    #[inline]
    pub const fn contains_ordinal(self, ordinal: u32) -> bool {
        ordinal < OBJECT_STATUS_COUNT && (self.0 & (1u64 << ordinal)) != 0
    }

    #[inline]
    pub const fn intersects(self, object_statuses: u64) -> bool {
        (self.0 & object_statuses) != 0
    }

    #[inline]
    pub const fn is_subset_of(self, object_statuses: u64) -> bool {
        self.0 == 0 || (self.0 & object_statuses) == self.0
    }
}

/// One source-authored `UpgradeMuxData` payload.  Upgrade names intentionally
/// remain names here: Main has no global UpgradeStore mask allocation at the
/// template parser boundary, and resolving them earlier would make inherited
/// Object INI data depend on load order.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FireWeaponUpgradeMuxMetadata {
    /// C++ `TriggeredBy` / `m_activationUpgradeNames`, in source token order.
    pub triggered_by: Vec<String>,
    /// C++ `ConflictsWith` / `m_conflictingUpgradeNames`.
    pub conflicts_with: Vec<String>,
    /// C++ `RemovesUpgrades` / `m_removalUpgradeNames`.
    pub removes_upgrades: Vec<String>,
    /// C++ `FXListUpgrade`, retained as a source reference.  `None` covers
    /// both an omitted field and C++ `FXListUpgrade = None`.
    pub fx_list_upgrade: Option<String>,
    /// C++ `RequiresAllTriggers`, whose constructor default is false.
    pub requires_all_triggers: bool,
}

impl FireWeaponUpgradeMuxMetadata {
    fn owns_named_upgrade(owned: &[&str], name: &str) -> bool {
        owned.iter().any(|tag| tag.eq_ignore_ascii_case(name))
    }

    /// C++ `UpgradeMux::attemptUpgrade` / `wouldUpgrade` TriggeredBy check.
    /// `StartsActive` is handled by the caller via `upgrade_executed`.
    pub fn triggered_by_owned(&self, owned: &[&str]) -> bool {
        if self.triggered_by.is_empty() {
            return false;
        }
        if self.requires_all_triggers {
            self.triggered_by
                .iter()
                .all(|need| Self::owns_named_upgrade(owned, need))
        } else {
            self.triggered_by
                .iter()
                .any(|need| Self::owns_named_upgrade(owned, need))
        }
    }

    /// C++ `FireWeaponWhenDeadBehavior::onDie` lines 81-88:
    /// ConflictsWith skips only when the object or player *owns* a conflicting upgrade.
    pub fn conflicts_with_owned(&self, owned: &[&str]) -> bool {
        !self.conflicts_with.is_empty()
            && self
                .conflicts_with
                .iter()
                .any(|need| Self::owns_named_upgrade(owned, need))
    }
}

/// The four C++ `BodyDamageType` choices used by this behavior.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FireWeaponBodyDamageState {
    Pristine = 0,
    Damaged = 1,
    ReallyDamaged = 2,
    Rubble = 3,
}

impl FireWeaponBodyDamageState {
    pub const ALL: [Self; 4] = [
        Self::Pristine,
        Self::Damaged,
        Self::ReallyDamaged,
        Self::Rubble,
    ];
}

/// The exact C++ field and Xfer ordering of a damaged-behavior's eight owned
/// `Weapon*` values.  Do not collapse these into an ordinary PRIMARY slot:
/// two source fields naming the same WeaponTemplate still own separate clip,
/// reload, barrel, and projectile-stream state.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FireWeaponWhenDamagedWeaponRole {
    ReactionPristine = 0,
    ReactionDamaged = 1,
    ReactionReallyDamaged = 2,
    ReactionRubble = 3,
    ContinuousPristine = 4,
    ContinuousDamaged = 5,
    ContinuousReallyDamaged = 6,
    ContinuousRubble = 7,
}

impl FireWeaponWhenDamagedWeaponRole {
    /// C++ `FireWeaponWhenDamagedBehavior::xfer` order.
    pub const XFER_ORDER: [Self; 8] = [
        Self::ReactionPristine,
        Self::ReactionDamaged,
        Self::ReactionReallyDamaged,
        Self::ReactionRubble,
        Self::ContinuousPristine,
        Self::ContinuousDamaged,
        Self::ContinuousReallyDamaged,
        Self::ContinuousRubble,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn body_damage_state(self) -> FireWeaponBodyDamageState {
        match self {
            Self::ReactionPristine | Self::ContinuousPristine => {
                FireWeaponBodyDamageState::Pristine
            }
            Self::ReactionDamaged | Self::ContinuousDamaged => FireWeaponBodyDamageState::Damaged,
            Self::ReactionReallyDamaged | Self::ContinuousReallyDamaged => {
                FireWeaponBodyDamageState::ReallyDamaged
            }
            Self::ReactionRubble | Self::ContinuousRubble => FireWeaponBodyDamageState::Rubble,
        }
    }

    #[inline]
    pub const fn is_continuous(self) -> bool {
        matches!(
            self,
            Self::ContinuousPristine
                | Self::ContinuousDamaged
                | Self::ContinuousReallyDamaged
                | Self::ContinuousRubble
        )
    }
}

/// Stable identity of one C++ `FireWeaponWhenDamagedBehavior` owned Weapon.
/// `module_source_index` is the declaration index in the fully inherited
/// `ObjectDefinition::behavior_modules` vector, not a template-name-derived
/// key.  That makes two same-template modules independently addressable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TemporaryWeaponRuntimeKey {
    pub module_source_index: u32,
    pub role: FireWeaponWhenDamagedWeaponRole,
}

/// C++ allocates all damaged-behavior weapons with `PRIMARY_WEAPON`, even
/// when their owner object's normal weapon set has other slots.
///
/// `Weapon::xfer` uses `xferUser(sizeof(WeaponSlotType))`; the original
/// unscoped C++ enum is a 32-bit signed value, not a compact byte.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporaryWeaponSlot {
    Primary = 0,
    Secondary = 1,
    Tertiary = 2,
    /// C++ `WEAPONSLOT_COUNT`; never selected for these temporary weapons,
    /// but retained so the raw Xfer enum domain is not narrowed.
    Count = 3,
}

impl Default for TemporaryWeaponSlot {
    fn default() -> Self {
        Self::Primary
    }
}

/// C++ `WeaponStatus` wire ordinals (`WeaponStatus.h`). `Weapon::xfer` writes
/// `sizeof(WeaponStatus)`, so retain the original 32-bit enum width even
/// though this source foundation only uses the five named values.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporaryWeaponStatus {
    ReadyToFire = 0,
    OutOfAmmo = 1,
    BetweenFiringShots = 2,
    ReloadingClip = 3,
    PreAttack = 4,
    /// C++ `WEAPON_STATUS_COUNT`; a sentinel rather than a fireable state.
    Count = 5,
}

impl Default for TemporaryWeaponStatus {
    fn default() -> Self {
        // `Weapon::Weapon` starts empty; `FireWeaponWhenDamagedBehavior`
        // subsequently calls `reloadAmmo`, not `loadAmmoNow`.
        Self::OutOfAmmo
    }
}

/// One persistent C++ damaged-behavior `Weapon` allocation before its mutable
/// state is constructed.  The source template name is retained separately
/// from the object's normal primary/secondary/tertiary weapon references.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporaryWeaponRuntimeSpec {
    pub key: TemporaryWeaponRuntimeKey,
    pub weapon_template_name: String,
    pub weapon_slot: TemporaryWeaponSlot,
}

/// Authored template values consumed by the C++ `Weapon` constructor and
/// copy/assignment operators. This is deliberately an explicit value object
/// rather than a lookup by template name: temporary behavior state must never
/// guess these fields from an object or weapon basename.
///
/// The live Main Object does not own this contract yet. A future Object
/// runtime bridge must obtain these values from the authoritative WeaponStore
/// before constructing a retained temporary Weapon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TemporaryWeaponConstructionDefaults {
    pub min_target_pitch: f32,
    pub max_target_pitch: f32,
    pub shots_per_barrel: i32,
    pub suspend_fx_delay: u32,
    /// C++ template property consulted when firing; construction itself
    /// still initializes `m_leechWeaponRangeActive` to false.
    pub leech_range_weapon: bool,
    /// C++ `WeaponTemplate::m_clipSize`, retained for the initial
    /// `FireWeaponWhenDamagedBehavior` `reloadAmmo` call.
    pub clip_size: u32,
    /// C++ `WeaponTemplate::getClipReloadTime` in logic frames.  The host
    /// temporary bridge has no active upgrade-bonus authority yet, so this
    /// is the authored store value, not a template-name approximation.
    pub clip_reload_frames: u32,
    /// Number of authored scatter target entries used to rebuild the C++
    /// `m_scatterTargetsUnused` index list on reload.
    pub scatter_target_count: u32,
}

impl Default for TemporaryWeaponConstructionDefaults {
    fn default() -> Self {
        Self {
            min_target_pitch: -std::f32::consts::PI,
            max_target_pitch: std::f32::consts::PI,
            shots_per_barrel: 1,
            suspend_fx_delay: 0,
            leech_range_weapon: false,
            clip_size: 0,
            clip_reload_frames: 0,
            scatter_target_count: 0,
        }
    }
}

/// Mutable C++ `Weapon::xfer` payload for one damaged-behavior allocation.
///
/// `key` is Rust-side ownership identity and is not an additional C++ wire
/// field.  When persistence is added, the containing behavior writes its
/// eight optional states in [`FireWeaponWhenDamagedWeaponRole::XFER_ORDER`]
/// after its `UpgradeMux` flag, matching the C++ boolean-plus-snapshot loop.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporaryWeaponRuntimeState {
    pub key: TemporaryWeaponRuntimeKey,
    /// C++ Weapon Xfer v2+ serializes the template name before the slot.
    pub weapon_template_name: String,
    pub weapon_slot: TemporaryWeaponSlot,
    pub status: TemporaryWeaponStatus,
    pub ammo_in_clip: u32,
    pub when_we_can_fire_again: u32,
    pub when_pre_attack_finished: u32,
    pub when_last_reload_started: u32,
    pub last_fire_frame: u32,
    /// C++ Weapon Xfer v3+ field.  It is not a rendering-only shortcut: the
    /// firing weapon's SuspendFX behavior must survive a save/load boundary.
    pub suspend_fx_frame: u32,
    pub projectile_stream_id: ObjectId,
    /// Wire-only compatibility placeholder immediately following
    /// `projectile_stream_id` in C++ `Weapon::xfer`. C++ saves a local
    /// `laserIDUnused` initialized to `INVALID_ID` and discards it on load;
    /// it must never become live weapon ownership in Rust. A future encoder
    /// therefore writes `INVALID_OBJECT_ID` here, while a decoder consumes
    /// the archived value before continuing with `max_shot_count`.
    pub laser_object_id_unused: ObjectId,
    pub max_shot_count: i32,
    pub current_barrel: i32,
    pub num_shots_for_current_barrel: i32,
    pub scatter_targets_unused: Vec<i32>,
    pub pitch_limited: bool,
    pub leech_weapon_range_active: bool,
}

impl TemporaryWeaponRuntimeState {
    /// Construct the mutable state initialized by C++ `Weapon::Weapon`.
    ///
    /// This mirrors `Weapon.cpp:1724-1743`: a new Weapon starts empty, uses
    /// the supplied slot, derives pitch limitation from the authored pitch
    /// window, initializes barrel cadence from `ShotsPerBarrel`, and schedules
    /// SuspendFX from the current logic frame. No reload or fire side effect
    /// occurs here; those belong to the eventual live Object/WeaponStore
    /// bridge.
    pub fn from_cxx_constructor(
        spec: &TemporaryWeaponRuntimeSpec,
        defaults: TemporaryWeaponConstructionDefaults,
        logic_frame: u32,
    ) -> Self {
        Self {
            key: spec.key,
            weapon_template_name: spec.weapon_template_name.clone(),
            weapon_slot: spec.weapon_slot,
            status: TemporaryWeaponStatus::OutOfAmmo,
            ammo_in_clip: 0,
            when_we_can_fire_again: 0,
            when_pre_attack_finished: 0,
            when_last_reload_started: 0,
            last_fire_frame: 0,
            // C++ UnsignedInt addition wraps on overflow.
            suspend_fx_frame: logic_frame.wrapping_add(defaults.suspend_fx_delay),
            projectile_stream_id: crate::game_logic::INVALID_OBJECT_ID,
            // `Weapon::xfer` consumes this local placeholder but never stores
            // it as live state.
            laser_object_id_unused: crate::game_logic::INVALID_OBJECT_ID,
            max_shot_count: TEMPORARY_WEAPON_NO_MAX_SHOTS_LIMIT,
            current_barrel: 0,
            num_shots_for_current_barrel: defaults.shots_per_barrel,
            scatter_targets_unused: Vec::new(),
            pitch_limited: defaults.min_target_pitch > -std::f32::consts::PI
                || defaults.max_target_pitch < std::f32::consts::PI,
            leech_weapon_range_active: false,
        }
    }

    /// Mirror C++ `Weapon(const Weapon&)`.
    ///
    /// C++ copies the template/slot identity and the source's
    /// `m_suspendFXFrame`, but intentionally drops every other mutable value:
    /// copied Weapons lose ammo, cooldown, projectile ownership, barrel
    /// progress, scatter targets, and active leech state. The destination
    /// defaults are explicit so the copied template's authored fields remain
    /// available without a template-name lookup.
    pub fn from_cxx_copy(source: &Self, defaults: TemporaryWeaponConstructionDefaults) -> Self {
        // C++ copies m_template and m_wslot from `that`. The Rust ownership
        // key is likewise copied; assignment below deliberately preserves
        // the receiving owner's key because it is not a C++ wire field.
        let source_spec = TemporaryWeaponRuntimeSpec {
            key: source.key,
            weapon_template_name: source.weapon_template_name.clone(),
            weapon_slot: source.weapon_slot,
        };
        let mut copied = Self::from_cxx_constructor(&source_spec, defaults, 0);
        copied.suspend_fx_frame = source.suspend_fx_frame;
        copied
    }

    /// Mirror C++ `Weapon::operator=`.
    ///
    /// Assignment has the same reset semantics as copy construction and
    /// preserves only the source SuspendFX frame. The destination identity is
    /// supplied by its owner, never copied from another behavior role.
    pub fn assign_from_cxx(
        &mut self,
        source: &Self,
        defaults: TemporaryWeaponConstructionDefaults,
    ) {
        let destination_key = self.key;
        let mut assigned = Self::from_cxx_copy(source, defaults);
        assigned.key = destination_key;
        *self = assigned;
    }

    /// Mirror the initial `Weapon::reloadAmmo(source)` performed by
    /// `FireWeaponWhenDamagedBehavior` after construction.  This is kept
    /// separate from [`Self::from_cxx_constructor`] because the C++ copy and
    /// assignment operators deliberately do *not* reload their destination.
    pub fn reload_ammo_from_cxx(
        &mut self,
        defaults: TemporaryWeaponConstructionDefaults,
        logic_frame: u32,
    ) {
        self.ammo_in_clip = if defaults.clip_size == 0 {
            TEMPORARY_WEAPON_NO_MAX_SHOTS_LIMIT as u32
        } else {
            defaults.clip_size
        };
        self.status = TemporaryWeaponStatus::ReloadingClip;
        self.when_last_reload_started = logic_frame;
        self.when_we_can_fire_again = logic_frame.wrapping_add(defaults.clip_reload_frames);
        self.scatter_targets_unused = (0..defaults.scatter_target_count)
            .filter_map(|index| i32::try_from(index).ok())
            .collect();
    }

    fn empty_for_key(key: TemporaryWeaponRuntimeKey) -> Self {
        Self {
            key,
            weapon_template_name: String::new(),
            weapon_slot: TemporaryWeaponSlot::Primary,
            status: TemporaryWeaponStatus::OutOfAmmo,
            ammo_in_clip: 0,
            when_we_can_fire_again: 0,
            when_pre_attack_finished: 0,
            when_last_reload_started: 0,
            last_fire_frame: 0,
            suspend_fx_frame: 0,
            projectile_stream_id: crate::game_logic::INVALID_OBJECT_ID,
            laser_object_id_unused: crate::game_logic::INVALID_OBJECT_ID,
            max_shot_count: TEMPORARY_WEAPON_NO_MAX_SHOTS_LIMIT,
            current_barrel: 0,
            num_shots_for_current_barrel: 0,
            scatter_targets_unused: Vec::new(),
            pitch_limited: false,
            leech_weapon_range_active: false,
        }
    }

    /// Wire only the C++ `Weapon::xfer` payload.  The Rust ownership key is
    /// supplied by the containing behavior's fixed role order and is never
    /// serialized as an extra C++ Weapon field.
    fn xfer_payload(
        &mut self,
        xfer: &mut dyn Xfer,
        key: TemporaryWeaponRuntimeKey,
    ) -> SaveLoadResult<()> {
        const CURRENT_VERSION: crate::save_load::XferVersion = TEMPORARY_WEAPON_XFER_VERSION;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)?;

        if version >= 2 {
            xfer.xfer_marker_label("WeaponTemplateName")?;
            self.weapon_template_name.xfer(xfer)?;
        }

        xfer.xfer_marker_label("WeaponSlot")?;
        let mut slot = self.weapon_slot as i32;
        xfer.xfer_i32(&mut slot)?;
        self.weapon_slot = match slot {
            0 => TemporaryWeaponSlot::Primary,
            1 => TemporaryWeaponSlot::Secondary,
            2 => TemporaryWeaponSlot::Tertiary,
            3 => TemporaryWeaponSlot::Count,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid temporary Weapon slot {other}"
                )));
            }
        };

        xfer.xfer_marker_label("WeaponStatus")?;
        let mut status = self.status as i32;
        xfer.xfer_i32(&mut status)?;
        self.status = match status {
            0 => TemporaryWeaponStatus::ReadyToFire,
            1 => TemporaryWeaponStatus::OutOfAmmo,
            2 => TemporaryWeaponStatus::BetweenFiringShots,
            3 => TemporaryWeaponStatus::ReloadingClip,
            4 => TemporaryWeaponStatus::PreAttack,
            5 => TemporaryWeaponStatus::Count,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid temporary Weapon status {other}"
                )));
            }
        };
        xfer.xfer_marker_label("AmmoInClip")?;
        self.ammo_in_clip.xfer(xfer)?;
        xfer.xfer_marker_label("WhenWeCanFireAgain")?;
        self.when_we_can_fire_again.xfer(xfer)?;
        xfer.xfer_marker_label("WhenPreAttackFinished")?;
        self.when_pre_attack_finished.xfer(xfer)?;
        xfer.xfer_marker_label("WhenLastReloadStarted")?;
        self.when_last_reload_started.xfer(xfer)?;
        xfer.xfer_marker_label("LastFireFrame")?;
        self.last_fire_frame.xfer(xfer)?;
        if version >= 3 {
            xfer.xfer_marker_label("SuspendFxFrame")?;
            self.suspend_fx_frame.xfer(xfer)?;
        } else if xfer.get_mode() == XferMode::Load {
            self.suspend_fx_frame = 0;
        }
        xfer.xfer_marker_label("ProjectileStreamId")?;
        self.projectile_stream_id.xfer(xfer)?;
        xfer.xfer_marker_label("LaserObjectIdUnused")?;
        let mut laser_object_id_unused = crate::game_logic::INVALID_OBJECT_ID;
        laser_object_id_unused.xfer(xfer)?;
        self.laser_object_id_unused = crate::game_logic::INVALID_OBJECT_ID;
        xfer.xfer_marker_label("MaxShotCount")?;
        self.max_shot_count.xfer(xfer)?;
        xfer.xfer_marker_label("CurrentBarrel")?;
        self.current_barrel.xfer(xfer)?;
        xfer.xfer_marker_label("NumShotsForCurrentBarrel")?;
        self.num_shots_for_current_barrel.xfer(xfer)?;
        xfer.xfer_marker_label("ScatterTargetsUnused")?;
        let mut scatter_count = u16::try_from(self.scatter_targets_unused.len()).map_err(|_| {
            SaveLoadError::Corrupted(
                "Temporary Weapon scatter target count exceeds C++ UnsignedShort".to_string(),
            )
        })?;
        xfer.xfer_u16(&mut scatter_count)?;
        if xfer.get_mode() == XferMode::Load {
            self.scatter_targets_unused.clear();
            self.scatter_targets_unused.reserve(scatter_count as usize);
            for _ in 0..scatter_count {
                let mut target = 0i32;
                target.xfer(xfer)?;
                self.scatter_targets_unused.push(target);
            }
        } else {
            for target in &mut self.scatter_targets_unused {
                target.xfer(xfer)?;
            }
        }
        xfer.xfer_marker_label("PitchLimited")?;
        self.pitch_limited.xfer(xfer)?;
        xfer.xfer_marker_label("LeechWeaponRangeActive")?;
        self.leech_weapon_range_active.xfer(xfer)?;
        self.key = key;
        Ok(())
    }

    /// A runtime state must stay paired to its exact allocation spec.  A
    /// future loader uses this to reject mismatched template/role data rather
    /// than silently transferring a cooldown into another temporary weapon.
    #[inline]
    pub fn matches_spec(&self, spec: &TemporaryWeaponRuntimeSpec) -> bool {
        self.key == spec.key
            && self
                .weapon_template_name
                .eq_ignore_ascii_case(&spec.weapon_template_name)
            && self.weapon_slot == spec.weapon_slot
    }
}

impl XferData for TemporaryWeaponRuntimeState {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let key = self.key;
        self.xfer_payload(xfer, key)
    }
}

/// Per-behavior mutable state required by C++ `FireWeaponWhenDamagedBehavior`.
/// It is intentionally not yet a field of Main's `Object` or `ObjectSnapshot`.
/// The current snapshot schema has no behavior-runtime tail; adding one here
/// would lose state on load or collide with the independently evolving object
/// snapshot version.  This type establishes the exact future ownership shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FireWeaponWhenDamagedRuntimeState {
    pub module_source_index: u32,
    /// C++ `UpdateModule::m_nextCallFrameAndPhase`, written by the base
    /// `UpdateModule::xfer` after its base-version chain and before the
    /// behavior's `UpgradeMux` state. The two low phase bits are deliberately
    /// retained verbatim; C++ repairs them to the module's update phase only
    /// while loading the concrete runtime module.
    pub next_call_frame_and_phase: u32,
    /// C++ `UpgradeMux::m_upgradeExecuted`, Xferred before all eight weapons.
    pub upgrade_executed: bool,
    /// Positional C++ Xfer slots in `FireWeaponWhenDamagedWeaponRole::XFER_ORDER`.
    pub weapons: [Option<TemporaryWeaponRuntimeState>; 8],
}

impl Default for FireWeaponWhenDamagedRuntimeState {
    fn default() -> Self {
        Self {
            module_source_index: 0,
            next_call_frame_and_phase: 0,
            upgrade_executed: false,
            weapons: std::array::from_fn(|_| None),
        }
    }
}

impl FireWeaponWhenDamagedRuntimeState {
    /// Ensure a future snapshot loader cannot attach a behavior state to a
    /// different inherited source module merely because both declarations
    /// happen to reference the same temporary weapon template.
    #[inline]
    pub fn belongs_to_metadata(&self, metadata: &FireWeaponWhenDamagedMetadata) -> bool {
        self.module_source_index == metadata.module_source_index
    }

    #[inline]
    pub fn weapon(&self, key: TemporaryWeaponRuntimeKey) -> Option<&TemporaryWeaponRuntimeState> {
        (key.module_source_index == self.module_source_index)
            .then(|| self.weapons[key.role.index()].as_ref())
            .flatten()
            .filter(|state| state.key == key)
    }

    #[inline]
    pub fn weapon_mut(
        &mut self,
        key: TemporaryWeaponRuntimeKey,
    ) -> Option<&mut TemporaryWeaponRuntimeState> {
        (key.module_source_index == self.module_source_index)
            .then(|| self.weapons[key.role.index()].as_mut())
            .flatten()
            .filter(|state| state.key == key)
    }

    /// Replace exactly one owned weapon state.  A key for another behavior is
    /// rejected instead of being redirected into this behavior's PRIMARY slot.
    pub fn replace_weapon_state(&mut self, state: TemporaryWeaponRuntimeState) -> bool {
        if state.key.module_source_index != self.module_source_index {
            return false;
        }
        let role_index = state.key.role.index();
        self.weapons[role_index] = Some(state);
        true
    }
}

/// Source metadata for one `FireWeaponWhenDamagedBehavior` declaration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FireWeaponWhenDamagedMetadata {
    pub module_source_index: u32,
    pub module_tag: Option<String>,
    pub starts_active: bool,
    pub damage_types: FireWeaponDamageTypeMask,
    pub damage_amount: f32,
    pub upgrade_mux: FireWeaponUpgradeMuxMetadata,
    /// Exact C++ field ordering, indexed by
    /// [`FireWeaponWhenDamagedWeaponRole::XFER_ORDER`].
    pub weapon_template_names: [Option<String>; 8],
}

impl FireWeaponWhenDamagedMetadata {
    #[inline]
    pub fn weapon_template_name(&self, role: FireWeaponWhenDamagedWeaponRole) -> Option<&str> {
        self.weapon_template_names[role.index()].as_deref()
    }

    /// Build one spec for every C++-allocated damaged-behavior Weapon.  A
    /// shared source WeaponTemplate still produces multiple distinct keys.
    pub fn runtime_specs(&self) -> Vec<TemporaryWeaponRuntimeSpec> {
        FireWeaponWhenDamagedWeaponRole::XFER_ORDER
            .into_iter()
            .filter_map(|role| {
                self.weapon_template_name(role).map(|weapon_template_name| {
                    TemporaryWeaponRuntimeSpec {
                        key: TemporaryWeaponRuntimeKey {
                            module_source_index: self.module_source_index,
                            role,
                        },
                        weapon_template_name: weapon_template_name.to_string(),
                        weapon_slot: TemporaryWeaponSlot::Primary,
                    }
                })
            })
            .collect()
    }
}

/// Source metadata for one `FireWeaponWhenDeadBehavior` declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FireWeaponWhenDeadMetadata {
    pub module_source_index: u32,
    pub module_tag: Option<String>,
    pub starts_active: bool,
    pub death_weapon: Option<String>,
    pub upgrade_mux: FireWeaponUpgradeMuxMetadata,
    pub death_types: FireWeaponDeathTypeMask,
    pub veterancy_levels: FireWeaponVeterancyLevelMask,
    pub exempt_status: FireWeaponObjectStatusMask,
    pub required_status: FireWeaponObjectStatusMask,
}

/// Mutable C++ `UpgradeMux` state for one `FireWeaponWhenDeadBehavior`.
/// There is deliberately no persistent Weapon payload alongside it:
/// `createAndFireTempWeapon` allocates, loads, fires, and deletes a fresh
/// PRIMARY Weapon in the same death callback.  Like the damaged-behavior
/// state, this remains unattached until Object snapshot schema ownership is
/// explicitly versioned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FireWeaponWhenDeadRuntimeState {
    pub module_source_index: u32,
    /// C++ `UpgradeMux::m_upgradeExecuted`, the only behavior-owned mutable
    /// state written by `FireWeaponWhenDeadBehavior::xfer` after base data.
    pub upgrade_executed: bool,
}

impl FireWeaponWhenDeadRuntimeState {
    #[inline]
    pub fn belongs_to_metadata(&self, metadata: &FireWeaponWhenDeadMetadata) -> bool {
        self.module_source_index == metadata.module_source_index
    }
}

impl FireWeaponWhenDeadMetadata {
    /// Exact pure `DieMuxData::isDieApplicable` gate.  C++ also checks
    /// `UNDER_CONSTRUCTION` and conflicting completed upgrades in `onDie`;
    /// those object/player-dependent gates intentionally remain outside this
    /// source record until live behavior activation is snapshot-safe.
    #[inline]
    pub const fn die_mux_allows(
        &self,
        death_type_ordinal: u32,
        veterancy_level_ordinal: u32,
        object_statuses: u64,
    ) -> bool {
        self.death_types.contains_ordinal(death_type_ordinal)
            && self
                .veterancy_levels
                .contains_ordinal(veterancy_level_ordinal)
            && !self.exempt_status.intersects(object_statuses)
            && self.required_status.is_subset_of(object_statuses)
    }

    /// C++ `TheWeaponStore::createAndFireTempWeapon` creates a fresh PRIMARY
    /// Weapon for every qualifying death, calls `loadAmmoNow`, fires it, then
    /// deletes it.  It deliberately has no persistent runtime state/Xfer slot.
    pub fn ephemeral_weapon_spec(&self) -> Option<FireWeaponWhenDeadEphemeralWeaponSpec> {
        self.death_weapon.as_deref().map(|weapon_template_name| {
            FireWeaponWhenDeadEphemeralWeaponSpec {
                module_source_index: self.module_source_index,
                weapon_template_name: weapon_template_name.to_string(),
                weapon_slot: TemporaryWeaponSlot::Primary,
            }
        })
    }
}

/// One C++ `createAndFireTempWeapon` request.  Unlike
/// [`TemporaryWeaponRuntimeSpec`], this has no durable Weapon instance and
/// therefore must never be appended to a save snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FireWeaponWhenDeadEphemeralWeaponSpec {
    pub module_source_index: u32,
    pub weapon_template_name: String,
    pub weapon_slot: TemporaryWeaponSlot,
}

/// Object-owned temporary-behavior runtime.  The vectors are declaration
/// ordered, matching C++'s module ownership order; each damaged behavior
/// retains eight independent PRIMARY `Weapon` payloads and each dead behavior
/// retains only its `UpgradeMux` bit because its PRIMARY weapon is created,
/// fired, and deleted inside `onDie`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporaryWeaponRuntimeBundle {
    pub damaged: Vec<FireWeaponWhenDamagedRuntimeState>,
    pub dead: Vec<FireWeaponWhenDeadRuntimeState>,
}

impl TemporaryWeaponRuntimeBundle {
    #[inline]
    pub fn has_behavior_modules(&self) -> bool {
        !self.damaged.is_empty() || !self.dead.is_empty()
    }

    /// Construct the exact inactive runtime records owned by an Object.
    /// Missing WeaponStore templates fail closed for that individual C++
    /// pointer, just as `findWeaponTemplate("None")` yields null; no
    /// template-name or object-kind fallback is permitted here.
    pub fn from_thing_template(
        template: &crate::game_logic::ThingTemplate,
        logic_frame: u32,
    ) -> Self {
        let damaged = template
            .fire_weapon_when_damaged_behaviors
            .iter()
            .map(|metadata| {
                let mut runtime = FireWeaponWhenDamagedRuntimeState {
                    module_source_index: metadata.module_source_index,
                    upgrade_executed: metadata.starts_active,
                    ..Default::default()
                };
                for spec in metadata.runtime_specs() {
                    let Some((defaults, _template_name)) =
                        temporary_weapon_defaults_for_name(&spec.weapon_template_name)
                    else {
                        continue;
                    };
                    let mut state = TemporaryWeaponRuntimeState::from_cxx_constructor(
                        &spec,
                        defaults,
                        logic_frame,
                    );
                    state.reload_ammo_from_cxx(defaults, logic_frame);
                    let _ = runtime.replace_weapon_state(state);
                }
                runtime
            })
            .collect();

        let dead = template
            .fire_weapon_when_dead_behaviors
            .iter()
            .map(|metadata| FireWeaponWhenDeadRuntimeState {
                module_source_index: metadata.module_source_index,
                // C++ `StartsActive = Yes` executes the UpgradeMux on create.
                upgrade_executed: metadata.starts_active,
            })
            .collect();

        Self { damaged, dead }
    }

    /// Validate a loaded tail against the restored template's source-ordered
    /// behavior metadata.  This prevents a same-template temporary weapon or
    /// inherited module from receiving another module's cooldown/barrel state.
    pub fn matches_thing_template(&self, template: &crate::game_logic::ThingTemplate) -> bool {
        if self.damaged.len() != template.fire_weapon_when_damaged_behaviors.len()
            || self.dead.len() != template.fire_weapon_when_dead_behaviors.len()
        {
            return false;
        }

        self.damaged
            .iter()
            .zip(&template.fire_weapon_when_damaged_behaviors)
            .all(|(runtime, metadata)| {
                runtime.belongs_to_metadata(metadata)
                    && FireWeaponWhenDamagedWeaponRole::XFER_ORDER
                        .into_iter()
                        .enumerate()
                        .all(|(index, role)| {
                            runtime.weapons[index].as_ref().is_none_or(|state| {
                                state.matches_spec(&TemporaryWeaponRuntimeSpec {
                                    key: TemporaryWeaponRuntimeKey {
                                        module_source_index: metadata.module_source_index,
                                        role,
                                    },
                                    weapon_template_name: metadata
                                        .weapon_template_name(role)
                                        .unwrap_or_default()
                                        .to_string(),
                                    weapon_slot: TemporaryWeaponSlot::Primary,
                                })
                            })
                        })
            })
            && self
                .dead
                .iter()
                .zip(&template.fire_weapon_when_dead_behaviors)
                .all(|(runtime, metadata)| runtime.belongs_to_metadata(metadata))
    }
}

impl XferData for FireWeaponWhenDamagedRuntimeState {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        const CURRENT_VERSION: crate::save_load::XferVersion =
            FIRE_WEAPON_WHEN_DAMAGED_XFER_VERSION;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)?;
        if version == 0 {
            return Err(SaveLoadError::Corrupted(
                "Invalid FireWeaponWhenDamagedBehavior Xfer version 0".to_string(),
            ));
        }

        // `module_source_index` is a Rust-side source identity envelope.  It
        // is outside the C++ behavior body, whose fixed declaration order is
        // represented by the surrounding bundle/vector.
        xfer.xfer_marker_label("ModuleSourceIndex")?;
        self.module_source_index.xfer(xfer)?;
        xfer.xfer_marker_label("NextCallFrameAndPhase")?;
        self.next_call_frame_and_phase.xfer(xfer)?;
        xfer.xfer_marker_label("UpgradeExecuted")?;
        self.upgrade_executed.xfer(xfer)?;

        for role in FireWeaponWhenDamagedWeaponRole::XFER_ORDER {
            let index = role.index();
            xfer.xfer_marker_label("WeaponPresent")?;
            let mut present = self.weapons[index].is_some();
            xfer.xfer_bool(&mut present)?;
            if present {
                if self.weapons[index].is_none() {
                    self.weapons[index] = Some(TemporaryWeaponRuntimeState::empty_for_key(
                        TemporaryWeaponRuntimeKey {
                            module_source_index: self.module_source_index,
                            role,
                        },
                    ));
                }
                self.weapons[index]
                    .as_mut()
                    .expect("present temporary Weapon state")
                    .xfer_payload(
                        xfer,
                        TemporaryWeaponRuntimeKey {
                            module_source_index: self.module_source_index,
                            role,
                        },
                    )?;
            } else if xfer.get_mode() == XferMode::Load {
                self.weapons[index] = None;
            }
        }
        Ok(())
    }
}

impl XferData for FireWeaponWhenDeadRuntimeState {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        const CURRENT_VERSION: crate::save_load::XferVersion = FIRE_WEAPON_WHEN_DEAD_XFER_VERSION;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)?;
        if version == 0 {
            return Err(SaveLoadError::Corrupted(
                "Invalid FireWeaponWhenDeadBehavior Xfer version 0".to_string(),
            ));
        }
        xfer.xfer_marker_label("ModuleSourceIndex")?;
        self.module_source_index.xfer(xfer)?;
        xfer.xfer_marker_label("UpgradeExecuted")?;
        self.upgrade_executed.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for TemporaryWeaponRuntimeBundle {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        const CURRENT_VERSION: crate::save_load::XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)?;
        if version == 0 {
            return Err(SaveLoadError::Corrupted(
                "Invalid temporary Weapon runtime bundle Xfer version 0".to_string(),
            ));
        }

        xfer.xfer_marker_label("DamagedBehaviorCount")?;
        let mut damaged_count = u32::try_from(self.damaged.len()).map_err(|_| {
            SaveLoadError::Corrupted("Temporary damaged behavior count overflow".to_string())
        })?;
        xfer.xfer_u32(&mut damaged_count)?;
        if xfer.get_mode() == XferMode::Load {
            if damaged_count > 4096 {
                return Err(SaveLoadError::Corrupted(
                    "Temporary damaged behavior count is unreasonable".to_string(),
                ));
            }
            self.damaged.clear();
            self.damaged.reserve(damaged_count as usize);
            for _ in 0..damaged_count {
                let mut state = FireWeaponWhenDamagedRuntimeState::default();
                state.xfer(xfer)?;
                self.damaged.push(state);
            }
        } else {
            for state in &mut self.damaged {
                state.xfer(xfer)?;
            }
        }

        xfer.xfer_marker_label("DeadBehaviorCount")?;
        let mut dead_count = u32::try_from(self.dead.len()).map_err(|_| {
            SaveLoadError::Corrupted("Temporary dead behavior count overflow".to_string())
        })?;
        xfer.xfer_u32(&mut dead_count)?;
        if xfer.get_mode() == XferMode::Load {
            if dead_count > 4096 {
                return Err(SaveLoadError::Corrupted(
                    "Temporary dead behavior count is unreasonable".to_string(),
                ));
            }
            self.dead.clear();
            self.dead.reserve(dead_count as usize);
            for _ in 0..dead_count {
                let mut state = FireWeaponWhenDeadRuntimeState::default();
                state.xfer(xfer)?;
                self.dead.push(state);
            }
        } else {
            for state in &mut self.dead {
                state.xfer(xfer)?;
            }
        }
        Ok(())
    }
}

/// Resolve authored construction fields from the authoritative GameLogic
/// WeaponStore.  The returned name is only a diagnostic identity; all fields
/// used by construction/reload come from the store record itself.
fn temporary_weapon_defaults_for_name(
    name: &str,
) -> Option<(TemporaryWeaponConstructionDefaults, String)> {
    let template =
        gamelogic::weapon::with_weapon_store(|store| store.find_weapon_template(name).cloned())
            .ok()??;
    Some((
        TemporaryWeaponConstructionDefaults {
            min_target_pitch: template.min_target_pitch,
            max_target_pitch: template.max_target_pitch,
            shots_per_barrel: template.shots_per_barrel,
            suspend_fx_delay: template.suspend_fx_delay,
            leech_range_weapon: template.leech_range_weapon,
            clip_size: template.clip_size.max(0) as u32,
            clip_reload_frames: template.clip_reload_time.max(0) as u32,
            scatter_target_count: template.scatter_targets.len() as u32,
        },
        template.name.clone(),
    ))
}

fn source_tokens(value: &str) -> impl Iterator<Item = &str> {
    // `IniParser` has already removed ordinary inline comments.  Keeping this
    // small trim makes direct parser tests and callers with a retained `;`
    // comment behave like C++ tokenization without treating punctuation as an
    // invented upgrade/weapon name.
    value
        .split(';')
        .next()
        .unwrap_or_default()
        .split(|character: char| character.is_ascii_whitespace() || character == '=')
        .filter(|token| !token.is_empty())
}

fn first_source_token(value: &str) -> Option<&str> {
    source_tokens(value).next()
}

fn parse_cxx_bool(value: &str) -> Option<bool> {
    match first_source_token(value)?.to_ascii_lowercase().as_str() {
        "yes" => Some(true),
        "no" => Some(false),
        _ => None,
    }
}

/// Return the decimal prefix consumed by C++ `sscanf(token, "%f", ...)`.
/// `scanReal` deliberately does not require the token to end at the numeric
/// value, so a Rust `str::parse` of the whole token would be too strict for
/// source compatibility (for example, C++ accepts `7.5f`). Hexadecimal
/// floating literals are outside this bounded source parser because they do
/// not occur in the GeneralsMD Object INI data.
fn cxx_decimal_real_prefix(token: &str) -> Option<&str> {
    let bytes = token.as_bytes();
    let mut index = 0;
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        index += 1;
    }

    let integer_start = index;
    while matches!(bytes.get(index), Some(byte) if byte.is_ascii_digit()) {
        index += 1;
    }
    let mut has_digits = index != integer_start;

    if matches!(bytes.get(index), Some(b'.')) {
        index += 1;
        let fraction_start = index;
        while matches!(bytes.get(index), Some(byte) if byte.is_ascii_digit()) {
            index += 1;
        }
        has_digits |= index != fraction_start;
    }
    if !has_digits {
        return None;
    }

    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        let exponent_start = index;
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let exponent_digits_start = index;
        while matches!(bytes.get(index), Some(byte) if byte.is_ascii_digit()) {
            index += 1;
        }
        // An incomplete exponent does not form part of the matched decimal
        // prefix.  Preserve the valid mantissa instead of rejecting it.
        if index == exponent_digits_start {
            index = exponent_start;
        }
    }

    token.get(..index)
}

fn parse_cxx_real(value: &str) -> Option<f32> {
    let token = first_source_token(value)?;
    let prefix = cxx_decimal_real_prefix(token)?;
    prefix.parse::<f32>().ok()
}

fn parse_source_reference(value: &str) -> Option<Option<String>> {
    let token = first_source_token(value)?;
    Some((!token.eq_ignore_ascii_case("none")).then(|| token.to_string()))
}

fn parse_source_vector(value: &str) -> Vec<String> {
    source_tokens(value).map(str::to_string).collect()
}

fn parse_upgrade_mux(module: &BehaviorModuleDefinition) -> Option<FireWeaponUpgradeMuxMetadata> {
    let fx_list_upgrade = match module.attribute("FXListUpgrade") {
        Some(value) => parse_source_reference(value)?,
        None => None,
    };
    let requires_all_triggers = match module.attribute("RequiresAllTriggers") {
        Some(value) => parse_cxx_bool(value)?,
        None => false,
    };
    Some(FireWeaponUpgradeMuxMetadata {
        triggered_by: module
            .attribute("TriggeredBy")
            .map(parse_source_vector)
            .unwrap_or_default(),
        conflicts_with: module
            .attribute("ConflictsWith")
            .map(parse_source_vector)
            .unwrap_or_default(),
        removes_upgrades: module
            .attribute("RemovesUpgrades")
            .map(parse_source_vector)
            .unwrap_or_default(),
        fx_list_upgrade,
        requires_all_triggers,
    })
}

fn module_has_only_known_fields(module: &BehaviorModuleDefinition, known_fields: &[&str]) -> bool {
    // C++ `initFromINIMulti` rejects an unknown field in the behavior block.
    // This matters even while the record is inert: retaining a malformed
    // custom module as if it were a valid retail contract would let a future
    // runtime path gain authority from data C++ would not load.
    module.attributes.keys().all(|field| {
        known_fields
            .iter()
            .any(|known| field.eq_ignore_ascii_case(known))
    })
}

const FIRE_WEAPON_WHEN_DAMAGED_FIELDS: &[&str] = &[
    "StartsActive",
    "ReactionWeaponPristine",
    "ReactionWeaponDamaged",
    "ReactionWeaponReallyDamaged",
    "ReactionWeaponRubble",
    "ContinuousWeaponPristine",
    "ContinuousWeaponDamaged",
    "ContinuousWeaponReallyDamaged",
    "ContinuousWeaponRubble",
    "DamageTypes",
    "DamageAmount",
    "TriggeredBy",
    "ConflictsWith",
    "RemovesUpgrades",
    "FXListUpgrade",
    "RequiresAllTriggers",
];

const FIRE_WEAPON_WHEN_DEAD_FIELDS: &[&str] = &[
    "StartsActive",
    "DeathWeapon",
    "TriggeredBy",
    "ConflictsWith",
    "RemovesUpgrades",
    "FXListUpgrade",
    "RequiresAllTriggers",
    "DeathTypes",
    "VeterancyLevels",
    "ExemptStatus",
    "RequiredStatus",
];

fn parse_cxx_all_plus_minus_mask(
    value: &str,
    all_mask: u64,
    bit_count: u32,
    lookup: impl Fn(&str) -> Option<usize>,
) -> Option<u64> {
    // `INI::parseDamageTypeFlags`, `parseDeathTypeFlags`, and
    // `parseVeterancyLevelFlags` all start at ALL and accept only ALL/NONE or
    // +/- entries.  Bare names are intentionally invalid.
    let mut mask = all_mask;
    let mut found_token = false;
    for token in source_tokens(value) {
        found_token = true;
        if token.eq_ignore_ascii_case("all") {
            mask = all_mask;
            continue;
        }
        if token.eq_ignore_ascii_case("none") {
            mask = 0;
            continue;
        }
        let (add, name) = match token.as_bytes().first() {
            Some(b'+') => (true, &token[1..]),
            Some(b'-') => (false, &token[1..]),
            _ => return None,
        };
        let bit = lookup(name)?;
        if bit >= bit_count as usize {
            return None;
        }
        let flag = 1u64 << bit;
        if add {
            mask |= flag;
        } else {
            mask &= !flag;
        }
    }
    found_token.then_some(mask & all_mask)
}

fn parse_cxx_object_status_mask(value: &str) -> Option<FireWeaponObjectStatusMask> {
    // This mirrors `BitFlags<OBJECT_STATUS_COUNT>::parse`, including its
    // normal-vs-+/- mode rule.  `NONE` ends parsing after clearing, just as
    // the C++ loop breaks rather than examining later tokens.
    let mut mask = 0u64;
    let mut found_normal = false;
    let mut found_add_or_sub = false;
    for token in source_tokens(value) {
        if token.eq_ignore_ascii_case("none") {
            if found_normal || found_add_or_sub {
                return None;
            }
            mask = 0;
            break;
        }

        let (operation, name) = match token.as_bytes().first() {
            Some(b'+') => {
                if found_normal {
                    return None;
                }
                found_add_or_sub = true;
                (Some(true), &token[1..])
            }
            Some(b'-') => {
                if found_normal {
                    return None;
                }
                found_add_or_sub = true;
                (Some(false), &token[1..])
            }
            _ => {
                if found_add_or_sub {
                    return None;
                }
                if !found_normal {
                    mask = 0;
                }
                found_normal = true;
                (None, token)
            }
        };
        let bit = object_status_bit_name_index(name)?;
        if bit >= OBJECT_STATUS_COUNT as usize {
            return None;
        }
        let flag = 1u64 << bit;
        match operation {
            Some(true) | None => mask |= flag,
            Some(false) => mask &= !flag,
        }
    }
    Some(FireWeaponObjectStatusMask(
        mask & FireWeaponObjectStatusMask::ALL.0,
    ))
}

/// Parse exactly one retained `FireWeaponWhenDamagedBehavior` source module.
/// `None` means either a nonmatching class or malformed fields.  The caller
/// intentionally fails closed for malformed metadata rather than letting a
/// template-name residual manufacture live authority.
pub fn parse_fire_weapon_when_damaged_metadata(
    module: &BehaviorModuleDefinition,
    source_index: usize,
) -> Option<FireWeaponWhenDamagedMetadata> {
    if !module
        .class_name
        .eq_ignore_ascii_case("FireWeaponWhenDamagedBehavior")
    {
        return None;
    }
    if !module_has_only_known_fields(module, FIRE_WEAPON_WHEN_DAMAGED_FIELDS) {
        return None;
    }

    let module_source_index = u32::try_from(source_index).ok()?;
    let starts_active = match module.attribute("StartsActive") {
        Some(value) => parse_cxx_bool(value)?,
        None => false,
    };
    let damage_types = match module.attribute("DamageTypes") {
        Some(value) => FireWeaponDamageTypeMask(parse_cxx_all_plus_minus_mask(
            value,
            FireWeaponDamageTypeMask::ALL.0,
            DAMAGE_NUM_TYPES,
            damage_type_bit_name_index,
        )?),
        None => FireWeaponDamageTypeMask::ALL,
    };
    let damage_amount = match module.attribute("DamageAmount") {
        Some(value) => parse_cxx_real(value)?,
        None => 0.0,
    };

    let mut weapon_template_names: [Option<String>; 8] = std::array::from_fn(|_| None);
    let fields = [
        (
            FireWeaponWhenDamagedWeaponRole::ReactionPristine,
            "ReactionWeaponPristine",
        ),
        (
            FireWeaponWhenDamagedWeaponRole::ReactionDamaged,
            "ReactionWeaponDamaged",
        ),
        (
            FireWeaponWhenDamagedWeaponRole::ReactionReallyDamaged,
            "ReactionWeaponReallyDamaged",
        ),
        (
            FireWeaponWhenDamagedWeaponRole::ReactionRubble,
            "ReactionWeaponRubble",
        ),
        (
            FireWeaponWhenDamagedWeaponRole::ContinuousPristine,
            "ContinuousWeaponPristine",
        ),
        (
            FireWeaponWhenDamagedWeaponRole::ContinuousDamaged,
            "ContinuousWeaponDamaged",
        ),
        (
            FireWeaponWhenDamagedWeaponRole::ContinuousReallyDamaged,
            "ContinuousWeaponReallyDamaged",
        ),
        (
            FireWeaponWhenDamagedWeaponRole::ContinuousRubble,
            "ContinuousWeaponRubble",
        ),
    ];
    for (role, field) in fields {
        weapon_template_names[role.index()] = match module.attribute(field) {
            Some(value) => parse_source_reference(value)?,
            None => None,
        };
    }

    Some(FireWeaponWhenDamagedMetadata {
        module_source_index,
        module_tag: module.module_tag.clone(),
        starts_active,
        damage_types,
        damage_amount,
        upgrade_mux: parse_upgrade_mux(module)?,
        weapon_template_names,
    })
}

/// Parse exactly one retained `FireWeaponWhenDeadBehavior` source module.
/// See [`parse_fire_weapon_when_damaged_metadata`] for malformed-field policy.
pub fn parse_fire_weapon_when_dead_metadata(
    module: &BehaviorModuleDefinition,
    source_index: usize,
) -> Option<FireWeaponWhenDeadMetadata> {
    if !module
        .class_name
        .eq_ignore_ascii_case("FireWeaponWhenDeadBehavior")
    {
        return None;
    }
    if !module_has_only_known_fields(module, FIRE_WEAPON_WHEN_DEAD_FIELDS) {
        return None;
    }

    let module_source_index = u32::try_from(source_index).ok()?;
    let starts_active = match module.attribute("StartsActive") {
        Some(value) => parse_cxx_bool(value)?,
        None => false,
    };
    let death_weapon = match module.attribute("DeathWeapon") {
        Some(value) => parse_source_reference(value)?,
        None => None,
    };
    let death_types = match module.attribute("DeathTypes") {
        Some(value) => FireWeaponDeathTypeMask(
            u32::try_from(parse_cxx_all_plus_minus_mask(
                value,
                u64::from(FireWeaponDeathTypeMask::ALL.0),
                DEATH_NUM_TYPES,
                death_type_name_index,
            )?)
            .ok()?,
        ),
        None => FireWeaponDeathTypeMask::ALL,
    };
    let veterancy_levels = match module.attribute("VeterancyLevels") {
        Some(value) => FireWeaponVeterancyLevelMask(
            u8::try_from(parse_cxx_all_plus_minus_mask(
                value,
                u64::from(FireWeaponVeterancyLevelMask::ALL.0),
                VETERANCY_LEVEL_COUNT,
                veterancy_level_name_index,
            )?)
            .ok()?,
        ),
        None => FireWeaponVeterancyLevelMask::ALL,
    };
    let exempt_status = match module.attribute("ExemptStatus") {
        Some(value) => parse_cxx_object_status_mask(value)?,
        None => FireWeaponObjectStatusMask::NONE,
    };
    let required_status = match module.attribute("RequiredStatus") {
        Some(value) => parse_cxx_object_status_mask(value)?,
        None => FireWeaponObjectStatusMask::NONE,
    };

    Some(FireWeaponWhenDeadMetadata {
        module_source_index,
        module_tag: module.module_tag.clone(),
        starts_active,
        death_weapon,
        upgrade_mux: parse_upgrade_mux(module)?,
        death_types,
        veterancy_levels,
        exempt_status,
        required_status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn module(
        class_name: &str,
        module_tag: Option<&str>,
        fields: &[(&str, &str)],
    ) -> BehaviorModuleDefinition {
        BehaviorModuleDefinition {
            class_name: class_name.to_string(),
            module_tag: module_tag.map(str::to_string),
            attributes: fields
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect::<HashMap<_, _>>(),
        }
    }

    #[test]
    fn parses_cxx_defaults_and_all_none_plus_minus_masks() {
        assert_eq!(std::mem::size_of::<TemporaryWeaponSlot>(), 4);
        assert_eq!(std::mem::size_of::<TemporaryWeaponStatus>(), 4);

        let damaged = module(
            "FireWeaponWhenDamagedBehavior",
            Some("ModuleTag_Fire"),
            &[
                ("StartsActive", "Yes"),
                ("DamageTypes", "NONE +FLAME +POISON -FLAME"),
                // `INI::scanReal` delegates to `sscanf("%f")`, which
                // accepts this numeric prefix instead of requiring whole
                // token consumption.
                ("DamageAmount", "7.5f"),
                ("ReactionWeaponDamaged", "SharedTempWeapon"),
                ("ContinuousWeaponDamaged", "SharedTempWeapon"),
                ("TriggeredBy", "Upgrade_A Upgrade_B"),
                ("ConflictsWith", "Upgrade_C"),
                ("RemovesUpgrades", "Upgrade_D"),
                ("FXListUpgrade", "UpgradeFX"),
                ("RequiresAllTriggers", "Yes"),
            ],
        );
        let parsed = parse_fire_weapon_when_damaged_metadata(&damaged, 12).expect("metadata");
        assert_eq!(parsed.module_source_index, 12);
        assert!(parsed.starts_active);
        assert_eq!(parsed.damage_types.0, 1u64 << 9, "POISON only");
        assert_eq!(parsed.damage_amount, 7.5);
        assert_eq!(
            parsed.weapon_template_name(FireWeaponWhenDamagedWeaponRole::ReactionDamaged),
            Some("SharedTempWeapon")
        );
        assert_eq!(
            parsed.weapon_template_name(FireWeaponWhenDamagedWeaponRole::ContinuousDamaged),
            Some("SharedTempWeapon")
        );
        assert_eq!(parsed.upgrade_mux.triggered_by, ["Upgrade_A", "Upgrade_B"]);
        assert_eq!(parsed.upgrade_mux.conflicts_with, ["Upgrade_C"]);
        assert_eq!(parsed.upgrade_mux.removes_upgrades, ["Upgrade_D"]);
        assert_eq!(
            parsed.upgrade_mux.fx_list_upgrade.as_deref(),
            Some("UpgradeFX")
        );
        assert!(parsed.upgrade_mux.requires_all_triggers);

        let dead = module(
            "FireWeaponWhenDeadBehavior",
            None,
            &[
                ("DeathWeapon", "DeathTempWeapon"),
                ("DeathTypes", "NONE +EXPLODED"),
                ("VeterancyLevels", "NONE +VETERAN +HEROIC"),
                ("ExemptStatus", "+UNDER_CONSTRUCTION"),
                ("RequiredStatus", "DEPLOYED"),
            ],
        );
        let dead = parse_fire_weapon_when_dead_metadata(&dead, 4).expect("metadata");
        assert_eq!(dead.death_types.0, 1u32 << 4);
        assert_eq!(dead.veterancy_levels.0, (1u8 << 1) | (1u8 << 3));
        assert!(dead.exempt_status.contains_ordinal(3));
        assert!(dead.required_status.contains_ordinal(44));
        assert!(dead.die_mux_allows(4, 1, 1u64 << 44));
        assert!(!dead.die_mux_allows(4, 1, (1u64 << 44) | (1u64 << 3)));
        assert!(
            FireWeaponWhenDeadRuntimeState {
                module_source_index: 4,
                upgrade_executed: true,
            }
            .belongs_to_metadata(&dead)
        );

        let defaults = module("FireWeaponWhenDamagedBehavior", None, &[]);
        let defaults = parse_fire_weapon_when_damaged_metadata(&defaults, 0).expect("defaults");
        assert_eq!(defaults.damage_types, FireWeaponDamageTypeMask::ALL);
        assert_eq!(defaults.damage_amount, 0.0);
        assert!(!defaults.starts_active);
    }

    /// C++ FireWeaponWhenDeadBehavior.cpp:81-88 / UpgradeMux wouldUpgrade.
    #[test]
    fn upgrade_mux_triggered_by_and_conflicts_use_owned_names() {
        let mux = FireWeaponUpgradeMuxMetadata {
            triggered_by: vec!["Upgrade_HE".into()],
            conflicts_with: vec!["Upgrade_Bio".into()],
            ..Default::default()
        };
        assert!(!mux.conflicts_with_owned(&[]));
        assert!(mux.conflicts_with_owned(&["Upgrade_Bio"]));
        assert!(!mux.triggered_by_owned(&[]));
        assert!(mux.triggered_by_owned(&["Upgrade_HE"]));
    }

    #[test]
    fn rejects_non_cxx_flag_or_bool_syntax_instead_of_guessing() {
        let bare_damage = module(
            "FireWeaponWhenDamagedBehavior",
            None,
            &[("DamageTypes", "FLAME")],
        );
        assert!(parse_fire_weapon_when_damaged_metadata(&bare_damage, 0).is_none());

        let empty_damage = module(
            "FireWeaponWhenDamagedBehavior",
            None,
            &[("DamageTypes", "")],
        );
        assert!(parse_fire_weapon_when_damaged_metadata(&empty_damage, 0).is_none());

        let comma_delimited_damage = module(
            "FireWeaponWhenDamagedBehavior",
            None,
            &[("DamageTypes", "NONE,+FLAME")],
        );
        assert!(parse_fire_weapon_when_damaged_metadata(&comma_delimited_damage, 0).is_none());

        let lenient_bool = module(
            "FireWeaponWhenDeadBehavior",
            None,
            &[("StartsActive", "true")],
        );
        assert!(parse_fire_weapon_when_dead_metadata(&lenient_bool, 0).is_none());

        let mixed_status = module(
            "FireWeaponWhenDeadBehavior",
            None,
            &[("RequiredStatus", "+DEPLOYED UNDER_CONSTRUCTION")],
        );
        assert!(parse_fire_weapon_when_dead_metadata(&mixed_status, 0).is_none());

        let unknown_cxx_field = module(
            "FireWeaponWhenDamagedBehavior",
            None,
            &[("ReactionDamaged", "not a C++ module-data field")],
        );
        assert!(parse_fire_weapon_when_damaged_metadata(&unknown_cxx_field, 0).is_none());
    }

    #[test]
    fn same_template_specs_and_states_remain_independently_addressable() {
        let metadata = parse_fire_weapon_when_damaged_metadata(
            &module(
                "FireWeaponWhenDamagedBehavior",
                None,
                &[
                    ("ReactionWeaponDamaged", "SharedTempWeapon"),
                    ("ContinuousWeaponDamaged", "SharedTempWeapon"),
                ],
            ),
            9,
        )
        .expect("metadata");
        let specs = metadata.runtime_specs();
        assert_eq!(specs.len(), 2);
        assert_ne!(specs[0].key, specs[1].key);
        assert_eq!(specs[0].weapon_template_name, specs[1].weapon_template_name);
        assert!(
            specs
                .iter()
                .all(|spec| spec.weapon_slot == TemporaryWeaponSlot::Primary)
        );

        let mut runtime = FireWeaponWhenDamagedRuntimeState {
            module_source_index: 9,
            next_call_frame_and_phase: 0x1234_567a,
            ..Default::default()
        };
        for (index, spec) in specs.iter().enumerate() {
            assert!(runtime.replace_weapon_state(TemporaryWeaponRuntimeState {
                key: spec.key,
                weapon_template_name: spec.weapon_template_name.clone(),
                weapon_slot: TemporaryWeaponSlot::Primary,
                status: if index == 0 {
                    TemporaryWeaponStatus::ReloadingClip
                } else {
                    TemporaryWeaponStatus::ReadyToFire
                },
                ammo_in_clip: index as u32,
                when_we_can_fire_again: 0,
                when_pre_attack_finished: 0,
                when_last_reload_started: 0,
                last_fire_frame: 0,
                suspend_fx_frame: 0,
                projectile_stream_id: crate::game_logic::INVALID_OBJECT_ID,
                laser_object_id_unused: crate::game_logic::INVALID_OBJECT_ID,
                max_shot_count: TEMPORARY_WEAPON_NO_MAX_SHOTS_LIMIT,
                current_barrel: 0,
                num_shots_for_current_barrel: 1,
                scatter_targets_unused: Vec::new(),
                pitch_limited: false,
                leech_weapon_range_active: false,
            }));
        }
        assert_eq!(
            runtime.weapon(specs[0].key).map(|state| state.status),
            Some(TemporaryWeaponStatus::ReloadingClip)
        );
        assert_eq!(
            runtime.weapon(specs[1].key).map(|state| state.status),
            Some(TemporaryWeaponStatus::ReadyToFire)
        );
        assert!(
            runtime
                .weapon(specs[0].key)
                .unwrap()
                .matches_spec(&specs[0])
        );
        assert!(
            runtime
                .weapon(specs[1].key)
                .unwrap()
                .matches_spec(&specs[1])
        );
        assert_eq!(runtime.next_call_frame_and_phase, 0x1234_567a);
    }

    #[test]
    fn cxx_constructor_copy_and_assignment_reset_state_exactly() {
        let source_spec = TemporaryWeaponRuntimeSpec {
            key: TemporaryWeaponRuntimeKey {
                module_source_index: 3,
                role: FireWeaponWhenDamagedWeaponRole::ReactionDamaged,
            },
            weapon_template_name: "SourceTempWeapon".to_string(),
            weapon_slot: TemporaryWeaponSlot::Primary,
        };
        let destination_spec = TemporaryWeaponRuntimeSpec {
            key: TemporaryWeaponRuntimeKey {
                module_source_index: 3,
                role: FireWeaponWhenDamagedWeaponRole::ContinuousDamaged,
            },
            weapon_template_name: "DestinationTempWeapon".to_string(),
            weapon_slot: TemporaryWeaponSlot::Primary,
        };
        let source_defaults = TemporaryWeaponConstructionDefaults {
            min_target_pitch: -1.0,
            max_target_pitch: 1.0,
            shots_per_barrel: 3,
            suspend_fx_delay: 4,
            leech_range_weapon: true,
            clip_size: 6,
            clip_reload_frames: 7,
            scatter_target_count: 2,
        };
        let destination_defaults = TemporaryWeaponConstructionDefaults {
            min_target_pitch: -std::f32::consts::PI,
            max_target_pitch: std::f32::consts::PI,
            shots_per_barrel: 7,
            suspend_fx_delay: 100,
            leech_range_weapon: true,
            clip_size: 0,
            clip_reload_frames: 11,
            scatter_target_count: 0,
        };

        let mut source = TemporaryWeaponRuntimeState::from_cxx_constructor(
            &source_spec,
            source_defaults,
            u32::MAX - 1,
        );
        assert_eq!(source.suspend_fx_frame, 2, "C++ UnsignedInt addition wraps");
        assert!(source.pitch_limited);
        assert_eq!(source.num_shots_for_current_barrel, 3);
        assert!(!source.leech_weapon_range_active);
        assert_eq!(
            source.projectile_stream_id,
            crate::game_logic::INVALID_OBJECT_ID
        );
        assert_eq!(
            source.laser_object_id_unused,
            crate::game_logic::INVALID_OBJECT_ID
        );

        // Simulate a live source Weapon. C++ copy/assignment intentionally
        // discard all of these mutable fields except SuspendFXFrame.
        source.status = TemporaryWeaponStatus::BetweenFiringShots;
        source.ammo_in_clip = 11;
        source.when_we_can_fire_again = 12;
        source.when_pre_attack_finished = 13;
        source.when_last_reload_started = 14;
        source.last_fire_frame = 15;
        source.projectile_stream_id = crate::game_logic::ObjectId(99);
        source.laser_object_id_unused = crate::game_logic::ObjectId(98);
        source.max_shot_count = 17;
        source.current_barrel = 2;
        source.num_shots_for_current_barrel = 1;
        source.scatter_targets_unused = vec![4, 5];
        source.leech_weapon_range_active = true;
        let source_suspend_fx_frame = source.suspend_fx_frame;

        let copied = TemporaryWeaponRuntimeState::from_cxx_copy(&source, destination_defaults);
        assert_eq!(copied.key, source_spec.key);
        assert_eq!(
            copied.weapon_template_name,
            source_spec.weapon_template_name
        );
        assert_eq!(copied.weapon_slot, source_spec.weapon_slot);
        assert_eq!(copied.status, TemporaryWeaponStatus::OutOfAmmo);
        assert_eq!(copied.ammo_in_clip, 0);
        assert_eq!(copied.when_we_can_fire_again, 0);
        assert_eq!(copied.when_pre_attack_finished, 0);
        assert_eq!(copied.when_last_reload_started, 0);
        assert_eq!(copied.last_fire_frame, 0);
        assert_eq!(copied.suspend_fx_frame, source_suspend_fx_frame);
        assert_eq!(
            copied.projectile_stream_id,
            crate::game_logic::INVALID_OBJECT_ID
        );
        assert_eq!(
            copied.laser_object_id_unused,
            crate::game_logic::INVALID_OBJECT_ID
        );
        assert_eq!(copied.max_shot_count, TEMPORARY_WEAPON_NO_MAX_SHOTS_LIMIT);
        assert_eq!(copied.current_barrel, 0);
        assert_eq!(copied.num_shots_for_current_barrel, 7);
        assert!(copied.scatter_targets_unused.is_empty());
        assert!(!copied.pitch_limited);
        assert!(!copied.leech_weapon_range_active);

        let mut assigned = TemporaryWeaponRuntimeState::from_cxx_constructor(
            &destination_spec,
            destination_defaults,
            1_000,
        );
        assigned.assign_from_cxx(&source, destination_defaults);
        let mut expected_assigned = copied.clone();
        expected_assigned.key = destination_spec.key;
        assert_eq!(assigned, expected_assigned);
    }
}
