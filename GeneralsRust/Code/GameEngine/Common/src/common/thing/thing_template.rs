////////////////////////////////////////////////////////////////////////////////
//																			//
//  (c) 2001-2003 Electronic Arts Inc.										//
//																			//
////////////////////////////////////////////////////////////////////////////////

//! Thing templates are a 'roadmap' to creating things
//! Contains all the data needed to construct objects and drawables

use crate::common::bit_flags::{
    create_armor_set_flags, create_weapon_set_flags, ArmorSetBitFlags, BitFlags, WeaponSetBitFlags,
};
use crate::common::system::Snapshotable;
#[cfg(test)]
use crate::common::thing::module::BaseModuleData;
use crate::common::thing::module_factory::register_descriptor_set_global;
#[cfg(test)]
use crate::common::thing::module_factory::{
    clear_pending_descriptors_for_test, get_module_factory, ModuleFactory,
};
use crate::common::thing::sparse_match_finder::{
    SparseBitSet, SparseMatchCandidate, SparseMatchFinder,
};
use crate::common::{
    audio::AudioEventRts,
    global_data,
    rts::{
        get_science_store, AsciiString, Color, NameKeyType, ProductionPrerequisite, Real,
        UnicodeString, UnsignedByte, UnsignedShort, SCIENCE_INVALID,
    },
    system::{
        geometry::{GeometryInfo, GeometryType},
        Overridable, Xfer,
    },
    thing::module::{ModuleData, ModuleInterfaceType, ModuleType},
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

impl SparseBitSet for BitFlags {
    fn bit_len(&self) -> usize {
        self.size()
    }

    fn bit_test(&self, index: usize) -> bool {
        self.test(index)
    }

    fn yes_match_count(&self, other: &Self) -> usize {
        self.count_intersection(other)
    }

    fn extraneous_yes_count(&self, other: &Self) -> usize {
        self.count_inverse_intersection(other)
    }
}

/// Maximum number of upgrade cameos
pub const MAX_UPGRADE_CAMEO_UPGRADES: usize = 5;

/// Number of weapon slots (primary, secondary, tertiary)
pub const WEAPON_SLOT_COUNT: usize = 3;

/// Experience levels
pub const LEVEL_COUNT: usize = 4;

/// Use experience value for skill value sentinel
const USE_EXP_VALUE_FOR_SKILL_VALUE: i32 = -999;

/// Thing template audio types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum ThingTemplateAudioType {
    VoiceSelect = 0,
    VoiceGroupSelect,
    VoiceSelectElite,
    VoiceMove,
    VoiceAttack,
    VoiceEnter,
    VoiceFear,
    VoiceCreated,
    VoiceNearEnemy,
    VoiceTaskUnable,
    VoiceTaskComplete,
    VoiceMeetEnemy,
    SoundMoveStart,
    SoundMoveStartDamaged,
    SoundMoveLoop,
    SoundMoveLoopDamaged,
    SoundAmbient,
    SoundAmbientDamaged,
    SoundAmbientReallyDamaged,
    SoundAmbientRubble,
    SoundStealthOn,
    SoundStealthOff,
    SoundCreated,
    SoundOnDamaged,
    SoundOnReallyDamaged,
    SoundEnter,
    SoundExit,
    SoundPromotedVeteran,
    SoundPromotedElite,
    SoundPromotedHero,
    VoiceGarrison,
    SoundFalling,
    #[cfg(feature = "allow_surrender")]
    VoiceSurrender,
    VoiceDefect,
    VoiceAttackSpecial,
    VoiceAttackAir,
    VoiceGuard,
    Count,
}

/// Audio array for template sounds
#[derive(Debug, Clone)]
pub struct AudioArray {
    audio: [Option<AudioEventRts>; ThingTemplateAudioType::Count as usize],
}

impl AudioArray {
    pub fn new() -> Self {
        const INIT: Option<AudioEventRts> = None;
        Self {
            audio: [INIT; ThingTemplateAudioType::Count as usize],
        }
    }

    pub fn get(&self, audio_type: ThingTemplateAudioType) -> Option<&AudioEventRts> {
        self.audio[audio_type as usize].as_ref()
    }

    pub fn set(&mut self, audio_type: ThingTemplateAudioType, audio: AudioEventRts) {
        self.audio[audio_type as usize] = Some(audio);
    }
}

/// Build completion types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildCompletionType {
    Invalid = 0,
    AppearsAtRallyPoint,
    PlacedByPlayer,
}

/// Buildable status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildableStatus {
    Yes = 0,
    IgnorePrerequisites,
    No,
    OnlyByAi,
}

/// Radar priority types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarPriorityType {
    Invalid = 0,
    Low,
    Medium,
    High,
    Critical,
}

/// Editor sorting types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorSortingType {
    Invalid = 0,
    Unit,
    Building,
    Infrastructure,
    Civilian,
}

/// Shadow types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowType {
    None = 0,
    Volume,
    Decal,
}

/// Module parsing modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleParseMode {
    Normal,
    AddRemoveReplace,
    Inheritable,
    OverrideableByLikeKind,
}

/// Module information container
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    info: Vec<ModuleNugget>,
}

/// Read-only view describing a single module entry stored within `ModuleInfo`.
#[derive(Debug, Clone, Copy)]
pub struct ModuleInfoEntry<'a> {
    pub name: &'a AsciiString,
    pub module_tag: &'a AsciiString,
    pub data: &'a Arc<dyn ModuleData>,
    pub interface_mask: i32,
    pub inheritable: bool,
    pub overrideable_by_like_kind: bool,
    pub copied_from_default: bool,
}

#[derive(Debug, Clone)]
pub struct ModuleNugget {
    name: AsciiString,
    module_tag: AsciiString,
    data: Arc<dyn ModuleData>,
    interface_mask: i32,
    copied_from_default: bool,
    inheritable: bool,
    overrideable_by_like_kind: bool,
}

impl ModuleNugget {
    fn new(
        name: AsciiString,
        module_tag: AsciiString,
        data: Arc<dyn ModuleData>,
        interface_mask: i32,
        inheritable: bool,
        overrideable_by_like_kind: bool,
    ) -> Self {
        Self {
            name,
            module_tag,
            data,
            interface_mask,
            copied_from_default: false,
            inheritable,
            overrideable_by_like_kind,
        }
    }
}

impl<'a> From<&'a ModuleNugget> for ModuleInfoEntry<'a> {
    fn from(nugget: &'a ModuleNugget) -> Self {
        Self {
            name: &nugget.name,
            module_tag: &nugget.module_tag,
            data: &nugget.data,
            interface_mask: nugget.interface_mask,
            inheritable: nugget.inheritable,
            overrideable_by_like_kind: nugget.overrideable_by_like_kind,
            copied_from_default: nugget.copied_from_default,
        }
    }
}

/// Summary of the data needed to instantiate a module entry.
#[derive(Debug, Clone)]
pub struct ModuleDescriptor {
    pub name: AsciiString,
    pub module_tag: AsciiString,
    pub interface_mask: ModuleInterfaceType,
    pub inheritable: bool,
    pub overrideable_by_like_kind: bool,
    pub copied_from_default: bool,
}

impl ModuleDescriptor {
    /// Returns `true` when the descriptor advertises the supplied interface flag.
    pub fn supports(&self, interface: ModuleInterfaceType) -> bool {
        (self.interface_mask.0 & interface.0) != 0
    }
}

/// Collection of descriptors grouped by legacy module families.
#[derive(Debug, Clone, Default)]
pub struct ModuleDescriptorSet {
    pub behavior: Vec<ModuleDescriptor>,
    pub draw: Vec<ModuleDescriptor>,
    pub client_update: Vec<ModuleDescriptor>,
}

impl ModuleDescriptorSet {
    /// Returns the descriptor slice that matches the requested module type.
    pub fn for_type(&self, module_type: ModuleType) -> &[ModuleDescriptor] {
        match module_type {
            ModuleType::Behavior => &self.behavior,
            ModuleType::Draw => &self.draw,
            ModuleType::ClientUpdate => &self.client_update,
        }
    }

    /// Returns a mutable descriptor list for the requested module type.
    pub fn for_type_mut(&mut self, module_type: ModuleType) -> &mut Vec<ModuleDescriptor> {
        match module_type {
            ModuleType::Behavior => &mut self.behavior,
            ModuleType::Draw => &mut self.draw,
            ModuleType::ClientUpdate => &mut self.client_update,
        }
    }
}

impl<'a> ModuleInfoEntry<'a> {
    pub fn interface_flags(&self) -> ModuleInterfaceType {
        ModuleInterfaceType(self.interface_mask as u32)
    }

    pub fn supports(&self, interface: ModuleInterfaceType) -> bool {
        (self.interface_mask as u32 & interface.0) != 0
    }

    pub fn to_descriptor(&self) -> ModuleDescriptor {
        ModuleDescriptor {
            name: self.name.clone(),
            module_tag: self.module_tag.clone(),
            interface_mask: self.interface_flags(),
            inheritable: self.inheritable,
            overrideable_by_like_kind: self.overrideable_by_like_kind,
            copied_from_default: self.copied_from_default,
        }
    }
}

/// Build a KindOf u64 mask from an array of discriminant positions.
/// Each position `p` sets bit `1u64 << p`.
fn kindof_mask(positions: &[u32]) -> u64 {
    let mut mask = 0u64;
    for &p in positions {
        mask |= 1u64 << p;
    }
    mask
}

impl ModuleInfo {
    pub fn new() -> Self {
        Self { info: Vec::new() }
    }

    pub fn add_module_info(
        &mut self,
        name: AsciiString,
        module_tag: AsciiString,
        data: Arc<dyn ModuleData>,
        interface_mask: i32,
        inheritable: bool,
        overrideable_by_like_kind: bool,
    ) {
        let nugget = ModuleNugget::new(
            name,
            module_tag,
            data,
            interface_mask,
            inheritable,
            overrideable_by_like_kind,
        );
        self.info.push(nugget);
    }

    pub fn get_nugget_with_tag(&self, tag: &AsciiString) -> Option<&ModuleNugget> {
        self.info.iter().find(|nugget| &nugget.module_tag == tag)
    }

    pub fn get_count(&self) -> usize {
        self.info.len()
    }

    pub fn get_nth_name(&self, index: usize) -> Option<&AsciiString> {
        self.info.get(index).map(|nugget| &nugget.name)
    }

    pub fn get_nth_tag(&self, index: usize) -> Option<&AsciiString> {
        self.info.get(index).map(|nugget| &nugget.module_tag)
    }

    pub fn get_nth_data(&self, index: usize) -> Option<&Arc<dyn ModuleData>> {
        self.info.get(index).map(|nugget| &nugget.data)
    }

    pub fn descriptors(&self) -> Vec<ModuleDescriptor> {
        self.info
            .iter()
            .map(|n| ModuleInfoEntry::from(n).to_descriptor())
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = ModuleInfoEntry<'_>> {
        self.info.iter().map(ModuleInfoEntry::from)
    }

    pub fn is_empty(&self) -> bool {
        self.info.is_empty()
    }

    pub fn clear(&mut self) {
        self.info.clear();
    }

    pub fn set_copied_from_default(&mut self, value: bool) {
        for nugget in &mut self.info {
            nugget.copied_from_default = value;
        }
    }

    pub fn clear_module_data_with_tag(
        &mut self,
        tag_to_clear: &AsciiString,
    ) -> Option<AsciiString> {
        if let Some(pos) = self
            .info
            .iter()
            .position(|nugget| &nugget.module_tag == tag_to_clear)
        {
            let removed = self.info.remove(pos);
            return Some(removed.name);
        }
        None
    }

    pub fn clear_copied_from_default_entries(
        &mut self,
        interface_mask: i32,
        new_name: &AsciiString,
        full_template: &ThingTemplate,
    ) -> bool {
        // C++ Reference: ThingTemplate.cpp line 382-455 clearCopiedFromDefaultEntries
        //
        // Build KindOf masks using discriminant positions from the KindOf enum.
        // The mask is 1u64 << discriminant.

        // ImmuneToGPSScramblerMask: types that should NOT receive GPS scrambler modules
        let immune_mask: u64 = kindof_mask(&[
            5,   // Aircraft
            41,  // Shrubbery
            133, // OptimizedTree
            9,   // Structure
            88,  // DrawableOnly
            79,  // MobNexus
            78,  // IgnoredInGui
            136, // ClearedByBuild
            108, // DefensiveWall
            114, // BallisticMissile
            7,   // SupplySource
            87,  // Boat
            66,  // Inert (alias for Immobile)
            12,  // Bridge
            134, // LandmarkBridge
            65,  // BridgeTower
        ]);
        let disallowed = full_template.is_any_kind_of(&immune_mask);

        // CandidateForGPSScramblerMask: types that CAN receive GPS scrambler modules
        let candidate_mask: u64 = kindof_mask(&[
            89, // Score
            3,  // Vehicle
            4,  // Infantry
            69, // PortableStructure
        ]);
        let candidate = full_template.is_any_kind_of(&candidate_mask);

        let mut removed_any = false;
        let mut i = 0;
        while i < self.info.len() {
            let nugget = &self.info[i];
            if (nugget.interface_mask & interface_mask) != 0 && nugget.copied_from_default {
                if nugget.inheritable {
                    // Special case: don't inherit DefaultAutoHealBehavior if template
                    // is not trainable (module would be entirely useless).
                    if nugget.module_tag == "ModuleTag_DefaultAutoHealBehavior"
                        && !full_template.is_trainable()
                    {
                        self.info.remove(i);
                        removed_any = true;
                        continue;
                    }
                    // Keep this inherited module, skip to next.
                } else if nugget.overrideable_by_like_kind {
                    // Remove if: name matches new (INI author specified same class),
                    // or disallowed kind, or not a candidate kind.
                    if nugget.name == *new_name || disallowed || !candidate {
                        self.info.remove(i);
                        removed_any = true;
                        continue;
                    }
                    // No match — preserve the default module instance.
                } else {
                    // Non-inheritable, non-overrideable — always remove.
                    self.info.remove(i);
                    removed_any = true;
                    continue;
                }
            }
            i += 1;
        }

        removed_any
    }

    pub fn clear_ai_module_info(&mut self) -> bool {
        let initial_len = self.info.len();
        self.info.retain(|nugget| !nugget.data.is_ai_module_data());
        self.info.len() != initial_len
    }
}

/// Per-unit sound map type
pub type PerUnitSoundMap = HashMap<AsciiString, AudioEventRts>;

/// Per-unit FX map type (using Any for FXList placeholder)
pub type PerUnitFxMap = HashMap<AsciiString, Option<Arc<dyn std::any::Any + Send + Sync>>>;

/// Weapon template set placeholder
#[derive(Debug, Clone)]
pub struct WeaponTemplateSet {
    /// Bit-flag mask describing when this weapon set applies.
    types: WeaponSetBitFlags,
    /// Optional weapon template names for each slot (PRIMARY, SECONDARY, TERTIARY).
    weapon_template_names: [Option<AsciiString>; WEAPON_SLOT_COUNT],
    /// Command source mask per slot mirroring auto-choose rules.
    auto_choose_masks: [u32; WEAPON_SLOT_COUNT],
    /// Preferred target kind mask per slot (KindOfMaskType placeholder).
    preferred_against_masks: [u32; WEAPON_SLOT_COUNT],
    /// Whether reload times are shared across all slots in this set.
    is_reload_time_shared: bool,
    /// Whether weapon locks persist when switching to similar sets.
    is_weapon_lock_shared_across_sets: bool,
}

impl WeaponTemplateSet {
    /// Create an empty weapon template set with all flags cleared.
    pub fn new() -> Self {
        Self {
            types: create_weapon_set_flags(),
            weapon_template_names: [None, None, None],
            auto_choose_masks: [u32::MAX; WEAPON_SLOT_COUNT],
            preferred_against_masks: [0; WEAPON_SLOT_COUNT],
            is_reload_time_shared: false,
            is_weapon_lock_shared_across_sets: false,
        }
    }

    /// Reset the set to its default state.
    pub fn clear(&mut self) {
        self.types.clear();
        self.weapon_template_names = [None, None, None];
        self.auto_choose_masks = [u32::MAX; WEAPON_SLOT_COUNT];
        self.preferred_against_masks = [0; WEAPON_SLOT_COUNT];
        self.is_reload_time_shared = false;
        self.is_weapon_lock_shared_across_sets = false;
    }

    /// Access the flag mask.
    pub fn types(&self) -> &WeaponSetBitFlags {
        &self.types
    }

    /// Mutable access to the flag mask for INI parsing and overrides.
    pub fn types_mut(&mut self) -> &mut WeaponSetBitFlags {
        &mut self.types
    }

    /// Inspect the configured weapon template name for a slot, if any.
    pub fn weapon_template_name(&self, slot: usize) -> Option<&AsciiString> {
        self.weapon_template_names
            .get(slot)
            .and_then(|name| name.as_ref())
    }

    /// Assign a weapon template name for the given slot.
    pub fn set_weapon_template_name(&mut self, slot: usize, name: Option<AsciiString>) {
        if let Some(entry) = self.weapon_template_names.get_mut(slot) {
            *entry = name;
        } else {
            debug_assert!(false, "weapon slot index out of range");
        }
    }

    /// Retrieve the auto-choose mask for a slot.
    pub fn auto_choose_mask(&self, slot: usize) -> u32 {
        self.auto_choose_masks
            .get(slot)
            .copied()
            .unwrap_or(u32::MAX)
    }

    /// Define the auto-choose mask for a slot.
    pub fn set_auto_choose_mask(&mut self, slot: usize, mask: u32) {
        if let Some(entry) = self.auto_choose_masks.get_mut(slot) {
            *entry = mask;
        } else {
            debug_assert!(false, "weapon slot index out of range");
        }
    }

    /// Retrieve the preferred target mask for a slot.
    pub fn preferred_against_mask(&self, slot: usize) -> u32 {
        self.preferred_against_masks.get(slot).copied().unwrap_or(0)
    }

    /// Define the preferred target mask for a slot.
    pub fn set_preferred_against_mask(&mut self, slot: usize, mask: u32) {
        if let Some(entry) = self.preferred_against_masks.get_mut(slot) {
            *entry = mask;
        } else {
            debug_assert!(false, "weapon slot index out of range");
        }
    }

    /// Flag whether reload time should be shared across all weapons in this set.
    pub fn set_reload_time_shared(&mut self, shared: bool) {
        self.is_reload_time_shared = shared;
    }

    /// Check if reload time is shared across weapons.
    pub fn is_reload_time_shared(&self) -> bool {
        self.is_reload_time_shared
    }

    /// Flag whether weapon locks persist when switching sets.
    pub fn set_weapon_lock_shared_across_sets(&mut self, shared: bool) {
        self.is_weapon_lock_shared_across_sets = shared;
    }

    /// Check if weapon locks are shared across weapon sets.
    pub fn is_weapon_lock_shared_across_sets(&self) -> bool {
        self.is_weapon_lock_shared_across_sets
    }

    /// Check if any weapon templates are assigned in this set.
    pub fn has_any_weapons(&self) -> bool {
        self.weapon_template_names.iter().any(|name| name.is_some())
    }

    /// `SparseMatchFinder` compatibility: number of "yes" condition blocks.
    pub fn conditions_yes_count(&self) -> usize {
        1
    }

    /// Access the `index`th "yes" condition block.
    pub fn nth_conditions_yes(&self, index: usize) -> &WeaponSetBitFlags {
        debug_assert!(index == 0, "WeaponTemplateSet exposes a single YES set");
        &self.types
    }
}

impl Default for WeaponTemplateSet {
    fn default() -> Self {
        Self::new()
    }
}

impl SparseMatchCandidate<WeaponSetBitFlags> for WeaponTemplateSet {
    fn conditions_yes_count(&self) -> usize {
        WeaponTemplateSet::conditions_yes_count(self)
    }

    fn nth_conditions_yes(&self, index: usize) -> &WeaponSetBitFlags {
        WeaponTemplateSet::nth_conditions_yes(self, index)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WeaponSetDefinition {
    conditions: Vec<String>,
    weapon_names: [Option<AsciiString>; WEAPON_SLOT_COUNT],
    auto_choose_masks: [Option<u32>; WEAPON_SLOT_COUNT],
    preferred_against_masks: [Option<u32>; WEAPON_SLOT_COUNT],
    share_reload_time: Option<bool>,
    share_weapon_lock: Option<bool>,
}

impl WeaponSetDefinition {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_condition<S: AsRef<str>>(&mut self, condition: S) {
        let token = condition.as_ref().trim();
        if token.is_empty() {
            return;
        }
        self.conditions.push(token.to_ascii_uppercase());
    }

    pub fn set_weapon_name(&mut self, slot: usize, name: Option<AsciiString>) {
        if slot < WEAPON_SLOT_COUNT {
            self.weapon_names[slot] =
                name.and_then(|value| if value.is_empty() { None } else { Some(value) });
        } else {
            debug_assert!(
                false,
                "weapon slot out of range in WeaponSetDefinition::set_weapon_name"
            );
        }
    }

    pub fn set_weapon_name_str(&mut self, slot: usize, name: Option<&str>) {
        self.set_weapon_name(slot, name.map(AsciiString::from));
    }

    pub fn set_auto_choose_mask(&mut self, slot: usize, mask: Option<u32>) {
        if slot < WEAPON_SLOT_COUNT {
            self.auto_choose_masks[slot] = mask;
        } else {
            debug_assert!(
                false,
                "weapon slot out of range in WeaponSetDefinition::set_auto_choose_mask"
            );
        }
    }

    pub fn set_preferred_against_mask(&mut self, slot: usize, mask: Option<u32>) {
        if slot < WEAPON_SLOT_COUNT {
            self.preferred_against_masks[slot] = mask;
        } else {
            debug_assert!(
                false,
                "weapon slot out of range in WeaponSetDefinition::set_preferred_against_mask"
            );
        }
    }

    pub fn set_share_reload_time(&mut self, shared: Option<bool>) {
        self.share_reload_time = shared;
    }

    pub fn set_share_weapon_lock(&mut self, shared: Option<bool>) {
        self.share_weapon_lock = shared;
    }

    pub fn apply_to(&self, set: &mut WeaponTemplateSet) -> Result<(), String> {
        set.clear();
        {
            let flags = set.types_mut();
            for condition in &self.conditions {
                if condition.is_empty() {
                    continue;
                }
                if !flags.set_bit_by_name(condition) {
                    return Err(format!("Unknown weapon set condition '{}'", condition));
                }
            }
        }

        for slot in 0..WEAPON_SLOT_COUNT {
            if let Some(name) = &self.weapon_names[slot] {
                set.set_weapon_template_name(slot, Some(name.clone()));
            }
            if let Some(mask) = self.auto_choose_masks[slot] {
                set.set_auto_choose_mask(slot, mask);
            }
            if let Some(mask) = self.preferred_against_masks[slot] {
                set.set_preferred_against_mask(slot, mask);
            }
        }

        if let Some(shared) = self.share_reload_time {
            set.set_reload_time_shared(shared);
        }
        if let Some(shared) = self.share_weapon_lock {
            set.set_weapon_lock_shared_across_sets(shared);
        }

        Ok(())
    }
}

#[derive(Default)]
struct WeaponSetDefinitionBuilder {
    conditions: Vec<String>,
    weapon_names: [Option<AsciiString>; WEAPON_SLOT_COUNT],
    auto_choose_masks: [Option<u32>; WEAPON_SLOT_COUNT],
    preferred_against_masks: [Option<u32>; WEAPON_SLOT_COUNT],
    share_reload_time: Option<bool>,
    share_weapon_lock: Option<bool>,
}

impl WeaponSetDefinitionBuilder {
    fn apply_field(&mut self, field: &str, value: &str) -> Result<(), String> {
        let trimmed = value.trim();
        match field {
            "Conditions" => {
                for token in split_weapon_condition_tokens(trimmed) {
                    self.conditions.push(token);
                }
            }
            "ShareWeaponReloadTime" | "ShareReloadTime" => {
                self.share_reload_time = Some(parse_bool_field(trimmed)?);
            }
            "WeaponLockSharedAcrossSets" | "ShareWeaponLock" => {
                self.share_weapon_lock = Some(parse_bool_field(trimmed)?);
            }
            "PrimaryWeapon" => self.weapon_names[0] = Some(AsciiString::from(trimmed)),
            "SecondaryWeapon" => self.weapon_names[1] = Some(AsciiString::from(trimmed)),
            "TertiaryWeapon" => self.weapon_names[2] = Some(AsciiString::from(trimmed)),
            "AutoChoosePrimary" | "AutoChooseSourcesPrimary" => {
                self.auto_choose_masks[0] = Some(parse_u32_field(trimmed)?);
            }
            "AutoChooseSecondary" | "AutoChooseSourcesSecondary" => {
                self.auto_choose_masks[1] = Some(parse_u32_field(trimmed)?);
            }
            "AutoChooseTertiary" | "AutoChooseSourcesTertiary" => {
                self.auto_choose_masks[2] = Some(parse_u32_field(trimmed)?);
            }
            "PreferredAgainstPrimary" => {
                self.preferred_against_masks[0] = Some(parse_u32_field(trimmed)?);
            }
            "PreferredAgainstSecondary" => {
                self.preferred_against_masks[1] = Some(parse_u32_field(trimmed)?);
            }
            "PreferredAgainstTertiary" => {
                self.preferred_against_masks[2] = Some(parse_u32_field(trimmed)?);
            }
            _ => {
                return Err(format!("Unrecognised weapon set field '{}'", field));
            }
        }

        Ok(())
    }

    fn build(self) -> WeaponSetDefinition {
        let mut definition = WeaponSetDefinition::new();
        for condition in self.conditions {
            definition.add_condition(condition);
        }
        for slot in 0..WEAPON_SLOT_COUNT {
            definition.weapon_names[slot] = self.weapon_names[slot].clone();
            definition.auto_choose_masks[slot] = self.auto_choose_masks[slot];
            definition.preferred_against_masks[slot] = self.preferred_against_masks[slot];
        }
        definition.share_reload_time = self.share_reload_time;
        definition.share_weapon_lock = self.share_weapon_lock;
        definition
    }
}

pub(crate) fn parse_bool_field(value: &str) -> Result<bool, String> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value '{}'", value)),
    }
}

pub(crate) fn parse_u32_field(value: &str) -> Result<u32, String> {
    let v = value.trim();
    if v.is_empty() {
        return Err("Empty integer value".to_string());
    }
    if let Some(stripped) = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
        u32::from_str_radix(stripped, 16).map_err(|_| format!("Invalid hex value '{}'", value))
    } else {
        v.parse::<u32>()
            .map_err(|_| format!("Invalid integer value '{}'", value))
    }
}

pub(crate) fn split_weapon_condition_tokens(value: &str) -> Vec<String> {
    value
        .split(|c: char| c == '|' || c == ',' || c.is_whitespace())
        .filter_map(|token| {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_ascii_uppercase())
            }
        })
        .collect()
}
/// Armor template set definition mirroring the legacy C++ structure.
#[derive(Debug, Clone)]
pub struct ArmorTemplateSet {
    /// Bit-flag mask describing when this armor template applies.
    types: ArmorSetBitFlags,
    /// Optional armor template name referenced in `GameLogic::TheArmorStore`.
    armor_template_name: Option<AsciiString>,
    /// Optional damage FX block name resolved through the shared DamageFX store.
    damage_fx_name: Option<AsciiString>,
}

impl ArmorTemplateSet {
    /// Create an empty armor template set with all flags cleared.
    pub fn new() -> Self {
        Self {
            types: create_armor_set_flags(),
            armor_template_name: None,
            damage_fx_name: None,
        }
    }

    /// Reset the set back to its default state.
    pub fn clear(&mut self) {
        self.types.clear();
        self.armor_template_name = None;
        self.damage_fx_name = None;
    }

    /// Access the flag mask.
    pub fn types(&self) -> &ArmorSetBitFlags {
        &self.types
    }

    /// Mutable access to the flag mask for INI parsing and overrides.
    pub fn types_mut(&mut self) -> &mut ArmorSetBitFlags {
        &mut self.types
    }

    /// Assign the armor template name (case-preserving).
    pub fn set_armor_template_name(&mut self, name: Option<AsciiString>) {
        self.armor_template_name = name;
    }

    /// Retrieve the configured armor template name, if any.
    pub fn armor_template_name(&self) -> Option<&AsciiString> {
        self.armor_template_name.as_ref()
    }

    /// Assign the damage FX name associated with this set.
    pub fn set_damage_fx_name(&mut self, name: Option<AsciiString>) {
        self.damage_fx_name = name;
    }

    /// Inspect the configured damage FX name, if present.
    pub fn damage_fx_name(&self) -> Option<&AsciiString> {
        self.damage_fx_name.as_ref()
    }

    /// `SparseMatchFinder` compatibility: number of "yes" condition blocks.
    pub fn conditions_yes_count(&self) -> usize {
        1
    }

    /// Access the `index`th "yes" condition block.
    pub fn nth_conditions_yes(&self, index: usize) -> &ArmorSetBitFlags {
        debug_assert!(index == 0, "ArmorTemplateSet exposes a single YES set");
        &self.types
    }
}

impl Default for ArmorTemplateSet {
    fn default() -> Self {
        Self::new()
    }
}

impl SparseMatchCandidate<ArmorSetBitFlags> for ArmorTemplateSet {
    fn conditions_yes_count(&self) -> usize {
        ArmorTemplateSet::conditions_yes_count(self)
    }

    fn nth_conditions_yes(&self, index: usize) -> &ArmorSetBitFlags {
        ArmorTemplateSet::nth_conditions_yes(self, index)
    }
}

/// Thing template - contains all data needed to create things
#[derive(Debug, Clone)]
pub struct ThingTemplate {
    // Identification
    template_id: UnsignedShort,
    name_string: AsciiString,
    next_thing_template: Option<Arc<ThingTemplate>>,
    next_override: Arc<RwLock<Option<Arc<ThingTemplate>>>>,
    is_override: bool,
    reskinned_from: Option<Arc<ThingTemplate>>,

    // Display properties
    display_name: UnicodeString,
    display_color: Color,
    editor_sorting: EditorSortingType,

    // Physical properties
    geometry_info: GeometryInfo,
    asset_scale: Real,
    instance_scale_fuzziness: Real,

    // Audio
    audioarray: AudioArray,
    per_unit_sounds: PerUnitSoundMap,
    per_unit_fx: PerUnitFxMap,

    // Module information
    behavior_module_info: ModuleInfo,
    draw_module_info: ModuleInfo,
    client_update_module_info: ModuleInfo,

    // Build and prerequisite data
    prereq_info: Vec<ProductionPrerequisite>,
    build_variations: Vec<AsciiString>,
    build_cost: UnsignedShort,
    build_time: Real,
    refund_value: UnsignedShort,
    buildable: BuildableStatus,
    build_completion: BuildCompletionType,
    is_build_facility: bool,
    is_prerequisite: bool,
    is_forbidden: bool,

    // Gameplay properties
    kindof: u64, // KindOfMaskType placeholder
    default_owning_side: AsciiString,
    command_set_string: AsciiString,
    skill_point_values: [i32; LEVEL_COUNT],
    experience_values: [i32; LEVEL_COUNT],
    experience_required: [i32; LEVEL_COUNT],
    is_trainable: bool,
    enter_guard: bool,
    hijack_guard: bool,

    // Visual properties
    selected_portrait_image: Option<Arc<dyn std::any::Any + Send + Sync>>, // Image placeholder
    button_image: Option<Arc<dyn std::any::Any + Send + Sync>>,            // Image placeholder
    selected_portrait_image_name: AsciiString,
    button_image_name: AsciiString,
    upgrade_cameo_upgrade_names: [AsciiString; MAX_UPGRADE_CAMEO_UPGRADES],

    // Shadow properties
    shadow_type: ShadowType,
    shadow_size_x: Real,
    shadow_size_y: Real,
    shadow_offset_x: Real,
    shadow_offset_y: Real,
    shadow_texture_name: AsciiString,
    occlusion_delay: u32,

    // Tactical properties
    radar_priority: RadarPriorityType,
    transport_slot_count: UnsignedByte,
    fence_width: Real,
    fence_x_offset: Real,
    is_bridge: bool,
    vision_range: Real,
    shroud_clearing_range: Real,
    shroud_reveal_to_all_range: Real,
    placement_view_angle: Real,
    factory_exit_width: Real,
    factory_extra_bib_width: Real,

    // Energy and resources
    energy_production: i32,
    energy_bonus: i32,

    // Combat properties
    weapon_template_sets: Vec<WeaponTemplateSet>,
    weapon_template_set_finder: SparseMatchFinder<WeaponTemplateSet, WeaponSetBitFlags>,
    armor_template_sets: Vec<ArmorTemplateSet>,
    armor_template_set_finder: SparseMatchFinder<ArmorTemplateSet, ArmorSetBitFlags>,
    threat_value: UnsignedShort,
    max_simultaneous_of_type: UnsignedShort,
    max_simultaneous_link_key: NameKeyType,
    max_simultaneous_determined_by_superweapon_restriction: bool,
    crusher_level: UnsignedByte,
    crushable_level: UnsignedByte,
    structure_rubble_height: UnsignedByte,

    // Internal state
    armor_copied_from_default: bool,
    weapons_copied_from_default: bool,
    module_parsing_mode: ModuleParseMode,
    module_being_replaced_name: AsciiString,
    module_being_replaced_tag: AsciiString,

    #[cfg(feature = "load_test_assets")]
    lta_name: AsciiString,
}

impl ThingTemplate {
    pub fn new() -> Self {
        Self {
            template_id: 0,
            name_string: AsciiString::new(),
            next_thing_template: None,
            next_override: Arc::new(RwLock::new(None)),
            is_override: false,
            reskinned_from: None,

            display_name: UnicodeString::new(),
            display_color: Color::white(),
            editor_sorting: EditorSortingType::Invalid,

            geometry_info: GeometryInfo::new(GeometryType::Sphere, false, 1.0, 1.0, 1.0),
            asset_scale: 1.0,
            instance_scale_fuzziness: 0.0,

            audioarray: AudioArray::new(),
            per_unit_sounds: HashMap::new(),
            per_unit_fx: HashMap::new(),

            behavior_module_info: ModuleInfo::new(),
            draw_module_info: ModuleInfo::new(),
            client_update_module_info: ModuleInfo::new(),

            prereq_info: Vec::new(),
            build_variations: Vec::new(),
            build_cost: 0,
            build_time: 1.0,
            refund_value: 0,
            buildable: BuildableStatus::Yes,
            build_completion: BuildCompletionType::AppearsAtRallyPoint,
            is_build_facility: false,
            is_prerequisite: false,
            is_forbidden: false,

            kindof: 0,
            default_owning_side: AsciiString::new(),
            command_set_string: AsciiString::new(),
            skill_point_values: [USE_EXP_VALUE_FOR_SKILL_VALUE; LEVEL_COUNT],
            experience_values: [0; LEVEL_COUNT],
            experience_required: [0; LEVEL_COUNT],
            is_trainable: false,
            enter_guard: false,
            hijack_guard: false,

            selected_portrait_image: None,
            button_image: None,
            selected_portrait_image_name: AsciiString::new(),
            button_image_name: AsciiString::new(),
            upgrade_cameo_upgrade_names: [
                AsciiString::new(),
                AsciiString::new(),
                AsciiString::new(),
                AsciiString::new(),
                AsciiString::new(),
            ],

            shadow_type: ShadowType::None,
            shadow_size_x: 0.0,
            shadow_size_y: 0.0,
            shadow_offset_x: 0.0,
            shadow_offset_y: 0.0,
            shadow_texture_name: AsciiString::new(),
            occlusion_delay: global_data::read().default_occlusion_delay,

            radar_priority: RadarPriorityType::Invalid,
            transport_slot_count: 0,
            fence_width: 0.0,
            fence_x_offset: 0.0,
            is_bridge: false,
            vision_range: 0.0,
            shroud_clearing_range: -1.0,
            shroud_reveal_to_all_range: -1.0,
            placement_view_angle: 0.0,
            factory_exit_width: 0.0,
            factory_extra_bib_width: 0.0,

            energy_production: 0,
            energy_bonus: 0,

            weapon_template_sets: Vec::new(),
            weapon_template_set_finder: SparseMatchFinder::new(),
            armor_template_sets: Vec::new(),
            armor_template_set_finder: SparseMatchFinder::new(),
            threat_value: 0,
            max_simultaneous_of_type: 0,
            max_simultaneous_link_key: 0,
            max_simultaneous_determined_by_superweapon_restriction: false,
            crusher_level: 0,
            crushable_level: 255,
            structure_rubble_height: 0,

            armor_copied_from_default: false,
            weapons_copied_from_default: false,
            module_parsing_mode: ModuleParseMode::Normal,
            module_being_replaced_name: AsciiString::new(),
            module_being_replaced_tag: AsciiString::new(),

            #[cfg(feature = "load_test_assets")]
            lta_name: AsciiString::new(),
        }
    }

    // Getters
    pub fn get_template_id(&self) -> UnsignedShort {
        self.template_id
    }
    pub fn get_name(&self) -> &AsciiString {
        &self.name_string
    }
    pub fn get_display_name(&self) -> &UnicodeString {
        &self.display_name
    }

    /// Number of production prerequisites attached to this template.
    pub fn get_prereq_count(&self) -> usize {
        self.prereq_info.len()
    }

    /// Access a prerequisite by index.
    pub fn get_prereq(&self, index: usize) -> Option<&ProductionPrerequisite> {
        self.prereq_info.get(index)
    }

    /// Access all prerequisites for this template.
    pub fn get_prereqs(&self) -> &[ProductionPrerequisite] {
        &self.prereq_info
    }

    /// Optional rubble height (0 means use default from global data).
    pub fn structure_rubble_height(&self) -> Option<u8> {
        if self.structure_rubble_height == 0 {
            None
        } else {
            Some(self.structure_rubble_height)
        }
    }

    pub fn get_display_color(&self) -> Color {
        self.display_color
    }
    pub fn get_editor_sorting(&self) -> EditorSortingType {
        self.editor_sorting
    }
    pub fn get_template_geometry_info(&self) -> &GeometryInfo {
        &self.geometry_info
    }
    pub fn calc_vision_range(&self) -> Real {
        if self.vision_range > 0.0 {
            self.vision_range
        } else {
            self.geometry_info.height.max(0.0)
        }
    }
    pub fn calc_shroud_clearing_range(&self) -> Real {
        if self.shroud_clearing_range >= 0.0 {
            self.shroud_clearing_range
        } else {
            self.calc_vision_range()
        }
    }

    pub fn get_shroud_reveal_to_all_range(&self) -> Real {
        self.shroud_reveal_to_all_range
    }

    pub fn get_threat_value(&self) -> UnsignedShort {
        self.threat_value
    }

    /// Returns the crushing power rating for this template.
    /// C++ Reference: ThingTemplate.h getCrusherLevel()
    pub fn get_crusher_level(&self) -> UnsignedByte {
        self.crusher_level
    }

    /// Returns the vulnerability to being crushed for this template.
    /// C++ Reference: ThingTemplate.h getCrushableLevel()
    pub fn get_crushable_level(&self) -> UnsignedByte {
        self.crushable_level
    }

    pub fn get_asset_scale(&self) -> Real {
        self.asset_scale
    }
    pub fn get_instance_scale_fuzziness(&self) -> Real {
        self.instance_scale_fuzziness
    }

    pub fn get_behavior_module_info(&self) -> &ModuleInfo {
        &self.behavior_module_info
    }

    /// Returns descriptors for behavior modules defined on this template.
    pub fn behavior_module_descriptors(&self) -> Vec<ModuleDescriptor> {
        self.behavior_module_info.descriptors()
    }
    pub fn get_draw_module_info(&self) -> &ModuleInfo {
        &self.draw_module_info
    }

    /// Returns descriptors for draw modules defined on this template.
    pub fn draw_module_descriptors(&self) -> Vec<ModuleDescriptor> {
        self.draw_module_info.descriptors()
    }
    pub fn get_client_update_module_info(&self) -> &ModuleInfo {
        &self.client_update_module_info
    }

    /// Returns the grouped module descriptors extracted from this template.
    pub fn module_descriptors(&self) -> ModuleDescriptorSet {
        let descriptors = ModuleDescriptorSet {
            behavior: self.behavior_module_info.descriptors(),
            draw: self.draw_module_info.descriptors(),
            client_update: self.client_update_module_info.descriptors(),
        };

        register_descriptor_set_global(&descriptors);

        descriptors
    }

    /// Returns descriptors for client-update modules defined on this template.
    pub fn client_update_module_descriptors(&self) -> Vec<ModuleDescriptor> {
        self.client_update_module_info.descriptors()
    }

    /// Returns descriptors for the requested module type.
    pub fn module_descriptors_for_type(&self, module_type: ModuleType) -> Vec<ModuleDescriptor> {
        self.module_descriptors().for_type(module_type).to_vec()
    }

    pub fn get_build_variations(&self) -> &Vec<AsciiString> {
        &self.build_variations
    }
    pub fn get_build_cost(&self) -> UnsignedShort {
        self.build_cost
    }
    pub fn get_build_time(&self) -> Real {
        self.build_time
    }
    pub fn get_refund_value(&self) -> UnsignedShort {
        self.refund_value
    }
    pub fn get_buildable(&self) -> BuildableStatus {
        self.buildable
    }

    /// ThingTemplate.h line 520: `UnsignedInt getOcclusionDelay() const`
    pub fn get_occlusion_delay(&self) -> u32 {
        self.occlusion_delay
    }

    pub fn set_occlusion_delay(&mut self, delay: u32) {
        self.occlusion_delay = delay;
    }
    pub fn get_build_completion(&self) -> BuildCompletionType {
        self.build_completion
    }

    pub fn is_build_facility(&self) -> bool {
        self.is_build_facility
    }

    /// Get energy production/consumption value
    ///
    /// # C++ Reference
    /// ThingTemplate.h line 525: `Int getEnergyProduction() const`
    ///
    /// Returns:
    /// - Positive values: Building produces power (e.g., power plant = +5)
    /// - Negative values: Building consumes power (e.g., barracks = -1)
    /// - Zero: Building is power-neutral
    pub fn get_energy_production(&self) -> i32 {
        self.energy_production
    }

    /// Get energy bonus value from upgrades
    ///
    /// # C++ Reference
    /// ThingTemplate.h line 526: `Int getEnergyBonus() const`
    ///
    /// This is the extra energy production gained from upgrades.
    /// For example, the American "Control Rods" upgrade to the Cold Fusion
    /// Reactor grants +3 bonus energy production.
    pub fn get_energy_bonus(&self) -> i32 {
        self.energy_bonus
    }

    pub fn is_kind_of(&self, kind: u32) -> bool {
        (self.kindof & kind as u64) != 0
    }

    pub fn is_kind_of_multi(&self, must_be_set: &u64, must_be_clear: &u64) -> bool {
        (self.kindof & must_be_set) == *must_be_set && (self.kindof & must_be_clear) == 0
    }

    pub fn is_any_kind_of(&self, any_kind_of: &u64) -> bool {
        (self.kindof & any_kind_of) != 0
    }

    pub fn get_kindof_mask(&self) -> u64 {
        self.kindof
    }

    pub fn get_default_owning_side(&self) -> &AsciiString {
        &self.default_owning_side
    }
    pub fn get_command_set_string(&self) -> &AsciiString {
        &self.command_set_string
    }

    pub fn get_skill_point_value(&self, level: usize) -> i32 {
        let value = self.skill_point_values[level];
        if value == USE_EXP_VALUE_FOR_SKILL_VALUE {
            self.get_experience_value(level)
        } else {
            value
        }
    }

    pub fn get_experience_value(&self, level: usize) -> i32 {
        self.experience_values[level]
    }
    pub fn get_experience_required(&self, level: usize) -> i32 {
        self.experience_required[level]
    }
    pub fn is_trainable(&self) -> bool {
        self.is_trainable
    }
    pub fn is_enter_guard(&self) -> bool {
        self.enter_guard
    }
    pub fn is_hijack_guard(&self) -> bool {
        self.hijack_guard
    }

    // Audio getters
    pub fn get_voice_select(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceSelect)
    }

    pub fn get_voice_attack(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceAttack)
    }

    pub fn get_voice_attack_special(&self) -> Option<&AudioEventRts> {
        self.audioarray
            .get(ThingTemplateAudioType::VoiceAttackSpecial)
    }

    pub fn get_voice_attack_air(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceAttackAir)
    }

    pub fn get_voice_task_complete(&self) -> Option<&AudioEventRts> {
        self.audioarray
            .get(ThingTemplateAudioType::VoiceTaskComplete)
    }

    pub fn get_sound_move_start(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::SoundMoveStart)
    }

    pub fn get_sound_move_start_damaged(&self) -> Option<&AudioEventRts> {
        self.audioarray
            .get(ThingTemplateAudioType::SoundMoveStartDamaged)
    }

    pub fn get_sound_move_loop(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::SoundMoveLoop)
    }

    pub fn get_sound_move_loop_damaged(&self) -> Option<&AudioEventRts> {
        self.audioarray
            .get(ThingTemplateAudioType::SoundMoveLoopDamaged)
    }

    pub fn get_sound_ambient(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::SoundAmbient)
    }

    pub fn get_sound_ambient_damaged(&self) -> Option<&AudioEventRts> {
        self.audioarray
            .get(ThingTemplateAudioType::SoundAmbientDamaged)
    }

    pub fn get_sound_ambient_really_damaged(&self) -> Option<&AudioEventRts> {
        self.audioarray
            .get(ThingTemplateAudioType::SoundAmbientReallyDamaged)
    }

    pub fn get_sound_ambient_rubble(&self) -> Option<&AudioEventRts> {
        self.audioarray
            .get(ThingTemplateAudioType::SoundAmbientRubble)
    }

    pub fn get_per_unit_sound(&self, sound_name: &AsciiString) -> Option<&AudioEventRts> {
        self.per_unit_sounds.get(sound_name)
    }

    pub fn get_per_unit_fx(
        &self,
        fx_name: &AsciiString,
    ) -> Option<&Arc<dyn std::any::Any + Send + Sync>> {
        self.per_unit_fx.get(fx_name).and_then(|fx| fx.as_ref())
    }

    /// Access the configured weapon template sets.
    pub fn weapon_template_sets(&self) -> &[WeaponTemplateSet] {
        &self.weapon_template_sets
    }

    /// Append a new weapon template set and invalidate cached lookups.
    pub fn add_weapon_template_set(&mut self, set: WeaponTemplateSet) {
        self.weapon_template_sets.push(set);
        self.weapon_template_set_finder.clear();
    }

    /// Clear all weapon template sets and reset the lookup cache.
    pub fn clear_weapon_template_sets(&mut self) {
        self.weapon_template_sets.clear();
        self.weapon_template_set_finder.clear();
    }

    /// Find the best matching weapon template set for the supplied flags.
    pub fn find_weapon_template_set(
        &self,
        flags: &WeaponSetBitFlags,
    ) -> Option<&WeaponTemplateSet> {
        self.weapon_template_set_finder
            .find_best(&self.weapon_template_sets, flags)
    }

    /// Replace all weapon template sets with the provided definitions.
    pub fn load_weapon_sets_from_definitions(
        &mut self,
        definitions: &[WeaponSetDefinition],
    ) -> Result<(), String> {
        self.clear_weapon_template_sets();
        for definition in definitions {
            let mut set = WeaponTemplateSet::new();
            definition.apply_to(&mut set)?;
            self.add_weapon_template_set(set);
        }
        Ok(())
    }

    /// Returns true if any weapon template set contains at least one weapon template.
    pub fn can_possibly_have_any_weapon(&self) -> bool {
        self.weapon_template_sets
            .iter()
            .any(|set| set.has_any_weapons())
    }

    /// Access the configured armor template sets.
    pub fn armor_template_sets(&self) -> &[ArmorTemplateSet] {
        &self.armor_template_sets
    }

    /// Append a new armor template set and invalidate cached lookups.
    pub fn add_armor_template_set(&mut self, set: ArmorTemplateSet) {
        self.armor_template_sets.push(set);
        self.armor_template_set_finder.clear();
    }

    /// Clear all armor template sets and reset the lookup cache.
    pub fn clear_armor_template_sets(&mut self) {
        self.armor_template_sets.clear();
        self.armor_template_set_finder.clear();
    }

    /// Find the best matching armor template set for the supplied flags.
    pub fn find_armor_template_set(&self, flags: &ArmorSetBitFlags) -> Option<&ArmorTemplateSet> {
        self.armor_template_set_finder
            .find_best(&self.armor_template_sets, flags)
    }

    // Setters (friend functions)
    pub fn set_template_id(&mut self, id: UnsignedShort) {
        self.template_id = id;
    }
    pub fn set_template_name(&mut self, name: AsciiString) {
        self.name_string = name;
    }
    pub fn get_next_template(&self) -> &Option<Arc<ThingTemplate>> {
        &self.next_thing_template
    }
    pub fn set_next_template(&mut self, template: Option<Arc<ThingTemplate>>) {
        self.next_thing_template = template;
    }

    /// Get the default radar priority level for this template.
    /// C++ Reference: ThingTemplate.h line 468 (getDefaultRadarPriority)
    pub fn get_radar_priority(&self) -> RadarPriorityType {
        self.radar_priority
    }

    // Utility methods
    pub fn copy_from(&mut self, other: &ThingTemplate) {
        // Preserve identity fields
        let id = self.template_id;
        let name = self.name_string.clone();
        let next = self.next_thing_template.clone();
        let next_override = self.next_override.clone();
        let is_override = self.is_override;

        // Copy all data
        *self = other.clone();

        // Restore identity
        self.template_id = id;
        self.name_string = name;
        self.next_thing_template = next;
        self.next_override = next_override;
        self.is_override = is_override;
    }

    pub fn set_copied_from_default(&mut self) {
        self.armor_copied_from_default = true;
        self.weapons_copied_from_default = true;
        self.behavior_module_info.set_copied_from_default(true);
        self.draw_module_info.set_copied_from_default(true);
        self.client_update_module_info.set_copied_from_default(true);
    }

    pub fn set_reskinned_from(&mut self, template: Arc<ThingTemplate>) {
        debug_assert!(self.reskinned_from.is_none(), "should be None");
        self.reskinned_from = Some(template);
    }

    /// Set buildable status.
    /// C++ Reference: ThingTemplate.h m_buildable
    pub fn set_buildable(&mut self, status: BuildableStatus) {
        self.buildable = status;
    }

    /// Set whether this thing is considered a prerequisite for other things.
    /// C++ Reference: ThingTemplate.h m_isPrerequisite
    pub fn set_is_prerequisite(&mut self, value: bool) {
        self.is_prerequisite = value;
    }

    /// Set whether this thing is forbidden.
    /// C++ Reference: ThingTemplate.h m_isForbidden
    pub fn set_is_forbidden(&mut self, value: bool) {
        self.is_forbidden = value;
    }

    /// Set build cost.
    /// C++ Reference: ThingTemplate.h m_buildCost
    pub fn set_build_cost(&mut self, cost: UnsignedShort) {
        self.build_cost = cost;
    }

    /// Set build time.
    /// C++ Reference: ThingTemplate.h m_buildTime
    pub fn set_build_time(&mut self, time: Real) {
        self.build_time = time;
    }

    /// Set refund value.
    /// C++ Reference: ThingTemplate.h m_refundValue
    pub fn set_refund_value(&mut self, value: UnsignedShort) {
        self.refund_value = value;
    }

    /// Set default owning side.
    /// C++ Reference: ThingTemplate.h m_defaultOwningSide
    pub fn set_default_owning_side(&mut self, side: AsciiString) {
        self.default_owning_side = side;
    }

    /// Set command set string.
    /// C++ Reference: ThingTemplate.h m_commandSetString
    pub fn set_command_set_string(&mut self, cmd_set: AsciiString) {
        self.command_set_string = cmd_set;
    }

    /// Set build completion type.
    /// C++ Reference: ThingTemplate.h m_buildCompletion
    pub fn set_build_completion(&mut self, completion: BuildCompletionType) {
        self.build_completion = completion;
    }

    /// Clear and set all prerequisites.
    /// C++ Reference: ThingTemplate.h m_prereqInfo
    pub fn set_prereq_info(&mut self, prereqs: Vec<ProductionPrerequisite>) {
        self.prereq_info = prereqs;
    }

    /// Add a prerequisite entry.
    /// C++ Reference: ThingTemplate::parsePrerequisites pushes into m_prereqInfo
    pub fn add_prereq(&mut self, prereq: ProductionPrerequisite) {
        self.prereq_info.push(prereq);
    }

    /// Parse the `Prerequisites` INI block and populate m_prereqInfo.
    ///
    /// C++ Reference: ThingTemplate::parsePrerequisites (ThingTemplate.cpp lines 635-651)
    ///
    /// INI format:
    /// ```ini
    /// Prerequisites
    ///   Object = Barracks WarFactory    ; each line creates one ProductionPrerequisite
    ///   Science = SCIENCE_BattleDrone   ; science prereq
    /// End
    /// ```
    ///
    /// Each line produces a separate `ProductionPrerequisite` entry in `m_prereqInfo`.
    /// Tokens on an `Object` line are OR'd together (the first has no flag,
    /// subsequent tokens get UNIT_OR_WITH_PREV).
    /// Player::canBuild requires ALL entries to be satisfied (AND logic).
    pub fn parse_prerequisites_block(&mut self, lines: &[String]) {
        self.prereq_info.clear();

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            // Split on '=' to get key and value
            if let Some(eq_pos) = trimmed.find('=') {
                let key = trimmed[..eq_pos].trim();
                let value = trimmed[eq_pos + 1..].trim();

                if key.eq_ignore_ascii_case("Object") {
                    // C++ parsePrerequisiteUnit: each token is OR'd with previous
                    let mut prereq = ProductionPrerequisite::new();
                    let tokens: Vec<&str> = value.split_whitespace().collect();
                    for (i, token) in tokens.iter().enumerate() {
                        prereq.add_unit_prereq(token.to_string(), i > 0);
                    }
                    self.prereq_info.push(prereq);
                } else if key.eq_ignore_ascii_case("Science") {
                    // C++ parsePrerequisiteScience: lookup science by name
                    let mut prereq = ProductionPrerequisite::new();
                    if let Some(science_store) = get_science_store() {
                        let science_type = science_store.get_science_from_internal_name(value);
                        if science_type != SCIENCE_INVALID {
                            prereq.add_science_prereq(science_type);
                        } else {
                            #[cfg(any(debug_assertions, feature = "internal"))]
                            eprintln!("WARNING: could not find science prerequisite '{}'", value);
                        }
                    }
                    self.prereq_info.push(prereq);
                }
                // C++ only supports Object and Science in Prerequisites block
            }
        }
    }

    pub fn validate(&self) {
        // Validation logic would go here
        self.validate_audio();

        if self.name_string == "DefaultThingTemplate" {
            return;
        }

        // Additional validation checks...
    }

    fn validate_audio(&self) {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            // Audio validation would check if sounds exist in audio system
            for sound in &self.per_unit_sounds {
                if !sound.1.get_event_name().is_empty() && sound.1.get_event_name() != "NoSound" {
                    // Check if audio event is valid
                    // debug_assert!(TheAudio->isValidAudioEvent(&sound.1),
                    //              "Invalid UnitSpecificSound '{}' in Object '{}'",
                    //              sound.0, self.name_string);
                }
            }
        }
    }

    pub fn resolve_names(&mut self) {
        // Resolve prerequisite names
        for prereq in &mut self.prereq_info {
            prereq.resolve_names();
        }

        // Mark build facilities
        // This would iterate through prerequisites and mark templates as build facilities
        if self.is_kind_of(0x4000) {
            // KINDOF_COMMANDCENTER placeholder
            self.is_build_facility = true;
        }

        // Resolve image names
        if !self.selected_portrait_image_name.is_empty() {
            // self.selected_portrait_image = TheMappedImageCollection->findImageByName(name);
            self.selected_portrait_image_name.clear();
        }

        if !self.button_image_name.is_empty() {
            // self.button_image = TheMappedImageCollection->findImageByName(name);
            self.button_image_name.clear();
        }
    }

    #[cfg(feature = "load_test_assets")]
    pub fn init_for_lta(&mut self, name: &AsciiString) {
        self.name_string = name.clone();

        // Extract LTA name from full path
        let name_str = name.as_str();
        if let Some(slash_pos) = name_str.find('/') {
            self.lta_name = AsciiString::from(&name_str[slash_pos + 1..]);
        } else {
            self.lta_name = name.clone();
        }

        // Initialize default modules for test assets
        self.behavior_module_info.clear();
        self.draw_module_info.clear();
        self.client_update_module_info.clear();

        // Add default modules
        // This would add DestroyDie, InactiveBody, W3DDefaultDraw, etc.

        self.armor_copied_from_default = false;
        self.weapons_copied_from_default = false;
        self.kindof = 0;
        self.asset_scale = 1.0;
        self.instance_scale_fuzziness = 0.0;
        self.display_name = UnicodeString::from(name.as_str());
        self.shadow_type = ShadowType::Volume;
        self.geometry_info = GeometryInfo::new(GeometryType::Sphere, false, 10.0, 10.0, 10.0);
    }

    #[cfg(feature = "load_test_assets")]
    pub fn get_lta_name(&self) -> &AsciiString {
        &self.lta_name
    }

    // Override-related methods
    pub fn is_override(&self) -> bool {
        self.is_override
    }

    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }

    pub fn get_final_override(template: &Arc<ThingTemplate>) -> Arc<ThingTemplate> {
        let mut current = template.clone();
        loop {
            let next = current.next_override.read().unwrap().clone();
            if let Some(next) = next {
                current = next;
            } else {
                return current;
            }
        }
    }

    pub fn get_next_override(&self) -> Option<Arc<ThingTemplate>> {
        self.next_override.read().unwrap().clone()
    }

    pub fn set_next_override(&self, override_template: Option<Arc<ThingTemplate>>) {
        *self.next_override.write().unwrap() = override_template;
    }

    pub fn delete_overrides(&self) {
        *self.next_override.write().unwrap() = None;
    }

    pub fn is_null_template(&self) -> bool {
        self.name_string.is_empty()
    }

    // Calculation methods for build cost/time with player bonuses
    pub fn calc_cost_to_build(&self, _player: Option<&dyn std::any::Any>) -> i32 {
        // This would apply player handicaps and faction modifiers
        self.build_cost as i32
    }

    pub fn calc_time_to_build(&self, _player: Option<&dyn std::any::Any>) -> i32 {
        // This would apply player handicaps, energy penalties, etc.
        (self.build_time * 30.0) as i32 // Assuming 30 logic frames per second
    }

    pub fn is_buildable_item(&self) -> bool {
        self.build_cost != 0
    }

    // -----------------------------------------------------------------------
    // INI field parsing -- mirrors C++ s_objectFieldParseTable
    //
    // Each field here corresponds to an entry in the C++ field parse table
    // defined in ThingTemplate.cpp lines 90-229.
    // -----------------------------------------------------------------------

    /// Apply parsed INI key=value properties to this template.
    ///
    /// This is the Rust equivalent of `initFromINI(self, getFieldParse())` in C++.
    /// It reads each known INI field name and writes the value into the
    /// corresponding struct member.  Unknown fields are silently ignored so
    /// that forward-compatibility is maintained when new INI keys are added.
    ///
    /// WeaponSet and ArmorSet sub-blocks are handled by their own dedicated
    /// parsers (see `load_weapon_sets_from_definitions` and
    /// `parse_armor_set_from_properties`) and are NOT processed here.
    pub fn parse_object_fields_from_ini(
        &mut self,
        properties: &std::collections::HashMap<String, String>,
    ) {
        for (key, value) in properties {
            let trimmed = value.trim();
            match key.as_str() {
                // --- Display ---
                "DisplayName" => {
                    // C++ uses parseAndTranslateLabel -> UnicodeString
                    self.display_name = UnicodeString::from(trimmed);
                }
                "DisplayColor" => {
                    if let Ok(c) = parse_color_int(trimmed) {
                        self.display_color = c;
                    }
                }
                "EditorSorting" => {
                    self.editor_sorting = parse_editor_sorting(trimmed);
                }

                // --- Physical ---
                "Scale" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.asset_scale = v;
                    }
                }
                "InstanceScaleFuzziness" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.instance_scale_fuzziness = v;
                    }
                }

                // --- Radar & transport ---
                "RadarPriority" => {
                    self.radar_priority = parse_radar_priority(trimmed);
                }
                "TransportSlotCount" => {
                    if let Ok(v) = trimmed.parse::<UnsignedByte>() {
                        self.transport_slot_count = v;
                    }
                }

                // --- Fence / bridge ---
                "FenceWidth" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.fence_width = v;
                    }
                }
                "FenceXOffset" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.fence_x_offset = v;
                    }
                }
                "IsBridge" => {
                    if let Ok(v) = parse_bool_simple(trimmed) {
                        self.is_bridge = v;
                    }
                }

                // --- Vision / shroud ---
                "VisionRange" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.vision_range = v;
                    }
                }
                "ShroudClearingRange" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shroud_clearing_range = v;
                    }
                }
                "ShroudRevealToAllRange" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shroud_reveal_to_all_range = v;
                    }
                }

                // --- Placement / factory ---
                "PlacementViewAngle" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.placement_view_angle = v;
                    }
                }
                "FactoryExitWidth" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.factory_exit_width = v;
                    }
                }
                "FactoryExtraBibWidth" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.factory_extra_bib_width = v;
                    }
                }

                // --- Experience / skill ---
                "SkillPointValue" => {
                    parse_int_list_into(trimmed, &mut self.skill_point_values);
                }
                "ExperienceValue" => {
                    parse_int_list_into(trimmed, &mut self.experience_values);
                }
                "ExperienceRequired" => {
                    parse_int_list_into(trimmed, &mut self.experience_required);
                }
                "IsTrainable" => {
                    if let Ok(v) = parse_bool_simple(trimmed) {
                        self.is_trainable = v;
                    }
                }
                "EnterGuard" => {
                    if let Ok(v) = parse_bool_simple(trimmed) {
                        self.enter_guard = v;
                    }
                }
                "HijackGuard" => {
                    if let Ok(v) = parse_bool_simple(trimmed) {
                        self.hijack_guard = v;
                    }
                }

                // --- Side ---
                "Side" => {
                    self.default_owning_side = AsciiString::from(trimmed);
                }

                // --- Build ---
                "Buildable" => {
                    self.buildable = parse_buildable_status(trimmed);
                }
                "BuildCost" => {
                    if let Ok(v) = trimmed.parse::<UnsignedShort>() {
                        self.build_cost = v;
                    }
                }
                "BuildTime" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.build_time = v;
                    }
                }
                "RefundValue" => {
                    if let Ok(v) = trimmed.parse::<UnsignedShort>() {
                        self.refund_value = v;
                    }
                }
                "BuildCompletion" => {
                    self.build_completion = parse_build_completion(trimmed);
                }
                "EnergyProduction" => {
                    if let Ok(v) = trimmed.parse::<i32>() {
                        self.energy_production = v;
                    }
                }
                "EnergyBonus" => {
                    if let Ok(v) = trimmed.parse::<i32>() {
                        self.energy_bonus = v;
                    }
                }
                "IsForbidden" => {
                    if let Ok(v) = parse_bool_simple(trimmed) {
                        self.is_forbidden = v;
                    }
                }
                "IsPrerequisite" => {
                    if let Ok(v) = parse_bool_simple(trimmed) {
                        self.is_prerequisite = v;
                    }
                }

                // --- Command set / build variations ---
                "CommandSet" => {
                    self.command_set_string = AsciiString::from(trimmed);
                }
                "BuildVariations" => {
                    self.build_variations = trimmed
                        .split_whitespace()
                        .map(|s| AsciiString::from(s))
                        .collect();
                }

                // --- KindOf ---
                "KindOf" => {
                    use crate::common::system::kind_of::KindOfMask;
                    let mut mask = KindOfMask::empty();
                    for token in trimmed.split_whitespace() {
                        if let Some(flag) = KindOfMask::from_string(token) {
                            mask |= flag;
                        }
                    }
                    self.kindof = mask.bits() as u64;
                }

                // --- UI ---
                "SelectPortrait" => {
                    self.selected_portrait_image_name = AsciiString::from(trimmed);
                }
                "ButtonImage" => {
                    self.button_image_name = AsciiString::from(trimmed);
                }
                "UpgradeCameo1" => {
                    self.upgrade_cameo_upgrade_names[0] = AsciiString::from(trimmed);
                }
                "UpgradeCameo2" => {
                    self.upgrade_cameo_upgrade_names[1] = AsciiString::from(trimmed);
                }
                "UpgradeCameo3" => {
                    self.upgrade_cameo_upgrade_names[2] = AsciiString::from(trimmed);
                }
                "UpgradeCameo4" => {
                    self.upgrade_cameo_upgrade_names[3] = AsciiString::from(trimmed);
                }
                "UpgradeCameo5" => {
                    self.upgrade_cameo_upgrade_names[4] = AsciiString::from(trimmed);
                }

                // --- Shadow ---
                "Shadow" => {
                    self.shadow_type = parse_shadow_type(trimmed);
                }
                "ShadowSizeX" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shadow_size_x = v;
                    }
                }
                "ShadowSizeY" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shadow_size_y = v;
                    }
                }
                "ShadowOffsetX" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shadow_offset_x = v;
                    }
                }
                "ShadowOffsetY" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.shadow_offset_y = v;
                    }
                }
                "ShadowTexture" => {
                    self.shadow_texture_name = AsciiString::from(trimmed);
                }

                // --- Occlusion ---
                "OcclusionDelay" => {
                    // C++ uses parseDurationUnsignedInt -- frames at 30 FPS
                    if let Ok(v) = trimmed.parse::<u32>() {
                        self.occlusion_delay = v;
                    }
                }

                // --- Combat ---
                "ThreatValue" => {
                    if let Ok(v) = trimmed.parse::<UnsignedShort>() {
                        self.threat_value = v;
                    }
                }
                "MaxSimultaneousOfType" => {
                    if trimmed.eq_ignore_ascii_case("DeterminedBySuperweaponRestriction") {
                        self.max_simultaneous_determined_by_superweapon_restriction = true;
                        self.max_simultaneous_of_type = 0;
                    } else if let Ok(v) = trimmed.parse::<UnsignedShort>() {
                        self.max_simultaneous_of_type = v;
                    }
                }
                "CrusherLevel" => {
                    if let Ok(v) = trimmed.parse::<UnsignedByte>() {
                        self.crusher_level = v;
                    }
                }
                "CrushableLevel" => {
                    if let Ok(v) = trimmed.parse::<UnsignedByte>() {
                        self.crushable_level = v;
                    }
                }

                // --- Structure ---
                "StructureRubbleHeight" => {
                    if let Ok(v) = trimmed.parse::<UnsignedByte>() {
                        self.structure_rubble_height = v;
                    }
                }

                // --- Geometry (delegated to GeometryInfo) ---
                "Geometry" => {
                    self.geometry_info.geometry_type = parse_geometry_type(trimmed);
                }
                "GeometryMajorRadius" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.geometry_info.width = v;
                    }
                }
                "GeometryMinorRadius" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.geometry_info.depth = v;
                    }
                }
                "GeometryHeight" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.geometry_info.height = v;
                    }
                }
                "GeometryIsSmall" => {
                    if let Ok(v) = parse_bool_simple(trimmed) {
                        self.geometry_info.is_small = v;
                    }
                }

                // --- WeaponSet / ArmorSet are handled separately ---
                "WeaponSet" | "ArmorSet" | "Prerequisites" => {
                    // Sub-block fields parsed by dedicated methods
                }

                // Everything else: silently skip (module blocks, etc.)
                _ => {}
            }
        }
    }

    /// Set the KindOf mask from a resolved bitmask.
    ///
    /// Called by the GameLogic layer after resolving KindOf flag names to bits.
    pub fn set_kindof_mask(&mut self, mask: u64) {
        self.kindof = mask;
    }
}

// ---------------------------------------------------------------------------
// INI field parsing helpers
// ---------------------------------------------------------------------------

fn parse_bool_simple(s: &str) -> Result<bool, ()> {
    match s {
        "yes" | "Yes" | "YES" | "true" | "True" | "TRUE" | "1" => Ok(true),
        "no" | "No" | "NO" | "false" | "False" | "FALSE" | "0" => Ok(false),
        _ => Err(()),
    }
}

fn parse_color_int(s: &str) -> Result<Color, ()> {
    // C++ parseColorInt: expects RRGGBB hex, stored as ARGB u32
    let v = u32::from_str_radix(s.trim_start_matches("0x"), 16).map_err(|_| ())?;
    Ok(Color(0xFF000000 | v))
}

fn parse_editor_sorting(s: &str) -> EditorSortingType {
    match s.trim() {
        "Unit" => EditorSortingType::Unit,
        "Building" => EditorSortingType::Building,
        "Infrastructure" => EditorSortingType::Infrastructure,
        "Civilian" => EditorSortingType::Civilian,
        _ => EditorSortingType::Invalid,
    }
}

fn parse_radar_priority(s: &str) -> RadarPriorityType {
    match s.trim() {
        "Low" => RadarPriorityType::Low,
        "Medium" => RadarPriorityType::Medium,
        "High" => RadarPriorityType::High,
        "Critical" => RadarPriorityType::Critical,
        _ => RadarPriorityType::Invalid,
    }
}

fn parse_buildable_status(s: &str) -> BuildableStatus {
    match s.trim() {
        "IgnorePrerequisites" => BuildableStatus::IgnorePrerequisites,
        "No" => BuildableStatus::No,
        "OnlyByAI" => BuildableStatus::OnlyByAi,
        _ => BuildableStatus::Yes,
    }
}

fn parse_build_completion(s: &str) -> BuildCompletionType {
    match s.trim() {
        "PlacedByPlayer" => BuildCompletionType::PlacedByPlayer,
        _ => BuildCompletionType::AppearsAtRallyPoint,
    }
}

fn parse_shadow_type(s: &str) -> ShadowType {
    match s.trim() {
        "VOLUME" | "Volume" => ShadowType::Volume,
        "DECAL" | "Decal" => ShadowType::Decal,
        _ => ShadowType::None,
    }
}

fn parse_geometry_type(s: &str) -> GeometryType {
    match s.trim() {
        "SPHERE" | "Sphere" => GeometryType::Sphere,
        "CYLINDER" | "Cylinder" => GeometryType::Cylinder,
        "BOX" | "Box" => GeometryType::Box,
        _ => GeometryType::Sphere,
    }
}

/// Parse a space-separated list of integers into a fixed-size array.
/// Mirrors C++ ThingTemplate::parseIntList.
fn parse_int_list_into(s: &str, out: &mut [i32; LEVEL_COUNT]) {
    let tokens: Vec<&str> = s.split_whitespace().collect();
    for (i, token) in tokens.iter().enumerate() {
        if i >= LEVEL_COUNT {
            break;
        }
        if *token == "USE_EXP_VALUE" || *token == "-999" {
            out[i] = USE_EXP_VALUE_FOR_SKILL_VALUE;
        } else if let Ok(v) = token.parse::<i32>() {
            out[i] = v;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::bit_flags::{
        ArmorSetFlags as ArmorSetBits, WeaponSetFlags as WeaponSetBits,
    };
    use crate::common::thing::module::ModuleType;
    use std::sync::Arc;

    #[test]
    fn find_weapon_template_set_respects_flags() {
        let mut template = ThingTemplate::new();

        let mut base = WeaponTemplateSet::new();
        base.set_weapon_template_name(0, Some(AsciiString::from("BasePrimary")));
        template.add_weapon_template_set(base);

        let mut hero = WeaponTemplateSet::new();
        hero.types_mut().set(WeaponSetBits::HERO, true);
        hero.set_weapon_template_name(0, Some(AsciiString::from("HeroPrimary")));
        template.add_weapon_template_set(hero);

        let flags = create_weapon_set_flags();
        let base_set = template
            .find_weapon_template_set(&flags)
            .expect("expected base weapon set");
        assert_eq!(
            base_set.weapon_template_name(0).map(|name| name.as_str()),
            Some("BasePrimary"),
        );

        let mut hero_flags = create_weapon_set_flags();
        hero_flags.set(WeaponSetBits::HERO, true);
        let hero_set = template
            .find_weapon_template_set(&hero_flags)
            .expect("expected hero weapon set");
        assert_eq!(
            hero_set.weapon_template_name(0).map(|name| name.as_str()),
            Some("HeroPrimary"),
        );
    }

    #[test]
    fn load_weapon_sets_from_definitions_populates_template() {
        let mut definition = WeaponSetDefinition::new();
        definition.add_condition("Hero");
        definition.set_weapon_name(0, Some(AsciiString::from("HeroPrimary")));
        definition.set_auto_choose_mask(0, Some(0x1));
        definition.set_preferred_against_mask(0, Some(0x2));
        definition.set_share_reload_time(Some(true));
        definition.set_share_weapon_lock(Some(false));

        let mut template = ThingTemplate::new();
        template
            .load_weapon_sets_from_definitions(&[definition])
            .expect("load weapon sets");

        assert_eq!(template.weapon_template_sets().len(), 1);
        let engine_set = &template.weapon_template_sets()[0];
        assert!(engine_set.types().test(WeaponSetBits::HERO));
        assert_eq!(
            engine_set.weapon_template_name(0).map(|name| name.as_str()),
            Some("HeroPrimary"),
        );
        assert_eq!(engine_set.auto_choose_mask(0), 0x1);
        assert_eq!(engine_set.preferred_against_mask(0), 0x2);
        assert!(engine_set.is_reload_time_shared());
        assert!(!engine_set.is_weapon_lock_shared_across_sets());
    }

    #[test]
    fn module_descriptor_helpers_reflect_module_info() {
        let mut template = ThingTemplate::new();

        let behavior_data: Arc<dyn ModuleData> = Arc::new(BaseModuleData::new());
        template.behavior_module_info.add_module_info(
            AsciiString::from("TestBehavior"),
            AsciiString::from("TagBehavior"),
            behavior_data,
            ModuleInterfaceType::BODY.0 as i32,
            false,
            false,
        );

        let draw_data: Arc<dyn ModuleData> = Arc::new(BaseModuleData::new());
        template.draw_module_info.add_module_info(
            AsciiString::from("TestDraw"),
            AsciiString::from("TagDraw"),
            draw_data,
            ModuleInterfaceType::DRAW.0 as i32,
            false,
            false,
        );

        let client_update_data: Arc<dyn ModuleData> = Arc::new(BaseModuleData::new());
        template.client_update_module_info.add_module_info(
            AsciiString::from("TestClientUpdate"),
            AsciiString::from("TagClient"),
            client_update_data,
            ModuleInterfaceType::CLIENT_UPDATE.0 as i32,
            false,
            false,
        );

        let descriptor_set = template.module_descriptors();

        assert_eq!(descriptor_set.behavior.len(), 1);
        assert_eq!(descriptor_set.draw.len(), 1);
        assert_eq!(descriptor_set.client_update.len(), 1);

        assert_eq!(
            template
                .module_descriptors_for_type(ModuleType::Behavior)
                .len(),
            1
        );
        assert_eq!(
            template
                .module_descriptors_for_type(ModuleType::Behavior)
                .first()
                .map(|d| d.name.as_str()),
            Some("TestBehavior"),
        );
        assert_eq!(
            descriptor_set
                .for_type(ModuleType::Draw)
                .first()
                .map(|d| d.module_tag.as_str()),
            Some("TagDraw"),
        );
        assert_eq!(
            template
                .module_descriptors_for_type(ModuleType::ClientUpdate)
                .first()
                .map(|d| d.name.as_str()),
            Some("TestClientUpdate"),
        );

        let behavior_descriptor = &descriptor_set.behavior[0];
        assert!(behavior_descriptor.supports(ModuleInterfaceType::BODY));
        assert_eq!(behavior_descriptor.name.as_str(), "TestBehavior");

        let draw_descriptor = &descriptor_set.draw[0];
        assert!(draw_descriptor.supports(ModuleInterfaceType::DRAW));
        assert_eq!(draw_descriptor.module_tag.as_str(), "TagDraw");

        let client_descriptor = &descriptor_set.client_update[0];
        assert!(client_descriptor.supports(ModuleInterfaceType::CLIENT_UPDATE));
        assert_eq!(client_descriptor.name.as_str(), "TestClientUpdate");
    }

    #[test]
    fn module_descriptors_register_with_global_factory() {
        clear_pending_descriptors_for_test();
        let mut guard = get_module_factory().expect("module factory mutex poisoned");
        let previous = guard.take();
        *guard = Some(ModuleFactory::new());
        drop(guard);

        let mut template = ThingTemplate::new();
        let behavior_data: Arc<dyn ModuleData> = Arc::new(BaseModuleData::new());
        template.behavior_module_info.add_module_info(
            AsciiString::from("AutoHealBehavior"),
            AsciiString::from("TagBehavior"),
            behavior_data,
            ModuleInterfaceType::BODY.0 as i32,
            false,
            false,
        );

        let descriptors = template.module_descriptors();
        assert_eq!(descriptors.behavior.len(), 1, "descriptor not surfaced");

        {
            let guard = get_module_factory().expect("module factory mutex poisoned");
            let factory = guard
                .as_ref()
                .expect("module factory should be initialized for descriptor sync");
            let name = AsciiString::from("AutoHealBehavior");
            assert!(
                factory
                    .descriptor_for(ModuleType::Behavior, &name)
                    .is_some(),
                "descriptor should be recorded in global factory"
            );
        }

        let mut guard = get_module_factory().expect("module factory mutex poisoned");
        *guard = previous;
        drop(guard);
        clear_pending_descriptors_for_test();
    }

    #[test]
    fn can_possibly_have_any_weapon_reflects_assigned_templates() {
        let mut template = ThingTemplate::new();
        assert!(!template.can_possibly_have_any_weapon());

        template.add_weapon_template_set(WeaponTemplateSet::new());
        assert!(!template.can_possibly_have_any_weapon());

        let mut armed_set = WeaponTemplateSet::new();
        armed_set.set_weapon_template_name(0, Some(AsciiString::from("ArmedPrimary")));
        template.add_weapon_template_set(armed_set);
        assert!(template.can_possibly_have_any_weapon());
    }

    #[test]
    fn is_kind_of_handles_high_bit_masks_without_panicking() {
        let mut template = ThingTemplate::new();
        template.kindof = 0x1000;

        assert!(template.is_kind_of(0x1000));
        assert!(!template.is_kind_of(0x4000));
    }

    #[test]
    fn find_armor_template_set_respects_flags() {
        let mut template = ThingTemplate::new();

        let mut base = ArmorTemplateSet::new();
        base.set_armor_template_name(Some(AsciiString::from("Base")));
        template.add_armor_template_set(base);

        let mut hero = ArmorTemplateSet::new();
        hero.types_mut().set(ArmorSetBits::HERO, true);
        hero.set_armor_template_name(Some(AsciiString::from("Hero")));
        template.add_armor_template_set(hero);

        let flags = create_armor_set_flags();
        let base_set = template
            .find_armor_template_set(&flags)
            .expect("expected base set");
        assert_eq!(base_set.armor_template_name().unwrap().as_str(), "Base");

        let mut hero_flags = create_armor_set_flags();
        hero_flags.set(ArmorSetBits::HERO, true);
        let hero_set = template
            .find_armor_template_set(&hero_flags)
            .expect("expected hero set");
        assert_eq!(hero_set.armor_template_name().unwrap().as_str(), "Hero");
    }
}

impl Overridable for ThingTemplate {
    fn is_override(&self) -> bool {
        ThingTemplate::is_override(self)
    }

    fn delete_overrides(&self) {
        ThingTemplate::delete_overrides(self)
    }
}

impl Snapshotable for ThingTemplate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // CRC implementation
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // Serialization implementation
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Post-load processing
        Ok(())
    }
}
