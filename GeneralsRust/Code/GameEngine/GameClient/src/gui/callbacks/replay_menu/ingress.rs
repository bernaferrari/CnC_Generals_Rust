//! Replay admission owns its source identity and holds no window/tree loan.
use super::*;
use std::rc::Weak;

#[derive(Debug)]
pub struct ReplayPlayback {
    filename: String,
    parent: Option<Weak<RefCell<GameWindow>>>,
    owner: crate::gui::window_manager::WindowManagerReceipt,
}
impl ReplayPlayback {
    pub(crate) fn captured(
        filename: String,
        parent: Option<Weak<RefCell<GameWindow>>>,
        owner: crate::gui::window_manager::WindowManagerReceipt,
    ) -> Self {
        Self {
            filename,
            parent,
            owner,
        }
    }
    pub fn filename(&self) -> &str {
        &self.filename
    }
    /// ReplayMenu.cpp412–417/474–479/542–547 hide after playback even on failure.
    pub fn playback(self, receiver: &mut dyn ReplayMenuReceiver) -> Result<(), String> {
        let result = receiver.playback_file(self.filename);
        if let Some(parent) = self.parent {
            self.owner.hide_registered_window(parent);
        }
        result
    }
}

/// UI delivery belongs to the receiving application, just like replay admission.
/// Headless receivers explicitly reject unsupported UI actions.
pub trait ReplayMenuReceiver {
    fn version_differs(&mut self, filename: &str) -> bool;
    fn playback_file(&mut self, filename: String) -> Result<(), String>;
    fn confirm_older(&mut self, _playback: ReplayPlayback) -> Result<(), String> {
        Err("replay confirmation requires receiving UI".into())
    }
    fn no_selection(&mut self) -> Result<(), String> {
        Err("replay selection feedback requires receiving UI".into())
    }
    fn confirm_restart(&mut self) -> Result<(), String> {
        Err("replay restart confirmation requires receiving UI".into())
    }
    fn restart(&mut self) -> Result<(), String>;
}
#[derive(Debug)]
pub enum ReplayMenuStep {
    CheckVersion(ReplayPlayback),
    Playback(ReplayPlayback),
    Restart,
    ConfirmRestart,
    NoSelection,
}
impl ReplayMenuStep {
    pub(crate) fn complete_compatibility(
        self,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        let result = self.complete(&mut CompatibilityReplayReceiver { effects });
        if let Err(error) = result {
            effects.with_manager(|manager, effects| {
                let _ = message_box_ok(
                    &GameText::fetch("GUI:Error"),
                    &error,
                    None,
                    manager,
                    effects,
                );
            });
        }
    }
    /// Same admitted operation, after the selected Window and manager loan end.
    pub fn complete(self, receiver: &mut dyn ReplayMenuReceiver) -> Result<(), String> {
        match self {
            Self::CheckVersion(playback) if receiver.version_differs(playback.filename()) => {
                receiver.confirm_older(playback)
            }
            Self::CheckVersion(playback) | Self::Playback(playback) => playback.playback(receiver),
            Self::Restart => receiver.restart(),
            Self::ConfirmRestart => receiver.confirm_restart(),
            Self::NoSelection => receiver.no_selection(),
        }
    }
}

pub struct ReplayMenuOperation {
    pending: Option<ReplayMenuStep>,
    allow_offline_restart: bool,
    owner: crate::gui::window_manager::WindowManagerReceipt,
}
impl ReplayMenuOperation {
    /// Supplied from the driving host's actual non-network mode. Multiplayer
    /// surrender remains its separate callback and never enters this route.
    pub fn new(
        allow_offline_restart: bool,
        owner: crate::gui::window_manager::WindowManagerReceipt,
    ) -> Self {
        Self {
            pending: None,
            owner,
            allow_offline_restart,
        }
    }
    pub(crate) fn allow_offline_restart(&self) -> bool {
        self.allow_offline_restart
    }
    pub(crate) fn capture_restart(&mut self) {
        self.pending = Some(ReplayMenuStep::ConfirmRestart);
    }

    pub fn take(&mut self) -> Option<ReplayMenuStep> {
        self.pending.take()
    }

    pub(crate) fn capture_menu(
        &mut self,
        manager: &mut crate::gui::window_manager::WindowManager,
        direct: bool,
    ) {
        let Some(state) = manager.replay_menu.as_ref() else {
            log::warn!("native ReplayMenu requires receiving application policy before init");
            return;
        };
        let mut state = state.borrow_mut();
        sync_selected_index(&mut state);
        match state.menu.selected_replay_filename() {
            Ok(filename) => {
                let playback = ReplayPlayback {
                    filename,
                    parent: state.parent.clone(),
                    owner: self.owner.clone(),
                };
                self.pending = Some(if direct {
                    ReplayMenuStep::Playback(playback)
                } else {
                    ReplayMenuStep::CheckVersion(playback)
                });
            }
            Err(_) => {
                // Dialog creation re-enters the manager, so defer only the
                // owned domain response until the outer loan has ended.
                self.pending = Some(ReplayMenuStep::NoSelection);
            }
        }
    }

    /// Native dialog decisions run at the selected notification while the
    /// actual manager is explicitly borrowed. Only playback/YES returns to
    /// Main after this loan; no ambient manager is rediscovered by completion.
    pub(crate) fn complete_ui_in(
        &mut self,
        manager: &mut crate::gui::window_manager::WindowManager,
        version: Option<&mut dyn FnMut(&str) -> bool>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.pending = match self.pending.take() {
            Some(ReplayMenuStep::CheckVersion(playback)) => {
                if let Some(version) = version {
                    if version(playback.filename()) {
                        super::super::message_box::replay_version_confirmation_in(
                            manager, playback, effects,
                        );
                        None
                    } else {
                        Some(ReplayMenuStep::Playback(playback))
                    }
                } else {
                    log::warn!("native replay Load has no receiving Recorder version capability");
                    None
                }
            }
            Some(ReplayMenuStep::NoSelection) => {
                super::super::message_box::replay_no_selection_in(manager, effects);
                None
            }
            Some(ReplayMenuStep::ConfirmRestart) => {
                super::super::message_box::restart_confirmation_in(manager, effects);
                None
            }
            other => other,
        };
    }

    pub(crate) fn capture_confirmation(&mut self, owner: &GameWindow) {
        self.pending = owner
            .get_user_data::<super::super::message_box::WindowMessageBoxData>()
            .and_then(|data| data.take_owned_replay());
    }
}

/// Core-only adapter. Its pending step has already left all WND callbacks.
struct CompatibilityReplayReceiver<'a, 'effects> {
    effects: &'a mut crate::gui::game_window::WindowEventEffects<'effects>,
}
impl ReplayMenuReceiver for CompatibilityReplayReceiver<'_, '_> {
    fn version_differs(&mut self, filename: &str) -> bool {
        with_recorder_mut(|recorder| recorder.test_version_playback(filename.into()))
            .and_then(Result::ok)
            .unwrap_or(false)
    }
    fn playback_file(&mut self, filename: String) -> Result<(), String> {
        with_recorder_mut(|recorder| recorder.playback_file(filename))
            .ok_or_else(|| "standalone replay requires configured Recorder".to_string())?
            .map(|_| ())
            .map_err(|error| error.to_string())
    }
    fn confirm_older(&mut self, playback: ReplayPlayback) -> Result<(), String> {
        let mut playback = Some(playback);
        let callback: super::super::message_box::MessageBoxFunc = Box::new(move |effects| {
            if let Some(playback) = playback.take() {
                effects.queue_compatibility_replay(ReplayMenuStep::Playback(playback));
            }
        });
        self.effects.with_manager(|manager, effects| {
            let _ = super::super::message_box::message_box_ok_cancel(
                &GameText::fetch("GUI:OlderReplayVersionTitle"),
                &GameText::fetch("GUI:OlderReplayVersion"),
                Some(callback),
                None,
                manager,
                effects,
            );
        });
        Ok(())
    }
    fn no_selection(&mut self) -> Result<(), String> {
        self.effects.with_manager(|manager, effects| {
            let _ = message_box_ok(
                &GameText::fetch("GUI:NoFileSelected"),
                &GameText::fetch("GUI:PleaseSelectAFile"),
                None,
                manager,
                effects,
            );
        });
        Ok(())
    }
    fn confirm_restart(&mut self) -> Result<(), String> {
        let callback: super::super::message_box::MessageBoxFunc =
            Box::new(|effects| effects.queue_compatibility_replay(ReplayMenuStep::Restart));
        self.effects.with_manager(|manager, effects| {
            let _ = super::super::message_box::message_box_yes_no(
                &GameText::fetch("GUI:RestartConfirmationTitle"),
                &GameText::fetch("GUI:RestartConfirmation"),
                Some(callback),
                None,
                manager,
                effects,
            );
        });
        Ok(())
    }
    fn restart(&mut self) -> Result<(), String> {
        Err("Core replay requires an explicit receiving restart service".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Receiver {
        files: Vec<String>,
        version_queries: Vec<String>,
    }
    impl ReplayMenuReceiver for Receiver {
        fn version_differs(&mut self, filename: &str) -> bool {
            self.version_queries.push(filename.into());
            false
        }
        fn playback_file(&mut self, filename: String) {
            self.files.push(filename);
        }
        fn restart(&mut self) {
            panic!("unexpected restart")
        }
    }
    #[test]
    fn captured_filename_survives_other_menu_and_parent_reset() {
        let parent_a = Rc::new(RefCell::new(GameWindow::new()));
        let parent_b = Rc::new(RefCell::new(GameWindow::new()));
        let first = ReplayPlayback::captured("A.rep".into(), Some(Rc::downgrade(&parent_a)));
        let second = ReplayPlayback::captured("B.rep".into(), Some(Rc::downgrade(&parent_b)));
        drop(parent_a);
        let mut receiver = Receiver::default();
        second.playback(&mut receiver);
        first.playback(&mut receiver);
        assert_eq!(receiver.files, ["B.rep", "A.rep"]);
        assert!(parent_b.borrow().is_hidden());
    }
    #[test]
    fn failed_playback_call_still_hides_only_captured_parent() {
        // Receiver deliberately admits no successful game: source hides after
        // the call regardless of header Boolean/error, hq-ae0hh.
        let parent = Rc::new(RefCell::new(GameWindow::new()));
        let foreign = Rc::new(RefCell::new(GameWindow::new()));
        let _ = parent.borrow_mut().hide(false);
        let _ = foreign.borrow_mut().hide(false);
        let mut receiver = Receiver::default();
        ReplayPlayback::captured("missing.rep".into(), Some(Rc::downgrade(&parent)))
            .playback(&mut receiver);
        assert_eq!(receiver.files, ["missing.rep"]);
        assert!(parent.borrow().is_hidden());
        assert!(!foreign.borrow().is_hidden());
    }
    #[test]
    fn direct_double_click_does_not_add_version_query() {
        let mut receiver = Receiver::default();
        ReplayMenuStep::Playback(ReplayPlayback::captured("direct.rep".into(), None))
            .complete(&mut receiver);
        assert_eq!(receiver.files, ["direct.rep"]);
        assert!(receiver.version_queries.is_empty());
        ReplayMenuStep::CheckVersion(ReplayPlayback::captured("checked.rep".into(), None))
            .complete(&mut receiver);
        assert_eq!(receiver.version_queries, ["checked.rep"]);
        assert_eq!(receiver.files, ["direct.rep", "checked.rep"]);
    }
    #[test]
    fn actual_two_notification_load_keeps_receiving_menu_and_recorded_filename() {
        use crate::gui::callbacks::control_bar_callbacks::ControlBarViewport;
        use crate::gui::control_bar::ControlBar;
        use crate::gui::gadgets::{ListBox, PushButton};
        use crate::gui::game_window::{WindowInputReturnCode, WindowWidget};
        use crate::gui::window_manager::{ControlBarInputContext, WindowManager};
        use crate::gui::window_script::WindowDefinition;
        use game_engine::common::recorder::{Recorder, RecorderOwnerInputs, ReplayGameInfo};
        let dir = tempfile::tempdir().unwrap();
        let policy = RecorderOwnerInputs {
            map: "Maps\\OwnedReplay.map".into(),
            seed: 47,
            replay_directory: dir.path().join("Replays"),
            exe_crc: 2,
            ini_crc: 3,
            save_stats: false,
            stats_directory: dir.path().join("Stats"),
        };
        let mut writer = Recorder::new_owned(policy.clone());
        let mut info = ReplayGameInfo::new();
        info.map = policy.map.clone();
        info.seed = policy.seed;
        info.slots[0].is_human = true;
        info.slots[0].is_occupied = true;
        info.slots[0].name = "Owner".into();
        info.slots[0].player_template = 0;
        info.slots[0].color = 0;
        info.slots[0].start_pos = 0;
        writer.set_recording_lobby(info, 0);
        let filename = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        writer.start_recording(1, 2, 0, 30).unwrap();
        writer.stop_recording();
        let version = game_engine::common::version::get_version();
        let inputs = crate::gui::shell::replay_menu::ReplayMenuInputs {
            replay_directory: policy.replay_directory.clone(),
            replay_extension: ".rep".into(),
            last_replay_filename: "00000000".into(),
            current_version: version.get_unicode_version(),
            current_version_number: version.get_version_number(),
            current_exe_crc: 2,
            current_ini_crc: 3,
            metadata: crate::gui::shell::replay_menu::fixture_replay_metadata_policy(1, 1),
        };
        let mut manager = WindowManager::new();
        manager.configure_replay_menu_owned(inputs.clone());
        let key = NameKeyGenerator::name_to_key;
        let parent = manager
            .create_window_with_id(
                None,
                0,
                0,
                220,
                120,
                key("ReplayMenu.wnd:ParentReplayMenu") as i32,
            )
            .unwrap();
        manager.bind_window_callbacks(
            &mut parent.borrow_mut(),
            &WindowDefinition {
                system_callback: "ReplayMenuSystem".into(),
                ..Default::default()
            },
        );
        let id = key("ReplayMenu.wnd:ButtonLoadReplay") as i32;
        let button = manager
            .create_window_with_id(Some(&parent), 10, 10, 40, 20, id)
            .unwrap();
        button
            .borrow_mut()
            .set_widget(WindowWidget::PushButton(PushButton::new(
                id as u32, 0, 0, 40, 20,
            )));
        manager.bind_window_callbacks(
            &mut button.borrow_mut(),
            &WindowDefinition {
                input_callback: "GadgetPushButtonInput".into(),
                ..Default::default()
            },
        );
        let list_id = key("ReplayMenu.wnd:ListboxReplayFiles") as i32;
        let list = manager
            .create_window_with_id(Some(&parent), 70, 10, 120, 80, list_id)
            .unwrap();
        list.borrow_mut()
            .set_widget(WindowWidget::ListBox(ListBox::new(
                list_id as u32,
                0,
                0,
                120,
                80,
            )));
        let owner = manager.replay_menu.as_ref().unwrap().clone();
        {
            let mut state = owner.borrow_mut();
            state.parent = Some(Rc::downgrade(&parent));
            state.listbox_window = Some(list);
            state.menu.init();
            populate_replay_listbox(&mut state);
            assert_eq!(
                state.menu.get_replay_list().len(),
                1,
                "real owned Recorder header must populate native list"
            );
        }
        struct View;
        impl ControlBarViewport for View {
            fn display_size(&self) -> (u32, u32) {
                (800, 600)
            }
            fn set_height_px(&mut self, _: i32) {}
        }
        let mut bar = ControlBar::new();
        let mut view = View;
        let mut receiver = Receiver::default();
        let mut reader = Recorder::new_owned(policy);
        for message in [WindowMessage::LeftDown, WindowMessage::LeftUp] {
            manager.configure_replay_menu_owned(inputs.clone());
            assert!(
                Rc::ptr_eq(&owner, manager.replay_menu.as_ref().unwrap()),
                "input admission must retain the actual selected menu owner"
            );
            let mut operation = ReplayMenuOperation::new(true);
            let mut probe = |selected: &str| {
                assert_eq!(selected, filename);
                reader.test_version_playback(selected.into()).unwrap()
            };
            let mut context = ControlBarInputContext {
            menus: None,
            ime:None,
            in_game_movies: None,
            script_display: None,
            window_videos: None,
            upgrade_tooltip: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
                replay_menu: Some(&mut operation),
                replay_version: Some(&mut probe),
                camera_options: None,
                observer_roster: None,
                control_bar: &mut bar,
                viewport: &mut view,
                replay: false,
                game_ending: false,
                tooltips_disabled: false,
                cursor_mouse: None,
                emitted_messages: None,

                main_menu: None,
            };
            assert_eq!(
                manager.process_mouse_event_with_control_bar(
                    message,
                    15,
                    15,
                    ((15u32 << 16) | 15) as WindowMsgData,
                    &mut context
                ),
                WindowInputReturnCode::Used
            );
            drop(context);
            if let Some(action) = operation.take() {
                action.complete(&mut receiver);
            }
        }
        assert_eq!(receiver.files, [filename]);
        assert!(parent.borrow().is_hidden());
    }
}

#[cfg(test)]
#[path = "policy_refresh_tests.rs"]
mod policy_refresh_tests;
