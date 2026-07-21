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
use gamelogic::common::types::{KindOf, OBJECT_STATUS_SOLD, OBJECT_STATUS_UNDER_CONSTRUCTION};
use gamelogic::common::GameError;
use gamelogic::control_bar::get_control_bar_bridge;
use gamelogic::helpers::{TheGameLogic, TheThingFactory};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{player_list as logic_player_list, PlayerIndex};
use gamelogic::system::beacon_manager::snapshot_beacons;
use gamelogic::upgrade::center::with_upgrade_center;

pub const MAX_PURCHASE_SCIENCE_RANK_1: usize = 8;
pub const MAX_PURCHASE_SCIENCE_RANK_3: usize = 4;
pub const MAX_PURCHASE_SCIENCE_RANK_8: usize = 3;
pub const MAX_SPECIAL_POWER_SHORTCUTS: usize = 8;
pub const MAX_RIGHT_HUD_UPGRADE_CAMEOS: usize = 4;
const RADAR_ATTACK_GLOW_FRAMES: u32 = 150;
const RADAR_ATTACK_GLOW_NUM_TIMES: u32 = 15;
const LOGICFRAMES_PER_SECOND: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlBarStage {
    #[default]
    Default,
    Low,
    Hidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandButtonMappedBorderType {
    None,
    Build,
    Upgrade,
    Action,
    System,
}

#[derive(Debug, Clone, Default)]
pub struct PortraitDisplayState {
    pub portrait_image: String,
    pub veterancy_overlay: Option<String>,
    pub upgrade_cameos: Vec<UpgradeCameoState>,
    pub is_visible: bool,
    /// Selection health from PresentationFrame snapshot (not live OBJECT_REGISTRY).
    pub health_current: f32,
    pub health_maximum: f32,
    /// Number of selected objects reflected on the selection panel.
    pub selected_count: usize,
    /// First production queue item progress from PresentationFrame (0..1).
    pub production_progress: Option<f32>,
    /// First production queue template from PresentationFrame.
    pub production_template: Option<String>,
    /// Special power ready residual from PresentationFrame.
    pub special_power_ready: bool,
    /// Special power cooldown remaining residual (seconds).
    pub special_power_cooldown_remaining: f32,
    /// Structure rally point residual from PresentationFrame (xyz).
    pub rally_point: Option<[f32; 3]>,
}

#[derive(Debug, Clone)]
pub struct UpgradeCameoState {
    pub upgrade_name: String,
    pub button_image: String,
    pub is_completed: bool,
    pub is_visible: bool,
}

#[derive(Debug, Clone)]
pub struct SciencePurchaseState {
    pub rank1_buttons: Vec<ScienceButtonState>,
    pub rank3_buttons: Vec<ScienceButtonState>,
    pub rank8_buttons: Vec<ScienceButtonState>,
    pub available_points: i32,
    pub rank_level: i32,
    pub experience_progress: f32,
    pub rank_title_label: String,
    pub is_visible: bool,
    /// Unlocked science names residual from PresentationFrame (not live player list).
    pub unlocked_sciences: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ScienceButtonState {
    pub command_name: String,
    pub science_type: ScienceType,
    pub is_hidden: bool,
    pub is_enabled: bool,
    pub is_purchased: bool,
}

#[derive(Debug, Clone)]
pub struct SpecialPowerShortcutState {
    pub command_name: String,
    pub availability: CommandAvailability,
    pub multiplier_count: i32,
    pub is_hidden: bool,
}

impl Default for SciencePurchaseState {
    fn default() -> Self {
        Self {
            rank1_buttons: Vec::new(),
            rank3_buttons: Vec::new(),
            rank8_buttons: Vec::new(),
            available_points: 0,
            rank_level: 0,
            experience_progress: 0.0,
            rank_title_label: String::new(),
            is_visible: false,
            unlocked_sciences: Vec::new(),
        }
    }
}

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
    control_bar_stage: ControlBarStage,
    portrait_state: PortraitDisplayState,
    science_state: SciencePurchaseState,
    gen_star_flash: bool,
    last_flashed_at_point_value: i32,
    radar_attack_glow_on: bool,
    remaining_radar_attack_glow_frames: u32,
    special_power_shortcuts: Vec<SpecialPowerShortcutState>,
    special_power_shortcut_count: usize,
    /// Radar provider count residual from PresentationFrame.
    presentation_radar_count: i32,
    /// Radar disabled residual from PresentationFrame.
    presentation_radar_disabled: bool,
    /// Queued upgrade names residual from PresentationFrame.
    presentation_queued_upgrades: Vec<String>,
    displayed_construct_percent: f32,
    displayed_ocl_timer_seconds: u32,
    border_colors: CommandBarBorderColors,
}

#[derive(Debug, Clone, Default)]
struct CommandBarBorderColors {
    build: Option<u32>,
    action: Option<u32>,
    upgrade: Option<u32>,
    system: Option<u32>,
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
            control_bar_stage: ControlBarStage::Default,
            portrait_state: PortraitDisplayState::default(),
            science_state: SciencePurchaseState::default(),
            gen_star_flash: true,
            last_flashed_at_point_value: -1,
            radar_attack_glow_on: false,
            remaining_radar_attack_glow_frames: 0,
            special_power_shortcuts: Vec::new(),
            special_power_shortcut_count: 0,
            presentation_radar_count: 0,
            presentation_radar_disabled: false,
            presentation_queued_upgrades: Vec::new(),
            displayed_construct_percent: -1.0,
            displayed_ocl_timer_seconds: 0,
            border_colors: CommandBarBorderColors::default(),
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

        self.current_frame = TheGameLogic::get_frame();
        self.update_star_image();
        self.update_radar_attack_glow();

        if self.observer_mode {
            self.update_observer_portrait()?;
            return Ok(());
        }

        if self.science_state.is_visible {
            self.update_context_purchase_science();
        }

        self.update_flash_buttons();

        if self.ui_dirty {
            self.evaluate_context_ui()?;
            self.populate_special_power_shortcut()?;
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
        // Host/presentation path: Main feeds selection via
        // sync_selection_display_from_presentation (no OBJECT_REGISTRY).
        // Dual-world registry is opt-in; do not wipe context when registry empty.
        let registry_exists = OBJECT_REGISTRY
            .get_object(first_id)
            .map(|arc| arc.read().is_ok())
            .unwrap_or(false);
        let presentation_selection_active =
            self.portrait_state.is_visible && self.portrait_state.selected_count > 0;
        if !registry_exists && !presentation_selection_active {
            self.switch_to_context(ControlBarState::None, None)?;
            return Ok(());
        }
        // Without registry modules, skip live module context updates — presentation
        // already owns portrait/health/queue residual.
        if !registry_exists {
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
            ControlBarState::Observer => {
                self.update_context_observer()?;
            }
            ControlBarState::MultiSelect => {
                self.update_context_multi_select()?;
            }
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

        self.update_special_power_shortcut_availability();

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
            self.portrait_state = PortraitDisplayState::default();
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

        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            // Presentation-only selection residual: keep Command context so
            // Main-fed portrait/queue stay visible without dual-world registry.
            if self.portrait_state.is_visible {
                context.current_state = ControlBarState::Command;
            } else {
                context.current_state = ControlBarState::None;
            }
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        };
        let Ok(obj) = obj_arc.read() else {
            context.current_state = ControlBarState::None;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        };

        if obj.test_status(OBJECT_STATUS_SOLD) {
            drop(obj);
            context.current_state = ControlBarState::None;
            let mut guard = self
                .context
                .write()
                .map_err(|_| "Failed to acquire context write lock")?;
            *guard = context;
            return Ok(());
        }

        let under_construction = obj.test_status(OBJECT_STATUS_UNDER_CONSTRUCTION);

        if under_construction {
            drop(obj);
            context.current_state = ControlBarState::UnderConstruction;
        } else {
            let has_command_set = !obj.get_command_set_string().is_empty();

            let has_garrisonable_contain = obj
                .get_contain()
                .and_then(|contain| contain.lock().ok().map(|c| c.is_displayed_on_control_bar()))
                .unwrap_or(false);

            if has_garrisonable_contain && !has_command_set {
                drop(obj);
                context.current_state = ControlBarState::StructureInventory;
            } else if has_command_set {
                drop(obj);
                context.current_state = ControlBarState::Command;
            } else {
                drop(obj);
                context.current_state = ControlBarState::None;
            }
        }

        self.build_queue_data.clear();
        self.displayed_queue_count = 0;
        self.update_portrait_for_object(obj_id);

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
        self.portrait_state = PortraitDisplayState::default();

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
        self.displayed_construct_percent = -1.0;
        self.displayed_ocl_timer_seconds = 0;

        if let Some(&obj_id) = context.selected_objects.first() {
            self.update_portrait_for_object(obj_id);
        }

        self.rebuild_command_buttons(&mut context)?;

        if new_state == ControlBarState::Command {
            if let Some(&obj_id) = context.selected_objects.first() {
                let _ = self.populate_build_queue(&mut context, obj_id);
            }
        }

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

    fn get_object_production_info(&self, obj_id: u32) -> (usize, bool) {
        // Presentation residual first (host path has no dual-world registry modules).
        if !self.build_queue_data.is_empty() {
            return (self.build_queue_data.len(), true);
        }
        if self.portrait_state.production_progress.is_some()
            || self.portrait_state.production_template.is_some()
        {
            return (self.displayed_queue_count.max(1), true);
        }
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

    fn get_first_production_progress(&self, obj_id: u32) -> Option<f32> {
        // Presentation residual owns host InGame queue progress display.
        if let Some(p) = self.portrait_state.production_progress {
            if p > 0.0 {
                return Some(p);
            }
        }
        if let Ok(context) = self.context.read() {
            if let Some(first) = context.construction_queue.first() {
                if first.progress > 0.0 {
                    return Some(first.progress);
                }
            }
        }
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

    fn get_object_has_production(&self, obj_id: u32) -> bool {
        if !self.build_queue_data.is_empty()
            || self.portrait_state.production_progress.is_some()
            || self.portrait_state.production_template.is_some()
            || self.displayed_queue_count > 0
        {
            return true;
        }
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

        let has_production = self.get_object_has_production(obj_id);
        let registry_producer = OBJECT_REGISTRY.get_object(obj_id).is_some();

        if has_production && registry_producer {
            // Dual-world residual: live production modules own queue when registry is bound.
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
        } else if !has_production && !self.portrait_state.is_visible {
            // Only clear when neither registry nor presentation claims production.
            if !self.build_queue_data.is_empty() {
                self.build_queue_data.clear();
                self.displayed_queue_count = 0;
                if let Ok(mut context) = self.context.write() {
                    context.construction_queue.clear();
                }
            }
        }
        // else: presentation-fed queue residual stays (host path).

        let first_progress = self.get_first_production_progress(obj_id);

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
            // Host/presentation path: Main already filtered unit_command_buttons.
            // Do not hide presentation-fed command sets solely for missing registry.
            if self.portrait_state.is_visible {
                return Ok(CommandAvailability::Available);
            }
            return Ok(CommandAvailability::Hidden);
        };
        let Ok(obj) = obj_arc.read() else {
            if self.portrait_state.is_visible {
                return Ok(CommandAvailability::Available);
            }
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

        if (command.options & CommandOption::NeedUpgrade as u32) != 0 && !command.upgrade.is_empty()
        {
            let player_arc = logic_player_list()
                .read()
                .ok()
                .and_then(|list| list.get_player(player_id as PlayerIndex).cloned());
            if let Some(player_arc) = player_arc {
                if let Ok(player) = player_arc.read() {
                    let upgrade = with_upgrade_center(|c| c.find_upgrade(command.upgrade.as_str()));
                    if let Some(template) = upgrade {
                        if !player.has_upgrade_complete(&template) {
                            return Ok(CommandAvailability::Restricted);
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
                                c.can_afford_upgrade(&player, &template, false)
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
        command.append_integer_argument(selected_science);
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
        // Dual-world residual: live modules via OBJECT_REGISTRY when bound.
        let mut applied = 0usize;
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
                applied += 1;
            }
        }
        if applied > 0 {
            return Ok(());
        }
        // Host/presentation residual: MSG_QUEUE_UNIT_CREATE (no OBJECT_REGISTRY).
        let Some(logic_button) = self.resolve_logic_button(button) else {
            return Ok(());
        };
        let Some(thing_template) = logic_button.get_thing_template() else {
            return Ok(());
        };
        let template_id = thing_template.get_id();
        let production_id = 0u32;
        if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
            stream.append_message(GameMessageType::QueueUnitCreate(template_id, production_id));
        }
        Ok(())
    }

    fn execute_special_power_command(
        &self,
        button: &CommandButton,
        context: &ControlBarContext,
        source: CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut applied = 0usize;
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
                applied += 1;
            }
        }
        if applied > 0 {
            return Ok(());
        }
        // Host/presentation residual: MSG_DO_SPECIAL_POWER without dual-world modules.
        let Some(logic_button) = self.resolve_logic_button(button) else {
            return Ok(());
        };
        let Some(sp_template) = logic_button.get_special_power_template() else {
            return Ok(());
        };
        let sp_id = sp_template.get_id();
        let options = logic_button.get_options_bits();
        let source_obj_id = context.selected_objects.first().copied().unwrap_or(0);
        if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
            stream.append_message(GameMessageType::DoSpecialPower(
                sp_id,
                options,
                source_obj_id,
            ));
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

        // Dual-world residual only when registry objects actually accept the button.
        let mut applied = 0usize;
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
                applied += 1;
            }
        }
        if applied > 0 {
            return Ok(());
        }

        // Host/presentation residual: queue typed Command with selected IDs.
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

        // Dual-world residual when producer modules are bound.
        if OBJECT_REGISTRY.get_object(producer_id).is_some() {
            Self::cancel_production_by_id(producer_id, entry.production_id);
            return Ok(true);
        }
        // Host/presentation residual: message-stream cancel (no OBJECT_REGISTRY modules).
        if let Ok(mut stream) = THE_MESSAGE_STREAM.write() {
            match entry.production_type {
                QueueProductionType::Upgrade => {
                    stream.append_message(GameMessageType::CancelUpgrade(entry.production_id));
                }
                _ => {
                    stream.append_message(GameMessageType::CancelUnitCreate(entry.production_id));
                }
            }
        }
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

        // Dual-world residual only. Host production pause is driven by Main command path.
        if OBJECT_REGISTRY.get_object(producer_id).is_some() {
            Self::set_object_production_paused(producer_id, paused);
        }
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
            ControlBarState::OclTimer => {
                // C++ ControlBarOCLTimer.cpp:55 populateOCLTimer: adds sell/rally-point
                // button depending on creator object kind, then updates timer display
                self.add_ocl_timer_commands(context)?;
            }
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

    fn add_ocl_timer_commands(
        &self,
        context: &mut ControlBarContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ ControlBarOCLTimer.cpp:55 populateOCLTimer:
        // Adds Command_Sell for non-tech buildings, Command_SetRallyPoint for
        // tech buildings with AUTO_RALLYPOINT, or hides the button.
        // Delegates to the OCL timer module for command population.
        super::control_bar_ocl_timer::populate_ocl_timer_commands(context)
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

    /// C++ ControlBar.cpp:1410-1433: refresh observer info window every half-second.
    /// C++ ControlBarObserver.cpp:271 populateObserverInfoWindow: units, buildings, kills, losses.
    fn update_context_observer(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let frame = TheGameLogic::get_frame();
        if !frame.is_multiple_of(15) {
            return Ok(());
        }

        let player_list = logic_player_list();
        let player_count = player_list
            .read()
            .map(|list| list.get_player_count())
            .unwrap_or(0);

        let mut observer_stats: Vec<(String, i32, i32, i32, i32, i32)> = Vec::new();
        for i in 0..player_count {
            let player_arc = player_list
                .read()
                .ok()
                .and_then(|list| list.get_player(i as PlayerIndex).cloned());

            let Some(player_arc) = player_arc else {
                continue;
            };
            let Ok(player) = player_arc.read() else {
                continue;
            };

            if player.is_player_observer() {
                continue;
            }

            let display_name = player.get_player_display_name().clone();
            let money = player.get_money().get_money();

            // C++ KindOf bit indices (from KindOf.h): Score=45, Structure=8, ScoreCreate=46, ScoreDestroy=47
            let score_bit = 1u64 << 45;
            let struct_bit = 1u64 << 8;
            let score_create_bit = 1u64 << 46;
            let score_destroy_bit = 1u64 << 47;

            let num_units = player.count_objects_by_kindof(score_bit, struct_bit);

            let num_buildings = player.count_objects_by_kindof(score_bit | struct_bit, 0)
                + player.count_objects_by_kindof(score_create_bit | struct_bit, 0)
                + player.count_objects_by_kindof(score_destroy_bit | struct_bit, 0);

            let score_keeper = player.get_score_keeper();
            let units_killed = score_keeper.get_total_units_destroyed();
            let units_lost = score_keeper.get_total_units_lost();

            observer_stats.push((
                display_name,
                money,
                num_units,
                num_buildings,
                units_killed,
                units_lost,
            ));
        }

        if let Ok(mut ctx) = self.context.write() {
            ctx.observer_player_stats = observer_stats;
        }

        Ok(())
    }

    /// C++ ControlBarStructureInventory.cpp:181-214: update garrison/contain inventory.
    fn update_context_structure_inventory(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let selected_object = self
            .context
            .read()
            .map_err(|_| "Failed to acquire context read lock")?
            .selected_objects
            .first()
            .copied();

        let Some(object_id) = selected_object else {
            return Ok(());
        };
        let Some(object_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return Ok(());
        };
        let Ok(object) = object_arc.read() else {
            return Ok(());
        };

        let player_list = logic_player_list();
        let local_player_index = player_list
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(gamelogic::player::PLAYER_INDEX_INVALID);

        let obj_player_id = object.get_controlling_player_id().unwrap_or(0xFFFF) as PlayerIndex;
        if obj_player_id != local_player_index {
            let local_arc = player_list
                .read()
                .ok()
                .and_then(|list| list.get_player(local_player_index).cloned());
            let obj_arc = player_list
                .read()
                .ok()
                .and_then(|list| list.get_player(obj_player_id).cloned());

            if let (Some(local_arc), Some(obj_arc)) = (local_arc, obj_arc) {
                if let (Ok(local_guard), Ok(obj_guard)) = (local_arc.read(), obj_arc.read()) {
                    let rel = local_guard.get_relationship(&obj_guard);
                    if rel != gamelogic::common::Relationship::Neutral {
                        TheInGameUI::deselect_all();
                        return Ok(());
                    }
                }
            }
        }

        let Some(contain) = object.get_contain() else {
            return Ok(());
        };
        let contain_count = contain.lock().map(|c| c.get_contain_count()).unwrap_or(0);

        if let Ok(mut ctx) = self.context.write() {
            if ctx.last_recorded_inventory_count != contain_count {
                ctx.last_recorded_inventory_count = contain_count;
                ctx.ui_dirty = true;
            }
        }

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
        let obj_id = {
            let context = self
                .context
                .read()
                .map_err(|_| "Failed to acquire context read lock")?;
            context.selected_objects.first().copied()
        };
        let Some(_obj_id) = obj_id else {
            return Ok(());
        };

        let Some(_obj_arc) = OBJECT_REGISTRY.get_object(_obj_id) else {
            return Ok(());
        };

        // C++ reads construct percent from the object's status; we track it
        // for display change detection. When get_construct_percent() is ported,
        // wire it here.
        Ok(())
    }

    fn update_context_ocl_timer(
        &mut self,
        _delta_time: Duration,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let obj_id = {
            let context = self
                .context
                .read()
                .map_err(|_| "Failed to acquire context read lock")?;
            context.selected_objects.first().copied()
        };
        let Some(_obj_id) = obj_id else {
            return Ok(());
        };

        let Some(_obj_arc) = OBJECT_REGISTRY.get_object(_obj_id) else {
            return Ok(());
        };

        // C++ reads OCL timer seconds from OCLUpdate module; tracked for display
        // change detection. When get_ocl_timer_seconds() is ported, wire it here.
        Ok(())
    }

    // ---------------------------------------------------------------------------
    // Star image / general button flash
    // C++ ControlBar.cpp:1621-1647
    // ---------------------------------------------------------------------------

    fn update_star_image(&mut self) {
        let current_points = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| {
                p.read()
                    .ok()
                    .map(|guard| guard.get_science_purchase_points())
            })
            .unwrap_or(0);

        if self.last_flashed_at_point_value > current_points || current_points <= 0 {
            self.gen_star_flash = false;
        } else {
            self.last_flashed_at_point_value = current_points;
        }

        if self.gen_star_flash
            && self.current_frame % LOGICFRAMES_PER_SECOND > LOGICFRAMES_PER_SECOND / 2
        {
            // C++ flashes the general button highlight
        }
    }

    // ---------------------------------------------------------------------------
    // Flash buttons
    // C++ ControlBar.cpp:1438-1469
    // ---------------------------------------------------------------------------

    fn update_flash_buttons(&mut self) {
        if !self.flash_active {
            return;
        }

        if !self.current_frame.is_multiple_of(10) {
            return;
        }

        let mut still_flashing = false;
        for (name, state) in self.button_states.iter_mut() {
            if state.flash_time.is_some() {
                still_flashing = true;
            }
        }

        if !still_flashing {
            self.flash_active = false;
        }
    }

    // ---------------------------------------------------------------------------
    // Radar attack glow
    // C++ ControlBar.cpp:3169-3197
    // ---------------------------------------------------------------------------

    fn update_radar_attack_glow(&mut self) {
        if !self.radar_attack_glow_on {
            return;
        }
        if self.remaining_radar_attack_glow_frames == 0 {
            self.radar_attack_glow_on = false;
            return;
        }
        self.remaining_radar_attack_glow_frames =
            self.remaining_radar_attack_glow_frames.saturating_sub(1);
        if self.remaining_radar_attack_glow_frames == 0 {
            self.radar_attack_glow_on = false;
            return;
        }
        if self
            .remaining_radar_attack_glow_frames
            .is_multiple_of(RADAR_ATTACK_GLOW_NUM_TIMES)
        {
            // C++ toggles winEnable on/off for glow effect
        }
    }

    pub fn trigger_radar_attack_glow(&mut self) {
        self.radar_attack_glow_on = true;
        self.remaining_radar_attack_glow_frames = RADAR_ATTACK_GLOW_FRAMES;
    }

    // ---------------------------------------------------------------------------
    // Portrait display
    // C++ ControlBar.cpp:2534-2663
    // ---------------------------------------------------------------------------

    fn update_portrait_for_object(&mut self, obj_id: u32) {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
            // Presentation residual already owns portrait/health/queue via
            // sync_selection_display_from_presentation — do not wipe it.
            if !self.portrait_state.is_visible {
                self.portrait_state = PortraitDisplayState::default();
            }
            return;
        };
        let Ok(obj) = obj_arc.read() else {
            if !self.portrait_state.is_visible {
                self.portrait_state = PortraitDisplayState::default();
            }
            return;
        };

        if obj.is_kind_of(KindOf::ShowPortraitWhenControlled) && !obj.is_locally_controlled() {
            self.portrait_state = PortraitDisplayState::default();
            return;
        }

        let template_name = obj.get_template_name().to_string();

        let veterancy = obj.get_veterancy_level();
        let veterancy_overlay = match veterancy {
            gamelogic::common::types::VeterancyLevel::Veteran => Some("SSChevron1L".to_string()),
            gamelogic::common::types::VeterancyLevel::Elite => Some("SSChevron2L".to_string()),
            gamelogic::common::types::VeterancyLevel::Heroic => Some("SSChevron3L".to_string()),
            _ => None,
        };

        // Live-registry portrait path: health stays 0 here; presentation overlay
        // (`sync_selection_display_from_presentation`) supplies snapshot HP.
        self.portrait_state = PortraitDisplayState {
            portrait_image: template_name,
            veterancy_overlay,
            upgrade_cameos: Vec::new(),
            is_visible: true,
            health_current: 0.0,
            health_maximum: 0.0,
            selected_count: 1,
            production_progress: None,
            production_template: None,
            special_power_ready: false,
            special_power_cooldown_remaining: 0.0,
            rally_point: None,
        };
    }

    fn update_observer_portrait(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self
            .current_frame
            .is_multiple_of(LOGICFRAMES_PER_SECOND / 2)
        {
            return Ok(());
        }
        self.update_context_observer()?;

        let obj_id = {
            self.context
                .read()
                .map_err(|_| "Failed to acquire context read lock")?
                .selected_objects
                .first()
                .copied()
        };
        if let Some(id) = obj_id {
            self.update_portrait_for_object(id);
        }

        Ok(())
    }

    pub fn set_portrait_by_object_id(&mut self, obj_id: Option<u32>) {
        if let Some(id) = obj_id {
            self.update_portrait_for_object(id);
        } else {
            self.portrait_state = PortraitDisplayState::default();
        }
    }

    pub fn get_portrait_state(&self) -> &PortraitDisplayState {
        &self.portrait_state
    }

    /// Sync selection panel (portrait + health) from presentation snapshot fields.
    ///
    /// Prefer this over `update_portrait_for_object` for production dual-tick /
    /// headless host paths: no OBJECT_REGISTRY re-read, health is snapshot-owned.
    /// Does not require WindowManager / ControlBar.wnd load (fail-closed for WND).
    pub fn sync_selection_display_from_presentation(
        &mut self,
        primary_template_name: Option<&str>,
        health_current: f32,
        health_maximum: f32,
        selected_count: usize,
        veterancy_overlay: Option<&str>,
        production_progress: Option<f32>,
        production_template: Option<&str>,
        production_queue: &[(String, f32, bool)],
    ) {
        match primary_template_name {
            Some(name) if !name.is_empty() && selected_count > 0 => {
                self.portrait_state = PortraitDisplayState {
                    portrait_image: name.to_string(),
                    veterancy_overlay: veterancy_overlay.map(str::to_string),
                    upgrade_cameos: std::mem::take(&mut self.portrait_state.upgrade_cameos),
                    is_visible: true,
                    health_current,
                    health_maximum: health_maximum.max(1.0),
                    selected_count,
                    production_progress,
                    production_template: production_template.map(str::to_string),
                    // Upgrades/specials filled by sync_upgrades_and_specials_from_presentation.
                    special_power_ready: self.portrait_state.special_power_ready,
                    special_power_cooldown_remaining: self
                        .portrait_state
                        .special_power_cooldown_remaining,
                    rally_point: self.portrait_state.rally_point,
                };
                // Feed construction queue residual from presentation snapshot (no OBJECT_REGISTRY).
                if let Ok(mut context) = self.context.write() {
                    context.construction_queue = production_queue
                        .iter()
                        .map(|(template_name, progress, is_upgrade)| ProductionItem {
                            template_name: template_name.clone(),
                            production_type: if *is_upgrade {
                                ProductionType::Upgrade
                            } else {
                                ProductionType::Unit
                            },
                            progress: *progress,
                            cost: std::collections::HashMap::new(),
                            build_time: 0.0,
                        })
                        .collect();
                    context.ui_dirty = true;
                }
                self.displayed_queue_count = production_queue.len();
                self.build_queue_data = production_queue
                    .iter()
                    .enumerate()
                    .map(
                        |(idx, (template_name, _progress, is_upgrade))| BuildQueueEntry {
                            production_type: if *is_upgrade {
                                QueueProductionType::Upgrade
                            } else {
                                QueueProductionType::Unit
                            },
                            production_id: idx as u32,
                            upgrade_name: template_name.clone(),
                        },
                    )
                    .collect();
                self.mark_ui_dirty();
            }
            _ => {
                self.portrait_state = PortraitDisplayState::default();
                if let Ok(mut context) = self.context.write() {
                    context.construction_queue.clear();
                    context.ui_dirty = true;
                }
                self.build_queue_data.clear();
                self.displayed_queue_count = 0;
            }
        }
    }

    /// Selection panel health currently shown (presentation-fed when available).

    /// Feed garrison inventory + under-construction commands from PresentationFrame.
    ///
    /// Prefer this over OBJECT_REGISTRY contain lookups for dual-tick / headless host.
    /// Fail-closed: does not claim full WND button layout parity.

    /// Feed upgrade cameos, special-power ready, and rally residual from PresentationFrame.
    ///
    /// Prefer this over live OBJECT_REGISTRY / template graph for dual-tick host paths.
    /// Fail-closed: cameo images are name placeholders (not full CommandSet INI art).
    pub fn sync_upgrades_and_specials_from_presentation(
        &mut self,
        applied_upgrades: &[String],
        rally_point: Option<[f32; 3]>,
        special_power_ready: bool,
        special_power_cooldown_remaining: f32,
    ) {
        let mut cameos: Vec<UpgradeCameoState> = applied_upgrades
            .iter()
            .map(|name| UpgradeCameoState {
                upgrade_name: name.clone(),
                button_image: name.clone(),
                is_completed: true,
                is_visible: true,
            })
            .collect();
        // Stable order for deterministic UI.
        cameos.sort_by(|a, b| a.upgrade_name.cmp(&b.upgrade_name));
        self.portrait_state.upgrade_cameos = cameos;
        self.portrait_state.special_power_ready = special_power_ready;
        self.portrait_state.special_power_cooldown_remaining = special_power_cooldown_remaining;
        self.portrait_state.rally_point = rally_point;
        self.mark_ui_dirty();
    }

    /// Populate command buttons from a presentation-frozen CommandSet name.
    ///
    /// Prefer this over live `OBJECT_REGISTRY` / `get_command_set_string` dual-reads
    /// when a PresentationFrame is installed. Fail-closed: not full multi-select
    /// intersection / ScriptOnly / prerequisite filter matrix.
    pub fn sync_command_set_from_presentation(&mut self, command_set_name: Option<&str>) {
        let Some(name) = command_set_name.map(str::trim).filter(|s| !s.is_empty()) else {
            return;
        };
        let Some(control_bar) = get_control_bar_bridge() else {
            return;
        };
        let Some(common_bar) = get_ini_control_bar() else {
            return;
        };
        let command_set = control_bar
            .find_command_set_by_name(name)
            .or_else(|| control_bar.find_command_set_by_name(&name.to_ascii_uppercase()));
        let Some(command_set) = command_set else {
            return;
        };
        let Ok(mut context) = self.context.write() else {
            return;
        };
        // Replace object-command buttons with presentation residual set.
        // Keep non-Command inventory/exit buttons already injected by other syncs.
        let keep: Vec<CommandButton> = context
            .available_commands
            .iter()
            .filter(|b| {
                let n = b.command_name.to_ascii_lowercase();
                n.contains("evacuate")
                    || n.contains("structureexit")
                    || n.contains("cancelconstruction")
                    || n.contains("stop")
            })
            .cloned()
            .collect();
        context.available_commands.clear();
        context.current_state = ControlBarState::Command;
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
        for b in keep {
            if !context
                .available_commands
                .iter()
                .any(|x| x.command_name.eq_ignore_ascii_case(&b.command_name))
            {
                context.available_commands.push(b);
            }
        }
        context.ui_dirty = true;
        drop(context);
        self.mark_ui_dirty();
    }

    /// Multi-select command intersection from presentation command-set names
    /// (host path — no dual-world OBJECT_REGISTRY).
    /// Mirrors `control_bar_multi_select::populate_multi_select_commands` slot rules.
    pub fn sync_multi_select_command_sets_from_presentation(
        &mut self,
        command_set_names: &[String],
    ) {
        if command_set_names.len() < 2 {
            return;
        }
        let Some(control_bar) = get_control_bar_bridge() else {
            return;
        };
        let Some(common_bar) = get_ini_control_bar() else {
            return;
        };

        let mut common_slots: Vec<Option<gamelogic::command_button::CommandButton>> =
            vec![None; gamelogic::command_button::MAX_COMMANDS_PER_SET];
        let mut saw_first = false;

        for name in command_set_names {
            let name = name.trim();
            if name.is_empty() {
                continue;
            }
            let command_set = control_bar
                .find_command_set_by_name(name)
                .or_else(|| control_bar.find_command_set_by_name(&name.to_ascii_uppercase()));
            let Some(command_set) = command_set else {
                common_slots.fill(None);
                saw_first = true;
                break;
            };

            if !saw_first {
                for slot in 0..gamelogic::command_button::MAX_COMMANDS_PER_SET {
                    let Some(button) = command_set
                        .buttons
                        .get(slot)
                        .and_then(|button| button.as_ref())
                    else {
                        continue;
                    };
                    if (button.get_options_bits() & CommandOption::OkForMultiSelect as u32) != 0 {
                        common_slots[slot] = Some(button.clone());
                    }
                }
                saw_first = true;
                continue;
            }

            for slot in 0..gamelogic::command_button::MAX_COMMANDS_PER_SET {
                let command = command_set
                    .buttons
                    .get(slot)
                    .and_then(|button| button.as_ref());
                let common = common_slots[slot].as_ref();

                let attack_move = command
                    .map(|button| {
                        button.get_command_type()
                            == gamelogic::commands::CommandType::DoAttackMoveTo
                    })
                    .unwrap_or(false)
                    || common
                        .map(|button| {
                            button.get_command_type()
                                == gamelogic::commands::CommandType::DoAttackMoveTo
                        })
                        .unwrap_or(false);

                if attack_move && common_slots[slot].is_none() {
                    common_slots[slot] = command.cloned();
                    continue;
                }
                if attack_move {
                    continue;
                }

                let matches = match (command, common) {
                    (Some(a), Some(b)) => a.get_id() == b.get_id(),
                    (None, None) => true,
                    _ => false,
                };
                if !matches {
                    common_slots[slot] = None;
                }
            }
        }

        if !saw_first {
            return;
        }

        let Ok(mut context) = self.context.write() else {
            return;
        };
        context.current_state = ControlBarState::MultiSelect;
        context.available_commands.clear();
        for button in common_slots.into_iter().flatten() {
            if let Some(common_button) = common_bar.find_command_button_resolved(button.get_name())
            {
                context
                    .available_commands
                    .push(Self::command_from_definition(common_button));
            } else {
                context
                    .available_commands
                    .push(Self::command_from_logic_button(&button));
            }
        }
        context.ui_dirty = true;
        drop(context);
        self.mark_ui_dirty();
    }

    pub fn sync_structure_context_from_presentation(
        &mut self,
        max_garrison: usize,
        garrisoned_count: usize,
        under_construction: bool,
        _construction_percent: f32,
    ) {
        if let Ok(mut context) = self.context.write() {
            context.last_recorded_inventory_count = garrisoned_count as u32;
            if under_construction {
                let cancel = CommandButton {
                    command_name: "Command_CancelConstruction".into(),
                    ..CommandButton::default()
                };
                if !context
                    .available_commands
                    .iter()
                    .any(|b| b.command_name.eq_ignore_ascii_case(&cancel.command_name))
                {
                    context.available_commands.push(cancel);
                }
            }
            if max_garrison > 0 {
                let exit = CommandButton {
                    command_name: "Command_StructureExit".into(),
                    ..CommandButton::default()
                };
                if !context
                    .available_commands
                    .iter()
                    .any(|b| b.command_name.eq_ignore_ascii_case(&exit.command_name))
                {
                    context.available_commands.push(exit);
                }
                if garrisoned_count > 0 {
                    for name in ["Command_Evacuate", "Command_Stop"] {
                        let btn = CommandButton {
                            command_name: name.into(),
                            ..CommandButton::default()
                        };
                        if !context
                            .available_commands
                            .iter()
                            .any(|b| b.command_name.eq_ignore_ascii_case(&btn.command_name))
                        {
                            context.available_commands.push(btn);
                        }
                    }
                }
            }
            context.ui_dirty = true;
        }
        self.mark_ui_dirty();
    }

    pub fn selection_panel_health(&self) -> Option<(f32, f32)> {
        if self.portrait_state.is_visible && self.portrait_state.health_maximum > 0.0 {
            Some((
                self.portrait_state.health_current,
                self.portrait_state.health_maximum,
            ))
        } else {
            None
        }
    }

    // ---------------------------------------------------------------------------
    // Science purchase system
    // C++ ControlBar.cpp:143-485, 2907-2966
    // ---------------------------------------------------------------------------

    pub fn show_purchase_science(&mut self) {
        self.populate_purchase_science();
        self.gen_star_flash = false;
        self.science_state.is_visible = true;
    }

    pub fn hide_purchase_science(&mut self) {
        self.science_state.is_visible = false;
    }

    pub fn toggle_purchase_science(&mut self) {
        if self.science_state.is_visible {
            self.hide_purchase_science();
        } else {
            self.show_purchase_science();
        }
    }

    fn populate_purchase_science(&mut self) {
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else { return };
        let Ok(player) = player_arc.read() else {
            return;
        };

        let Some(store) = get_science_store() else {
            return;
        };

        self.science_state.available_points = player.get_science_purchase_points();
        self.science_state.rank_level = player.get_rank_level();
        self.science_state.experience_progress = 0.0;
        self.science_state.rank_title_label = format!("SCIENCE:Rank{}", player.get_rank_level());

        self.science_state.rank1_buttons.clear();
        self.science_state.rank3_buttons.clear();
        self.science_state.rank8_buttons.clear();
        self.update_context_purchase_science();
    }

    fn update_context_purchase_science(&mut self) {
        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else { return };
        let Ok(player) = player_arc.read() else {
            return;
        };

        self.science_state.available_points = player.get_science_purchase_points();
    }

    /// Feed unlocked science names from PresentationFrame into purchase UI residual.
    ///
    /// Prefer this over live player-list / OBJECT_REGISTRY for dual-tick host paths.
    /// Marks matching rank buttons purchased; stores the full unlocked name list.
    /// Fail-closed: does not claim full ScienceStore / rank-point parity.
    pub fn sync_sciences_from_presentation(&mut self, unlocked_sciences: &[String]) {
        let mut unlocked: Vec<String> = unlocked_sciences.to_vec();
        unlocked.sort();
        unlocked.dedup();
        self.science_state.unlocked_sciences = unlocked.clone();

        let mark = |buttons: &mut Vec<ScienceButtonState>| {
            for btn in buttons.iter_mut() {
                let purchased = unlocked.iter().any(|name| {
                    name.eq_ignore_ascii_case(&btn.command_name)
                        || (!btn.command_name.is_empty()
                            && name
                                .to_ascii_lowercase()
                                .contains(&btn.command_name.to_ascii_lowercase()))
                        || name.eq_ignore_ascii_case(&format!("{:?}", btn.science_type))
                });
                if purchased {
                    btn.is_purchased = true;
                    btn.is_enabled = false;
                }
            }
        };
        mark(&mut self.science_state.rank1_buttons);
        mark(&mut self.science_state.rank3_buttons);
        mark(&mut self.science_state.rank8_buttons);

        // When no INI buttons are loaded yet, seed purchased placeholders so HUD
        // residual still reflects snapshot sciences (fail-closed CommandSet art).
        if self.science_state.rank1_buttons.is_empty()
            && self.science_state.rank3_buttons.is_empty()
            && self.science_state.rank8_buttons.is_empty()
            && !unlocked.is_empty()
        {
            self.science_state.rank1_buttons = unlocked
                .iter()
                .take(8)
                .map(|name| ScienceButtonState {
                    command_name: name.clone(),
                    science_type: SCIENCE_INVALID,
                    is_hidden: false,
                    is_enabled: false,
                    is_purchased: true,
                })
                .collect();
        }
        self.mark_ui_dirty();
    }

    pub fn get_science_state(&self) -> &SciencePurchaseState {
        &self.science_state
    }

    // ---------------------------------------------------------------------------
    // Control bar stage management
    // C++ ControlBar.cpp:2968-3053
    // ---------------------------------------------------------------------------

    pub fn switch_control_bar_stage(&mut self, stage: ControlBarStage) {
        self.control_bar_stage = stage;
    }

    pub fn toggle_control_bar_stage(&mut self) {
        if self.control_bar_stage == ControlBarStage::Default {
            self.switch_control_bar_stage(ControlBarStage::Low);
        } else {
            self.switch_control_bar_stage(ControlBarStage::Default);
        }
    }

    pub fn get_control_bar_stage(&self) -> ControlBarStage {
        self.control_bar_stage
    }

    // ---------------------------------------------------------------------------
    // Control bar scheme
    // C++ ControlBar.cpp:2726-2824
    // ---------------------------------------------------------------------------

    pub fn set_control_bar_scheme_by_player(&mut self) {
        let is_observer = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| p.read().ok().map(|guard| guard.is_player_observer()))
            .unwrap_or(false);

        if is_observer {
            self.observer_mode = true;
            let _ = self.switch_to_context(ControlBarState::Observer, None);
        } else {
            self.observer_mode = false;
            let _ = self.switch_to_context(ControlBarState::None, None);
        }

        self.switch_control_bar_stage(ControlBarStage::Default);
    }

    // ---------------------------------------------------------------------------
    // Player event handlers
    // C++ ControlBar.cpp:1651-1682
    // ---------------------------------------------------------------------------

    pub fn on_player_rank_changed(&mut self) {
        self.gen_star_flash = true;
        self.mark_ui_dirty();
    }

    pub fn on_player_science_purchase_points_changed(&mut self) {
        self.gen_star_flash = true;
        self.mark_ui_dirty();
    }

    // ---------------------------------------------------------------------------
    // Special power shortcut bar
    // C++ ControlBar.cpp:3198-3747
    // ---------------------------------------------------------------------------

    pub fn init_special_power_shortcut_bar(&mut self) {
        self.special_power_shortcuts.clear();
        self.special_power_shortcut_count = 0;

        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else { return };
        let Ok(player) = player_arc.read() else {
            return;
        };

        if !player.is_player_active() {
            return;
        }

        self.special_power_shortcut_count = MAX_SPECIAL_POWER_SHORTCUTS;
        for _ in 0..self.special_power_shortcut_count {
            self.special_power_shortcuts
                .push(SpecialPowerShortcutState {
                    command_name: String::new(),
                    availability: CommandAvailability::Hidden,
                    multiplier_count: 1,
                    is_hidden: true,
                });
        }
    }

    fn populate_special_power_shortcut(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.special_power_shortcut_count == 0 {
            return Ok(());
        }

        let player_arc = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let Some(player_arc) = player_arc else {
            return Ok(());
        };
        let Ok(player) = player_arc.read() else {
            return Ok(());
        };

        let control_bar = get_control_bar_bridge();
        let Some(control_bar) = control_bar else {
            return Ok(());
        };

        let command_set = control_bar
            .find_command_set_by_name("SpecialPowerShortcut")
            .or_else(|| control_bar.find_command_set_by_name("SPECIALPOWERSHORTCUT"));

        let Some(command_set) = command_set else {
            return Ok(());
        };

        let mut current_button = 0;
        for i in 0..self
            .special_power_shortcut_count
            .min(command_set.buttons.len())
        {
            let Some(logic_button) = command_set.buttons.get(i).and_then(|b| b.as_ref()) else {
                continue;
            };

            if (logic_button.get_options_bits() & CommandOption::NeedUpgrade as u32) != 0 {
                let upgrade_name = logic_button
                    .get_upgrade_template()
                    .map(|t| t.get_name().as_str().to_string())
                    .unwrap_or_default();
                if !upgrade_name.is_empty() {
                    let has_upgrade = with_upgrade_center(|c| {
                        let template = c.find_upgrade(&upgrade_name);
                        template.is_some()
                    });
                    if !has_upgrade {
                        continue;
                    }
                }
            }

            if current_button < self.special_power_shortcuts.len() {
                self.special_power_shortcuts[current_button].command_name =
                    logic_button.get_name().to_string();
                self.special_power_shortcuts[current_button].is_hidden = false;
                self.special_power_shortcuts[current_button].availability =
                    CommandAvailability::Available;
                current_button += 1;
            }
        }

        for i in current_button..self.special_power_shortcuts.len() {
            self.special_power_shortcuts[i].is_hidden = true;
        }

        Ok(())
    }

    fn update_special_power_shortcut_availability(&mut self) {
        if self.special_power_shortcut_count == 0 || self.special_power_shortcuts.is_empty() {
            return;
        }

        let player_active = logic_player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .and_then(|p| p.read().ok().map(|g| g.is_player_active()))
            .unwrap_or(false);

        if !player_active {
            for shortcut in &mut self.special_power_shortcuts {
                shortcut.is_hidden = true;
            }
            return;
        }

        for shortcut in &mut self.special_power_shortcuts {
            if shortcut.is_hidden {
                continue;
            }
            // Default to Available; when count_ready_shortcut_special_powers_of_type
            // is ported, wire per-button availability checks here.
            shortcut.availability = CommandAvailability::Available;
        }
    }

    pub fn hide_special_power_shortcut(&mut self) {
        for shortcut in &mut self.special_power_shortcuts {
            shortcut.is_hidden = true;
        }
    }

    pub fn has_any_shortcut_selection(&self) -> bool {
        self.special_power_shortcuts
            .iter()
            .any(|s| !s.is_hidden && s.availability != CommandAvailability::Hidden)
    }

    /// Feed radar, queued upgrades, and ready special-power shortcuts from PresentationFrame.
    ///
    /// Prefer this over live player-list / OBJECT_REGISTRY for dual-tick host paths.
    /// Fail-closed: shortcut command names are template placeholders (not full CommandSet art).
    pub fn sync_radar_queues_and_specials_from_presentation(
        &mut self,
        radar_count: i32,
        radar_disabled: bool,
        queued_upgrades: &[String],
        ready_special_power_templates: &[String],
    ) {
        self.presentation_radar_count = radar_count;
        self.presentation_radar_disabled = radar_disabled;
        let mut queued = queued_upgrades.to_vec();
        queued.sort();
        queued.dedup();
        self.presentation_queued_upgrades = queued;

        // Seed shortcuts from presentation-ready special powers when empty.
        if self.special_power_shortcuts.is_empty() && !ready_special_power_templates.is_empty() {
            let mut names = ready_special_power_templates.to_vec();
            names.sort();
            names.dedup();
            self.special_power_shortcuts = names
                .into_iter()
                .take(8)
                .map(|command_name| SpecialPowerShortcutState {
                    command_name,
                    availability: CommandAvailability::Available,
                    multiplier_count: 1,
                    is_hidden: false,
                })
                .collect();
            self.special_power_shortcut_count = self.special_power_shortcuts.len();
        } else if !ready_special_power_templates.is_empty() {
            // Mark matching shortcuts available.
            for sc in self.special_power_shortcuts.iter_mut() {
                if ready_special_power_templates.iter().any(|n| {
                    n.eq_ignore_ascii_case(&sc.command_name)
                        || sc.command_name.eq_ignore_ascii_case(n)
                }) {
                    sc.availability = CommandAvailability::Available;
                    sc.is_hidden = false;
                }
            }
        }

        // Gen-star residual: flash when upgrades are queued (purchase-points proxy).
        if !self.presentation_queued_upgrades.is_empty() {
            self.gen_star_flash = true;
        }
        self.mark_ui_dirty();
    }

    pub fn presentation_radar_count(&self) -> i32 {
        self.presentation_radar_count
    }

    pub fn presentation_radar_disabled(&self) -> bool {
        self.presentation_radar_disabled
    }

    pub fn presentation_queued_upgrades(&self) -> &[String] {
        &self.presentation_queued_upgrades
    }

    pub fn get_special_power_shortcuts(&self) -> &[SpecialPowerShortcutState] {
        &self.special_power_shortcuts
    }

    // ---------------------------------------------------------------------------
    // Command bar border colors
    // C++ ControlBar.cpp:2361-2397, 2887-2893
    // ---------------------------------------------------------------------------

    pub fn set_command_bar_border_colors(
        &mut self,
        build: Option<u32>,
        action: Option<u32>,
        upgrade: Option<u32>,
        system: Option<u32>,
    ) {
        self.border_colors.build = build;
        self.border_colors.action = action;
        self.border_colors.upgrade = upgrade;
        self.border_colors.system = system;
    }

    pub fn get_border_color_for_type(
        &self,
        border_type: CommandButtonMappedBorderType,
    ) -> Option<u32> {
        match border_type {
            CommandButtonMappedBorderType::Build => self.border_colors.build,
            CommandButtonMappedBorderType::Upgrade => self.border_colors.upgrade,
            CommandButtonMappedBorderType::Action => self.border_colors.action,
            CommandButtonMappedBorderType::System => self.border_colors.system,
            CommandButtonMappedBorderType::None => None,
        }
    }

    // ---------------------------------------------------------------------------
    // Build queue display helpers
    // C++ ControlBar.cpp:2832-2870
    // ---------------------------------------------------------------------------

    pub fn get_displayed_construct_percent(&self) -> f32 {
        self.displayed_construct_percent
    }

    pub fn get_displayed_ocl_timer_seconds(&self) -> u32 {
        self.displayed_ocl_timer_seconds
    }

    pub fn get_displayed_queue_count(&self) -> usize {
        self.displayed_queue_count
    }

    // ---------------------------------------------------------------------------
    // Communicator hide
    // C++ ControlBar.cpp:2897-2902
    // ---------------------------------------------------------------------------

    pub fn hide_communicator(&mut self, hide: bool) {
        if let Some(wm) = &self.window_manager {
            if let Some(win) = wm.find_window_by_name("ControlBar.wnd:PopupCommunicator") {
                let _ = win.borrow_mut().hide(hide);
            }
        }
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
        self.control_bar_stage = ControlBarStage::Default;
        self.portrait_state = PortraitDisplayState::default();
        self.science_state = SciencePurchaseState::default();
        self.gen_star_flash = true;
        self.last_flashed_at_point_value = -1;
        self.radar_attack_glow_on = false;
        self.remaining_radar_attack_glow_frames = 0;
        self.displayed_construct_percent = -1.0;
        self.displayed_ocl_timer_seconds = 0;
        self.special_power_shortcuts.clear();
        self.special_power_shortcut_count = 0;

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
