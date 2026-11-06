//! Real layout-owned slider + selected WND callbacks; no fake Options event producer.
use super::*;
use crate::gui::callbacks::OptionsCameraInputObservation;
use crate::gui::gadgets::HorizontalSlider;
use game_engine::common::{
    ini::ini_game_data::CameraInputRules, user_preferences::UserPreferences,
};
fn options_wnd() -> (
    WindowManager,
    WindowLayout,
    Rc<RefCell<GameWindow>>,
    Rc<RefCell<GameWindow>>,
    Rc<RefCell<GameWindow>>,
    Rc<RefCell<GameWindow>>,
) {
    let mut manager = WindowManager::new();
    let key = |name: &str| NameKeyGenerator::name_to_key(name) as i32;
    let parent = manager
        .test_create_window_with_id(
            None,
            0,
            0,
            500,
            300,
            key("OptionsMenu.wnd:OptionsMenuParent"),
        )
        .unwrap();
    manager.bind_window_callbacks(
        &mut parent.borrow_mut(),
        &WindowDefinition {
            system_callback: "OptionsMenuSystem".into(),
            ..Default::default()
        },
    );
    let mut layout = WindowLayout::new("Menus/OptionsMenu.wnd".into());
    manager.bind_layout_callbacks(
        &mut layout,
        &crate::gui::window_script::WindowLayoutDefinition {
            init_callback: "OptionsMenuInit".into(),
            ..Default::default()
        },
    );
    layout.add_window(parent.clone());
    let mut button = |name: &str, x| {
        let id = key(name);
        let window = manager
            .test_create_window_with_id(Some(&parent), x, 10, 50, 24, id)
            .unwrap();
        window
            .borrow_mut()
            .set_widget(WindowWidget::PushButton(PushButton::new(
                id as u32, 0, 0, 50, 24,
            )));
        manager.bind_window_callbacks(
            &mut window.borrow_mut(),
            &WindowDefinition {
                input_callback: "GadgetPushButtonInput".into(),
                ..Default::default()
            },
        );
        layout.add_window(window.clone());
        window
    };
    let accept = button("OptionsMenu.wnd:ButtonAccept", 10);
    let defaults = button("OptionsMenu.wnd:ButtonDefaults", 80);
    let back = button("OptionsMenu.wnd:ButtonBack", 150);
    let id = key("OptionsMenu.wnd:SliderScrollSpeed");
    let slider = manager
        .test_create_window_with_id(Some(&parent), 10, 100, 200, 20, id)
        .unwrap();
    slider
        .borrow_mut()
        .set_widget(WindowWidget::HorizontalSlider(
            HorizontalSlider::new(id as u32, 0, 0, 200, 20)
                .with_range(0, 100)
                .with_value(99),
        ));
    layout.add_window(slider.clone());
    (manager, layout, accept, defaults, back, slider)
}
fn click_options(
    manager: &mut WindowManager,
    button: &Rc<RefCell<GameWindow>>,
    prefs: &mut UserPreferences,
    rules: CameraInputRules,
) {
    let (x, y) = button.borrow().get_screen_position();
    let (x, y) = (x + 5, y + 5);
    let data = (((y as u32) << 16) | ((x as u32) & 0xffff)) as WindowMsgData;
    let mut bar = ControlBar::new();
    let mut viewport = Viewport {
        width: 801,
        height: 603,
        view_height: 482,
        writes: Vec::new(),
    };
    let mut emitted_messages = Vec::new();
    // Accept/Defaults/Back complete through OptionsMenu + queued WND ops; park the
    // layout manager in a receiving owner and loan menus+ui for that boundary.
    let owner = crate::gui::window_manager::WindowManagerOwner::new();
    owner.with_manager(|owned| std::mem::swap(owned, manager));
    let mut menus = crate::gui::callbacks::menu_callbacks::MenuManager::new();
    let mut ui = crate::core::subsystems::InGameUISubsystem::default();
    let mut ime = crate::gui::ime_manager::ImeManager::new();
    let mut menu_host = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
    let mut script_display =
        crate::core::script_action_handler::ScriptDisplayReceiver::default();
    let mut main_menu = crate::gui::shell::main_menu::MainMenu::new();
    let mut context = ControlBarInputContext {
        control_bar_observation: None,
        diplomacy_world: None,
        menu_audio: None,
        input_provenance: crate::gui::control_bar::HostControlBarInputProvenance::InjectedOrUnknown,
        menus: Some(
            menus.receiving_menus(crate::core::game_client::ClientInputAuthority::NativeHosted),
        ),
        ime: Some(&mut ime),
        in_game_movies: None,
        script_display: Some(&mut script_display),
        window_videos: None,
        upgrade_tooltip:
            crate::gui::gui_callbacks::control_bar_popup_description::UpgradeTooltipSource::Standalone,
        replay_version: None,
        replay_menu: None,
        observer_roster: None,
        camera_options: Some(CameraOptionsInputContext::new(prefs, rules)),
        control_bar: &mut bar,
        viewport: &mut viewport,
        replay: false,
        game_ending: false,
        tooltips_disabled: false,
        cursor_mouse: None,
        emitted_messages: Some(&mut emitted_messages),
        campaign_manager: None,
        window_manager_owner: Some(&owner),
        client_frame: None,
        logic_frame: None,
        ui: Some(&mut ui),
        menu_world: Some(crate::gui::campaign_launch_host_bridge::MenuWorldFacts::default()),
        menu_host: Some(&mut menu_host),
        main_menu: Some(&mut main_menu),
    };
    // Fresh MenuManager still needs Options IDs + ignore_selected cleared so
    // Accept/Defaults apply against the live WND tree (init ran on a prior loan).
    {
        let mut messages = Vec::new();
        let mut bind = crate::gui::game_window::WindowEventEffects::new(&mut messages);
        bind.menus = context.menus.as_mut().map(|menus| menus.reborrow());
        crate::gui::callbacks::menu_callbacks::simulate_options_menu_bind_controls(&mut bind);
    }
    for message in [WindowMessage::LeftDown, WindowMessage::LeftUp] {
        owner.with_manager(|manager| {
            assert_eq!(
                manager.process_mouse_event_with_control_bar(message, x, y, data, &mut context),
                WindowInputReturnCode::Used,
                "{message:?} for control {} at ({x}, {y}), hidden={}, enabled={}",
                button.borrow().get_id(),
                button.borrow().is_hidden(),
                button.borrow().is_enabled()
            );
            // Same completion boundary as the actual OS dispatch, after manager/widget loans end.
            context.complete_options_accept(manager);
        });
    }
    {
        let mut messages = Vec::new();
        let mut drain = crate::gui::game_window::WindowEventEffects::new(&mut messages);
        drain.window_manager_owner = Some(&owner);
        owner.complete_event_callbacks(&mut drain);
    }
    owner.with_manager(|owned| std::mem::swap(owned, manager));
}
fn value(slider: &Rc<RefCell<GameWindow>>) -> i32 {
    match slider.borrow().widget() {
        Some(WindowWidget::HorizontalSlider(slider)) => slider.value(),
        _ => panic!("real authored slider missing"),
    }
}

fn run_options_init(
    manager: &mut WindowManager,
    layout: &WindowLayout,
    observation: &OptionsCameraInputObservation,
) {
    // OptionsMenuInit only binds init_callback_with_effects; bare run_init is a
    // no-op. Init queues focus through the receiving owner, so temporarily park
    // the layout's WindowManager inside a WindowManagerOwner for the call.
    let owner = crate::gui::window_manager::WindowManagerOwner::new();
    owner.with_manager(|owned| std::mem::swap(owned, manager));
    let mut menus = crate::gui::callbacks::menu_callbacks::MenuManager::new();
    let mut messages = Vec::new();
    let mut ui = crate::core::subsystems::InGameUISubsystem::default();
    let mut effects = crate::gui::game_window::WindowEventEffects::new(&mut messages);
    effects.window_manager_owner = Some(&owner);
    effects.menus = Some(
        menus.receiving_menus(crate::core::game_client::ClientInputAuthority::NativeHosted),
    );
    effects.ui = Some(&mut ui);
    layout.run_init_with_effects(Some(observation as &dyn std::any::Any), &mut effects);
    owner.complete_event_callbacks(&mut effects);
    owner.with_manager(|owned| std::mem::swap(owned, manager));
}
#[test]
#[cfg(not(target_arch = "wasm32"))]
fn receiving_options_layout_apply_defaults_and_cancel_keep_equal_id_peer_isolated() {
    crate::test_support::ensure_ambient_host();
    const NAME: &str =
        "receiving_options_layout_apply_defaults_and_cancel_keep_equal_id_peer_isolated";
    const MARKER: &str = "GENERALS_RECEIVING_CAMERA_OPTIONS_CHILD";
    if std::env::var(MARKER).as_deref() != Ok(NAME) {
        let module = module_path!().split_once("::").unwrap().1;
        let name = format!("{module}::{NAME}");
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &name, "--test-threads=1", "--nocapture"])
            .env(MARKER, NAME)
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("running 1 test"),
            "child must execute exactly one real regression"
        );
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    // Options first-open also exercises the legacy LOD recommendation writer.
    // Its existing user-data owner must stay inside this already isolated child;
    // changing cwd would also change unrelated optional asset discovery.
    let legacy_user_data = directory.path().join("legacy-lod-user-data");
    game_engine::common::ini::ini_game_data::ensure_global_data()
        .write()
        .set_path_user_data(legacy_user_data.to_str().unwrap().to_string());
    let path_a = directory.path().join("A.ini");
    let path_b = directory.path().join("B.ini");
    std::fs::write(&path_a, "MusicVolume = 71\n").unwrap();
    std::fs::write(&path_b, "ScrollFactor = 25\n").unwrap();
    let (mut prefs_a, mut prefs_b) = (UserPreferences::new(), UserPreferences::new());
    let _ = prefs_a.load(path_a.to_str().unwrap());
    let _ = prefs_b.load(path_b.to_str().unwrap());
    let (mut a, layout_a, accept_a, defaults_a, back_a, slider_a) = options_wnd();
    let (mut b, layout_b, _accept_b, _defaults_b, _back_b, slider_b) = options_wnd();
    assert_eq!(slider_a.borrow().get_id(), slider_b.borrow().get_id());
    let rules = CameraInputRules {
        keyboard_default_scroll_factor: 0.675,
        ..Default::default()
    };
    {
        let mut global = game_engine::common::global_data::write();
        global.keyboard_scroll_factor = 0.99;
        global.keyboard_default_scroll_factor = 0.01;
    }
    let observation = OptionsCameraInputObservation::observe(rules, &prefs_a);
    run_options_init(&mut a, &layout_a, &observation);
    assert_eq!(value(&slider_a), 67);
    assert_eq!(rules.keyboard_scroll_factor(&prefs_a), 0.675);
    let peer = OptionsCameraInputObservation::observe(rules, &prefs_b);
    run_options_init(&mut b, &layout_b, &peer);
    assert_eq!(value(&slider_b), 25);
    assert_eq!(value(&slider_a), 67);
    slider_a
        .borrow_mut()
        .horizontal_slider_mut()
        .unwrap()
        .set_value(80);
    click_options(&mut a, &accept_a, &mut prefs_a, rules);
    assert_eq!(prefs_a.get_int("ScrollFactor"), Some(80));
    assert_eq!(rules.keyboard_scroll_factor(&prefs_a), 0.8);
    assert_eq!(prefs_b.get_int("ScrollFactor"), Some(25));
    assert_eq!(value(&slider_b), 25);
    assert!(
        std::fs::read_to_string(&path_a)
            .unwrap()
            .contains("ScrollFactor = 80")
    );
    assert_eq!(
        std::fs::read_to_string(&path_b).unwrap(),
        "ScrollFactor = 25\n"
    );
    let new_default = CameraInputRules {
        keyboard_default_scroll_factor: 0.375,
        ..rules
    };
    let reopened = OptionsCameraInputObservation::observe(new_default, &prefs_a);
    run_options_init(&mut a, &layout_a, &reopened);
    click_options(&mut a, &defaults_a, &mut prefs_a, new_default);
    assert_eq!(value(&slider_a), 37);
    assert_eq!(prefs_a.get_int("ScrollFactor"), Some(80));
    assert_eq!(new_default.keyboard_scroll_factor(&prefs_a), 0.8);
    click_options(&mut a, &back_a, &mut prefs_a, new_default);
    assert_eq!(prefs_a.get_int("ScrollFactor"), Some(80));
    assert_eq!(value(&slider_b), 25);
    assert_eq!(
        game_engine::common::global_data::read().keyboard_scroll_factor,
        0.99,
        "owned Apply cannot replace another adapter's runtime scroll value"
    );
    let legacy_options = std::fs::read_to_string(legacy_user_data.join("Options.ini")).unwrap();
    assert!(
        legacy_options
            .lines()
            .any(|line| line.starts_with("IdealStaticGameLOD = ")),
        "actual first-open LOD persistence must be contained in the child temp directory"
    );
    assert!(
        legacy_options
            .lines()
            .any(|line| line.starts_with("StaticGameLOD = "))
    );
}

#[test]
fn actual_quit_and_no_save_options_buttons_select_borrowed_camera_admission() {
    crate::test_support::ensure_ambient_host();
    for prefix in ["QuitMenu.wnd", "QuitNoSave.wnd"] {
        let mut manager = WindowManager::new();
        let key = |name: &str| NameKeyGenerator::name_to_key(name) as i32;
        let parent = manager
            .test_create_window_with_id(None, 0, 0, 500, 300, key(&format!("{prefix}:Parent")))
            .unwrap();
        manager.bind_window_callbacks(
            &mut parent.borrow_mut(),
            &WindowDefinition {
                system_callback: "QuitMenuSystem".into(),
                ..Default::default()
            },
        );
        let id = key(&format!("{prefix}:ButtonOptions"));
        let button = manager
            .test_create_window_with_id(Some(&parent), 0, 0, 50, 24, id)
            .unwrap();
        button
            .borrow_mut()
            .set_widget(WindowWidget::PushButton(PushButton::new(
                id as u32, 0, 0, 50, 24,
            )));
        manager.bind_window_callbacks(
            &mut button.borrow_mut(),
            &WindowDefinition {
                input_callback: "GadgetPushButtonInput".into(),
                ..Default::default()
            },
        );
        button.borrow_mut().set_cursor_position(5, 5).unwrap();
        let (_, down) = crate::gui::game_window::WindowEventEffects::with_standalone(|effects| {
            button.borrow_mut().send_input_with_control_bar_notification(
                WindowMessage::LeftDown,
                0,
                0,
                true,
                false,
                false,
                effects,
                None,
            )
        });
        assert!(down.is_none());
        let (_, up) = crate::gui::game_window::WindowEventEffects::with_standalone(|effects| {
            button.borrow_mut().send_input_with_control_bar_notification(
                WindowMessage::LeftUp,
                0,
                0,
                true,
                false,
                false,
                effects,
                None,
            )
        });
        let up = up.expect("actual push button must produce the selected pause Options action");
        assert!(matches!(up.action, SelectedOwnerAction::ShowQuitOptions));
        assert_eq!(up.control_id, id as WindowMsgData);
        assert!(Rc::ptr_eq(&up.owner, &parent));
    }
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn receiving_replay_checkbox_actual_wnd_commits_only_own_store_before_accept() {
    crate::test_support::ensure_ambient_host();
    const NAME: &str = "receiving_replay_checkbox_actual_wnd_commits_only_own_store_before_accept";
    const MARKER: &str = "GENERALS_RECEIVING_REPLAY_OPTIONS_CHILD";
    if std::env::var(MARKER).as_deref() != Ok(NAME) {
        let module = module_path!().split_once("::").unwrap().1;
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                &format!("{module}::{NAME}"),
                "--test-threads=1",
                "--nocapture",
            ])
            .env(MARKER, NAME)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&output.stdout).contains("running 1 test"));
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let directory = tempfile::tempdir().unwrap();
    game_engine::common::ini::ini_game_data::ensure_global_data()
        .write()
        .set_path_user_data(
            directory
                .path()
                .join("legacy")
                .to_string_lossy()
                .into_owned(),
        );
    let path = directory.path().join("A.ini");
    std::fs::write(
        &path,
        "UseCameraInReplays = yes\nSaveCameraInReplays = yes\n",
    )
    .unwrap();
    let mut owner = UserPreferences::new();
    owner.load(path.to_str().unwrap());
    let mut peer = UserPreferences::new();
    peer.set_string("UseCameraInReplays", "yes".into());
    let (mut manager, mut layout, accept, defaults, back, _) = options_wnd();
    // Layout insertion order is not hierarchy: attach to the actual Options parent.
    let parent = accept.borrow().get_parent().unwrap();
    let id = NameKeyGenerator::name_to_key("OptionsMenu.wnd:CheckBoxUseCamera") as i32;
    let checkbox = manager
        .test_create_window_with_id(Some(&parent), 10, 180, 30, 30, id)
        .unwrap();
    checkbox
        .borrow_mut()
        .set_widget(WindowWidget::CheckBox(crate::gui::gadgets::CheckBox::new(
            id as u32, 0, 0, 24,
        )));
    manager.bind_window_callbacks(
        &mut checkbox.borrow_mut(),
        &WindowDefinition {
            input_callback: "GadgetCheckBoxInput".into(),
            ..Default::default()
        },
    );
    // Authored OptionsMenu.wnd uses CHECKBOX+MOUSETRACK; entering arms release.
    checkbox.borrow_mut().instance_data_mut().style |= GWS_CHECK_BOX | GWS_MOUSE_TRACK;
    layout.add_window(checkbox.clone());
    let rules = CameraInputRules::default();
    let observation = OptionsCameraInputObservation::observe(rules, &owner);
    run_options_init(&mut manager, &layout, &observation);
    assert!(checkbox.borrow_mut().check_box_mut().unwrap().is_checked());
    game_engine::common::global_data::write().use_camera_in_replay = true;
    click_options(&mut manager, &checkbox, &mut owner, rules);
    assert!(
        !owner.use_camera_in_replays(),
        "actual WND checkbox must commit before Accept"
    );
    assert!(peer.use_camera_in_replays());
    assert!(game_engine::common::global_data::read().use_camera_in_replay);
    let disk = std::fs::read(&path).unwrap();
    click_options(&mut manager, &defaults, &mut owner, rules);
    assert!(!owner.use_camera_in_replays());
    click_options(&mut manager, &back, &mut owner, rules);
    assert!(!owner.use_camera_in_replays());
    assert_eq!(std::fs::read(&path).unwrap(), disk);
    // Reuse the real kernel after Back destroys its layout, without pretending
    // a destroyed button is a fresh input: source shared-store Apply is covered
    // separately by Main's actual custom Apply test.
    let _ = accept;
}
