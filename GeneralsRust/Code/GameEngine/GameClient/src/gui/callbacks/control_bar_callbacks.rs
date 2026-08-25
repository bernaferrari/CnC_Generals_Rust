//! Control Bar Callback Functions
//!
//! Implements in-game control bar callbacks and radar input handling, matching
//! the behavior of C++ ControlBarCallback.cpp.

use crate::display::view::{Point3, set_tactical_view_height_frac, with_tactical_view};
use crate::gui::callbacks::replay_controls::{
    hide_replay_controls, show_replay_controls, toggle_replay_controls,
};
use crate::gui::control_bar::{
    ControlBar, ControlBarStage, HostMinimapInteractionRequest, HostMinimapMouseButton,
    host_control_bar_bridge_enabled, publish_host_minimap_interaction,
    publish_host_select_next_idle_worker,
};
use crate::gui::{
    AnimateWindowManager, AnimationType, GameWindow, WindowMessage, WindowMsgData,
    WindowMsgHandled, hide_quit_menu, toggle_diplomacy, toggle_quit_menu, with_window_manager,
    with_window_manager_ref,
};
use crate::helpers::{TheControlBar, TheInGameUI};

use crate::input::{MouseButton, with_mouse};
use crate::language_filter::get_language_filter;
use crate::message_stream::{GameMessageType, get_message_stream};
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::radar::{
    ICoord2D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH, get_radar_system,
};
use gamelogic::commands::command::CommandType;
use gamelogic::commands::selection::get_selection_manager;
use gamelogic::helpers::{TheGameLogic, TheScriptEngine};
use gamelogic::player::ThePlayerList;
use std::sync::{Arc, RwLock};

const CMD_NEED_TARGET_POS: u32 = 0x0000_0020;
const CMD_ATTACK_OBJECTS_POSITION: u32 = 0x0000_1000;
const KEY_ESC: usize = 0x1B;

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

/// After C++ `LeftHUDInput` validates radar availability, a held middle button
/// falls through to ordinary camera handling.
fn middle_mouse_is_held() -> bool {
    with_mouse(|mouse| mouse.get_mouse_status().is_button_down(MouseButton::Middle))
}

fn local_pixel_to_radar(local_x: i32, local_y: i32, width: i32, height: i32) -> Option<ICoord2D> {
    use crate::gui::ingame_ui::local_pixel_to_radar_cell;
    use game_engine::common::system::radar::get_radar_system;
    let extent = get_radar_system()
        .read()
        .ok()
        .map(|radar| radar.map_extent())
        .unwrap_or(game_engine::common::system::radar::Region3D {
            lo: game_engine::common::system::radar::Coord3D {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            hi: game_engine::common::system::radar::Coord3D {
                x: 1.0,
                y: 1.0,
                z: 0.0,
            },
        });
    let cell = local_pixel_to_radar_cell(
        local_x,
        local_y,
        width,
        height,
        extent.hi.x - extent.lo.x,
        extent.hi.y - extent.lo.y,
    )?;
    Some(ICoord2D::new(cell.x, cell.y))
}

fn is_alternate_mouse_enabled() -> bool {
    get_global_data()
        .map(|data| data.read().use_alternate_mouse)
        .unwrap_or(false)
}

/// Local player's side for ControlBar scheme (C++ Player::getSide).
fn local_control_bar_player_side() -> String {
    ThePlayerList()
        .read()
        .ok()
        .and_then(|list| list.get_local_player().cloned())
        .and_then(|player| player.read().ok().map(|guard| guard.get_side().clone()))
        .filter(|side| !side.is_empty())
        .unwrap_or_else(|| "Observer".to_string())
}

fn selection_is_empty() -> bool {
    selection_count() == 0
}

fn selection_count() -> usize {
    let Ok(list) = ThePlayerList().read() else {
        return 0;
    };
    let local_index = list.get_local_player_index();
    let selection_manager = get_selection_manager();
    let Ok(manager) = selection_manager.read() else {
        return 0;
    };
    manager
        .get_player_selection_ref(local_index)
        .map(|selection| selection.get_selection_count())
        .unwrap_or(0)
}

fn should_submit_beacon_text(is_multiplayer: bool, selection_count: usize) -> bool {
    is_multiplayer && selection_count == 1
}

fn clear_local_selection() {
    let local_index = ThePlayerList()
        .read()
        .map(|list| list.get_local_player_index())
        .unwrap_or(0);
    let selection_manager = get_selection_manager();
    let Ok(mut manager) = selection_manager.write() else {
        return;
    };
    if let Some(selection) = manager.get_player_selection(local_index) {
        selection.clear_selection();
    }
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

fn is_known_cursor_name(cursor_name: &str) -> bool {
    matches!(
        cursor_name,
        "ARROW"
            | "CROSS"
            | "SELECTING"
            | "MOVETO"
            | "ATTACKMOVETO"
            | "WAYPOINT"
            | "ATTACK_OBJECT"
            | "OUTRANGE"
            | "FORCE_ATTACK_OBJECT"
            | "FORCE_ATTACK_GROUND"
            | "GET_REPAIRED"
            | "DOCK"
            | "GET_HEALED"
            | "DO_REPAIR"
            | "RESUME_CONSTRUCTION"
            | "ENTER_FRIENDLY"
            | "ENTER_AGGRESSIVELY"
            | "DEFECTOR"
            | "CAPTUREBUILDING"
            | "HACK"
            | "GENERIC_INVALID"
            | "SET_RALLY_POINT"
            | "PARTICLE_UPLINK_CANNON"
    )
}

fn radar_targeting_cursor_name(cursor_name: Option<&str>) -> &str {
    match cursor_name {
        Some(name) if is_known_cursor_name(name) => name,
        _ => "CROSS",
    }
}

fn refresh_radar_cursor(msg: WindowMessage) {
    if msg == WindowMessage::MouseLeaving {
        TheInGameUI::set_cursor_arrow();
        return;
    }

    if has_pending_radar_targeting_mode() {
        // C++ LeftHUDInput: keep a targeted command cursor over radar, falling
        // back to CROSS when no valid cursor name is available.
        let cursor_name = TheInGameUI::get_pending_command()
            .map(|pending| pending.cursor_name)
            .filter(|name| !name.is_empty());
        TheInGameUI::set_cursor_by_name(radar_targeting_cursor_name(cursor_name.as_deref()));
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
                    NameKeyGenerator::name_to_key("ControlBar.wnd:PopupCommunicator");
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
        let beacon_place_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonPlaceBeacon");
        let beacon_delete_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonDeleteBeacon");
        let beacon_clear_text_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonClearBeaconText");
        let beacon_general_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonGeneral");
        let button_large_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonLarge");
        let button_options_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonOptions");
        let button_idle_worker_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker");

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
            // C++ calls InGameUI::selectNextIdleWorker directly here.  While
            // Main owns the match, send the same UI intent to its typed
            // selection boundary rather than its dormant legacy message path.
            if publish_host_select_next_idle_worker() {
                return;
            }
            // Standalone GameClient retains its existing legacy route.
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
        let text_id = NameKeyGenerator::name_to_key("ControlBar.wnd:EditBeaconText");
        if control_id != text_id {
            return;
        }

        if should_submit_beacon_text(TheGameLogic::is_in_multiplayer_game(), selection_count()) {
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
        // C++ ToggleControlBar: toggleReplayControls() then flip the bar.
        toggle_replay_controls();
        if self.state.visible {
            self.hide_control_bar_window(immediate)?;
        } else {
            self.show_control_bar_window(immediate)?;
        }
        Ok(())
    }

    /// Hide the control bar
    pub fn hide_control_bar(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        // C++ HideControlBar: hideReplayControls() even if already hidden.
        hide_replay_controls();
        self.hide_control_bar_window(immediate)
    }

    pub fn show_control_bar(&mut self, immediate: bool) -> Result<(), Box<dyn std::error::Error>> {
        // C++ ShowControlBar: showReplayControls() then switch DEFAULT + winHide(FALSE).
        show_replay_controls();
        self.show_control_bar_window(immediate)
    }

    fn hide_control_bar_window(
        &mut self,
        immediate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.state.visible {
            return Ok(());
        }
        self.state.visible = false;
        TheControlBar::hide_special_power_shortcut();
        self.apply_visibility_change(immediate, false)?;
        Ok(())
    }

    fn show_control_bar_window(
        &mut self,
        immediate: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ ShowControlBar always switchControlBarStage(DEFAULT) + winHide(FALSE)
        // even when already shown. Do not early-return on state.visible.
        if !self.state.visible {
            self.state.visible = true;
            TheControlBar::show_special_power_shortcut();
        }
        self.apply_visibility_change(immediate, true)?;
        Ok(())
    }

    /// C++ GameLogic.cpp:2233 setControlBarSchemeByPlayer + ControlBarCallback.cpp:489 DEFAULT.
    fn apply_show_control_bar_scheme_and_default_stage(&self) {
        let parent_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent") as i32;
        let parent_live =
            with_window_manager(|manager| manager.get_window_by_id(parent_id).is_some());
        if !parent_live {
            return;
        }
        let side = local_control_bar_player_side();
        TheControlBar::set_control_bar_scheme_by_player(&side);
        ControlBar::request_switch_control_bar_stage(ControlBarStage::Default);
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

        let mut parent_live = false;
        with_window_manager(|manager| {
            if let Some(handle) = manager.get_window_by_id(control_bar_id) {
                parent_live = true;
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
        // C++ HideControlBar setHeight(Display) / ShowControlBar 80%. Live 3D
        // viewport reads tactical_view_height_frac, not only leftover view height.
        set_tactical_view_height_frac(if show { 0.80 } else { 1.0 });
        with_tactical_view(|view| {
            view.set_height(target_height);
        });
        if show {
            if parent_live {
                // C++ ShowControlBar: scheme is applied at InGame start; stage DEFAULT
                // when ControlBarParent exists (ControlBarCallback.cpp:489).
                self.apply_show_control_bar_scheme_and_default_stage();
            }
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

pub fn beacon_window_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char && data1 == KEY_ESC {
        clear_local_selection();
        return WindowMsgHandled::Handled;
    }

    WindowMsgHandled::Ignored
}

/// Left HUD input handler
pub struct LeftHUDCallbacks {}

impl Default for LeftHUDCallbacks {
    fn default() -> Self {
        Self::new()
    }
}

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
        let host_bridge = host_control_bar_bridge_enabled();

        // C++ first eats input when the radar is unavailable, then checks
        // held MMB.  Main owns radar authority while the bridge is active,
        // so defer the equivalent check to its existing presentation-side
        // gate rather than consulting the dormant standalone globals.
        if !host_bridge && !radar_allows_input() {
            return WindowMsgHandled::Handled;
        }

        // C++ ControlBarCallback.cpp checks middleState before its switch.
        // In particular, do not turn an LMB/RMB held alongside MMB into an
        // authoritative-host minimap interaction; returning Ignored lets the
        // normal middle-mouse route retain ownership.
        if middle_mouse_is_held() {
            return WindowMsgHandled::Ignored;
        }

        // The executable owns the Rust simulation, not this standalone
        // GameClient's legacy GameLogic globals.  Send real LeftHUD clicks to
        // Main before consulting those globals, while leaving the standalone
        // path below exactly intact when the bridge is disabled.
        if host_bridge && matches!(msg, WindowMessage::LeftDown | WindowMessage::RightDown) {
            self.publish_host_minimap_mouse_down(window, msg, data1);
            return WindowMsgHandled::Handled;
        }

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

    /// Publish a real `ControlBar.wnd:LeftHUD` click for Main's WGPU minimap.
    ///
    /// We only establish that the pointer belongs to this WND rectangle here.
    /// World conversion and FOW remain at Main's authoritative presentation
    /// boundary. `publish_host_minimap_interaction` captures the WND dispatch
    /// provenance itself, so `FromUser`-style legacy state cannot forge an OS
    /// physical click.
    fn publish_host_minimap_mouse_down(
        &self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
    ) {
        let (mouse_x, mouse_y) = decode_mouse_pos(data1);
        let (screen_x, screen_y) = window.get_screen_position();
        let (width, height) = window.get_size();
        let local_x = mouse_x - screen_x;
        let local_y = mouse_y - screen_y;
        if local_pixel_to_radar(local_x, local_y, width, height).is_none() {
            return;
        }

        let button = match msg {
            WindowMessage::LeftDown => HostMinimapMouseButton::Left,
            WindowMessage::RightDown => HostMinimapMouseButton::Right,
            _ => return,
        };
        let _ = publish_host_minimap_interaction(HostMinimapInteractionRequest {
            screen_position: [mouse_x, mouse_y],
            screen_top_left: [screen_x, screen_y],
            screen_size: [width, height],
            button,
            alternate_mouse: is_alternate_mouse_enabled(),
        });
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
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        crate::gui::control_bar::control_bar_observer::control_bar_observer_system(
            window, msg, data1, data2,
        )
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

pub fn is_control_bar_visible() -> bool {
    let system = get_control_bar_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system
        .get_callbacks()
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .is_visible()
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
    fn hide_control_bar_lifts_tactical_view_frac() {
        use crate::display::view::tactical_view_height_frac;
        set_tactical_view_height_frac(0.80);
        let mut callbacks = ControlBarCallbacks::new();
        callbacks.state.visible = true;
        assert!(callbacks.hide_control_bar(true).is_ok());
        assert!((tactical_view_height_frac() - 1.0).abs() < f32::EPSILON);
        assert!(callbacks.show_control_bar(true).is_ok());
        assert!((tactical_view_height_frac() - 0.80).abs() < f32::EPSILON);
    }

    #[test]
    fn beacon_edit_text_is_language_filtered() {
        get_language_filter().set_words_for_test(["badword"]);

        let filtered = filter_beacon_edit_text("hold badword beacon".to_string());

        assert_eq!(filtered, "hold ******* beacon");
    }

    #[test]
    fn beacon_window_input_handles_escape_only() {
        let window = GameWindow::new();

        assert_eq!(
            beacon_window_input(&window, WindowMessage::Char, KEY_ESC as WindowMsgData, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            beacon_window_input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn radar_targeting_cursor_preserves_valid_pending_cursor() {
        assert_eq!(
            radar_targeting_cursor_name(Some("PARTICLE_UPLINK_CANNON")),
            "PARTICLE_UPLINK_CANNON"
        );
        assert_eq!(
            radar_targeting_cursor_name(Some("ATTACK_OBJECT")),
            "ATTACK_OBJECT"
        );
    }

    #[test]
    fn radar_targeting_cursor_falls_back_to_cross_for_missing_or_invalid_cursor() {
        assert_eq!(radar_targeting_cursor_name(None), "CROSS");
        assert_eq!(radar_targeting_cursor_name(Some("")), "CROSS");
        assert_eq!(radar_targeting_cursor_name(Some("BogusCursor")), "CROSS");
    }

    #[test]
    fn beacon_text_submit_matches_cpp_multiplayer_single_selection_gate() {
        assert!(should_submit_beacon_text(true, 1));
        assert!(!should_submit_beacon_text(false, 1));
        assert!(!should_submit_beacon_text(true, 0));
        assert!(!should_submit_beacon_text(true, 2));
    }

    #[test]
    fn idle_worker_button_publishes_typed_host_request_when_bridge_enabled() {
        let _guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        crate::gui::control_bar::set_host_control_bar_bridge_enabled(true);

        let mut callbacks = ControlBarCallbacks::new();
        callbacks.handle_button_selected(
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker"),
            false,
        );

        assert!(matches!(
            crate::gui::control_bar::take_host_control_bar_requests().as_slice(),
            [crate::gui::control_bar::HostControlBarRequest::SelectNextIdleWorker]
        ));
    }

    #[test]
    fn left_hud_host_request_captures_the_live_alternate_mouse_setting() {
        let _bridge_guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
        let previous_alternate_mouse = ini_global.read().use_alternate_mouse;

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            ini_global.write().use_alternate_mouse = true;
            crate::gui::control_bar::set_host_control_bar_bridge_enabled(true);

            let mut window = GameWindow::new();
            window
                .set_position(7, 443)
                .expect("synthetic LeftHUD position is valid");
            window
                .set_size(167, 152)
                .expect("synthetic LeftHUD size is valid");

            let mut callbacks = LeftHUDCallbacks::new();
            let click = 42usize | (491usize << 16);
            assert_eq!(
                callbacks.input(&window, WindowMessage::LeftDown, click, 0),
                WindowMsgHandled::Handled
            );

            let published = crate::gui::control_bar::take_host_minimap_interactions();
            assert!(matches!(
                published.as_slice(),
                [interaction]
                    if interaction.button
                        == crate::gui::control_bar::HostMinimapMouseButton::Left
                        && interaction.alternate_mouse
            ));
        }));

        ini_global.write().use_alternate_mouse = previous_alternate_mouse;
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }

    #[test]
    fn held_middle_button_bypasses_left_hud_before_host_minimap_publication() {
        let _bridge_guard = crate::gui::control_bar::acquire_host_control_bar_bridge_test_guard();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::gui::control_bar::set_host_control_bar_bridge_enabled(true);

            let mut window = GameWindow::new();
            window
                .set_position(7, 443)
                .expect("synthetic LeftHUD position is valid");
            window
                .set_size(167, 152)
                .expect("synthetic LeftHUD size is valid");

            with_mouse(|mouse| {
                let _ =
                    mouse.handle_mouse_button(MouseButton::Middle, true, std::time::Instant::now());
            });

            let mut callbacks = LeftHUDCallbacks::new();
            let click = 42usize | (491usize << 16);
            for message in [WindowMessage::LeftDown, WindowMessage::RightDown] {
                assert_eq!(
                    callbacks.input(&window, message, click, 0),
                    WindowMsgHandled::Ignored,
                    "C++ LeftHUDInput yields to the held-middle route before {message:?}"
                );
            }
            assert!(
                crate::gui::control_bar::take_host_minimap_interactions().is_empty(),
                "a held middle button must not publish an authoritative minimap click"
            );
        }));

        with_mouse(|mouse| {
            let _ =
                mouse.handle_mouse_button(MouseButton::Middle, false, std::time::Instant::now());
        });
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    }
}

/// Residual: last ControlBar action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualControlBarAction {
    None = 0,
    Show = 1,
    Hide = 2,
    Toggle = 3,
    Options = 4,
    IdleWorker = 5,
    General = 6,
    Large = 7,
    PlaceBeacon = 8,
    DeleteBeacon = 9,
    ClearBeaconText = 10,
    Communicator = 11,
}

static RESIDUAL_CONTROL_BAR_ACTION: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_CONTROL_BAR_VISIBLE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

fn residual_control_bar_action_store(action: ResidualControlBarAction) {
    RESIDUAL_CONTROL_BAR_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last ControlBar residual action.
pub fn residual_control_bar_last_action() -> ResidualControlBarAction {
    match RESIDUAL_CONTROL_BAR_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualControlBarAction::Show,
        2 => ResidualControlBarAction::Hide,
        3 => ResidualControlBarAction::Toggle,
        4 => ResidualControlBarAction::Options,
        5 => ResidualControlBarAction::IdleWorker,
        6 => ResidualControlBarAction::General,
        7 => ResidualControlBarAction::Large,
        8 => ResidualControlBarAction::PlaceBeacon,
        9 => ResidualControlBarAction::DeleteBeacon,
        10 => ResidualControlBarAction::ClearBeaconText,
        11 => ResidualControlBarAction::Communicator,
        _ => ResidualControlBarAction::None,
    }
}

/// Residual: ControlBar visibility latch (independent of live layout).
pub fn residual_control_bar_is_visible() -> bool {
    RESIDUAL_CONTROL_BAR_VISIBLE.load(std::sync::atomic::Ordering::Relaxed)
}

fn with_control_bar_callbacks_mut<R>(f: impl FnOnce(&mut ControlBarCallbacks) -> R) -> R {
    let system = get_control_bar_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    let callbacks = system.get_callbacks();
    let mut callbacks = callbacks.write().unwrap_or_else(|e| e.into_inner());
    f(&mut callbacks)
}

/// Residual: bind ControlBar control name keys (no layout load).
pub fn simulate_control_bar_bind_controls() -> bool {
    with_control_bar_callbacks_mut(|cb| {
        cb.button_communicator = NameKeyGenerator::name_to_key("ControlBar.wnd:PopupCommunicator");
        let _ = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonOptions");
        let _ = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker");
        let _ = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonGeneral");
        let _ = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonLarge");
        let _ = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonPlaceBeacon");
        let _ = NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent");
        true
    })
}

/// Residual: show ControlBar without animate/layout.
pub fn simulate_control_bar_show() -> bool {
    with_control_bar_callbacks_mut(|cb| {
        cb.state.visible = true;
        RESIDUAL_CONTROL_BAR_VISIBLE.store(true, std::sync::atomic::Ordering::Relaxed);
        residual_control_bar_action_store(ResidualControlBarAction::Show);
        residual_control_bar_is_visible()
    })
}

/// Residual: hide ControlBar without animate/layout.
pub fn simulate_control_bar_hide() -> bool {
    with_control_bar_callbacks_mut(|cb| {
        cb.state.visible = false;
        RESIDUAL_CONTROL_BAR_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
        residual_control_bar_action_store(ResidualControlBarAction::Hide);
        !residual_control_bar_is_visible()
    })
}

/// Residual: toggle ControlBar visibility without animate.
pub fn simulate_control_bar_toggle() -> bool {
    with_control_bar_callbacks_mut(|cb| {
        cb.state.visible = !cb.state.visible;
        RESIDUAL_CONTROL_BAR_VISIBLE.store(cb.state.visible, std::sync::atomic::Ordering::Relaxed);
        residual_control_bar_action_store(ResidualControlBarAction::Toggle);
        true
    })
}

/// Residual: fire ButtonOptions without opening options layout.
pub fn simulate_control_bar_options_button_gadget_selected() -> bool {
    let _ = simulate_control_bar_bind_controls();
    residual_control_bar_action_store(ResidualControlBarAction::Options);
    true
}

/// Residual: fire ButtonIdleWorker without idle worker cycle.
pub fn simulate_control_bar_idle_worker_button_gadget_selected() -> bool {
    let _ = simulate_control_bar_bind_controls();
    residual_control_bar_action_store(ResidualControlBarAction::IdleWorker);
    true
}

/// Residual: fire ButtonGeneral without generals exp UI.
pub fn simulate_control_bar_general_button_gadget_selected() -> bool {
    let _ = simulate_control_bar_bind_controls();
    residual_control_bar_action_store(ResidualControlBarAction::General);
    true
}

/// Residual: fire ButtonLarge without control bar size toggle side effects.
pub fn simulate_control_bar_large_button_gadget_selected() -> bool {
    let _ = simulate_control_bar_bind_controls();
    residual_control_bar_action_store(ResidualControlBarAction::Large);
    true
}

/// Residual: fire ButtonPlaceBeacon without pending command set.
pub fn simulate_control_bar_place_beacon_button_gadget_selected() -> bool {
    let _ = simulate_control_bar_bind_controls();
    residual_control_bar_action_store(ResidualControlBarAction::PlaceBeacon);
    true
}

/// Residual: fire ButtonDeleteBeacon without multiplayer delete.
pub fn simulate_control_bar_delete_beacon_button_gadget_selected() -> bool {
    let _ = simulate_control_bar_bind_controls();
    residual_control_bar_action_store(ResidualControlBarAction::DeleteBeacon);
    true
}

/// Residual: fire ButtonClearBeaconText without edit clear.
pub fn simulate_control_bar_clear_beacon_text_button_gadget_selected() -> bool {
    let _ = simulate_control_bar_bind_controls();
    residual_control_bar_action_store(ResidualControlBarAction::ClearBeaconText);
    true
}

/// Residual: fire PopupCommunicator without diplomacy toggle.
pub fn simulate_control_bar_communicator_button_gadget_selected() -> bool {
    let _ = simulate_control_bar_bind_controls();
    residual_control_bar_action_store(ResidualControlBarAction::Communicator);
    true
}

/// Residual: show + Options composite (in-game options path honesty).
pub fn simulate_control_bar_prepare_options() -> bool {
    if !simulate_control_bar_bind_controls() {
        return false;
    }
    if !simulate_control_bar_show() {
        return false;
    }
    simulate_control_bar_options_button_gadget_selected()
}
