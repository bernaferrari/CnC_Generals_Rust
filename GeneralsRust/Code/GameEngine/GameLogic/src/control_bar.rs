use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard};

use crate::command_button::{CommandButton, CommandButtonId, CommandSet, MAX_COMMANDS_PER_SET};
use crate::common::types::ControlBarInterface;
use crate::common::KindOf;
use crate::object_manager::get_object_manager;
use crate::player::player_list;
use game_engine::common::ini::ini_command_button::get_control_bar;
use game_engine::common::ini::ini_command_set::get_command_set_manager;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::{CommandSetHandle, PlayerHandle, ThingTemplateHandle};
use game_engine::rts::academy_stats::{
    set_academy_template_context_provider, AcademyTemplateContext,
};
use once_cell::sync::OnceCell;
use std::sync::Arc;

#[derive(Debug)]
pub struct ControlBarBridge {
    buttons_by_id: HashMap<CommandButtonId, CommandButton>,
    command_sets: HashMap<String, CommandSet>,
}

impl ControlBarBridge {
    pub fn build_from_common() -> Result<Self, String> {
        let control_bar = get_control_bar().ok_or("ControlBar not initialized")?;
        let mut button_names: Vec<String> = control_bar
            .get_button_names()
            .into_iter()
            .cloned()
            .collect();
        button_names.sort();

        let mut buttons_by_id = HashMap::new();
        let mut name_to_id = HashMap::new();
        let mut next_id: CommandButtonId = 1;

        for name in button_names {
            let Some(common_button) = control_bar.find_command_button_resolved(&name) else {
                continue;
            };
            let command_button = CommandButton::from_common(next_id, common_button);
            name_to_id.insert(name.clone(), next_id);
            buttons_by_id.insert(next_id, command_button);
            next_id += 1;
        }

        let mut command_sets = HashMap::new();
        if let Some(manager) = get_command_set_manager() {
            for (name, set) in manager.iter_resolved_sets() {
                let mut command_set = CommandSet::new(name.clone());

                for index in 0..MAX_COMMANDS_PER_SET {
                    if let Some(button_name) = set.get_button_at_position(index) {
                        if let Some(id) = name_to_id.get(button_name) {
                            if let Some(button) = buttons_by_id.get(id) {
                                command_set.set_command_button(index, Some(button.clone()));
                            }
                        }
                    }
                }

                command_sets.insert(name.clone(), command_set);
            }
        }

        Ok(Self {
            buttons_by_id,
            command_sets,
        })
    }

    pub fn find_command_button_by_name(&self, name: &str) -> Option<&CommandButton> {
        self.buttons_by_id
            .values()
            .find(|button| button.name.eq_ignore_ascii_case(name))
    }

    pub fn find_command_set_by_name(&self, name: &str) -> Option<&CommandSet> {
        self.command_sets.get(name)
    }

    pub fn set_command_set_slot_override(
        &mut self,
        command_set_name: &str,
        slot: usize,
        button_name: Option<&str>,
    ) -> Result<(), String> {
        if slot >= MAX_COMMANDS_PER_SET {
            return Err(format!(
                "Command slot {} out of range [0, {})",
                slot, MAX_COMMANDS_PER_SET
            ));
        }

        let set_key = self
            .command_sets
            .keys()
            .find(|name| name.eq_ignore_ascii_case(command_set_name))
            .cloned()
            .ok_or_else(|| format!("Command set '{}' not found", command_set_name))?;

        let button = if let Some(name) = button_name {
            Some(
                self.find_command_button_by_name(name)
                    .cloned()
                    .ok_or_else(|| format!("Command button '{}' not found", name))?,
            )
        } else {
            None
        };

        let Some(command_set) = self.command_sets.get_mut(&set_key) else {
            return Err(format!("Command set '{}' not found", command_set_name));
        };
        command_set.buttons[slot] = button;
        Ok(())
    }
}

static CONTROL_BAR_BRIDGE: OnceCell<RwLock<ControlBarBridge>> = OnceCell::new();

pub fn initialize_control_bar_bridge_from_common() -> Result<(), String> {
    let bridge = ControlBarBridge::build_from_common()?;
    CONTROL_BAR_BRIDGE
        .set(RwLock::new(bridge))
        .map_err(|_| "ControlBarBridge already initialized".to_string())?;
    Ok(())
}

pub fn refresh_control_bar_bridge_from_common() -> Result<(), String> {
    let bridge = ControlBarBridge::build_from_common()?;
    if let Some(cell) = CONTROL_BAR_BRIDGE.get() {
        let mut guard = cell
            .write()
            .map_err(|_| "ControlBarBridge lock poisoned".to_string())?;
        *guard = bridge;
        return Ok(());
    }
    initialize_control_bar_bridge_from_common()
}

pub fn get_control_bar_bridge() -> Option<RwLockReadGuard<'static, ControlBarBridge>> {
    CONTROL_BAR_BRIDGE.get().and_then(|cell| cell.read().ok())
}

pub fn register_academy_template_context_provider() {
    set_academy_template_context_provider(|player| find_academy_template_context(player));
}

fn find_academy_template_context(player: PlayerHandle) -> Option<AcademyTemplateContext> {
    if !player.is_valid() || player.value() > i32::MAX as u32 {
        return None;
    }

    let player_index = player.value() as i32;
    let player_arc = player_list()
        .read()
        .ok()
        .and_then(|players| players.get_player(player_index).cloned())?;
    let object_ids = get_object_manager()
        .read()
        .ok()?
        .get_objects_owned_by_player(player.value());
    let control_bar = get_control_bar_bridge()?;
    let object_manager = get_object_manager();
    let object_manager = object_manager.read().ok()?;

    for object_id in object_ids {
        let Some(object_arc) = object_manager.get_object(object_id) else {
            continue;
        };
        let command_set_name = object_arc
            .read()
            .ok()
            .and_then(|object| {
                object
                    .base
                    .read()
                    .ok()
                    .map(|base| base.get_command_set_string().to_string())
            })
            .unwrap_or_default();
        if command_set_name.is_empty() {
            continue;
        }

        let Some(command_set) = control_bar.find_command_set_by_name(&command_set_name) else {
            continue;
        };
        let mut context = AcademyTemplateContext {
            dozer_command_set: CommandSetHandle::new(NameKeyGenerator::name_to_key(
                command_set.name.as_str(),
            )),
            ..AcademyTemplateContext::default()
        };

        for index in 0..MAX_COMMANDS_PER_SET {
            let Some(button) = command_set.get_command_button(index) else {
                continue;
            };
            let Some(template) = button.get_thing_template() else {
                continue;
            };

            if template.is_kind_of(KindOf::CommandCenter) {
                context.command_center_template = ThingTemplateHandle::new(template.get_id());
            } else if template.is_kind_of(KindOf::FSSupplyCenter) {
                context.supply_center_template = ThingTemplateHandle::new(template.get_id());
                context.supply_center_cost = player_arc
                    .read()
                    .ok()
                    .map(|player| template.calc_cost_to_build(Some(&*player)).max(0) as u32)
                    .unwrap_or_else(|| template.get_build_cost().max(0) as u32);
            }
        }

        if context.command_center_template.is_valid() || context.supply_center_template.is_valid() {
            return Some(context);
        }
    }

    None
}

pub fn set_command_set_slot_override(
    command_set_name: &str,
    slot: usize,
    button_name: Option<&str>,
) -> Result<(), String> {
    let Some(cell) = CONTROL_BAR_BRIDGE.get() else {
        return Err("ControlBarBridge not initialized".to_string());
    };
    let mut guard = cell
        .write()
        .map_err(|_| "ControlBarBridge lock poisoned".to_string())?;
    guard.set_command_set_slot_override(command_set_name, slot, button_name)?;
    if let Ok(mut logic) = crate::system::game_logic::lock_game_logic() {
        logic.set_control_bar_override(command_set_name, slot as i32, button_name);
    }
    Ok(())
}

/// Hooks for notifying the live UI to refresh control bar state.
pub trait ControlBarUiHooks: Send + Sync {
    fn mark_ui_dirty(&self);
    fn on_player_science_purchase_points_changed(&self, player_id: i32, points: i32);
    fn on_player_rank_changed(&self, player_id: i32, rank_level: i32, points: i32);
}

static CONTROL_BAR_UI_HOOKS: OnceCell<Arc<dyn ControlBarUiHooks>> = OnceCell::new();

pub fn register_control_bar_ui_hooks(hooks: Arc<dyn ControlBarUiHooks>) -> bool {
    CONTROL_BAR_UI_HOOKS.set(hooks).is_ok()
}

/// Notify the control bar that UI state needs to be refreshed.
pub fn mark_ui_dirty() {
    let _ = get_control_bar_bridge();
    if let Some(hooks) = CONTROL_BAR_UI_HOOKS.get() {
        hooks.mark_ui_dirty();
    }
}

pub fn notify_science_purchase_points_changed(player_id: i32, points: i32) {
    if let Some(hooks) = CONTROL_BAR_UI_HOOKS.get() {
        hooks.on_player_science_purchase_points_changed(player_id, points);
    }
}

pub fn notify_player_rank_changed(player_id: i32, rank_level: i32, points: i32) {
    if let Some(hooks) = CONTROL_BAR_UI_HOOKS.get() {
        hooks.on_player_rank_changed(player_id, rank_level, points);
    }
}

impl ControlBarInterface for ControlBarBridge {
    fn find_command_set(&self, name: &str) -> Option<&dyn std::any::Any> {
        self.command_sets
            .get(name)
            .map(|set| set as &dyn std::any::Any)
    }

    fn get_command_button(&self, button_id: CommandButtonId) -> Option<&CommandButton> {
        self.buttons_by_id.get(&button_id)
    }
}
