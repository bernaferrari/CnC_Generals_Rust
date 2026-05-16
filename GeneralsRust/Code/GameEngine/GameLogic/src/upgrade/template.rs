//! Upgrade Template System
//!
//! Defines upgrade templates that are loaded from INI files.
//! Matches C++ UpgradeTemplate from Upgrade.h/.cpp
//!
//! Original C++ Author: Colin Day, March 2002

use super::{upgrade_mask_for_name, UpgradeError, UpgradeMask, UpgradeResult};
use crate::common::*;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};

/// Type of upgrade
/// Matches C++ UpgradeType from Upgrade.h
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum UpgradeType {
    /// Applies to entire player
    Player = 0,
    /// Applies to specific object instance
    Object = 1,
}

/// Upgrade template definition
/// Matches C++ UpgradeTemplate from Upgrade.h
#[derive(Debug, Clone)]
pub struct UpgradeTemplate {
    /// Upgrade type (player or object)
    upgrade_type: UpgradeType,
    /// Upgrade name
    name: AsciiString,
    /// Name key for fast lookup
    name_key: NameKeyType,
    /// Display name label for UI
    display_name_label: AsciiString,
    /// Build time in seconds
    build_time: Real,
    /// Cost in money
    cost: Int,
    /// Unique bit mask for this upgrade
    mask: UpgradeMask,
    /// Sound played when research completed
    research_sound: AudioEventRTS,
    /// Secondary sound played when research completed
    unit_specific_sound: AudioEventRTS,
    /// Button image name
    button_image_name: AsciiString,
    /// Academy classification
    academy_classification: u32,
    /// Whether upgrade affects existing objects of this type
    affects_existing_objects: bool,
    /// Whether upgrade can be stacked multiple times
    is_stackable: bool,
}

impl UpgradeTemplate {
    /// Create a new upgrade template
    pub fn new(name: AsciiString) -> Self {
        let name_key = NameKeyGenerator::name_to_key(&name);
        let mask = upgrade_mask_for_name(&name);

        Self {
            upgrade_type: UpgradeType::Player,
            name,
            name_key,
            display_name_label: AsciiString::default(),
            build_time: 0.0,
            cost: 0,
            mask,
            research_sound: AudioEventRTS::default(),
            unit_specific_sound: AudioEventRTS::default(),
            button_image_name: AsciiString::default(),
            academy_classification: 0,
            affects_existing_objects: true,
            is_stackable: false,
        }
    }

    /// Create a veterancy upgrade
    /// Matches C++ UpgradeTemplate::friend_makeVeterancyUpgrade
    pub fn make_veterancy_upgrade(level: &str) -> Self {
        let mut name = AsciiString::from("Upgrade_Veterancy_");
        name.push_str(level);

        let name_key = NameKeyGenerator::name_to_key(&name);
        let mask = upgrade_mask_for_name(&name);

        Self {
            upgrade_type: UpgradeType::Object,
            name,
            name_key,
            display_name_label: AsciiString::default(),
            build_time: 0.0,
            cost: 0,
            mask,
            research_sound: AudioEventRTS::default(),
            unit_specific_sound: AudioEventRTS::default(),
            button_image_name: AsciiString::default(),
            academy_classification: 0,
            affects_existing_objects: true,
            is_stackable: false,
        }
    }

    // Getters
    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn get_name_key(&self) -> NameKeyType {
        self.name_key
    }

    /// Get the upgrade ID (name key)
    /// Matches C++ UpgradeTemplate::GetID()
    pub fn get_id(&self) -> NameKeyType {
        self.name_key
    }

    pub fn get_display_name(&self) -> &AsciiString {
        &self.display_name_label
    }

    /// Matches C++ UpgradeTemplate::getAcademyClassificationType
    pub fn get_academy_classification(&self) -> u32 {
        self.academy_classification
    }

    pub fn get_upgrade_type(&self) -> UpgradeType {
        self.upgrade_type
    }

    pub fn get_mask(&self) -> UpgradeMask {
        self.mask
    }

    /// Alias for get_mask() for compatibility
    pub fn mask(&self) -> UpgradeMask {
        self.mask
    }

    pub fn get_build_time(&self) -> Real {
        self.build_time
    }

    pub fn get_cost(&self) -> Int {
        self.cost
    }

    pub fn get_research_sound(&self) -> &AudioEventRTS {
        &self.research_sound
    }

    pub fn get_unit_specific_sound(&self) -> &AudioEventRTS {
        &self.unit_specific_sound
    }

    pub fn get_button_image_name(&self) -> &AsciiString {
        &self.button_image_name
    }

    pub fn affects_existing_objects(&self) -> bool {
        self.affects_existing_objects
    }

    pub fn is_stackable(&self) -> bool {
        self.is_stackable
    }

    // Setters
    pub fn set_upgrade_type(&mut self, upgrade_type: UpgradeType) {
        self.upgrade_type = upgrade_type;
    }

    pub fn set_name(&mut self, name: AsciiString) {
        self.name_key = NameKeyGenerator::name_to_key(&name);
        self.mask = upgrade_mask_for_name(&name);
        self.name = name;
    }

    pub fn set_display_name(&mut self, label: AsciiString) {
        self.display_name_label = label;
    }

    pub fn set_build_time(&mut self, time: Real) {
        self.build_time = time;
    }

    pub fn set_cost(&mut self, cost: Int) {
        self.cost = cost;
    }

    pub fn set_button_image_name(&mut self, name: AsciiString) {
        self.button_image_name = name;
    }

    /// Calculate actual time to build in logic frames
    /// Matches C++ UpgradeTemplate::calcTimeToBuild
    pub fn calc_time_to_build(&self, player: &Player) -> Int {
        // In debug builds with instant build cheat
        #[cfg(any(debug_assertions, feature = "internal", feature = "allow_debug_cheats"))]
        if player.builds_instantly() {
            return 1;
        }
        let _ = player; // Avoid unused warning in release builds

        // Convert seconds to logic frames (30 FPS)
        const LOGICFRAMES_PER_SECOND: Real = 30.0;
        (self.build_time * LOGICFRAMES_PER_SECOND) as Int
    }

    /// Calculate actual cost to build
    /// Matches C++ UpgradeTemplate::calcCostToBuild
    pub fn calc_cost_to_build(&self, _player: &Player) -> Int {
        self.cost
    }

    /// Parse from INI file
    /// Matches C++ UpgradeTemplate field parse table
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, UPGRADE_TEMPLATE_FIELDS)
    }
}

impl Default for UpgradeTemplate {
    fn default() -> Self {
        Self::new(AsciiString::default())
    }
}

impl Snapshotable for UpgradeTemplate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// INI parsing functions
fn parse_display_name(
    _ini: &mut INI,
    template: &mut UpgradeTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    template.display_name_label = AsciiString::from(*value);
    Ok(())
}

fn parse_upgrade_type(
    _ini: &mut INI,
    template: &mut UpgradeTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    template.upgrade_type = match *value {
        "PLAYER" => UpgradeType::Player,
        "OBJECT" => UpgradeType::Object,
        _ => return Err(INIError::InvalidData),
    };
    Ok(())
}

fn parse_build_time(
    _ini: &mut INI,
    template: &mut UpgradeTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    template.build_time = INI::parse_real(value)?;
    Ok(())
}

fn parse_build_cost(
    _ini: &mut INI,
    template: &mut UpgradeTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    template.cost = INI::parse_int(value)?;
    Ok(())
}

fn parse_button_image(
    _ini: &mut INI,
    template: &mut UpgradeTemplate,
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = tokens
        .iter()
        .skip_while(|t| **t == "=")
        .next()
        .ok_or(INIError::InvalidData)?;
    template.button_image_name = AsciiString::from(*value);
    Ok(())
}

const UPGRADE_TEMPLATE_FIELDS: &[FieldParse<UpgradeTemplate>] = &[
    FieldParse {
        token: "DisplayName",
        parse: parse_display_name,
    },
    FieldParse {
        token: "Type",
        parse: parse_upgrade_type,
    },
    FieldParse {
        token: "BuildTime",
        parse: parse_build_time,
    },
    FieldParse {
        token: "BuildCost",
        parse: parse_build_cost,
    },
    FieldParse {
        token: "ButtonImage",
        parse: parse_button_image,
    },
];

// Mock-based tests removed to avoid mocks in fidelity-critical code.
