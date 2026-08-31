////////////////////////////////////////////////////////////////////////////////
//																			//
//  (c) 2001-2003 Electronic Arts Inc.										//
//																			//
////////////////////////////////////////////////////////////////////////////////

//! Thing templates are a 'roadmap' to creating things
//! Contains all the data needed to construct objects and drawables

use crate::common::bit_flags::{
    ArmorSetBitFlags, BitFlags, WeaponSetBitFlags, create_armor_set_flags, create_weapon_set_flags,
};
use crate::common::ini::INI;
use crate::common::ini::ini_mapped_image::{Image, get_mapped_image_collection};

use crate::common::system::Snapshotable;
use crate::common::thing::module::{BaseModuleData, CapturedModuleData};
#[cfg(test)]
use crate::common::thing::module_factory::clear_pending_descriptors_for_test;
use crate::common::thing::module_factory::{
    ModuleFactory, get_module_factory, register_descriptor_set_global,
};
use crate::common::thing::sparse_match_finder::{
    SparseBitSet, SparseMatchCandidate, SparseMatchFinder,
};
use crate::common::{
    audio::AudioEventRts,
    global_data,
    name_key_generator::NameKeyGenerator,
    rts::{
        AsciiString, Color, NameKeyType, ProductionPrerequisite, Real, SCIENCE_INVALID,
        UnicodeString, UnsignedByte, UnsignedShort, get_science_store,
    },
    system::{
        Overridable, Xfer,
        geometry::{GeometryInfo, GeometryType},
    },
    thing::module::{ModuleData, ModuleInterfaceType, ModuleType},
};
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, RwLock},
};

#[path = "thing_template_snapshot.rs"]
mod thing_template_snapshot;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

const CPP_OBJECT_FIELDS: &[&str] = &[
    "DisplayName",
    "RadarPriority",
    "TransportSlotCount",
    "FenceWidth",
    "FenceXOffset",
    "IsBridge",
    "ArmorSet",
    "WeaponSet",
    "VisionRange",
    "ShroudClearingRange",
    "ShroudRevealToAllRange",
    "PlacementViewAngle",
    "FactoryExitWidth",
    "FactoryExtraBibWidth",
    "SkillPointValue",
    "ExperienceValue",
    "ExperienceRequired",
    "IsTrainable",
    "EnterGuard",
    "HijackGuard",
    "Side",
    "Prerequisites",
    "Buildable",
    "BuildCost",
    "BuildTime",
    "RefundValue",
    "BuildCompletion",
    "EnergyProduction",
    "EnergyBonus",
    "IsForbidden",
    "IsPrerequisite",
    "DisplayColor",
    "EditorSorting",
    "KindOf",
    "CommandSet",
    "BuildVariations",
    "Behavior",
    "Body",
    "Draw",
    "ClientUpdate",
    "SelectPortrait",
    "ButtonImage",
    "UpgradeCameo1",
    "UpgradeCameo2",
    "UpgradeCameo3",
    "UpgradeCameo4",
    "UpgradeCameo5",
    "VoiceSelect",
    "VoiceGroupSelect",
    "VoiceMove",
    "VoiceAttack",
    "VoiceEnter",
    "VoiceFear",
    "VoiceSelectElite",
    "VoiceCreated",
    "VoiceTaskUnable",
    "VoiceTaskComplete",
    "VoiceMeetEnemy",
    "VoiceGarrison",
    "VoiceDefect",
    "VoiceAttackSpecial",
    "VoiceAttackAir",
    "VoiceGuard",
    "SoundMoveStart",
    "SoundMoveStartDamaged",
    "SoundMoveLoop",
    "SoundMoveLoopDamaged",
    "SoundAmbient",
    "SoundAmbientDamaged",
    "SoundAmbientReallyDamaged",
    "SoundAmbientRubble",
    "SoundStealthOn",
    "SoundStealthOff",
    "SoundCreated",
    "SoundOnDamaged",
    "SoundOnReallyDamaged",
    "SoundEnter",
    "SoundExit",
    "SoundPromotedVeteran",
    "SoundPromotedElite",
    "SoundPromotedHero",
    "SoundFallingFromPlane",
    "UnitSpecificSounds",
    "UnitSpecificFX",
    "Scale",
    "Geometry",
    "GeometryMajorRadius",
    "GeometryMinorRadius",
    "GeometryHeight",
    "GeometryIsSmall",
    "Shadow",
    "ShadowSizeX",
    "ShadowSizeY",
    "ShadowOffsetX",
    "ShadowOffsetY",
    "ShadowTexture",
    "OcclusionDelay",
    "AddModule",
    "RemoveModule",
    "ReplaceModule",
    "InheritableModule",
    "OverrideableByLikeKind",
    "Locomotor",
    "InstanceScaleFuzziness",
    "StructureRubbleHeight",
    "ThreatValue",
    "MaxSimultaneousOfType",
    "MaxSimultaneousLinkKey",
    "CrusherLevel",
    "CrushableLevel",
];

fn is_cpp_object_field(key: &str) -> bool {
    let key = property_base_key(key);
    CPP_OBJECT_FIELDS
        .iter()
        .any(|field| field.eq_ignore_ascii_case(key))
        || key.starts_with("WeaponSet")
        || key.starts_with("ArmorSet")
        || is_module_body_property(key)
}

fn is_module_body_property(key: &str) -> bool {
    key.split_once('.')
        .map(|(header, _)| {
            let base = property_base_key(header);
            is_module_object_field(base)
        })
        .unwrap_or(false)
}

fn property_base_key(key: &str) -> &str {
    if let Some((base, repeat)) = key.rsplit_once('#') {
        if repeat.parse::<usize>().is_ok() {
            return base;
        }
    }
    key
}

fn property_repeat_index(key: &str) -> usize {
    key.rsplit_once('#')
        .and_then(|(_, repeat)| repeat.parse::<usize>().ok())
        .unwrap_or(0)
}

fn module_field_order(field: &str) -> usize {
    match field {
        "Behavior" => 0,
        "Body" => 1,
        "Draw" => 2,
        "ClientUpdate" => 3,
        _ => usize::MAX,
    }
}

fn is_module_object_field(field: &str) -> bool {
    matches!(field, "Behavior" | "Body" | "Draw" | "ClientUpdate")
}

const CPP_RESKIN_FIELDS: &[&str] = &[
    "Draw",
    "Geometry",
    "GeometryMajorRadius",
    "GeometryMinorRadius",
    "GeometryHeight",
    "GeometryIsSmall",
    "FenceWidth",
    "FenceXOffset",
    "MaxSimultaneousOfType",
    "MaxSimultaneousLinkKey",
];

fn is_reskin_property(key: &str) -> bool {
    let base = property_base_key(key);
    if CPP_RESKIN_FIELDS
        .iter()
        .any(|field| field.eq_ignore_ascii_case(base))
    {
        return true;
    }
    key.split_once('.')
        .map(|(header, _)| property_base_key(header).eq_ignore_ascii_case("Draw"))
        .unwrap_or(false)
}

fn collect_module_body(
    properties: &HashMap<String, String>,
    header_key: &str,
) -> (String, HashMap<String, String>) {
    let prefix = format!("{}.", header_key);
    let mut fields = HashMap::new();
    let mut raw_body = String::new();
    for (key, value) in properties {
        let Some(rest) = key.strip_prefix(&prefix) else {
            continue;
        };
        if rest == "__body" {
            raw_body = value.clone();
            continue;
        }
        if rest == "<raw>" || rest.starts_with("<raw>#") {
            continue;
        }
        fields.insert(rest.to_string(), value.clone());
    }
    if raw_body.is_empty() && !fields.is_empty() {
        raw_body = fields
            .iter()
            .map(|(key, value)| format!("{} = {}", key, value))
            .collect::<Vec<_>>()
            .join("\n");
    }
    (raw_body, fields)
}

fn parse_override_module_body(lines: &[&str]) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    let mut prefix: Option<String> = None;
    let mut depth = 0u32;
    let mut body_lines: Vec<String> = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let first = line.split_whitespace().next().unwrap_or("");
        if first.eq_ignore_ascii_case("End") {
            if depth > 0 {
                if depth > 1 {
                    body_lines.push("End".to_string());
                }
                depth -= 1;
                if depth == 0 {
                    if let Some(prefix) = prefix.take() {
                        if !body_lines.is_empty() {
                            properties.insert(format!("{}.__body", prefix), body_lines.join("\n"));
                        }
                    }
                    body_lines.clear();
                }
            }
            continue;
        }
        if depth == 0 {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if is_module_object_field(key) {
                    insert_repeated_local(&mut properties, key.to_string(), value.to_string());
                    prefix = Some(current_repeatable_local(&properties, key));
                    depth = 1;
                    body_lines.clear();
                }
            }
            continue;
        }
        body_lines.push(line.to_string());
        if let Some((key, value)) = line.split_once('=') {
            if let Some(prefix) = prefix.as_deref() {
                insert_repeated_local(
                    &mut properties,
                    format!("{}.{}", prefix, key.trim()),
                    value.trim().to_string(),
                );
            }
            if is_module_object_field(key.trim())
                || key.trim().eq_ignore_ascii_case("ConditionState")
                || key.trim().eq_ignore_ascii_case("TransitionState")
            {
                depth += 1;
            }
        } else if first.eq_ignore_ascii_case("DefaultConditionState")
            || first.eq_ignore_ascii_case("ConditionState")
            || first.eq_ignore_ascii_case("TransitionState")
        {
            depth += 1;
        }
    }
    properties
}

fn insert_repeated_local(properties: &mut HashMap<String, String>, key: String, value: String) {
    if !properties.contains_key(&key) {
        properties.insert(key, value);
        return;
    }
    for index in 1.. {
        let repeated = format!("{}#{}", key, index);
        if !properties.contains_key(&repeated) {
            properties.insert(repeated, value);
            return;
        }
    }
}

fn current_repeatable_local(properties: &HashMap<String, String>, field: &str) -> String {
    let mut last = field.to_string();
    for index in 1.. {
        let repeated = format!("{}#{}", field, index);
        if properties.contains_key(&repeated) {
            last = repeated;
        } else {
            break;
        }
    }
    last
}

fn module_data_from_body(
    module_name: &str,
    module_type: ModuleType,
    module_tag: &str,
    raw_body: &str,
    fields: HashMap<String, String>,
) -> Arc<dyn ModuleData> {
    if let Some(data) = try_factory_module_data(module_name, module_type, module_tag, raw_body) {
        return data;
    }
    if !raw_body.trim().is_empty() || !fields.is_empty() {
        return Arc::new(CapturedModuleData::new(
            module_tag,
            raw_body.to_string(),
            fields,
        ));
    }
    let mut data = BaseModuleData::new();
    data.set_module_tag_name_key(NameKeyGenerator::name_to_key(module_tag));
    Arc::new(data)
}

fn try_factory_module_data(
    module_name: &str,
    module_type: ModuleType,
    module_tag: &str,
    raw_body: &str,
) -> Option<Arc<dyn ModuleData>> {
    let mut guard = get_module_factory().ok()?;
    let factory = guard.as_mut()?;
    if !factory.has_module_data_proc(module_name, module_type) {
        return None;
    }
    let synthetic = if raw_body.trim().is_empty() {
        "End\n".to_string()
    } else {
        format!("{}\nEnd\n", raw_body.trim_end())
    };
    let mut ini = INI::new();
    ini.with_inline_source(&synthetic, |ini| {
        factory
            .try_new_module_data_from_ini(Some(ini), module_name, module_type, module_tag)
            .ok_or(crate::common::ini::INIError::InvalidData)
    })
    .ok()
}

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

fn object_audio_field_type(field: &str) -> Option<ThingTemplateAudioType> {
    match field {
        "VoiceSelect" => Some(ThingTemplateAudioType::VoiceSelect),
        "VoiceGroupSelect" => Some(ThingTemplateAudioType::VoiceGroupSelect),
        "VoiceMove" => Some(ThingTemplateAudioType::VoiceMove),
        "VoiceAttack" => Some(ThingTemplateAudioType::VoiceAttack),
        "VoiceEnter" => Some(ThingTemplateAudioType::VoiceEnter),
        "VoiceFear" => Some(ThingTemplateAudioType::VoiceFear),
        "VoiceSelectElite" => Some(ThingTemplateAudioType::VoiceSelectElite),
        "VoiceCreated" => Some(ThingTemplateAudioType::VoiceCreated),
        "VoiceTaskUnable" => Some(ThingTemplateAudioType::VoiceTaskUnable),
        "VoiceTaskComplete" => Some(ThingTemplateAudioType::VoiceTaskComplete),
        "VoiceMeetEnemy" => Some(ThingTemplateAudioType::VoiceNearEnemy),
        "VoiceGarrison" => Some(ThingTemplateAudioType::VoiceGarrison),
        "VoiceDefect" => Some(ThingTemplateAudioType::VoiceDefect),
        "VoiceAttackSpecial" => Some(ThingTemplateAudioType::VoiceAttackSpecial),
        "VoiceAttackAir" => Some(ThingTemplateAudioType::VoiceAttackAir),
        "VoiceGuard" => Some(ThingTemplateAudioType::VoiceGuard),
        "SoundMoveStart" => Some(ThingTemplateAudioType::SoundMoveStart),
        "SoundMoveStartDamaged" => Some(ThingTemplateAudioType::SoundMoveStartDamaged),
        "SoundMoveLoop" => Some(ThingTemplateAudioType::SoundMoveLoop),
        "SoundMoveLoopDamaged" => Some(ThingTemplateAudioType::SoundMoveLoopDamaged),
        "SoundAmbient" => Some(ThingTemplateAudioType::SoundAmbient),
        "SoundAmbientDamaged" => Some(ThingTemplateAudioType::SoundAmbientDamaged),
        "SoundAmbientReallyDamaged" => Some(ThingTemplateAudioType::SoundAmbientReallyDamaged),
        "SoundAmbientRubble" => Some(ThingTemplateAudioType::SoundAmbientRubble),
        "SoundStealthOn" => Some(ThingTemplateAudioType::SoundStealthOn),
        "SoundStealthOff" => Some(ThingTemplateAudioType::SoundStealthOff),
        "SoundCreated" => Some(ThingTemplateAudioType::SoundCreated),
        "SoundOnDamaged" => Some(ThingTemplateAudioType::SoundOnDamaged),
        "SoundOnReallyDamaged" => Some(ThingTemplateAudioType::SoundOnReallyDamaged),
        "SoundEnter" => Some(ThingTemplateAudioType::SoundEnter),
        "SoundExit" => Some(ThingTemplateAudioType::SoundExit),
        "SoundPromotedVeteran" => Some(ThingTemplateAudioType::SoundPromotedVeteran),
        "SoundPromotedElite" => Some(ThingTemplateAudioType::SoundPromotedElite),
        "SoundPromotedHero" => Some(ThingTemplateAudioType::SoundPromotedHero),
        "SoundFallingFromPlane" => Some(ThingTemplateAudioType::SoundFalling),
        _ => None,
    }
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
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarPriorityType {
    Invalid = 0,
    NotOnRadar,
    Structure,
    Unit,
    LocalUnitOnly,
}

/// Editor sorting types, preserving C++ Common/ThingSort.h discriminants.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorSortingType {
    None = 0,
    Structure,
    Infantry,
    Vehicle,
    Shrubbery,
    MiscManMade,
    MiscNatural,
    Debris,
    System,
    Audio,
    Test,
    ForReview,
    Road,
    Waypoint,
    NumSortingTypes,
}

/// Shadow types. Values match C++ `Shadow.h` (`TheShadowNames` / `parseBitString8`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShadowType(pub u8);

impl ShadowType {
    pub const None: Self = Self(0);
    pub const Decal: Self = Self(0x01);
    pub const Volume: Self = Self(0x02);
    pub const Projection: Self = Self(0x04);
    pub const DynamicProjection: Self = Self(0x08);
    pub const DirectionalProjection: Self = Self(0x10);
    pub const AlphaDecal: Self = Self(0x20);
    pub const AdditiveDecal: Self = Self(0x40);

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }
}

/// Module parsing modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
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

/// Full-width KindOf mask from named flags (bits >= 64 stay set).
fn kindof_mask_from(flags: crate::common::system::kind_of::KindOfMask) -> u128 {
    flags.bits()
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

    fn replace_first_ai_module_data(&mut self, data: Arc<dyn ModuleData>) -> bool {
        if let Some(nugget) = self
            .info
            .iter_mut()
            .find(|nugget| nugget.data.is_ai_module_data())
        {
            nugget.data = data;
            return true;
        }
        false
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
        is_trainable: bool,
        disallowed: bool,
        candidate: bool,
    ) -> bool {
        // C++ Reference: ThingTemplate.cpp line 382-455 clearCopiedFromDefaultEntries
        //
        // KindOf masks computed by the caller so we do not borrow ThingTemplate
        // while mutating ModuleInfo owned by that same template.
        let mut removed_any = false;
        let mut i = 0;
        while i < self.info.len() {
            let nugget = &self.info[i];
            if (nugget.interface_mask & interface_mask) != 0 && nugget.copied_from_default {
                if nugget.inheritable {
                    // Special case: don't inherit DefaultAutoHealBehavior if template
                    // is not trainable (module would be entirely useless).
                    if nugget.module_tag == "ModuleTag_DefaultAutoHealBehavior" && !is_trainable {
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

pub type PerUnitFxMap = HashMap<AsciiString, Option<Arc<crate::common::ini::ini_fx_list::FXList>>>;

/// Weapon template set definition mirroring the legacy C++ structure.
#[derive(Debug, Clone)]
pub struct WeaponTemplateSet {
    /// Bit-flag mask describing when this weapon set applies.
    types: WeaponSetBitFlags,
    /// Optional weapon template names for each slot (PRIMARY, SECONDARY, TERTIARY).
    weapon_template_names: [Option<AsciiString>; WEAPON_SLOT_COUNT],
    /// Command source mask per slot mirroring auto-choose rules.
    auto_choose_masks: [u32; WEAPON_SLOT_COUNT],
    /// Preferred target kind mask per slot.
    preferred_against_masks: [crate::common::system::kind_of::KindOfMask; WEAPON_SLOT_COUNT],
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
            preferred_against_masks: [crate::common::system::kind_of::KindOfMask::empty();
                WEAPON_SLOT_COUNT],
            is_reload_time_shared: false,
            is_weapon_lock_shared_across_sets: false,
        }
    }

    /// Reset the set to its default state.
    pub fn clear(&mut self) {
        self.types.clear();
        self.weapon_template_names = [None, None, None];
        self.auto_choose_masks = [u32::MAX; WEAPON_SLOT_COUNT];
        self.preferred_against_masks =
            [crate::common::system::kind_of::KindOfMask::empty(); WEAPON_SLOT_COUNT];
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
    pub fn preferred_against_mask(
        &self,
        slot: usize,
    ) -> crate::common::system::kind_of::KindOfMask {
        *self
            .preferred_against_masks
            .get(slot)
            .unwrap_or(&crate::common::system::kind_of::KindOfMask::empty())
    }

    /// Define the preferred target mask for a slot.
    pub fn set_preferred_against_mask(
        &mut self,
        slot: usize,
        mask: crate::common::system::kind_of::KindOfMask,
    ) {
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
    preferred_against_masks:
        [Option<crate::common::system::kind_of::KindOfMask>; WEAPON_SLOT_COUNT],
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

    pub fn set_preferred_against_mask(
        &mut self,
        slot: usize,
        mask: Option<crate::common::system::kind_of::KindOfMask>,
    ) {
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

#[derive(Debug, Clone)]
struct IndexedSubblockField {
    repeat_index: usize,
    field: String,
    value: String,
}

fn indexed_subblock_field_order(field: &str) -> usize {
    match field {
        "Conditions" => 0,
        "Weapon" | "Armor" => 1,
        "AutoChooseSources" | "DamageFX" => 2,
        "PreferredAgainst" => 3,
        "ShareWeaponReloadTime" => 4,
        "WeaponLockSharedAcrossSets" => 5,
        _ => usize::MAX,
    }
}

fn collect_indexed_subblocks(
    properties: &HashMap<String, String>,
    prefix: &str,
) -> BTreeMap<usize, Vec<IndexedSubblockField>> {
    let mut blocks: BTreeMap<usize, Vec<IndexedSubblockField>> = BTreeMap::new();

    for (key, value) in properties {
        let Some(rest) = key.strip_prefix(prefix) else {
            continue;
        };
        let digit_len = rest
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .count();
        if digit_len == 0 {
            continue;
        }
        let (index_text, field_text) = rest.split_at(digit_len);
        let Some(field_text) = field_text.strip_prefix('.') else {
            continue;
        };
        let Ok(block_index) = index_text.parse::<usize>() else {
            continue;
        };
        let (field, repeat_index) = if let Some((field, repeat)) = field_text.rsplit_once('#') {
            (field, repeat.parse::<usize>().unwrap_or(0))
        } else {
            (field_text, 0)
        };
        blocks
            .entry(block_index)
            .or_default()
            .push(IndexedSubblockField {
                repeat_index,
                field: field.to_string(),
                value: value.clone(),
            });
    }

    for fields in blocks.values_mut() {
        fields.sort_by_key(|field| {
            (
                field.repeat_index,
                indexed_subblock_field_order(&field.field),
                field.field.clone(),
            )
        });
    }

    blocks
}

fn collect_named_subblock_fields(
    properties: &HashMap<String, String>,
    prefix: &str,
) -> Vec<IndexedSubblockField> {
    let dotted_prefix = format!("{}.", prefix);
    let mut fields = Vec::new();

    for (key, value) in properties {
        let Some(field_text) = key.strip_prefix(&dotted_prefix) else {
            continue;
        };
        let (field, repeat_index) = if let Some((field, repeat)) = field_text.rsplit_once('#') {
            (field, repeat.parse::<usize>().unwrap_or(0))
        } else {
            (field_text, 0)
        };
        fields.push(IndexedSubblockField {
            repeat_index,
            field: field.to_string(),
            value: value.clone(),
        });
    }

    fields.sort_by_key(|field| {
        (
            field.repeat_index,
            indexed_subblock_field_order(&field.field),
            field.field.clone(),
        )
    });
    fields
}

fn parse_weapon_slot(token: &str) -> Result<usize, String> {
    match token.to_ascii_uppercase().as_str() {
        "PRIMARY" => Ok(0),
        "SECONDARY" => Ok(1),
        "TERTIARY" => Ok(2),
        _ => Err(format!("Unknown weapon slot '{}'", token)),
    }
}

fn parse_slot_prefixed_value(value: &str) -> Result<(usize, String), String> {
    let mut parts = value.split_whitespace();
    let slot = parts
        .next()
        .ok_or_else(|| "Missing weapon slot".to_string())
        .and_then(parse_weapon_slot)?;
    let remainder = parts.collect::<Vec<_>>().join(" ");
    Ok((slot, remainder))
}

fn parse_command_source_mask(value: &str) -> Result<u32, String> {
    let mut mask = 0u32;
    for token in split_weapon_condition_tokens(value) {
        match token.as_str() {
            "NONE" => {}
            "FROM_PLAYER" => mask |= 1 << 0,
            "FROM_SCRIPT" => mask |= 1 << 1,
            "FROM_AI" => mask |= 1 << 2,
            "FROM_DOZER" => mask |= 1 << 3,
            "DEFAULT_SWITCH_WEAPON" => mask |= 1 << 4,
            other => return Err(format!("Unknown command source '{}'", other)),
        }
    }
    Ok(mask)
}

fn parse_kindof_mask(value: &str) -> Result<crate::common::system::kind_of::KindOfMask, String> {
    crate::common::system::kind_of::KindOfMask::parse_ini(
        crate::common::system::kind_of::KindOfMask::empty(),
        value,
    )
}

fn apply_flag_tokens(flags: &mut BitFlags, value: &str, field_name: &str) -> Result<(), String> {
    flags.clear();
    for token in split_weapon_condition_tokens(value) {
        if token == "NONE" {
            continue;
        }
        if !flags.set_bit_by_name(&token) {
            return Err(format!("Unknown {} condition '{}'", field_name, token));
        }
    }
    Ok(())
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
    kindof: u128, // KindOfMask.bits() full width so bits >= 64 survive
    default_owning_side: AsciiString,
    command_set_string: AsciiString,
    skill_point_values: [i32; LEVEL_COUNT],
    experience_values: [i32; LEVEL_COUNT],
    experience_required: [i32; LEVEL_COUNT],
    is_trainable: bool,
    enter_guard: bool,
    hijack_guard: bool,

    // Visual properties
    selected_portrait_image: Option<Image>,
    button_image: Option<Image>,

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

    armor_copied_from_default: bool,
    weapons_copied_from_default: bool,
    module_parsing_mode: ModuleParseMode,
    module_being_replaced_name: AsciiString,
    module_being_replaced_tag: AsciiString,
    /// Top-level `Locomotor = SET_NORMAL Foo` entries (C++ AIUpdateModuleData).
    locomotor_sets: HashMap<String, Vec<AsciiString>>,

    #[cfg(feature = "load_test_assets")]
    lta_name: AsciiString,
}

impl ThingTemplate {
    pub fn get_fence_width(&self) -> Real {
        self.fence_width
    }

    pub fn get_fence_x_offset(&self) -> Real {
        self.fence_x_offset
    }

    /// C++ `ThingTemplate::getRawTransportSlotCount` — the parsed
    /// `TransportSlotCount` INI field.
    pub fn get_raw_transport_slot_count(&self) -> UnsignedByte {
        self.transport_slot_count
    }

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
            editor_sorting: EditorSortingType::None,

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
            locomotor_sets: HashMap::new(),

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
    /// C++ ThingTemplate::getSelectedPortraitImage().
    pub fn get_selected_portrait_image(&self) -> Option<&Image> {
        self.selected_portrait_image.as_ref()
    }

    /// C++ ThingTemplate::getButtonImage().
    pub fn get_button_image(&self) -> Option<&Image> {
        self.button_image.as_ref()
    }

    /// C++ `ThingTemplate::getUpgradeCameoName(n)`.
    pub fn get_upgrade_cameo_name(&self, n: usize) -> AsciiString {
        self.upgrade_cameo_upgrade_names
            .get(n)
            .cloned()
            .unwrap_or_default()
    }

    /// Authored `UpgradeCameo1..5` slots in C++ declaration order.
    pub fn upgrade_cameo_names(&self) -> [AsciiString; MAX_UPGRADE_CAMEO_UPGRADES] {
        self.upgrade_cameo_upgrade_names.clone()
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
    /// C++ `ThingTemplate::getShadowSizeX`.
    pub fn get_shadow_size_x(&self) -> Real {
        self.shadow_size_x
    }
    /// C++ `ThingTemplate::getShadowSizeY`.
    pub fn get_shadow_size_y(&self) -> Real {
        self.shadow_size_y
    }
    /// C++ `ThingTemplate::getShadowType`.
    pub fn get_shadow_type(&self) -> ShadowType {
        self.shadow_type
    }
    /// C++ `ThingTemplate::getShadowOffsetX`.
    pub fn get_shadow_offset_x(&self) -> Real {
        self.shadow_offset_x
    }
    /// C++ `ThingTemplate::getShadowOffsetY`.
    pub fn get_shadow_offset_y(&self) -> Real {
        self.shadow_offset_y
    }
    /// C++ `ThingTemplate::getShadowTextureName`.
    pub fn get_shadow_texture_name(&self) -> &AsciiString {
        &self.shadow_texture_name
    }

    pub fn calc_vision_range(&self) -> Real {
        // C++ ThingTemplate.h:405 friend_calcVisionRange — raw field, no geometry fallback.
        self.vision_range
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

    pub fn add_draw_module_info(
        &mut self,
        name: AsciiString,
        module_tag: AsciiString,
        data: Arc<dyn ModuleData>,
        interface_mask: ModuleInterfaceType,
    ) {
        self.draw_module_info.add_module_info(
            name,
            module_tag,
            data,
            interface_mask.0 as i32,
            false,
            false,
        );
    }

    /// Returns descriptors for draw modules defined on this template.
    pub fn draw_module_descriptors(&self) -> Vec<ModuleDescriptor> {
        self.draw_module_info.descriptors()
    }
    pub fn get_client_update_module_info(&self) -> &ModuleInfo {
        &self.client_update_module_info
    }

    pub fn add_client_update_module_info(
        &mut self,
        name: AsciiString,
        module_tag: AsciiString,
        data: Arc<dyn ModuleData>,
        interface_mask: ModuleInterfaceType,
    ) {
        self.client_update_module_info.add_module_info(
            name,
            module_tag,
            data,
            interface_mask.0 as i32,
            false,
            false,
        );
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
    pub fn get_max_simultaneous_of_type(&self) -> u16 {
        self.max_simultaneous_of_type
    }
    pub fn get_max_simultaneous_link_key(&self) -> NameKeyType {
        self.max_simultaneous_link_key
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

    /// C++ `ThingTemplate.h:374-377` `isKindOf(KindOfType t)` / `KindOf.h:158-161`
    /// `TEST_KINDOFMASK`: `t` is a sequential KindOfType bit index, not a mask.
    /// Values in `0..128` are treated as indices (`1 << t`). Larger values cannot be
    /// KindOfType indices and keep historical mask-AND behavior.
    pub fn is_kind_of(&self, kind: impl Into<u128>) -> bool {
        let kind = kind.into();
        if kind < 128 {
            (self.kindof & (1u128 << kind)) != 0
        } else {
            (self.kindof & kind) != 0
        }
    }

    /// Mask intersection. Use this when the argument is `KindOfMask::bits()`, not a
    /// KindOfType index. C++ `isKindOf` never takes a mask; this is the Rust split.
    pub fn is_kind_of_mask(&self, mask: impl Into<u128>) -> bool {
        (self.kindof & mask.into()) != 0
    }

    pub fn is_kind_of_multi(&self, must_be_set: &u64, must_be_clear: &u64) -> bool {
        let must_be_set = *must_be_set as u128;
        let must_be_clear = *must_be_clear as u128;
        (self.kindof & must_be_set) == must_be_set && (self.kindof & must_be_clear) == 0
    }

    pub fn is_any_kind_of(&self, any_kind_of: &u64) -> bool {
        self.is_any_kind_of_bits(*any_kind_of as u128)
    }

    pub fn is_any_kind_of_bits(&self, any_kind_of: u128) -> bool {
        (self.kindof & any_kind_of) != 0
    }

    pub fn get_kindof_mask(&self) -> u64 {
        self.kindof as u64
    }

    /// Full-width KindOf bits (`KindOfMask::bits()`), including positions >= 64.
    pub fn get_kindof_bits(&self) -> u128 {
        self.kindof
    }

    /// C++ ThingTemplate::getPlacementViewAngle().
    pub fn get_placement_view_angle(&self) -> Real {
        self.placement_view_angle
    }

    /// C++ ThingTemplate::getFactoryExitWidth().
    pub fn get_factory_exit_width(&self) -> Real {
        self.factory_exit_width
    }

    /// C++ ThingTemplate::getFactoryExtraBibWidth().
    pub fn get_factory_extra_bib_width(&self) -> Real {
        self.factory_extra_bib_width
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
    /// C++ `ThingTemplate::isBridge()`.
    pub fn is_bridge(&self) -> bool {
        self.is_bridge
    }

    /// C++ ThingTemplate.cpp:384-409 KindOf masks used when clearing default modules.
    fn gps_scrambler_inherit_flags(&self) -> (bool, bool, bool) {
        use crate::common::system::kind_of::KindOfMask;
        let immune_mask = kindof_mask_from(
            KindOfMask::AIRCRAFT
                | KindOfMask::SHRUBBERY
                | KindOfMask::OPTIMIZED_TREE
                | KindOfMask::STRUCTURE
                | KindOfMask::DRAWABLE_ONLY
                | KindOfMask::MOB_NEXUS
                | KindOfMask::IGNORED_IN_GUI
                | KindOfMask::CLEARED_BY_BUILD
                | KindOfMask::DEFENSIVE_WALL
                | KindOfMask::BALLISTIC_MISSILE
                | KindOfMask::SUPPLY_SOURCE
                | KindOfMask::BOAT
                | KindOfMask::INERT
                | KindOfMask::BRIDGE
                | KindOfMask::LANDMARK_BRIDGE
                | KindOfMask::BRIDGE_TOWER,
        );
        let candidate_mask = kindof_mask_from(
            KindOfMask::SCORE
                | KindOfMask::VEHICLE
                | KindOfMask::INFANTRY
                | KindOfMask::PORTABLE_STRUCTURE,
        );
        (
            self.is_trainable(),
            self.is_any_kind_of_bits(immune_mask),
            self.is_any_kind_of_bits(candidate_mask),
        )
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

    pub fn get_voice_move(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceMove)
    }

    pub fn get_voice_enter(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceEnter)
    }

    pub fn get_voice_garrison(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceGarrison)
    }

    pub fn get_voice_guard(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceGuard)
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

    pub fn get_voice_created(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceCreated)
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

    /// C++ `ThingTemplate::getVoiceFear` / `TTAUDIO_voiceFear`.
    pub fn get_voice_fear(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceFear)
    }

    /// C++ `ThingTemplate::getVoiceDefect` / `TTAUDIO_voiceDefect`.
    pub fn get_voice_defect(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::VoiceDefect)
    }

    /// C++ `ThingTemplate::getSoundOnDamaged` / `TTAUDIO_soundOnDamaged`.
    pub fn get_sound_on_damaged(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::SoundOnDamaged)
    }

    /// C++ `ThingTemplate::getSoundOnReallyDamaged` / `TTAUDIO_soundOnReallyDamaged`.
    pub fn get_sound_on_really_damaged(&self) -> Option<&AudioEventRts> {
        self.audioarray
            .get(ThingTemplateAudioType::SoundOnReallyDamaged)
    }

    /// C++ `ThingTemplate::getSoundEnter` / `TTAUDIO_soundEnter`.
    pub fn get_sound_enter(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::SoundEnter)
    }

    /// C++ `ThingTemplate::getSoundExit` / `TTAUDIO_soundExit`.
    pub fn get_sound_exit(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::SoundExit)
    }

    /// C++ `ThingTemplate::getSoundFalling` / `TTAUDIO_soundFalling`.
    pub fn get_sound_falling(&self) -> Option<&AudioEventRts> {
        self.audioarray.get(ThingTemplateAudioType::SoundFalling)
    }

    pub fn audio_event(&self, audio_type: ThingTemplateAudioType) -> Option<&AudioEventRts> {
        self.audioarray.get(audio_type)
    }

    pub fn get_per_unit_sound(&self, sound_name: &AsciiString) -> Option<&AudioEventRts> {
        self.per_unit_sounds.get(sound_name)
    }

    pub fn get_per_unit_fx(
        &self,
        fx_name: &AsciiString,
    ) -> Option<&Arc<crate::common::ini::ini_fx_list::FXList>> {
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

    fn load_weapon_sets_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> Result<(), String> {
        let blocks = collect_indexed_subblocks(properties, "WeaponSet");
        if blocks.is_empty() {
            return Ok(());
        }

        self.clear_weapon_template_sets();
        for fields in blocks.values() {
            let mut set = WeaponTemplateSet::new();
            for field in fields {
                match field.field.as_str() {
                    "Conditions" => {
                        apply_flag_tokens(set.types_mut(), &field.value, "weapon set")?;
                    }
                    "Weapon" => {
                        let (slot, weapon_name) = parse_slot_prefixed_value(&field.value)?;
                        let weapon_name = weapon_name.trim();
                        let weapon_name =
                            if weapon_name.is_empty() || weapon_name.eq_ignore_ascii_case("None") {
                                None
                            } else {
                                Some(AsciiString::from(weapon_name))
                            };
                        set.set_weapon_template_name(slot, weapon_name);
                    }
                    "AutoChooseSources" => {
                        let (slot, source_names) = parse_slot_prefixed_value(&field.value)?;
                        set.set_auto_choose_mask(slot, parse_command_source_mask(&source_names)?);
                    }
                    "PreferredAgainst" => {
                        let (slot, kindof_names) = parse_slot_prefixed_value(&field.value)?;
                        set.set_preferred_against_mask(slot, parse_kindof_mask(&kindof_names)?);
                    }
                    "ShareWeaponReloadTime" => {
                        set.set_reload_time_shared(parse_bool_field(&field.value)?);
                    }
                    "WeaponLockSharedAcrossSets" => {
                        set.set_weapon_lock_shared_across_sets(parse_bool_field(&field.value)?);
                    }
                    other => return Err(format!("Unknown WeaponSet field '{}'", other)),
                }
            }
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

    fn load_armor_sets_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> Result<(), String> {
        let blocks = collect_indexed_subblocks(properties, "ArmorSet");
        if blocks.is_empty() {
            return Ok(());
        }

        self.clear_armor_template_sets();
        for fields in blocks.values() {
            let mut set = ArmorTemplateSet::new();
            for field in fields {
                match field.field.as_str() {
                    "Conditions" => {
                        apply_flag_tokens(set.types_mut(), &field.value, "armor set")?;
                    }
                    "Armor" => {
                        let value = field.value.trim();
                        let name = if value.is_empty() || value.eq_ignore_ascii_case("None") {
                            None
                        } else {
                            Some(AsciiString::from(value))
                        };
                        set.set_armor_template_name(name);
                    }
                    "DamageFX" => {
                        let value = field.value.trim();
                        let name = if value.is_empty() || value.eq_ignore_ascii_case("None") {
                            None
                        } else {
                            Some(AsciiString::from(value))
                        };
                        set.set_damage_fx_name(name);
                    }
                    other => return Err(format!("Unknown ArmorSet field '{}'", other)),
                }
            }
            self.add_armor_template_set(set);
        }

        Ok(())
    }

    fn load_per_unit_sounds_from_properties(&mut self, properties: &HashMap<String, String>) {
        let fields = collect_named_subblock_fields(properties, "UnitSpecificSounds");
        if fields.is_empty() {
            return;
        }

        self.per_unit_sounds.clear();
        for field in fields {
            let name = AsciiString::from(field.field.as_str());
            if self.per_unit_sounds.contains_key(&name) {
                continue;
            }
            self.per_unit_sounds
                .insert(name, AudioEventRts::with_event_name(field.value.trim()));
        }
    }

    fn load_per_unit_fx_from_properties(&mut self, properties: &HashMap<String, String>) {
        let fields = collect_named_subblock_fields(properties, "UnitSpecificFX");
        if fields.is_empty() {
            return;
        }

        self.per_unit_fx.clear();
        for field in fields {
            let name = AsciiString::from(field.field.as_str());
            if self.per_unit_fx.contains_key(&name) {
                continue;
            }
            let fx_name = field.value.trim();
            let fx_list = if fx_name.eq_ignore_ascii_case("None") || fx_name.is_empty() {
                None
            } else {
                crate::common::ini::ini_fx_list::get_fx_list_store()
                    .find_fx_list(fx_name)
                    .cloned()
                    .map(Arc::new)
            };
            self.per_unit_fx.insert(name, fx_list);
        }
    }

    fn load_prerequisites_from_properties(&mut self, properties: &HashMap<String, String>) {
        let fields = collect_named_subblock_fields(properties, "Prerequisites");
        if fields.is_empty() {
            return;
        }

        let lines = fields
            .into_iter()
            .map(|field| format!("{} = {}", field.field, field.value.trim()))
            .collect::<Vec<_>>();
        self.parse_prerequisites_block(&lines);
    }

    fn add_module_from_property(
        &mut self,
        field_name: &str,
        property_key: &str,
        value: &str,
        properties: &HashMap<String, String>,
    ) -> Result<(), String> {
        let mut tokens = value.split_whitespace();
        let module_name = tokens
            .next()
            .ok_or_else(|| format!("Missing module name for '{}'", field_name))?;
        let module_tag = tokens
            .next()
            .ok_or_else(|| format!("Missing module tag for '{}'", module_name))?;

        let (module_type, fallback_mask) = match field_name {
            "Behavior" => (ModuleType::Behavior, ModuleInterfaceType::UPDATE),
            "Body" => (ModuleType::Behavior, ModuleInterfaceType::BODY),
            "Draw" => (ModuleType::Draw, ModuleInterfaceType::DRAW),
            "ClientUpdate" => (ModuleType::ClientUpdate, ModuleInterfaceType::CLIENT_UPDATE),
            other => return Err(format!("Unknown module field '{}'", other)),
        };

        let interface_mask = lookup_module_interface_mask(module_name, module_type, fallback_mask);

        if self.module_parsing_mode != ModuleParseMode::AddRemoveReplace {
            let new_name = AsciiString::from(module_name);
            let mask = interface_mask.0 as i32;
            let (is_trainable, disallowed, candidate) = self.gps_scrambler_inherit_flags();
            self.behavior_module_info.clear_copied_from_default_entries(
                mask,
                &new_name,
                is_trainable,
                disallowed,
                candidate,
            );
            self.draw_module_info.clear_copied_from_default_entries(
                mask,
                &new_name,
                is_trainable,
                disallowed,
                candidate,
            );
            self.client_update_module_info
                .clear_copied_from_default_entries(
                    mask,
                    &new_name,
                    is_trainable,
                    disallowed,
                    candidate,
                );
        }

        if self.module_parsing_mode == ModuleParseMode::AddRemoveReplace
            && !self.module_being_replaced_name.is_empty()
            && self.module_being_replaced_name.as_str() != module_name
        {
            return Err(format!(
                "ReplaceModule must replace modules with another module of the same type, but you are attempting to replace a {} with a {}",
                self.module_being_replaced_name, module_name
            ));
        }
        if self.module_parsing_mode == ModuleParseMode::AddRemoveReplace
            && !self.module_being_replaced_tag.is_empty()
            && self.module_being_replaced_tag.as_str() == module_tag
        {
            return Err(format!(
                "ReplaceModule must specify a new, unique tag for the replaced module, but you are not doing so for {} ({})",
                module_tag, self.module_being_replaced_name
            ));
        }

        let (raw_body, fields) = collect_module_body(properties, property_key);
        let data = module_data_from_body(module_name, module_type, module_tag, &raw_body, fields);
        let target_info = match module_type {
            ModuleType::Behavior => &mut self.behavior_module_info,
            ModuleType::Draw => &mut self.draw_module_info,
            ModuleType::ClientUpdate => &mut self.client_update_module_info,
        };
        if data.is_ai_module_data() {
            target_info.clear_ai_module_info();
        }
        let inheritable = self.module_parsing_mode == ModuleParseMode::Inheritable;
        let overrideable = self.module_parsing_mode == ModuleParseMode::OverrideableByLikeKind;
        target_info.add_module_info(
            AsciiString::from(module_name),
            AsciiString::from(module_tag),
            data,
            interface_mask.0 as i32,
            inheritable,
            overrideable,
        );

        Ok(())
    }

    fn load_modules_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> Result<(), String> {
        let mut fields = properties
            .iter()
            .filter_map(|(key, value)| {
                if key.contains('.') {
                    return None;
                }
                let base_key = property_base_key(key);
                is_module_object_field(base_key).then_some((
                    module_field_order(base_key),
                    property_repeat_index(key),
                    base_key,
                    key.as_str(),
                    value.as_str(),
                ))
            })
            .collect::<Vec<_>>();

        fields.sort_by_key(|(field_order, repeat_index, field_name, _, _)| {
            (*field_order, *repeat_index, (*field_name).to_string())
        });

        for (_, _, field_name, property_key, value) in fields {
            self.add_module_from_property(field_name, property_key, value.trim(), properties)?;
        }

        self.load_module_overrides(properties)?;
        Ok(())
    }

    fn collect_repeatable_keys(properties: &HashMap<String, String>, field: &str) -> Vec<String> {
        let mut keys = Vec::new();
        if properties.contains_key(field) {
            keys.push(field.to_string());
        }
        for index in 1.. {
            let repeated = format!("{}#{}", field, index);
            if properties.contains_key(&repeated) {
                keys.push(repeated);
            } else {
                break;
            }
        }
        keys
    }

    fn remove_module_info(&mut self, tag: &AsciiString) -> Option<AsciiString> {
        if let Some(name) = self.behavior_module_info.clear_module_data_with_tag(tag) {
            return Some(name);
        }
        if let Some(name) = self.draw_module_info.clear_module_data_with_tag(tag) {
            return Some(name);
        }
        self.client_update_module_info
            .clear_module_data_with_tag(tag)
    }

    fn load_modules_from_override_prefix(
        &mut self,
        properties: &HashMap<String, String>,
        prefix: &str,
    ) -> Result<(), String> {
        let mut nested = HashMap::new();
        let dotted = format!("{}.", prefix);
        for (key, value) in properties {
            if let Some(rest) = key.strip_prefix(&dotted) {
                nested.insert(rest.to_string(), value.clone());
            }
        }
        if let Some(body) = properties.get(&format!("{}.__body", prefix)) {
            let lines: Vec<&str> = body.lines().collect();
            let from_body = parse_override_module_body(&lines);
            for (key, value) in from_body {
                nested.entry(key).or_insert(value);
            }
        }
        self.load_modules_from_properties_without_overrides(&nested)
    }

    fn load_modules_from_properties_without_overrides(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> Result<(), String> {
        let mut fields = properties
            .iter()
            .filter_map(|(key, value)| {
                if key.contains('.') {
                    return None;
                }
                let base_key = property_base_key(key);
                is_module_object_field(base_key).then_some((
                    module_field_order(base_key),
                    property_repeat_index(key),
                    base_key,
                    key.as_str(),
                    value.as_str(),
                ))
            })
            .collect::<Vec<_>>();
        fields.sort_by_key(|(field_order, repeat_index, field_name, _, _)| {
            (*field_order, *repeat_index, (*field_name).to_string())
        });
        for (_, _, field_name, property_key, value) in fields {
            self.add_module_from_property(field_name, property_key, value.trim(), properties)?;
        }
        Ok(())
    }

    fn load_module_overrides(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> Result<(), String> {
        for key in Self::collect_repeatable_keys(properties, "RemoveModule") {
            if let Some(tag) = properties.get(&key) {
                let tag = AsciiString::from(tag.trim());
                if !tag.is_empty() {
                    let _ = self.remove_module_info(&tag);
                }
            }
        }

        for key in Self::collect_repeatable_keys(properties, "ReplaceModule") {
            let tag = properties
                .get(&key)
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !tag.is_empty() {
                let tag_ascii = AsciiString::from(tag.as_str());
                let removed = self.remove_module_info(&tag_ascii);
                self.module_being_replaced_name = removed.unwrap_or_default();
                self.module_being_replaced_tag = tag_ascii;
            }
            self.module_parsing_mode = ModuleParseMode::AddRemoveReplace;
            self.load_modules_from_override_prefix(properties, &key)?;
            self.module_being_replaced_name.clear();
            self.module_being_replaced_tag.clear();
            self.module_parsing_mode = ModuleParseMode::Normal;
        }

        for key in Self::collect_repeatable_keys(properties, "AddModule") {
            self.module_parsing_mode = ModuleParseMode::AddRemoveReplace;
            self.load_modules_from_override_prefix(properties, &key)?;
            self.module_parsing_mode = ModuleParseMode::Normal;
        }

        for key in Self::collect_repeatable_keys(properties, "InheritableModule") {
            self.module_parsing_mode = ModuleParseMode::Inheritable;
            self.load_modules_from_override_prefix(properties, &key)?;
            self.module_parsing_mode = ModuleParseMode::Normal;
        }

        for key in Self::collect_repeatable_keys(properties, "OverrideableByLikeKind") {
            self.module_parsing_mode = ModuleParseMode::OverrideableByLikeKind;
            self.load_modules_from_override_prefix(properties, &key)?;
            self.module_parsing_mode = ModuleParseMode::Normal;
        }

        Ok(())
    }

    /// C++ `AIUpdateModuleData::parseLocomotorSet` via ThingTemplate Locomotor field.
    pub fn parse_locomotor_field(&mut self, value: &str) -> Result<(), String> {
        let mut tokens = value.split_whitespace();
        let set_name = tokens
            .next()
            .ok_or_else(|| "Locomotor field missing set name".to_string())?;
        let names: Vec<AsciiString> = tokens
            .filter(|token| !token.is_empty() && !token.eq_ignore_ascii_case("None"))
            .map(AsciiString::from)
            .collect();
        if let Some(existing) = self.locomotor_sets.get(set_name) {
            if !existing.is_empty()
                && !crate::common::thing::thing_template_locomotor::locomotor_overrides_allowed()
            {
                return Err("re-specifying a LocomotorSet is no longer allowed".to_string());
            }
        }
        self.locomotor_sets
            .insert(set_name.to_string(), names.clone());
        self.write_locomotor_set_into_ai_module(set_name, &names)
    }

    /// C++ `ThingTemplate::friend_getAIModuleInfo`.
    pub fn friend_get_ai_module_info(&self) -> Option<&Arc<dyn ModuleData>> {
        for i in 0..self.behavior_module_info.get_count() {
            if let Some(data) = self.behavior_module_info.get_nth_data(i) {
                if data.is_ai_module_data() {
                    return Some(data);
                }
            }
        }
        None
    }

    fn write_locomotor_set_into_ai_module(
        &mut self,
        set_name: &str,
        names: &[AsciiString],
    ) -> Result<(), String> {
        let Some(data) = self.friend_get_ai_module_info().cloned() else {
            return Err(format!(
                "Attempted to specify a locomotor for object {} without an AIUpdate block.",
                self.name_string
            ));
        };
        let updated =
            crate::common::thing::thing_template_locomotor::apply_locomotor_set_to_module_data(
                data, set_name, names,
            )?;
        self.behavior_module_info
            .replace_first_ai_module_data(updated);
        Ok(())
    }

    /// Replay stored top-level Locomotor lines into AI module data (late hook install).
    pub fn apply_stored_locomotors_to_ai_module(&mut self) -> Result<(), String> {
        if self.locomotor_sets.is_empty() {
            return Ok(());
        }
        crate::common::thing::thing_template_locomotor::set_locomotor_overrides_allowed(true);
        let sets: Vec<(String, Vec<AsciiString>)> = self
            .locomotor_sets
            .iter()
            .map(|(set, names)| (set.clone(), names.clone()))
            .collect();
        let result = (|| {
            for (set_name, names) in sets {
                self.write_locomotor_set_into_ai_module(&set_name, &names)?;
            }
            Ok(())
        })();
        crate::common::thing::thing_template_locomotor::set_locomotor_overrides_allowed(false);
        result
    }

    pub fn locomotor_sets(&self) -> &HashMap<String, Vec<AsciiString>> {
        &self.locomotor_sets
    }

    pub fn locomotor_set_names(&self, set: &str) -> Option<&[AsciiString]> {
        self.locomotor_sets.get(set).map(Vec::as_slice)
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

    pub fn get_reskinned_from(&self) -> Option<&Arc<ThingTemplate>> {
        self.reskinned_from.as_ref()
    }

    /// C++ ThingTemplate::isEquivalentTo (ThingTemplate.cpp:1454).
    pub fn is_equivalent_to(&self, other: &ThingTemplate) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }

        if Self::same_final_override(self, other) {
            return true;
        }

        if let Some(from) = self.get_reskinned_from() {
            if std::ptr::eq(from.as_ref(), other) {
                return true;
            }
        }
        if let Some(from) = other.get_reskinned_from() {
            if std::ptr::eq(from.as_ref(), self) {
                return true;
            }
        }
        if let (Some(a), Some(b)) = (self.get_reskinned_from(), other.get_reskinned_from()) {
            if Arc::ptr_eq(a, b) {
                return true;
            }
        }

        let other_name = other.get_name();
        if self
            .get_build_variations()
            .iter()
            .any(|variation| variation.eq_ignore_ascii_case(other_name))
        {
            return true;
        }
        let self_name = self.get_name();
        if other
            .get_build_variations()
            .iter()
            .any(|variation| variation.eq_ignore_ascii_case(self_name))
        {
            return true;
        }
        false
    }

    fn same_final_override(a: &ThingTemplate, b: &ThingTemplate) -> bool {
        match (Self::final_override_arc(a), Self::final_override_arc(b)) {
            (None, None) => false,
            (Some(fa), Some(fb)) => Arc::ptr_eq(&fa, &fb),
            (Some(fa), None) => std::ptr::eq(fa.as_ref(), b),
            (None, Some(fb)) => std::ptr::eq(fb.as_ref(), a),
        }
    }

    fn final_override_arc(template: &ThingTemplate) -> Option<Arc<ThingTemplate>> {
        let mut current = template.get_next_override()?;
        loop {
            match current.get_next_override() {
                Some(next) => current = next,
                None => return Some(current),
            }
        }
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
        if self.is_kind_of_mask(crate::common::system::kind_of::KindOfMask::COMMANDCENTER.bits()) {
            self.is_build_facility = true;
        }

        // C++ ThingTemplate::resolveNames: findImageByName THEN clear the name.
        if !self.selected_portrait_image_name.is_empty() {
            if let Some(collection) = get_mapped_image_collection() {
                if let Some(image) = collection
                    .read()
                    .find_image_by_name(self.selected_portrait_image_name.as_str())
                {
                    self.selected_portrait_image = Some(image.clone());
                }
            }
            self.selected_portrait_image_name.clear();
        }

        if !self.button_image_name.is_empty() {
            if let Some(collection) = get_mapped_image_collection() {
                if let Some(image) = collection
                    .read()
                    .find_image_by_name(self.button_image_name.as_str())
                {
                    self.button_image = Some(image.clone());
                }
            }
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

    /// Calculate the cost to build this template for a given player.
    ///
    /// C++ Reference: ThingTemplate::calcCostToBuild (ThingTemplate.cpp lines 1508-1517)
    pub fn calc_cost_to_build(
        &self,
        player: Option<&dyn crate::common::thing::module::Thing>,
    ) -> i32 {
        let Some(player) = player else {
            return 0;
        };

        let base_cost = self.get_build_cost() as f32;
        let mut faction_modifier =
            1.0 + player.get_production_cost_change_percent(self.get_name().as_str());
        faction_modifier *= player.get_production_cost_change_based_on_kind_of(self.kindof as u64);
        let result = base_cost * faction_modifier * player.get_build_cost_handicap(self);
        result as i32
    }

    /// Calculate the time (in logic frames) to build this template for a given player.
    ///
    /// C++ Reference: ThingTemplate::calcTimeToBuild (ThingTemplate.cpp lines 1524-1576)
    ///
    pub fn calc_time_to_build(
        &self,
        player: Option<&dyn crate::common::thing::module::Thing>,
    ) -> i32 {
        let mut build_time = (self.get_build_time()
            * crate::common::game_common::LOGICFRAMES_PER_SECOND as f32)
            as i32;

        let Some(player) = player else {
            return build_time;
        };

        build_time = ((build_time as f32) * player.get_build_time_handicap(self)) as i32;

        let faction_modifier =
            1.0 + player.get_production_time_change_percent(self.get_name().as_str());
        build_time = ((build_time as f32) * faction_modifier) as i32;

        if player.builds_instantly_for_debug() {
            build_time = 1;
        }

        let globals = global_data::read();
        let energy_percent = player.get_energy_supply_ratio().min(1.0);
        let energy_short = (1.0 - energy_percent) * globals.low_energy_penalty_modifier;
        let mut penalty_rate = 1.0 - energy_short;
        penalty_rate = penalty_rate.max(globals.min_low_energy_production_speed);
        if energy_percent < 1.0 {
            penalty_rate = penalty_rate.min(globals.max_low_energy_production_speed);
        }
        let penalty_rate = if penalty_rate <= 0.0 {
            0.01
        } else {
            penalty_rate
        };
        build_time = ((build_time as f32) / penalty_rate) as i32;

        if self.get_build_completion() == BuildCompletionType::AppearsAtRallyPoint {
            let count = player.count_equivalent_build_facilities(self);
            let factory_multiplier = globals.multiple_factory;
            if factory_multiplier > 0.0 {
                for _ in 0..count.saturating_sub(1) {
                    build_time = ((build_time as f32) * factory_multiplier) as i32;
                }
            }
        }

        build_time
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
    /// corresponding struct member.  Unknown fields return an error, matching
    /// C++ `INI::initFromINI`, which throws `INI_UNKNOWN_TOKEN` for unmatched
    /// fields.
    ///
    /// WeaponSet and ArmorSet sub-blocks are handled by their own dedicated
    /// parsers (see `load_weapon_sets_from_definitions` and
    /// `parse_armor_set_from_properties`) and are NOT processed here.
    pub fn parse_object_fields_from_ini(
        &mut self,
        properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        self.load_weapon_sets_from_properties(properties)?;
        self.load_armor_sets_from_properties(properties)?;
        self.load_per_unit_sounds_from_properties(properties);
        self.load_per_unit_fx_from_properties(properties);
        self.load_prerequisites_from_properties(properties);
        self.load_modules_from_properties(properties)?;

        for (key, value) in properties {
            let base_key = property_base_key(key);
            let trimmed = value.trim();
            if let Some(audio_type) = object_audio_field_type(base_key) {
                self.audioarray
                    .set(audio_type, AudioEventRts::with_event_name(trimmed));
                continue;
            }
            if key.starts_with("UnitSpecificSounds.")
                || key.starts_with("UnitSpecificFX.")
                || key.starts_with("Prerequisites.")
                || is_module_body_property(key)
            {
                continue;
            }
            if is_module_object_field(base_key) {
                continue;
            }

            match base_key {
                // --- Display ---
                "DisplayName" => {
                    // C++ INI::parseAndTranslateLabel
                    let translated =
                        INI::translate_label(trimmed).unwrap_or_else(|_| trimmed.to_string());
                    self.display_name = UnicodeString::from(translated.as_str());
                }
                "DisplayColor" => {
                    if let Ok(c) = parse_color_int(trimmed) {
                        self.display_color = c;
                    }
                }
                "EditorSorting" => {
                    self.editor_sorting = parse_editor_sorting(trimmed)?;
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
                    if let Ok(v) = INI::parse_angle_real(trimmed) {
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
                    self.buildable = parse_buildable_status(trimmed)?;
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
                    self.build_completion = parse_build_completion(trimmed)?;
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
                    // C++ BitFlagsIO.h:38-107 — NONE, +NAME, -NAME, unknown errors.
                    let existing = KindOfMask::from_bits_retain(self.kindof);
                    self.kindof = KindOfMask::parse_ini(existing, trimmed)?.bits();
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
                    self.shadow_type = parse_shadow_type(trimmed)?;
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
                    // C++ uses INI::parseDurationUnsignedInt: object INI values are
                    // millisecond durations (optionally suffixed with `ms` / `s`),
                    // while the stored value is in 30 Hz logic frames.  Treating the
                    // raw token as a frame count makes retail values 30 times too
                    // large and keeps occluders alive far beyond their C++ lifetime.
                    if let Ok(v) =
                        crate::common::ini::ini::INI::parse_duration_unsigned_int(trimmed)
                    {
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
                        self.max_simultaneous_determined_by_superweapon_restriction = false;
                    }
                }
                "MaxSimultaneousLinkKey" => {
                    self.max_simultaneous_link_key = if trimmed.is_empty() {
                        0
                    } else {
                        NameKeyGenerator::name_to_key(trimmed)
                    };
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
                    self.geometry_info.geometry_type = parse_geometry_type(trimmed)?;
                    self.geometry_info.calc_bounding_stuff();
                }

                "GeometryMajorRadius" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.geometry_info.set_major_radius(v);
                    }
                }
                "GeometryMinorRadius" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.geometry_info.set_minor_radius(v);
                    }
                }
                "GeometryHeight" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.geometry_info.height = v;
                        self.geometry_info.calc_bounding_stuff();
                    }
                }
                "GeometryIsSmall" => {
                    if let Ok(v) = parse_bool_simple(trimmed) {
                        self.geometry_info.is_small = v;
                    }
                }
                "Locomotor" => {
                    self.parse_locomotor_field(trimmed)?;
                }

                // --- WeaponSet / ArmorSet are handled separately ---
                "WeaponSet" | "ArmorSet" | "Prerequisites" => {
                    // Sub-block fields parsed by dedicated methods
                }

                // Valid C++ fields not yet wired to Rust state are accepted
                // here so they are not mistaken for unknown tokens.
                _ if is_cpp_object_field(key) => {}
                _ => {
                    return Err(format!("Unknown object field '{}'", key));
                }
            }
        }

        Ok(())
    }

    /// C++ `getReskinFieldParse()`: Draw, Geometry*, Fence*, MaxSimultaneous*.
    pub fn parse_reskin_fields_from_ini(
        &mut self,
        properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), String> {
        let filtered: HashMap<String, String> = properties
            .iter()
            .filter(|(key, _)| is_reskin_property(key))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        self.load_modules_from_properties_without_overrides(&filtered)?;

        for (key, value) in &filtered {
            let base_key = property_base_key(key);
            let trimmed = value.trim();
            if is_module_object_field(base_key) || is_module_body_property(key) {
                continue;
            }
            match base_key {
                "Geometry" => {
                    self.geometry_info.geometry_type = parse_geometry_type(trimmed)?;
                    self.geometry_info.calc_bounding_stuff();
                }
                "GeometryMajorRadius" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.geometry_info.set_major_radius(v);
                    }
                }
                "GeometryMinorRadius" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.geometry_info.set_minor_radius(v);
                    }
                }
                "GeometryHeight" => {
                    if let Ok(v) = trimmed.parse::<Real>() {
                        self.geometry_info.height = v;
                        self.geometry_info.calc_bounding_stuff();
                    }
                }
                "GeometryIsSmall" => {
                    if let Ok(v) = parse_bool_simple(trimmed) {
                        self.geometry_info.is_small = v;
                    }
                }
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
                "MaxSimultaneousOfType" => {
                    if trimmed.eq_ignore_ascii_case("DeterminedBySuperweaponRestriction") {
                        self.max_simultaneous_determined_by_superweapon_restriction = true;
                        self.max_simultaneous_of_type = 0;
                    } else if let Ok(v) = trimmed.parse::<UnsignedShort>() {
                        self.max_simultaneous_of_type = v;
                        self.max_simultaneous_determined_by_superweapon_restriction = false;
                    }
                }
                "MaxSimultaneousLinkKey" => {
                    self.max_simultaneous_link_key = if trimmed.is_empty() {
                        0
                    } else {
                        NameKeyGenerator::name_to_key(trimmed)
                    };
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Set the KindOf mask from a resolved bitmask (`u64` or full `u128`).
    ///
    /// Called by the GameLogic layer after resolving KindOf flag names to bits.
    pub fn set_kindof_mask(&mut self, mask: impl Into<u128>) {
        self.kindof = mask.into();
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
    crate::common::thing::thing_template_color::parse_color_int(s)
}

fn parse_editor_sorting(s: &str) -> Result<EditorSortingType, String> {
    match s.trim() {
        "NONE" | "None" | "Invalid" => Ok(EditorSortingType::None),
        "STRUCTURE" | "Structure" | "Building" => Ok(EditorSortingType::Structure),
        "INFANTRY" | "Infantry" | "Unit" => Ok(EditorSortingType::Infantry),
        "VEHICLE" | "Vehicle" => Ok(EditorSortingType::Vehicle),
        "SHRUBBERY" | "Shrubbery" => Ok(EditorSortingType::Shrubbery),
        "MISC_MAN_MADE" | "MiscManMade" | "Infrastructure" => Ok(EditorSortingType::MiscManMade),
        "MISC_NATURAL" | "MiscNatural" | "Civilian" => Ok(EditorSortingType::MiscNatural),
        "DEBRIS" | "Debris" => Ok(EditorSortingType::Debris),
        "SYSTEM" | "System" => Ok(EditorSortingType::System),
        "AUDIO" | "Audio" => Ok(EditorSortingType::Audio),
        "TEST" | "Test" => Ok(EditorSortingType::Test),
        "FOR_REVIEW" | "ForReview" => Ok(EditorSortingType::ForReview),
        "ROAD" | "Road" => Ok(EditorSortingType::Road),
        "WAYPOINT" | "Waypoint" => Ok(EditorSortingType::Waypoint),
        _ => Err(format!("Unknown EditorSorting token '{}'", s)),
    }
}

fn parse_radar_priority(s: &str) -> RadarPriorityType {
    // `RadarPriorityNames` in C++ uses symbolic gameplay categories, not a
    // low-to-critical scale.  This is present on most retail object templates
    // and determines whether/how an object appears on the minimap.
    match s.trim().to_ascii_uppercase().as_str() {
        "NOT_ON_RADAR" => RadarPriorityType::NotOnRadar,
        "STRUCTURE" => RadarPriorityType::Structure,
        "UNIT" => RadarPriorityType::Unit,
        "LOCAL_UNIT_ONLY" => RadarPriorityType::LocalUnitOnly,
        _ => RadarPriorityType::Invalid,
    }
}

fn lookup_module_interface_mask(
    module_name: &str,
    module_type: ModuleType,
    fallback_mask: ModuleInterfaceType,
) -> ModuleInterfaceType {
    if let Ok(factory_guard) = get_module_factory() {
        if let Some(factory) = factory_guard.as_ref() {
            let mask = factory.find_module_interface_mask(module_name, module_type);
            if mask != ModuleInterfaceType::NONE {
                return mask;
            }
        }
    }

    let mask = ModuleFactory::new().find_module_interface_mask(module_name, module_type);
    if mask != ModuleInterfaceType::NONE {
        mask
    } else {
        fallback_mask
    }
}

fn parse_buildable_status(s: &str) -> Result<BuildableStatus, String> {
    match s.trim().to_ascii_uppercase().as_str() {
        "YES" => Ok(BuildableStatus::Yes),
        "IGNORE_PREREQUISITES" | "IGNOREPREREQUISITES" => Ok(BuildableStatus::IgnorePrerequisites),
        "NO" => Ok(BuildableStatus::No),
        "ONLY_BY_AI" | "ONLYBYAI" => Ok(BuildableStatus::OnlyByAi),
        _ => Err(format!("Unknown Buildable token '{}'", s)),
    }
}

fn parse_build_completion(s: &str) -> Result<BuildCompletionType, String> {
    match s.trim().to_ascii_uppercase().as_str() {
        "PLACED_BY_PLAYER" | "PLACEDBYPLAYER" => Ok(BuildCompletionType::PlacedByPlayer),
        "APPEARS_AT_RALLY_POINT" | "APPEARSATRALLYPOINT" => {
            Ok(BuildCompletionType::AppearsAtRallyPoint)
        }
        _ => Err(format!("Unknown BuildCompletion token '{}'", s)),
    }
}

const SHADOW_NAMES: &[&str] = &[
    "SHADOW_DECAL",
    "SHADOW_VOLUME",
    "SHADOW_PROJECTION",
    "SHADOW_DYNAMIC_PROJECTION",
    "SHADOW_DIRECTIONAL_PROJECTION",
    "SHADOW_ALPHA_DECAL",
    "SHADOW_ADDITIVE_DECAL",
];

fn parse_shadow_type(s: &str) -> Result<ShadowType, String> {
    let mapped: Vec<String> = s
        .split_whitespace()
        .map(|token| match token.to_ascii_uppercase().as_str() {
            "VOLUME" => "SHADOW_VOLUME".to_string(),
            "DECAL" => "SHADOW_DECAL".to_string(),
            other => other.to_string(),
        })
        .collect();
    let tokens: Vec<&str> = mapped.iter().map(String::as_str).collect();
    let bits = INI::parse_bit_string_32(&tokens, SHADOW_NAMES)
        .map_err(|_| format!("Unknown Shadow token '{}'", s))?;
    Ok(ShadowType(bits as u8))
}

fn parse_geometry_type(s: &str) -> Result<GeometryType, String> {
    match s.trim() {
        "SPHERE" | "Sphere" => Ok(GeometryType::Sphere),
        "CYLINDER" | "Cylinder" => Ok(GeometryType::Cylinder),
        "BOX" | "Box" => Ok(GeometryType::Box),
        _ => Err(format!("Unknown Geometry token '{}'", s)),
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

impl Overridable for ThingTemplate {
    fn is_override(&self) -> bool {
        ThingTemplate::is_override(self)
    }

    fn delete_overrides(&self) {
        ThingTemplate::delete_overrides(self)
    }
}
