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
    Rc<dyn Fn(&mut GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>;
pub type SystemCallback =
    Rc<dyn Fn(&mut GameWindow, WindowMessage, WindowMsgData, WindowMsgData) -> WindowMsgHandled>;
/// Effects borrowed from the receiving GameClient for one synchronous window event.
/// The effects never outlive callback delivery or enter a queued continuation.
pub struct WindowEventEffects<'a> {
    pub(crate) standalone_view: Option<&'a mut crate::display::view::View>,
    pub(crate) client_frame: Option<u32>,
    pub(crate) logic_frame: Option<u32>,
    pub(crate) ui: Option<&'a mut crate::core::subsystems::InGameUISubsystem>,
    /// Stack-owned typed parameters, lent across the same synchronous callbacks.
    pub(crate) payloads: super::payload::WindowMsgPayloadArena,
    pub(crate) ime: Option<&'a mut crate::gui::ime_manager::ImeManager>,
    pub(crate) manager: Option<&'a mut crate::gui::window_manager::WindowManager>,
    pub(crate) menus: Option<crate::gui::callbacks::menu_callbacks::MenuCallbackRecipients<'a>>,
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
impl<'effects> WindowEventEffects<'effects> {
    /// Begin a synchronous operation with its own typed message storage.
    /// Capabilities are supplied by the receiving client when needed.
    pub fn new(emitted_messages: &'effects mut Vec<GameMessage>) -> Self {
        Self {
            payloads: Default::default(),
            ui: None,
            logic_frame: None,
            client_frame: None,
            standalone_view: None,
            ime: None,
            manager: None,
            menus: None,
            window_manager_owner: None,
            selected_window: None,
            emitted_messages,
            pending_control_bar_clicks: None,
            campaign_manager: None,
            menu_world: None,
            menu_host: None,
            window_videos: None,
            script_display: None,
            main_menu: None,
            pending_main_menu_actions: Vec::new(),
        }
    }

    pub fn input_authority(&self) -> crate::core::game_client::ClientInputAuthority {
        self.menus
            .as_ref()
            .expect("operation requires receiving menu owner")
            .input_authority
    }
    /// A state phase cannot retain Effects or execute a WND callback.
    pub fn with_ingame_ui<R>(
        &mut self,
        operation: impl FnOnce(&mut crate::gui::callbacks::ingame_callbacks::InGameUISystem) -> R,
    ) -> R {
        operation(
            self.menus
                .as_mut()
                .expect("operation requires receiving menu owner")
                .ingame_ui,
        )
    }
    pub(crate) fn queue_compatibility_replay(
        &mut self,
        step: crate::gui::callbacks::replay_menu::ingress::ReplayMenuStep,
    ) {
        self.window_manager_owner
            .expect("replay command requires receiving owner")
            .queue_replay_step(step);
    }

    pub(crate) fn popup_replay(&self) -> &crate::gui::callbacks::popup_replay::PopupReplayState {
        self.menus
            .as_ref()
            .expect("replay popup requires receiving client")
            .popup_replay
            .as_deref()
            .expect("popup belongs to this operation")
    }
    pub(crate) fn popup_replay_mut(
        &mut self,
    ) -> &mut crate::gui::callbacks::popup_replay::PopupReplayState {
        self.menus
            .as_mut()
            .expect("replay popup requires receiving client")
            .popup_replay
            .as_deref_mut()
            .expect("popup belongs to this operation")
    }

    pub(crate) fn menu_flags(&self) -> &crate::gui::menu_flags::MenuFlags {
        self.menus
            .as_ref()
            .expect("menu flags require receiving client")
            .flags
            .as_deref()
            .expect("flags belong to this operation")
    }
    pub(crate) fn menu_flags_mut(&mut self) -> &mut crate::gui::menu_flags::MenuFlags {
        self.menus
            .as_mut()
            .expect("menu flags require receiving client")
            .flags
            .as_deref_mut()
            .expect("flags belong to this operation")
    }

    /// Borrow only this client's score state. Callers release the loan before
    /// invoking window, replay, movie or Shell callbacks.
    pub(crate) fn score_screen(&self) -> &crate::gui::callbacks::score_screen::ScoreScreenState {
        self.menus
            .as_ref()
            .expect("score screen requires receiving client")
            .score
            .as_deref()
            .expect("score state belongs to this operation")
    }
    pub(crate) fn score_screen_mut(
        &mut self,
    ) -> &mut crate::gui::callbacks::score_screen::ScoreScreenState {
        self.menus
            .as_mut()
            .expect("score screen requires receiving client")
            .score
            .as_deref_mut()
            .expect("score state belongs to this operation")
    }

    /// A bounded phase on this client's canonical QuitMenu state. No callbacks
    /// may execute while the state is borrowed.
    pub fn with_quit_menu<R>(
        &mut self,
        operation: impl FnOnce(&mut crate::gui::callbacks::quit_menu::QuitMenuState) -> R,
    ) -> R {
        let menu = self
            .menus
            .as_mut()
            .expect("QuitMenu requires receiving client")
            .quit
            .as_deref_mut()
            .expect("QuitMenu phase remains available");
        operation(menu)
    }

    pub(crate) fn with_disconnect_menu<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::menus::disconnect_menu::DisconnectMenu,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        let menu = self
            .menus
            .as_mut()
            .expect("connection menu requires receiving client")
            .disconnect
            .take()
            .expect("same-menu operation must finish its current borrowed phase");
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(menu, self)));
        self.menus
            .as_mut()
            .expect("menu recipients remain installed")
            .disconnect = Some(menu);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    pub(crate) fn with_establish_connections_menu<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::menus::establish_connections_menu::EstablishConnectionsMenu,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        let menu = self
            .menus
            .as_mut()
            .expect("connection menu requires receiving client")
            .establish_connections
            .take()
            .expect("same-menu operation must finish its current borrowed phase");
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(menu, self)));
        self.menus
            .as_mut()
            .expect("menu recipients remain installed")
            .establish_connections = Some(menu);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
    pub(crate) fn receiving_disconnect_menu_visible(&self) -> bool {
        self.menus
            .as_ref()
            .expect("disconnect observation requires receiving client")
            .disconnect
            .as_deref()
            .expect("disconnect observation must occur outside its mutable operation")
            .is_visible()
    }

    pub(crate) fn with_basic_main_menu<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::callbacks::menu_callbacks::MainMenu,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        let menu = self
            .menus
            .as_mut()
            .expect("menu callback requires its receiving client")
            .basic_main
            .take()
            .expect("same-menu reentry must resume after the current borrowed phase");
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(menu, self)));
        self.menus
            .as_mut()
            .expect("menu recipient remains installed")
            .basic_main = Some(menu);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(crate) fn with_single_player_menu<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::callbacks::menu_callbacks::SinglePlayerMenu,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        let menu = self
            .menus
            .as_mut()
            .expect("menu callback requires its receiving client")
            .single_player
            .take()
            .expect("same-menu reentry must resume after the current borrowed phase");
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(menu, self)));
        self.menus
            .as_mut()
            .expect("menu recipient remains installed")
            .single_player = Some(menu);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(crate) fn with_options_menu<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::callbacks::menu_callbacks::OptionsMenu,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        let menu = self
            .menus
            .as_mut()
            .expect("menu callback requires its receiving client")
            .options
            .take()
            .expect("same-menu reentry must resume after the current borrowed phase");
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(menu, self)));
        self.menus
            .as_mut()
            .expect("menu recipient remains installed")
            .options = Some(menu);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(crate) fn with_map_select_menu<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::callbacks::menu_callbacks::MapSelectMenu,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        let menu = self
            .menus
            .as_mut()
            .expect("menu callback requires its receiving client")
            .map_select
            .take()
            .expect("same-menu reentry must resume after the current borrowed phase");
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(menu, self)));
        self.menus
            .as_mut()
            .expect("menu recipient remains installed")
            .map_select = Some(menu);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(crate) fn with_credits_menu<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::callbacks::menu_callbacks::CreditsMenu,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        let menu = self
            .menus
            .as_mut()
            .expect("menu callback requires its receiving client")
            .credits
            .take()
            .expect("same-menu reentry must resume after the current borrowed phase");
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(menu, self)));
        self.menus
            .as_mut()
            .expect("menu recipient remains installed")
            .credits = Some(menu);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(crate) fn with_lan_lobby_menu<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::callbacks::menu_callbacks::LanLobbyMenu,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        let menu = self
            .menus
            .as_mut()
            .expect("menu callback requires its receiving client")
            .lan_lobby
            .take()
            .expect("same-menu reentry must resume after the current borrowed phase");
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(menu, self)));
        self.menus
            .as_mut()
            .expect("menu recipient remains installed")
            .lan_lobby = Some(menu);
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    /// Borrow the campaign belonging to this event recipient.
    pub fn receiving_campaign_manager_mut(&mut self) -> Option<&mut CampaignManager> {
        self.campaign_manager.as_deref_mut()
    }

    /// Complete this operation before admitting its emitted messages. The
    /// receiver remains the same client across every synchronous callback.
    pub fn complete_callbacks_into(&mut self, stream: &mut crate::message_stream::MessageStream) {
        if let Some(owner) = self.window_manager_owner {
            owner.complete_event_callbacks(self);
        }
        for message in self.emitted_messages.drain(..) {
            stream.append_message_value(message);
        }
    }

    pub fn logic_frame(&self) -> u32 {
        self.logic_frame
            .expect("UI operation requires its receiving logic frame")
    }
    pub fn receiving_ui(&mut self) -> crate::message_stream::ReceivingUi<'_> {
        let frame = self.logic_frame();
        let client_frame = self
            .client_frame
            .expect("UI operation requires receiving client frame");
        crate::message_stream::ReceivingUi::new(
            self.ui.as_deref_mut().expect("receiving UI"),
            frame,
            client_frame,
        )
        .with_emitter(self.emitted_messages)
    }
    pub fn message(&mut self, text: &str) {
        let frame = self.logic_frame();
        self.ui_mut().message(text, frame);
    }
    pub fn set_hint_text(&mut self, text: &str) {
        let frame = self.logic_frame();
        self.ui_mut().set_hint_text(text, frame);
    }

    pub fn has_ui(&self) -> bool {
        self.ui.is_some()
    }
    pub fn ui(&self) -> &crate::core::subsystems::InGameUISubsystem {
        self.ui
            .as_deref()
            .expect("UI observation requires receiving client")
    }
    pub fn ui_mut(&mut self) -> &mut crate::core::subsystems::InGameUISubsystem {
        self.ui
            .as_deref_mut()
            .expect("UI operation requires receiving client")
    }

    pub(crate) fn ime(&self) -> &crate::gui::ime_manager::ImeManager {
        self.ime
            .as_deref()
            .expect("IME operation requires receiving client owner")
    }
    pub(crate) fn ime_mut(&mut self) -> &mut crate::gui::ime_manager::ImeManager {
        self.ime
            .as_deref_mut()
            .expect("IME operation requires receiving client owner")
    }

    pub(crate) fn shell_handle(&self) -> crate::gui::shell::ShellHandle {
        self.window_manager_owner
            .expect("Shell callback requires receiving owner")
            .shell_handle()
    }

    /// A leaf callback borrows the actual driving manager for exactly its call.
    /// The outer dispatcher cannot inspect it until the callback returns.
    pub(crate) fn with_receiving_manager<R>(
        &mut self,
        manager: &mut crate::gui::window_manager::WindowManager,
        operation: impl FnOnce(&mut WindowEventEffects<'_>) -> R,
    ) -> R {
        let mut local = WindowEventEffects {
            payloads: std::mem::take(&mut self.payloads),
            ui: self.ui.as_deref_mut(),
            logic_frame: self.logic_frame,
            client_frame: self.client_frame,
            standalone_view: self.standalone_view.as_deref_mut(),
            manager: Some(manager),
            menus: self.menus.as_mut().map(|menus| menus.reborrow()),
            ime: self.ime.as_deref_mut(),
            window_manager_owner: self.window_manager_owner,
            selected_window: self.selected_window.clone(),
            emitted_messages: &mut *self.emitted_messages,
            pending_control_bar_clicks: self.pending_control_bar_clicks.as_deref_mut(),
            campaign_manager: self.campaign_manager.as_deref_mut(),
            menu_world: self.menu_world,
            menu_host: self.menu_host.as_deref_mut(),
            window_videos: self.window_videos.as_deref_mut(),
            script_display: self.script_display.as_deref_mut(),
            main_menu: self.main_menu.as_deref_mut(),
            pending_main_menu_actions: Vec::new(),
        };
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(&mut local)));
        self.payloads = std::mem::take(&mut local.payloads);
        self.pending_main_menu_actions
            .append(&mut local.pending_main_menu_actions);
        match result {
            Ok(value) => value,
            Err(panic) => std::panic::resume_unwind(panic),
        }
    }
    /// Borrow the supplied manager or this operation's explicit owner. Never
    /// discover an active tree or manufacture a replacement on reentry.
    pub fn with_manager<R>(
        &mut self,
        operation: impl FnOnce(
            &mut crate::gui::window_manager::WindowManager,
            &mut WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        if let Some(manager) = self.manager.take() {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(manager, self)));
            self.manager = Some(manager);
            match result {
                Ok(value) => value,
                Err(panic) => std::panic::resume_unwind(panic),
            }
        } else {
            let owner = self
                .window_manager_owner
                .expect("WND operation requires receiving manager");
            owner.with_manager(|manager| {
                self.with_receiving_manager(manager, |effects| effects.with_manager(operation))
            })
        }
    }
    pub fn with_manager_ref<R>(
        &mut self,
        operation: impl FnOnce(&crate::gui::window_manager::WindowManager) -> R,
    ) -> R {
        if let Some(manager) = self.manager.as_deref() {
            operation(manager)
        } else {
            self.window_manager_owner
                .expect("WND observation requires receiving manager")
                .with_manager_ref(operation)
        }
    }

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
            crate::gui::shell::main_menu::MainMenu::execute_action(
                action,
                camera_input,
                self,
                None,
            );
        }
        result
    }

    pub(crate) fn with_main_menu_for_window<R>(
        &mut self,
        window: &mut GameWindow,
        operation: impl FnOnce(
            &mut crate::gui::shell::main_menu::MainMenu,
            &mut WindowEventEffects<'_>,
            &mut GameWindow,
        ) -> R,
    ) -> R {
        let menu = self
            .main_menu
            .take()
            .expect("menu callback requires receiving client");
        let result = operation(menu, self, window);
        self.pending_main_menu_actions.extend(
            menu.take_pending_actions()
                .into_iter()
                .map(|action| (action, None)),
        );
        self.main_menu = Some(menu);
        let actions = std::mem::take(&mut self.pending_main_menu_actions);
        for (action, camera) in actions {
            crate::gui::shell::main_menu::MainMenu::execute_action(
                action,
                camera,
                self,
                Some(&mut *window),
            );
        }
        result
    }
    pub(crate) fn complete_pending_main_menu_actions(&mut self) {
        let actions = std::mem::take(&mut self.pending_main_menu_actions);
        for (action, camera) in actions {
            crate::gui::shell::main_menu::MainMenu::execute_action(action, camera, self, None);
        }
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
            effects.with_receiving_manager(manager, |effects| {
                target
                    .borrow_mut()
                    .send_system_message_with_message_sink(msg, data1, data2, effects, None)
            });
            effects.selected_window = previous;
        });
    }
}

pub type InputCallbackWithMessageSink = Rc<
    dyn for<'a> Fn(
        &mut GameWindow,
        WindowMessage,
        WindowMsgData,
        WindowMsgData,
        &mut WindowEventEffects<'a>,
    ) -> WindowMsgHandled,
>;
pub type SystemCallbackWithMessageSink = Rc<
    dyn for<'a> Fn(
        &mut GameWindow,
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

impl WindowEventEffects<'_> {
    /// Ordinary callback-state loans. Callers end this phase before nested
    /// WND lifecycle effects; the canonical state is never removed/replaced.
    pub fn control_bar_callbacks(
        &self,
    ) -> &crate::gui::callbacks::control_bar_callbacks::ControlBarCallbacks {
        self.menus
            .as_ref()
            .expect("receiving menus")
            .control_bar_callbacks
            .as_deref()
            .expect("receiving control bar callbacks")
    }
    pub fn control_bar_callbacks_mut(
        &mut self,
    ) -> &mut crate::gui::callbacks::control_bar_callbacks::ControlBarCallbacks {
        self.menus
            .as_mut()
            .expect("receiving menus")
            .control_bar_callbacks
            .as_deref_mut()
            .expect("receiving control bar callbacks")
    }
}
