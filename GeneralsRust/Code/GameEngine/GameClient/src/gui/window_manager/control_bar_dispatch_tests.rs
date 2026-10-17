//! Actual WND callback binding + hit-tested gadget input, with borrowed owners.
#[path = "camera_options_input_tests.rs"]
mod camera_options_input_tests;
use super::*;
use crate::gui::callbacks::control_bar_callbacks::ControlBarViewport;
use crate::gui::control_bar::{ControlBar, ControlBarStage};
use crate::gui::gadgets::PushButton;
use crate::gui::game_window::*;
use crate::gui::window_script::WindowDefinition;
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;

struct Viewport {
    width: u32,
    height: u32,
    view_height: i32,
    writes: Vec<i32>,
}
impl ControlBarViewport for Viewport {
    fn display_size(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    fn set_height_px(&mut self, height: i32) {
        self.view_height = height;
        self.writes.push(height);
    }
}
fn wnd() -> (
    WindowManager,
    Rc<RefCell<GameWindow>>,
    Rc<RefCell<GameWindow>>,
) {
    let mut manager = WindowManager::new();
    let parent_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ControlBarParent") as i32;
    let button_id = NameKeyGenerator::name_to_key("ControlBar.wnd:ButtonLarge") as i32;
    let parent = manager
        .create_window_with_id(None, 0, 100, 200, 100, parent_id)
        .unwrap();
    manager.bind_window_callbacks(
        &mut parent.borrow_mut(),
        &WindowDefinition {
            system_callback: "ControlBarSystem".into(),
            ..Default::default()
        },
    );
    let button = manager
        .create_window_with_id(Some(&parent), 10, 10, 40, 20, button_id)
        .unwrap();
    button
        .borrow_mut()
        .set_widget(WindowWidget::PushButton(PushButton::new(
            button_id as u32,
            0,
            0,
            40,
            20,
        )));
    manager.bind_window_callbacks(
        &mut button.borrow_mut(),
        &WindowDefinition {
            input_callback: "GadgetPushButtonInput".into(),
            ..Default::default()
        },
    );
    (manager, parent, button)
}
fn click(
    manager: &mut WindowManager,
    button: &Rc<RefCell<GameWindow>>,
    bar: &mut ControlBar,
    viewport: &mut Viewport,
) {
    let (x, y) = button.borrow().get_screen_position();
    let (x, y) = (x + 5, y + 5);
    let data = (((y as u32) << 16) | ((x as u32) & 0xffff)) as WindowMsgData;
    let mut context = ControlBarInputContext {
        camera_options: None,
        control_bar: bar,
        viewport,
        replay: false,
        game_ending: false,
        tooltips_disabled: false,
        cursor_mouse: None,
    };
    for message in [WindowMessage::LeftDown, WindowMessage::LeftUp] {
        assert_eq!(
            manager.process_mouse_event_with_control_bar(message, x, y, data, &mut context),
            WindowInputReturnCode::Used
        );
    }
}
#[test]
fn wnd_large_button_synchronously_changes_only_receiving_bar_before_update() {
    let (mut a, parent_a, button_a) = wnd();
    let (mut b, parent_b, button_b) = wnd();
    assert_eq!(button_a.borrow().get_id(), button_b.borrow().get_id());
    let (mut bar_a, mut bar_b) = (ControlBar::new(), ControlBar::new());
    let mut view_a = Viewport {
        width: 801,
        height: 603,
        view_height: 482,
        writes: Vec::new(),
    };
    let mut view_b = Viewport {
        width: 1001,
        height: 751,
        view_height: 600,
        writes: Vec::new(),
    };
    assert_eq!(bar_a.get_control_bar_stage(), ControlBarStage::Default);
    click(&mut a, &button_a, &mut bar_a, &mut view_a);
    assert_eq!(bar_a.get_control_bar_stage(), ControlBarStage::Low);
    assert_eq!(view_a.view_height, 603);
    assert_eq!(parent_a.borrow().get_position(), (0, 542));
    assert_eq!(view_a.writes, vec![603]);
    assert_eq!(bar_b.get_control_bar_stage(), ControlBarStage::Default);
    assert_eq!(view_b.view_height, 600);
    assert!(view_b.writes.is_empty());
    assert_eq!(parent_b.borrow().get_position(), (0, 100));
    click(&mut a, &button_a, &mut bar_a, &mut view_a);
    assert_eq!(bar_a.get_control_bar_stage(), ControlBarStage::Default);
    assert_eq!(view_a.view_height, 482);
    assert_eq!(parent_a.borrow().get_position(), (0, 100));
    assert_eq!(
        view_a.writes,
        vec![603, 482],
        "each actual selected notification is delivered once before input returns"
    );
    click(&mut b, &button_b, &mut bar_b, &mut view_b);
    assert_eq!(bar_b.get_control_bar_stage(), ControlBarStage::Low);
    assert_eq!(view_b.view_height, 751);
    assert_eq!(view_a.writes, vec![603, 482]);
    // Successful explicit stage completion reborrowed the actual sender after
    // its gadget RefMut ended; parent and child remain independently usable.
    assert!(button_a.try_borrow_mut().is_ok());
    assert!(parent_a.try_borrow_mut().is_ok());
}
#[test]
fn wnd_large_button_keyboard_and_ending_replay_guards_use_receiving_context() {
    let (mut manager, parent, button) = wnd();
    let mut bar = ControlBar::new();
    let mut viewport = Viewport {
        width: 801,
        height: 603,
        view_height: 482,
        writes: Vec::new(),
    };
    manager.set_focus(Some(&button)).unwrap();
    {
        let mut context = ControlBarInputContext {
            camera_options: None,
            control_bar: &mut bar,
            viewport: &mut viewport,
            replay: false,
            game_ending: true,
            tooltips_disabled: false,
            cursor_mouse: None,
        };
        for state in [0x02, 0x01] {
            let _ = manager.process_key_event_with_control_bar(0x20, state, &mut context);
        }
    }
    assert!(viewport.writes.is_empty());
    assert_eq!(bar.get_control_bar_stage(), ControlBarStage::Default);
    {
        let mut context = ControlBarInputContext {
            camera_options: None,
            control_bar: &mut bar,
            viewport: &mut viewport,
            replay: true,
            game_ending: false,
            tooltips_disabled: false,
            cursor_mouse: None,
        };
        for state in [0x02, 0x01] {
            let _ = manager.process_key_event_with_control_bar(0x20, state, &mut context);
        }
    }
    assert!(viewport.writes.is_empty());
    {
        let mut context = ControlBarInputContext {
            camera_options: None,
            control_bar: &mut bar,
            viewport: &mut viewport,
            replay: false,
            game_ending: false,
            tooltips_disabled: false,
            cursor_mouse: None,
        };
        for state in [0x02, 0x01] {
            let _ = manager.process_key_event_with_control_bar(0x20, state, &mut context);
        }
    }
    assert_eq!(viewport.writes, vec![603]);
    assert_eq!(bar.get_control_bar_stage(), ControlBarStage::Low);
    assert_eq!(parent.borrow().get_position(), (0, 542));
}

#[test]
fn wnd_large_button_captures_authored_marker_with_borrowed_offset_parent() {
    let schemes = game_engine::common::ini::ensure_control_bar_scheme_manager();
    schemes.write().clear();
    let (mut manager, parent, button) = wnd();
    parent.borrow_mut().set_position(23, 117).unwrap();
    let marker_id = NameKeyGenerator::name_to_key("ControlBar.wnd:BackgroundMarker") as i32;
    let marker = manager
        .create_window_with_id(Some(&parent), 6, 144, 4, 4, marker_id)
        .unwrap();
    assert_eq!(marker.borrow().get_screen_position(), (29, 261));
    let mut bar = ControlBar::new();
    let mut viewport = Viewport {
        width: 801,
        height: 603,
        view_height: 482,
        writes: Vec::new(),
    };
    click(&mut manager, &button, &mut bar, &mut viewport);
    assert_eq!(parent.borrow().get_position(), (23, 542));
    assert_eq!(marker.borrow().get_screen_position(), (29, 686));
    {
        let scheme = schemes.read();
        assert!(scheme.marker_base_captured());
        let background = scheme.get_background_marker_pos();
        let foreground = scheme.get_foreground_marker_pos();
        assert_eq!((background.x, background.y), (29, 261));
        assert_eq!((foreground.x, foreground.y), (29, 261));
    }
    click(&mut manager, &button, &mut bar, &mut viewport);
    assert_eq!(parent.borrow().get_position(), (23, 117));
    assert_eq!(marker.borrow().get_screen_position(), (29, 261));
    let base = schemes.read().get_background_marker_pos();
    assert_eq!((base.x, base.y), (29, 261));
}

#[test]
fn wnd_restoring_default_applies_receiving_context_visibility_before_parent_borrow() {
    let (mut manager, parent, button) = wnd();
    let (mut peer, _, _) = wnd();
    let mut windows = Vec::new();
    for (name, initially_hidden, final_hidden) in [
        ("CommandWindow", true, false),
        ("UnderConstructionWindow", false, true),
        ("OCLTimerWindow", false, true),
        ("ObserverPlayerListWindow", false, true),
        ("ObserverPlayerInfoWindow", false, true),
        ("ButtonCommand01", true, false),
        ("ButtonCommand14", true, false),
    ] {
        let full_name = format!("ControlBar.wnd:{name}");
        let id = NameKeyGenerator::name_to_key(&full_name) as i32;
        let actual = manager
            .create_window_with_id(Some(&parent), 0, 0, 1, 1, id)
            .unwrap();
        actual.borrow_mut().set_name(&full_name);
        actual.borrow_mut().hide(initially_hidden).unwrap();
        let foreign = peer.create_window_with_id(None, 0, 0, 1, 1, id).unwrap();
        foreign.borrow_mut().set_name(&full_name);
        foreign.borrow_mut().hide(initially_hidden).unwrap();
        windows.push((actual, foreign, initially_hidden, final_hidden));
    }
    let mut bar = ControlBar::new();
    let mut viewport = Viewport {
        width: 801,
        height: 603,
        view_height: 482,
        writes: Vec::new(),
    };
    click(&mut manager, &button, &mut bar, &mut viewport);
    for (actual, _, initial, _) in &windows {
        assert_eq!(actual.borrow().is_hidden(), *initial);
    }
    click(&mut manager, &button, &mut bar, &mut viewport);
    assert_eq!(bar.get_control_bar_stage(), ControlBarStage::Default);
    for (actual, foreign, initial, final_hidden) in windows {
        assert_eq!(actual.borrow().is_hidden(), final_hidden);
        assert_eq!(foreign.borrow().is_hidden(), initial);
    }
}
