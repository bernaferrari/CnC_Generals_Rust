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
                );
            }
            None => {}
        }
    }
}
pub struct ControlBarInputContext<'a> {
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
    pub script_display: Option<&'a mut crate::core::script_action_handler::ScriptDisplayReceiver>,
    pub main_menu: Option<&'a mut crate::gui::shell::main_menu::MainMenu>,
    pub emitted_messages: Option<&'a mut Vec<crate::message_stream::game_message::GameMessage>>,
}
impl ControlBarInputContext<'_> {
    pub(super) fn complete_options_accept(&mut self) {
        if let Some(options) = self.camera_options.as_mut() {
            let mut effects = crate::gui::game_window::WindowEventEffects {
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
            options.complete(&mut effects);
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
        mut context: Option<&mut ControlBarInputContext<'_>>,
        emitted_messages: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        assert!(
            context.is_none(),
            "native input context and stream translator are distinct synchronous dispatch paths"
        );
        emitted_messages.selected_window = Some(Rc::downgrade(window));
        window.borrow_mut().send_input_message_with_message_sink(
            msg,
            data1,
            data2,
            routed,
            emitted_messages,
            None,
        )
    }

    pub(super) fn send_input_selected(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        routed: bool,
        mut context: Option<&mut ControlBarInputContext<'_>>,
    ) -> WindowMsgHandled {
        let Some(context) = context.as_deref_mut() else {
            return if routed {
                window
                    .borrow_mut()
                    .send_routed_input_message(msg, data1, data2)
            } else {
                window.borrow_mut().send_input_message(msg, data1, data2)
            };
        };
        let owned_options = context.camera_options.is_some() || context.observer_roster.is_some();
        let owned_replay = context.replay_menu.is_some();
        let owned_restart = context
            .replay_menu
            .as_ref()
            .is_some_and(|op| op.allow_offline_restart());
        let click_start = context.control_bar.pending_command_clicks.len();
        let mut effects = context
            .emitted_messages
            .as_deref_mut()
            .map(
                |emitted_messages| crate::gui::game_window::WindowEventEffects {
                    window_manager_owner: context.window_manager_owner,
                    selected_window: Some(Rc::downgrade(window)),
                    emitted_messages,
                    pending_control_bar_clicks: Some(
                        &mut context.control_bar.pending_command_clicks,
                    ),
                    campaign_manager: context.campaign_manager.as_deref_mut(),
                    main_menu: context.main_menu.as_deref_mut(),
                    menu_world: context.menu_world,
                    menu_host: context.menu_host.as_deref_mut(),
                    script_display: context.script_display.as_deref_mut(),
                    window_videos: context.window_videos.as_deref_mut(),
                    pending_main_menu_actions: Vec::new(),
                },
            );
        let emitted_messages = effects.as_mut();
        let (handled, notification) = {
            let pending_control_bar_clicks = None;
            window
                .borrow_mut()
                .send_input_with_control_bar_notification(
                    msg,
                    data1,
                    data2,
                    owned_options,
                    owned_replay,
                    owned_restart,
                    emitted_messages,
                    pending_control_bar_clicks,
                )
        };
        drop(effects);
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
                if let Some(operation) = context.replay_menu.as_deref_mut() {
                    match notification.action {
                        SelectedOwnerAction::ReplayMenuLoad => operation.capture_menu(self, false),
                        SelectedOwnerAction::ReplayMenuDoubleClick => {
                            operation.capture_menu(self, true)
                        }
                        SelectedOwnerAction::ReplayMenuRestart => operation.capture_restart(),
                        SelectedOwnerAction::ReplayMenuConfirm => {
                            operation.capture_confirmation(&notification.owner.borrow());
                            let _ = self.unset_modal(&notification.owner);
                            let _ = self.destroy_window(notification.owner.clone());
                            self.flush_destroy_queue();
                        }
                        _ => unreachable!(),
                    }
                    if let Some(version) = context.replay_version.as_mut() {
                        operation.complete_ui_in(self, Some(&mut **version));
                    } else {
                        operation.complete_ui_in(self, None);
                    }
                }
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
