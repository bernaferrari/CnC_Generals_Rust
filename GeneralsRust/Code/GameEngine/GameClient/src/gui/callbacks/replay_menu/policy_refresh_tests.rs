use super::*;
#[derive(Default)]
struct Receiver {
    files: Vec<String>,
}
impl ReplayMenuReceiver for Receiver {
    fn version_differs(&mut self, _: &str) -> bool {
        false
    }
    fn playback_file(&mut self, file: String) {
        self.files.push(file);
    }
    fn restart(&mut self) {
        unreachable!("test selects Load only")
    }
}
#[test]
fn admitted_content_crc_refresh_preserves_active_native_selection_and_release() {
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
    let mut boot_inputs = inputs.clone();
    boot_inputs.current_ini_crc = 0;
    let mut pending_content = boot_inputs.clone();
    pending_content.metadata.limits.color_count = 0;
    pending_content.metadata.limits.player_template_count = 0;
    manager.configure_replay_menu_owned(pending_content);
    let boot_owner = manager.replay_menu.as_ref().unwrap().clone();
    // The worker's actual loaded definitions become ready before authored
    // ReplayMenu initialization. Re-admit them without replacing the owner.
    manager.configure_replay_menu_owned(boot_inputs.clone());
    assert!(Rc::ptr_eq(
        &boot_owner,
        manager.replay_menu.as_ref().unwrap()
    ));
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
    for (phase, message) in [WindowMessage::LeftDown, WindowMessage::LeftUp]
        .into_iter()
        .enumerate()
    {
        manager.configure_replay_menu_owned(if phase == 0 {
            boot_inputs.clone()
        } else {
            inputs.clone()
        });
        {
            let state = owner.borrow();
            assert_eq!(state.menu.get_selected_index(), 0);
            assert_eq!(state.menu.selected_replay_filename().unwrap(), filename);
            let expected = if phase == 0 { 128 } else { 255 };
            assert_eq!(state.menu.get_replay_list()[0].color.r, expected);
            let mut list_window = state.listbox_window.as_ref().unwrap().borrow_mut();
            let list = list_window.list_box_mut().unwrap();
            assert_eq!(list.items().len(), 1);
            assert_eq!(list.items()[0].text_color.unwrap().r, expected);
            let displayed_color = list.items()[0].column_colors[0]
                .or(list.items()[0].text_color)
                .unwrap();
            assert_eq!(displayed_color.r, expected);
        }
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
            script_display: None,
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
