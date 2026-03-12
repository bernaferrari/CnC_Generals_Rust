//! Control Bar Implementation
//!
//! Rust conversion of ControlBar.cpp - the main control bar system that provides
//! context-sensitive command interface for the game.
//!
//! Original C++ file: GameClient/GUI/ControlBar/ControlBar.cpp
//! Original Author: Colin Day, March 2002

use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use super::{
    CommandButton, CommandOption, CommandSourceType, ControlBarContext, ControlBarState,
    ProductionItem, ProductionType,
};
use crate::gui::{GameWindow, WindowManager};
use crate::helpers::TheInGameUI;
use crate::message_stream::game_message::GameMessageType;
use crate::message_stream::message_stream::THE_MESSAGE_STREAM;
use crate::system::SubsystemInterface;
use game_engine::common::ini::ini_command_button::{
    get_control_bar as get_ini_control_bar, CommandButton as IniCommandButton,
};
use game_engine::common::rts::{get_science_store, ScienceType, SCIENCE_INVALID};
use gamelogic::command_button::map_gui_command_to_command_type;
use gamelogic::commands::{get_command_queue_manager, Command, CommandPriority, QueuedCommand};
use gamelogic::common::types::{ControlBarInterface, OBJECT_STATUS_UNDER_CONSTRUCTION};
use gamelogic::common::GameError;
use gamelogic::control_bar::get_control_bar_bridge;
use gamelogic::helpers::TheGameLogic;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{player_list as logic_player_list, PlayerIndex};
use gamelogic::upgrade::center::with_upgrade_center;

/// Main Control Bar class - equivalent to C++ ControlBar
pub struct ControlBar {
    /// Current control bar context
    context: Arc<RwLock<ControlBarContext>>,

    /// Game window manager reference
    window_manager: Option<Arc<WindowManager>>,

    /// Control bar scheme manager
    scheme_manager: Option<Arc<dyn ControlBarSchemeManager>>,

    /// Control bar resizer
    resizer: Option<Arc<dyn ControlBarResizer>>,

    /// Current game window
    current_window: Option<Arc<GameWindow>>,

    /// Animation state
    is_animating: bool,
    animation_start_time: Instant,
    animation_duration: Duration,

    /// Button state tracking
    button_states: HashMap<String, ButtonState>,

    /// Observer mode flag
    observer_mode: bool,

    /// Multi-select mode flag
    multi_select_mode: bool,
}

/// Button state information
#[derive(Debug, Clone)]
struct ButtonState {
    enabled: bool,
    visible: bool,
    pressed: bool,
    progress: f32,
    flash_time: Option<Instant>,
}

/// Control Bar Scheme Manager trait
pub trait ControlBarSchemeManager: Send + Sync {
    fn load_scheme(&self, scheme_name: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn get_scheme(&self) -> Option<Arc<ControlBarScheme>>;
    fn set_scheme(&mut self, scheme: Arc<ControlBarScheme>);
}

/// Control Bar Resizer trait
pub trait ControlBarResizer: Send + Sync {
    fn resize(&self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>>;
    fn get_optimal_size(&self) -> (u32, u32);
}

/// Control Bar Scheme data
#[derive(Debug, Clone)]
pub struct ControlBarScheme {
    pub name: String,
    pub images: HashMap<String, String>,
    pub animations: HashMap<String, ControlBarAnimation>,
    pub layout: ControlBarLayout,
}

/// Control Bar Animation
#[derive(Debug, Clone)]
pub struct ControlBarAnimation {
    pub frames: Vec<String>,
    pub frame_duration: Duration,
    pub loop_animation: bool,
}

/// Control Bar Layout
#[derive(Debug, Clone)]
pub struct ControlBarLayout {
    pub command_buttons: Vec<ButtonLayout>,
    pub info_panels: Vec<PanelLayout>,
    pub construction_queue: QueueLayout,
}

/// Button layout information
#[derive(Debug, Clone)]
pub struct ButtonLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub command_name: String,
}

/// Panel layout information  
#[derive(Debug, Clone)]
pub struct PanelLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub panel_type: String,
}

/// Queue layout information
#[derive(Debug, Clone)]
pub struct QueueLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub max_visible_items: u32,
}

impl ControlBar {
    /// Create new ControlBar instance
    pub fn new() -> Self {
        Self {
            context: Arc::new(RwLock::new(ControlBarContext::default())),
            window_manager: None,
            scheme_manager: None,
            resizer: None,
            current_window: None,
            is_animating: false,
            animation_start_time: Instant::now(),
            animation_duration: Duration::from_millis(500),
            button_states: HashMap::new(),
            observer_mode: false,
            multi_select_mode: false,
        }
    }

    /// Set window manager
    pub fn set_window_manager(&mut self, manager: Arc<WindowManager>) {
        self.window_manager = Some(manager);
    }

    /// Set scheme manager
    pub fn set_scheme_manager(&mut self, manager: Arc<dyn ControlBarSchemeManager>) {
        self.scheme_manager = Some(manager);
    }

    /// Set resizer
    pub fn set_resizer(&mut self, resizer: Arc<dyn ControlBarResizer>) {
        self.resizer = Some(resizer);
    }

    /// Update control bar based on current selection
    pub fn update_for_selection(
        &mut self,
        selected_objects: Vec<u32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut state_changed = false;
        {
            let mut context = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;

            context.selected_objects = selected_objects;
            let new_state = if self.observer_mode {
                ControlBarState::Observer
            } else if context.selected_objects.is_empty() {
                ControlBarState::Default
            } else if context.selected_objects.len() > 1 {
                ControlBarState::MultiSelect
            } else {
                let selected = context.selected_objects[0];
                let under_construction = OBJECT_REGISTRY
                    .get_object(selected)
                    .and_then(|obj| {
                        obj.read()
                            .ok()
                            .map(|guard| guard.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION))
                    })
                    .unwrap_or(false);
                if under_construction {
                    ControlBarState::UnderConstruction
                } else {
                    ControlBarState::Command
                }
            };
            state_changed = context.current_state != new_state;
            context.current_state = new_state;
        }

        if state_changed {
            self.rebuild_command_buttons_for_current_context()?;
        }

        Ok(())
    }

    /// Set observer mode
    pub fn set_observer_mode(&mut self, observer: bool) {
        self.observer_mode = observer;
        if let Ok(mut context) = self.context.write() {
            context.current_state = if observer {
                ControlBarState::Observer
            } else {
                ControlBarState::Default
            };
        }
        let _ = self.rebuild_command_buttons_for_current_context();
    }

    /// Process command button click
    pub fn process_command(
        &mut self,
        command_name: &str,
        source: CommandSourceType,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        log::info!("Processing command: {} from {:?}", command_name, source);

        let context = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?;

        // Find the command button
        if let Some(button) = context
            .available_commands
            .iter()
            .find(|b| b.command_name == command_name)
        {
            // Check if command is enabled
            if !self.is_command_enabled(&button, &context) {
                log::warn!("Command {} is not enabled", command_name);
                return Ok(false);
            }

            // Process based on command type
            self.execute_command(button, source, &context)?;
            Ok(true)
        } else {
            log::warn!("Command {} not found in available commands", command_name);
            Ok(false)
        }
    }

    /// Check if command is enabled
    fn is_command_enabled(&self, button: &CommandButton, context: &ControlBarContext) -> bool {
        // Check if we're in observer mode
        if self.observer_mode && (button.options & CommandOption::ScriptOnly as u32) == 0 {
            return false;
        }

        // Check multi-select compatibility
        if context.selected_objects.len() > 1
            && (button.options & CommandOption::OkForMultiSelect as u32) == 0
        {
            return false;
        }

        let sciences_to_check: Option<Vec<ScienceType>> = if !button.sciences_ids.is_empty() {
            Some(button.sciences_ids.clone())
        } else if !button.sciences.is_empty() {
            get_science_store().map(|store| {
                button
                    .sciences
                    .iter()
                    .map(|name| store.get_science_from_internal_name(name))
                    .collect()
            })
        } else {
            None
        };

        if let Some(sciences) = sciences_to_check {
            if let Some(store) = get_science_store() {
                if let Ok(player_list_guard) = logic_player_list().read() {
                    let player_index: PlayerIndex = context.player_id as PlayerIndex;
                    if let Some(player_arc) = player_list_guard.get_player(player_index) {
                        if let Ok(player) = player_arc.read() {
                            for science in sciences {
                                if science == SCIENCE_INVALID {
                                    continue;
                                }

                                if player.is_science_hidden(science)
                                    || !store.player_has_root_prereqs_for_science(&*player, science)
                                {
                                    return false;
                                }

                                if player.is_science_disabled(science) {
                                    return false;
                                }

                                if !player.has_science(science) {
                                    if !store.player_has_prereqs_for_science(&*player, science) {
                                        return false;
                                    }

                                    let cost = store.get_science_purchase_cost(science);
                                    if cost > 0 && cost > player.get_science_purchase_points() {
                                        return false;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let player_arc = if let Ok(player_list_guard) = logic_player_list().read() {
            let player_index: PlayerIndex = context.player_id as PlayerIndex;
            player_list_guard.get_player(player_index).cloned()
        } else {
            None
        };

        if !button.upgrade.is_empty() {
            let Some(player_arc) = player_arc.as_ref() else {
                return false;
            };
            let Ok(player) = player_arc.read() else {
                return false;
            };
            let upgrade =
                with_upgrade_center(|center| center.find_upgrade(button.upgrade.as_str()));
            if let Some(template) = upgrade {
                if player.has_upgrade_complete(&template)
                    || player.has_upgrade_in_production(&template)
                {
                    return false;
                }
                if !with_upgrade_center(|center| {
                    center.can_afford_upgrade(&*player, &template, false)
                }) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if !button.purchase_cost.is_empty() {
            let Some(player_arc) = player_arc.as_ref() else {
                return false;
            };
            let Ok(player) = player_arc.read() else {
                return false;
            };
            for (resource, cost) in &button.purchase_cost {
                if *cost <= 0 {
                    continue;
                }
                if resource.eq_ignore_ascii_case("cash")
                    || resource.eq_ignore_ascii_case("money")
                    || resource.eq_ignore_ascii_case("supplies")
                {
                    if !player.get_money().can_afford(*cost) {
                        return false;
                    }
                }
            }
        }

        if !button.special_power.is_empty() {
            if context.selected_objects.is_empty() {
                return false;
            }

            let mut any_ready = false;
            for object_id in &context.selected_objects {
                let Some(obj_arc) = OBJECT_REGISTRY.get_object(*object_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                let Some(sp_behavior) =
                    obj_guard.get_special_power_module_by_name(button.special_power.as_str())
                else {
                    continue;
                };
                let behavior_guard = sp_behavior.lock().unwrap();
                let Some(sp_module) = behavior_guard.get_special_power_module_interface_const()
                else {
                    continue;
                };
                if sp_module.is_ready() {
                    any_ready = true;
                    break;
                }
            }

            if !any_ready {
                return false;
            }
        }

        true
    }

    /// Execute command
    fn execute_command(
        &self,
        button: &CommandButton,
        source: CommandSourceType,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!(
            "Executing command: {} (options: 0x{:08X})",
            button.command_name,
            button.options
        );

        if Self::command_needs_target(button.options) {
            self.enter_targeting_mode(button, context)?;
            return Ok(());
        }

        if button.command_type == gamelogic::commands::CommandType::PurchaseScience {
            self.execute_purchase_science(button, context)?;
            return Ok(());
        }

        if button.command_type == gamelogic::commands::CommandType::MetaSelectMatchingUnits {
            self.select_all_units_of_type(button, context)?;
            return Ok(());
        }

        // Handle upgrade commands
        if !button.upgrade.is_empty() {
            self.execute_upgrade_command(button, context, source)?;
            return Ok(());
        }

        if !button.object.is_empty() {
            self.execute_production_command(button, context, source)?;
        } else if !button.special_power.is_empty() {
            self.execute_special_power_command(button, context, source)?;
        } else {
            self.execute_direct_command(button, context, source)?;
        }

        Ok(())
    }

    fn command_needs_target(options: u32) -> bool {
        let mut mask = CommandOption::NeedTargetEnemyObject as u32
            | CommandOption::NeedTargetNeutralObject as u32
            | CommandOption::NeedTargetAllyObject as u32
            | CommandOption::NeedTargetPos as u32
            | CommandOption::ContextmodeCommand as u32;
        #[cfg(feature = "allow_surrender")]
        {
            mask |= CommandOption::NeedTargetPrisoner as u32;
        }
        options & mask != 0
    }

    fn enter_targeting_mode(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let source_id = context.selected_objects.first().copied().unwrap_or(0);
        TheInGameUI::place_build_available(None, None);
        TheInGameUI::clear_pending_special_power();
        TheInGameUI::set_force_attack_mode(false);
        TheInGameUI::set_force_move_to_mode(false);
        TheInGameUI::set_prefer_selection_mode(false);

        if (button.options & CommandOption::UsesMineClearingWeaponSet as u32) != 0 {
            if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
                stream.append_message(GameMessageType::SetMineClearingDetail(0));
            }
        }

        if button.command_type == gamelogic::commands::CommandType::DozerConstruct
            && !button.object.is_empty()
        {
            TheInGameUI::place_build_available(Some(button.object.clone()), Some(source_id));
        }

        if !button.special_power.is_empty() {
            if let Some(logic_button) = self.resolve_logic_button(button) {
                if let Some(sp_template) = logic_button.get_special_power_template() {
                    TheInGameUI::set_pending_special_power(
                        sp_template.get_id(),
                        button.options,
                        source_id,
                    );
                }
            }
        }

        if (button.options & CommandOption::NeedTargetEnemyObject as u32) != 0
            || (button.options & CommandOption::AttackObjectsPosition as u32) != 0
        {
            TheInGameUI::set_force_attack_mode(true);
        }
        if (button.options & CommandOption::NeedTargetPos as u32) != 0 {
            TheInGameUI::set_force_move_to_mode(true);
        }
        if (button.options
            & (CommandOption::NeedTargetAllyObject as u32
                | CommandOption::NeedTargetNeutralObject as u32))
            != 0
        {
            TheInGameUI::set_prefer_selection_mode(true);
        }

        Ok(())
    }

    fn resolve_logic_button(
        &self,
        button: &CommandButton,
    ) -> Option<gamelogic::command_button::CommandButton> {
        let control_bar = get_control_bar_bridge()?;
        control_bar
            .find_command_button_by_name(&button.command_name)
            .cloned()
    }

    fn execute_purchase_science(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(store) = get_science_store() else {
            return Ok(());
        };
        let player_index: PlayerIndex = context.player_id as PlayerIndex;
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index).cloned());
        let Some(player_arc) = player_arc else {
            return Ok(());
        };
        let Ok(player) = player_arc.read() else {
            return Ok(());
        };

        let mut selected_science = SCIENCE_INVALID;
        for &science in &button.sciences_ids {
            if science == SCIENCE_INVALID {
                continue;
            }
            if !player.has_science(science)
                && store.player_has_prereqs_for_science(&*player, science)
                && store.get_science_purchase_cost(science) <= player.get_science_purchase_points()
            {
                selected_science = science;
                break;
            }
        }

        if selected_science == SCIENCE_INVALID {
            return Ok(());
        }

        let mut command = Command::new(gamelogic::commands::CommandType::PurchaseScience);
        command.set_player_index(context.player_id as i32);
        command.append_integer_argument(selected_science as i32);
        self.queue_command(context.player_id as i32, command)?;
        Ok(())
    }

    fn select_all_units_of_type(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let template_id = if !button.object.is_empty() {
            gamelogic::helpers::TheThingFactory::find_template(button.object.as_str())
                .map(|t| t.get_id())
        } else {
            self.resolve_logic_button(button)
                .and_then(|logic_button| logic_button.get_thing_template().map(|t| t.get_id()))
        };

        let Some(template_id) = template_id else {
            return Ok(());
        };

        let player_index: PlayerIndex = context.player_id as PlayerIndex;
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_index).cloned());
        let Some(player_arc) = player_arc else {
            return Ok(());
        };
        let Ok(player) = player_arc.read() else {
            return Ok(());
        };

        let mut matches: Vec<u32> = Vec::new();
        let _ = player.iterate_objects(|obj| {
            let guard = obj.read().map_err(|_| GameError::LockError)?;
            if guard.get_template().get_id() == template_id {
                matches.push(guard.get_id());
            }
            Ok(())
        });

        if matches.is_empty() {
            return Ok(());
        }

        if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
            stream.append_message(GameMessageType::CreateSelectedGroup(true, matches));
        }
        Ok(())
    }

    /// Execute upgrade command (player or object upgrade)
    fn execute_upgrade_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        _source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Executing upgrade command for: {}", button.upgrade);

        let upgrade_name = button.upgrade.as_str();

        // Get the player's upgrade center and find the template
        let upgrade_template = with_upgrade_center(|center| center.find_upgrade(upgrade_name));
        let Some(template) = upgrade_template else {
            log::warn!("Upgrade template '{}' not found", upgrade_name);
            return Ok(());
        };

        // Get the first selected object for object upgrades
        let source_obj_id = context.selected_objects.first().copied();

        // Create the upgrade command
        let mut command = Command::new(gamelogic::commands::CommandType::QueueUpgrade);
        command.set_player_index(context.player_id as i32);

        if let Some(obj_id) = source_obj_id {
            command.append_object_id_argument(obj_id);
        }

        command.append_integer_argument(template.get_name_key() as i32);

        self.queue_command(context.player_id as i32, command)?;
        Ok(())
    }

    /// Execute production command (build units/structures)
    fn execute_production_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Executing production command for: {}", button.object);

        let button_id = self.resolve_command_button_id(button)?;
        let cmd_source = Self::map_command_source(source);
        for object_id in &context.selected_objects {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let _ = obj_guard.do_command_button(button_id, cmd_source);
        }

        Ok(())
    }

    /// Execute special power command
    fn execute_special_power_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Executing special power: {}", button.special_power);

        let button_id = self.resolve_command_button_id(button)?;
        let cmd_source = Self::map_command_source(source);
        for object_id in &context.selected_objects {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*object_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };
            let _ = obj_guard.do_command_button(button_id, cmd_source);
        }

        Ok(())
    }

    /// Execute direct command (no targeting required)
    fn execute_direct_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Executing direct command: {}", button.command_name);

        // Implementation would send command directly to selected objects
        // Examples: Stop, Guard, Aggressive stance, etc.
        let player_id = context.player_id as i32;
        if context.selected_objects.is_empty() {
            return Ok(());
        }
        if button.command_type == gamelogic::commands::CommandType::Invalid {
            return Ok(());
        }

        if let Ok(button_id) = self.resolve_command_button_id(button) {
            let cmd_source = Self::map_command_source(source);
            for object_id in &context.selected_objects {
                let Some(obj_arc) = OBJECT_REGISTRY.get_object(*object_id) else {
                    continue;
                };
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };
                let _ = obj_guard.do_command_button(button_id, cmd_source);
            }
            return Ok(());
        }

        let mut command = Command::new(button.command_type);
        command.set_player_index(player_id);
        for object_id in &context.selected_objects {
            command.append_object_id_argument(*object_id);
        }

        self.queue_command(player_id, command)?;
        Ok(())
    }

    /// Rebuild available command buttons based on current context
    fn rebuild_command_buttons(
        &mut self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        context.available_commands.clear();

        match context.current_state {
            ControlBarState::Default => {
                // Add default commands (general powers, etc.)
                self.add_default_commands(context)?;
            }
            ControlBarState::Command => {
                // Add commands for selected object(s)
                self.add_object_commands(context)?;
            }
            ControlBarState::MultiSelect => {
                // Add multi-select compatible commands
                self.add_multi_select_commands(context)?;
            }
            ControlBarState::Observer => {
                // Add observer-only commands
                self.add_observer_commands(context)?;
            }
            ControlBarState::UnderConstruction => {
                // Add construction commands
                self.add_construction_commands(context)?;
            }
        }

        log::debug!(
            "Rebuilt command buttons: {} available",
            context.available_commands.len()
        );
        self.refresh_button_states(context);
        Ok(())
    }

    fn rebuild_command_buttons_for_current_context(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut context = {
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            std::mem::take(&mut *guard)
        };
        self.rebuild_command_buttons(&mut context)?;
        let mut guard = self
            .context
            .write()
            .map_err(|_| "Failed to acquire context write lock")?;
        *guard = context;
        Ok(())
    }

    /// Add default commands
    fn add_default_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::debug!("Adding default commands for player {}", context.player_id);

        if let Some(control_bar) = get_ini_control_bar() {
            for (_name, definition) in control_bar.iter_buttons().take(12) {
                let button = Self::command_from_definition(definition);
                context.available_commands.push(button);
            }
        }
        Ok(())
    }

    pub(super) fn command_from_definition(definition: &IniCommandButton) -> CommandButton {
        let mut button = CommandButton::default();

        button.command_name = if !definition.command.is_empty() {
            definition.command.clone()
        } else {
            definition.name.clone()
        };
        button.command_type = map_gui_command_to_command_type(&button.command_name);

        button.button_image = definition.button_image.clone();
        button.button_border_type = definition.button_border_type.clone();
        button.text_label = definition.text_label.clone();
        button.text_label_placehold = definition.text_label.clone();
        button.descriptive_text = definition.descriptive_text.clone();
        button.conflicting_element = definition.conflicting_element.clone();
        button.cursor_name = definition.cursor_name.clone();
        button.invalid_cursor_name = definition.invalid_cursor_name.clone();
        button.unit_specific_sound = definition.unit_specific_sound.clone();
        button.sciences = definition.science_required.clone();
        button.sciences_ids = definition.parsed_science_required.clone();
        button.options = definition.options_bits;
        button.object = definition.object.clone();
        button.upgrade = definition.upgrade.clone();
        button.special_power = definition
            .special_power_template
            .clone()
            .unwrap_or_default();
        button.radius_cursor_type = definition.radius_cursor_type.clone();
        button.max_shorable_instances = definition.max_shots_to_fire;

        if definition.purchase_cost != 0 {
            button
                .purchase_cost
                .insert("Cash".to_string(), definition.purchase_cost);
        }

        button
    }

    pub(super) fn command_from_logic_button(
        logic_button: &gamelogic::command_button::CommandButton,
    ) -> CommandButton {
        let mut button = CommandButton::default();
        button.command_name = logic_button.get_name().to_string();
        button.command_type = logic_button.get_command_type();
        button.text_label = logic_button.get_name().to_string();
        button.descriptive_text = logic_button.tooltip.clone();
        button.options = logic_button.get_options_bits();
        button.sciences_ids = logic_button.science_vec().to_vec();
        button.max_shorable_instances = logic_button.get_max_shots_to_fire();
        if let Some(template) = logic_button.get_thing_template() {
            button.object = template.get_name().as_str().to_string();
        }
        if let Some(upgrade) = logic_button.get_upgrade_template() {
            button.upgrade = upgrade.get_name().as_str().to_string();
        }
        if let Some(sp) = logic_button.get_special_power_template() {
            button.special_power = sp.get_name().to_string();
        }
        button
    }

    pub(super) fn push_command_if_missing(context: &mut ControlBarContext, button: CommandButton) {
        if context.available_commands.iter().any(|existing| {
            existing
                .command_name
                .eq_ignore_ascii_case(&button.command_name)
        }) {
            return;
        }
        context.available_commands.push(button);
    }

    fn resolve_command_button_id(
        &self,
        button: &CommandButton,
    ) -> Result<gamelogic::command_button::CommandButtonId, Box<dyn std::error::Error>> {
        let Some(control_bar) = get_control_bar_bridge() else {
            return Err("Control bar bridge not initialized".into());
        };
        let Some(logic_button) = control_bar.find_command_button_by_name(&button.command_name)
        else {
            return Err(format!(
                "Command button '{}' not found in GameLogic bridge",
                button.command_name
            )
            .into());
        };
        Ok(logic_button.get_id())
    }

    fn map_command_source(source: CommandSourceType) -> gamelogic::common::CommandSourceType {
        match source {
            CommandSourceType::FromUser => gamelogic::common::CommandSourceType::FromPlayer,
            CommandSourceType::FromScript => gamelogic::common::CommandSourceType::FromScript,
            CommandSourceType::FromAI => gamelogic::common::CommandSourceType::FromAi,
            CommandSourceType::None => gamelogic::common::CommandSourceType::FromPlayer,
        }
    }

    fn queue_command(
        &self,
        player_id: i32,
        command: Command,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let current_frame = TheGameLogic::get_frame();
        let queued = QueuedCommand::new(command, CommandPriority::Normal, current_frame);

        let queue_manager = get_command_queue_manager();
        let mut manager = queue_manager
            .lock()
            .map_err(|_| "Failed to lock command queue manager")?;

        if let Err(err) = manager.queue_player_command(player_id, queued.clone()) {
            if let Err(init_err) = manager.initialize_player(player_id) {
                return Err(format!(
                    "Failed to initialize player {} for command queue: {}",
                    player_id, init_err
                )
                .into());
            }
            if let Err(queue_err) = manager.queue_player_command(player_id, queued) {
                return Err(format!(
                    "Failed to queue command for player {}: {}",
                    player_id, queue_err
                )
                .into());
            }
        }

        Ok(())
    }

    /// Add object-specific commands
    fn add_object_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::debug!(
            "Adding object commands for {} selected objects",
            context.selected_objects.len()
        );
        if context.selected_objects.is_empty() {
            return Ok(());
        }

        let Some(control_bar) = get_control_bar_bridge() else {
            return Ok(());
        };
        let Some(common_bar) = get_ini_control_bar() else {
            return Ok(());
        };

        let Some(first_id) = context.selected_objects.first().copied() else {
            return Ok(());
        };
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(first_id) else {
            return Ok(());
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return Ok(());
        };
        let command_set_name = obj_guard.get_command_set_string();
        let command_set = control_bar
            .find_command_set_by_name(command_set_name)
            .or_else(|| {
                control_bar.find_command_set_by_name(&command_set_name.to_ascii_uppercase())
            });

        let Some(command_set) = command_set else {
            return Ok(());
        };

        for button_opt in &command_set.buttons {
            let Some(button) = button_opt.as_ref() else {
                continue;
            };
            if let Some(common_button) = common_bar.find_command_button_resolved(button.get_name())
            {
                context
                    .available_commands
                    .push(Self::command_from_definition(common_button));
            } else {
                context
                    .available_commands
                    .push(Self::command_from_logic_button(button));
            }
        }

        super::control_bar_structure_inventory::append_structure_inventory_commands(context)?;
        super::control_bar_beacon::append_beacon_commands(context)?;
        Ok(())
    }

    /// Add multi-select commands
    fn add_multi_select_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::debug!(
            "Adding multi-select commands for {} objects",
            context.selected_objects.len()
        );
        super::control_bar_multi_select::populate_multi_select_commands(context)?;

        if context.available_commands.is_empty() {
            self.add_object_commands(context)?;
        }
        Ok(())
    }

    /// Add observer commands
    fn add_observer_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::debug!("Adding observer commands");
        super::control_bar_observer::populate_observer_commands(context)?;
        Ok(())
    }

    /// Add construction commands
    fn add_construction_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        log::debug!("Adding construction commands");
        super::control_bar_under_construction::populate_under_construction_commands(context)?;
        Ok(())
    }

    /// Update control bar animations and state
    pub fn update(&mut self, delta_time: Duration) -> Result<(), Box<dyn std::error::Error>> {
        // Update animation state
        if self.is_animating {
            let elapsed = self.animation_start_time.elapsed();
            if elapsed >= self.animation_duration {
                self.is_animating = false;
            }
        }

        // Update button states (flashing, progress bars, etc.)
        let current_time = Instant::now();
        for (_, state) in self.button_states.iter_mut() {
            if let Some(flash_time) = state.flash_time {
                if current_time.duration_since(flash_time) > Duration::from_millis(500) {
                    state.flash_time = None;
                }
            }
        }

        // Update construction queue progress
        if let Ok(mut context) = self.context.write() {
            for item in context.construction_queue.iter_mut() {
                if item.progress < 1.0 {
                    item.progress += delta_time.as_secs_f32() / item.build_time;
                    item.progress = item.progress.min(1.0);
                }
            }
        }

        let context_snapshot = self.context.read().ok().map(|context| context.clone());
        if let Some(context) = context_snapshot {
            self.refresh_button_states(&context);
        }

        Ok(())
    }

    /// Get current context (read-only)
    pub fn get_context(&self) -> Arc<RwLock<ControlBarContext>> {
        self.context.clone()
    }
}

impl SubsystemInterface for ControlBar {
    fn init(&mut self) -> Result<(), Box<dyn Error>> {
        log::info!("Initializing Control Bar");

        if let Some(scheme_manager) = &self.scheme_manager {
            scheme_manager.load_scheme("Default")?;
        }

        log::info!("Control Bar initialized successfully");
        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn Error>> {
        let delta_time = Duration::from_millis(16); // ~60 FPS
        self.update(delta_time)?;
        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn Error>> {
        log::info!("Resetting Control Bar");

        if let Ok(mut context) = self.context.write() {
            *context = ControlBarContext::default();
        }

        self.button_states.clear();
        self.is_animating = false;

        Ok(())
    }
}

impl Default for ControlBar {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            enabled: true,
            visible: true,
            pressed: false,
            progress: 0.0,
            flash_time: None,
        }
    }
}

impl ControlBar {
    fn refresh_button_states(&mut self, context: &ControlBarContext) {
        let mut refreshed = HashMap::new();
        for button in &context.available_commands {
            let mut state = self
                .button_states
                .get(&button.command_name)
                .cloned()
                .unwrap_or_default();
            state.enabled = self.is_command_enabled(button, context);
            state.visible = true;
            refreshed.insert(button.command_name.clone(), state);
        }
        self.button_states = refreshed;
    }
}
