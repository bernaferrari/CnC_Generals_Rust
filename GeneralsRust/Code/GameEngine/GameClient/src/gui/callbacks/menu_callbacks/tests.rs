use crate::test_support::with_receiving_ui;

use super::*;

fn with_global_data_residences_restored(f: impl FnOnce()) {
    runtime_global_data::with_global_data_restored(|| {
        let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
        let ini_snapshot = ini_global.read().clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
        *ini_global.write() = ini_snapshot;
        if let Err(payload) = result {
            std::panic::resume_unwind(payload);
        }
    });
}

#[test]
fn test_main_menu_lifecycle() {
    crate::test_support::ensure_ambient_host();
    let mut main_menu = MainMenu::new();
    let layout = WindowLayout::new("TestLayout".to_string());

    // Test initialization
    assert!(
        WindowEventEffects::with_standalone(|effects| main_menu.init(&layout, None, effects))
            .is_ok()
    );
    assert!(main_menu.initialized);

    // Test update
    assert!(
        WindowEventEffects::with_standalone(|effects| main_menu.update(&layout, None, effects))
            .is_ok()
    );

    // Test shutdown
    assert!(
        WindowEventEffects::with_standalone(|effects| main_menu.shutdown(&layout, None, effects))
            .is_ok()
    );
    assert!(!main_menu.initialized);
}

#[test]
fn test_menu_manager() {
    crate::test_support::ensure_ambient_host();
    let mut manager = MenuManager::new();

    // Test that all menus are accessible as owned fields
    let _ = manager.main_menu_mut();
    let _ = manager.single_player_menu_mut();
    let _ = manager.options_menu_mut();
    let _ = manager.map_select_menu_mut();
    let _ = manager.credits_menu_mut();
    let _ = manager.lan_lobby_menu_mut();
}

#[test]
fn menu_recipients_borrow_the_same_owned_fields_and_leave_peers_independent() {
    crate::test_support::ensure_ambient_host();
    let mut owner = MenuManager::new();
    let mut peer = MenuManager::new();
    let expected = std::ptr::from_mut(owner.options_menu_mut());
    {
        let mut recipients =
            owner.receiving_menus(crate::core::game_client::ClientInputAuthority::NativeHosted);
        assert_eq!(
            std::ptr::from_mut(recipients.options.as_deref_mut().unwrap()),
            expected
        );
        let mut child = recipients.reborrow();
        child.options.as_deref_mut().unwrap().initialized = true;
        assert!(!peer.options_menu_mut().initialized);
    }
    assert!(owner.options_menu_mut().initialized);
}

#[test]
fn options_menu_esc_char_is_consumed_before_key_up_like_cpp() {
    crate::test_support::ensure_ambient_host();
    let mut menu = OptionsMenu::new();
    let mut window = GameWindow::new();

    assert_eq!(
        WindowEventEffects::with_standalone(|effects| menu.input(
            &mut window,
            WindowMessage::Char,
            0x1B,
            0,
            effects
        )),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        WindowEventEffects::with_standalone(|effects| menu.input(
            &mut window,
            WindowMessage::Char,
            0x1B,
            0x0001,
            effects
        )),
        WindowMsgHandled::Handled
    );
    assert_eq!(
        WindowEventEffects::with_standalone(|effects| menu.input(
            &mut window,
            WindowMessage::Char,
            b'A' as WindowMsgData,
            0,
            effects
        )),
        WindowMsgHandled::Ignored
    );
}

#[test]
fn options_sfx_slider_splits_2d_and_3d_volume_like_cpp() {
    crate::test_support::ensure_ambient_host();
    fn assert_split(relative: f32, expected_2d: i32, expected_3d: i32) {
        let (volume_2d, volume_3d, factor_2d, factor_3d) =
            OptionsMenu::split_sfx_volume_for_relative(80, relative);
        assert_eq!((volume_2d, volume_3d), (expected_2d, expected_3d));
        assert!((factor_2d - expected_2d as f32 / 100.0).abs() < 0.001);
        assert!((factor_3d - expected_3d as f32 / 100.0).abs() < 0.001);
    }

    assert_split(-0.25, 60, 80);
    assert_split(0.0, 80, 80);
    assert_split(0.25, 80, 60);
    assert_split(2.0, 80, 0);
}

#[test]
fn options_defaults_resolution_index_prefers_800x600_like_cpp() {
    crate::test_support::ensure_ambient_host();
    let mut menu = OptionsMenu::new();
    menu.resolution_modes = vec![(1024, 768), (800, 600), (1920, 1080)];
    assert_eq!(menu.default_resolution_index(), 1);

    menu.resolution_modes = vec![(1024, 768), (1280, 720)];
    assert_eq!(menu.default_resolution_index(), 0);
}

#[test]
fn options_checkbox_immediate_effects_match_cpp() {
    crate::test_support::ensure_ambient_host();
    with_receiving_ui(|ui| {
        let original_draw_anchor = ui.get_draw_rmb_scroll_anchor();
        let original_move_anchor = ui.get_move_rmb_scroll_anchor();
        let (original_save_camera, original_use_camera) = {
            let global = runtime_global_data::read();
            (global.save_camera_in_replay, global.use_camera_in_replay)
        };

        let mut messages = Vec::new();
        let mut mailbox = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.menu_host = Some(&mut mailbox);
        effects.ui = Some(ui);
        let mut menu = OptionsMenu::new();
        menu.check_draw_anchor_id = 11;
        menu.check_move_anchor_id = 12;
        menu.check_save_camera_id = 13;
        menu.check_use_camera_id = 14;

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_draw_anchor_id,
            true,
            &mut effects
        ));
        assert!(effects.ui.as_ref().unwrap().get_draw_rmb_scroll_anchor());

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_move_anchor_id,
            true,
            &mut effects
        ));
        assert!(effects.ui.as_ref().unwrap().get_move_rmb_scroll_anchor());

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_save_camera_id,
            false,
            &mut effects
        ));
        assert!(!runtime_global_data::read().save_camera_in_replay);

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_use_camera_id,
            false,
            &mut effects
        ));
        assert!(!runtime_global_data::read().use_camera_in_replay);

        assert!(!menu.apply_immediate_checkbox_effect(99, true, &mut effects));

        effects
            .ui
            .as_mut()
            .unwrap()
            .set_draw_rmb_scroll_anchor(original_draw_anchor);
        effects
            .ui
            .as_mut()
            .unwrap()
            .set_move_rmb_scroll_anchor(original_move_anchor);
        let mut global = runtime_global_data::write();
        global.save_camera_in_replay = original_save_camera;
        global.use_camera_in_replay = original_use_camera;
    });
}

#[test]
fn options_move_rmb_scroll_anchor_publishes_only_to_an_enabled_host() {
    crate::test_support::ensure_ambient_host();
    with_receiving_ui(|ui| {
        let original_move_anchor = ui.get_move_rmb_scroll_anchor();

        use crate::gui::options_host_bridge::HostOptionsRequest;

        let mut messages = Vec::new();
        let mut mailbox = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.menu_host = Some(&mut mailbox);
        effects.ui = Some(ui);
        let mut menu = OptionsMenu::new();
        menu.check_move_anchor_id = 12;

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_move_anchor_id,
            true,
            &mut effects
        ));
        assert_eq!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default(),
            vec![HostOptionsRequest::MoveRmbScrollAnchor { enabled: true }]
        );
        assert!(effects.ui.as_ref().unwrap().get_move_rmb_scroll_anchor());

        effects.menu_host = None;
        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_move_anchor_id,
            false,
            &mut effects
        ));
        assert!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default()
                .is_empty()
        );
        assert!(
            !effects.ui.as_ref().unwrap().get_move_rmb_scroll_anchor(),
            "disabled host delivery must retain the standalone legacy callback"
        );

        effects
            .ui
            .as_mut()
            .unwrap()
            .set_move_rmb_scroll_anchor(original_move_anchor);
    });
}

#[test]
fn options_draw_rmb_scroll_anchor_publishes_only_to_an_enabled_host() {
    crate::test_support::ensure_ambient_host();
    with_receiving_ui(|ui| {
        let original_draw_anchor = ui.get_draw_rmb_scroll_anchor();

        use crate::gui::options_host_bridge::HostOptionsRequest;

        let mut messages = Vec::new();
        let mut mailbox = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.menu_host = Some(&mut mailbox);
        effects.ui = Some(ui);
        let mut menu = OptionsMenu::new();
        menu.check_draw_anchor_id = 11;

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_draw_anchor_id,
            true,
            &mut effects
        ));
        assert_eq!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default(),
            vec![HostOptionsRequest::DrawRmbScrollAnchor { enabled: true }]
        );
        assert!(effects.ui.as_ref().unwrap().get_draw_rmb_scroll_anchor());

        effects.menu_host = None;
        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_draw_anchor_id,
            false,
            &mut effects
        ));
        assert!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default()
                .is_empty()
        );
        assert!(
            !effects.ui.as_ref().unwrap().get_draw_rmb_scroll_anchor(),
            "disabled host delivery must retain the standalone legacy callback"
        );

        effects
            .ui
            .as_mut()
            .unwrap()
            .set_draw_rmb_scroll_anchor(original_draw_anchor);
    });
}

#[test]
fn options_accept_converges_alternate_mouse_residences_and_publishes_to_main() {
    crate::test_support::ensure_ambient_host();
    use crate::gui::options_host_bridge::HostOptionsRequest;

    let mut messages = Vec::new();
    let mut mailbox = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
    let mut effects = WindowEventEffects::new(&mut messages);
    effects.menu_host = Some(&mut mailbox);

    with_global_data_residences_restored(|| {
        let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
        ini_global.write().use_alternate_mouse = false;
        runtime_global_data::write().use_alternate_mouse = true;

        OptionsMenu::commit_alternate_mouse_setting(
            true,
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| &mut host.options),
        );
        assert!(ini_global.read().use_alternate_mouse);
        assert!(runtime_global_data::read().use_alternate_mouse);
        assert_eq!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default(),
            vec![HostOptionsRequest::AlternateMouse { enabled: true }]
        );

        OptionsMenu::commit_alternate_mouse_setting(
            false,
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| &mut host.options),
        );
        assert!(!ini_global.read().use_alternate_mouse);
        assert!(!runtime_global_data::read().use_alternate_mouse);
        assert_eq!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default(),
            vec![HostOptionsRequest::AlternateMouse { enabled: false }]
        );
    });
}

#[test]
fn map_select_double_click_uses_event_row_selection_like_cpp() {
    crate::test_support::ensure_ambient_host();
    let mut menu = MapSelectMenu::new();
    menu.listbox_map_id = 42;
    menu.button_ok_id = 77;

    let mut listbox = crate::gui::gadgets::ListBox::new(42, 0, 0, 200, 80);
    listbox.add_item_with_data(
        0,
        "first",
        Some(ListBoxItemData::Text("Maps\\First\\First.map".to_string())),
    );
    listbox.add_item_with_data(
        1,
        "second",
        Some(ListBoxItemData::Text(
            "Maps\\Second\\Second.map".to_string(),
        )),
    );
    listbox.set_selected_indices(&[1]);

    let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
    listbox_window
        .borrow_mut()
        .set_widget(WindowWidget::ListBox(listbox));
    menu.listbox_map = Some(listbox_window.clone());

    let mut window = GameWindow::new();

    assert_eq!(
        WindowEventEffects::with_standalone(|effects| menu.system(
            &mut window,
            WindowMessage::User(GLM_DOUBLE_CLICKED),
            42,
            0,
            effects
        )),
        WindowMsgHandled::Handled
    );

    assert_eq!(menu.selected_map.as_deref(), Some("Maps\\First\\First.map"));
    let selected = listbox_window
        .borrow()
        .widget()
        .and_then(|widget| match widget {
            WindowWidget::ListBox(listbox) => listbox.selected_indices().first().copied(),
            _ => None,
        });
    assert_eq!(selected, Some(0));
}

#[test]
fn map_select_start_game_mirrors_selected_map_to_both_global_data_residences() {
    crate::test_support::ensure_ambient_host();
    with_global_data_residences_restored(|| {
        let selected_map = "Maps\\Official\\MapSelectExact.map".to_string();
        let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
        ini_global.write().pending_file = "Maps\\Legacy\\Ini.map".to_string();
        runtime_global_data::write().pending_file = "Maps\\Legacy\\Runtime.map".to_string();

        let mut menu = MapSelectMenu::new();
        menu.selected_map = Some(selected_map.clone());
        WindowEventEffects::with_standalone(|effects| menu.start_game(effects));

        assert!(menu.start_game);
        assert_eq!(ini_global.read().pending_file, selected_map);
        assert_eq!(runtime_global_data::read().pending_file, selected_map);
    });
}

#[test]
fn map_select_start_game_without_selection_preserves_both_pending_maps() {
    crate::test_support::ensure_ambient_host();
    with_global_data_residences_restored(|| {
        let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
        ini_global.write().pending_file = "Maps\\Legacy\\Ini.map".to_string();
        runtime_global_data::write().pending_file = "Maps\\Legacy\\Runtime.map".to_string();

        let mut menu = MapSelectMenu::new();
        WindowEventEffects::with_standalone(|effects| menu.start_game(effects));

        assert!(!menu.start_game);
        assert_eq!(ini_global.read().pending_file, "Maps\\Legacy\\Ini.map");
        assert_eq!(
            runtime_global_data::read().pending_file,
            "Maps\\Legacy\\Runtime.map"
        );
    });
}
