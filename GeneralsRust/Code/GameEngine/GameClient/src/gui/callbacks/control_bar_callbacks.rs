//! Control Bar Callback Functions
//!
//! Implements in-game control bar callbacks and radar input handling, matching
//! the behavior of C++ ControlBarCallback.cpp.

use crate::display::view::{with_tactical_view, Point3};
use crate::gui::{
    hide_quit_menu, toggle_diplomacy, toggle_quit_menu, with_window_manager,
    with_window_manager_ref, AnimateWindowManager, AnimationType, GameWindow, WindowMessage,
    WindowMsgData, WindowMsgHandled,
};
use crate::helpers::{TheControlBar, TheInGameUI};
use crate::language_filter::get_language_filter;
use crate::message_stream::{get_message_stream, GameMessageType};
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::radar::{
    get_radar_system, ICoord2D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH,
};
use gamelogic::commands::command::CommandType;
use gamelogic::commands::selection::get_selection_manager;
use gamelogic::helpers::{TheGameLogic, TheScriptEngine};
use gamelogic::player::ThePlayerList;
use std::sync::{Arc, RwLock};

const CMD_NEED_TARGET_POS: u32 = 0x0000_0020;
const CMD_ATTACK_OBJECTS_POSITION: u32 = 0x0000_1000;

/// Control bar system state
#[derive(Debug, Clone)]
pub struct ControlBarState {
    pub visible: bool,
}

impl Default for ControlBarState {
    fn default() -> Self {
        Self { visible: true }
    }
}

fn decode_mouse_pos(data: WindowMsgData) -> (i32, i32) {
    let x = (data & 0xFFFF) as i32;
    let y = (data >> 16) as i32;
    (x, y)
}

fn local_player_has_radar() -> bool {
    let Ok(list) = ThePlayerList().read() else {
        return false;
    };
    let Some(player) = list.get_local_player() else {
        return false;
    };
    let Ok(player_guard) = player.read() else {
        return false;
    };
    player_guard.has_radar()
}

fn radar_allows_input() -> bool {
    let radar_system = get_radar_system();
    let Ok(radar) = radar_system.read() else {
        return false;
    };
    if radar.is_radar_forced() {
        return true;
    }
    if radar.is_radar_hidden() {
        return false;
    }
    local_player_has_radar()
}

fn local_pixel_to_radar(local_x: i32, local_y: i32, width: i32, height: i32) -> Option<ICoord2D> {
    if width <= 0 || height <= 0 {
        return None;
    }
    if local_x < 0 || local_y < 0 || local_x >= width || local_y >= height {
        return None;
    }
    let radar_x = (local_x as i64 * RADAR_CELL_WIDTH as i64) / width as i64;
    let radar_y = (local_y as i64 * RADAR_CELL_HEIGHT as i64) / height as i64;
    Some(ICoord2D::new(radar_x as i32, radar_y as i32))
}

fn is_alternate_mouse_enabled() -> bool {
    get_global_data()
        .map(|data| data.read().use_alternate_mouse)
        .unwrap_or(false)
}

fn selection_is_empty() -> bool {
    let Ok(list) = ThePlayerList().read() else {
        return true;
    };
    let local_index = list.get_local_player_index();
    let selection_manager = get_selection_manager();
    let Ok(manager) = selection_manager.read() else {
        return true;
    };
    manager
        .get_player_selection_ref(local_index)
        .map(|selection| selection.get_selection_count() == 0)
        .unwrap_or(true)
}

fn filter_beacon_edit_text(mut text: String) -> String {
    get_language_filter().filter_line(&mut text);
    text
}

fn has_pending_radar_targeting_mode() -> bool {
    if let Some(pending_power) = TheInGameUI::get_pending_special_power() {
        if (pending_power.options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION)) != 0 {
            return true;
        }
    }

    if let Some(pending_command) = TheInGameUI::get_pending_command() {
        if (pending_command.options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION)) != 0 {
            return true;
        }
    }

    false
}

fn refresh_radar_cursor(msg: WindowMessage) {
    if msg == WindowMessage::MouseLeaving {
        TheInGameUI::set_cursor_arrow();
        return;
    }

    if has_pending_radar_targeting_mode() {
        // C++ parity fallback: when targeted command cursor metadata is unavailable,
        // use CROSS while targeting over radar.
        TheInGameUI::set_cursor_by_name("CROSS");
        return;
    }

    if selection_is_empty() {
        TheInGameUI::set_cursor_arrow();
        return;
    }

    if TheInGameUI::is_in_attack_move_to_mode() {
        TheInGameUI::set_cursor_by_name("ATTACKMOVETO");
    } else {
        TheInGameUI::set_cursor_by_name("MOVETO");
    }
}

/// Control bar callback handler
pub struct ControlBarCallbacks {
    state: ControlBarState,
    animate_manager: AnimateWindowManager,
    button_communicator: u32,
}

impl ControlBarCallbacks {
    pub fn new() -> Self {
        Self {
            state: ControlBarState::default(),
            animate_manager: AnimateWindowManager::new(),
            button_communicator: 0,
        }
    }

    /// Handle control bar system messages
    pub fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if TheScriptEngine::is_game_ending() {
            return WindowMsgHandled::Ignored;
        }

        match msg {
            WindowMessage::Create => {
                self.button_communicator =
                    NameKeyGenerator::name_to_key("ControlBar.wnd:PopupCommunicator") as u32;
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetMouseEntering | WindowMessage::GadgetMouseLeaving => {
                let control_id = data1 as u32;
                let entering = msg == WindowMessage::GadgetMouseEntering;
                TheControlBar::process_context_sensitive_button_transition(control_id, entering);
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetSelected | WindowMessage::GadgetRightClick => {
                let control_id = data1 as u32;
                self.handle_button_selected(control_id, msg == WindowMessage::GadgetRightClick);
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetEditDone => {
                self.handle_edit_done(data1 as u32);
                WindowMsgHandled::Handled
            }
            WindowMessage::None => {
                self.animate_manager.update();
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn handle_button_selected(&mut self, control_id: u32, right_click: bool) {
        let beacon_place_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonPlaceBeacon") as u32;
        let beacon_delete_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonDeleteBeacon") as u32;
        let beacon_clear_text_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonClearBeaconText") as u32;
        let beacon_general_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonGeneral") as u32;
        let button_large_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonLarge") as u32;
        let button_options_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonOptions") as u32;
        let button_idle_worker_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker") as u32;

        if control_id == self.button_communicator {
            let _ = toggle_diplomacy(false);
            return;
        }

        if control_id == beacon_place_id {
            if TheGameLogic::is_in_multiplayer_game()
                && ThePlayerList()
                    .read()
                    .ok()
                    .and_then(|list| list.get_local_player().cloned())
                    .and_then(|player| player.read().ok().map(|p| p.is_player_active()))
                    .unwrap_or(false)
            {
                TheInGameUI::clear_pending_special_power();
                TheInGameUI::set_pending_command(CommandType::PlaceBeacon, CMD_NEED_TARGET_POS, 0);
                TheInGameUI::set_force_attack_mode(false);
                TheInGameUI::set_force_move_to_mode(false);
                TheInGameUI::set_prefer_selection_mode(false);
            }
            return;
        }

        if control_id == beacon_delete_id {
            if TheGameLogic::is_in_multiplayer_game() {
                let message_stream = get_message_stream();
                let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
                stream.append_message(GameMessageType::RemoveBeacon(
                    crate::message_stream::game_message::Coord3D::default(),
                ));
            }
            return;
        }

        if control_id == beacon_clear_text_id {
            if TheGameLogic::is_in_multiplayer_game() {
                let text_id = NameKeyGenerator::name_to_key("ControlBar.wnd:EditBeaconText") as i32;
                with_window_manager(|manager| {
                    if let Some(handle) = manager.get_window_by_id(text_id) {
                        let _ = handle.borrow_mut().set_text("");
                    }
                });
            }
            return;
        }

        if control_id == beacon_general_id {
            hide_quit_menu();
            TheControlBar::toggle_purchase_science();
            return;
        }

        if control_id == button_large_id {
            TheControlBar::toggle_control_bar_stage();
            return;
        }

        if control_id == button_options_id {
            toggle_quit_menu();
            return;
        }

        if control_id == button_idle_worker_id {
            hide_quit_menu();
            let message_stream = get_message_stream();
            let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
            stream.append_message(GameMessageType::MetaSelectNextWorker);
            return;
        }

        const GGM_LEFT_DRAG: u32 = 16384;
        const GBM_SELECTED: u32 = GGM_LEFT_DRAG + 8;
        const GBM_SELECTED_RIGHT: u32 = GGM_LEFT_DRAG + 9;
        let msg = if right_click {
            GBM_SELECTED_RIGHT
        } else {
            GBM_SELECTED
        };
        TheControlBar::process_context_sensitive_button_click(control_id, msg);
    }

    fn handle_edit_done(&mut self, control_id: u32) {
        let text_id = NameKeyGenerator::name_to_key("ControlBar.wnd:EditBeaconText") as u32;
        if control_id != text_id {
            return;
        }

        if TheGameLogic::is_in_multiplayer_game() && !selection_is_empty() {
            let text = with_window_manager(|manager| {
                manager
                    .get_window_by_id(text_id as i32)
                    .map(|win| win.borrow().get_text().to_string())
            })
            .unwrap_or_default();
            let text = filter_beacon_edit_text(text);
            let message_stream = get_message_stream();
            let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
            stream.append_message(GameMessageType::SetBeaconText(
                crate::message_stream::game_message::Coord3D::default(),
                text,
            ));
        }
    }

    /// Toggle control bar visibility
    pub fn toggle_control_bar(
        &mut self,
        immediate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.state.visible {
            self.hide_control_bar(immediate)?;
        } else {
            self.show_control_bar(immediate)?;
        }
        Ok(())
    }

    /// Hide the control bar
    pub fn hide_control_bar(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        if !self.state.visible {
            return Ok(());
        }
        self.state.visible = false;
        TheControlBar::hide_special_power_shortcut();
        self.apply_visibility_change(immediate, false)?;
        Ok(())
    }

    /// Show the control bar
    pub fn show_control_bar(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        if self.state.visible {
            return Ok(());
        }
        self.state.visible = true;
        TheControlBar::show_special_power_shortcut();
        self.apply_visibility_change(immediate, true)?;
        Ok(())
    }

    /// Check if control bar is visible
    pub fn is_visible(&self) -> bool {
        self.state.visible
    }

    fn apply_visibility_change(
        &mut self,
        immediate: bool,
        show: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let control_bar_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent") as i32;
        let (screen_w, screen_h) = with_window_manager_ref(|manager| manager.screen_size());
        self.animate_manager.set_screen_size(screen_w, screen_h);

        with_window_manager(|manager| {
            if let Some(handle) = manager.get_window_by_id(control_bar_id) {
                if immediate {
                    let _ = handle.borrow_mut().hide(!show);
                } else {
                    self.animate_manager.reset();
                    self.animate_manager.register_window(
                        handle.clone(),
                        AnimationType::SlideBottom,
                        true,
                        500,
                        0,
                    );
                    let _ = handle.borrow_mut().hide(false);
                    if !show {
                        self.animate_manager.reverse_animate_window();
                    }
                }
            }
        });

        let target_height = if show {
            (screen_h as f32 * 0.80f32) as i32
        } else {
            screen_h
        };
        with_tactical_view(|view| {
            view.set_height(target_height);
        });
        if show {
            TheControlBar::animate_special_power_shortcut(true);
        } else {
            TheControlBar::animate_special_power_shortcut(false);
            TheControlBar::hide_purchase_science();
        }
        Ok(())
    }
}

impl Default for ControlBarCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

/// Left HUD input handler
pub struct LeftHUDCallbacks {}

impl LeftHUDCallbacks {
    pub fn new() -> Self {
        Self {}
    }

    /// Handle left HUD input messages
    pub fn input(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if !radar_allows_input() {
            return WindowMsgHandled::Handled;
        }

        match msg {
            WindowMessage::MiddleDown => WindowMsgHandled::Ignored,
            WindowMessage::None | WindowMessage::MouseEntering | WindowMessage::MouseLeaving => {
                refresh_radar_cursor(msg);
                WindowMsgHandled::Handled
            }
            WindowMessage::MousePos => {
                self.handle_mouse_pos(window, data1);
                WindowMsgHandled::Handled
            }
            WindowMessage::LeftDown | WindowMessage::RightDown => {
                self.handle_mouse_down(window, msg, data1);
                WindowMsgHandled::Handled
            }
            WindowMessage::LeftUp | WindowMessage::RightUp => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn handle_mouse_pos(&self, window: &GameWindow, data1: WindowMsgData) {
        let (mouse_x, mouse_y) = decode_mouse_pos(data1);
        let (screen_x, screen_y) = window.get_screen_position();
        let (width, height) = window.get_size();
        let local_x = mouse_x - screen_x;
        let local_y = mouse_y - screen_y;
        if local_pixel_to_radar(local_x, local_y, width, height).is_some() {
            refresh_radar_cursor(WindowMessage::MousePos);
        }
    }

    fn handle_mouse_down(&self, window: &GameWindow, msg: WindowMessage, data1: WindowMsgData) {
        let (mouse_x, mouse_y) = decode_mouse_pos(data1);
        let (screen_x, screen_y) = window.get_screen_position();
        let (width, height) = window.get_size();
        let local_x = mouse_x - screen_x;
        let local_y = mouse_y - screen_y;
        let Some(radar_pos) = local_pixel_to_radar(local_x, local_y, width, height) else {
            return;
        };
        let radar_system = get_radar_system();
        let Ok(radar) = radar_system.read() else {
            return;
        };
        let Some(world) = radar.radar_to_world(&radar_pos) else {
            return;
        };

        let use_alternate = is_alternate_mouse_enabled();
        let selection_empty = selection_is_empty();
        let uses_right = msg == WindowMessage::RightDown;
        let uses_left = msg == WindowMessage::LeftDown;

        if selection_empty || (!use_alternate && uses_right) || (use_alternate && uses_left) {
            with_tactical_view(|view| {
                view.look_at(&Point3::new(world.x, world.y, world.z));
            });
        } else {
            let message_stream = get_message_stream();
            let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
            let world_pos =
                crate::message_stream::game_message::Coord3D::new(world.x, world.y, world.z);

            if let Some(pending_power) = TheInGameUI::get_pending_special_power() {
                if (pending_power.options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION))
                    != 0
                {
                    stream.append_message(GameMessageType::DoSpecialPowerAtLocation(
                        pending_power.power_id,
                        world_pos,
                        0.0,
                        0,
                        pending_power.options,
                        pending_power.source_object_id,
                    ));
                    TheInGameUI::clear_pending_special_power();
                    TheInGameUI::clear_attack_move_to_mode();
                    return;
                }
            }

            if let Some(pending_command) = TheInGameUI::get_pending_command() {
                if (pending_command.options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION))
                    != 0
                {
                    let pending_message = match pending_command.command_type {
                        CommandType::DoAttackMoveTo => {
                            Some(GameMessageType::DoAttackMoveTo(world_pos.clone()))
                        }
                        CommandType::DoGuardPosition => {
                            Some(GameMessageType::DoGuardPosition(world_pos.clone(), 0))
                        }
                        CommandType::PlaceBeacon => {
                            Some(GameMessageType::PlaceBeacon(world_pos.clone()))
                        }
                        CommandType::RemoveBeacon => {
                            Some(GameMessageType::RemoveBeacon(world_pos.clone()))
                        }
                        CommandType::SetRallyPoint => Some(GameMessageType::SetRallyPoint(
                            pending_command.source_object_id,
                            world_pos.clone(),
                        )),
                        _ => None,
                    };

                    if let Some(message) = pending_message {
                        stream.append_message(message);
                        TheInGameUI::clear_pending_command();
                        TheInGameUI::clear_attack_move_to_mode();
                        return;
                    }
                }
            }

            if TheInGameUI::is_in_attack_move_to_mode() {
                stream.append_message(GameMessageType::DoAttackMoveTo(world_pos.clone()));
            } else {
                stream.append_message(GameMessageType::DoMoveTo(world_pos));
            }
        }

        TheInGameUI::clear_attack_move_to_mode();
    }
}

/// Control bar observer system handler
pub struct ControlBarObserverCallbacks {}

impl ControlBarObserverCallbacks {
    pub fn new() -> Self {
        Self {}
    }

    /// Handle control bar observer system messages
    pub fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::None | WindowMessage::Create | WindowMessage::Destroy => {
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }
}

impl Default for ControlBarObserverCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

/// Combined control bar callback system
pub struct ControlBarSystem {
    callbacks: Arc<RwLock<ControlBarCallbacks>>,
    left_hud: Arc<RwLock<LeftHUDCallbacks>>,
    observer: Arc<RwLock<ControlBarObserverCallbacks>>,
}

impl ControlBarSystem {
    pub fn new() -> Self {
        Self {
            callbacks: Arc::new(RwLock::new(ControlBarCallbacks::new())),
            left_hud: Arc::new(RwLock::new(LeftHUDCallbacks::new())),
            observer: Arc::new(RwLock::new(ControlBarObserverCallbacks::new())),
        }
    }

    pub fn get_callbacks(&self) -> Arc<RwLock<ControlBarCallbacks>> {
        self.callbacks.clone()
    }

    pub fn get_left_hud(&self) -> Arc<RwLock<LeftHUDCallbacks>> {
        self.left_hud.clone()
    }

    pub fn get_observer(&self) -> Arc<RwLock<ControlBarObserverCallbacks>> {
        self.observer.clone()
    }

    /// Toggle control bar visibility through the system
    pub fn toggle_control_bar(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.toggle_control_bar(immediate)
    }

    /// Hide control bar through the system
    pub fn hide_control_bar(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.hide_control_bar(immediate)
    }

    /// Show control bar through the system  
    pub fn show_control_bar(&self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.show_control_bar(immediate)
    }
}

impl Default for ControlBarSystem {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static THE_CONTROL_BAR_SYSTEM: Arc<RwLock<ControlBarSystem>> =
        Arc::new(RwLock::new(ControlBarSystem::new()));
}

/// Helper function to get the global control bar system
pub fn get_control_bar_system() -> Arc<RwLock<ControlBarSystem>> {
    THE_CONTROL_BAR_SYSTEM.with(|system| system.clone())
}

/// Convenience functions for global control bar operations
pub fn toggle_control_bar(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_control_bar_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.toggle_control_bar(immediate)
}

pub fn hide_control_bar(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_control_bar_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.hide_control_bar(immediate)
}

pub fn show_control_bar(immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_control_bar_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.show_control_bar(immediate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_control_bar_callbacks() {
        let mut callbacks = ControlBarCallbacks::new();

        // Test initial state
        assert!(callbacks.is_visible());

        // Test hiding
        assert!(callbacks.hide_control_bar(true).is_ok());
        assert!(!callbacks.is_visible());

        // Test showing
        assert!(callbacks.show_control_bar(true).is_ok());
        assert!(callbacks.is_visible());

        // Test toggling
        assert!(callbacks.toggle_control_bar(true).is_ok());
        assert!(!callbacks.is_visible());

        assert!(callbacks.toggle_control_bar(true).is_ok());
        assert!(callbacks.is_visible());
    }

    #[test]
    fn test_control_bar_system() {
        let system = ControlBarSystem::new();

        // Test that all components are accessible
        assert!(system.get_callbacks().read().is_ok());
        assert!(system.get_left_hud().read().is_ok());
        assert!(system.get_observer().read().is_ok());

        // Test system-level operations
        assert!(system.toggle_control_bar(true).is_ok());
        assert!(system.hide_control_bar(true).is_ok());
        assert!(system.show_control_bar(true).is_ok());
    }

    #[test]
    fn test_global_functions() {
        assert!(toggle_control_bar(true).is_ok());
        assert!(hide_control_bar(true).is_ok());
        assert!(show_control_bar(true).is_ok());
    }

    #[test]
    fn beacon_edit_text_is_language_filtered() {
        get_language_filter().set_words_for_test(["badword"]);

        let filtered = filter_beacon_edit_text("hold badword beacon".to_string());

        assert_eq!(filtered, "hold ******* beacon");
    }
}
