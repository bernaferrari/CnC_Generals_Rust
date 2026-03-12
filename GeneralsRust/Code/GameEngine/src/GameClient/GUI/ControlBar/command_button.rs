// FILE: command_button.rs
// Port of CommandButton class from C++
// Original: ControlBar.h and ControlBar.cpp

use std::sync::Arc;
use crate::common::types::*;
use super::types::*;

/// Command Button Structure
/// Represents a button that can be assigned to the GUI context-sensitive interface
#[derive(Clone)]
pub struct CommandButton {
    /// Template name
    name: String,

    /// Type of command this button represents
    command: GUICommandType,

    /// Command options (see command option constants)
    options: u32,

    /// Thing template for commands that use thing templates
    thing_template: Option<Arc<ThingTemplate>>,

    /// Upgrade template for commands that use upgrade templates
    upgrade_template: Option<Arc<UpgradeTemplate>>,

    /// Special power template
    special_power: Option<Arc<SpecialPowerTemplate>>,

    /// Radius cursor type, if any
    radius_cursor: RadiusCursorType,

    /// Cursor name for placement (NEED_TARGET_POS) or valid version (CONTEXTMODE_COMMAND)
    cursor_name: String,

    /// Cursor name for invalid version
    invalid_cursor_name: String,

    /// String manager text label
    text_label: String,

    /// Description of the current command
    description_label: String,

    /// Description if command has already been purchased
    purchased_label: String,

    /// Description if command can't be selected due to mutually-exclusive choice
    conflicting_label: String,

    /// Weapon slot for commands that refer to a weapon slot
    weapon_slot: WeaponSlotType,

    /// Maximum shots to fire for fire weapon commands
    max_shots_to_fire: i32,

    /// Science requirements
    science: Vec<ScienceType>,

    /// Command button border type
    command_button_border: CommandButtonMappedBorderType,

    /// Button image name
    button_image_name: String,

    /// Window associated with this button (runtime)
    window: Option<Arc<GameWindow>>,

    /// Unit-specific sound played when button is clicked
    unit_specific_sound: AudioEventRTS,

    /// Button image (cached)
    button_image: Option<Arc<Image>>,

    /// Flash count for cameo flashing
    flash_count: i32,

    /// Next button in linked list
    next: Option<Box<CommandButton>>,
}

impl CommandButton {
    /// Create a new CommandButton with default values
    pub fn new() -> Self {
        Self {
            name: String::new(),
            command: GUICommandType::None,
            options: COMMAND_OPTION_NONE,
            thing_template: None,
            upgrade_template: None,
            special_power: None,
            radius_cursor: RadiusCursorType::None,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            text_label: String::new(),
            description_label: String::new(),
            purchased_label: String::new(),
            conflicting_label: String::new(),
            weapon_slot: WeaponSlotType::Primary,
            max_shots_to_fire: 0x7fffffff,
            science: Vec::new(),
            command_button_border: CommandButtonMappedBorderType::None,
            button_image_name: String::new(),
            window: None,
            unit_specific_sound: AudioEventRTS::default(),
            button_image: None,
            flash_count: 0,
            next: None,
        }
    }

    /// Check if this is a context sensitive command
    pub fn is_context_command(&self) -> bool {
        (self.options & CONTEXTMODE_COMMAND) != 0
    }

    /// Check if the given relationship is a valid target
    pub fn is_valid_relationship_target(&self, relationship: Relationship) -> bool {
        let mut mask = 0u32;

        match relationship {
            Relationship::Enemies => mask |= NEED_TARGET_ENEMY_OBJECT,
            Relationship::Allies => mask |= NEED_TARGET_ALLY_OBJECT,
            Relationship::Neutral => mask |= NEED_TARGET_NEUTRAL_OBJECT,
        }

        (self.options & mask) != 0
    }

    /// Check if target object is valid for the source player
    pub fn is_valid_object_target_for_player(
        &self,
        source_player: Option<&Player>,
        target_obj: Option<&Object>
    ) -> bool {
        match (source_player, target_obj) {
            (Some(player), Some(obj)) => {
                let relationship = player.get_relationship(obj.get_team());
                self.is_valid_relationship_target(relationship)
            },
            _ => false,
        }
    }

    /// Check if target object is valid for the source object
    pub fn is_valid_object_target_for_object(
        &self,
        source_obj: Option<&Object>,
        target_obj: Option<&Object>
    ) -> bool {
        match (source_obj, target_obj) {
            (Some(src), Some(tgt)) => {
                let relationship = src.get_relationship(tgt);
                self.is_valid_relationship_target(relationship)
            },
            _ => false,
        }
    }

    /// Check if target drawable is valid for the source drawable
    pub fn is_valid_object_target_for_drawable(
        &self,
        source: Option<&Drawable>,
        target: Option<&Drawable>
    ) -> bool {
        let source_obj = source.and_then(|d| d.get_object());
        let target_obj = target.and_then(|d| d.get_object());
        self.is_valid_object_target_for_object(source_obj, target_obj)
    }

    /// Check if command is valid to use on the given target
    pub fn is_valid_to_use_on(
        &self,
        source_obj: &Object,
        target_obj: Option<&Object>,
        target_location: Option<&Coord3D>,
        command_source: CommandSourceType
    ) -> bool {
        // Check upgrade template
        if let Some(upgrade_template) = &self.upgrade_template {
            if let Some(production_interface) = source_obj.get_production_update_interface() {
                // Check if there's already an upgrade in production
                let mut current_production = production_interface.first_production();
                while let Some(prod_entry) = current_production {
                    if prod_entry.get_production_upgrade().is_some() {
                        return false;
                    }
                    current_production = production_interface.next_production(prod_entry);
                }

                return source_obj.affected_by_upgrade(upgrade_template)
                    && !source_obj.has_upgrade(upgrade_template);
            }
            return false;
        }

        // Check if we need an object target but don't have one
        if (self.options & COMMAND_OPTION_NEED_OBJECT_TARGET) != 0 && target_obj.is_none() {
            return false;
        }

        // Get position for location-based targeting
        let mut pos = Coord3D::default();
        if let Some(loc) = target_location {
            pos = *loc;
        }

        // Check if we need a target position
        if (self.options & NEED_TARGET_POS) != 0 && target_location.is_none() {
            if let Some(obj) = target_obj {
                pos = obj.get_position();
            } else {
                return false;
            }
        }

        // Check special power validity based on targeting type
        if (self.options & COMMAND_OPTION_NEED_OBJECT_TARGET) != 0 {
            return ActionManager::can_do_special_power_at_object(
                source_obj,
                target_obj.unwrap(),
                command_source,
                self.special_power.as_ref(),
                self.options,
                false
            );
        }

        if (self.options & NEED_TARGET_POS) != 0 {
            return ActionManager::can_do_special_power_at_location(
                source_obj,
                &pos,
                command_source,
                self.special_power.as_ref(),
                None,
                self.options,
                false
            );
        }

        ActionManager::can_do_special_power(
            source_obj,
            self.special_power.as_ref(),
            command_source,
            self.options,
            false
        )
    }

    /// Check if command is ready to use
    pub fn is_ready(&self, source_obj: &Object) -> bool {
        // Check special power readiness
        if let Some(special_power) = &self.special_power {
            if let Some(module) = source_obj.get_special_power_module(special_power) {
                if module.get_percent_ready() == 1.0 {
                    return true;
                }
            }
        }

        // Check upgrade readiness
        if let Some(upgrade) = &self.upgrade_template {
            if source_obj.affected_by_upgrade(upgrade) && !source_obj.has_upgrade(upgrade) {
                return true;
            }
        }

        false
    }

    /// Copy images from another command button
    pub fn copy_images_from(&mut self, button: &CommandButton, mark_ui_dirty: bool) {
        if self.button_image != button.button_image {
            self.button_image = button.button_image.clone();

            if mark_ui_dirty {
                // Mark UI dirty
                if let Some(control_bar) = ControlBar::get_instance() {
                    control_bar.mark_ui_dirty();
                }
            }
        }
    }

    /// Copy button text from another command button
    pub fn copy_button_text_from(&mut self, button: &CommandButton, shortcut_button: bool, mark_ui_dirty: bool) {
        let mut changed = false;

        if shortcut_button {
            // For shortcut buttons, conflicting label is actually the shortcut label
            if !button.conflicting_label.is_empty() && self.text_label != button.conflicting_label {
                self.text_label = button.conflicting_label.clone();
                changed = true;
            }
        } else {
            // Copy text from purchase science button if it exists
            if !button.text_label.is_empty() && self.text_label != button.text_label {
                self.text_label = button.text_label.clone();
                changed = true;
            }
        }

        if !button.description_label.is_empty() && self.description_label != button.description_label {
            self.description_label = button.description_label.clone();
            changed = true;
        }

        if mark_ui_dirty && changed {
            if let Some(control_bar) = ControlBar::get_instance() {
                control_bar.mark_ui_dirty();
            }
        }
    }

    /// Cache the button image from the mapped image collection
    pub fn cache_button_image(&mut self) {
        if !self.button_image_name.is_empty() {
            self.button_image = MappedImageCollection::find_image_by_name(&self.button_image_name);
        }
    }

    // Getters
    pub fn get_name(&self) -> &str { &self.name }
    pub fn get_cursor_name(&self) -> &str { &self.cursor_name }
    pub fn get_invalid_cursor_name(&self) -> &str { &self.invalid_cursor_name }
    pub fn get_text_label(&self) -> &str { &self.text_label }
    pub fn get_description_label(&self) -> &str { &self.description_label }
    pub fn get_purchased_label(&self) -> &str { &self.purchased_label }
    pub fn get_conflicting_label(&self) -> &str { &self.conflicting_label }
    pub fn get_unit_specific_sound(&self) -> &AudioEventRTS { &self.unit_specific_sound }
    pub fn get_command_type(&self) -> GUICommandType { self.command }
    pub fn get_options(&self) -> u32 { self.options }
    pub fn get_thing_template(&self) -> Option<&Arc<ThingTemplate>> { self.thing_template.as_ref() }
    pub fn get_upgrade_template(&self) -> Option<&Arc<UpgradeTemplate>> { self.upgrade_template.as_ref() }
    pub fn get_special_power_template(&self) -> Option<&Arc<SpecialPowerTemplate>> { self.special_power.as_ref() }
    pub fn get_radius_cursor_type(&self) -> RadiusCursorType { self.radius_cursor }
    pub fn get_weapon_slot(&self) -> WeaponSlotType { self.weapon_slot }
    pub fn get_max_shots_to_fire(&self) -> i32 { self.max_shots_to_fire }
    pub fn get_science_vec(&self) -> &[ScienceType] { &self.science }
    pub fn get_command_button_mapped_border_type(&self) -> CommandButtonMappedBorderType { self.command_button_border }
    pub fn get_button_image(&self) -> Option<&Arc<Image>> { self.button_image.as_ref() }
    pub fn get_window(&self) -> Option<&Arc<GameWindow>> { self.window.as_ref() }
    pub fn get_flash_count(&self) -> i32 { self.flash_count }
    pub fn get_next(&self) -> Option<&CommandButton> { self.next.as_ref().map(|b| &**b) }

    // Setters
    pub fn set_name(&mut self, name: String) { self.name = name; }
    pub fn set_button_image(&mut self, image: Option<Arc<Image>>) { self.button_image = image; }
    pub fn set_flash_count(&mut self, count: i32) { self.flash_count = count; }

    // Friend methods for ControlBar
    pub fn friend_add_to_list(&mut self, list: &mut Option<Box<CommandButton>>) {
        self.next = list.take();
        *list = Some(Box::new(self.clone()));
    }

    pub fn friend_get_next_mut(&mut self) -> Option<&mut CommandButton> {
        self.next.as_mut().map(|b| &mut **b)
    }
}

impl Default for CommandButton {
    fn default() -> Self {
        Self::new()
    }
}

// Placeholder types - these would be defined in their respective modules
#[derive(Clone, Copy, Debug)]
pub enum RadiusCursorType {
    None,
    // Add other cursor types as needed
}

#[derive(Clone, Copy, Debug)]
pub enum WeaponSlotType {
    Primary,
    Secondary,
    Tertiary,
}

#[derive(Clone, Debug, Default)]
pub struct AudioEventRTS {
    // Audio event fields
}

// Placeholder structs for types that would be defined elsewhere
pub struct ThingTemplate;
pub struct UpgradeTemplate;
pub struct SpecialPowerTemplate;
pub struct GameWindow;
pub struct Image;
pub struct Player;
pub struct Object;
pub struct Drawable;
pub struct ControlBar;
pub struct MappedImageCollection;
pub struct ActionManager;

#[derive(Clone, Copy, Debug)]
pub enum Relationship {
    Enemies,
    Allies,
    Neutral,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Clone, Copy, Debug)]
pub enum CommandSourceType {
    Default,
    // Add other source types
}

pub type ScienceType = u32;

// These would normally be in their respective implementation files
impl Player {
    pub fn get_relationship(&self, _team: u32) -> Relationship {
        Relationship::Neutral
    }
}

impl Object {
    pub fn get_team(&self) -> u32 { 0 }
    pub fn get_relationship(&self, _other: &Object) -> Relationship { Relationship::Neutral }
    pub fn get_position(&self) -> Coord3D { Coord3D::default() }
    pub fn get_production_update_interface(&self) -> Option<&ProductionUpdateInterface> { None }
    pub fn affected_by_upgrade(&self, _upgrade: &UpgradeTemplate) -> bool { false }
    pub fn has_upgrade(&self, _upgrade: &UpgradeTemplate) -> bool { false }
    pub fn get_special_power_module(&self, _power: &SpecialPowerTemplate) -> Option<&SpecialPowerModule> { None }
}

impl Drawable {
    pub fn get_object(&self) -> Option<&Object> { None }
}

impl ControlBar {
    pub fn get_instance() -> Option<&'static ControlBar> { None }
    pub fn mark_ui_dirty(&self) {}
}

impl MappedImageCollection {
    pub fn find_image_by_name(_name: &str) -> Option<Arc<Image>> { None }
}

impl ActionManager {
    pub fn can_do_special_power_at_object(
        _source: &Object,
        _target: &Object,
        _command_source: CommandSourceType,
        _power: Option<&Arc<SpecialPowerTemplate>>,
        _options: u32,
        _check: bool
    ) -> bool { false }

    pub fn can_do_special_power_at_location(
        _source: &Object,
        _location: &Coord3D,
        _command_source: CommandSourceType,
        _power: Option<&Arc<SpecialPowerTemplate>>,
        _target: Option<&Object>,
        _options: u32,
        _check: bool
    ) -> bool { false }

    pub fn can_do_special_power(
        _source: &Object,
        _power: Option<&Arc<SpecialPowerTemplate>>,
        _command_source: CommandSourceType,
        _options: u32,
        _check: bool
    ) -> bool { false }
}

pub struct ProductionUpdateInterface;
pub struct ProductionEntry;
pub struct SpecialPowerModule;

impl ProductionUpdateInterface {
    pub fn first_production(&self) -> Option<&ProductionEntry> { None }
    pub fn next_production(&self, _entry: &ProductionEntry) -> Option<&ProductionEntry> { None }
}

impl ProductionEntry {
    pub fn get_production_upgrade(&self) -> Option<&UpgradeTemplate> { None }
}

impl SpecialPowerModule {
    pub fn get_percent_ready(&self) -> f32 { 0.0 }
}
