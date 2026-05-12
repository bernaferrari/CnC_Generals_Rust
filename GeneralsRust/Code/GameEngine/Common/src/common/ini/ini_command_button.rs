//! INI Command Button parsing module
//! Author: Colin Day, March 2002
//! Desc: Command buttons are the atomic units we can configure into command sets
//!       to then display in the context sensitive user interface

use super::ini::{FieldParse, INIError, INILoadType, INIResult, LookupListRec, INI};
use crate::common::ini::ini_misc_audio::AudioEventRTS;
use crate::common::rts::{get_science_store, ScienceType, WeaponSlotType, SCIENCE_INVALID};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Command button options flags
#[derive(Debug, Clone, Copy)]
pub struct CommandButtonOptions {
    pub need_special_power_science: bool,
    pub contextual_animates: bool,
    pub one_shot: bool,
    pub scripted_only: bool,
    pub player_upgrade: bool,
    pub cancel_all: bool,
    pub not_queueable: bool,
    pub ok_for_multi_select: bool,
    pub check_like: bool,
    pub toggle_image_on_selection: bool,
    pub must_be_stopped: bool,
    pub cancel_like: bool,
    pub need_upgrade: bool,
    pub reverse_button_order: bool,
}

const COMMAND_OPTION_NAMES: &[&str] = &[
    "NEED_TARGET_ENEMY_OBJECT",
    "NEED_TARGET_NEUTRAL_OBJECT",
    "NEED_TARGET_ALLY_OBJECT",
    "NEED_TARGET_PRISONER",
    "ALLOW_SHRUBBERY_TARGET",
    "NEED_TARGET_POS",
    "NEED_UPGRADE",
    "NEED_SPECIAL_POWER_SCIENCE",
    "OK_FOR_MULTI_SELECT",
    "CONTEXTMODE_COMMAND",
    "CHECK_LIKE",
    "ALLOW_MINE_TARGET",
    "ATTACK_OBJECTS_POSITION",
    "OPTION_ONE",
    "OPTION_TWO",
    "OPTION_THREE",
    "NOT_QUEUEABLE",
    "SINGLE_USE_COMMAND",
    "---DO-NOT-USE---",
    "SCRIPT_ONLY",
    "IGNORES_UNDERPOWERED",
    "USES_MINE_CLEARING_WEAPONSET",
    "CAN_USE_WAYPOINTS",
    "MUST_BE_STOPPED",
];

const WEAPON_SLOT_LOOKUP_LIST: &[LookupListRec] = &[
    LookupListRec {
        name: "PRIMARY",
        value: 0,
    },
    LookupListRec {
        name: "SECONDARY",
        value: 1,
    },
    LookupListRec {
        name: "TERTIARY",
        value: 2,
    },
    LookupListRec { name: "", value: 0 },
];

const BUTTON_BORDER_LOOKUP_LIST: &[LookupListRec] = &[
    LookupListRec {
        name: "NONE",
        value: 0,
    },
    LookupListRec {
        name: "BUILD",
        value: 1,
    },
    LookupListRec {
        name: "UPGRADE",
        value: 2,
    },
    LookupListRec {
        name: "ACTION",
        value: 3,
    },
    LookupListRec {
        name: "SYSTEM",
        value: 4,
    },
    LookupListRec { name: "", value: 0 },
];

const RADIUS_CURSOR_NAMES: &[&str] = &[
    "NONE",
    "ATTACK_DAMAGE_AREA",
    "ATTACK_SCATTER_AREA",
    "ATTACK_CONTINUE_AREA",
    "GUARD_AREA",
    "EMERGENCY_REPAIR",
    "FRIENDLY_SPECIALPOWER",
    "OFFENSIVE_SPECIALPOWER",
    "SUPERWEAPON_SCATTER_AREA",
    "PARTICLECANNON",
    "A10STRIKE",
    "CARPETBOMB",
    "DAISYCUTTER",
    "PARADROP",
    "SPYSATELLITE",
    "SPECTREGUNSHIP",
    "HELIX_NAPALM_BOMB",
    "NUCLEARMISSILE",
    "EMPPULSE",
    "ARTILLERYBARRAGE",
    "NAPALMSTRIKE",
    "CLUSTERMINES",
    "SCUDSTORM",
    "ANTHRAXBOMB",
    "AMBUSH",
    "RADAR",
    "SPYDRONE",
    "FRENZY",
    "CLEARMINES",
    "AMBULANCE",
];

impl Default for CommandButtonOptions {
    fn default() -> Self {
        Self {
            need_special_power_science: false,
            contextual_animates: false,
            one_shot: false,
            scripted_only: false,
            player_upgrade: false,
            cancel_all: false,
            not_queueable: false,
            ok_for_multi_select: false,
            check_like: false,
            toggle_image_on_selection: false,
            must_be_stopped: false,
            cancel_like: false,
            need_upgrade: false,
            reverse_button_order: false,
        }
    }
}

/// Command button structure
#[derive(Debug, Clone)]
pub struct CommandButton {
    pub name: String,
    pub command: String,
    pub object: String,
    pub upgrade: String,
    pub text_label: String,
    pub purchased_label: String,
    pub conflicting_label: String,
    pub button_image: String,
    pub cursor_name: String,
    pub button_border_type: String,
    pub descriptive_text: String,
    pub conflicting_element: String,
    pub purchase_cost: i32,
    pub weapon_slot: WeaponSlotType,
    pub max_shots_to_fire: i32,
    pub special_power_template: Option<String>,
    pub science_required: Vec<String>,
    pub science_disabled: Vec<String>,
    pub parsed_science_required: Vec<ScienceType>,
    pub parsed_science_disabled: Vec<ScienceType>,
    pub sciences_ids: Vec<ScienceType>,
    pub options_bits: u32,
    pub upgrade_discount: f32,
    pub disable_on_modes: Vec<String>,
    pub options: CommandButtonOptions,
    pub radius_cursor_type: String,
    pub invalid_cursor_name: String,
    pub unit_specific_sound: AudioEventRTS,
    pub maximum_cast_range: f32,
    pub is_override: bool,
}

impl Default for CommandButton {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: String::new(),
            object: String::new(),
            upgrade: String::new(),
            text_label: String::new(),
            purchased_label: String::new(),
            conflicting_label: String::new(),
            button_image: String::new(),
            cursor_name: String::new(),
            button_border_type: "SYSTEM".to_string(),
            descriptive_text: String::new(),
            conflicting_element: String::new(),
            purchase_cost: 0,
            weapon_slot: WeaponSlotType::Primary,
            max_shots_to_fire: i32::MAX,
            special_power_template: None,
            science_required: Vec::new(),
            science_disabled: Vec::new(),
            parsed_science_required: Vec::new(),
            parsed_science_disabled: Vec::new(),
            sciences_ids: Vec::new(),
            options_bits: 0,
            upgrade_discount: 1.0,
            disable_on_modes: Vec::new(),
            options: CommandButtonOptions::default(),
            radius_cursor_type: String::new(),
            invalid_cursor_name: String::new(),
            unit_specific_sound: AudioEventRTS::default(),
            maximum_cast_range: 0.0,
            is_override: false,
        }
    }
}

impl CommandButton {
    /// Create a new command button with the given name
    pub fn new(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    /// Get the name of this command button
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Mark this button as an override
    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }

    /// Check if this button is an override
    pub fn is_override(&self) -> bool {
        self.is_override
    }

    /// Get the special power template name
    pub fn get_special_power_template(&self) -> Option<&String> {
        self.special_power_template.as_ref()
    }

    /// Get the options flags
    pub fn get_options(&self) -> CommandButtonOptions {
        self.options
    }

    /// Parse command button from INI
    pub fn parse_from_ini(ini: &mut INI, name: String) -> INIResult<Self> {
        let mut button = Self::new(name);
        ini.init_from_ini_with_fields(&mut button, CommandButton::get_field_parse())?;
        Ok(button)
    }

    /// Parse command button fields
    /// Get the field parsing table for command buttons
    pub fn get_field_parse() -> &'static [FieldParse<Self>] {
        const PARSE_TABLE: &[FieldParse<CommandButton>] = &[
            FieldParse {
                token: "Command",
                parse: parse_command_field_command,
            },
            FieldParse {
                token: "Options",
                parse: parse_command_field_options,
            },
            FieldParse {
                token: "Object",
                parse: parse_command_field_object,
            },
            FieldParse {
                token: "Upgrade",
                parse: parse_command_field_upgrade,
            },
            FieldParse {
                token: "TextLabel",
                parse: parse_command_field_text_label,
            },
            FieldParse {
                token: "DescriptLabel",
                parse: parse_command_field_descript_label,
            },
            FieldParse {
                token: "PurchasedLabel",
                parse: parse_command_field_purchased_label,
            },
            FieldParse {
                token: "ConflictingLabel",
                parse: parse_command_field_conflicting_label,
            },
            FieldParse {
                token: "ButtonImage",
                parse: parse_command_field_button_image,
            },
            FieldParse {
                token: "CursorName",
                parse: parse_command_field_cursor_name,
            },
            FieldParse {
                token: "InvalidCursorName",
                parse: parse_command_field_invalid_cursor,
            },
            FieldParse {
                token: "WeaponSlot",
                parse: parse_command_field_weapon_slot,
            },
            FieldParse {
                token: "MaxShotsToFire",
                parse: parse_command_field_max_shots_to_fire,
            },
            FieldParse {
                token: "ButtonBorderType",
                parse: parse_command_field_button_border_type,
            },
            FieldParse {
                token: "RadiusCursorType",
                parse: parse_command_field_radius_cursor_type,
            },
            FieldParse {
                token: "UnitSpecificSound",
                parse: parse_command_field_unit_specific_sound,
            },
            FieldParse {
                token: "Science",
                parse: parse_command_field_science,
            },
            FieldParse {
                token: "SpecialPower",
                parse: parse_command_field_special_power,
            },
        ];
        PARSE_TABLE
    }

    /// Validate the command button configuration
    pub fn validate(&self) -> INIResult<()> {
        // Check if button with special power template also has the appropriate option set
        if self.special_power_template.is_some() && !self.options.need_special_power_science {
            eprintln!(
                "CommandButton {} has SpecialPower = {} but the button also requires Options = NEED_SPECIAL_POWER_SCIENCE",
                self.name,
                self.special_power_template.as_ref().unwrap()
            );
            return Err(INIError::InvalidData);
        }

        // Check if button has NEED_SPECIAL_POWER_SCIENCE but no special power template
        if self.options.need_special_power_science && self.special_power_template.is_none() {
            eprintln!(
                "CommandButton {} has Options = NEED_SPECIAL_POWER_SCIENCE but doesn't specify a SpecialPower",
                self.name
            );
            return Err(INIError::InvalidData);
        }

        Ok(())
    }

    /// Check if button requires a specific science
    pub fn requires_science(&self, science: &str) -> bool {
        self.science_required.iter().any(|s| s == science)
    }

    /// Check if button is disabled by a specific science
    pub fn disabled_by_science(&self, science: &str) -> bool {
        self.science_disabled.iter().any(|s| s == science)
    }

    /// Check if button is disabled in a specific mode
    pub fn disabled_in_mode(&self, mode: &str) -> bool {
        self.disable_on_modes.iter().any(|m| m == mode)
    }
}

/// Control bar manager for handling command buttons
#[derive(Debug)]
pub struct ControlBar {
    command_buttons: HashMap<String, CommandButton>,
    button_order: Vec<String>,
    button_overrides: HashMap<String, Vec<CommandButton>>,
}

impl ControlBar {
    /// Create a new control bar
    pub fn new() -> Self {
        Self {
            command_buttons: HashMap::new(),
            button_order: Vec::new(),
            button_overrides: HashMap::new(),
        }
    }

    /// Find a command button by name (non-const version)
    pub fn find_non_const_command_button(&mut self, name: &str) -> Option<&mut CommandButton> {
        self.command_buttons.get_mut(name)
    }

    /// Find a command button by name
    pub fn find_command_button(&self, name: &str) -> Option<&CommandButton> {
        self.command_buttons.get(name)
    }

    /// Find a command button by name, resolving overrides to the final entry.
    pub fn find_command_button_resolved(&self, name: &str) -> Option<&CommandButton> {
        if let Some(overrides) = self.button_overrides.get(name) {
            if let Some(last) = overrides.last() {
                return Some(last);
            }
        }
        self.find_command_button(name)
    }

    /// Create a new command button
    pub fn new_command_button(&mut self, name: String) -> &mut CommandButton {
        let button = CommandButton::new(name.clone());
        if !self.command_buttons.contains_key(&name) {
            self.button_order.insert(0, name.clone());
        }
        self.command_buttons.insert(name.clone(), button);
        self.command_buttons.get_mut(&name).unwrap()
    }

    /// Create a new command button override
    pub fn new_command_button_override(
        &mut self,
        base_button: &CommandButton,
    ) -> &mut CommandButton {
        let mut override_button = base_button.clone();
        override_button.mark_as_override();

        let name = base_button.name.clone();
        let overrides = self
            .button_overrides
            .entry(name.clone())
            .or_insert_with(Vec::new);
        overrides.push(override_button);

        overrides.last_mut().unwrap()
    }

    /// Get all command button names
    pub fn get_button_names(&self) -> Vec<&String> {
        self.button_order.iter().collect()
    }

    /// Iterate over all stored command buttons.
    pub fn iter_buttons(&self) -> impl Iterator<Item = (&String, &CommandButton)> {
        self.button_order
            .iter()
            .filter_map(|name| self.command_buttons.get(name).map(|button| (name, button)))
    }

    /// Iterate over resolved command buttons, returning overrides when present.
    pub fn iter_resolved_buttons(&self) -> Vec<(&String, &CommandButton)> {
        self.button_order
            .iter()
            .filter_map(|name| {
                let base = self.command_buttons.get(name)?;
                let resolved = self.find_command_button_resolved(name).unwrap_or(base);
                Some((name, resolved))
            })
            .collect()
    }

    /// Get the number of command buttons
    pub fn count(&self) -> usize {
        self.command_buttons.len()
    }

    /// Clear all command buttons
    pub fn clear(&mut self) {
        self.command_buttons.clear();
        self.button_order.clear();
        self.button_overrides.clear();
    }

    /// Parse command button definition from INI
    pub fn parse_command_button_definition(ini: &mut INI) -> INIResult<()> {
        // Read the button name
        let name = match ini.get_next_value_token().or_else(|| ini.get_first_token()) {
            Some(token) => token,
            None => return Err(INIError::InvalidData),
        };

        initialize_control_bar();
        let mut control_bar = control_bar_write();

        let existing_button = control_bar.find_command_button(&name).cloned();

        let button = if let Some(existing_button) = existing_button {
            if ini.get_load_type() != INILoadType::CreateOverrides {
                eprintln!(
                    "Duplicate commandbutton {} found at line {} in '{}'",
                    name,
                    ini.get_line_num(),
                    ini.get_filename()
                );
                return Err(INIError::InvalidData);
            }

            control_bar.new_command_button_override(&existing_button)
        } else {
            let button = control_bar.new_command_button(name.to_string());
            if ini.get_load_type() == INILoadType::CreateOverrides {
                button.mark_as_override();
            }
            button
        };

        // Parse the button fields
        ini.init_from_ini_with_fields(button, CommandButton::get_field_parse())?;

        resolve_sciences(button);

        // Validate the button configuration
        button.validate()?;

        Ok(())
    }
}

impl Default for ControlBar {
    fn default() -> Self {
        Self::new()
    }
}

/// Global control bar instance (simulating TheControlBar)
static CONTROL_BAR: OnceCell<RwLock<ControlBar>> = OnceCell::new();

fn control_bar_cell() -> &'static RwLock<ControlBar> {
    CONTROL_BAR.get_or_init(|| RwLock::new(ControlBar::new()))
}

fn control_bar_read() -> RwLockReadGuard<'static, ControlBar> {
    control_bar_cell().read().expect("ControlBar poisoned")
}

fn control_bar_write() -> RwLockWriteGuard<'static, ControlBar> {
    control_bar_cell().write().expect("ControlBar poisoned")
}

/// Initialize the global control bar
pub fn initialize_control_bar() {
    let _ = control_bar_cell();
}

/// Get a reference to the global control bar
pub fn get_control_bar() -> Option<RwLockReadGuard<'static, ControlBar>> {
    Some(control_bar_read())
}

/// Get a mutable reference to the global control bar
pub fn get_control_bar_mut() -> Option<RwLockWriteGuard<'static, ControlBar>> {
    Some(control_bar_write())
}

/// Parse command button definition from INI file
/// This is the main entry point called by the INI parser
pub fn parse_command_button_definition(ini: &mut INI) -> INIResult<()> {
    ControlBar::parse_command_button_definition(ini)
}

fn resolve_sciences(button: &mut CommandButton) {
    button.parsed_science_required.clear();
    button.parsed_science_disabled.clear();
    button.sciences_ids.clear();

    if let Some(store) = crate::common::rts::get_science_store() {
        for science_name in &button.science_required {
            let science = store.get_science_from_internal_name(science_name);
            if science != SCIENCE_INVALID {
                button.parsed_science_required.push(science);
                if !button.sciences_ids.contains(&science) {
                    button.sciences_ids.push(science);
                }
            }
        }

        for science_name in &button.science_disabled {
            let science = store.get_science_from_internal_name(science_name);
            if science != SCIENCE_INVALID {
                button.parsed_science_disabled.push(science);
            }
        }
    }
}

fn first_non_equals<'a>(values: &'a [&'a str]) -> Option<&'a str> {
    values.iter().copied().find(|token| *token != "=")
}

fn assign_string_field(target: &mut String, values: &[&str]) -> INIResult<()> {
    if let Some(value) = first_non_equals(values) {
        *target = value.trim_matches('"').to_string();
        Ok(())
    } else {
        Err(INIError::InvalidData)
    }
}

fn parse_command_field_command(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.command, values)
}

fn parse_command_field_object(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.object, values)
}

fn parse_command_field_upgrade(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.upgrade, values)
}

fn parse_command_field_text_label(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.text_label, values)
}

fn parse_command_field_purchased_label(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.purchased_label, values)
}

fn parse_command_field_conflicting_label(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.conflicting_label, values)
}

fn parse_command_field_descript_label(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.descriptive_text, values)
}

fn parse_command_field_button_image(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.button_image, values)
}

fn parse_command_field_cursor_name(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.cursor_name, values)
}

fn parse_command_field_invalid_cursor(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    assign_string_field(&mut button.invalid_cursor_name, values)
}

fn parse_command_field_weapon_slot(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    let tokens: Vec<&str> = values.iter().copied().filter(|v| *v != "=").collect();
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let slot_value = INI::parse_lookup_list(token, WEAPON_SLOT_LOOKUP_LIST)?;
    button.weapon_slot = match slot_value {
        0 => WeaponSlotType::Primary,
        1 => WeaponSlotType::Secondary,
        2 => WeaponSlotType::Tertiary,
        _ => WeaponSlotType::Primary,
    };
    Ok(())
}

fn parse_command_field_max_shots_to_fire(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    let tokens: Vec<&str> = values.iter().copied().filter(|v| *v != "=").collect();
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    button.max_shots_to_fire = INI::parse_int(token)?;
    Ok(())
}

fn parse_command_field_button_border_type(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    let tokens: Vec<&str> = values.iter().copied().filter(|v| *v != "=").collect();
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let border_value = INI::parse_lookup_list(token, BUTTON_BORDER_LOOKUP_LIST)?;
    button.button_border_type = match border_value {
        0 => "NONE",
        1 => "BUILD",
        2 => "UPGRADE",
        3 => "ACTION",
        4 => "SYSTEM",
        _ => "SYSTEM",
    }
    .to_string();
    Ok(())
}

fn parse_command_field_radius_cursor_type(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    let tokens: Vec<&str> = values.iter().copied().filter(|v| *v != "=").collect();
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let index = INI::parse_index_list(token, RADIUS_CURSOR_NAMES)?;
    button.radius_cursor_type = RADIUS_CURSOR_NAMES[index].to_string();
    Ok(())
}

fn parse_command_field_unit_specific_sound(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    let tokens: Vec<&str> = values.iter().copied().filter(|v| *v != "=").collect();
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    button.unit_specific_sound = AudioEventRTS::from_sound_file((*token).to_string());
    Ok(())
}

fn parse_command_field_science(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    let entries: Vec<String> = values
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .map(|value| value.trim_matches('"').to_string())
        .collect();
    button.science_required = entries;
    Ok(())
}

fn parse_command_field_special_power(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    button.special_power_template = Some(
        first_non_equals(values)
            .ok_or(INIError::InvalidData)?
            .trim_matches('"')
            .to_string(),
    );
    Ok(())
}

fn parse_command_field_options(
    _ini: &mut INI,
    button: &mut CommandButton,
    values: &[&str],
) -> INIResult<()> {
    let filtered: Vec<&str> = values
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect();

    if filtered.is_empty() {
        button.options_bits = 0;
        button.options = CommandButtonOptions::default();
        return Ok(());
    }

    let mut bits = 0u32;
    let mut saw_direct = false;

    for raw in filtered {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut name = trimmed;
        let mut set = true;

        if let Some(rest) = trimmed.strip_prefix('+') {
            name = rest;
        } else if let Some(rest) = trimmed.strip_prefix('-') {
            name = rest;
            set = false;
        } else if !saw_direct {
            bits = 0;
            saw_direct = true;
        }

        let mut normalized = name.trim_matches('"');
        if normalized.eq_ignore_ascii_case("unused-reserved") {
            normalized = "NEED_TARGET_PRISONER";
        }

        if normalized.eq_ignore_ascii_case("---DO-NOT-USE---") {
            continue;
        }

        if let Some(index) = COMMAND_OPTION_NAMES
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(normalized))
        {
            let mask = 1u32 << index;
            if set {
                bits |= mask;
            } else {
                bits &= !mask;
            }
        } else {
            return Err(INIError::InvalidData);
        }
    }

    button.options_bits = bits;
    button.options = CommandButtonOptions::from_bits(bits);
    Ok(())
}

impl CommandButtonOptions {
    fn from_bits(bits: u32) -> Self {
        Self {
            need_special_power_science: has_command_option(bits, "NEED_SPECIAL_POWER_SCIENCE"),
            contextual_animates: has_command_option(bits, "CONTEXTMODE_COMMAND"),
            one_shot: has_command_option(bits, "SINGLE_USE_COMMAND"),
            scripted_only: has_command_option(bits, "SCRIPT_ONLY"),
            player_upgrade: false,
            cancel_all: false,
            not_queueable: has_command_option(bits, "NOT_QUEUEABLE"),
            ok_for_multi_select: has_command_option(bits, "OK_FOR_MULTI_SELECT"),
            check_like: has_command_option(bits, "CHECK_LIKE"),
            toggle_image_on_selection: false,
            must_be_stopped: has_command_option(bits, "MUST_BE_STOPPED"),
            cancel_like: false,
            need_upgrade: has_command_option(bits, "NEED_UPGRADE"),
            reverse_button_order: false,
        }
    }
}

fn has_command_option(bits: u32, name: &str) -> bool {
    COMMAND_OPTION_NAMES
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(name))
        .map(|index| (bits & (1u32 << index)) != 0)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_field(button: &mut CommandButton, token: &str, values: &[&str]) {
        let field = CommandButton::get_field_parse()
            .iter()
            .find(|field| field.token == token)
            .expect("field exists");
        let mut ini = INI::new();
        (field.parse)(&mut ini, button, values).expect("field parses");
    }

    #[test]
    fn test_command_button_creation() {
        let button = CommandButton::new("TestButton".to_string());
        assert_eq!(button.get_name(), "TestButton");
        assert_eq!(button.purchase_cost, 0);
        assert_eq!(button.upgrade_discount, 1.0);
        assert!(!button.is_override());
    }

    #[test]
    fn test_command_button_validation_valid() {
        let button = CommandButton::new("TestButton".to_string());
        assert!(button.validate().is_ok());
    }

    #[test]
    fn test_command_button_validation_special_power_mismatch() {
        let mut button = CommandButton::new("TestButton".to_string());
        button.special_power_template = Some("TestPower".to_string());
        // Missing need_special_power_science flag

        let result = button.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_command_button_validation_missing_special_power() {
        let mut button = CommandButton::new("TestButton".to_string());
        button.options.need_special_power_science = true;
        // Missing special power template

        let result = button.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_command_button_science_checks() {
        let mut button = CommandButton::new("TestButton".to_string());
        button
            .science_required
            .push("SCIENCE_ADVANCED_TRAINING".to_string());
        button
            .science_disabled
            .push("SCIENCE_FANATICISM".to_string());

        assert!(button.requires_science("SCIENCE_ADVANCED_TRAINING"));
        assert!(!button.requires_science("SCIENCE_OTHER"));

        assert!(button.disabled_by_science("SCIENCE_FANATICISM"));
        assert!(!button.disabled_by_science("SCIENCE_OTHER"));
    }

    #[test]
    fn test_control_bar() {
        let mut control_bar = ControlBar::new();
        assert_eq!(control_bar.count(), 0);

        // Add a button
        let button = control_bar.new_command_button("TestButton".to_string());
        button.purchase_cost = 100;

        assert_eq!(control_bar.count(), 1);

        // Find the button
        let found = control_bar.find_command_button("TestButton");
        assert!(found.is_some());
        assert_eq!(found.unwrap().purchase_cost, 100);

        // Try to find non-existent button
        let not_found = control_bar.find_command_button("NonExistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn control_bar_enumerates_command_buttons_in_cpp_list_order() {
        let mut control_bar = ControlBar::new();

        control_bar.new_command_button("FirstButton".to_string());
        control_bar.new_command_button("SecondButton".to_string());
        control_bar.new_command_button("ThirdButton".to_string());

        let names: Vec<&str> = control_bar
            .get_button_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names, vec!["ThirdButton", "SecondButton", "FirstButton"]);

        let first_button = control_bar
            .find_command_button("FirstButton")
            .unwrap()
            .clone();
        let first_override = control_bar.new_command_button_override(&first_button);
        first_override.purchase_cost = 123;

        let iter_names: Vec<&str> = control_bar
            .iter_buttons()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(
            iter_names,
            vec!["ThirdButton", "SecondButton", "FirstButton"]
        );

        let resolved_names: Vec<&str> = control_bar
            .iter_resolved_buttons()
            .into_iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(
            resolved_names,
            vec!["ThirdButton", "SecondButton", "FirstButton"]
        );
        assert_eq!(
            control_bar
                .find_command_button_resolved("FirstButton")
                .map(|button| button.purchase_cost),
            Some(123)
        );
    }

    #[test]
    fn test_command_button_override() {
        let mut control_bar = ControlBar::new();

        // Create base button
        let base_button = control_bar.new_command_button("TestButton".to_string());
        base_button.purchase_cost = 100;
        let base_button_copy = base_button.clone();

        // Create override
        let override_button = control_bar.new_command_button_override(&base_button_copy);
        assert!(override_button.is_override());
        assert_eq!(override_button.purchase_cost, 100); // Inherited from base
    }

    #[test]
    fn test_command_button_options_default() {
        let options = CommandButtonOptions::default();
        assert!(!options.need_special_power_science);
        assert!(!options.one_shot);
        assert!(!options.scripted_only);
        assert!(!options.cancel_all);
    }

    #[test]
    fn parses_special_power_field_with_ini_equals_token() {
        let mut button = CommandButton::new("Command_A10Strike".to_string());

        parse_field(
            &mut button,
            "SpecialPower",
            &["=", "SuperweaponA10ThunderboltMissileStrike"],
        );

        assert_eq!(
            button.get_special_power_template().map(String::as_str),
            Some("SuperweaponA10ThunderboltMissileStrike")
        );
    }

    #[test]
    fn parsed_options_keep_boolean_view_in_sync() {
        let mut button = CommandButton::new("Command_A10Strike".to_string());

        parse_field(
            &mut button,
            "Options",
            &[
                "=",
                "NEED_SPECIAL_POWER_SCIENCE",
                "OK_FOR_MULTI_SELECT",
                "SCRIPT_ONLY",
                "MUST_BE_STOPPED",
                "NOT_QUEUEABLE",
            ],
        );

        assert_ne!(button.options_bits, 0);
        assert!(button.options.need_special_power_science);
        assert!(button.options.ok_for_multi_select);
        assert!(button.options.scripted_only);
        assert!(button.options.must_be_stopped);
        assert!(button.options.not_queueable);
        assert!(!button.options.one_shot);
    }

    #[test]
    fn special_power_command_validates_after_field_parse() {
        let mut button = CommandButton::new("Command_A10Strike".to_string());

        parse_field(&mut button, "Options", &["=", "NEED_SPECIAL_POWER_SCIENCE"]);
        parse_field(
            &mut button,
            "SpecialPower",
            &["=", "SuperweaponA10ThunderboltMissileStrike"],
        );

        assert!(button.validate().is_ok());
    }
}
