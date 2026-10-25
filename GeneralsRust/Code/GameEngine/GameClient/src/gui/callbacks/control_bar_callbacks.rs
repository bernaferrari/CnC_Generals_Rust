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
use crate::helpers::{TheControlBar, TheInGameUI};

use crate::input::{MouseButton, with_mouse};
use crate::language_filter::get_language_filter;
use crate::message_stream::{GameMessage, GameMessageType};
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::radar::{ICoord2D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH};
use gamelogic::commands::command::CommandType;
use gamelogic::commands::selection::get_selection_manager;
use gamelogic::helpers::{TheGameLogic, TheScriptEngine};
use gamelogic::player::ThePlayerList;
use std::sync::{Arc, RwLock};

const CMD_NEED_TARGET_POS: u32 = 0x0000_0020;
const CMD_ATTACK_OBJECTS_POSITION: u32 = 0x0000_1000;
const KEY_ESC: usize = 0x1B;

/// Borrowed writer into the camera belonging to the receiving client.
/// Display dimensions and the resulting integer height share that owner.
pub trait ControlBarViewport {
    fn display_size(&self) -> (u32, u32);
    fn set_height_px(&mut self, height: i32);
}

/// Standalone layout dimensions are captured from the receiving manager.
/// Native clients supply their SessionCamera through ControlBarViewport.
pub struct StandaloneControlBarViewport {
    dimensions: (u32, u32),
}
impl StandaloneControlBarViewport {
    pub fn new(manager: &crate::gui::window_manager::WindowManager) -> Self {
        let (width, height) = manager.screen_size();
        Self {
            dimensions: (width.max(0) as u32, height.max(0) as u32),
        }
    }
}
impl ControlBarViewport for StandaloneControlBarViewport {
    fn display_size(&self) -> (u32, u32) {
        self.dimensions
    }
    fn set_height_px(&mut self, height: i32) {
        with_tactical_view(|view| view.set_height(height));
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

fn radar_allows_input() -> bool {
    TheInGameUI::radar_presentation().is_some_and(|radar| radar.is_radar_shown())
}

/// After C++ `LeftHUDInput` validates radar availability, a held middle button
/// falls through to ordinary camera handling.
fn middle_mouse_is_held() -> bool {
    with_mouse(|mouse| mouse.get_mouse_status().is_button_down(MouseButton::Middle))
}

fn local_pixel_to_radar(local_x: i32, local_y: i32, width: i32, height: i32) -> Option<ICoord2D> {
    use crate::gui::ingame_ui::local_pixel_to_radar_cell;
    let extent = TheInGameUI::radar_presentation()?.map_extent();
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
        if parent.borrow().is_hidden() {
            bar.show_special_power_shortcut_receiving(
                upgrade_source,
                game_ending,
                receiving_manager,
                effects,
            );
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
                self.handle_button_selected(
                    control_id,
                    msg == WindowMessage::GadgetRightClick,
                    effects,
                    Some(&mut *_window),
                );
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetEditDone => {
                effects.with_manager(|receiving_manager, effects| {
                    self.handle_edit_done(data1 as u32, effects.emitted_messages, receiving_manager)
                });
                WindowMsgHandled::Handled
            }
            WindowMessage::None => {
                self.animate_manager.update();
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn handle_button_selected(
        &mut self,
        control_id: u32,
        right_click: bool,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
        mut selected_window: Option<&mut crate::gui::GameWindow>,
    ) {
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
            let _ = effects.with_manager(|manager, _effects| {
                toggle_diplomacy(false, manager, _effects, selected_window.as_deref_mut())
            });
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
                effects
                    .emitted_messages
                    .push(GameMessage::new(GameMessageType::RemoveBeacon(
                        crate::message_stream::game_message::Coord3D::default(),
                    )));
            }
            return;
        }

        if control_id == beacon_clear_text_id {
            if TheGameLogic::is_in_multiplayer_game() {
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
            TheControlBar::toggle_purchase_science();
            return;
        }

        if control_id == button_large_id {
            TheControlBar::toggle_control_bar_stage();
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
            if publish_host_select_next_idle_worker() {
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
            clicks.push(crate::helpers::LiveControlBarClick::from_current_dispatch(
                control_id, msg,
            ));
        }
    }

    fn handle_edit_done(
        &mut self,
        control_id: u32,
        emitted_messages: &mut Vec<GameMessage>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        let text_id = NameKeyGenerator::name_to_key("ControlBar.wnd:EditBeaconText");
        if control_id != text_id {
            return;
        }

        if should_submit_beacon_text(TheGameLogic::is_in_multiplayer_game(), selection_count()) {
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

    /// Toggle control bar visibility
    pub fn toggle_control_bar(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ ToggleControlBar: toggleReplayControls() then flip the bar.
        toggle_replay_controls(receiving_manager);
        if self.state.visible {
            self.hide_control_bar_window(immediate, receiving_manager)?;
        } else {
            self.show_control_bar_window(immediate, receiving_manager)?;
        }
        Ok(())
    }

    /// Hide the control bar
    pub fn hide_control_bar(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ HideControlBar: hideReplayControls() even if already hidden.
        hide_replay_controls(receiving_manager);
        self.hide_control_bar_window(immediate, receiving_manager)
    }

    pub fn show_control_bar(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ ShowControlBar: showReplayControls() then switch DEFAULT + winHide(FALSE).
        show_replay_controls(receiving_manager);
        self.show_control_bar_window(immediate, receiving_manager)
    }

    fn hide_control_bar_window(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.state.visible {
            return Ok(());
        }
        self.state.visible = false;
        TheControlBar::hide_special_power_shortcut();
        self.apply_visibility_change(immediate, false, receiving_manager)?;
        Ok(())
    }

    fn show_control_bar_window(
        &mut self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // C++ ShowControlBar always switchControlBarStage(DEFAULT) + winHide(FALSE)
        // even when already shown. Do not early-return on state.visible.
        if !self.state.visible {
            self.state.visible = true;
            TheControlBar::show_special_power_shortcut();
        }
        self.apply_visibility_change(immediate, true, receiving_manager)?;
        Ok(())
    }

    /// C++ GameLogic.cpp:2233 setControlBarSchemeByPlayer + ControlBarCallback.cpp:489 DEFAULT.
    fn apply_show_control_bar_scheme_and_default_stage(
        &self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        let parent_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent") as i32;
        let parent_live = (|manager: &mut crate::gui::window_manager::WindowManager| {
            manager.get_window_by_id(parent_id).is_some()
        })(receiving_manager);
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
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
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
                self.apply_show_control_bar_scheme_and_default_stage(receiving_manager);
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
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
        emitted_messages: &mut Vec<GameMessage>,
        _pending_clicks: Option<&mut Vec<crate::helpers::LiveControlBarClick>>,
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
                self.handle_mouse_down(window, msg, data1, emitted_messages);
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

    fn handle_mouse_down(
        &self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        emitted_messages: &mut Vec<GameMessage>,
    ) {
        let (mouse_x, mouse_y) = decode_mouse_pos(data1);
        let (screen_x, screen_y) = window.get_screen_position();
        let (width, height) = window.get_size();
        let local_x = mouse_x - screen_x;
        let local_y = mouse_y - screen_y;
        let Some(radar_pos) = local_pixel_to_radar(local_x, local_y, width, height) else {
            return;
        };
        let Some(radar) = TheInGameUI::radar_presentation() else {
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

            if let Some(pending_power) = TheInGameUI::get_pending_special_power() {
                if (pending_power.options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION))
                    != 0
                {
                    emitted_messages.push(GameMessage::new(
                        GameMessageType::DoSpecialPowerAtLocation(
                            pending_power.power_id,
                            world_pos,
                            0.0,
                            0,
                            pending_power.options,
                            pending_power.source_object_id,
                        ),
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
                        emitted_messages.push(GameMessage::new(message));
                        TheInGameUI::clear_pending_command();
                        TheInGameUI::clear_attack_move_to_mode();
                        return;
                    }
                }
            }

            if TheInGameUI::is_in_attack_move_to_mode() {
                emitted_messages.push(GameMessage::new(GameMessageType::DoAttackMoveTo(
                    world_pos.clone(),
                )));
            } else {
                emitted_messages.push(GameMessage::new(GameMessageType::DoMoveTo(world_pos)));
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
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        crate::gui::control_bar::control_bar_observer::control_bar_observer_system(
            window, msg, data1, data2, effects,
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
    pub fn toggle_control_bar(
        &self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.toggle_control_bar(immediate, receiving_manager)
    }

    /// Hide control bar through the system
    pub fn hide_control_bar(
        &self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.hide_control_bar(immediate, receiving_manager)
    }

    /// Show control bar through the system  
    pub fn show_control_bar(
        &self,
        immediate: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut callbacks = self.callbacks.write().unwrap_or_else(|e| e.into_inner());
        callbacks.show_control_bar(immediate, receiving_manager)
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
pub fn toggle_control_bar(
    immediate: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_control_bar_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.toggle_control_bar(immediate, receiving_manager)
}

pub fn hide_control_bar(
    immediate: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_control_bar_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.hide_control_bar(immediate, receiving_manager)
}

pub fn show_control_bar(
    immediate: bool,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
) -> Result<(), Box<dyn std::error::Error>> {
    let system = get_control_bar_system();
    let system = system.read().unwrap_or_else(|e| e.into_inner());
    system.show_control_bar(immediate, receiving_manager)
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

    struct OwnedViewport {
        display: (u32, u32),
        height: i32,
    }
    impl ControlBarViewport for OwnedViewport {
        fn display_size(&self) -> (u32, u32) {
            self.display
        }
        fn set_height_px(&mut self, height: i32) {
            self.height = height;
        }
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
            &mut Vec::new(),
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
                callbacks.input(
                    &window,
                    WindowMessage::LeftDown,
                    click,
                    0,
                    &mut Vec::new(),
                    None
                ),
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
                    callbacks.input(&window, message, click, 0, &mut Vec::new(), None),
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
