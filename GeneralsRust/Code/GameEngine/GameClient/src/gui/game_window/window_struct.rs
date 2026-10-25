//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use std::cell::{Cell, RefCell};
use std::fmt;
use std::rc::{Rc, Weak};

use crate::gui::campaign_manager::CampaignManager;
use crate::gui::display_string::DisplayStringHandle;
use crate::gui::gadgets::{
    CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton,
    StaticText, TabControl, TextEntry, VerticalSlider,
};
use crate::helpers::LiveControlBarClick;
use crate::message_stream::game_message::{GameMessage, GameMessageType};
use crate::video_buffer::VideoBufferHandle;

use super::callbacks::{
    default_input_callback, default_system_callback, legacy_default_draw_callback,
};
use super::font::{
    Color, GameFont, Image, Point2D, WindowDrawData, WindowInstanceData, WindowRegion, WindowState,
    WindowTextColors,
};
use super::messages::{WINDOW_ID_INVALID, WindowMessage, WindowMsgHandled, WindowStatus};
use super::payload::{WindowId, WindowMsgData};

/// Callback function types
pub type DrawCallback = Box<
    dyn Fn(
        &GameWindow,
        &WindowInstanceData,
        &mut crate::gui::window_draw_context::WindowDrawContext<'_>,
    ),
>;
pub type TooltipCallback = Box<dyn Fn(&GameWindow, &WindowInstanceData, u32)>;
pub type InputCallback =
    Box<dyn Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>;
pub type SystemCallback =
    Box<dyn Fn(&GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>;
/// Effects borrowed from the receiving GameClient for one synchronous window event.
/// The effects never outlive callback delivery or enter a queued continuation.
pub struct WindowEventEffects<'a> {
    pub(crate) window_manager_owner: Option<&'a crate::gui::window_manager::WindowManagerOwner>,
    pub(crate) selected_window: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) emitted_messages: &'a mut Vec<GameMessage>,
    pub(crate) pending_control_bar_clicks: Option<&'a mut Vec<LiveControlBarClick>>,
    pub(crate) campaign_manager: Option<&'a mut CampaignManager>,
    pub(crate) menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
    pub(crate) menu_host: Option<&'a mut crate::gui::campaign_launch_host_bridge::MenuHostMailbox>,
    pub(crate) window_videos: Option<&'a mut crate::gui::window_video_manager::WindowVideoManager>,
    pub(crate) script_display:
        Option<&'a mut crate::core::script_action_handler::ScriptDisplayReceiver>,
    pub(crate) main_menu: Option<&'a mut crate::gui::shell::main_menu::MainMenu>,
    pub(crate) pending_main_menu_actions: Vec<(
        crate::gui::shell::main_menu::PendingMainMenuAction,
        Option<crate::gui::callbacks::OptionsCameraInputObservation>,
    )>,
}
impl WindowEventEffects<'_> {
    pub(crate) fn window_videos(
        &mut self,
    ) -> &mut crate::gui::window_video_manager::WindowVideoManager {
        self.window_videos
            .as_deref_mut()
            .expect("window movie callback requires receiving client video owner")
    }
    pub(crate) fn script_display(
        &mut self,
    ) -> &mut crate::core::script_action_handler::ScriptDisplayReceiver {
        self.script_display
            .as_deref_mut()
            .expect("display callback requires its receiving presentation")
    }
    pub(crate) fn menu_world(&self) -> crate::gui::campaign_launch_host_bridge::MenuWorldFacts {
        self.menu_world
            .expect("native menu callback requires receiving world facts")
    }
    pub(crate) fn request_menu_host(
        &mut self,
        request: crate::gui::campaign_launch_host_bridge::MenuHostRequest,
    ) {
        self.menu_host
            .as_deref_mut()
            .expect("menu lifecycle requires receiving host mailbox")
            .request(request);
    }
    pub(crate) fn menu_lifecycle_pending(&self) -> bool {
        self.menu_host
            .as_deref()
            .is_some_and(|host| host.has_lifecycle_request())
    }
    pub(crate) fn shell_map_settings(
        &self,
    ) -> Option<&crate::gui::campaign_launch_host_bridge::ShellMapSettings> {
        self.menu_host
            .as_deref()
            .map(|host| host.shell_map_settings())
    }

    /// Shell.cpp queues ClearGameData before NewGame. It does not reset inline.
    /// Keep those commands in this receiving stream and preserve their source order.
    pub fn request_shell_map(&mut self, on: bool) {
        use crate::gui::campaign_launch_host_bridge::MenuHostRequest;
        // C++ explicitly does nothing while no GameLogic exists. An absent
        // driving-world input never selects an ambient Core world.
        let Some(facts) = self.menu_world else {
            return;
        };
        let host = self
            .menu_host
            .as_deref_mut()
            .expect("Shell map requires receiving client host");
        let settings = host.shell_map_settings();
        if !settings.initial_file.is_empty() {
            return;
        }
        let enabled = on && settings.enabled;
        let map = settings.map.clone();
        if enabled && facts.in_shell_game {
            return;
        }
        if (enabled && facts.in_game) || (!enabled && facts.in_shell_game) {
            self.emitted_messages
                .push(GameMessage::new(GameMessageType::ClearGameData));
        }
        if enabled {
            host.request(MenuHostRequest::SeedLogic(0));
            let mut message = GameMessage::new(GameMessageType::NewGame);
            message.append_integer_argument(gamelogic::system::game_logic::GAME_SHELL);
            message.set_shell_map_launch(
                game_engine::common::message_stream::ClientShellMapLaunch { map, seed: 0 },
            );
            self.emitted_messages.push(message);
        }
        // MainMenuInit runs while Shell is borrowed. Apply its display-only
        // tail at the existing receiving Shell callback boundary.
        let owner = self
            .window_manager_owner
            .expect("Shell map requires receiving WindowManager");
        owner.queue_shell_operation_with_effects(move |shell, effects| {
            shell.apply_shell_map_presentation(enabled, effects);
        });
    }

    pub(crate) fn with_main_menu<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::shell::main_menu::MainMenu,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        let menu = self
            .main_menu
            .take()
            .expect("menu callback requires its receiving GameClient");
        let result = operation(menu, self);
        self.pending_main_menu_actions.extend(
            menu.take_pending_actions()
                .into_iter()
                .map(|action| (action, None)),
        );
        self.main_menu = Some(menu);
        let actions = std::mem::take(&mut self.pending_main_menu_actions);
        for (action, camera_input) in actions {
            crate::gui::shell::main_menu::MainMenu::execute_action(action, camera_input, self);
        }
        result
    }

    pub(crate) fn queue_window_operation(
        &self,
        operation: impl for<'a> FnOnce(
            &mut crate::gui::window_manager::WindowManager,
            &mut WindowEventEffects<'a>,
        ) + 'static,
    ) {
        self.window_manager_owner
            .expect("WND continuation requires receiving owner")
            .queue_window_operation_with_effects(operation);
    }

    pub(crate) fn queue_system_message(
        &self,
        target: Weak<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) {
        self.queue_window_operation(move |manager, effects| {
            let Some(target) = target.upgrade() else {
                return;
            };
            let id = target.borrow().get_id();
            let Some(registered) = manager.get_window_by_id(id) else {
                return;
            };
            if !Rc::ptr_eq(&target, &registered)
                || target
                    .borrow()
                    .get_status()
                    .contains(WindowStatus::DESTROYED)
            {
                return;
            }
            let previous = effects.selected_window.replace(Rc::downgrade(&target));
            target
                .borrow_mut()
                .send_system_message_with_message_sink(msg, data1, data2, effects, None);
            effects.selected_window = previous;
        });
    }
}

pub type InputCallbackWithMessageSink = Box<
    dyn for<'a> Fn(
        &GameWindow,
        WindowMessage,
        WindowMsgData,
        WindowMsgData,
        &mut WindowEventEffects<'a>,
    ) -> WindowMsgHandled,
>;
pub type SystemCallbackWithMessageSink = Box<
    dyn for<'a> Fn(
        &GameWindow,
        WindowMessage,
        WindowMsgData,
        WindowMsgData,
        &mut WindowEventEffects<'a>,
    ) -> WindowMsgHandled,
>;

/// Data attached to each window specifically for the GUI editor.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameWindowEditData {
    pub system_callback_string: String,
    pub input_callback_string: String,
    pub tooltip_callback_string: String,
    pub draw_callback_string: String,
}

/// Window callback functions
#[derive(Default)]
pub struct WindowCallbacks {
    pub draw: Option<DrawCallback>,
    pub tooltip: Option<TooltipCallback>,
    pub input: Option<InputCallback>,
    pub input_with_message_sink: Option<InputCallbackWithMessageSink>,
    pub system: Option<SystemCallback>,
    pub system_with_message_sink: Option<SystemCallbackWithMessageSink>,
    pub(crate) control_bar_system: bool,
    pub(crate) control_bar_observer_system: bool,
    pub(crate) options_menu_system: bool,
    pub(crate) main_menu_system: bool,
    pub(crate) quit_menu_system: bool,
    pub(crate) replay_menu_system: bool,
    pub(crate) owned_radar_draw: bool,
    pub(crate) owned_control_bar_tooltip: bool,
}

/// Main GameWindow struct representing a UI window or control
pub struct GameWindow {
    // Core properties
    pub(crate) id: WindowId,
    pub(crate) status: WindowStatus,
    pub(crate) size: Point2D,
    pub(crate) region: WindowRegion,
    pub(crate) cursor_pos: Cell<Point2D>,

    // Instance data
    pub(crate) inst_data: WindowInstanceData,

    // User data
    pub(crate) user_data: Option<Box<dyn std::any::Any>>,
    pub(crate) edit_data: Option<GameWindowEditData>,

    // Hierarchy
    pub(crate) parent: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) children: Vec<Rc<RefCell<GameWindow>>>,
    pub(crate) next_sibling: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) prev_sibling: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) owner_is_self: bool,

    // Layout information
    pub(crate) next_in_layout: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) prev_in_layout: Option<Weak<RefCell<GameWindow>>>,
    pub(crate) layout: Option<Weak<RefCell<crate::gui::WindowLayout>>>,

    // Callbacks
    pub(crate) callbacks: WindowCallbacks,

    // Optional gadget backing this window
    pub(crate) widget: Option<WindowWidget>,

    // Combo box child window references (drop-down, edit, list)
    pub(crate) combobox_links: Option<ComboBoxLinks>,

    // List box scrollbar child window references
    pub(crate) listbox_links: Option<ListBoxLinks>,

    // Slider thumb child window reference
    pub(crate) slider_thumb: Option<WindowId>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ComboBoxLinks {
    pub drop_down: WindowId,
    pub edit_box: WindowId,
    pub list_box: WindowId,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ListBoxLinks {
    pub up_button: WindowId,
    pub down_button: WindowId,
    pub slider: WindowId,
    pub thumb: Option<WindowId>,
}

/// Gadget backing types for windows created from scripts.
pub enum WindowWidget {
    PushButton(PushButton),
    RadioButton(RadioButton),
    CheckBox(CheckBox),
    VerticalSlider(VerticalSlider),
    HorizontalSlider(HorizontalSlider),
    ListBox(ListBox),
    TextEntry(TextEntry),
    StaticText(StaticText),
    ProgressBar(ProgressBar),
    TabControl(TabControl),
    ComboBox(ComboBox),
    TabPane,
    User,
    Animated,
    MouseTrack,
}

impl fmt::Debug for WindowWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::PushButton(_) => "PushButton",
            Self::RadioButton(_) => "RadioButton",
            Self::CheckBox(_) => "CheckBox",
            Self::VerticalSlider(_) => "VerticalSlider",
            Self::HorizontalSlider(_) => "HorizontalSlider",
            Self::ListBox(_) => "ListBox",
            Self::TextEntry(_) => "TextEntry",
            Self::StaticText(_) => "StaticText",
            Self::ProgressBar(_) => "ProgressBar",
            Self::TabControl(_) => "TabControl",
            Self::ComboBox(_) => "ComboBox",
            Self::TabPane => "TabPane",
            Self::User => "User",
            Self::Animated => "Animated",
            Self::MouseTrack => "MouseTrack",
        };
        f.write_str(name)
    }
}

impl fmt::Debug for GameWindow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GameWindow")
            .field("id", &self.id)
            .field("status", &self.status)
            .field("child_count", &self.children.len())
            .finish()
    }
}

impl Default for GameWindow {
    fn default() -> Self {
        Self::new()
    }
}

/// Bounded notification from one selected gadget input. The caller releases
/// the sender borrow before delivering it synchronously to its actual owner.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectedOwnerAction {
    ControlBarToggle,
    ReplayMenuLoad,
    ReplayMenuDoubleClick,
    ReplayMenuConfirm,
    ReplayMenuRestart,
    ObserverButtonSlot(Option<usize>),
    OptionsAccept,
    OptionsDefaults,
    OptionsReplayPreference { use_camera: bool, checked: bool },
    ShowOptions,
    ShowQuitOptions,
}
pub(crate) struct ControlBarNotification {
    pub(crate) owner: Rc<RefCell<GameWindow>>,
    pub(crate) action: SelectedOwnerAction,
    pub(crate) control_id: WindowMsgData,
}
