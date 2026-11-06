//! Menu Callback Functions
//!
//! This module contains callback functions for all the shell menus,
//! including main menu, single player menu, options menu, etc.

mod options_menu;
mod map_select_menu;
mod options_residual;
#[cfg(test)]
mod options_os_wnd_tests;
#[cfg(test)]
mod tests;
pub use map_select_menu::*;
pub use options_menu::*;
pub use options_residual::*;

use crate::core::game_client::apply_lod_texture_reduction;
use crate::game_text::GameText;
use crate::gui::callbacks::quit_menu::destroy_quit_menu;
use crate::gui::gadgets::ComboBoxItem;
use crate::gui::gadgets::ListBoxItemData;
use crate::gui::game_window::WindowEventEffects;
use crate::gui::header_template::get_header_template_manager;
use crate::gui::options_host_bridge::HostOptionsRequest;
use crate::gui::shell::main_menu::DisplaySettings;
use crate::gui::{
    AnimationType, GLM_DOUBLE_CLICKED, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowWidget, queue_shell_pop, queue_shell_push,
    queue_shell_reverse_animate_window, queue_shell_shutdown_complete,
    queue_shell_window_animation, show_shell_map_if_available, write_input_focus_response,
};

use crate::map_util::{get_map_cache_manager, populate_map_listbox};
use crate::message_stream::{GameMessage, GameMessageType};
use game_engine::common::audio::AudioAffect as EngineAudioAffect;
use game_engine::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager,
};
use game_engine::common::global_data as runtime_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::init_random_with_seed;
use game_engine::common::user_preferences::UserPreferences;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::{TheAudio, TheGameLogic, TheScriptEngine};
use gamelogic::system::game_logic::{GAME_SHELL, GAME_SINGLE_PLAYER};
use log::{debug, error, info, warn};
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(feature = "online_ui")]
use crate::gamespy_overlay::{GameSpyOverlayType, close_overlay, is_overlay_open};

#[cfg(feature = "online_ui")]
fn options_overlay_is_open() -> bool {
    is_overlay_open(GameSpyOverlayType::Options)
}

#[cfg(not(feature = "online_ui"))]
fn options_overlay_is_open() -> bool {
    false
}

#[cfg(feature = "online_ui")]
fn close_options_overlay() {
    close_overlay(GameSpyOverlayType::Options);
}

#[cfg(not(feature = "online_ui"))]
fn close_options_overlay() {}

fn publish_host_option(effects: &mut WindowEventEffects<'_>, request: HostOptionsRequest) {
    if let Some(host) = effects.menu_host.as_deref_mut() {
        host.options.publish(request);
    }
}

/// Menu callback trait
pub trait MenuCallbacks {
    /// Initialize the menu
    fn init(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        _effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.init(layout, user_data, _effects)
    }

    /// Update the menu (called every frame)
    fn update(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Update using the receiving GameClient's borrowed event effects.
    fn update_with_effects(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        _effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.update(layout, user_data, _effects)
    }

    /// Shutdown the menu
    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn shutdown_with_effects(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        _effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.shutdown(layout, user_data, _effects)
    }

    /// Handle system messages
    fn system(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled;

    /// Dispatch using the receiving GameClient's borrowed event effects.
    fn system_with_effects(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        _effects: &mut WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        self.system(window, msg, data1, data2, _effects)
    }

    /// Handle input messages
    fn input(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled;
    fn input_with_effects(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        _effects: &mut WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        self.input(window, msg, data1, data2, _effects)
    }
}

/// Main Menu implementation
pub struct MainMenu {
    initialized: bool,
}

impl MainMenu {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for MainMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for MainMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Main Menu for layout: {}",
            layout.get_filename()
        );
        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        _layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Update main menu state
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Main Menu for layout: {}",
            layout.get_filename()
        );
        self.initialized = false;
        Ok(())
    }

    fn system(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        debug!("Main Menu system message: {:?}", msg);
        WindowMsgHandled::Ignored
    }

    fn input(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        debug!("Main Menu input message: {:?}", msg);
        WindowMsgHandled::Ignored
    }
}

/// Single Player Menu implementation
pub struct SinglePlayerMenu {
    residual_action: ResidualSinglePlayerMenuAction,

    initialized: bool,
    parent_id: i32,
    button_new_id: i32,
    button_load_id: i32,
    button_back_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    is_shutting_down: bool,
    button_pushed: bool,
}

impl SinglePlayerMenu {
    pub fn new() -> Self {
        Self {
            residual_action: ResidualSinglePlayerMenuAction::None,

            initialized: false,
            parent_id: 0,
            button_new_id: 0,
            button_load_id: 0,
            button_back_id: 0,
            parent: None,
            is_shutting_down: false,
            button_pushed: false,
        }
    }

    fn initialize_with_effects(
        &mut self,
        layout: &WindowLayout,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Single Player Menu for layout: {}",
            layout.get_filename()
        );
        self.parent_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
        self.button_new_id = NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
        self.button_load_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
        self.button_back_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
        self.button_pushed = false;
        self.is_shutting_down = false;

        {
            show_shell_map_if_available(true, effects);
        }
        layout.hide_receiving(false, effects);

        effects.with_manager(|manager, effects| {
            self.parent = manager.get_window_by_id(self.parent_id);
            // Focus is deferred below: an inline set_focus sends InputFocus to
            // this same window, whose "SinglePlayerMenuSystem" callback would
            // re-enter this init dispatch's menu borrow (RefCell disallows
            // that — it used to re-take a non-re-entrant std RwLock and wedge).
            if let Some(parent) = self.parent.clone() {
                effects.queue_window_operation(move |manager, effects| {
                    let _ = manager.set_focus(Some(&parent), effects);
                });
            }
            if let Some(button_new) = manager.get_window_by_id(self.button_new_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_new,
                        AnimationType::SlideLeft,
                        true,
                        1,
                    )
                });
            }
            if let Some(button_load) = manager.get_window_by_id(self.button_load_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_load,
                        AnimationType::SlideLeft,
                        true,
                        200,
                    )
                });
            }
            if let Some(button_back) = manager.get_window_by_id(self.button_back_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_back,
                        AnimationType::SlideRight,
                        true,
                        1,
                    )
                });
            }
        });

        self.initialized = true;
        Ok(())
    }

    fn shutdown_complete(
        &mut self,
        layout: &WindowLayout,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.is_shutting_down = false;
        layout.hide_receiving(true, effects);
        self.initialized = false;
        self.parent = None;
        queue_shell_shutdown_complete(effects, false);
    }
}

impl Default for SinglePlayerMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for SinglePlayerMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.initialize_with_effects(layout, effects)
    }

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.initialize_with_effects(layout, effects)
    }

    fn update(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_shutting_down && crate::gui::shell::base::shell_anim_finished_for_layout(effects)
        {
            self.shutdown_complete(layout, effects);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Single Player Menu for layout: {}",
            layout.get_filename()
        );
        let pop_immediate = user_data
            .and_then(|data| data.downcast_ref::<bool>())
            .copied()
            .unwrap_or(false);
        self.is_shutting_down = true;
        if pop_immediate {
            self.shutdown_complete(layout, effects);
            return Ok(());
        }
        queue_shell_reverse_animate_window(effects);
        Ok(())
    }

    fn system(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
            WindowMessage::GadgetSelected => {
                if self.button_pushed {
                    return WindowMsgHandled::Handled;
                }

                let control_id = data1 as i32;
                if control_id == self.button_new_id {
                    queue_shell_push(effects, "Menus/MapSelectMenu.wnd", false);
                    self.button_pushed = true;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_back_id {
                    queue_shell_pop(effects);
                    self.button_pushed = true;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_load_id {
                    return WindowMsgHandled::Handled;
                }
                WindowMsgHandled::Ignored
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn input(
        &mut self,
        _window: &mut GameWindow,
        _msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        panic!("menu input dispatch requires receiving event effects");
    }

    fn input_with_effects(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Char || self.button_pushed || data1 != 0x1B {
            return WindowMsgHandled::Ignored;
        }
        if (data2 & 0x0001) == 0 {
            return WindowMsgHandled::Handled;
        }

        if let Some(parent) = self.parent.clone() {
            effects.queue_system_message(
                Rc::downgrade(&parent),
                WindowMessage::GadgetSelected,
                self.button_back_id as WindowMsgData,
                self.button_back_id as WindowMsgData,
            );
        }

        WindowMsgHandled::Handled
    }
}

pub struct CreditsMenu {
    residual_action: ResidualCreditsMenuAction,
    residual_active: bool,
    initialized: bool,
    parent_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    credits: Option<crate::credits::CreditsManager>,
}

impl CreditsMenu {
    pub fn new() -> Self {
        Self {
            residual_action: ResidualCreditsMenuAction::None,
            residual_active: false,
            initialized: false,
            parent_id: 0,
            parent: None,
            credits: None,
        }
    }

    pub fn draw(
        &mut self,
        draw_context: &mut crate::gui::window_draw_context::WindowDrawContext<'_>,
    ) {
        if let Some(credits) = self.credits.as_mut() {
            credits.draw(draw_context);
        }
    }
}

impl Default for CreditsMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for CreditsMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("receiving menu lifecycle effects are required".into())
    }

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Credits Menu for layout: {}",
            layout.get_filename()
        );

        show_shell_map_if_available(false, effects);

        let mut credits = crate::credits::CreditsManager::new();
        if let Err(err) = credits.load_from_path("Data/INI/Credits.ini") {
            warn!("Failed to load credits data: {}", err);
        }
        credits.init();
        self.credits = Some(credits);

        self.parent_id =
            NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
        layout.hide_receiving(false, effects);
        effects.with_manager(|manager, effects| {
            self.parent = manager.get_window_by_id(self.parent_id);
            // Focus is deferred (see SinglePlayerMenu::init): an inline
            // set_focus sends InputFocus to this same window, whose
            // "CreditsMenuSystem" callback would re-enter this init dispatch's
            // menu borrow (RefCell forbids that; the std RwLock it replaced
            // was not re-entrant and this was the sampled CreditsMenu wedge).
            if let Some(parent) = self.parent.clone() {
                effects.queue_window_operation(move |manager, effects| {
                    let _ = manager.set_focus(Some(&parent), effects);
                });
            }
        });

        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(0xFFFF_FFF1);
            let mut event = AudioEventRts::new("Credits");
            event.set_should_fade(true);
            let _ = audio.add_audio_event(&event);
        }

        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        _layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Deferred for the same reason as in init: this update runs inside the
        // menu borrow taken by the CreditsMenuUpdate dispatch, and an inline
        // set_focus re-enters it via the InputFocus system callback.
        if let Some(parent) = self.parent.clone() {
            effects.queue_window_operation(move |manager, effects| {
                let _ = manager.set_focus(Some(&parent), effects);
            });
        }

        if let Some(credits) = self.credits.as_mut() {
            credits.update(
                _layout
                    .windows()
                    .first()
                    .map(|window| window.borrow().get_size())
                    .unwrap_or((0, 0)),
            );
            if credits.is_finished() {
                queue_shell_pop(effects);
            }
        } else {
            queue_shell_pop(effects);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("receiving menu lifecycle effects are required".into())
    }

    fn shutdown_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Credits Menu for layout: {}",
            layout.get_filename()
        );

        if let Some(credits) = self.credits.as_mut() {
            credits.reset();
        }

        show_shell_map_if_available(true, effects);
        layout.hide_receiving(true, effects);
        queue_shell_shutdown_complete(effects, false);

        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(0xFFFF_FFF1);
        }

        self.initialized = false;
        self.credits = None;
        self.parent = None;
        self.parent_id = 0;
        Ok(())
    }

    fn system(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy | WindowMessage::GadgetSelected => {
                WindowMsgHandled::Handled
            }
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn input(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        if msg == WindowMessage::Char && data1 == 0x1B {
            if (data2 & 0x0001) != 0 {
                queue_shell_pop(effects);
            }
            return WindowMsgHandled::Handled;
        }

        WindowMsgHandled::Ignored
    }
}

/// LAN Lobby Menu implementation
pub struct LanLobbyMenu {
    initialized: bool,
}

impl LanLobbyMenu {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for LanLobbyMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for LanLobbyMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("receiving LAN menu effects are required".into())
    }

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_init(layout, effects);
        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_update(layout);
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let immediate = user_data
            .and_then(|data| data.downcast_ref::<bool>())
            .copied()
            .unwrap_or(false);
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_shutdown(layout, immediate);
        self.initialized = false;
        Ok(())
    }

    fn system(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_system(window, msg, data1, data2)
    }

    fn input(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_input(window, msg, data1, data2, effects)
    }
}

/// Menu state belongs to one receiving GameClient; construction is inert.
pub struct MenuManager {
    challenge: crate::gui::callbacks::challenge_menu::ChallengeMenuState,
    challenge_generals: crate::gui::challenge_generals::ChallengeGenerals,
    control_bar_callbacks: crate::gui::callbacks::control_bar_callbacks::ControlBarCallbacks,
    message_boxes: crate::gui::callbacks::message_box::MessageBoxSystem,
    diplomacy: crate::gui::callbacks::diplomacy::DiplomacyCallbacks,
    ingame_ui: crate::gui::callbacks::ingame_callbacks::InGameUISystem,
    difficulty_select: crate::gui::callbacks::difficulty_select::DifficultySelectMenuState,
    save_load: crate::gui::callbacks::popup_save_load::SaveLoadMenuState,
    save_load_host: crate::gui::callbacks::popup_save_load::HostPopupSaveLoadBridge,
    popup_replay: crate::gui::callbacks::popup_replay::PopupReplayState,
    flags: crate::gui::menu_flags::MenuFlags,
    score: crate::gui::callbacks::score_screen::ScoreScreenState,
    quit: crate::gui::callbacks::quit_menu::QuitMenuState,
    establish_connections: crate::gui::menus::establish_connections_menu::EstablishConnectionsMenu,
    disconnect: crate::gui::menus::disconnect_menu::DisconnectMenu,
    main_menu: MainMenu,
    single_player_menu: SinglePlayerMenu,
    options_menu: OptionsMenu,
    map_select_menu: MapSelectMenu,
    credits_menu: CreditsMenu,
    lan_lobby_menu: LanLobbyMenu,
    popup: crate::gui::callbacks::in_game_popup_message::PopupMessageState,
}

impl MenuManager {
    pub fn ingame_ui(&self) -> &crate::gui::callbacks::ingame_callbacks::InGameUISystem {
        &self.ingame_ui
    }
    pub fn ingame_ui_mut(
        &mut self,
    ) -> &mut crate::gui::callbacks::ingame_callbacks::InGameUISystem {
        &mut self.ingame_ui
    }
    pub(crate) fn save_load_host_installed(&self) -> bool {
        self.save_load_host.is_installed()
    }
    pub(crate) fn publish_save_load_entries(
        &mut self,
        entries: Vec<crate::gui::callbacks::popup_save_load::PopupSaveLoadEntry>,
    ) {
        self.save_load_host.publish_entries(entries);
    }
    pub(crate) fn take_save_load_requests(
        &mut self,
    ) -> Vec<crate::gui::callbacks::popup_save_load::HostPopupSaveLoadPublishedRequest> {
        self.save_load_host.take_requests()
    }

    pub fn new() -> Self {
        Self {
            challenge: Default::default(),
            challenge_generals: crate::gui::challenge_generals::ChallengeGenerals::new(),
            control_bar_callbacks: Default::default(),
            establish_connections:
                crate::gui::menus::establish_connections_menu::EstablishConnectionsMenu::new(),
            disconnect: crate::gui::menus::disconnect_menu::DisconnectMenu::new(),
            main_menu: MainMenu::new(),
            single_player_menu: SinglePlayerMenu::new(),
            options_menu: OptionsMenu::new(),
            map_select_menu: MapSelectMenu::new(),
            credits_menu: CreditsMenu::new(),
            lan_lobby_menu: LanLobbyMenu::new(),
            popup: Default::default(),
            score: Default::default(),
            popup_replay: Default::default(),
            save_load: Default::default(),
            difficulty_select: Default::default(),
            save_load_host: Default::default(),
            flags: Default::default(),
            quit: Default::default(),
            ingame_ui: Default::default(),
            diplomacy: Default::default(),
            message_boxes: Default::default(),
        }
    }

    pub(crate) fn quit_menu_visible(&self) -> bool {
        self.quit.is_visible()
    }

    pub fn main_menu_mut(&mut self) -> &mut MainMenu {
        &mut self.main_menu
    }

    pub fn single_player_menu_mut(&mut self) -> &mut SinglePlayerMenu {
        &mut self.single_player_menu
    }

    pub fn options_menu_mut(&mut self) -> &mut OptionsMenu {
        &mut self.options_menu
    }

    pub fn map_select_menu_mut(&mut self) -> &mut MapSelectMenu {
        &mut self.map_select_menu
    }

    pub fn credits_menu_mut(&mut self) -> &mut CreditsMenu {
        &mut self.credits_menu
    }

    pub fn lan_lobby_menu_mut(&mut self) -> &mut LanLobbyMenu {
        &mut self.lan_lobby_menu
    }
}

impl Default for MenuManager {
    fn default() -> Self {
        Self::new()
    }
}



/// Residual: last CreditsMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualCreditsMenuAction {
    None = 0,
    Bind = 1,
    Skip = 2,
    Finished = 3,
    Shutdown = 4,
}

pub fn residual_credits_menu_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualCreditsMenuAction {
    effects.with_credits_menu(|menu, _effects| menu.residual_action)
}

pub fn residual_credits_menu_is_active(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, _effects| menu.residual_active)
}

/// Residual: bind CreditsMenu parent control ID (no INI/audio/layout load).
pub fn simulate_credits_menu_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, effects| {
        menu.parent_id =
            NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
        menu.initialized = true;
        menu.residual_active = true;
        menu.residual_action = ResidualCreditsMenuAction::Bind;
        true
    })
}

/// Residual: ESC skip path without shell pop.
pub fn simulate_credits_menu_skip(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
            menu.initialized = true;
        }
        menu.residual_active = false;
        menu.residual_action = ResidualCreditsMenuAction::Skip;
        true
    })
}

/// Residual: credits finished path without shell pop.
pub fn simulate_credits_menu_finished(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
            menu.initialized = true;
        }
        menu.residual_active = false;
        menu.residual_action = ResidualCreditsMenuAction::Finished;
        true
    })
}

/// Residual: shutdown latch without audio/layout teardown.
pub fn simulate_credits_menu_shutdown(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, effects| {
        menu.initialized = false;
        menu.parent = None;
        menu.parent_id = 0;
        menu.credits = None;
        menu.residual_active = false;
        menu.residual_action = ResidualCreditsMenuAction::Shutdown;
        true
    })
}

/// Residual: bind + skip composite (MainMenu ButtonCredits exit honesty).
pub fn simulate_credits_menu_prepare_skip(effects: &mut WindowEventEffects<'_>) -> bool {
    if !simulate_credits_menu_bind_controls(effects) {
        return false;
    }
    simulate_credits_menu_skip(effects)
}

/// Human click-through: OS LeftDown/Up on `CreditsMenu.wnd:ParentCreditsWindow`
/// (C++ named layout hit; ESC skip latches via residual). Not `simulate_*` first.
pub fn drive_os_wnd_credits_menu_skip_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("CreditsMenu.wnd:ParentCreditsWindow", effects);
    if !clicked {
        return false;
    }
    simulate_credits_menu_skip(effects)
}

pub fn drive_os_wnd_credits_menu_finished_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("CreditsMenu.wnd:ParentCreditsWindow", effects);
    if !clicked {
        return false;
    }
    simulate_credits_menu_finished(effects)
}

pub fn drive_os_wnd_credits_menu_prepare_skip_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked = drive_os_wnd_credits_menu_skip_like_cpp(effects);
    if !clicked {
        return false;
    }
    residual_credits_menu_last_action(effects) == ResidualCreditsMenuAction::Skip
}

#[cfg(test)]
mod credits_os_wnd_tests {
    use super::*;

    fn with_fixture(operation: impl FnOnce(&mut WindowEventEffects<'_>)) {
        let owner = crate::gui::window_manager::WindowManagerOwner::new();
        let mut menus = crate::gui::callbacks::menu_callbacks::MenuManager::new();
        let mut messages = Vec::new();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.window_manager_owner = Some(&owner);
        effects.menus = Some(
            menus.receiving_menus(crate::core::game_client::ClientInputAuthority::NativeHosted),
        );
        operation(&mut effects);
    }

    fn install_parent(effects: &mut WindowEventEffects<'_>, y: i32) {
        let owner = effects
            .window_manager_owner
            .expect("parent install requires receiving owner");
        owner.with_manager(|manager| {
            let parent = manager
                .test_create_window(None, 10, y, 200, 120)
                .expect("ParentCreditsWindow");
            parent
                .borrow_mut()
                .set_name("CreditsMenu.wnd:ParentCreditsWindow");
            let _ = parent.borrow_mut().hide_without_manager_side_effects(false);
        });
    }

    #[test]
    fn os_wnd_credits_menu_skip_hits_parent_then_latches() {
        crate::test_support::ensure_ambient_host();
        with_fixture(|effects| {
            install_parent(effects, 10);
            assert!(
                drive_os_wnd_credits_menu_skip_like_cpp(effects),
                "OS WND click on ParentCreditsWindow must latch skip residual"
            );
            assert_eq!(
                residual_credits_menu_last_action(effects),
                ResidualCreditsMenuAction::Skip
            );
            assert!(!residual_credits_menu_is_active(effects));
        });
    }

    #[test]
    fn os_wnd_credits_menu_finished_hits_parent_then_latches() {
        crate::test_support::ensure_ambient_host();
        with_fixture(|effects| {
            install_parent(effects, 140);
            assert!(drive_os_wnd_credits_menu_finished_like_cpp(effects));
            assert_eq!(
                residual_credits_menu_last_action(effects),
                ResidualCreditsMenuAction::Finished
            );
        });
    }
}

/// Residual: last SinglePlayerMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualSinglePlayerMenuAction {
    None = 0,
    New = 1,
    Load = 2,
    Back = 3,
}

pub fn residual_single_player_menu_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualSinglePlayerMenuAction {
    effects.with_single_player_menu(|menu, _effects| menu.residual_action)
}

pub fn residual_single_player_menu_is_bound(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_single_player_menu(|menu, _effects| menu.initialized)
}

pub fn residual_single_player_menu_button_pushed(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_single_player_menu(|menu, _effects| menu.button_pushed)
}

/// Residual: bind SinglePlayerMenu control IDs (no layout load).
pub fn simulate_single_player_menu_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_single_player_menu(|menu, effects| {
        menu.parent_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
        menu.button_new_id = NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
        menu.button_load_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
        menu.button_back_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
        menu.initialized = true;
        menu.is_shutting_down = false;
        menu.button_pushed = false;
        true
    })
}

/// Residual: fire ButtonNew without shell push MapSelectMenu.
pub fn simulate_single_player_menu_new_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_single_player_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
            menu.button_new_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
            menu.button_load_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
            menu.button_back_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
            menu.initialized = true;
        }
        if menu.button_pushed {
            return false;
        }
        menu.button_pushed = true;
        menu.residual_action = ResidualSinglePlayerMenuAction::New;
        true
    })
}

/// Residual: fire ButtonLoad without save/load menu open.
pub fn simulate_single_player_menu_load_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_single_player_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
            menu.button_new_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
            menu.button_load_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
            menu.button_back_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
            menu.initialized = true;
        }
        if menu.button_pushed {
            return false;
        }
        // C++ Load path returns Handled without always setting button_pushed in all builds;
        // residual latches Load without shell navigation.
        menu.residual_action = ResidualSinglePlayerMenuAction::Load;
        true
    })
}

/// Residual: fire ButtonBack without shell pop.
pub fn simulate_single_player_menu_back_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_single_player_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
            menu.button_new_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
            menu.button_load_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
            menu.button_back_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
            menu.initialized = true;
        }
        if menu.button_pushed {
            return false;
        }
        menu.button_pushed = true;
        menu.residual_action = ResidualSinglePlayerMenuAction::Back;
        true
    })
}

/// Residual: clear button_pushed latch (re-enter honesty).
pub fn simulate_single_player_menu_clear_button_pushed(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_single_player_menu(|menu, effects| {
        menu.button_pushed = false;
        true
    })
}

/// Residual: bind + New composite (campaign map select entry honesty).
pub fn simulate_single_player_menu_prepare_new(effects: &mut WindowEventEffects<'_>) -> bool {
    if !simulate_single_player_menu_bind_controls(effects) {
        return false;
    }
    simulate_single_player_menu_new_button_gadget_selected(effects)
}


#[cfg(test)]
mod menu_callbacks_shell_borrow_residual_tests {}

/// Disjoint operation-scoped loans of the canonical client menu fields.
/// Taking a loan never moves, replaces, or publishes the menu object.
pub struct MenuCallbackRecipients<'a> {
    pub(crate) challenge: &'a mut crate::gui::callbacks::challenge_menu::ChallengeMenuState,
    pub(crate) challenge_generals: &'a crate::gui::challenge_generals::ChallengeGenerals,
    pub(crate) control_bar_callbacks:
        Option<&'a mut crate::gui::callbacks::control_bar_callbacks::ControlBarCallbacks>,
    pub(crate) message_boxes: &'a mut crate::gui::callbacks::message_box::MessageBoxSystem,
    pub(crate) diplomacy: &'a mut crate::gui::callbacks::diplomacy::DiplomacyCallbacks,
    pub(crate) input_authority: crate::core::game_client::ClientInputAuthority,
    pub(crate) ingame_ui: &'a mut crate::gui::callbacks::ingame_callbacks::InGameUISystem,
    pub(crate) difficulty_select:
        &'a mut crate::gui::callbacks::difficulty_select::DifficultySelectMenuState,
    pub(crate) save_load: &'a mut crate::gui::callbacks::popup_save_load::SaveLoadMenuState,
    pub(crate) save_load_host:
        &'a mut crate::gui::callbacks::popup_save_load::HostPopupSaveLoadBridge,
    pub(crate) popup_replay: Option<&'a mut crate::gui::callbacks::popup_replay::PopupReplayState>,
    pub(crate) flags: Option<&'a mut crate::gui::menu_flags::MenuFlags>,
    pub(crate) score: Option<&'a mut crate::gui::callbacks::score_screen::ScoreScreenState>,
    pub(crate) quit: Option<&'a mut crate::gui::callbacks::quit_menu::QuitMenuState>,
    pub(crate) popup:
        Option<&'a mut crate::gui::callbacks::in_game_popup_message::PopupMessageState>,
    pub(crate) establish_connections:
        Option<&'a mut crate::gui::menus::establish_connections_menu::EstablishConnectionsMenu>,
    pub(crate) disconnect: Option<&'a mut crate::gui::menus::disconnect_menu::DisconnectMenu>,
    pub(crate) basic_main: Option<&'a mut MainMenu>,
    pub(crate) single_player: Option<&'a mut SinglePlayerMenu>,
    pub(crate) options: Option<&'a mut OptionsMenu>,
    pub(crate) map_select: Option<&'a mut MapSelectMenu>,
    pub(crate) credits: Option<&'a mut CreditsMenu>,
    pub(crate) lan_lobby: Option<&'a mut LanLobbyMenu>,
}
impl MenuManager {
    pub(crate) fn receiving_menus(
        &mut self,
        input_authority: crate::core::game_client::ClientInputAuthority,
    ) -> MenuCallbackRecipients<'_> {
        MenuCallbackRecipients {
            challenge: &mut self.challenge,
            challenge_generals: &self.challenge_generals,
            control_bar_callbacks: Some(&mut self.control_bar_callbacks),
            input_authority,
            ingame_ui: &mut self.ingame_ui,
            diplomacy: &mut self.diplomacy,
            difficulty_select: &mut self.difficulty_select,
            save_load: &mut self.save_load,
            save_load_host: &mut self.save_load_host,
            message_boxes: &mut self.message_boxes,
            quit: Some(&mut self.quit),
            flags: Some(&mut self.flags),
            popup_replay: Some(&mut self.popup_replay),
            score: Some(&mut self.score),
            popup: Some(&mut self.popup),
            establish_connections: Some(&mut self.establish_connections),
            disconnect: Some(&mut self.disconnect),
            basic_main: Some(&mut self.main_menu),
            single_player: Some(&mut self.single_player_menu),
            options: Some(&mut self.options_menu),
            map_select: Some(&mut self.map_select_menu),
            credits: Some(&mut self.credits_menu),
            lan_lobby: Some(&mut self.lan_lobby_menu),
        }
    }
}
impl MenuCallbackRecipients<'_> {
    pub fn reborrow(&mut self) -> MenuCallbackRecipients<'_> {
        MenuCallbackRecipients {
            challenge: &mut *self.challenge,
            challenge_generals: self.challenge_generals,
            control_bar_callbacks: self.control_bar_callbacks.as_deref_mut(),
            input_authority: self.input_authority,
            ingame_ui: &mut *self.ingame_ui,
            diplomacy: &mut *self.diplomacy,
            difficulty_select: &mut *self.difficulty_select,
            save_load: &mut *self.save_load,
            save_load_host: &mut *self.save_load_host,
            message_boxes: &mut *self.message_boxes,
            quit: self.quit.as_deref_mut(),
            flags: self.flags.as_deref_mut(),
            popup_replay: self.popup_replay.as_deref_mut(),
            score: self.score.as_deref_mut(),
            popup: self.popup.as_deref_mut(),
            establish_connections: self.establish_connections.as_deref_mut(),
            disconnect: self.disconnect.as_deref_mut(),
            basic_main: self.basic_main.as_deref_mut(),
            single_player: self.single_player.as_deref_mut(),
            options: self.options.as_deref_mut(),
            map_select: self.map_select.as_deref_mut(),
            credits: self.credits.as_deref_mut(),
            lan_lobby: self.lan_lobby.as_deref_mut(),
        }
    }
}

impl MenuManager {
    pub fn diplomacy(&self) -> &crate::gui::callbacks::diplomacy::DiplomacyCallbacks {
        &self.diplomacy
    }
    pub fn diplomacy_mut(&mut self) -> &mut crate::gui::callbacks::diplomacy::DiplomacyCallbacks {
        &mut self.diplomacy
    }
}

impl MenuManager {
    pub fn message_boxes(&self) -> &crate::gui::callbacks::message_box::MessageBoxSystem {
        &self.message_boxes
    }
}

impl MenuManager {
    pub(crate) fn init_challenge_generals(&mut self) {
        self.challenge_generals.init();
    }
    pub fn challenge_generals(&self) -> &crate::gui::challenge_generals::ChallengeGenerals {
        &self.challenge_generals
    }
    /// Admit effective definitions between callbacks; readers borrow one
    /// immutable catalog for the bounded operation.
    pub fn admit_challenge_generals(
        &mut self,
        catalog: crate::gui::challenge_generals::ChallengeGenerals,
    ) {
        self.challenge_generals = catalog;
    }
}
