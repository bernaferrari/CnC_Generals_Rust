// CommandButtonHuntUpdate - Handles "Hunting" using a special power
// Author: John Ahlquist, Sept. 2002
// Ported to Rust
//
// If the unit is idle and the power is not active, it targets a new unit with the power.
// This is an update rather than an AI state because many special abilities use the AI
// to perform portions of the special ability.

use std::any::Any;
use std::cmp::Ordering;
use std::sync::{Arc, Mutex};

use crate::ai::THE_AI;
use crate::command_button::{CommandButton, CommandButtonId, MAX_COMMANDS_PER_SET};
use crate::commands::command::CommandType;
use crate::common::types::ControlBarInterface;
use crate::common::{
    AsciiString, Bool, CommandSourceType, KindOf, ObjectID, Real, Relationship, UnsignedInt,
    LOGICFRAMES_PER_SECOND,
};
use crate::control_bar::get_control_bar_bridge;
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BehaviorModuleInterface, SpecialAbilityUpdateExt,
    UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::get_special_power_store;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::xfer::XferMode;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

#[derive(Debug, Clone)]
pub struct CommandButtonHuntUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub scan_frames: UnsignedInt,
    pub scan_range: Real,
}

impl Default for CommandButtonHuntUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            scan_frames: LOGICFRAMES_PER_SECOND,
            scan_range: 9999.0,
        }
    }
}

impl CommandButtonHuntUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, COMMAND_BUTTON_HUNT_UPDATE_FIELDS)
    }
}

impl ModuleData for CommandButtonHuntUpdateModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

impl Snapshotable for CommandButtonHuntUpdateModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn first_value_token<'a>(tokens: &'a [&str]) -> Option<&'a str> {
    tokens.iter().copied().find(|token| *token != "=")
}

fn parse_duration_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    INI::parse_duration_unsigned_int(value)
}

fn parse_real(tokens: &[&str]) -> Result<Real, INIError> {
    let value = first_value_token(tokens).ok_or(INIError::InvalidData)?;
    value.parse::<Real>().map_err(|_| INIError::InvalidData)
}

const COMMAND_BUTTON_HUNT_UPDATE_FIELDS: &[FieldParse<CommandButtonHuntUpdateModuleData>] = &[
    FieldParse {
        token: "ScanRate",
        parse: |_, data, tokens| {
            data.scan_frames = parse_duration_frames(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "ScanRange",
        parse: |_, data, tokens| {
            data.scan_range = parse_real(tokens)?;
            Ok(())
        },
    },
];

#[derive(Debug)]
pub struct CommandButtonHuntUpdate {
    object_id: ObjectID,
    module_data: Arc<CommandButtonHuntUpdateModuleData>,
    command_button_name: String,
    command_button: Option<CommandButtonId>,
}

impl CommandButtonHuntUpdate {
    pub fn new(object_id: ObjectID, module_data: Arc<CommandButtonHuntUpdateModuleData>) -> Self {
        Self {
            object_id,
            module_data,
            command_button_name: String::new(),
            command_button: None,
        }
    }

    fn object_arc(&self) -> Option<Arc<std::sync::RwLock<crate::object::Object>>> {
        TheGameLogic::find_object_by_id(self.object_id)
    }

    pub fn on_object_created(&mut self) {
        // Matches C++ constructor: setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER)
        TheGameLogic::set_wake_frame(self.object_id, UpdateSleepTime::Forever);
    }

    pub fn set_command_button(&mut self, button_name: String) {
        self.command_button_name = button_name.clone();
        self.command_button = None;

        let Some(object_arc) = self.object_arc() else {
            return;
        };
        let Ok(object) = object_arc.read() else {
            return;
        };

        // Find the command button in the command set
        if let Some(control_bar) = get_control_bar_bridge() {
            if let Some(command_set) =
                control_bar.find_command_set_by_name(object.get_command_set_string())
            {
                for i in 0..MAX_COMMANDS_PER_SET {
                    if let Some(button) = command_set.get_command_button(i) {
                        if !button.get_name().is_empty() && button.get_name() == button_name {
                            self.command_button = Some(button.get_id());
                            break;
                        }
                    }
                }
            }
        }

        if self.command_button.is_some() {
            if let Some(ai) = object.get_ai_update_interface() {
                ai.ai_idle(CommandSourceType::FromAi);
            }
            let _ = self.update_simple();
            TheGameLogic::set_wake_frame(self.object_id, UpdateSleepTime::None);
        }
    }

    pub fn update(&mut self) -> UpdateSleepTime {
        let Some(object_arc) = self.object_arc() else {
            return UpdateSleepTime::Forever;
        };
        let Ok(object) = object_arc.read() else {
            return UpdateSleepTime::Forever;
        };

        let Some(ai) = object.get_ai_update_interface() else {
            return UpdateSleepTime::Forever;
        };

        let Some(button_id) = self.command_button else {
            return UpdateSleepTime::Forever;
        };

        // If a script or the player gave a command, quit hunting
        if ai.get_last_command_source() != CommandSourceType::FromAi {
            self.command_button = None;
            self.command_button_name.clear();
            return UpdateSleepTime::Forever;
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return UpdateSleepTime::Forever;
        };

        let Some(button_any) = control_bar.get_command_button(button_id) else {
            return UpdateSleepTime::Forever;
        };

        let Some(button) = button_any.downcast_ref::<CommandButton>() else {
            return UpdateSleepTime::Forever;
        };

        #[allow(unreachable_patterns)]
        match button.get_command_type() {
            CommandType::SpecialPower => self.hunt_special_power(ai),
            CommandType::SwitchWeapon | CommandType::FireWeapon => self.hunt_weapon(ai),
            CommandType::Enter
            | CommandType::HijackVehicle
            | CommandType::ConvertToCarBomb
            | CommandType::SabotageBuilding => self.hunt_enter(ai),
            _ => UpdateSleepTime::Forever,
        }
    }

    fn hunt_weapon(&self, ai: Arc<Mutex<dyn AIUpdateInterface>>) -> UpdateSleepTime {
        let Some(object_arc) = self.object_arc() else {
            return UpdateSleepTime::None;
        };
        let Ok(mut object) = object_arc.write() else {
            return UpdateSleepTime::None;
        };

        if ai.is_idle() {
            ai.ai_hunt(CommandSourceType::FromAi);
        }

        if let Some(button_id) = self.command_button {
            if let Some(control_bar) = get_control_bar_bridge() {
                if let Some(button_any) = control_bar.get_command_button(button_id) {
                    if let Some(button) = button_any.downcast_ref::<CommandButton>() {
                        let weapon_slot = button.get_weapon_slot();
                        object.set_weapon_lock(
                            weapon_slot,
                            crate::weapon::WeaponLockType::LockedTemporarily,
                        );
                    }
                }
            }
        }

        UpdateSleepTime::None
    }

    fn hunt_special_power(&self, ai: Arc<Mutex<dyn AIUpdateInterface>>) -> UpdateSleepTime {
        let Some(object_arc) = self.object_arc() else {
            return UpdateSleepTime::Forever;
        };
        let Ok(object) = object_arc.read() else {
            return UpdateSleepTime::Forever;
        };

        if !ai.is_idle() {
            return UpdateSleepTime::Frames(self.module_data.scan_frames);
        }

        let Some(button_id) = self.command_button else {
            return UpdateSleepTime::Forever;
        };

        let Some(control_bar) = get_control_bar_bridge() else {
            return UpdateSleepTime::Forever;
        };

        let Some(button_any) = control_bar.get_command_button(button_id) else {
            return UpdateSleepTime::Forever;
        };

        let Some(button) = button_any.downcast_ref::<CommandButton>() else {
            return UpdateSleepTime::Forever;
        };

        if let Some(sp_template) = button.get_special_power_template() {
            let power_type_value = sp_template.get_special_power_type() as u32;
            let Some(common_power_type) =
                crate::common::types::SpecialPowerType::from_u32(power_type_value)
            else {
                return UpdateSleepTime::Forever;
            };
            let Some(sp_update) = object.find_special_ability_update(common_power_type) else {
                return UpdateSleepTime::Forever;
            };
            if sp_update.is_active() {
                return UpdateSleepTime::Frames(self.module_data.scan_frames);
            }
        }

        // Periodic scanning (expensive)
        if let Some(victim_id) = self.scan_closest_target() {
            if let Some(victim) = TheGameLogic::find_object_by_id(victim_id) {
                if let Ok(victim_guard) = victim.read() {
                    if let Err(err) = object.do_command_button_at_object(
                        button_id,
                        &victim_guard,
                        CommandSourceType::FromAi,
                    ) {
                        log::debug!(
                            "CommandButtonHuntUpdate::idle_enter do_command_button_at_object failed: {}",
                            err
                        );
                    }
                }
            }
        }

        UpdateSleepTime::Frames(self.module_data.scan_frames)
    }

    fn hunt_enter(&self, ai: Arc<Mutex<dyn AIUpdateInterface>>) -> UpdateSleepTime {
        let Some(object_arc) = self.object_arc() else {
            return UpdateSleepTime::Forever;
        };
        let Ok(object) = object_arc.read() else {
            return UpdateSleepTime::Forever;
        };

        if !ai.is_idle() {
            return UpdateSleepTime::Frames(self.module_data.scan_frames);
        }

        // Periodic scanning (expensive)
        if let Some(victim_id) = self.scan_closest_target() {
            if let Some(button_id) = self.command_button {
                if let Some(victim) = TheGameLogic::find_object_by_id(victim_id) {
                    if let Ok(victim_guard) = victim.read() {
                        if let Err(err) = object.do_command_button_at_object(
                            button_id,
                            &victim_guard,
                            CommandSourceType::FromAi,
                        ) {
                            log::debug!(
                                "CommandButtonHuntUpdate::hunt_enter do_command_button_at_object failed: {}",
                                err
                            );
                        }
                    }
                }
            }
        }

        UpdateSleepTime::Frames(self.module_data.scan_frames)
    }

    fn scan_closest_target(&self) -> Option<ObjectID> {
        let Some(object_arc) = self.object_arc() else {
            return None;
        };
        let Ok(object) = object_arc.read() else {
            return None;
        };

        let Some(button_id) = self.command_button else {
            return None;
        };

        let Some(control_bar) = get_control_bar_bridge() else {
            return None;
        };

        let Some(button_any) = control_bar.get_command_button(button_id) else {
            return None;
        };

        let Some(button) = button_any.downcast_ref::<CommandButton>() else {
            return None;
        };

        let command_type = button.get_command_type();

        let mut allow_neutral_only = false;
        let mut is_enter = false;
        #[allow(unreachable_patterns)]
        match command_type {
            CommandType::ConvertToCarBomb => {
                allow_neutral_only = true;
                is_enter = true;
            }
            CommandType::Enter | CommandType::HijackVehicle | CommandType::SabotageBuilding => {
                is_enter = true;
            }
            _ => {}
        }

        let mut is_black_lotus_vehicle_hack = false;
        let mut is_capture_building = false;
        let mut is_place_explosive = false;
        let mut sp_template = None;

        if !is_enter {
            sp_template = button.get_special_power_template().cloned();
            let Some(template) = sp_template.as_ref() else {
                return None;
            };
            if object.get_special_power_module(template.get_id()).is_none() {
                return None;
            }
            let sp_type = template.get_special_power_type();
            is_black_lotus_vehicle_hack = sp_type
                == crate::object::special_power_types::SpecialPowerType::BlackLotusDisableVehicleHack;
            is_capture_building = sp_type
                == crate::object::special_power_types::SpecialPowerType::InfantryCaptureBuilding;
            if is_capture_building {
                if sp_type == crate::object::special_power_types::SpecialPowerType::TimedCharges {
                    is_place_explosive = true;
                }
                if sp_type
                    == crate::object::special_power_types::SpecialPowerType::TankHunterTntAttack
                {
                    is_place_explosive = true;
                }
            }
        }

        let me_pos = *object.get_position();
        let scan_range = self.module_data.scan_range;
        let mut best_target = None;
        let mut best_priority: i32 = 0;
        let mut best_raw_priority: i32 = 0;
        let attack_priority_distance_modifier = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|data| data.attack_priority_distance_modifier)
            })
            .unwrap_or(0.0);

        let Some(partition) = ThePartitionManager::get() else {
            return None;
        };

        let mut candidates: Vec<(ObjectID, Real)> = partition
            .get_objects_in_range(&me_pos, scan_range)
            .into_iter()
            .filter_map(|id| {
                let other_arc = OBJECT_REGISTRY.get_object(id)?;
                let other = other_arc.read().ok()?;
                let dist_sqr = ThePartitionManager::get_distance_squared(
                    &object,
                    &other,
                    crate::common::FROM_BOUNDING_SPHERE_2D,
                );
                Some((id, dist_sqr))
            })
            .collect();

        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));

        for (id, dist_sqr) in candidates {
            let Some(other_arc) = OBJECT_REGISTRY.get_object(id) else {
                continue;
            };
            let Ok(other) = other_arc.read() else {
                continue;
            };

            if other.is_effectively_dead() {
                continue;
            }

            if other.is_off_map() != object.is_off_map() {
                continue;
            }

            if other.is_stealthed() && !other.is_detected() {
                continue;
            }

            let relationship = object.relationship_to(&other);
            if allow_neutral_only {
                if relationship != Relationship::Neutral {
                    continue;
                }
            } else if is_capture_building {
                let object_player_index = object
                    .get_controlling_player()
                    .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()));
                let other_player_index = other
                    .get_controlling_player()
                    .and_then(|player| player.read().ok().map(|guard| guard.get_player_index()));
                if object.get_controlling_player().is_some()
                    && object_player_index == other_player_index
                {
                    continue;
                }
                if matches!(relationship, Relationship::Ally | Relationship::Allies) {
                    continue;
                }
            } else if relationship != Relationship::Enemy {
                continue;
            }

            if is_black_lotus_vehicle_hack && other.is_disabled() {
                continue;
            }

            if is_enter {
                #[allow(unreachable_patterns)]
                let valid = match command_type {
                    CommandType::HijackVehicle => other.is_kind_of(KindOf::Vehicle),
                    CommandType::SabotageBuilding => other.is_kind_of(KindOf::Structure),
                    CommandType::ConvertToCarBomb => other.is_kind_of(KindOf::Vehicle),
                    _ => false,
                };
                if valid {
                    return Some(id);
                }
                continue;
            }

            if let Some(template) = sp_template.as_ref() {
                if let Some(store) = get_special_power_store() {
                    if !store.can_use_special_power(object.get_id(), template) {
                        continue;
                    }
                }

                if is_place_explosive {
                    let range = template.get_view_object_range();
                    if let Some(partition) = ThePartitionManager::get() {
                        let mut found_mine = false;
                        for mine_id in
                            partition.get_objects_in_range_boundary_2d(other.get_position(), range)
                        {
                            let Some(mine_arc) = OBJECT_REGISTRY.get_object(mine_id) else {
                                continue;
                            };
                            let Ok(mine) = mine_arc.read() else {
                                continue;
                            };
                            if !mine.is_kind_of(KindOf::Mine) {
                                continue;
                            }
                            let mine_player_index =
                                mine.get_controlling_player().and_then(|player| {
                                    player.read().ok().map(|guard| guard.get_player_index())
                                });
                            let object_player_index =
                                object.get_controlling_player().and_then(|player| {
                                    player.read().ok().map(|guard| guard.get_player_index())
                                });
                            let same_player = mine.get_controlling_player().is_some()
                                && mine_player_index == object_player_index;
                            if same_player {
                                found_mine = true;
                                break;
                            }
                        }
                        if found_mine {
                            continue;
                        }
                    }
                }
            }

            let dist = dist_sqr.sqrt();
            let raw_priority = (scan_range - dist) as i32;
            if raw_priority <= 0 {
                continue;
            }
            let modifier = if attack_priority_distance_modifier > 0.0 {
                (dist / attack_priority_distance_modifier) as i32
            } else {
                0
            };
            let mut effective_priority = raw_priority - modifier;
            if effective_priority < 1 {
                effective_priority = 1;
            }

            if effective_priority > best_priority
                || (effective_priority == best_priority && raw_priority > best_raw_priority)
            {
                best_priority = effective_priority;
                best_raw_priority = raw_priority;
                best_target = Some(id);
            }
        }

        best_target
    }
}

impl Snapshotable for CommandButtonHuntUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_ascii_string(&mut self.command_button_name)
            .map_err(|e| format!("Failed to xfer command button name: {:?}", e))?;

        if xfer.get_xfer_mode() == XferMode::Load {
            self.command_button = None;
            if !self.command_button_name.is_empty() {
                let Some(object_arc) = self.object_arc() else {
                    return Ok(());
                };
                let Ok(object) = object_arc.read() else {
                    return Ok(());
                };
                let Some(control_bar) = get_control_bar_bridge() else {
                    return Ok(());
                };
                if let Some(command_set) =
                    control_bar.find_command_set_by_name(object.get_command_set_string())
                {
                    for i in 0..MAX_COMMANDS_PER_SET {
                        if let Some(button) = command_set.get_command_button(i) {
                            if !button.get_name().is_empty()
                                && button.get_name() == self.command_button_name
                            {
                                self.command_button = Some(button.get_id());
                                break;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpdateModuleInterface for CommandButtonHuntUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        self.update()
    }
}

impl BehaviorModuleInterface for CommandButtonHuntUpdate {
    fn get_module_name(&self) -> &'static str {
        "CommandButtonHuntUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.on_object_created();
        Ok(())
    }
}

/// Glue that exposes CommandButtonHuntUpdate through the common Module trait.
pub struct CommandButtonHuntUpdateModule {
    behavior: CommandButtonHuntUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<CommandButtonHuntUpdateModuleData>,
}

impl CommandButtonHuntUpdateModule {
    pub fn new(
        behavior: CommandButtonHuntUpdate,
        module_name: &AsciiString,
        module_data: Arc<CommandButtonHuntUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut CommandButtonHuntUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for CommandButtonHuntUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for CommandButtonHuntUpdateModule {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_frames_accepts_duration_suffixes() {
        assert_eq!(parse_duration_frames(&["1500ms"]).expect("duration"), 45);
        assert_eq!(parse_duration_frames(&["1.5s"]).expect("duration"), 45);
    }

    #[test]
    fn parse_duration_frames_ignores_equals_token() {
        assert_eq!(parse_duration_frames(&["=", "1.5s"]).expect("duration"), 45);
    }
}
