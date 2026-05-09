//! Special Power System - Full implementation
//!
//! Manages special powers/superweapons in the game, including:
//! - Power activation and cooldowns
//! - Shared/synced powers across command centers
//! - Targeting and execution
//! - Science prerequisites
//!
//! Reference: /GeneralsMD/Code/GameEngine/Source/Common/RTS/SpecialPower.cpp
//! Reference: /GeneralsMD/Code/GameEngine/Include/Common/SpecialPower.h

use std::collections::HashMap;
use std::str::FromStr;

/// Default defection detection protection time limit (10 seconds at 30 FPS)
/// Reference: C++ DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT
pub const DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT: u32 = 30 * 10;

/// Invalid science type constant
pub const SCIENCE_INVALID: i32 = -1;

/// Academy classification type for tracking player behavior
/// Reference: C++ AcademyClassificationType enum (AcademyStats.h)
/// C++ defines: ACT_NONE=0, ACT_UPGRADE_RADAR=1, ACT_SUPERPOWER=2
/// Additional types added for extended gameplay tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u32)]
pub enum AcademyClassificationType {
    #[default]
    None = 0,
    UpgradeRadar = 1,
    Superpower = 2,
}

impl AcademyClassificationType {
    /// Parse from string (matches C++ TheAcademyClassificationTypeNames lookup)
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "NONE" | "ACT_NONE" => Self::None,
            "UPGRADE_RADAR" | "ACT_UPGRADE_RADAR" => Self::UpgradeRadar,
            "SUPERPOWER" | "SUPERWEAPON" | "ACT_SUPERPOWER" => Self::Superpower,
            _ => Self::None,
        }
    }
}

/// Special power types - matches C++ SpecialPowerType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum SpecialPowerType {
    Invalid = 0,

    // Superweapons
    DaisyCutter,
    ParadropAmerica,
    CarpetBomb,
    ClusterMines,
    EmpPulse,
    NapalmStrike,
    CashHack,
    NeutronMissile,
    SpySatellite,
    Defector,
    TerrorCell,
    Ambush,
    BlackMarketNuke,
    AnthraxBomb,
    ScudStorm,
    Demoralize,
    CrateDrop,
    A10ThunderboltStrike,
    DetonateDirtyNuke,
    ArtilleryBarrage,

    // Special abilities
    MissileDefenderLaserGuidedMissiles,
    RemoteCharges,
    TimedCharges,
    HelixNapalmBomb,
    HackerDisableBuilding,
    TankHunterTntAttack,
    BlackLotusCaptureBuilding,
    BlackLotusDisableVehicleHack,
    BlackLotusStealCashHack,
    InfantryCaptureBuilding,
    RadarVanScan,
    SpyDrone,
    DisguiseAsVehicle,
    BoobyTrap,
    RepairVehicles,
    ParticleUplinkCannon,
    CashBounty,
    ChangeBattlePlans,
    CiaIntelligence,
    CleanupArea,
    LaunchBaikonurRocket,
    SpectreGunship,
    GpsScrambler,
    Frenzy,
    SneakAttack,

    // Faction variants
    ChinaCarpetBomb,
    EarlyChinaCarpetBomb,
    LeafletDrop,
    EarlyLeafletDrop,
    EarlyFrenzy,
    CommunicationsDownload,
    EarlyRepairVehicles,
    TankParadrop,
    SupwParticleUplinkCannon,
    AirfDaisyCutter,
    NukeClusterMines,
    NukeNeutronMissile,
    AirfA10ThunderboltStrike,
    AirfSpectreGunship,
    InfaParadropAmerica,
    SlthGpsScrambler,
    AirfCarpetBomb,
    SuprCruiseMissile,
    LazrParticleUplinkCannon,
    SupwNeutronMissile,
    BattleshipBombardment,
}

impl Default for SpecialPowerType {
    fn default() -> Self {
        Self::Invalid
    }
}

impl std::str::FromStr for SpecialPowerType {
    type Err = String;

    /// Parse from string (matches C++ SpecialPowerMaskType::s_bitNameList)
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Match against C++ bit name list (case-insensitive)
        let upper = s.to_uppercase().replace("_", "");
        match upper.as_str() {
            "SPECIALINVALID" => Ok(Self::Invalid),
            "SPECIALDAISYCUTTER" => Ok(Self::DaisyCutter),
            "SPECIALPARADROPAMERICA" => Ok(Self::ParadropAmerica),
            "SPECIALCARPETBOMB" => Ok(Self::CarpetBomb),
            "SPECIALCLUSTORMINES" => Ok(Self::ClusterMines),
            "SPECIALEMPPULSE" => Ok(Self::EmpPulse),
            "SPECIALNAPALMSTRIKE" => Ok(Self::NapalmStrike),
            "SPECIALCASHHACK" => Ok(Self::CashHack),
            "SPECIALNEUTRONMISSILE" => Ok(Self::NeutronMissile),
            "SPECIALSPYSATELLITE" => Ok(Self::SpySatellite),
            "SPECIALDEFECTOR" => Ok(Self::Defector),
            "SPECIALTERRORCELL" => Ok(Self::TerrorCell),
            "SPECIALAMBUSH" => Ok(Self::Ambush),
            "SPECIALBLACKMARKETNUKE" => Ok(Self::BlackMarketNuke),
            "SPECIALANTHRAXBOMB" => Ok(Self::AnthraxBomb),
            "SPECIALSCUDSTORM" => Ok(Self::ScudStorm),
            "SPECIALDEMORALIZE" | "SPECIALDEMORALIZEOBSOLETE" => Ok(Self::Demoralize),
            "SPECIALCRATEDROP" => Ok(Self::CrateDrop),
            "SPECIALA10THUNDERBOLTSTRIKE" => Ok(Self::A10ThunderboltStrike),
            "SPECIALDETONATEDIRTYNUKE" => Ok(Self::DetonateDirtyNuke),
            "SPECIALARTILLERYBARRAGE" => Ok(Self::ArtilleryBarrage),
            "SPECIALMISSILEDEFENDERLASERGUIDEDMISSILES" => {
                Ok(Self::MissileDefenderLaserGuidedMissiles)
            }
            "SPECIALREMOTECHARGES" => Ok(Self::RemoteCharges),
            "SPECIALTIMEDCHARGES" => Ok(Self::TimedCharges),
            "SPECIALHELIXNAPALMBOMB" => Ok(Self::HelixNapalmBomb),
            "SPECIALHACKERDISABLEBUILDING" => Ok(Self::HackerDisableBuilding),
            "SPECIALTANKHOLDERTNTATTACK" => Ok(Self::TankHunterTntAttack),
            "SPECIALBLACKLOTUSCAPTUREBUILDING" => Ok(Self::BlackLotusCaptureBuilding),
            "SPECIALBLACKLOTUSDISABLEVEHICLEHACK" => Ok(Self::BlackLotusDisableVehicleHack),
            "SPECIALBLACKLOTUSSTEALCASHHACK" => Ok(Self::BlackLotusStealCashHack),
            "SPECIALINFANTRYCAPTUREBUILDING" => Ok(Self::InfantryCaptureBuilding),
            "SPECIALRADARVANSCAN" => Ok(Self::RadarVanScan),
            "SPECIALSPYDRONE" => Ok(Self::SpyDrone),
            "SPECIALDISGUISEASVEHICLE" => Ok(Self::DisguiseAsVehicle),
            "SPECIALBOOBYTRAP" => Ok(Self::BoobyTrap),
            "SPECIALREPAIRVEHICLES" => Ok(Self::RepairVehicles),
            "SPECIALPARTICLEUPLINKCANNON" => Ok(Self::ParticleUplinkCannon),
            "SPECIALCASHBOUNTY" => Ok(Self::CashBounty),
            "SPECIALCHANGEBATTLEPLANS" => Ok(Self::ChangeBattlePlans),
            "SPECIALCIAINTELLIGENCE" => Ok(Self::CiaIntelligence),
            "SPECIALCLEANUPAREA" => Ok(Self::CleanupArea),
            "SPECIALLAUNCHBAIKONURROCKET" => Ok(Self::LaunchBaikonurRocket),
            "SPECIALSPECTREGUNSHIP" => Ok(Self::SpectreGunship),
            "SPECIALGPSSCRAMBLER" => Ok(Self::GpsScrambler),
            "SPECIALFRENZY" => Ok(Self::Frenzy),
            "SPECIALSNEAKATTACK" => Ok(Self::SneakAttack),
            "SPECIALCHINACARPETBOMB" => Ok(Self::ChinaCarpetBomb),
            "EARLYSPECIALCHINACARPETBOMB" => Ok(Self::EarlyChinaCarpetBomb),
            "SPECIALLEAFLETDROP" => Ok(Self::LeafletDrop),
            "EARLYSPECIALLEAFLETDROP" => Ok(Self::EarlyLeafletDrop),
            "EARLYSPECIALFRENZY" => Ok(Self::EarlyFrenzy),
            "SPECIALCOMMUNICATIONSDOWNLOAD" => Ok(Self::CommunicationsDownload),
            "EARLYSPECIALREPAIRVEHICLES" => Ok(Self::EarlyRepairVehicles),
            "SPECIALTANKPARADROP" => Ok(Self::TankParadrop),
            "SUPWSPECIALPARTICLEUPLINKCANNON" => Ok(Self::SupwParticleUplinkCannon),
            "AIRFSPECIALDAISYCUTTER" => Ok(Self::AirfDaisyCutter),
            "NUKESPECIALCLUSTORMINES" => Ok(Self::NukeClusterMines),
            "NUKESPECIALNEUTRONMISSILE" => Ok(Self::NukeNeutronMissile),
            "AIRFSPECIALA10THUNDERBOLTSTRIKE" => Ok(Self::AirfA10ThunderboltStrike),
            "AIRFSPECIALSPECTREGUNSHIP" => Ok(Self::AirfSpectreGunship),
            "INFASPECIALPARADROPAMERICA" => Ok(Self::InfaParadropAmerica),
            "SLTHSPECIALGPSSCRAMBLER" => Ok(Self::SlthGpsScrambler),
            "AIRFSPECIALCARPETBOMB" => Ok(Self::AirfCarpetBomb),
            "SUPRSPECIALCRUISEMISSILE" => Ok(Self::SuprCruiseMissile),
            "LAZRSPECIALPARTICLEUPLINKCANNON" => Ok(Self::LazrParticleUplinkCannon),
            "SUPWSPECIALNEUTRONMISSILE" => Ok(Self::SupwNeutronMissile),
            "SPECIALBATTLESHIPBOMBARDMENT" => Ok(Self::BattleshipBombardment),
            _ => Err(format!("Unknown SpecialPowerType: {}", s)),
        }
    }
}

/// Special power template - defines a type of special power
/// Reference: C++ SpecialPowerTemplate class
#[derive(Debug, Clone)]
pub struct SpecialPowerTemplate {
    pub name: String,
    pub id: u32,
    pub power_type: SpecialPowerType,

    // Timing - matches C++ m_reloadTime (in frames)
    pub reload_time: u32,

    // Science requirements - matches C++ m_requiredScience
    pub required_science: i32, // ScienceType, SCIENCE_INVALID = -1

    // Audio - matches C++ m_initiateSound, m_initiateAtLocationSound
    pub initiate_sound: String,
    pub initiate_at_location_sound: String,

    // Flags - matches C++ boolean fields
    pub public_timer: bool,
    pub shared_n_sync: bool, // Shared between command centers
    pub shortcut_power: bool,

    // Detection and viewing - matches C++ m_detectionTime, m_viewObjectDuration, etc.
    pub detection_time: u32, // Frames for infiltration powers
    pub view_object_duration: u32,
    pub view_object_range: f32,
    pub radius_cursor_radius: f32,

    // Academy classification - matches C++ m_academyClassificationType (line 100)
    pub academy_classification_type: AcademyClassificationType,

    // Cost
    pub cost: i32,
}

impl SpecialPowerTemplate {
    /// Create a new special power template
    /// Reference: C++ SpecialPowerTemplate::SpecialPowerTemplate()
    pub fn new(name: String, id: u32) -> Self {
        Self {
            name,
            id,
            power_type: SpecialPowerType::Invalid,
            reload_time: 0,
            required_science: SCIENCE_INVALID,
            initiate_sound: String::new(),
            initiate_at_location_sound: String::new(),
            public_timer: false,
            shared_n_sync: false,
            shortcut_power: false,
            detection_time: DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT,
            view_object_duration: 0,
            view_object_range: 0.0,
            radius_cursor_radius: 0.0,
            academy_classification_type: AcademyClassificationType::default(),
            cost: 0,
        }
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_special_power_type(&self) -> SpecialPowerType {
        self.power_type
    }

    pub fn get_reload_time(&self) -> u32 {
        self.reload_time
    }

    /// Get required science type
    /// Reference: C++ SpecialPowerTemplate::getRequiredScience()
    pub fn get_required_science(&self) -> i32 {
        self.required_science
    }

    pub fn has_public_timer(&self) -> bool {
        self.public_timer
    }

    pub fn is_shared_n_sync(&self) -> bool {
        self.shared_n_sync
    }

    pub fn is_shortcut_power(&self) -> bool {
        self.shortcut_power
    }

    pub fn get_detection_time(&self) -> u32 {
        self.detection_time
    }

    /// Get academy classification type
    /// Reference: C++ m_academyClassificationType (line 100)
    pub fn get_academy_classification_type(&self) -> AcademyClassificationType {
        self.academy_classification_type
    }
}

/// Trait for objects that can use special powers
/// This allows the Common crate to check special power availability
/// without depending on the GameLogic crate's Object type
pub trait SpecialPowerObject {
    /// Check if object is disabled
    fn is_disabled(&self) -> bool;
    /// Get the object's controlling player index (for science lookup)
    fn get_controlling_player_index(&self) -> Option<i32>;
    /// Check if object has a special power module for this template
    fn has_special_power_module(&self, template: &SpecialPowerTemplate) -> bool;
}

/// Trait for players that can have sciences
pub trait SpecialPowerPlayer {
    /// Check if player has a specific science
    fn has_science(&self, science_type: i32) -> bool;
}

/// Special power store - manages all available special powers
/// Reference: C++ SpecialPowerStore class
#[derive(Debug)]
pub struct SpecialPowerStore {
    templates: Vec<SpecialPowerTemplate>,
    next_special_power_id: u32,
}

impl SpecialPowerStore {
    /// Create a new special power store
    /// Reference: C++ SpecialPowerStore::SpecialPowerStore()
    pub fn new() -> Self {
        Self {
            templates: Vec::new(),
            next_special_power_id: 0,
        }
    }

    /// Find a special power template by name
    /// Reference: C++ SpecialPowerStore::findSpecialPowerTemplatePrivate()
    pub fn find_template(&self, name: &str) -> Option<&SpecialPowerTemplate> {
        self.templates.iter().find(|t| t.name == name)
    }

    /// Find a special power template by ID
    /// Reference: C++ SpecialPowerStore::findSpecialPowerTemplateByID()
    pub fn find_template_by_id(&self, id: u32) -> Option<&SpecialPowerTemplate> {
        self.templates.iter().find(|t| t.id == id)
    }

    /// Get a special power template by index (for WorldBuilder)
    /// Reference: C++ SpecialPowerStore::getSpecialPowerTemplateByIndex()
    pub fn get_template_by_index(&self, index: usize) -> Option<&SpecialPowerTemplate> {
        self.templates.get(index)
    }

    /// Get the number of special powers
    /// Reference: C++ SpecialPowerStore::getNumSpecialPowers()
    pub fn get_num_special_powers(&self) -> usize {
        self.templates.len()
    }

    /// Add a new template
    pub fn add_template(&mut self, template: SpecialPowerTemplate) {
        self.templates.push(template);
    }

    /// Reset the store
    /// Reference: C++ SpecialPowerStore::reset()
    pub fn reset(&mut self) {
        self.templates.clear();
        self.next_special_power_id = 0;
    }

    /// Check if an object can use a special power
    /// Reference: C++ SpecialPowerStore::canUseSpecialPower() (lines 182-217)
    ///
    /// This checks:
    /// 1. Object and template are valid
    /// 2. Object has a special power module for this template
    /// 3. If required science, the player has it
    pub fn can_use_special_power(
        &self,
        obj: &dyn SpecialPowerObject,
        player: Option<&dyn SpecialPowerPlayer>,
        special_power_template: &SpecialPowerTemplate,
    ) -> bool {
        // Sanity check
        // Reference: C++ lines 184-185
        if special_power_template.get_id() == 0 {
            return false;
        }

        // As a first sanity check, the object must have a module capable of executing the power
        // Reference: C++ lines 187-188
        if !obj.has_special_power_module(special_power_template) {
            return false;
        }

        //
        // In order to execute the special powers we have attached special power modules to the objects
        // that can use them. However, just because an object has a module that is capable of
        // doing the power, does not mean the object and the player can actually execute the
        // power because some powers require a specialized science that the player must select and
        // they cannot have all of them.
        //
        // Reference: C++ comment block lines 190-197

        // Check for required science
        // Reference: C++ lines 199-207
        let required_science = special_power_template.get_required_science();
        if required_science != SCIENCE_INVALID {
            let Some(player) = player else {
                return false;
            };

            if !player.has_science(required_science) {
                return false;
            }
        }

        // I THINK THIS IS WHERE WE BAIL OUT IF A DIFFERENT CONYARD IS ALREADY CHARGING THIS SPECIAL RIGHT NOW
        // Reference: C++ comment line 210

        // All is well
        // Reference: C++ line 213
        true
    }

    /// Parse a special power definition from INI
    /// Reference: C++ SpecialPowerStore::parseSpecialPowerDefinition() (lines 35-82)
    pub fn parse_special_power_definition(
        &mut self,
        name: &str,
        properties: &HashMap<String, String>,
    ) -> Result<(), String> {
        // Check if template already exists
        if self.find_template(name).is_some() {
            return Err(format!("Special power '{}' already exists", name));
        }

        // Create new template with next ID
        let id = self.next_special_power_id + 1;
        self.next_special_power_id = id;

        let mut template = SpecialPowerTemplate::new(name.to_string(), id);

        // Parse properties using the field parse array
        // Reference: C++ m_specialPowerFieldParse array (lines 85-102)
        self.apply_field_parse(&mut template, properties);

        self.templates.push(template);
        Ok(())
    }

    /// Apply field parse to template
    /// Reference: C++ m_specialPowerFieldParse array (lines 85-102)
    fn apply_field_parse(
        &mut self,
        template: &mut SpecialPowerTemplate,
        properties: &HashMap<String, String>,
    ) {
        // Reference: C++ field parse array
        // { "ReloadTime",              INI::parseDurationUnsignedInt,  NULL, offsetof(SpecialPowerTemplate, m_reloadTime) },
        if let Some(value) = properties.get("ReloadTime") {
            if let Some(frames) = Self::parse_duration_frames(value) {
                template.reload_time = frames;
            }
        }

        // { "RequiredScience",         INI::parseScience,              NULL, offsetof(SpecialPowerTemplate, m_requiredScience) },
        if let Some(value) = properties.get("RequiredScience") {
            template.required_science = Self::parse_science(value);
        }

        // { "InitiateSound",           INI::parseAudioEventRTS,        NULL, offsetof(SpecialPowerTemplate, m_initiateSound) },
        if let Some(value) = properties.get("InitiateSound") {
            template.initiate_sound = value.clone();
        }

        // { "InitiateAtLocationSound", INI::parseAudioEventRTS,        NULL, offsetof(SpecialPowerTemplate, m_initiateAtLocationSound) },
        if let Some(value) = properties.get("InitiateAtLocationSound") {
            template.initiate_at_location_sound = value.clone();
        }

        // { "PublicTimer",             INI::parseBool,                 NULL, offsetof(SpecialPowerTemplate, m_publicTimer) },
        if let Some(value) = properties.get("PublicTimer") {
            template.public_timer = Self::parse_bool(value);
        }

        // { "Enum",                    INI::parseIndexList,            SpecialPowerMaskType::getBitNames(), offsetof(SpecialPowerTemplate, m_type) },
        if let Some(value) = properties.get("Enum") {
            template.power_type = Self::parse_special_power_enum(value);
        }

        // { "DetectionTime",           INI::parseDurationUnsignedInt,  NULL, offsetof(SpecialPowerTemplate, m_detectionTime) },
        if let Some(value) = properties.get("DetectionTime") {
            if let Some(frames) = Self::parse_duration_frames(value) {
                template.detection_time = frames;
            }
        }

        // { "SharedSyncedTimer",       INI::parseBool,                 NULL, offsetof(SpecialPowerTemplate, m_sharedNSync) },
        if let Some(value) = properties.get("SharedSyncedTimer") {
            template.shared_n_sync = Self::parse_bool(value);
        }

        // { "ViewObjectDuration",      INI::parseDurationUnsignedInt,  NULL, offsetof(SpecialPowerTemplate, m_viewObjectDuration) },
        if let Some(value) = properties.get("ViewObjectDuration") {
            if let Some(frames) = Self::parse_duration_frames(value) {
                template.view_object_duration = frames;
            }
        }

        // { "ViewObjectRange",         INI::parseReal,                 NULL, offsetof(SpecialPowerTemplate, m_viewObjectRange) },
        if let Some(value) = properties.get("ViewObjectRange") {
            if let Ok(range) = value.parse::<f32>() {
                template.view_object_range = range;
            }
        }

        // { "RadiusCursorRadius",      INI::parseReal,                 NULL, offsetof(SpecialPowerTemplate, m_radiusCursorRadius) },
        if let Some(value) = properties.get("RadiusCursorRadius") {
            if let Ok(radius) = value.parse::<f32>() {
                template.radius_cursor_radius = radius;
            }
        }

        // { "ShortcutPower",           INI::parseBool,                 NULL, offsetof(SpecialPowerTemplate, m_shortcutPower) },
        if let Some(value) = properties.get("ShortcutPower") {
            template.shortcut_power = Self::parse_bool(value);
        }

        // { "AcademyClassify",         INI::parseIndexList,            TheAcademyClassificationTypeNames, offsetof(SpecialPowerTemplate, m_academyClassificationType) },
        // Reference: C++ line 100
        if let Some(value) = properties.get("AcademyClassify") {
            template.academy_classification_type = AcademyClassificationType::from_str(value);
        }
    }

    /// Parse duration to frames (matches C++ INI::parseDurationUnsignedInt)
    fn parse_duration_frames(value: &str) -> Option<u32> {
        let value = value.trim();

        // Handle milliseconds (e.g., "1500ms")
        if value.ends_with("ms") {
            let ms_str = &value[..value.len() - 2];
            if let Ok(ms) = ms_str.parse::<u32>() {
                // Convert ms to frames (30 FPS)
                return Some((ms as f32 * 30.0 / 1000.0) as u32);
            }
        }

        // Handle seconds (e.g., "1.5s" or "1.5")
        let secs_str = if value.ends_with('s') {
            &value[..value.len() - 1]
        } else {
            value
        };

        if let Ok(secs) = secs_str.parse::<f32>() {
            // Convert seconds to frames (30 FPS)
            return Some((secs * 30.0) as u32);
        }

        // Try parsing as raw frames
        value.parse::<u32>().ok()
    }

    /// Parse boolean (matches C++ INI::parseBool)
    fn parse_bool(value: &str) -> bool {
        matches!(value.trim().to_lowercase().as_str(), "yes" | "1" | "true")
    }

    /// Parse science type (matches C++ INI::parseScience)
    fn parse_science(value: &str) -> i32 {
        let value = value.trim();
        if value.is_empty() || value.eq_ignore_ascii_case("None") {
            return SCIENCE_INVALID;
        }
        // In the full implementation, this would look up the science in the science store
        // For now, use a simple hash-based ID
        let mut hash: i32 = 0;
        for c in value.chars() {
            hash = hash.wrapping_mul(31).wrapping_add(c as i32);
        }
        hash.abs()
    }

    /// Parse special power enum (matches C++ INI::parseIndexList with SpecialPowerMaskType::getBitNames())
    fn parse_special_power_enum(value: &str) -> SpecialPowerType {
        let value = value.trim();
        SpecialPowerType::from_str(value).unwrap_or_else(|_| SpecialPowerType::Invalid)
    }
}

impl Default for SpecialPowerStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Special power module - runtime instance of a special power
/// Reference: C++ SpecialPowerModule class
#[derive(Debug, Clone)]
pub struct SpecialPowerModule {
    /// Template this module uses
    template: SpecialPowerTemplate,

    /// Frame when this power becomes available
    /// Reference: C++ m_availableOnFrame
    available_on_frame: u32,

    /// Pause reference count
    /// Reference: C++ m_pausedCount
    paused_count: i32,

    /// Frame when paused
    /// Reference: C++ m_pausedOnFrame
    paused_on_frame: u32,

    /// Percent ready when paused
    /// Reference: C++ m_pausedPercent
    paused_percent: f32,

    /// Update module starts attack flag
    _update_module_starts_attack: bool,

    /// Starts paused flag
    _starts_paused: bool,

    /// Script only flag
    scripted_special_power_only: bool,
}

impl SpecialPowerModule {
    /// Create a new special power module
    /// Reference: C++ SpecialPowerModule::SpecialPowerModule()
    pub fn new(template: SpecialPowerTemplate) -> Self {
        Self {
            template,
            available_on_frame: 0,
            paused_count: 0,
            paused_on_frame: 0,
            paused_percent: 0.0,
            _update_module_starts_attack: false,
            _starts_paused: false,
            scripted_special_power_only: false,
        }
    }

    /// Check if this module is for a specific power
    /// Reference: C++ SpecialPowerModule::isModuleForPower()
    pub fn is_module_for_power(&self, template: &SpecialPowerTemplate) -> bool {
        self.template.id == template.id
    }

    /// Check if the power is ready
    /// Reference: C++ SpecialPowerModule::isReady()
    pub fn is_ready(&self, current_frame: u32) -> bool {
        self.paused_count == 0 && current_frame >= self.available_on_frame
    }

    /// Get the percentage ready (0.0 to 1.0)
    /// Reference: C++ SpecialPowerModule::getPercentReady()
    pub fn get_percent_ready(&self, current_frame: u32) -> f32 {
        // Don't consider it ready if paused
        if self.paused_count > 0 && self.paused_percent == 1.0 {
            return 0.99999;
        }

        // Easy case - is ready
        if self.is_ready(current_frame) {
            return 1.0;
        }

        if self.paused_count > 0 {
            return self.paused_percent;
        }

        // Sanity check
        if self.template.reload_time == 0 {
            return 0.0;
        }

        // Calculate the percent
        let ready_frame = self.available_on_frame;
        if ready_frame <= current_frame {
            return 1.0;
        }

        let percent =
            1.0 - ((ready_frame - current_frame) as f32 / self.template.reload_time as f32);
        percent.max(0.0).min(1.0)
    }

    /// Get the ready frame
    /// Reference: C++ SpecialPowerModule::getReadyFrame()
    pub fn get_ready_frame(&self) -> u32 {
        self.available_on_frame
    }

    /// Set the ready frame
    /// Reference: C++ SpecialPowerModule::setReadyFrame()
    pub fn set_ready_frame(&mut self, frame: u32, current_frame: u32) {
        self.available_on_frame = frame;
        // Update paused frame if changed
        self.paused_on_frame = current_frame;
    }

    /// Start power recharge
    /// Reference: C++ SpecialPowerModule::startPowerRecharge()
    pub fn start_power_recharge(&mut self, current_frame: u32) {
        // Set the frame we will be 100% available on
        self.available_on_frame = current_frame + self.template.reload_time;
    }

    /// Pause or unpause the countdown
    /// Reference: C++ SpecialPowerModule::pauseCountdown()
    pub fn pause_countdown(&mut self, pause: bool, current_frame: u32) {
        if pause {
            if self.paused_count == 0 {
                // First pause - save current state
                self.paused_on_frame = current_frame;
                self.paused_percent = self.get_percent_ready(current_frame);
            }
            self.paused_count += 1;
        } else {
            self.paused_count -= 1;
            if self.paused_count == 0 {
                // Last unpause - restore state
                let elapsed = current_frame - self.paused_on_frame;
                self.available_on_frame += elapsed;
            }
        }
    }

    /// Check if this is a script-only power
    /// Reference: C++ SpecialPowerModule::isScriptOnly()
    pub fn is_script_only(&self) -> bool {
        self.scripted_special_power_only
    }

    /// Get the power name
    /// Reference: C++ SpecialPowerModule::getPowerName()
    pub fn get_power_name(&self) -> &str {
        &self.template.name
    }

    /// Get the template
    pub fn get_template(&self) -> &SpecialPowerTemplate {
        &self.template
    }
}

/// Player power state - tracks power availability for a player
/// Reference: C++ Player class special power methods
#[derive(Debug, Clone)]
pub struct PlayerPowerState {
    /// Shared power ready frames - for powers with shared_n_sync=true
    shared_power_frames: HashMap<u32, u32>,
}

impl PlayerPowerState {
    pub fn new() -> Self {
        Self {
            shared_power_frames: HashMap::new(),
        }
    }

    /// Get or start the ready frame for a shared power
    /// Reference: C++ Player::getOrStartSpecialPowerReadyFrame()
    pub fn get_or_start_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
        current_frame: u32,
    ) -> u32 {
        *self
            .shared_power_frames
            .entry(template.id)
            .or_insert_with(|| current_frame + template.reload_time)
    }

    /// Reset or start the ready frame for a shared power
    /// Reference: C++ Player::resetOrStartSpecialPowerReadyFrame()
    pub fn reset_or_start_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
        current_frame: u32,
    ) {
        self.shared_power_frames
            .insert(template.id, current_frame + template.reload_time);
    }

    /// Express the ready frame for a shared power
    /// Reference: C++ Player::expressSpecialPowerReadyFrame()
    pub fn express_special_power_ready_frame(
        &mut self,
        template: &SpecialPowerTemplate,
        frame: u32,
    ) {
        self.shared_power_frames.insert(template.id, frame);
    }
}

impl Default for PlayerPowerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Special power execution parameters
#[derive(Debug, Clone)]
pub struct SpecialPowerExecution {
    pub power_id: u32,
    pub target_position: Option<(f32, f32, f32)>,
    pub target_object_id: Option<u32>,
    pub angle: f32,
    pub command_options: u32,
}

impl SpecialPowerExecution {
    pub fn new(power_id: u32) -> Self {
        Self {
            power_id,
            target_position: None,
            target_object_id: None,
            angle: 0.0,
            command_options: 0,
        }
    }

    pub fn with_location(mut self, x: f32, y: f32, z: f32) -> Self {
        self.target_position = Some((x, y, z));
        self
    }

    pub fn with_object(mut self, object_id: u32) -> Self {
        self.target_object_id = Some(object_id);
        self
    }

    pub fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    pub fn with_options(mut self, options: u32) -> Self {
        self.command_options = options;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_special_power_template_creation() {
        let template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        assert_eq!(template.get_name(), "TestPower");
        assert_eq!(template.get_id(), 1);
        assert_eq!(template.get_special_power_type(), SpecialPowerType::Invalid);
        assert!(!template.has_public_timer());
        assert!(!template.is_shared_n_sync());
        // Default detection time is 300 frames (10 seconds at 30 FPS)
        assert_eq!(
            template.get_detection_time(),
            DEFAULT_DEFECTION_DETECTION_PROTECTION_TIME_LIMIT
        );
        // Default academy classification is ACT_NONE.
        assert_eq!(
            template.get_academy_classification_type(),
            AcademyClassificationType::None
        );
        // Default required science is SCIENCE_INVALID
        assert_eq!(template.get_required_science(), SCIENCE_INVALID);
    }

    #[test]
    fn test_special_power_store() {
        let mut store = SpecialPowerStore::new();

        let template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        store.add_template(template);

        assert_eq!(store.get_num_special_powers(), 1);

        let found = store.find_template("TestPower");
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_id(), 1);

        let found_by_id = store.find_template_by_id(1);
        assert!(found_by_id.is_some());
        assert_eq!(found_by_id.unwrap().get_name(), "TestPower");
    }

    #[test]
    fn test_can_use_special_power() {
        // Mock object that implements SpecialPowerObject
        struct MockObject {
            disabled: bool,
            has_module: bool,
        }
        impl SpecialPowerObject for MockObject {
            fn is_disabled(&self) -> bool {
                self.disabled
            }
            fn get_controlling_player_index(&self) -> Option<i32> {
                Some(0)
            }
            fn has_special_power_module(&self, _template: &SpecialPowerTemplate) -> bool {
                self.has_module
            }
        }

        // Mock player that implements SpecialPowerPlayer
        struct MockPlayer {
            sciences: Vec<i32>,
        }
        impl SpecialPowerPlayer for MockPlayer {
            fn has_science(&self, science: i32) -> bool {
                self.sciences.contains(&science)
            }
        }

        let store = SpecialPowerStore::new();
        let template = SpecialPowerTemplate::new("TestPower".to_string(), 1);

        // Object with module, no science required
        let obj = MockObject {
            disabled: false,
            has_module: true,
        };
        assert!(store.can_use_special_power(&obj, None, &template));

        // Object without module
        let obj_no_module = MockObject {
            disabled: false,
            has_module: false,
        };
        assert!(!store.can_use_special_power(&obj_no_module, None, &template));

        // Disabled object
        let obj_disabled = MockObject {
            disabled: true,
            has_module: true,
        };
        // Note: can_use_special_power doesn't check is_disabled in C++ (that's done by callers)
        // So this should still return true since the module exists
        assert!(store.can_use_special_power(&obj_disabled, None, &template));

        // With required science - player doesn't have it
        let mut template_with_science = SpecialPowerTemplate::new("SciencePower".to_string(), 2);
        template_with_science.required_science = 100; // Some science ID
        let player_no_science = MockPlayer { sciences: vec![] };
        assert!(!store.can_use_special_power(
            &obj,
            Some(&player_no_science),
            &template_with_science
        ));

        // With required science - player has it
        let player_with_science = MockPlayer {
            sciences: vec![100],
        };
        assert!(store.can_use_special_power(
            &obj,
            Some(&player_with_science),
            &template_with_science
        ));
    }

    #[test]
    fn test_parse_special_power_definition() {
        let mut store = SpecialPowerStore::new();
        let mut props = HashMap::new();
        props.insert("ReloadTime".to_string(), "5.0s".to_string());
        props.insert("PublicTimer".to_string(), "Yes".to_string());
        props.insert("Enum".to_string(), "SPECIAL_DAISY_CUTTER".to_string());
        props.insert("AcademyClassify".to_string(), "SUPERWEAPON".to_string());

        let result = store.parse_special_power_definition("TestPower", &props);
        assert!(result.is_ok());

        let template = store
            .find_template("TestPower")
            .expect("Template should exist");
        assert_eq!(template.reload_time, 150); // 5.0s * 30 fps
        assert!(template.public_timer);
        assert_eq!(template.power_type, SpecialPowerType::DaisyCutter);
        assert_eq!(
            template.academy_classification_type,
            AcademyClassificationType::Superpower
        );
    }

    #[test]
    fn test_academy_classification_type() {
        assert_eq!(
            AcademyClassificationType::from_str("TACTICAL"),
            AcademyClassificationType::None
        );
        assert_eq!(
            AcademyClassificationType::from_str("strategic"),
            AcademyClassificationType::None
        );
        assert_eq!(
            AcademyClassificationType::from_str("SUPERWEAPON"),
            AcademyClassificationType::Superpower
        );
        assert_eq!(
            AcademyClassificationType::from_str("SUPERPOWER"),
            AcademyClassificationType::Superpower
        );
        assert_eq!(
            AcademyClassificationType::from_str("defensive"),
            AcademyClassificationType::None
        );
        assert_eq!(
            AcademyClassificationType::from_str("economic"),
            AcademyClassificationType::None
        );
        assert_eq!(
            AcademyClassificationType::from_str("unknown"),
            AcademyClassificationType::None
        );
    }

    #[test]
    fn test_special_power_type_from_str() {
        use std::str::FromStr;
        assert_eq!(
            SpecialPowerType::from_str("SPECIAL_DAISY_CUTTER").unwrap(),
            SpecialPowerType::DaisyCutter
        );
        assert_eq!(
            SpecialPowerType::from_str("SPECIAL_NEUTRON_MISSILE").unwrap(),
            SpecialPowerType::NeutronMissile
        );
        assert_eq!(
            SpecialPowerType::from_str("SPECIAL_SCUD_STORM").unwrap(),
            SpecialPowerType::ScudStorm
        );
        assert!(SpecialPowerType::from_str("INVALID_TYPE").is_err());
    }

    #[test]
    fn test_special_power_module_ready_state() {
        let mut template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        template.reload_time = 100; // 100 frames

        let mut module = SpecialPowerModule::new(template);

        // Start recharge at frame 0
        module.start_power_recharge(0);

        // At frame 0, should not be ready
        assert!(!module.is_ready(0));
        assert_eq!(module.get_percent_ready(0), 0.0);

        // At frame 50, should be 50% ready
        assert!(!module.is_ready(50));
        assert!((module.get_percent_ready(50) - 0.5).abs() < 0.01);

        // At frame 100, should be ready
        assert!(module.is_ready(100));
        assert_eq!(module.get_percent_ready(100), 1.0);

        // After frame 100, should still be ready
        assert!(module.is_ready(150));
        assert_eq!(module.get_percent_ready(150), 1.0);
    }

    #[test]
    fn test_special_power_module_pause() {
        let mut template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        template.reload_time = 100;

        let mut module = SpecialPowerModule::new(template);
        module.start_power_recharge(0);

        // Pause at frame 50 (50% ready)
        module.pause_countdown(true, 50);
        assert!(!module.is_ready(50));

        // At frame 100, still paused so not ready
        assert!(!module.is_ready(100));

        // Unpause at frame 100
        module.pause_countdown(false, 100);

        // Should now be ready at frame 150 (50 frames paused + 100 original)
        assert!(!module.is_ready(100));
        assert!(module.is_ready(150));
    }

    #[test]
    fn test_player_power_state_shared_powers() {
        let mut state = PlayerPowerState::new();

        let mut template = SpecialPowerTemplate::new("SharedPower".to_string(), 1);
        template.reload_time = 100;
        template.shared_n_sync = true;

        // First call should start the timer
        let ready_frame = state.get_or_start_special_power_ready_frame(&template, 0);
        assert_eq!(ready_frame, 100);

        // Second call should return the same frame
        let ready_frame2 = state.get_or_start_special_power_ready_frame(&template, 50);
        assert_eq!(ready_frame2, 100);

        // Reset should update the frame
        state.reset_or_start_special_power_ready_frame(&template, 50);
        let ready_frame3 = state.get_or_start_special_power_ready_frame(&template, 60);
        assert_eq!(ready_frame3, 150);
    }

    #[test]
    fn test_special_power_execution() {
        let exec = SpecialPowerExecution::new(1)
            .with_location(100.0, 200.0, 0.0)
            .with_angle(45.0)
            .with_options(0x1);

        assert_eq!(exec.power_id, 1);
        assert_eq!(exec.target_position, Some((100.0, 200.0, 0.0)));
        assert_eq!(exec.angle, 45.0);
        assert_eq!(exec.command_options, 0x1);
    }

    #[test]
    fn test_multiple_pause_unpause() {
        let mut template = SpecialPowerTemplate::new("TestPower".to_string(), 1);
        template.reload_time = 100;

        let mut module = SpecialPowerModule::new(template);
        module.start_power_recharge(0);

        // Multiple pauses
        module.pause_countdown(true, 50);
        module.pause_countdown(true, 50);
        assert!(!module.is_ready(100));

        // First unpause - still paused
        module.pause_countdown(false, 100);
        assert!(!module.is_ready(100));

        // Second unpause - now unpaused
        module.pause_countdown(false, 100);
        assert!(module.is_ready(150));
    }

    #[test]
    fn test_parse_duration_frames() {
        assert_eq!(SpecialPowerStore::parse_duration_frames("1500ms"), Some(45));
        assert_eq!(SpecialPowerStore::parse_duration_frames("1.5s"), Some(45));
        assert_eq!(SpecialPowerStore::parse_duration_frames("2s"), Some(60));
        assert_eq!(SpecialPowerStore::parse_duration_frames("90"), Some(90)); // raw frames
    }

    #[test]
    fn test_parse_bool() {
        assert!(SpecialPowerStore::parse_bool("Yes"));
        assert!(SpecialPowerStore::parse_bool("yes"));
        assert!(SpecialPowerStore::parse_bool("TRUE"));
        assert!(SpecialPowerStore::parse_bool("true"));
        assert!(SpecialPowerStore::parse_bool("1"));
        assert!(!SpecialPowerStore::parse_bool("No"));
        assert!(!SpecialPowerStore::parse_bool("0"));
        assert!(!SpecialPowerStore::parse_bool("false"));
    }
}
