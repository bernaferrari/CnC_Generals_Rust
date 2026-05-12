//! Control Bar Implementation
//!
//! Rust conversion of ControlBar.cpp + ControlBarCommand.cpp - the main control bar system
//! that provides context-sensitive command interface for the game.
//!
//! Original C++ files:
//!   GameClient/GUI/ControlBar/ControlBar.cpp
//!   GameClient/GUI/ControlBar/ControlBarCommand.cpp
//! Original Author: Colin Day, March 2002

use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use super::{
    BuildQueueEntry, CommandAvailability, CommandButton, CommandOption, CommandSourceType,
    ControlBarContext, ControlBarState, ProductionItem, ProductionType, QueueProductionType,
    MAX_BUILD_QUEUE_BUTTONS,
};
use crate::gui::{GameWindow, WindowManager};
use crate::helpers::TheInGameUI;
use crate::message_stream::game_message::GameMessageType;
use crate::message_stream::message_stream::THE_MESSAGE_STREAM;
use crate::system::SubsystemInterface;
use game_engine::common::ini::ini_command_button::{
    get_control_bar as get_ini_control_bar, CommandButton as IniCommandButton,
};
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::rts::{get_science_store, ScienceType, SCIENCE_INVALID};
use gamelogic::command_button::map_gui_command_to_command_type;
use gamelogic::commands::command::CommandType;
use gamelogic::commands::{get_command_queue_manager, Command, CommandPriority, QueuedCommand};
use gamelogic::common::types::OBJECT_STATUS_UNDER_CONSTRUCTION;
use gamelogic::common::GameError;
use gamelogic::control_bar::get_control_bar_bridge;
use gamelogic::helpers::{TheGameLogic, TheThingFactory};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{player_list as logic_player_list, PlayerIndex};
use gamelogic::system::beacon_manager::snapshot_beacons;
use gamelogic::upgrade::center::with_upgrade_center;

pub struct ControlBar {
    context: Arc<RwLock<ControlBarContext>>,
    window_manager: Option<Arc<WindowManager>>,
    scheme_manager: Option<Arc<dyn ControlBarSchemeManager>>,
    resizer: Option<Arc<dyn ControlBarResizer>>,
    current_window: Option<Arc<GameWindow>>,
    is_animating: bool,
    animation_start_time: Instant,
    animation_duration: Duration,
    button_states: HashMap<String, ButtonState>,
    observer_mode: bool,
    multi_select_mode: bool,
    ui_dirty: bool,
    build_queue_data: Vec<BuildQueueEntry>,
    displayed_queue_count: usize,
    current_frame: u32,
    flash_active: bool,
}

#[derive(Debug, Clone)]
struct ButtonState {
    enabled: bool,
    visible: bool,
    pressed: bool,
    progress: f32,
    flash_time: Option<Instant>,
    availability: CommandAvailability,
    check_like_active: bool,
}

pub trait ControlBarSchemeManager: Send + Sync {
    fn load_scheme(&self, scheme_name: &str) -> Result<(), Box<dyn std::error::Error>>;
    fn get_scheme(&self) -> Option<Arc<ControlBarScheme>>;
    fn set_scheme(&mut self, scheme: Arc<ControlBarScheme>);
}

pub trait ControlBarResizer: Send + Sync {
    fn resize(&self, width: u32, height: u32) -> Result<(), Box<dyn std::error::Error>>;
    fn get_optimal_size(&self) -> (u32, u32);
}

#[derive(Debug, Clone)]
pub struct ControlBarScheme {
    pub name: String,
    pub images: HashMap<String, String>,
    pub animations: HashMap<String, ControlBarAnimation>,
    pub layout: ControlBarLayout,
}

#[derive(Debug, Clone)]
pub struct ControlBarAnimation {
    pub frames: Vec<String>,
    pub frame_duration: Duration,
    pub loop_animation: bool,
}

#[derive(Debug, Clone)]
pub struct ControlBarLayout {
    pub command_buttons: Vec<ButtonLayout>,
    pub info_panels: Vec<PanelLayout>,
    pub construction_queue: QueueLayout,
}

#[derive(Debug, Clone)]
pub struct ButtonLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub command_name: String,
}

#[derive(Debug, Clone)]
pub struct PanelLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub panel_type: String,
}

#[derive(Debug, Clone)]
pub struct QueueLayout {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub max_visible_items: u32,
}

impl ControlBar {
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
            ui_dirty: false,
            build_queue_data: Vec::new(),
            displayed_queue_count: 0,
            current_frame: 0,
            flash_active: false,
        }
    }

    pub fn set_window_manager(&mut self, manager: Arc<WindowManager>) {
        self.window_manager = Some(manager);
    }

    pub fn set_scheme_manager(&mut self, manager: Arc<dyn ControlBarSchemeManager>) {
        self.scheme_manager = Some(manager);
    }

    pub fn set_resizer(&mut self, resizer: Arc<dyn ControlBarResizer>) {
        self.resizer = Some(resizer);
    }

    // ---------------------------------------------------------------------------
    // markUIDirty / onDrawableSelected / onDrawableDeselected
    // C++ ControlBar.cpp:114-1617
    // ---------------------------------------------------------------------------

    /// Mark the UI dirty so context is re-evaluated on next update.
    /// C++: ControlBar::markUIDirty()
    pub fn mark_ui_dirty(&mut self) {
        self.ui_dirty = true;
    }

    /// Called when a drawable is selected. Cancels pending GUI commands.
    /// C++: ControlBar::onDrawableSelected()
    pub fn on_drawable_selected(&mut self) {
        self.mark_ui_dirty();
        TheInGameUI::clear_pending_special_power();
    }

    /// Called when a drawable is deselected.
    /// C++: ControlBar::onDrawableDeselected()
    pub fn on_drawable_deselected(&mut self, select_count: usize) {
        self.mark_ui_dirty();
        if select_count == 0 {
            TheInGameUI::clear_pending_special_power();
        }
        TheInGameUI::place_build_available(None, None);
    }

    // ---------------------------------------------------------------------------
    // update - main per-frame update
    // C++ ControlBar.cpp:1359-1580
    // ---------------------------------------------------------------------------

    /// Main update loop. Mirrors C++ ControlBar::update().
    pub fn update(&mut self, delta_time: Duration) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_animating {
            let elapsed = self.animation_start_time.elapsed();
            if elapsed >= self.animation_duration {
                self.is_animating = false;
            }
        }

        let current_time = Instant::now();
        for (_, state) in self.button_states.iter_mut() {
            if let Some(flash_time) = state.flash_time {
                if current_time.duration_since(flash_time) > Duration::from_millis(500) {
                    state.flash_time = None;
                }
            }
        }

        if self.observer_mode {
            return Ok(());
        }

        if self.ui_dirty {
            self.evaluate_context_ui()?;
            self.ui_dirty = false;
        }

        self.update_place_beacon_button_enabled();

        let context = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?;
        let current_state = context.current_state;
        let selected_objects = context.selected_objects.clone();
        let player_id = context.player_id;
        drop(context);

        if current_state == ControlBarState::MultiSelect {
            self.update_context_multi_select()?;
            return Ok(());
        }

        if selected_objects.is_empty() {
            return Ok(());
        }

        let Some(&first_id) = selected_objects.first() else {
            return Ok(());
        };
        let obj_exists = OBJECT_REGISTRY
            .get_object(first_id)
            .map(|arc| arc.read().is_ok())
            .unwrap_or(false);
        if !obj_exists {
            self.switch_to_context(ControlBarState::None, None)?;
            return Ok(());
        }

        match current_state {
            ControlBarState::None => {}
            ControlBarState::Command => {
                self.update_context_command()?;
            }
            ControlBarState::StructureInventory => {
                self.update_context_structure_inventory()?;
            }
            ControlBarState::Beacon => {
                self.update_context_beacon()?;
            }
            ControlBarState::UnderConstruction => {
                self.update_context_under_construction(delta_time)?;
            }
            ControlBarState::OclTimer => {
                self.update_context_ocl_timer(delta_time)?;
            }
            ControlBarState::Observer => {}
            ControlBarState::MultiSelect => {}
        }

        if let Ok(mut context) = self.context.write() {
            for item in context.construction_queue.iter_mut() {
                if item.progress < 1.0 && item.build_time > 0.0 {
                    item.progress += delta_time.as_secs_f32() / item.build_time;
                    item.progress = item.progress.min(1.0);
                }
            }
        }

        let context_snapshot = self.context.read().ok().map(|c| c.clone());
        if let Some(context) = context_snapshot {
            self.refresh_button_states(&context, player_id);
        }

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // evaluateContextUI - determine what context to show
    // C++ ControlBar.cpp:1689-1888
    // ---------------------------------------------------------------------------

    fn evaluate_context_ui(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.ui_dirty = false;

        let mut context = {
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            std::mem::take(&mut *guard)
        };

        if context.selected_objects.is_empty() {
            context.current_state = ControlBarState::None;
            context.available_commands.clear();
            context.construction_queue.clear();
            self.build_queue_data.clear();
            self.displayed_queue_count = 0;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        }

        let multi_select = context.selected_objects.len() > 1;
        let single_drawable_id = if multi_select {
            None
        } else {
            context.selected_objects.first().copied()
        };

        if multi_select {
            context.current_state = ControlBarState::MultiSelect;
            self.rebuild_command_buttons(&mut context)?;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        }

        let Some(obj_id) = single_drawable_id else {
            context.current_state = ControlBarState::None;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        };

        let under_construction = OBJECT_REGISTRY
            .get_object(obj_id)
            .and_then(|arc| {
                arc.read()
                    .ok()
                    .map(|guard| guard.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION))
            })
            .unwrap_or(false);

        if under_construction {
            context.current_state = ControlBarState::UnderConstruction;
        } else {
            let has_command_set = OBJECT_REGISTRY
                .get_object(obj_id)
                .map(|arc| {
                    arc.read()
                        .map(|guard| guard.get_command_set_string().is_empty())
                        .map(|empty| !empty)
                        .unwrap_or(false)
                })
                .unwrap_or(false);

            let has_garrisonable_contain = OBJECT_REGISTRY
                .get_object(obj_id)
                .and_then(|arc| {
                    arc.read().ok().and_then(|guard| {
                        guard.get_contain().and_then(|contain| {
                            contain.lock().ok().map(|c| c.is_displayed_on_control_bar())
                        })
                    })
                })
                .unwrap_or(false);

            if has_garrisonable_contain && !has_command_set {
                context.current_state = ControlBarState::StructureInventory;
            } else if has_command_set {
                context.current_state = ControlBarState::Command;
            } else {
                context.current_state = ControlBarState::None;
            }
        }

        self.build_queue_data.clear();
        self.displayed_queue_count = 0;

        self.rebuild_command_buttons(&mut context)?;

        if context.current_state == ControlBarState::Command {
            self.populate_build_queue(&mut context, obj_id)?;
        }

        let mut guard = self
            .context
            .write()
            .map_err(|_| "Failed to acquire context write lock")?;
        *guard = context;

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // switchToContext - change the active context
    // C++ ControlBar.cpp:2098-2359
    // ---------------------------------------------------------------------------

    fn switch_to_context(
        &mut self,
        new_state: ControlBarState,
        _draw_id: Option<u32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut context = {
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            guard.current_state = new_state;
            std::mem::take(&mut *guard)
        };

        context.available_commands.clear();
        context.construction_queue.clear();
        self.build_queue_data.clear();
        self.displayed_queue_count = 0;

        let mut guard = self
            .context
            .write()
            .map_err(|_| "Failed to acquire context write lock")?;
        *guard = context;
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // updateContextCommand - per-frame command context update
    // C++ ControlBarCommand.cpp:678-891
    // ---------------------------------------------------------------------------

    fn get_object_production_info(obj_id: u32) -> (usize, bool) {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return (0, false);
        };
        let Ok(obj) = obj_arc.read() else {
            return (0, false);
        };
        for module in obj.get_behavior_modules() {
            if let Ok(mut guard) = module.lock() {
                if guard.get_production_update_interface().is_some() {
                    return (0, true);
                }
            }
        }
        (0, false)
    }

    fn get_first_production_progress(obj_id: u32) -> Option<f32> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return None;
        };
        let Ok(obj) = obj_arc.read() else {
            return None;
        };
        for module in obj.get_behavior_modules() {
            if let Ok(mut guard) = module.lock() {
                if let Some(pu) = guard.get_production_update_interface() {
                    let progress = pu.get_production_progress();
                    if progress > 0.0 {
                        return Some(progress);
                    }
                }
            }
        }
        None
    }

    fn map_logic_production_type(
        production_type: gamelogic::object::production::queue::ProductionType,
    ) -> ProductionType {
        match production_type {
            gamelogic::object::production::queue::ProductionType::Unit => ProductionType::Unit,
            gamelogic::object::production::queue::ProductionType::Upgrade => {
                ProductionType::Upgrade
            }
            gamelogic::object::production::queue::ProductionType::SpecialPower => {
                ProductionType::SpecialPower
            }
        }
    }

    fn map_logic_queue_type(
        production_type: gamelogic::object::production::queue::ProductionType,
    ) -> QueueProductionType {
        match production_type {
            gamelogic::object::production::queue::ProductionType::Unit => QueueProductionType::Unit,
            gamelogic::object::production::queue::ProductionType::Upgrade => {
                QueueProductionType::Upgrade
            }
            gamelogic::object::production::queue::ProductionType::SpecialPower => {
                QueueProductionType::Invalid
            }
        }
    }

    fn get_object_has_production(obj_id: u32) -> bool {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return false;
        };
        let Ok(obj) = obj_arc.read() else {
            return false;
        };
        for module in obj.get_behavior_modules() {
            if let Ok(mut guard) = module.lock() {
                if guard.get_production_update_interface().is_some() {
                    return true;
                }
            }
        }
        false
    }

    fn set_object_production_paused(obj_id: u32, paused: bool) {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj) = obj_arc.read() else {
            return;
        };

        for module in obj.get_behavior_modules() {
            let Ok(mut guard) = module.lock() else {
                continue;
            };
            let Some(production) = guard.get_production_update_interface() else {
                continue;
            };
            if paused {
                production.pause_production();
            } else {
                production.resume_production();
            }
            break;
        }
    }

    fn cancel_production_by_id(obj_id: u32, production_id: u32) {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return;
        };
        let Ok(obj) = obj_arc.read() else {
            return;
        };
        let queue_index = production_id as usize;

        for module in obj.get_behavior_modules() {
            let Ok(mut guard) = module.lock() else {
                continue;
            };
            let Some(production) = guard.get_production_update_interface() else {
                continue;
            };
            let _ = production.cancel_production(queue_index);
            break;
        }
    }

    fn update_context_command(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let obj_id = {
            let context = self
                .context
                .read()
                .map_err(|_| "Failed to acquire context read lock")?;
            context.selected_objects.first().copied()
        };

        let Some(obj_id) = obj_id else {
            return Ok(());
        };

        let has_production = Self::get_object_has_production(obj_id);

        if has_production {
            let mut context = {
                let mut guard = self
                    .context
                    .write()
                    .map_err(|_| "Failed to acquire context write lock")?;
                std::mem::take(&mut *guard)
            };
            self.populate_build_queue(&mut context, obj_id)?;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
        } else if !self.build_queue_data.is_empty() {
            self.build_queue_data.clear();
            self.displayed_queue_count = 0;
            if let Ok(mut context) = self.context.write() {
                context.construction_queue.clear();
            }
        }

        let first_progress = Self::get_first_production_progress(obj_id);

        if let Some(percent) = first_progress {
            if let Ok(mut context) = self.context.write() {
                if let Some(first_item) = context.construction_queue.first_mut() {
                    first_item.progress = percent;
                }
            }
        }

        let context = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?;
        let player_id = context.player_id;
        let buttons_snapshot: Vec<CommandButton> = context.available_commands.clone();
        drop(context);

        for button in &buttons_snapshot {
            let availability = self.get_command_availability(button, obj_id, player_id)?;
            let name = button.command_name.clone();
            if let Ok(mut context) = self.context.write() {
                if let Some(state) = context
                    .available_commands
                    .iter_mut()
                    .find(|b| b.command_name == name)
                {
                    match availability {
                        CommandAvailability::Hidden => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.visible = false;
                            }
                        }
                        CommandAvailability::Restricted => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = false;
                                bs.availability = CommandAvailability::Restricted;
                            }
                        }
                        CommandAvailability::NotReady => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = false;
                                bs.availability = CommandAvailability::NotReady;
                            }
                        }
                        CommandAvailability::CantAfford => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = false;
                                bs.availability = CommandAvailability::CantAfford;
                            }
                        }
                        CommandAvailability::Active => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = true;
                                bs.availability = CommandAvailability::Active;
                                if (button.options & CommandOption::CheckLike as u32) != 0 {
                                    bs.check_like_active = true;
                                }
                            }
                        }
                        CommandAvailability::Available => {
                            if let Some(bs) = self.button_states.get_mut(&button.command_name) {
                                bs.enabled = true;
                                bs.availability = CommandAvailability::Available;
                                if (button.options & CommandOption::CheckLike as u32) != 0 {
                                    bs.check_like_active = false;
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // ---------------------------------------------------------------------------
    // getCommandAvailability - per-button availability check
    // C++ ControlBarCommand.cpp:993-1516
    // ---------------------------------------------------------------------------

    fn get_command_availability(
        &self,
        command: &CommandButton,
        obj_id: u32,
        player_id: u32,
    ) -> Result<CommandAvailability, Box<dyn std::error::Error>> {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            return Ok(CommandAvailability::Hidden);
        };
        let Ok(obj) = obj_arc.read() else {
            return Ok(CommandAvailability::Hidden);
        };

        if obj.is_disabled() && !self.force_disabled_evaluation(command) {
            let cmd_type = command.command_type;
            if cmd_type != CommandType::Sell
                && cmd_type != CommandType::Evacuate
                && cmd_type != CommandType::DoStop
            {
                return Ok(CommandAvailability::Restricted);
            }
        }

        if (command.options & CommandOption::NeedUpgrade as u32) != 0 {
            if !command.upgrade.is_empty() {
                let player_arc = logic_player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as PlayerIndex).cloned());
                if let Some(player_arc) = player_arc {
                    if let Ok(player) = player_arc.read() {
                        let upgrade =
                            with_upgrade_center(|c| c.find_upgrade(command.upgrade.as_str()));
                        if let Some(template) = upgrade {
                            if !player.has_upgrade_complete(&template) {
                                return Ok(CommandAvailability::Restricted);
                            }
                        }
                    }
                }
            }
        }

        let queue_count = self.build_queue_data.len();
        let queue_maxed = queue_count >= MAX_BUILD_QUEUE_BUTTONS;

        if queue_maxed && (command.options & CommandOption::NotQueueable as u32) != 0 {
            return Ok(CommandAvailability::Restricted);
        }

        match command.command_type {
            CommandType::DozerConstruct => {
                if queue_maxed {
                    return Ok(CommandAvailability::Restricted);
                }
                let player_arc = logic_player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as PlayerIndex).cloned());
                if let Some(player_arc) = player_arc {
                    if let Ok(player) = player_arc.read() {
                        if !command.purchase_cost.is_empty() {
                            for (resource, cost) in &command.purchase_cost {
                                if *cost > 0
                                    && (resource.eq_ignore_ascii_case("cash")
                                        || resource.eq_ignore_ascii_case("money"))
                                    && !player.get_money().can_afford(*cost)
                                {
                                    return Ok(CommandAvailability::Restricted);
                                }
                            }
                        }
                    }
                }
                Ok(CommandAvailability::Available)
            }
            CommandType::QueueUpgrade => {
                if queue_maxed {
                    return Ok(CommandAvailability::Restricted);
                }
                let player_arc = logic_player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.get_player(player_id as PlayerIndex).cloned());
                if let Some(player_arc) = player_arc {
                    if let Ok(player) = player_arc.read() {
                        let upgrade =
                            with_upgrade_center(|c| c.find_upgrade(command.upgrade.as_str()));
                        if let Some(template) = upgrade {
                            if player.has_upgrade_complete(&template)
                                || player.has_upgrade_in_production(&template)
                            {
                                return Ok(CommandAvailability::CantAfford);
                            }
                            if !with_upgrade_center(|c| {
                                c.can_afford_upgrade(&*player, &template, false)
                            }) {
                                return Ok(CommandAvailability::Restricted);
                            }
                        } else {
                            return Ok(CommandAvailability::Restricted);
                        }
                    }
                }
                Ok(CommandAvailability::Available)
            }
            CommandType::DoStop => Ok(CommandAvailability::Available),
            CommandType::DoGuardPosition | CommandType::DoGuardObject => {
                Ok(CommandAvailability::Available)
            }
            CommandType::Sell => Ok(CommandAvailability::Available),
            CommandType::Evacuate => Ok(CommandAvailability::Available),
            CommandType::SpecialPower => Ok(CommandAvailability::Available),
            CommandType::MetaSelectMatchingUnits => Ok(CommandAvailability::Available),
            CommandType::PurchaseScience => Ok(CommandAvailability::Available),
            _ => Ok(CommandAvailability::Available),
        }
    }

    fn force_disabled_evaluation(&self, _command: &CommandButton) -> bool {
        false
    }

    // ---------------------------------------------------------------------------
    // populateBuildQueue - fill build queue from producer object
    // C++ ControlBarCommand.cpp:531-674
    // ---------------------------------------------------------------------------

    fn populate_build_queue(
        &mut self,
        context: &mut ControlBarContext,
        producer_id: u32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.build_queue_data.clear();
        context.construction_queue.clear();

        let Some(obj_arc) = OBJECT_REGISTRY.get_object(producer_id) else {
            self.displayed_queue_count = 0;
            return Ok(());
        };
        let Ok(obj) = obj_arc.read() else {
            self.displayed_queue_count = 0;
            return Ok(());
        };

        let mut found_pu = false;
        for module in obj.get_behavior_modules() {
            if let Ok(mut guard) = module.lock() {
                if let Some(pu) = guard.get_production_update_interface() {
                    found_pu = true;
                    for entry in pu.get_queue_entries() {
                        let mut cost = HashMap::new();
                        cost.insert("Supplies".to_string(), entry.cost);
                        let progress = entry.progress().clamp(0.0, 1.0);
                        context.construction_queue.push(ProductionItem {
                            template_name: entry.template_name.clone(),
                            production_type: Self::map_logic_production_type(entry.production_type),
                            progress,
                            cost,
                            build_time: entry.build_time as f32,
                        });
                        self.build_queue_data.push(BuildQueueEntry {
                            production_type: Self::map_logic_queue_type(entry.production_type),
                            production_id: entry.queue_index as u32,
                            upgrade_name: if entry.production_type
                                == gamelogic::object::production::queue::ProductionType::Upgrade
                            {
                                entry.template_name
                            } else {
                                String::new()
                            },
                        });
                    }
                    break;
                }
            }
        }

        if !found_pu {
            self.displayed_queue_count = 0;
        } else {
            self.displayed_queue_count = context.construction_queue.len();
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Command processing (click dispatch)
    // C++ ControlBar.cpp:2071-2090
    // ---------------------------------------------------------------------------

    pub fn process_command(
        &mut self,
        command_name: &str,
        source: CommandSourceType,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        let context = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?;

        if let Some(button) = context
            .available_commands
            .iter()
            .find(|b| b.command_name == command_name)
        {
            let enabled = self
                .button_states
                .get(&button.command_name)
                .map(|s| s.enabled)
                .unwrap_or(false);

            if !enabled {
                return Ok(false);
            }

            self.execute_command(button, source, &context)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn execute_command(
        &self,
        button: &CommandButton,
        source: CommandSourceType,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if Self::command_needs_target(button.options) {
            self.enter_targeting_mode(button, context)?;
            return Ok(());
        }

        if button.command_type == CommandType::PurchaseScience {
            self.execute_purchase_science(button, context)?;
            return Ok(());
        }

        if button.command_type == CommandType::MetaSelectMatchingUnits {
            self.select_all_units_of_type(button, context)?;
            return Ok(());
        }

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

        if button.command_type == CommandType::DozerConstruct && !button.object.is_empty() {
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

        let pending_payload = if button.command_type == CommandType::FireWeapon {
            self.resolve_logic_button(button)
                .map(|logic_button| logic_button.get_weapon_slot() as u32)
                .unwrap_or(source_id)
        } else {
            source_id
        };
        TheInGameUI::set_pending_command_with_visual(
            button.command_type,
            button.options,
            pending_payload,
            button.cursor_name.clone(),
            button.invalid_cursor_name.clone(),
            button.radius_cursor_type.clone(),
        );

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

        let mut command = Command::new(CommandType::PurchaseScience);
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

    fn execute_upgrade_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        _source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let upgrade_name = button.upgrade.as_str();
        let upgrade_template = with_upgrade_center(|center| center.find_upgrade(upgrade_name));
        let Some(template) = upgrade_template else {
            return Ok(());
        };

        let source_obj_id = context.selected_objects.first().copied();
        let mut command = Command::new(CommandType::QueueUpgrade);
        command.set_player_index(context.player_id as i32);

        if let Some(obj_id) = source_obj_id {
            command.append_object_id_argument(obj_id);
        }

        command.append_integer_argument(template.get_name_key() as i32);
        self.queue_command(context.player_id as i32, command)?;
        Ok(())
    }

    fn execute_production_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    fn execute_special_power_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

    fn execute_direct_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if context.selected_objects.is_empty() {
            return Ok(());
        }
        if button.command_type == CommandType::Invalid {
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
        command.set_player_index(context.player_id as i32);
        for object_id in &context.selected_objects {
            command.append_object_id_argument(*object_id);
        }
        self.queue_command(context.player_id as i32, command)?;
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Build queue cancel
    // ---------------------------------------------------------------------------

    /// Cancel a build queue item by index. Mirrors C++ CancelUnitCreate/CancelUpgradeCreate.
    pub fn cancel_build_queue_item(
        &self,
        queue_index: usize,
        context: &ControlBarContext,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        if queue_index >= self.build_queue_data.len() {
            return Ok(false);
        }

        let entry = &self.build_queue_data[queue_index];
        let Some(&producer_id) = context.selected_objects.first() else {
            return Ok(false);
        };

        Self::cancel_production_by_id(producer_id, entry.production_id);
        Ok(true)
    }

    /// Pause/resume the build queue for the selected producer.
    pub fn set_build_queue_paused(
        &self,
        paused: bool,
        context: &ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(&producer_id) = context.selected_objects.first() else {
            return Ok(());
        };

        Self::set_object_production_paused(producer_id, paused);
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Command button rebuild helpers
    // ---------------------------------------------------------------------------

    fn rebuild_command_buttons(
        &mut self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        context.available_commands.clear();

        match context.current_state {
            ControlBarState::None => {
                self.add_default_commands(context)?;
            }
            ControlBarState::Command => {
                self.add_object_commands(context)?;
            }
            ControlBarState::MultiSelect => {
                self.add_multi_select_commands(context)?;
            }
            ControlBarState::Observer => {
                self.add_observer_commands(context)?;
            }
            ControlBarState::UnderConstruction => {
                self.add_construction_commands(context)?;
            }
            ControlBarState::StructureInventory => {
                self.add_structure_inventory_commands(context)?;
            }
            ControlBarState::Beacon => {
                self.add_beacon_commands(context)?;
            }
            ControlBarState::OclTimer => {}
        }

        Ok(())
    }

    fn add_default_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(control_bar) = get_ini_control_bar() {
            for (_name, definition) in control_bar.iter_buttons().take(12) {
                let button = Self::command_from_definition(definition);
                context.available_commands.push(button);
            }
        }
        Ok(())
    }

    fn add_object_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
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
        if command_set_name.is_empty() {
            return Ok(());
        }

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
            if (button.get_options_bits() & CommandOption::ScriptOnly as u32) != 0 {
                continue;
            }
            if button.get_command_type() == CommandType::Evacuate {
                continue;
            }
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

    fn add_multi_select_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        super::control_bar_multi_select::populate_multi_select_commands(context)?;
        if context.available_commands.is_empty() {
            self.add_object_commands(context)?;
        }
        Ok(())
    }

    fn add_observer_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        super::control_bar_observer::populate_observer_commands(context)?;
        Ok(())
    }

    fn add_construction_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        super::control_bar_under_construction::populate_under_construction_commands(context)?;
        Ok(())
    }

    fn add_structure_inventory_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        super::control_bar_structure_inventory::append_structure_inventory_commands(context)?;
        Ok(())
    }

    fn add_beacon_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        super::control_bar_beacon::append_beacon_commands(context)?;
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Utility / conversion helpers
    // ---------------------------------------------------------------------------

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

    // ---------------------------------------------------------------------------
    // Context update helpers
    // ---------------------------------------------------------------------------

    fn update_context_multi_select(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn update_context_structure_inventory(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn update_context_beacon(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let selected_object = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?
            .selected_objects
            .first()
            .copied();

        let Some(object_id) = selected_object else {
            self.populate_beacon_windows(false, "")?;
            return Ok(());
        };
        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            self.populate_beacon_windows(false, "")?;
            return Ok(());
        };
        let Ok(object) = object_arc.read() else {
            self.populate_beacon_windows(false, "")?;
            return Ok(());
        };

        let position = *object.get_position();
        let player_id = object.get_controlling_player_id().map(|id| id as i32);
        let caption = player_id
            .and_then(|player_id| {
                snapshot_beacons()
                    .into_iter()
                    .find(|entry| {
                        entry.player_id == player_id && (entry.position - position).length() <= 3.0
                    })
                    .and_then(|entry| entry.text.map(|text| text.to_string()))
            })
            .unwrap_or_default();

        self.populate_beacon_windows(object.is_locally_controlled(), &caption)?;
        Ok(())
    }

    fn update_place_beacon_button_enabled(&self) {
        let Some(window_manager) = self.window_manager.as_ref() else {
            return;
        };
        let place_button = window_manager.find_window_by_name("ControlBar.wnd:ButtonPlaceBeacon");
        let enabled = self.local_player_below_beacon_limit();
        Self::apply_place_beacon_button_enabled(&place_button, enabled);
    }

    fn local_player_below_beacon_limit(&self) -> bool {
        let Some(local_player) = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
        else {
            return false;
        };
        let Ok(local_player) = local_player.read() else {
            return false;
        };
        let Some(template_name) = local_player
            .get_player_template()
            .map(|template| template.beacon_name.clone())
        else {
            return false;
        };
        if template_name.is_empty() {
            return false;
        }
        let Some(beacon_template) = TheThingFactory::find_template(&template_name) else {
            return false;
        };
        let mut count = [0];
        local_player.count_objects_by_thing_template(
            std::slice::from_ref(&beacon_template),
            false,
            false,
            &mut count,
        );
        let max_beacons = with_multiplayer_settings(|settings| settings.max_beacons_per_player);
        count[0] < max_beacons
    }

    fn apply_place_beacon_button_enabled(
        place_button: &Option<Rc<RefCell<GameWindow>>>,
        enabled: bool,
    ) {
        if let Some(window) = place_button {
            let _ = window.borrow_mut().enable(enabled);
        }
    }

    fn populate_beacon_windows(
        &self,
        locally_controlled: bool,
        caption: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let Some(window_manager) = self.window_manager.as_ref() else {
            return Ok(());
        };

        let text_entry = window_manager.find_window_by_name("ControlBar.wnd:EditBeaconText");
        let static_text =
            window_manager.find_window_by_name("ControlBar.wnd:StaticTextBeaconLabel");
        let clear_button =
            window_manager.find_window_by_name("ControlBar.wnd:ButtonClearBeaconText");

        Self::apply_beacon_window_state(
            &text_entry,
            &static_text,
            &clear_button,
            locally_controlled,
            caption,
        );
        Ok(())
    }

    fn apply_beacon_window_state(
        text_entry: &Option<Rc<RefCell<GameWindow>>>,
        static_text: &Option<Rc<RefCell<GameWindow>>>,
        clear_button: &Option<Rc<RefCell<GameWindow>>>,
        locally_controlled: bool,
        caption: &str,
    ) {
        if locally_controlled {
            if let Some(window) = text_entry {
                let mut guard = window.borrow_mut();
                let _ = guard.hide(false);
                let _ = guard.set_text(caption);
            }
            if let Some(window) = static_text {
                let _ = window.borrow_mut().hide(false);
            }
            if let Some(window) = clear_button {
                let _ = window.borrow_mut().hide(false);
            }
        } else {
            if let Some(window) = text_entry {
                let _ = window.borrow_mut().hide(true);
            }
            if let Some(window) = static_text {
                let _ = window.borrow_mut().hide(true);
            }
            if let Some(window) = clear_button {
                let _ = window.borrow_mut().hide(true);
            }
        }
    }

    fn update_context_under_construction(
        &mut self,
        _delta_time: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn update_context_ocl_timer(
        &mut self,
        _delta_time: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Selection update (external entry point)
    // ---------------------------------------------------------------------------

    pub fn update_for_selection(
        &mut self,
        selected_objects: Vec<u32>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        {
            let mut context = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            context.selected_objects = selected_objects;
        }
        self.mark_ui_dirty();
        Ok(())
    }

    pub fn set_observer_mode(&mut self, observer: bool) {
        self.observer_mode = observer;
        self.mark_ui_dirty();
    }

    fn refresh_button_states(&mut self, context: &ControlBarContext, player_id: u32) {
        let mut refreshed = HashMap::new();
        for button in &context.available_commands {
            let mut state = self
                .button_states
                .get(&button.command_name)
                .cloned()
                .unwrap_or_default();
            state.visible = true;
            refreshed.insert(button.command_name.clone(), state);
        }

        for (name, state) in refreshed.iter_mut() {
            if let Some(button) = context
                .available_commands
                .iter()
                .find(|b| &b.command_name == name)
            {
                if let Some(&first_id) = context.selected_objects.first() {
                    match self.get_command_availability(button, first_id, player_id) {
                        Ok(availability) => {
                            state.availability = availability;
                            state.enabled = matches!(
                                availability,
                                CommandAvailability::Available | CommandAvailability::Active
                            );
                            state.check_like_active = availability == CommandAvailability::Active;
                        }
                        Err(_) => {
                            state.enabled = false;
                            state.availability = CommandAvailability::Restricted;
                        }
                    }
                }
            }
        }

        self.button_states = refreshed;
    }

    pub fn get_context(&self) -> Arc<RwLock<ControlBarContext>> {
        self.context.clone()
    }

    pub fn get_build_queue_data(&self) -> &[BuildQueueEntry] {
        &self.build_queue_data
    }

    pub fn get_button_state(&self, command_name: &str) -> Option<&ButtonState> {
        self.button_states.get(command_name)
    }

    pub fn is_ui_dirty(&self) -> bool {
        self.ui_dirty
    }

    pub fn get_current_state(&self) -> ControlBarState {
        self.context
            .read()
            .map(|c| c.current_state)
            .unwrap_or(ControlBarState::None)
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
        let delta_time = Duration::from_millis(33);
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
        self.build_queue_data.clear();
        self.displayed_queue_count = 0;
        self.ui_dirty = false;
        self.flash_active = false;

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
            availability: CommandAvailability::Available,
            check_like_active: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named_window(name: &str) -> Rc<RefCell<GameWindow>> {
        let window = Rc::new(RefCell::new(GameWindow::new()));
        window.borrow_mut().set_name(name);
        window
    }

    #[test]
    fn local_beacon_windows_show_editor_and_caption_text() {
        let text_entry = Some(named_window("ControlBar.wnd:EditBeaconText"));
        let static_text = Some(named_window("ControlBar.wnd:StaticTextBeaconLabel"));
        let clear_button = Some(named_window("ControlBar.wnd:ButtonClearBeaconText"));
        for window in [&text_entry, &static_text, &clear_button]
            .into_iter()
            .flatten()
        {
            window.borrow_mut().hide(true).unwrap();
        }

        ControlBar::apply_beacon_window_state(
            &text_entry,
            &static_text,
            &clear_button,
            true,
            "Beacon Alpha",
        );

        let edit = text_entry.unwrap();
        assert!(!edit.borrow().is_hidden());
        assert_eq!(edit.borrow().get_text(), "Beacon Alpha");
        assert!(!static_text.unwrap().borrow().is_hidden());
        assert!(!clear_button.unwrap().borrow().is_hidden());
    }

    #[test]
    fn nonlocal_beacon_windows_hide_editor_label_and_clear() {
        let text_entry = Some(named_window("ControlBar.wnd:EditBeaconText"));
        let static_text = Some(named_window("ControlBar.wnd:StaticTextBeaconLabel"));
        let clear_button = Some(named_window("ControlBar.wnd:ButtonClearBeaconText"));

        ControlBar::apply_beacon_window_state(
            &text_entry,
            &static_text,
            &clear_button,
            false,
            "Enemy Beacon",
        );

        assert!(text_entry.unwrap().borrow().is_hidden());
        assert!(static_text.unwrap().borrow().is_hidden());
        assert!(clear_button.unwrap().borrow().is_hidden());
    }

    #[test]
    fn place_beacon_button_enabled_state_tracks_limit() {
        let place_button = Some(named_window("ControlBar.wnd:ButtonPlaceBeacon"));

        ControlBar::apply_place_beacon_button_enabled(&place_button, false);
        assert!(!place_button.as_ref().unwrap().borrow().is_enabled());

        ControlBar::apply_place_beacon_button_enabled(&place_button, true);
        assert!(place_button.unwrap().borrow().is_enabled());
    }
}
