//! Selected native button callbacks use borrowed owners and complete within the OS operation.
use super::WindowManager;
use crate::gui::callbacks::control_bar_callbacks::ControlBarViewport;
use crate::gui::control_bar::{ControlBar, ControlBarStage};
use crate::gui::game_window::{
    GameWindow, SelectedOwnerAction, WindowMessage, WindowMsgData, WindowMsgHandled, WindowStatus,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;

enum PendingCameraOptions {
    Accept(Option<i32>),
    Defaults(Option<Rc<RefCell<GameWindow>>>),
    Open(Rc<RefCell<GameWindow>>),
    OpenQuit(WindowMsgData),
    ReplayPreference { use_camera: bool, checked: bool },
}
pub struct CameraOptionsInputContext<'a> {
    preferences: &'a mut game_engine::common::user_preferences::UserPreferences,
    rules: game_engine::common::ini::ini_game_data::CameraInputRules,
    pending: Option<PendingCameraOptions>,
    terrain_lod: Option<&'a mut dyn FnMut(i32)>,
}
impl<'a> CameraOptionsInputContext<'a> {
    pub fn new(
        preferences: &'a mut game_engine::common::user_preferences::UserPreferences,
        rules: game_engine::common::ini::ini_game_data::CameraInputRules,
    ) -> Self {
        Self {
            preferences,
            rules,
            pending: None,
            terrain_lod: None,
        }
    }
    /// Actual native input lends its installed renderer owner for this operation.
    pub fn with_terrain_lod(mut self, receiver: &'a mut dyn FnMut(i32)) -> Self {
        self.terrain_lod = Some(receiver);
        self
    }
    fn complete(&mut self, effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
        match self.pending.take() {
            Some(PendingCameraOptions::ReplayPreference {
                use_camera,
                checked,
            }) => {
                self.preferences.set_string(
                    if use_camera {
                        "UseCameraInReplays"
                    } else {
                        "SaveCameraInReplays"
                    },
                    if checked { "yes" } else { "no" }.into(),
                );
            }
            Some(PendingCameraOptions::Accept(scroll)) => {
                crate::gui::callbacks::menu_callbacks::complete_options_accept_with_preferences(
                    self.preferences,
                    scroll,
                    self.terrain_lod
                        .as_mut()
                        .map(|receiver| &mut **receiver as &mut dyn FnMut(i32)),
                    effects,
                )
            }
            Some(PendingCameraOptions::Defaults(slider)) => {
                crate::gui::callbacks::menu_callbacks::complete_options_defaults_with_preferences(
                    slider,
                    self.rules.keyboard_default_scroll_factor,
                    effects,
                )
            }
            Some(PendingCameraOptions::Open(window)) => {
                if let Some(path) = self
                    .preferences
                    .configured_path()
                    .map(|path| path.to_path_buf())
                {
                    if let Some(path) = path.to_str() {
                        let _ = self.preferences.load(path);
                    }
                }
                let observation = crate::gui::callbacks::OptionsCameraInputObservation::observe(
                    self.rules,
                    self.preferences,
                );
                let window_id = window.borrow().get_id() as u32;
                let _ = effects.with_main_menu(|menu, effects| {
                    menu.select_options_with_camera_input(window_id, observation, effects)
                });
            }
            Some(PendingCameraOptions::OpenQuit(control_id)) => {
                if let Some(path) = self
                    .preferences
                    .configured_path()
                    .map(|path| path.to_path_buf())
                {
                    if let Some(path) = path.to_str() {
                        let _ = self.preferences.load(path);
                    }
                }
                let observation = crate::gui::callbacks::OptionsCameraInputObservation::observe(
                    self.rules,
                    self.preferences,
                );
                let _ = crate::gui::callbacks::quit_menu::select_quit_options_with_camera_input(
                    control_id,
                    observation,
                    effects,
                );
            }
            None => {}
        }
    }
}
pub struct ControlBarInputContext<'a> {
    pub control_bar_observation:
        Option<crate::gui::callbacks::control_bar_observation::ControlBarCallbackObservation>,
    pub menu_audio: Option<&'a mut dyn crate::gui::menu_audio::MenuAudioEffects>,
    pub diplomacy_world: Option<&'a crate::gui::callbacks::diplomacy::DiplomacyWorldFacts>,
    pub input_provenance: crate::gui::control_bar::HostControlBarInputProvenance,
    pub client_frame: Option<u32>,
    pub logic_frame: Option<u32>,
    pub ui: Option<&'a mut crate::core::subsystems::InGameUISubsystem>,
    pub menus: Option<crate::gui::callbacks::menu_callbacks::MenuCallbackRecipients<'a>>,
    pub ime: Option<&'a mut crate::gui::ime_manager::ImeManager>,
    pub upgrade_tooltip:
        crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource<'a>,
    pub camera_options: Option<CameraOptionsInputContext<'a>>,
    /// Owned result for this synchronous native operation; consumed by Main
    /// immediately after dispatch, never retained across a frame.
    pub replay_menu:
        Option<&'a mut crate::gui::callbacks::replay_menu::ingress::ReplayMenuOperation>,
    /// Bounded Recorder-only probe; captured from the same receiving match,
    /// independent of the view loan. Never retained by windows or a dialog.
    pub replay_version: Option<&'a mut dyn FnMut(&str) -> bool>,
    pub observer_roster:
        Option<&'a crate::gui::control_bar::control_bar_observer::NativeObserverRoster>,
    pub control_bar: &'a mut ControlBar,
    pub viewport: &'a mut dyn ControlBarViewport,
    pub replay: bool,
    pub game_ending: bool,
    pub tooltips_disabled: bool,
    pub cursor_mouse: Option<&'a crate::core::subsystems::MouseHandle>,
    pub window_manager_owner: Option<&'a crate::gui::window_manager::WindowManagerOwner>,
    pub campaign_manager: Option<&'a mut crate::gui::campaign_manager::CampaignManager>,
    pub menu_world: Option<crate::gui::campaign_launch_host_bridge::MenuWorldFacts>,
    pub menu_host: Option<&'a mut crate::gui::campaign_launch_host_bridge::MenuHostMailbox>,
    pub window_videos: Option<&'a mut crate::gui::window_video_manager::WindowVideoManager>,
    pub in_game_movies: Option<&'a mut crate::gui::window_video_manager::WindowVideoManager>,
    pub script_display: Option<&'a mut crate::core::script_action_handler::ScriptDisplayReceiver>,
    pub main_menu: Option<&'a mut crate::gui::shell::main_menu::MainMenu>,
    pub emitted_messages: Option<&'a mut Vec<crate::message_stream::game_message::GameMessage>>,
}
impl ControlBarInputContext<'_> {
    fn with_effects_and_options<R>(
        &mut self,
        manager: &mut WindowManager,
        operation: impl FnOnce(
            &mut WindowManager,
            &mut crate::gui::game_window::WindowEventEffects<'_>,
            Option<&mut CameraOptionsInputContext<'_>>,
            &mut crate::gui::gui_callbacks::control_bar_popup_description::TooltipState,
        ) -> R,
    ) -> R {
        let mut effects = crate::gui::game_window::WindowEventEffects {
            control_bar_observation: self.control_bar_observation,
            menu_audio: self
                .menu_audio
                .as_mut()
                .map(|audio| &mut **audio as &mut dyn crate::gui::menu_audio::MenuAudioEffects),
            input_provenance: self.input_provenance,
            standalone_view: None,
            ui: self.ui.as_deref_mut(),
            logic_frame: self.logic_frame,
            client_frame: self.client_frame,
            payloads: Default::default(),
            pending_window_callbacks: Default::default(),
            diplomacy_world: self.diplomacy_world,
            menus: self.menus.as_mut().map(|menus| menus.reborrow()),
            ime: self.ime.as_deref_mut(),
            manager: None,
            window_manager_owner: self.window_manager_owner,
            selected_window: None,
            emitted_messages: self
                .emitted_messages
                .as_deref_mut()
                .expect("native callback requires receiving message sink"),
            pending_control_bar_clicks: Some(&mut self.control_bar.pending_command_clicks),
            campaign_manager: self.campaign_manager.as_deref_mut(),
            main_menu: self.main_menu.as_deref_mut(),
            menu_world: self.menu_world,
            menu_host: self.menu_host.as_deref_mut(),
            script_display: self.script_display.as_deref_mut(),
            window_videos: self.window_videos.as_deref_mut(),
            pending_main_menu_actions: Vec::new(),
        };
        let result = operation(
            manager,
            &mut effects,
            self.camera_options.as_mut(),
            &mut self.control_bar.tooltip,
        );
        effects.with_receiving_manager(manager, |effects| {
            effects.complete_window_callbacks();
            effects.complete_pending_main_menu_actions();
        });
        result
    }

    pub(crate) fn with_effects<R>(
        &mut self,
        manager: &mut WindowManager,
        operation: impl FnOnce(
            &mut WindowManager,
            &mut crate::gui::game_window::WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        self.with_effects_and_options(manager, |manager, effects, _options, _tooltip| {
            operation(manager, effects)
        })
    }

    pub(crate) fn with_tooltip_effects<R>(
        &mut self,
        manager: &mut WindowManager,
        operation: impl FnOnce(
            &mut crate::gui::gui_callbacks::control_bar_popup_description::TooltipState,
            &mut WindowManager,
            &mut crate::gui::game_window::WindowEventEffects<'_>,
        ) -> R,
    ) -> R {
        self.with_effects_and_options(manager, |manager, effects, _options, tooltip| {
            operation(tooltip, manager, effects)
        })
    }

    pub(super) fn complete_options_accept(&mut self, manager: &mut WindowManager) {
        if self.camera_options.is_some() {
            self.with_effects_and_options(manager, |manager, effects, options, _tooltip| {
                if let Some(options) = options {
                    effects.with_receiving_manager(manager, |effects| options.complete(effects));
                }
            });
        }
    }
}

impl WindowManager {
    pub(super) fn send_input_selected_with_message_sink(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        routed: bool,
        emitted_messages: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        emitted_messages.selected_window = Some(Rc::downgrade(window));
        emitted_messages.with_receiving_manager(self, |effects| {
            let result = window
                .borrow_mut()
                .send_input_message_with_message_sink(msg, data1, data2, routed, effects, None);
            effects.complete_window_callbacks();
            result
        })
    }

    pub(super) fn send_input_selected(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        routed: bool,
        context: &mut ControlBarInputContext<'_>,
    ) -> WindowMsgHandled {
        let owned_options = context.camera_options.is_some() || context.observer_roster.is_some();
        let owned_replay = context.replay_menu.is_some();
        let owned_restart = context
            .replay_menu
            .as_ref()
            .is_some_and(|op| op.allow_offline_restart());
        let click_start = context.control_bar.pending_command_clicks.len();
        let (handled, notification) = context.with_effects(self, |manager, effects| {
            effects.selected_window = Some(Rc::downgrade(window));
            effects.with_receiving_manager(manager, |effects| {
                window
                    .borrow_mut()
                    .send_input_with_control_bar_notification(
                        msg,
                        data1,
                        data2,
                        owned_options,
                        owned_replay,
                        owned_restart,
                        effects,
                        None,
                    )
            })
        });
        let target_id = window.borrow().get_id() as u32;
        for click in &mut context.control_bar.pending_command_clicks[click_start..] {
            if click.control_id == target_id {
                click.target = Some(Rc::downgrade(window));
            }
        }
        if let Some(notification) = notification {
            if matches!(
                notification.action,
                SelectedOwnerAction::ReplayMenuLoad
                    | SelectedOwnerAction::ReplayMenuDoubleClick
                    | SelectedOwnerAction::ReplayMenuConfirm
                    | SelectedOwnerAction::ReplayMenuRestart
            ) {
                // These are operation-scoped service loans, not arena entries.
                // Lend them separately while Effects borrows the other fields.
                let mut operation = context.replay_menu.take();
                let mut version = context.replay_version.take();
                context.with_effects(self, |manager, effects| {
                    if let Some(operation) = operation.as_deref_mut() {
                        match notification.action {
                            SelectedOwnerAction::ReplayMenuLoad => {
                                operation.capture_menu(manager, false)
                            }
                            SelectedOwnerAction::ReplayMenuDoubleClick => {
                                operation.capture_menu(manager, true)
                            }
                            SelectedOwnerAction::ReplayMenuRestart => operation.capture_restart(),
                            SelectedOwnerAction::ReplayMenuConfirm => {
                                operation.capture_confirmation(&notification.owner.borrow());
                                let _ = manager.unset_modal(&notification.owner);
                                let _ = manager.destroy_window(notification.owner.clone());
                                manager.flush_destroy_queue(effects);
                            }
                            _ => unreachable!(),
                        }
                        if let Some(version) = version.as_mut() {
                            operation.complete_ui_in(manager, Some(&mut **version), effects);
                        } else {
                            operation.complete_ui_in(manager, None, effects);
                        }
                    }
                });
                context.replay_menu = operation;
                context.replay_version = version;
                return handled;
            }
            if let SelectedOwnerAction::ObserverButtonSlot(slot) = notification.action {
                if let Some(roster) = context.observer_roster {
                    // Resolve the authored button slot against this operation's
                    // fresh receiving roster; stale Core/userdata IDs never select.
                    let index = match slot {
                        Some(slot) => {
                            let Some(player) = roster.selectable().nth(slot) else {
                                return handled;
                            };
                            Some(player.index)
                        }
                        None => None,
                    };
                    context
                        .control_bar
                        .admit_observer_button_owned(index, roster);
                    crate::gui::control_bar::control_bar_observer::refresh_observer_windows_in_manager(self, roster, index);
                } else {
                    log::warn!(
                        "native observer callback has no receiving Recorder/roster capability"
                    );
                }
                return handled;
            }
            if notification.action != SelectedOwnerAction::ControlBarToggle {
                if !notification
                    .owner
                    .borrow()
                    .get_status()
                    .contains(WindowStatus::DESTROYED)
                {
                    let slider_id =
                        NameKeyGenerator::name_to_key("OptionsMenu.wnd:SliderScrollSpeed") as i32;
                    let scroll = self.get_window_by_id(slider_id).and_then(|slider| {
                        let slider = slider.borrow();
                        match slider.widget() {
                            Some(crate::gui::game_window::WindowWidget::HorizontalSlider(
                                value,
                            )) => Some(value.value()),
                            _ => None,
                        }
                    });
                    if let Some(options) = context.camera_options.as_mut() {
                        options.pending = match notification.action {
                            SelectedOwnerAction::OptionsAccept => {
                                Some(PendingCameraOptions::Accept(scroll))
                            }
                            SelectedOwnerAction::OptionsReplayPreference {
                                use_camera,
                                checked,
                            } => Some(PendingCameraOptions::ReplayPreference {
                                use_camera,
                                checked,
                            }),
                            SelectedOwnerAction::OptionsDefaults => Some(
                                PendingCameraOptions::Defaults(self.get_window_by_id(slider_id)),
                            ),
                            SelectedOwnerAction::ShowOptions => {
                                Some(PendingCameraOptions::Open(notification.owner.clone()))
                            }
                            SelectedOwnerAction::ShowQuitOptions => {
                                Some(PendingCameraOptions::OpenQuit(notification.control_id))
                            }
                            SelectedOwnerAction::ControlBarToggle
                            | SelectedOwnerAction::ObserverButtonSlot(_)
                            | SelectedOwnerAction::ReplayMenuLoad
                            | SelectedOwnerAction::ReplayMenuDoubleClick
                            | SelectedOwnerAction::ReplayMenuConfirm
                            | SelectedOwnerAction::ReplayMenuRestart => None,
                        };
                    }
                }
                // The push button is the sole message. Outer OS dispatch drops
                // this manager loan, then completes Accept before returning.
                return handled;
            }
            // Both lookup and completion use this receiving manager. Neither
            // re-enters the singleton while its outer RefMut is live.
            let owner_destroyed = notification
                .owner
                .borrow()
                .get_status()
                .contains(WindowStatus::DESTROYED);
            if !context.game_ending && !context.replay && !owner_destroyed {
                let parent_id =
                    NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent") as i32;
                let marker_id =
                    NameKeyGenerator::name_to_key("ControlBar.wnd:BackgroundMarker") as i32;
                let marker = self.get_window_by_id(marker_id);
                if let Some(parent) = self.get_window_by_id(parent_id) {
                    // Valid WND layout has a distinct master parent and button.
                    // A malformed self-parent cannot be mutably borrowed twice.
                    if !Rc::ptr_eq(&parent, window) {
                        if context.control_bar.get_control_bar_stage() != ControlBarStage::Default {
                            ControlBar::reveal_ingame_command_window_owned(self);
                        }
                        let parent_handle = Rc::clone(&parent);
                        let mut parent = parent.borrow_mut();
                        let mut button = window.borrow_mut();
                        context
                            .control_bar
                            .toggle_control_bar_stage_with_button_owned(
                                context.viewport,
                                context.replay,
                                Some(&mut parent),
                                Some(&mut button),
                                self,
                            );
                        // The stage captured its authored default position. The
                        // marker helper walks the already-borrowed parent and
                        // adjusts the moved stage back to that default origin.
                        if let Some(marker) = marker.as_ref().filter(|marker| {
                            !Rc::ptr_eq(marker, window) && !Rc::ptr_eq(marker, &parent_handle)
                        }) {
                            context
                                .control_bar
                                .capture_control_bar_marker_owned(&marker.borrow(), &parent);
                        }
                    }
                }
            }
        }
        handled
    }
}

impl ControlBarInputContext<'_> {
    pub fn update_control_bar_animations(
        &mut self,
        parent: Option<std::rc::Rc<std::cell::RefCell<crate::gui::GameWindow>>>,
        manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        self.menus
            .as_mut()
            .expect("receiving menus")
            .control_bar_callbacks
            .as_deref_mut()
            .expect("receiving control bar callbacks")
            .update_window_animations(parent, manager);
    }
}
