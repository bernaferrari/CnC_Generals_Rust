//! Actual selected MessageBox WND notification retains its initiating replay.
use super::*;
use crate::gui::callbacks::control_bar_callbacks::ControlBarViewport;
use crate::gui::callbacks::message_box::WindowMessageBoxData;
use crate::gui::callbacks::replay_menu::ingress::{
    ReplayMenuOperation, ReplayMenuReceiver, ReplayPlayback,
};
use crate::gui::control_bar::ControlBar;
use crate::gui::gadgets::PushButton;
use crate::gui::game_window::*;
use crate::gui::window_script::WindowDefinition;
use game_engine::common::name_key_generator::NameKeyGenerator;

struct View;
impl ControlBarViewport for View {
    fn display_size(&self) -> (u32, u32) {
        (800, 600)
    }
    fn set_height_px(&mut self, _: i32) {}
}
#[derive(Default)]
struct Receiver(Vec<String>);
impl ReplayMenuReceiver for Receiver {
    fn version_differs(&mut self, _: &str) -> bool {
        panic!("confirmation must not recheck current menu")
    }
    fn playback_file(&mut self, filename: String) {
        self.0.push(filename);
    }
    fn restart(&mut self) {
        self.0.push("restart".into());
    }
}
fn confirmation(
    filename: &str,
) -> (
    WindowManager,
    Rc<RefCell<GameWindow>>,
    Rc<RefCell<GameWindow>>,
) {
    let mut manager = WindowManager::new();
    let key = NameKeyGenerator::name_to_key;
    let parent = manager
        .create_window_with_id(None, 0, 0, 200, 100, key("MessageBox.wnd:") as i32)
        .unwrap();
    let menu = Rc::new(RefCell::new(GameWindow::new()));
    let _ = menu.borrow_mut().hide(false);
    let playback = ReplayPlayback::captured(filename.into(), Some(Rc::downgrade(&menu)));
    parent
        .borrow_mut()
        .set_user_data(WindowMessageBoxData::default().with_owned_replay(playback));
    let id = key("MessageBox.wnd:ButtonOk") as i32;
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
    (manager, menu, button)
}
fn select(
    manager: &mut WindowManager,
    button: &Rc<RefCell<GameWindow>>,
    allow_restart: bool,
) -> ReplayMenuOperation {
    let (x, y) = button.borrow().get_screen_position();
    let (x, y) = (x + 5, y + 5);
    let data = (((y as u32) << 16) | ((x as u32) & 0xffff)) as WindowMsgData;
    let mut operation = ReplayMenuOperation::new(allow_restart);
    let mut bar = ControlBar::new();
    let mut view = View;
    let mut context = ControlBarInputContext {
            diplomacy_world: None,
            menus: None,
            ime:None,
            in_game_movies: None,
            script_display: None,
            window_videos: None,
            upgrade_tooltip: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
        replay_version: None,
        replay_menu: Some(&mut operation),
        camera_options: None,
        observer_roster: None,
        control_bar: &mut bar,
        viewport: &mut view,
        replay: false,
        game_ending: false,
        tooltips_disabled: false,
        cursor_mouse: None,
        emitted_messages: None,
            campaign_manager: None,
            window_manager_owner: None,

        main_menu: None,
    };
    for msg in [WindowMessage::LeftDown, WindowMessage::LeftUp] {
        assert_eq!(
            manager.process_mouse_event_with_control_bar(msg, x, y, data, &mut context),
            WindowInputReturnCode::Used
        );
    }
    drop(context);
    operation
}
#[test]
fn wnd_confirmation_uses_captured_file_across_two_receiving_managers() {
    let (mut a, menu_a, button_a) = confirmation("A.rep");
    let (mut b, menu_b, button_b) = confirmation("B.rep");
    assert_eq!(button_a.borrow().get_id(), button_b.borrow().get_id());
    let mut first = select(&mut a, &button_a, false);
    let mut second = select(&mut b, &button_b, false);
    assert!(!menu_a.borrow().is_hidden());
    assert!(!menu_b.borrow().is_hidden());
    let mut receiver = Receiver::default();
    second
        .take()
        .expect("actual B OK callback")
        .complete(&mut receiver);
    assert!(menu_b.borrow().is_hidden());
    assert!(!menu_a.borrow().is_hidden());
    first
        .take()
        .expect("actual A OK callback")
        .complete(&mut receiver);
    assert_eq!(receiver.0, ["B.rep", "A.rep"]);
    assert!(menu_a.borrow().is_hidden());
    assert!(first.take().is_none());
    assert!(second.take().is_none());
}

#[test]
fn wnd_restart_confirmation_delivers_receiving_restart_once() {
    let mut manager = WindowManager::new();
    let key = NameKeyGenerator::name_to_key;
    let parent = manager
        .create_window_with_id(None, 0, 0, 200, 100, key("MessageBox.wnd:") as i32)
        .unwrap();
    parent
        .borrow_mut()
        .set_user_data(WindowMessageBoxData::default().with_owned_restart());
    let id = key("MessageBox.wnd:ButtonYes") as i32;
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
    let mut operation = select(&mut manager, &button, true);
    let mut receiver = Receiver::default();
    assert!(
        receiver.0.is_empty(),
        "notification releases its window loan before restart"
    );
    operation
        .take()
        .expect("actual YES notification")
        .complete(&mut receiver);
    assert_eq!(receiver.0, ["restart"]);
    assert!(operation.take().is_none());
}
#[test]
fn wnd_quit_restart_only_opens_confirmation_before_host_changes() {
    let mut manager = WindowManager::new();
    let key = NameKeyGenerator::name_to_key;
    let parent = manager
        .create_window_with_id(None, 0, 0, 200, 100, key("QuitMenu.wnd:") as i32)
        .unwrap();
    manager.bind_window_callbacks(
        &mut parent.borrow_mut(),
        &WindowDefinition {
            system_callback: "QuitMenuSystem".into(),
            ..Default::default()
        },
    );
    let id = key("QuitMenu.wnd:ButtonRestart") as i32;
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
    let mut operation = select(&mut manager, &button, true);
    assert!(
        operation.take().is_none(),
        "Restart creates only its receiving modal, never a host start/clear step before YES"
    );
    assert!(!parent.borrow().is_hidden());
}

#[test]
fn wnd_load_without_selection_preserves_visible_receiving_menu() {
    let mut manager = WindowManager::new();
    manager.configure_replay_menu_owned(crate::gui::shell::replay_menu::ReplayMenuInputs {
        replay_directory: std::path::PathBuf::from("explicit-empty-replay-fixture"),
        replay_extension: ".rep".into(),
        last_replay_filename: "00000000".into(),
        current_version: "fixture".into(),
        current_version_number: 1,
        current_exe_crc: 2,
        current_ini_crc: 3,
        metadata: crate::gui::shell::replay_menu::fixture_replay_metadata_policy(1, 1),
    });
    let key = NameKeyGenerator::name_to_key;
    let parent = manager
        .create_window_with_id(
            None,
            0,
            0,
            200,
            100,
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
    let mut operation = select(&mut manager, &button, false);
    assert!(
        operation.take().is_none(),
        "no selection creates only its receiving dialog, never playback"
    );
    assert!(!parent.borrow().is_hidden());
}
