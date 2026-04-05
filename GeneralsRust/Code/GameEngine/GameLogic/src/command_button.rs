//! Command button system – partial port of the legacy WW3D/Generals command button model.
//!
//! The original C++ implementation couples `CommandButton` to a substantial amount of UI and
//! scripting infrastructure.  For now we focus on the data that gameplay systems require while
//! providing helpers that mirror the science gating behaviour used by the control bar.

use crate::action_manager::TheActionManager;
use crate::common::*;
use crate::helpers::TheThingFactory;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::update::special_power_update::SpecialPowerCommandOption;
use crate::upgrade::center::with_upgrade_center;
use crate::upgrade::UpgradeTemplate;
use game_engine::common::ini::ini_command_button as common_buttons;
use game_engine::common::rts::{
    get_science_store, ScienceAccess, ScienceStore, ScienceType, SCIENCE_INVALID,
};
use std::sync::Arc;

/// Command button identifier (matches C++ `CommandButtonID`)
pub type CommandButtonId = u32;

/// Trait describing the bits of player state required to evaluate science gating.
///
/// The legacy C++ code queries `Player::hasScience`, `isScienceDisabled`, `isScienceHidden`, and
/// the player's current science purchase points.  We abstract those requirements behind this trait
/// so call-sites can work with either the Rust `Player` type or test doubles.
pub trait SciencePlayerAccess: ScienceAccess {
    /// Returns true when the specified science has been explicitly disabled via scripting.
    fn is_science_disabled(&self, science: ScienceType) -> bool;
    /// Returns true when the specified science should be hidden from UI.
    fn is_science_hidden(&self, science: ScienceType) -> bool;
    /// Returns the number of remaining purchase points available to the player.
    fn science_purchase_points(&self) -> i32;
}

/// Outcome of evaluating a command button's science requirements for a specific player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScienceRequirementStatus {
    pub science: ScienceType,
    pub owned: bool,
    pub disabled: bool,
    pub hidden: bool,
    pub prereqs_met: bool,
    pub root_prereqs_met: bool,
    pub purchase_points: i32,
    pub cost: i32,
}

impl ScienceRequirementStatus {
    /// Returns true when the science should be completely hidden from the player.
    pub fn should_be_hidden(&self) -> bool {
        self.hidden || !self.root_prereqs_met
    }

    /// Returns true when the button should be disabled (greyed out) for the player.
    pub fn should_be_disabled(&self) -> bool {
        self.disabled
            || self.should_be_hidden()
            || self.owned
            || !self.prereqs_met
            || self.cost <= 0
            || self.cost > self.purchase_points
    }

    /// Returns true when the player is eligible to purchase this science right now.
    pub fn can_purchase(&self) -> bool {
        !self.should_be_hidden()
            && !self.disabled
            && !self.owned
            && self.prereqs_met
            && self.cost > 0
            && self.cost <= self.purchase_points
    }
}

/// Maximum commands per command set
pub const MAX_COMMANDS_PER_SET: usize = 18;

/// Core command button data.
#[derive(Debug, Clone)]
pub struct CommandButton {
    pub id: CommandButtonId,
    pub name: String,
    pub tooltip: String,
    pub cost: Money,
    command_type: crate::commands::command::CommandType,
    weapon_slot: crate::weapon::WeaponSlotType,
    max_shots_to_fire: i32,
    special_power_template:
        Option<Arc<crate::object::special_power_template::SpecialPowerTemplate>>,
    thing_template: Option<Arc<dyn crate::common::ThingTemplate>>,
    upgrade_template: Option<Arc<UpgradeTemplate>>,
    options_bits: u32,
    required_sciences: Vec<ScienceType>,
}

/// Command set - collection of command buttons
/// Stub implementation for compilation
#[derive(Debug, Clone)]
pub struct CommandSet {
    pub name: String,
    pub buttons: Vec<Option<CommandButton>>,
}

impl CommandSet {
    /// Get command button at index
    /// Stub implementation for compilation
    pub fn get_command_button(&self, index: usize) -> Option<&CommandButton> {
        if index < self.buttons.len() {
            self.buttons[index].as_ref()
        } else {
            None
        }
    }
}

impl CommandButton {
    pub fn new(id: CommandButtonId, name: String, tooltip: String, cost: Money) -> Self {
        Self {
            id,
            name,
            tooltip,
            cost,
            command_type: crate::commands::command::CommandType::Invalid,
            weapon_slot: crate::weapon::WeaponSlotType::Primary,
            max_shots_to_fire: i32::MAX,
            special_power_template: None,
            thing_template: None,
            upgrade_template: None,
            options_bits: 0,
            required_sciences: Vec::new(),
        }
    }

    pub fn from_common(id: CommandButtonId, button: &common_buttons::CommandButton) -> Self {
        let tooltip = if !button.descriptive_text.is_empty() {
            button.descriptive_text.clone()
        } else {
            button.text_label.clone()
        };

        let command_type = map_gui_command_to_command_type(&button.command);
        let weapon_slot = match button.weapon_slot {
            game_engine::common::rts::WeaponSlotType::Primary => {
                crate::weapon::WeaponSlotType::Primary
            }
            game_engine::common::rts::WeaponSlotType::Secondary => {
                crate::weapon::WeaponSlotType::Secondary
            }
            game_engine::common::rts::WeaponSlotType::Tertiary => {
                crate::weapon::WeaponSlotType::Tertiary
            }
        };

        let special_power_template = button
            .special_power_template
            .as_ref()
            .map(|name| find_or_create_special_power_template(&AsciiString::from(name.as_str())));
        let thing_template = if button.object.is_empty() {
            None
        } else {
            TheThingFactory::find_template(button.object.as_str())
        };
        let upgrade_template = if button.upgrade.is_empty() {
            None
        } else {
            with_upgrade_center(|center| center.find_upgrade(button.upgrade.as_str()))
        };

        let mut out = Self::new(id, button.name.clone(), tooltip, button.purchase_cost);
        out.command_type = command_type;
        out.weapon_slot = weapon_slot;
        out.max_shots_to_fire = button.max_shots_to_fire;
        out.special_power_template = special_power_template;
        out.thing_template = thing_template;
        out.upgrade_template = upgrade_template;
        out.options_bits = button.options_bits;
        out.required_sciences = button.parsed_science_required.clone();
        out
    }

    /// Adds a science requirement to this command button.
    pub fn add_science_requirement(&mut self, science: ScienceType) {
        if science != SCIENCE_INVALID && !self.required_sciences.contains(&science) {
            self.required_sciences.push(science);
        }
    }

    /// Returns an immutable view of the sciences attached to this button.
    pub fn science_vec(&self) -> &[ScienceType] {
        &self.required_sciences
    }

    /// Get the button's name
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get the button's ID
    pub fn get_id(&self) -> CommandButtonId {
        self.id
    }

    /// Get the command type.
    pub fn get_command_type(&self) -> crate::commands::command::CommandType {
        self.command_type
    }

    /// Get the weapon slot.
    pub fn get_weapon_slot(&self) -> crate::weapon::WeaponSlotType {
        self.weapon_slot
    }

    /// Get the special power template for the command, if any.
    pub fn get_special_power_template(
        &self,
    ) -> Option<&std::sync::Arc<crate::object::special_power_template::SpecialPowerTemplate>> {
        self.special_power_template.as_ref()
    }

    /// Get the thing template for unit/build commands.
    pub fn get_thing_template(&self) -> Option<&Arc<dyn crate::common::ThingTemplate>> {
        self.thing_template.as_ref()
    }

    /// Get the upgrade template for upgrade commands.
    pub fn get_upgrade_template(&self) -> Option<&Arc<UpgradeTemplate>> {
        self.upgrade_template.as_ref()
    }

    /// Get the maximum number of shots to fire when a fire-weapon button is used.
    pub fn get_max_shots_to_fire(&self) -> i32 {
        self.max_shots_to_fire
    }

    /// Get raw command option bits parsed from INI.
    pub fn get_options_bits(&self) -> u32 {
        self.options_bits
    }

    /// Determines whether this command button is a context-sensitive command.
    pub fn is_context_command(&self) -> bool {
        SpecialPowerCommandOption::from_bits_truncate(self.options_bits)
            .contains(SpecialPowerCommandOption::CONTEXTMODE_COMMAND)
    }

    /// Check relationship gating bits against a relationship.
    pub fn is_valid_relationship_target(&self, relationship: Relationship) -> bool {
        let options = SpecialPowerCommandOption::from_bits_truncate(self.options_bits);
        match relationship {
            Relationship::Enemies => {
                options.contains(SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT)
            }
            Relationship::Allies | Relationship::Allies => {
                options.contains(SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
            }
            Relationship::Neutral => {
                options.contains(SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT)
            }
            Relationship::Allies => {
                options.contains(SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT)
            }
        }
    }

    /// Validate target object using source player relationship.
    pub fn is_valid_object_target_for_player(
        &self,
        source_player: &crate::player::Player,
        target_obj: &crate::object::Object,
    ) -> bool {
        let Some(team) = target_obj.get_team() else {
            return false;
        };
        let Ok(team_guard) = team.read() else {
            return false;
        };
        let relationship = source_player.get_relationship_with_team(&team_guard);
        self.is_valid_relationship_target(relationship)
    }

    /// Validate target object using source object relationship.
    pub fn is_valid_object_target_for_object(
        &self,
        source_obj: &crate::object::Object,
        target_obj: &crate::object::Object,
    ) -> bool {
        let relationship = source_obj.relationship_to(target_obj);
        self.is_valid_relationship_target(relationship)
    }

    /// Validates whether the command can be used on a target object/location.
    /// Mirrors C++ CommandButton::isValidToUseOn.
    pub fn is_valid_to_use_on(
        &self,
        source_obj: &crate::object::Object,
        target_obj: Option<&crate::object::Object>,
        target_location: Option<&Coord3D>,
        command_source: crate::common::CommandSourceType,
    ) -> bool {
        if let Some(upgrade) = self.upgrade_template.as_ref() {
            if !source_obj.can_produce_upgrade(upgrade) {
                return false;
            }
            if has_upgrade_in_production_queue(source_obj) {
                return false;
            }
            return source_obj.affected_by_upgrade(upgrade) && !source_obj.has_upgrade(upgrade);
        }

        let options = SpecialPowerCommandOption::from_bits_truncate(self.options_bits);
        let needs_object_target = options.intersects(
            SpecialPowerCommandOption::NEED_TARGET_ENEMY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_NEUTRAL_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_ALLY_OBJECT
                | SpecialPowerCommandOption::NEED_TARGET_PRISONER,
        );
        if needs_object_target && target_obj.is_none() {
            return false;
        }

        let mut pos = Coord3D::new(0.0, 0.0, 0.0);
        let mut has_pos = false;
        if let Some(loc) = target_location {
            pos = *loc;
            has_pos = true;
        }

        if options.contains(SpecialPowerCommandOption::NEED_TARGET_POS) && !has_pos {
            if let Some(obj) = target_obj {
                pos = *obj.get_position();
                has_pos = true;
            } else {
                return false;
            }
        }

        let Some(sp_template) = self.special_power_template.as_ref() else {
            return false;
        };

        if needs_object_target {
            let Some(target) = target_obj else {
                return false;
            };
            return TheActionManager::can_do_special_power_at_object(
                source_obj,
                target,
                command_source,
                sp_template,
                self.options_bits,
                false,
            );
        }

        if options.contains(SpecialPowerCommandOption::NEED_TARGET_POS) && has_pos {
            return TheActionManager::can_do_special_power_at_location(
                source_obj,
                &pos,
                command_source,
                sp_template,
                None,
                self.options_bits,
                false,
            );
        }

        TheActionManager::can_do_special_power(
            source_obj,
            sp_template,
            command_source,
            self.options_bits,
            false,
        )
    }

    /// Returns true when a command button is ready for use.
    pub fn is_ready(&self, source_obj: &crate::object::Object) -> bool {
        if let Some(sp_template) = self.special_power_template.as_ref() {
            let name = sp_template.get_name();
            if let Some(ready) = source_obj
                .with_special_power_module_interface_by_name(name, |module| {
                    module.get_percent_ready() >= 1.0
                })
            {
                if ready {
                    return true;
                }
            }
        }

        if let Some(upgrade) = self.upgrade_template.as_ref() {
            if source_obj.affected_by_upgrade(upgrade) && !source_obj.has_upgrade(upgrade) {
                return true;
            }
        }

        false
    }

    /// Evaluates the user's science state using the global science store.
    ///
    /// Returns `None` when the button does not reference any sciences or when no global science
    /// store has been initialised yet.
    pub fn evaluate_science_requirement<P>(&self, player: &P) -> Option<ScienceRequirementStatus>
    where
        P: SciencePlayerAccess,
    {
        let store_guard = get_science_store()?;
        self.evaluate_science_requirement_with_store(&store_guard, player)
    }

    /// Evaluates science gating with an explicit `ScienceStore` reference.  This is useful for
    /// tests or tooling that wish to avoid touching the global singleton.
    pub fn evaluate_science_requirement_with_store<P>(
        &self,
        store: &ScienceStore,
        player: &P,
    ) -> Option<ScienceRequirementStatus>
    where
        P: SciencePlayerAccess,
    {
        let science = *self.required_sciences.first().unwrap_or(&SCIENCE_INVALID);
        if science == SCIENCE_INVALID {
            return None;
        }

        let owned = player.has_science(science);
        let hidden = player.is_science_hidden(science);
        let disabled = player.is_science_disabled(science);
        let prereqs_met = store.player_has_prereqs_for_science(player, science);
        let root_prereqs_met = store.player_has_root_prereqs_for_science(player, science);
        let cost = store.get_science_purchase_cost(science);

        Some(ScienceRequirementStatus {
            science,
            owned,
            disabled,
            hidden,
            prereqs_met,
            root_prereqs_met,
            purchase_points: player.science_purchase_points(),
            cost,
        })
    }
}

impl SciencePlayerAccess for crate::player::Player {
    fn is_science_disabled(&self, science: ScienceType) -> bool {
        self.is_science_disabled(science)
    }

    fn is_science_hidden(&self, science: ScienceType) -> bool {
        self.is_science_hidden(science)
    }

    fn science_purchase_points(&self) -> i32 {
        self.get_science_purchase_points()
    }
}

fn has_upgrade_in_production_queue(obj: &crate::object::Object) -> bool {
    for entry in obj.behavior_modules() {
        let found = entry
            .with_module_downcast::<
                crate::object::production::production_update_complete::ProductionUpdateCompleteModule,
                _,
                _,
            >(|prod| prod.behavior().has_any_upgrade_in_queue())
            .unwrap_or(false);
        if found {
            return true;
        }
    }

    false
}

pub fn map_gui_command_to_command_type(command: &str) -> crate::commands::command::CommandType {
    use crate::commands::command::CommandType;

    match command.trim().to_ascii_uppercase().as_str() {
        "NONE" => CommandType::Invalid,
        "DOZER_CONSTRUCT" => CommandType::DozerConstruct,
        "DOZER_CONSTRUCT_CANCEL" => CommandType::DozerCancelConstruct,
        "UNIT_BUILD" => CommandType::QueueUnitCreate,
        "CANCEL_UNIT_BUILD" => CommandType::CancelUnitCreate,
        "PLAYER_UPGRADE" | "OBJECT_UPGRADE" => CommandType::QueueUpgrade,
        "CANCEL_UPGRADE" => CommandType::CancelUpgrade,
        "ATTACK_MOVE" => CommandType::DoAttackMoveTo,
        "GUARD" | "GUARD_WITHOUT_PURSUIT" | "GUARD_FLYING_UNITS_ONLY" => {
            CommandType::DoGuardPosition
        }
        "STOP" => CommandType::DoStop,
        "WAYPOINTS" => CommandType::AddWaypoint,
        "EXIT_CONTAINER" => CommandType::Exit,
        "EVACUATE" => CommandType::Evacuate,
        "EXECUTE_RAILED_TRANSPORT" => CommandType::ExecuteRailedTransport,
        "BEACON_DELETE" => CommandType::RemoveBeacon,
        "SET_RALLY_POINT" => CommandType::SetRallyPoint,
        "SELL" => CommandType::Sell,
        "FIRE_WEAPON" => CommandType::FireWeapon,
        "SPECIAL_POWER"
        | "SPECIAL_POWER_FROM_SHORTCUT"
        | "SPECIAL_POWER_CONSTRUCT"
        | "SPECIAL_POWER_CONSTRUCT_FROM_SHORTCUT" => CommandType::SpecialPower,
        "PURCHASE_SCIENCE" => CommandType::PurchaseScience,
        "HACK_INTERNET" => CommandType::InternetHack,
        "TOGGLE_OVERCHARGE" => CommandType::ToggleOvercharge,
        "COMBATDROP" => CommandType::CombatDropAtLocation,
        "SWITCH_WEAPON" => CommandType::SwitchWeapons,
        "HIJACK_VEHICLE" => CommandType::Enter,
        "CONVERT_TO_CARBOMB" => CommandType::ConvertToCarBomb,
        "SABOTAGE_BUILDING" => CommandType::Enter,
        "PLACE_BEACON" => CommandType::PlaceBeacon,
        "SELECT_ALL_UNITS_OF_TYPE" => CommandType::MetaSelectMatchingUnits,
        _ => CommandType::Invalid,
    }
}
