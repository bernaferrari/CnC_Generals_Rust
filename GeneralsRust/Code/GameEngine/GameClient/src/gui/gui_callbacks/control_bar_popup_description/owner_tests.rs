use super::*;
use crate::gui::callbacks::control_bar_callbacks::ControlBarViewport;
use crate::gui::control_bar::ControlBar;
use crate::gui::game_window::WindowMessage;
use crate::gui::window_manager::ControlBarInputContext;
struct Viewport;
impl ControlBarViewport for Viewport {
    fn display_size(&self) -> (u32, u32) {
        (800, 600)
    }
    fn set_height_px(&mut self, _: i32) {}
}
#[test]
fn actual_owned_command_tooltip_hover_and_delete_use_receiving_bar_and_window_tree() {
    if let crate::test_process::TestProcess::ParentVerified = crate::test_process::run_bounded(
        module_path!(),
        "actual_owned_command_tooltip_hover_and_delete_use_receiving_bar_and_window_tree",
    ) {
        return;
    }
    let mut windows = WindowManager::new();
    let mut foreign = WindowManager::new();
    let button = windows.create_window(None, 10, 10, 90, 40).unwrap();
    let peer_button = foreign.create_window(None, 10, 10, 90, 40).unwrap();
    button.borrow_mut().set_id(42);
    peer_button.borrow_mut().set_id(42);
    button
        .borrow_mut()
        .set_control_bar_tooltip_callback(|_, _, _| {
            panic!("owned role must not invoke standalone tooltip adapter")
        });
    peer_button
        .borrow_mut()
        .set_control_bar_tooltip_callback(|_, _, _| {
            panic!("foreign tooltip callback must remain untouched")
        });
    let layout = Rc::new(RefCell::new(WindowLayout::new("ReceivingTooltip".into())));
    let peer_layout = Rc::new(RefCell::new(WindowLayout::new("PeerTooltip".into())));
    layout.borrow_mut().hide(true);
    peer_layout.borrow_mut().hide(false);
    let mut bar = ControlBar::new();
    let mut peer_bar = ControlBar::new();
    bar.tooltip.layout = Some(Rc::clone(&layout));
    peer_bar.tooltip.layout = Some(Rc::clone(&peer_layout));
    peer_bar.tooltip.prev_window_id = Some(99);
    peer_bar.tooltip.show_layout = true;
    button.borrow_mut().instance_data_mut().style = GWS_STATIC_TEXT;
    button.borrow_mut().instance_data_mut().tooltip_delay = 100;
    let mouse = Arc::new(Mutex::new(crate::input::Mouse::new()));
    let peer_mouse = Arc::new(Mutex::new(crate::input::Mouse::new()));
    peer_mouse
        .lock()
        .unwrap()
        .set_cursor_tooltip("Peer".into(), None, None, None);
    let generic = windows.create_window(None, 120, 10, 90, 40).unwrap();
    generic.borrow_mut().set_tooltip("Generic");
    let mut viewport = Viewport;
    {
        let mut context = ControlBarInputContext {
            script_display: None,
            window_videos: None,
            upgrade_tooltip: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
            replay_menu: None,
            replay_version: None,
            observer_roster: None,
            camera_options: None,
            control_bar: &mut bar,
            viewport: &mut viewport,
            replay: false,
            game_ending: false,
            tooltips_disabled: false,
            cursor_mouse: Some(&mouse),
            emitted_messages: None,

            main_menu: None,
        };
        windows.process_mouse_event_with_control_bar(
            WindowMessage::MousePos,
            130,
            20,
            0,
            &mut context,
        );
        assert_eq!(
            mouse.lock().unwrap().cursor_tooltip_state().tooltip_text,
            "Generic"
        );
        windows.process_mouse_event_with_control_bar(
            WindowMessage::MousePos,
            20,
            20,
            0,
            &mut context,
        );
    }
    assert!(
        mouse
            .lock()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text
            .is_empty(),
        "owned command hover clears prior receiving generic tooltip before waiting"
    );
    assert_eq!(bar.tooltip.prev_window_id, Some(42));
    assert!(bar.tooltip.wait_start.is_some());
    assert!(layout.borrow().is_hidden());
    assert_eq!(peer_bar.tooltip.prev_window_id, Some(99));
    assert_eq!(
        peer_mouse
            .lock()
            .unwrap()
            .cursor_tooltip_state()
            .tooltip_text,
        "Peer"
    );
    assert!(!peer_layout.borrow().is_hidden());
    // Deterministically cross the existing real-time delay without sleeping.
    // Repeated calls are the production admitted-frame ingress, not MousePos events.
    bar.tooltip.wait_start = Some(Instant::now() - Duration::from_millis(101));
    for _ in 0..4 {
        let mut context = ControlBarInputContext {
            script_display: None,
            window_videos: None,
            upgrade_tooltip: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
            replay_menu: None,
            replay_version: None,
            observer_roster: None,
            camera_options: None,
            control_bar: &mut bar,
            viewport: &mut viewport,
            replay: false,
            game_ending: false,
            tooltips_disabled: false,
            cursor_mouse: Some(&mouse),
            emitted_messages: None,

            main_menu: None,
        };
        windows.refresh_control_bar_tooltip(20, 20, &mut context);
        drop(context);
        bar.update_build_tooltip_layout_owned(false);
        assert!(bar.tooltip.wait_initialized);
        assert!(
            !layout.borrow().is_hidden(),
            "stationary admitted hover remains visible"
        );
        assert!(
            !bar.tooltip.show_layout,
            "ControlBar consumes the frame observation"
        );
        assert!(!peer_layout.borrow().is_hidden());
    }
    {
        let mut context = ControlBarInputContext {
            script_display: None,
            window_videos: None,
            upgrade_tooltip: crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
            replay_menu: None,
            replay_version: None,
            observer_roster: None,
            camera_options: None,
            control_bar: &mut bar,
            viewport: &mut viewport,
            replay: false,
            game_ending: false,
            tooltips_disabled: false,
            cursor_mouse: Some(&mouse),
            emitted_messages: None,

            main_menu: None,
        };
        windows.refresh_control_bar_tooltip(400, 400, &mut context);
    }
    bar.update_build_tooltip_layout_owned(false);
    assert!(
        layout.borrow().is_hidden(),
        "leaving command window ends only owner tooltip"
    );
    assert!(!peer_layout.borrow().is_hidden());
    layout.borrow_mut().hide(false);
    bar.tooltip.show_layout = true;
    bar.tooltip.prev_window_id = Some(42);
    let mut tooltip_animator = AnimateWindowManager::new();
    tooltip_animator.register_window(
        Rc::clone(&button),
        AnimationType::SlideRightFast,
        true,
        200,
        0,
    );
    tooltip_animator.reverse_animate_window();
    assert!(!tooltip_animator.is_empty());
    assert!(!tooltip_animator.is_finished());
    assert!(tooltip_animator.is_reversed());
    let animated_position = button.borrow().get_position();
    bar.tooltip.animate_manager = Some(tooltip_animator);
    crate::system::SubsystemInterface::reset(&mut bar).unwrap();
    assert!(layout.borrow().is_hidden());
    assert!(!bar.tooltip.show_layout);
    let retained_animator = bar.tooltip.animate_manager.as_ref().unwrap();
    assert!(!retained_animator.is_empty());
    assert!(!retained_animator.is_finished());
    assert!(retained_animator.is_reversed());
    assert_eq!(button.borrow().get_position(), animated_position);
    assert_eq!(
        bar.tooltip.prev_window_id,
        Some(42),
        "source reset retains wait/window identity"
    );
    assert!(!peer_layout.borrow().is_hidden());
    assert!(peer_bar.tooltip.show_layout);
    bar.delete_build_tooltip_layout_owned();
    assert_eq!(bar.tooltip.prev_window_id, None);
}
#[test]
fn generic_tooltip_callback_replacement_and_clear_remove_owned_role() {
    let mut window = GameWindow::new();
    window.set_control_bar_tooltip_callback(|_, _, _| {});
    assert!(window.has_owned_control_bar_tooltip());
    window.set_tooltip_callback(|_, _, _| {});
    assert!(!window.has_owned_control_bar_tooltip());
    window.set_control_bar_tooltip_callback(|_, _, _| {});
    window.clear_tooltip_callback();
    assert!(!window.has_owned_control_bar_tooltip());
    window.set_control_bar_tooltip_callback(|_, _, _| {});
    window.set_callbacks(None, None, None);
    assert!(!window.has_owned_control_bar_tooltip());
}
