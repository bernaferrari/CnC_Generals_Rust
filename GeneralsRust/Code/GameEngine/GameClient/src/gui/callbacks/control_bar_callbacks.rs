//! Control Bar Callback Functions
//!
//! Implements in-game control bar callbacks and radar input handling, matching
//! the behavior of C++ ControlBarCallback.cpp.

use crate::display::view::{Point3, with_tactical_view};
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
    WindowMsgHandled, hide_quit_menu, toggle_diplomacy, toggle_quit_menu,
};
use crate::helpers::TheControlBar;

use crate::input::MouseButton;
use crate::language_filter::get_language_filter;
use crate::message_stream::{GameMessage, GameMessageType};
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::radar::{ICoord2D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH};
use gamelogic::commands::command::CommandType;
use gamelogic::commands::selection::get_selection_manager;
use gamelogic::helpers::{TheGameLogic, TheScriptEngine};
use gamelogic::player::ThePlayerList;

pub use super::control_bar_observation::{
    ControlBarCallbackObservation, ControlBarMouseObservation, ControlBarPlayerObservation,
};
fn receiving_callback_observation(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> Option<ControlBarCallbackObservation> {
    if let Some(facts) = effects.control_bar_callback_observation() {
        return Some(facts);
    }
    let role = effects.menus.as_ref()?.input_authority;
    if role != crate::core::game_client::ClientInputAuthority::StandaloneCore {
        return None;
    }
    // Explicitly bounded unmigrated Core service adapter; native never enters.
    let local_player = ThePlayerList()
        .read()
        .ok()
        .and_then(|list| list.get_local_player().cloned())
        .and_then(|player| {
            player
                .read()
                .ok()
                .map(|player| ControlBarPlayerObservation {
                    id: player.get_player_index() as u32,
                    active: player.is_player_active(),
                    selection_count: selection_count(),
                })
        });
    let local_player_side = local_player.as_ref().map(|_| local_control_bar_player_side());
    Some(ControlBarCallbackObservation {
        game_ending: TheScriptEngine::is_game_ending(),
        multiplayer: TheGameLogic::is_in_multiplayer_game(),
        local_player,
        local_player_side,
        mouse: Some(ControlBarMouseObservation {
            middle_held: effects
                .cursor_mouse()
                .is_some_and(|mouse| mouse.get_mouse_status().is_button_down(MouseButton::Middle)),
            alternate_mouse: is_alternate_mouse_enabled(),
        }),
    })
}

const CMD_NEED_TARGET_POS: u32 = 0x0000_0020;
const CMD_ATTACK_OBJECTS_POSITION: u32 = 0x0000_1000;
const KEY_ESC: usize = 0x1B;

/// Borrowed writer into the camera belonging to the receiving client.
/// Display dimensions and the resulting integer height share that owner.
pub trait ControlBarViewport {
    fn display_size(&self) -> (u32, u32);
    fn set_height_px(&mut self, height: i32);
    fn camera_input(
        &mut self,
        _action: game_engine::common::message_stream::CameraInputAction,
    ) -> Option<game_engine::common::message_stream::CameraInputObservation> {
        None
    }
}

/// Standalone layout dimensions are captured from the receiving manager.
/// Native clients supply their SessionCamera through ControlBarViewport.
pub struct StandaloneControlBarViewport<'a> {
    dimensions: (u32, u32),
    view: &'a mut crate::display::view::View,
}
impl<'a> StandaloneControlBarViewport<'a> {
    pub fn new(
        manager: &crate::gui::window_manager::WindowManager,
        view: &'a mut crate::display::view::View,
    ) -> Self {
        let (width, height) = manager.screen_size();
        Self {
            dimensions: (width.max(0) as u32, height.max(0) as u32),
            view,
        }
    }
}
impl ControlBarViewport for StandaloneControlBarViewport<'_> {
    fn display_size(&self) -> (u32, u32) {
        self.dimensions
    }
    fn set_height_px(&mut self, height: i32) {
        self.view.set_height(height);
    }
    fn camera_input(
        &mut self,
        action: game_engine::common::message_stream::CameraInputAction,
    ) -> Option<game_engine::common::message_stream::CameraInputObservation> {
        Some(apply_camera_input(self.view, action))
    }
}

/// Execute one original camera operation on an explicitly borrowed standalone view.
pub fn apply_camera_input(
    view: &mut crate::display::view::View,
    action: game_engine::common::message_stream::CameraInputAction,
) -> game_engine::common::message_stream::CameraInputObservation {
    use game_engine::common::message_stream::{
        CameraInputAction as Action, CameraInputObservation, CameraLocation,
    };
    match action {
        Action::Observe => {}
        Action::SetMouseLock(value) => view.set_mouse_lock(value),
        Action::ClearTracking => {
            view.set_camera_lock(None);
            view.set_camera_lock_drawable(None);
        }
        Action::Scroll { x, y } => view.scroll_by(&crate::display::view::Vector2::new(x, y)),
        Action::Rotate(delta) => view.set_angle(view.angle() + delta),
        Action::Pitch(delta) => view.set_pitch(view.pitch() + delta),
        Action::ZoomIn => view.zoom_in(),
        Action::ZoomOut => view.zoom_out(),
        Action::ResetPose => {
            view.set_angle_and_pitch_to_default();
            view.set_zoom_to_default();
        }
        Action::SetLocation(location) => {
            if location.valid {
                let mut value = crate::display::view::ViewLocation::new();
                value.init(
                    location.position[0],
                    location.position[1],
                    location.position[2],
                    location.angle,
                    location.pitch,
                    location.zoom,
                );
                view.set_location(&value);
            }
        }
    }
    let location = view.get_location();
    let position = location.position();
    CameraInputObservation {
        width: view.width(),
        height: view.height(),
        movement_finished: view.is_camera_movement_finished(),
        location: CameraLocation {
            position: [position.x, position.y, position.z],
            angle: location.angle(),
            pitch: location.pitch(),
            zoom: location.zoom(),
            valid: location.is_valid(),
        },
    }
}

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

fn radar_allows_input(ui: &mut crate::core::subsystems::InGameUISubsystem) -> bool {
    ui.radar_presentation()
        .is_some_and(|radar| radar.is_radar_shown())
}

fn local_pixel_to_radar(
    local_x: i32,
    local_y: i32,
    width: i32,
    height: i32,
    ui: &mut crate::core::subsystems::InGameUISubsystem,
) -> Option<ICoord2D> {
    use crate::gui::ingame_ui::local_pixel_to_radar_cell;
    let extent = ui.radar_presentation()?.map_extent();
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

fn has_pending_radar_targeting_mode(ui: &mut crate::core::subsystems::InGameUISubsystem) -> bool {
    if let Some(pending_power) = ui.get_pending_special_power() {
        if (pending_power.options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION)) != 0 {
            return true;
        }
    }

    if let Some(pending_command) = ui.get_pending_command() {
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

fn refresh_radar_cursor(msg: WindowMessage, ui: &mut crate::core::subsystems::InGameUISubsystem) {
    if msg == WindowMessage::MouseLeaving {
        ui.set_cursor_arrow();
        return;
    }

    if has_pending_radar_targeting_mode(ui) {
        // C++ LeftHUDInput: keep a targeted command cursor over radar, falling
        // back to CROSS when no valid cursor name is available.
        let cursor_name = ui
            .get_pending_command()
            .map(|pending| pending.cursor_name)
            .filter(|name| !name.is_empty());
        ui.set_cursor_by_name(radar_targeting_cursor_name(cursor_name.as_deref()));
        return;
    }

    if selection_is_empty() {
        ui.set_cursor_arrow();
        return;
    }

    if ui.is_in_attack_move_to_mode() {
        ui.set_cursor_by_name("ATTACKMOVETO");
    } else {
        ui.set_cursor_by_name("MOVETO");
    }
}

/// Control bar callback handler
pub struct ControlBarCallbacks {
    state: ControlBarState,
    animate_manager: AnimateWindowManager,
    button_communicator: u32,
    last_action: ResidualControlBarAction,
}

impl ControlBarCallbacks {
    pub fn new() -> Self {
        Self {
            state: ControlBarState::default(),
            animate_manager: AnimateWindowManager::new(),
            button_communicator: 0,
            last_action: ResidualControlBarAction::None,
        }
    }

    /// CPP ControlBar::update advances the actual window animation manager
    /// during the admitted UI update, then hides a completed reversed parent.
    pub fn update_window_animations(
        &mut self,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        self.update_window_animations_at(parent, std::time::Instant::now(), receiving_manager);
    }

    pub fn update_window_animations_at(
        &mut self,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        now: std::time::Instant,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        self.animate_manager.update_at(now);
        if self.animate_manager.is_finished() && self.animate_manager.is_reversed() {
            if let Some(parent) = parent {
                let _ = parent.borrow_mut().hide(true, receiving_manager);
                self.state.visible = false;
            }
        }
    }

    /// Show the receiving bar. Parent lookup is performed by the caller;
    /// no process-global control bar or tactical view participates.
    pub fn show_control_bar_owned(
        &mut self,
        bar: &mut ControlBar,
        viewport: &mut dyn ControlBarViewport,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        replay: bool,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.show_control_bar_owned_with_upgrade_source(bar, viewport, parent, replay, immediate, crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone, receiving_manager, effects);
    }
    pub fn show_control_bar_owned_with_upgrade_source(
        &mut self,
        bar: &mut ControlBar,
        viewport: &mut dyn ControlBarViewport,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        replay: bool,
        immediate: bool,
        upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'_>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.show_control_bar_receiving(
            bar,
            viewport,
            parent,
            replay,
            immediate,
            upgrade_source,
            TheScriptEngine::is_game_ending(),
            receiving_manager,
            effects,
        );
    }
    pub fn show_control_bar_receiving(
        &mut self,
        bar: &mut ControlBar,
        viewport: &mut dyn ControlBarViewport,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        replay: bool,
        immediate: bool,
        upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'_>,
        game_ending: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        bar.show_special_power_shortcut_receiving(
            upgrade_source,
            game_ending,
            receiving_manager,
            effects,
        );
        self.finish_show_control_bar(bar, viewport, parent, replay, immediate, receiving_manager);
    }

    fn finish_show_control_bar(
        &mut self,
        bar: &mut ControlBar,
        viewport: &mut dyn ControlBarViewport,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        replay: bool,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        if let Some(parent) = parent {
            bar.switch_control_bar_stage_on_window_owned(
                ControlBarStage::Default,
                viewport,
                replay,
                Some(parent.clone()),
                receiving_manager,
            );
            let (_, height) = viewport.display_size();
            // Show writes 80% even when replay makes stage-switch inert.
            viewport.set_height_px((height as f32 * 0.80) as i32);
            if !immediate {
                let (width, height) = viewport.display_size();
                self.animate_manager
                    .set_screen_size(width as i32, height as i32);
                self.animate_manager.reset();
                self.animate_manager.register_window(
                    parent.clone(),
                    AnimationType::SlideBottom,
                    true,
                    500,
                    0,
                );
                bar.animate_special_power_shortcut(true);
            }
            let _ = parent.borrow_mut().hide(false, receiving_manager);
            self.state.visible = true;
        }
        bar.mark_ui_dirty();
    }

    pub fn hide_control_bar_owned(
        &mut self,
        bar: &mut ControlBar,
        viewport: &mut dyn ControlBarViewport,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        window_manager_present: bool,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        bar.hide_special_power_shortcut(receiving_manager);
        if window_manager_present {
            if let Some(parent) = parent {
                viewport.set_height_px(viewport.display_size().1 as i32);
                if immediate {
                    let _ = parent.borrow_mut().hide(true, receiving_manager);
                    bar.hide_special_power_shortcut(receiving_manager);
                }
                self.state.visible = false;
            }
            if !immediate {
                self.animate_manager.reverse_animate_window();
                bar.animate_special_power_shortcut(false);
            }
            bar.hide_purchase_science(receiving_manager);
        }
    }

    pub fn toggle_control_bar_owned(
        &mut self,
        bar: &mut ControlBar,
        viewport: &mut dyn ControlBarViewport,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        replay: bool,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.toggle_control_bar_owned_with_upgrade_source(bar, viewport, parent, replay, immediate, crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone, receiving_manager, effects);
    }
    pub fn toggle_control_bar_owned_with_upgrade_source(
        &mut self,
        bar: &mut ControlBar,
        viewport: &mut dyn ControlBarViewport,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        replay: bool,
        immediate: bool,
        upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'_>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.toggle_control_bar_receiving(
            bar,
            viewport,
            parent,
            replay,
            immediate,
            upgrade_source,
            TheScriptEngine::is_game_ending(),
            receiving_manager,
            effects,
        );
    }
    pub fn toggle_control_bar_receiving(
        &mut self,
        bar: &mut ControlBar,
        viewport: &mut dyn ControlBarViewport,
        parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
        replay: bool,
        immediate: bool,
        upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'_>,
        game_ending: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        let Some(parent) = parent else {
            return;
        };
        let show = parent.borrow().is_hidden();
        if show {
            bar.show_special_power_shortcut_receiving(
                upgrade_source,
                game_ending,
                receiving_manager,
                effects,
            );
        }
        self.finish_toggle_control_bar(
            bar,
            viewport,
            parent,
            show,
            replay,
            immediate,
            receiving_manager,
        );
    }

    fn finish_toggle_control_bar(
        &mut self,
        bar: &mut ControlBar,
        viewport: &mut dyn ControlBarViewport,
        parent: std::rc::Rc<std::cell::RefCell<GameWindow>>,
        show: bool,
        replay: bool,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        if show {
            viewport.set_height_px((viewport.display_size().1 as f32 * 0.80) as i32);
            let _ = parent.borrow_mut().hide(false, receiving_manager);
            bar.switch_control_bar_stage_on_window_owned(
                ControlBarStage::Default,
                viewport,
                replay,
                Some(parent.clone()),
                receiving_manager,
            );
            if !immediate {
                let (width, height) = viewport.display_size();
                self.animate_manager
                    .set_screen_size(width as i32, height as i32);
                self.animate_manager.reset();
                self.animate_manager.register_window(
                    parent.clone(),
                    AnimationType::SlideBottom,
                    true,
                    500,
                    0,
                );
                bar.animate_special_power_shortcut(true);
            }
            self.state.visible = true;
        } else {
            bar.hide_special_power_shortcut(receiving_manager);
            viewport.set_height_px(viewport.display_size().1 as i32);
            let _ = parent.borrow_mut().hide(true, receiving_manager);
            self.state.visible = false;
        }
    }

    /// Handle control bar system messages
    pub fn system(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        let Some(observation) = receiving_callback_observation(effects) else {
            return WindowMsgHandled::Ignored;
        };
        if observation.game_ending {
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
                TheControlBar::process_context_sensitive_button_transition(
                    control_id,
                    entering,
                    effects.ui_mut(),
                );
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetSelected | WindowMessage::GadgetRightClick => {
                let control_id = data1 as u32;
                Self::handle_button_selected(
                    self.button_communicator,
                    control_id,
                    msg == WindowMessage::GadgetRightClick,
                    effects,
                    Some(&mut *_window),
                );
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetEditDone => {
                effects.with_manager(|receiving_manager, effects| {
                    Self::handle_edit_done(
                        data1 as u32,
                        effects.emitted_messages,
                        receiving_manager,
                        observation,
                    )
                });
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn handle_button_selected(
        button_communicator: u32,
        control_id: u32,
        right_click: bool,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
        mut selected_window: Option<&mut crate::gui::GameWindow>,
    ) {
        let Some(observation) = receiving_callback_observation(effects) else {
            return;
        };
        let input_provenance = effects.input_provenance();
        let beacon_place_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonPlaceBeacon");
        let beacon_delete_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonDeleteBeacon");
        let beacon_clear_text_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonClearBeaconText");
        let beacon_general_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonGeneral");
        let button_large_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonLarge");
        let button_options_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonOptions");
        let button_idle_worker_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker");

        if control_id == button_communicator {
            let _ = effects.with_manager(|manager, _effects| {
                toggle_diplomacy(false, manager, _effects, selected_window.as_deref_mut())
            });
            return;
        }

        if control_id == beacon_place_id {
            if observation.multiplayer
                && observation.local_player.is_some_and(|player| player.active)
            {
                effects.ui_mut().clear_pending_special_power();
                effects.ui_mut().set_pending_command(
                    CommandType::PlaceBeacon,
                    CMD_NEED_TARGET_POS,
                    0,
                );
                effects.ui_mut().set_force_attack_mode(false);
                effects.ui_mut().set_force_move_to_mode(false);
                effects.ui_mut().set_prefer_selection_mode(false);
            }
            return;
        }

        if control_id == beacon_delete_id {
            if observation.multiplayer {
                effects
                    .emitted_messages
                    .push(GameMessage::new(GameMessageType::RemoveBeacon(
                        crate::message_stream::game_message::Coord3D::default(),
                    )));
            }
            return;
        }

        if control_id == beacon_clear_text_id {
            if observation.multiplayer {
                let text_id = NameKeyGenerator::name_to_key("ControlBar.wnd:EditBeaconText") as i32;
                effects.with_manager(|manager, effects| {
                    if let Some(handle) = manager.get_window_by_id(text_id) {
                        let _ = handle.borrow_mut().set_text("");
                    }
                });
            }
            return;
        }

        if control_id == beacon_general_id {
            effects.with_manager(|receiving_manager, effects| {
                hide_quit_menu(receiving_manager, selected_window.as_deref_mut(), effects)
            });
            TheControlBar::toggle_purchase_science(effects.ui_mut());
            return;
        }

        if control_id == button_large_id {
            TheControlBar::toggle_control_bar_stage(effects.ui_mut());
            return;
        }

        if control_id == button_options_id {
            toggle_quit_menu(effects, selected_window.as_deref_mut());
            return;
        }

        if control_id == button_idle_worker_id {
            effects.with_manager(|receiving_manager, effects| {
                hide_quit_menu(receiving_manager, selected_window.as_deref_mut(), effects)
            });
            // C++ calls InGameUI::selectNextIdleWorker directly here.  While
            // Main owns the match, send the same UI intent to its typed
            // selection boundary rather than its dormant legacy message path.
            if publish_host_select_next_idle_worker(effects.input_provenance(), effects.ui_mut()) {
                return;
            }
            // Standalone GameClient retains its existing legacy route.
            effects
                .emitted_messages
                .push(GameMessage::new(GameMessageType::MetaSelectNextWorker));
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
        if let Some(clicks) = effects.pending_control_bar_clicks.as_deref_mut() {
            clicks.push(crate::helpers::LiveControlBarClick::from_input(
                control_id,
                msg,
                input_provenance,
            ));
        }
    }

    fn handle_edit_done(
        control_id: u32,
        emitted_messages: &mut Vec<GameMessage>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        observation: ControlBarCallbackObservation,
    ) {
        let text_id = NameKeyGenerator::name_to_key("ControlBar.wnd:EditBeaconText");
        if control_id != text_id {
            return;
        }

        if should_submit_beacon_text(
            observation.multiplayer,
            observation
                .local_player
                .map(|player| player.selection_count)
                .unwrap_or(0),
        ) {
            let text = (|manager: &mut crate::gui::window_manager::WindowManager| {
                manager
                    .get_window_by_id(text_id as i32)
                    .map(|win| win.borrow().get_text().to_string())
            })(receiving_manager)
            .unwrap_or_default();
            let text = filter_beacon_edit_text(text);
            emitted_messages.push(GameMessage::new(GameMessageType::SetBeaconText(
                crate::message_stream::game_message::Coord3D::default(),
                text,
            )));
        }
    }

    fn toggle_control_bar_with_receiving_side(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
        local_player_side: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ ToggleControlBar: toggleReplayControls() then flip the bar.
        toggle_replay_controls(receiving_manager);
        if self.state.visible {
            self.hide_control_bar_window(immediate, receiving_manager, ui)?;
        } else {
            self.show_control_bar_window(
                immediate,
                receiving_manager,
                ui,
                local_player_side,
            )?;
        }
        Ok(())
    }

    /// Hide the control bar
    pub fn hide_control_bar(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ HideControlBar: hideReplayControls() even if already hidden.
        hide_replay_controls(receiving_manager);
        self.hide_control_bar_window(immediate, receiving_manager, ui)
    }

    fn show_control_bar_with_receiving_side(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
        local_player_side: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ ShowControlBar: showReplayControls() then switch DEFAULT + winHide(FALSE).
        show_replay_controls(receiving_manager);
        self.show_control_bar_window(
            immediate,
            receiving_manager,
            ui,
            local_player_side,
        )
    }

    fn hide_control_bar_window(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.state.visible {
            return Ok(());
        }
        self.state.visible = false;
        TheControlBar::hide_special_power_shortcut(ui);
        self.apply_visibility_change(immediate, false, receiving_manager, ui, None)?;
        Ok(())
    }

    fn show_control_bar_window(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
        local_player_side: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ ShowControlBar always switchControlBarStage(DEFAULT) + winHide(FALSE)
        // even when already shown. Do not early-return on state.visible.
        if !self.state.visible {
            self.state.visible = true;
            TheControlBar::show_special_power_shortcut(ui);
        }
        self.apply_visibility_change(immediate, true, receiving_manager, ui, local_player_side)?;
        Ok(())
    }

    /// C++ GameLogic.cpp:2233 setControlBarSchemeByPlayer + ControlBarCallback.cpp:489 DEFAULT.
    fn apply_show_control_bar_scheme_and_default_stage(
        &self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
        local_player_side: Option<&str>,
    ) {
        let parent_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent") as i32;
        let parent_live = (|manager: &mut crate::gui::window_manager::WindowManager| {
            manager.get_window_by_id(parent_id).is_some()
        })(receiving_manager);
        if !parent_live {
            return;
        }
        if let Some(side) = local_player_side {
            TheControlBar::set_control_bar_scheme_by_player(side, ui);
        }
        ControlBar::request_switch_control_bar_stage(ControlBarStage::Default, ui);
    }

    /// Check if control bar is visible
    pub fn is_visible(&self) -> bool {
        self.state.visible
    }

    fn apply_visibility_change(
        &mut self,
        immediate: bool,
        show: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
        local_player_side: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let control_bar_id =
            NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent") as i32;
        let (screen_w, screen_h) =
            (|manager: &mut crate::gui::window_manager::WindowManager| manager.screen_size())(
                receiving_manager,
            );
        self.animate_manager.set_screen_size(screen_w, screen_h);

        let mut parent_live = false;
        (|manager: &mut crate::gui::window_manager::WindowManager| {
            if let Some(handle) = manager.get_window_by_id(control_bar_id) {
                parent_live = true;
                if immediate {
                    let _ = handle.borrow_mut().hide(!show, manager);
                } else {
                    self.animate_manager.reset();
                    self.animate_manager.register_window(
                        handle.clone(),
                        AnimationType::SlideBottom,
                        true,
                        500,
                        0,
                    );
                    let _ = handle.borrow_mut().hide(false, manager);
                    if !show {
                        self.animate_manager.reverse_animate_window();
                    }
                }
            }
        })(receiving_manager);

        let target_height = if show {
            (screen_h as f32 * 0.80f32) as i32
        } else {
            screen_h
        };
        // C++ HideControlBar setHeight(Display) / ShowControlBar 80%. Live 3D
        // Standalone adapter writes its Core View directly.
        if parent_live {
            with_tactical_view(|view| {
                view.set_height(target_height);
            });
        }
        if show {
            if parent_live {
                // C++ ShowControlBar: scheme is applied at InGame start; stage DEFAULT
                // when ControlBarParent exists (ControlBarCallback.cpp:489).
                self.apply_show_control_bar_scheme_and_default_stage(
                    receiving_manager,
                    ui,
                    local_player_side,
                );
            }
            TheControlBar::animate_special_power_shortcut(true, ui);
        } else {
            TheControlBar::animate_special_power_shortcut(false, ui);
            TheControlBar::hide_purchase_science(ui);
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
    _window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char && data1 == KEY_ESC {
        if effects.input_authority() == crate::core::game_client::ClientInputAuthority::NativeHosted
        {
            let Some(observation) = receiving_callback_observation(effects) else {
                return WindowMsgHandled::Ignored;
            };
            let Some(player) = observation.local_player else {
                return WindowMsgHandled::Ignored;
            };
            effects.request_menu_host(
                crate::gui::campaign_launch_host_bridge::MenuHostRequest::ClearSelection {
                    player_id: player.id,
                },
            );
        } else {
            clear_local_selection();
        }
        return WindowMsgHandled::Handled;
    }

    WindowMsgHandled::Ignored
}

/// Left HUD input handler

/// Handle left HUD input messages
pub fn left_hud_input(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    let Some(observation) = receiving_callback_observation(effects) else {
        return WindowMsgHandled::Ignored;
    };
    let Some(mouse) = observation.mouse else {
        return WindowMsgHandled::Ignored;
    };
    let native =
        effects.input_authority() == crate::core::game_client::ClientInputAuthority::NativeHosted;
    let input_provenance = effects.input_provenance();
    let emitted_messages = &mut *effects.emitted_messages;
    let ui = effects
        .ui
        .as_deref_mut()
        .expect("LeftHUD requires receiving UI");
    let host_bridge = host_control_bar_bridge_enabled(ui);
    if native && !host_bridge {
        return WindowMsgHandled::Ignored;
    }

    // C++ first eats input when the radar is unavailable, then checks
    // held MMB.  Main owns radar authority while the bridge is active,
    // so defer the equivalent check to its existing presentation-side
    // gate rather than consulting the dormant standalone globals.
    if !host_bridge && !radar_allows_input(ui) {
        return WindowMsgHandled::Handled;
    }

    // C++ ControlBarCallback.cpp checks middleState before its switch.
    // In particular, do not turn an LMB/RMB held alongside MMB into an
    // authoritative-host minimap interaction; returning Ignored lets the
    // normal middle-mouse route retain ownership.
    if mouse.middle_held {
        return WindowMsgHandled::Ignored;
    }

    // The executable owns the Rust simulation, not this standalone
    // GameClient's legacy GameLogic globals.  Send real LeftHUD clicks to
    // Main before consulting those globals, while leaving the standalone
    // path below exactly intact when the bridge is disabled.
    if host_bridge && matches!(msg, WindowMessage::LeftDown | WindowMessage::RightDown) {
        left_hud_publish_host_minimap_mouse_down(
            window,
            msg,
            data1,
            ui,
            input_provenance,
            mouse.alternate_mouse,
        );
        return WindowMsgHandled::Handled;
    }

    if !radar_allows_input(ui) {
        return WindowMsgHandled::Handled;
    }

    match msg {
        WindowMessage::MiddleDown => WindowMsgHandled::Ignored,
        WindowMessage::None | WindowMessage::MouseEntering | WindowMessage::MouseLeaving => {
            refresh_radar_cursor(msg, ui);
            WindowMsgHandled::Handled
        }
        WindowMessage::MousePos => {
            left_hud_handle_mouse_pos(window, data1, ui);
            WindowMsgHandled::Handled
        }
        WindowMessage::LeftDown | WindowMessage::RightDown => {
            left_hud_handle_mouse_down(window, msg, data1, emitted_messages, ui);
            WindowMsgHandled::Handled
        }
        WindowMessage::LeftUp | WindowMessage::RightUp => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    }
}

fn left_hud_handle_mouse_pos(
    window: &GameWindow,
    data1: WindowMsgData,
    ui: &mut crate::core::subsystems::InGameUISubsystem,
) {
    let (mouse_x, mouse_y) = decode_mouse_pos(data1);
    let (screen_x, screen_y) = window.get_screen_position();
    let (width, height) = window.get_size();
    let local_x = mouse_x - screen_x;
    let local_y = mouse_y - screen_y;
    if local_pixel_to_radar(local_x, local_y, width, height, ui).is_some() {
        refresh_radar_cursor(WindowMessage::MousePos, ui);
    }
}

/// Publish a real `ControlBar.wnd:LeftHUD` click for Main's WGPU minimap.
///
/// We only establish that the pointer belongs to this WND rectangle here.
/// World conversion and FOW remain at Main's authoritative presentation
/// boundary. The receiving callback supplies its copied native input origin;
/// `FromUser` alone never implies a physical click.
fn left_hud_publish_host_minimap_mouse_down(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    ui: &mut crate::core::subsystems::InGameUISubsystem,
    input_provenance: crate::gui::control_bar::HostControlBarInputProvenance,
    alternate_mouse: bool,
) {
    let (mouse_x, mouse_y) = decode_mouse_pos(data1);
    let (screen_x, screen_y) = window.get_screen_position();
    let (width, height) = window.get_size();
    let local_x = mouse_x - screen_x;
    let local_y = mouse_y - screen_y;
    if local_pixel_to_radar(local_x, local_y, width, height, ui).is_none() {
        return;
    }

    let button = match msg {
        WindowMessage::LeftDown => HostMinimapMouseButton::Left,
        WindowMessage::RightDown => HostMinimapMouseButton::Right,
        _ => return,
    };
    let _ = publish_host_minimap_interaction(
        input_provenance,
        HostMinimapInteractionRequest {
            screen_position: [mouse_x, mouse_y],
            screen_top_left: [screen_x, screen_y],
            screen_size: [width, height],
            button,
            alternate_mouse,
        },
        ui,
    );
}

fn left_hud_handle_mouse_down(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    emitted_messages: &mut Vec<GameMessage>,
    ui: &mut crate::core::subsystems::InGameUISubsystem,
) {
    let (mouse_x, mouse_y) = decode_mouse_pos(data1);
    let (screen_x, screen_y) = window.get_screen_position();
    let (width, height) = window.get_size();
    let local_x = mouse_x - screen_x;
    let local_y = mouse_y - screen_y;
    let Some(radar_pos) = local_pixel_to_radar(local_x, local_y, width, height, ui) else {
        return;
    };
    let Some(radar) = ui.radar_presentation() else {
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
        let world_pos =
            crate::message_stream::game_message::Coord3D::new(world.x, world.y, world.z);

        if let Some(pending_power) = ui.get_pending_special_power() {
            if (pending_power.options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION)) != 0 {
                emitted_messages.push(GameMessage::new(GameMessageType::DoSpecialPowerAtLocation(
                    pending_power.power_id,
                    world_pos,
                    0.0,
                    0,
                    pending_power.options,
                    pending_power.source_object_id,
                )));
                ui.clear_pending_special_power();
                ui.clear_attack_move_to_mode();
                return;
            }
        }

        if let Some(pending_command) = ui.get_pending_command() {
            if (pending_command.options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION)) != 0
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
                    emitted_messages.push(GameMessage::new(message));
                    ui.clear_pending_command();
                    ui.clear_attack_move_to_mode();
                    return;
                }
            }
        }

        if ui.is_in_attack_move_to_mode() {
            emitted_messages.push(GameMessage::new(GameMessageType::DoAttackMoveTo(
                world_pos.clone(),
            )));
        } else {
            emitted_messages.push(GameMessage::new(GameMessageType::DoMoveTo(world_pos)));
        }
    }

    ui.clear_attack_move_to_mode();
}
/// Receiving control-bar operations; no global callback owner exists.
pub fn show_control_bar_receiving(
    bar: &mut ControlBar,
    viewport: &mut dyn ControlBarViewport,
    parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
    replay: bool,
    immediate: bool,
    upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<
        '_,
    >,
    game_ending: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let had_parent = parent.is_some();
    bar.show_special_power_shortcut_receiving(
        upgrade_source,
        game_ending,
        receiving_manager,
        effects,
    );
    effects.control_bar_callbacks_mut().finish_show_control_bar(
        bar,
        viewport,
        parent,
        replay,
        immediate,
        receiving_manager,
    );
    if had_parent {
        effects.control_bar_callbacks_mut().last_action = ResidualControlBarAction::Show;
    }
}

pub fn toggle_control_bar_receiving(
    bar: &mut ControlBar,
    viewport: &mut dyn ControlBarViewport,
    parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
    replay: bool,
    immediate: bool,
    upgrade_source: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<
        '_,
    >,
    game_ending: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let Some(parent) = parent else {
        return;
    };
    let show = parent.borrow().is_hidden();
    if show {
        bar.show_special_power_shortcut_receiving(
            upgrade_source,
            game_ending,
            receiving_manager,
            effects,
        );
    }
    effects
        .control_bar_callbacks_mut()
        .finish_toggle_control_bar(
            bar,
            viewport,
            parent,
            show,
            replay,
            immediate,
            receiving_manager,
        );
    effects.control_bar_callbacks_mut().last_action = ResidualControlBarAction::Toggle;
}

pub fn receiving_control_bar_system(
    _window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    let Some(observation) = receiving_callback_observation(effects) else {
        return WindowMsgHandled::Ignored;
    };
    if observation.game_ending {
        return WindowMsgHandled::Ignored;
    }

    match msg {
        WindowMessage::Create => {
            effects.control_bar_callbacks_mut().button_communicator =
                NameKeyGenerator::name_to_key("ControlBar.wnd:PopupCommunicator");
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetMouseEntering | WindowMessage::GadgetMouseLeaving => {
            let control_id = data1 as u32;
            let entering = msg == WindowMessage::GadgetMouseEntering;
            TheControlBar::process_context_sensitive_button_transition(
                control_id,
                entering,
                effects.ui_mut(),
            );
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetSelected | WindowMessage::GadgetRightClick => {
            let control_id = data1 as u32;
            let communicator = effects.control_bar_callbacks().button_communicator;
            ControlBarCallbacks::handle_button_selected(
                communicator,
                control_id,
                msg == WindowMessage::GadgetRightClick,
                effects,
                Some(&mut *_window),
            );
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            effects.with_manager(|receiving_manager, effects| {
                ControlBarCallbacks::handle_edit_done(
                    data1 as u32,
                    effects.emitted_messages,
                    receiving_manager,
                    observation,
                )
            });
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

pub fn hide_control_bar_owned(
    bar: &mut ControlBar,
    viewport: &mut dyn ControlBarViewport,
    parent: Option<std::rc::Rc<std::cell::RefCell<GameWindow>>>,
    manager_present: bool,
    immediate: bool,
    manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let had_parent = parent.is_some() && manager_present;
    effects.control_bar_callbacks_mut().hide_control_bar_owned(
        bar,
        viewport,
        parent,
        manager_present,
        immediate,
        manager,
    );
    if had_parent {
        effects.control_bar_callbacks_mut().last_action = ResidualControlBarAction::Hide;
    }
}
pub fn toggle_control_bar(
    immediate: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let observation = receiving_callback_observation(effects);
    let local_player_side = observation
        .as_ref()
        .and_then(|facts| facts.local_player_side.as_deref())
        .map(|side| if side.is_empty() { "Observer" } else { side });
    let (callbacks, ui) = effects.control_bar_callbacks_and_ui_mut();
    callbacks.toggle_control_bar_with_receiving_side(
        immediate,
        receiving_manager,
        ui,
        local_player_side,
    )
}
pub fn hide_control_bar(
    immediate: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (callbacks, ui) = effects.control_bar_callbacks_and_ui_mut();
    callbacks.hide_control_bar(immediate, receiving_manager, ui)
}
pub fn show_control_bar(
    immediate: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let observation = receiving_callback_observation(effects);
    let local_player_side = observation
        .as_ref()
        .and_then(|facts| facts.local_player_side.as_deref())
        .map(|side| if side.is_empty() { "Observer" } else { side });
    let (callbacks, ui) = effects.control_bar_callbacks_and_ui_mut();
    callbacks.show_control_bar_with_receiving_side(
        immediate,
        receiving_manager,
        ui,
        local_player_side,
    )
}
pub fn is_control_bar_visible(effects: &crate::gui::game_window::WindowEventEffects<'_>) -> bool {
    effects.control_bar_callbacks().is_visible()
}

#[cfg(test)]
mod tests {
    fn test_observation(
        game_ending: bool,
        middle_held: bool,
        alternate_mouse: bool,
    ) -> ControlBarCallbackObservation {
        ControlBarCallbackObservation {
            game_ending,
            multiplayer: true,
            local_player: Some(ControlBarPlayerObservation {
                id: 11,
                active: true,
                selection_count: 1,
            }),
            local_player_side: Some("America".to_string()),
            mouse: Some(ControlBarMouseObservation {
                middle_held,
                alternate_mouse,
            }),
        }
    }
    fn with_receiving_observation<R>(
        facts: ControlBarCallbackObservation,
        operation: impl FnOnce(&mut crate::gui::game_window::WindowEventEffects<'_>) -> R,
    ) -> R {
        let mut client = crate::core::GameClient::new_with_input_authority(
            crate::core::game_client::ClientInputAuthority::NativeHosted,
        )
        .unwrap();
        let mut ui = crate::core::subsystems::InGameUISubsystem::default();
        ui.admit_native_control_bar_transport();
        let mut radar = game_engine::common::system::radar::RadarSystem::new();
        radar.new_map_owned(
            game_engine::common::system::radar::Coord3D::new(0.0, 0.0, 0.0),
            game_engine::common::system::radar::Coord3D::new(100.0, 100.0, 0.0),
            &[],
        );
        ui.set_radar_presentation(std::sync::Arc::new(radar.presentation()));
        client.set_in_game_ui_subsystem(ui);
        client.with_window_event_effects(None, |effects| {
            effects.admit_control_bar_callback_observation(facts);
            operation(effects)
        })
    }
    fn left_hud_test_window(
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Rc<RefCell<GameWindow>> {
        effects.with_manager(|manager, effects| {
            manager
                .create_window(None, 7, 443, 167, 152, effects)
                .unwrap()
        })
    }

    use super::*;

    #[test]
    fn test_control_bar_callbacks() {
        with_receiving_observation(test_observation(false, false, false), |effects| {
            effects.with_manager(|manager, effects| {
                assert!(is_control_bar_visible(effects));
                assert!(hide_control_bar(true, manager, effects).is_ok());
                assert!(!is_control_bar_visible(effects));
                assert!(show_control_bar(true, manager, effects).is_ok());
                assert!(is_control_bar_visible(effects));
                assert!(toggle_control_bar(true, manager, effects).is_ok());
                assert!(!is_control_bar_visible(effects));
                assert!(toggle_control_bar(true, manager, effects).is_ok());
                assert!(is_control_bar_visible(effects));
            });
        });
    }

    #[test]
    fn callbacks_begin_with_owned_state() {
        let callbacks = ControlBarCallbacks::new();
        assert!(callbacks.is_visible());
        assert_eq!(callbacks.last_action, ResidualControlBarAction::None);
    }

    #[test]
    fn client_callback_actions_are_independent() {
        let mut a = crate::core::GameClient::new().unwrap();
        let mut b = crate::core::GameClient::new().unwrap();
        let mut bar = ControlBar::new();
        let mut viewport = OwnedViewport {
            display: (800, 600),
            height: 600,
        };
        let first_id = a.with_window_event_effects(None, |effects| {
            effects.with_manager(|manager, effects| {
                let parent = manager
                    .create_window(None, 0, 0, 100, 100, effects)
                    .unwrap();
                hide_control_bar_owned(
                    &mut bar,
                    &mut viewport,
                    Some(parent.clone()),
                    true,
                    true,
                    manager,
                    effects,
                );
                assert!(parent.borrow().is_hidden());
                assert!(!residual_control_bar_is_visible(effects));
                assert_eq!(
                    residual_control_bar_last_action(effects),
                    ResidualControlBarAction::Hide
                );
                let id = parent.borrow().get_id();
                id
            })
        });
        let second_id = b.with_window_event_effects(None, |effects| {
            let id = effects.with_manager(|manager, effects| {
                let parent = manager
                    .create_window(None, 0, 0, 100, 100, effects)
                    .unwrap();
                parent.borrow_mut().hide(false, manager).unwrap();
                assert!(!parent.borrow().is_hidden());
                let id = parent.borrow().get_id();
                id
            });
            assert!(residual_control_bar_is_visible(effects));
            assert_eq!(
                residual_control_bar_last_action(effects),
                ResidualControlBarAction::None
            );
            assert!(
                !simulate_control_bar_options_button_gadget_selected(effects),
                "missing button cannot report a click"
            );
            id
        });
        assert_eq!(
            first_id, second_id,
            "identity collision does not select the peer callback state"
        );
        drop(b);
        a.with_window_event_effects(None, |effects| assert!(!is_control_bar_visible(effects)));
    }

    #[test]
    fn owned_stage_and_visibility_write_receiving_integer_height_only() {
        let mut viewport = OwnedViewport {
            display: (801, 601),
            height: 601,
        };
        let peer = OwnedViewport {
            display: (1024, 777),
            height: 555,
        };
        let mut bar = ControlBar::new();
        let parent = std::rc::Rc::new(std::cell::RefCell::new(GameWindow::new()));
        let _ = parent.borrow_mut().set_position(17, 31);
        bar.switch_control_bar_stage_owned(
            ControlBarStage::Default,
            &mut viewport,
            false,
            Some(&mut parent.borrow_mut()),
        );
        assert_eq!(viewport.height, 480);
        bar.switch_control_bar_stage_owned(
            ControlBarStage::Low,
            &mut viewport,
            false,
            Some(&mut parent.borrow_mut()),
        );
        assert_eq!(viewport.height, 601);
        assert_eq!(parent.borrow().get_position(), (17, 540));
        bar.switch_control_bar_stage_owned(
            ControlBarStage::Hidden,
            &mut viewport,
            false,
            Some(&mut parent.borrow_mut()),
        );
        assert_eq!(viewport.height, 601);
        assert!(parent.borrow().is_hidden());
        bar.switch_control_bar_stage_owned(
            ControlBarStage::Default,
            &mut viewport,
            true,
            Some(&mut parent.borrow_mut()),
        );
        assert_eq!(bar.get_control_bar_stage(), ControlBarStage::Hidden);
        bar.switch_control_bar_stage_owned(
            ControlBarStage::Squished,
            &mut viewport,
            false,
            Some(&mut parent.borrow_mut()),
        );
        assert_eq!(bar.get_control_bar_stage(), ControlBarStage::Hidden);
        let mut callbacks = ControlBarCallbacks::new();
        callbacks.show_control_bar_owned(
            &mut bar,
            &mut viewport,
            Some(parent.clone()),
            false,
            true,
        );
        assert_eq!(viewport.height, 480);
        assert!(!parent.borrow().is_hidden());
        callbacks.hide_control_bar_owned(&mut bar, &mut viewport, Some(parent.clone()), true, true);
        assert_eq!(viewport.height, 601);
        assert!(parent.borrow().is_hidden());
        callbacks.show_control_bar_owned(&mut bar, &mut viewport, None, false, true);
        assert_eq!(
            viewport.height, 601,
            "missing parent cannot resize receiving view"
        );
        assert_eq!(peer.height, 555);
    }

    #[test]
    fn owned_window_animation_advances_only_receiving_manager_and_hides_after_reverse() {
        let mut viewport = OwnedViewport {
            display: (800, 600),
            height: 600,
        };
        let mut peer_viewport = OwnedViewport {
            display: (800, 600),
            height: 600,
        };
        let mut bar = ControlBar::new();
        let mut peer_bar = ControlBar::new();
        let parent = std::rc::Rc::new(std::cell::RefCell::new(GameWindow::new()));
        let peer = std::rc::Rc::new(std::cell::RefCell::new(GameWindow::new()));
        let _ = parent.borrow_mut().set_position(20, 40);
        let _ = peer.borrow_mut().set_position(20, 40);
        let mut callbacks = ControlBarCallbacks::new();
        let mut peer_callbacks = ControlBarCallbacks::new();
        callbacks.show_control_bar_owned(
            &mut bar,
            &mut viewport,
            Some(parent.clone()),
            false,
            false,
        );
        peer_callbacks.show_control_bar_owned(
            &mut peer_bar,
            &mut peer_viewport,
            Some(peer.clone()),
            false,
            false,
        );
        let initial = parent.borrow().get_position();
        assert_ne!(initial, (20, 40));
        assert_eq!(peer.borrow().get_position(), initial);
        let now = std::time::Instant::now() + std::time::Duration::from_secs(1);
        callbacks.update_window_animations_at(Some(parent.clone()), now);
        assert_ne!(parent.borrow().get_position(), initial);
        assert_eq!(peer.borrow().get_position(), initial);
        for _ in 0..100 {
            callbacks.update_window_animations_at(Some(parent.clone()), now);
        }
        assert_eq!(parent.borrow().get_position(), (20, 40));
        assert!(!parent.borrow().is_hidden());
        callbacks.hide_control_bar_owned(
            &mut bar,
            &mut viewport,
            Some(parent.clone()),
            true,
            false,
        );
        assert!(
            !parent.borrow().is_hidden(),
            "reverse hides only after completion"
        );
        for _ in 0..100 {
            callbacks.update_window_animations_at(Some(parent.clone()), now);
        }
        assert!(parent.borrow().is_hidden());
        assert_eq!(peer.borrow().get_position(), initial);
        assert!(!peer.borrow().is_hidden());
    }

    #[test]
    fn beacon_edit_text_is_language_filtered() {
        get_language_filter().set_words_for_test(["badword"]);

        let filtered = filter_beacon_edit_text("hold badword beacon".to_string());

        assert_eq!(filtered, "hold ******* beacon");
    }

    #[test]
    fn beacon_window_input_handles_escape_only() {
        with_receiving_observation(test_observation(false, false, true), |effects| {
            let mut window = GameWindow::new();
            assert_eq!(
                beacon_window_input(
                    &mut window,
                    WindowMessage::Char,
                    KEY_ESC as WindowMsgData,
                    0,
                    effects
                ),
                WindowMsgHandled::Handled
            );
            assert_eq!(
                beacon_window_input(
                    &mut window,
                    WindowMessage::Char,
                    b'A' as WindowMsgData,
                    0,
                    effects
                ),
                WindowMsgHandled::Ignored
            );
        });
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
        with_receiving_observation(test_observation(false, false, true), |effects| {
            let mut window = GameWindow::new();
            assert_eq!(
                receiving_control_bar_system(
                    &mut window,
                    WindowMessage::GadgetSelected,
                    NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonIdleWorker") as usize,
                    0,
                    effects
                ),
                WindowMsgHandled::Handled
            );
            assert!(matches!(
                crate::gui::control_bar::take_host_control_bar_requests(effects.ui_mut())
                    .as_slice(),
                [crate::gui::control_bar::HostControlBarRequest::SelectNextIdleWorker]
            ));
        });
    }

    #[test]
    fn left_hud_host_request_captures_the_live_alternate_mouse_setting() {
        with_receiving_observation(test_observation(false, false, true), |effects| {
            let window = left_hud_test_window(effects);
            let click = 42usize | (491usize << 16);
            assert_eq!(
                left_hud_input(
                    &mut window.borrow_mut(),
                    WindowMessage::LeftDown,
                    click,
                    0,
                    effects
                ),
                WindowMsgHandled::Handled
            );
            let published =
                crate::gui::control_bar::take_host_minimap_interactions(effects.ui_mut());
            assert!(
                matches!(published.as_slice(),[interaction] if interaction.button==crate::gui::control_bar::HostMinimapMouseButton::Left && interaction.alternate_mouse)
            );
        });
    }

    #[test]
    fn held_middle_button_bypasses_left_hud_before_host_minimap_publication() {
        with_receiving_observation(test_observation(false, true, false), |effects| {
            let window = left_hud_test_window(effects);
            let click = 42usize | (491usize << 16);
            for message in [WindowMessage::LeftDown, WindowMessage::RightDown] {
                assert_eq!(
                    left_hud_input(&mut window.borrow_mut(), message, click, 0, effects),
                    WindowMsgHandled::Ignored,
                    "C++ LeftHUDInput yields to the held-middle route before {message:?}"
                );
            }
            assert!(
                crate::gui::control_bar::take_host_minimap_interactions(effects.ui_mut())
                    .is_empty(),
                "a held middle button must not publish an authoritative minimap click"
            );
        });
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

fn residual_control_bar_action_store(
    action: ResidualControlBarAction,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    effects.control_bar_callbacks_mut().last_action = action;
}
pub fn residual_control_bar_last_action(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> ResidualControlBarAction {
    effects.control_bar_callbacks().last_action
}
pub fn residual_control_bar_is_visible(
    effects: &crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.control_bar_callbacks().state.visible
}
/// Bind only an existing receiving ControlBar; absence is a miss.
pub fn simulate_control_bar_bind_controls(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let present = effects.with_manager_ref(|manager| {
        manager
            .find_window_by_name("ControlBar.wnd:ControlBarParent")
            .is_some()
    });
    if present {
        effects.control_bar_callbacks_mut().button_communicator =
            NameKeyGenerator::name_to_key("ControlBar.wnd:PopupCommunicator");
    }
    present
}

fn simulate_control_bar_visibility(
    action: ResidualControlBarAction,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !simulate_control_bar_bind_controls(effects) {
        return false;
    }
    let result = effects.with_manager(|manager, effects| match action {
        ResidualControlBarAction::Show => show_control_bar(true, manager, effects),
        ResidualControlBarAction::Hide => hide_control_bar(true, manager, effects),
        ResidualControlBarAction::Toggle => toggle_control_bar(true, manager, effects),
        _ => unreachable!("visibility action"),
    });
    if result.is_ok() {
        residual_control_bar_action_store(action, effects);
    }
    result.is_ok()
}
pub fn simulate_control_bar_show(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_visibility(ResidualControlBarAction::Show, effects)
}
pub fn simulate_control_bar_hide(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_visibility(ResidualControlBarAction::Hide, effects)
}
pub fn simulate_control_bar_toggle(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_visibility(ResidualControlBarAction::Toggle, effects)
}

fn simulate_control_bar_button(
    name: &str,
    action: ResidualControlBarAction,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window(name, effects);
    if clicked {
        residual_control_bar_action_store(action, effects);
    }
    clicked
}
pub fn simulate_control_bar_options_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_button(
        "ControlBar.wnd:ButtonOptions",
        ResidualControlBarAction::Options,
        effects,
    )
}
pub fn simulate_control_bar_idle_worker_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_button(
        "ControlBar.wnd:ButtonIdleWorker",
        ResidualControlBarAction::IdleWorker,
        effects,
    )
}
pub fn simulate_control_bar_general_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_button(
        "ControlBar.wnd:ButtonGeneral",
        ResidualControlBarAction::General,
        effects,
    )
}
pub fn simulate_control_bar_large_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_button(
        "ControlBar.wnd:ButtonLarge",
        ResidualControlBarAction::Large,
        effects,
    )
}
pub fn simulate_control_bar_place_beacon_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_button(
        "ControlBar.wnd:ButtonPlaceBeacon",
        ResidualControlBarAction::PlaceBeacon,
        effects,
    )
}
pub fn simulate_control_bar_delete_beacon_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_button(
        "ControlBar.wnd:ButtonDeleteBeacon",
        ResidualControlBarAction::DeleteBeacon,
        effects,
    )
}
pub fn simulate_control_bar_clear_beacon_text_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_button(
        "ControlBar.wnd:ButtonClearBeaconText",
        ResidualControlBarAction::ClearBeaconText,
        effects,
    )
}
pub fn simulate_control_bar_communicator_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_button(
        "ControlBar.wnd:PopupCommunicator",
        ResidualControlBarAction::Communicator,
        effects,
    )
}
pub fn simulate_control_bar_prepare_options(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    simulate_control_bar_show(effects)
        && simulate_control_bar_options_button_gadget_selected(effects)
}
