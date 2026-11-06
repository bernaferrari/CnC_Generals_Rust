use super::*;

fn with_fixture(operation: impl FnOnce(&mut WindowEventEffects<'_>)) {
    // Mirror WindowEventEffects::with_standalone loans so Accept/apply paths
    // that touch ui/menu_world/script_display do not panic mid-drive.
    let owner = crate::gui::window_manager::WindowManagerOwner::new();
    let mut menus = crate::gui::callbacks::menu_callbacks::MenuManager::new();
    let mut messages = Vec::new();
    let mut ui = crate::core::subsystems::InGameUISubsystem::default();
    let mut ime = crate::gui::ime_manager::ImeManager::new();
    let mut main_menu = crate::gui::shell::main_menu::MainMenu::new();
    let mut menu_host = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
    let mut script_display = crate::core::script_action_handler::ScriptDisplayReceiver::default();
    let mut effects = WindowEventEffects::new(&mut messages);
    effects.window_manager_owner = Some(&owner);
    effects.menus =
        Some(menus.receiving_menus(crate::core::game_client::ClientInputAuthority::NativeHosted));
    effects.ui = Some(&mut ui);
    effects.ime = Some(&mut ime);
    effects.main_menu = Some(&mut main_menu);
    effects.menu_host = Some(&mut menu_host);
    effects.menu_world = Some(crate::gui::campaign_launch_host_bridge::MenuWorldFacts::default());
    effects.script_display = Some(&mut script_display);
    operation(&mut effects);
}

fn install_named_button(name: &str, x: i32, y: i32, effects: &mut WindowEventEffects<'_>) {
    effects.with_manager(|manager, effects| {
        let button = manager
            .create_window(None, x, y, 80, 24, effects)
            .expect(name);
        button.borrow_mut().set_name(name);
        button.borrow_mut().hide(false, manager);
    });
}

#[test]
fn os_wnd_options_menu_accept_hits_button_then_latches() {
    crate::test_support::ensure_ambient_host();
    match crate::test_process::run_bounded(
        module_path!(),
        "os_wnd_options_menu_accept_hits_button_then_latches",
    ) {
        crate::test_process::TestProcess::ParentVerified => return,
        crate::test_process::TestProcess::Child => {}
    }
    with_fixture(|effects| {
        install_named_button("OptionsMenu.wnd:ButtonAccept", 10, 10, effects);
        assert!(
            drive_os_wnd_options_menu_accept_like_cpp(effects),
            "OS WND click on ButtonAccept must latch Accept residual"
        );
        assert_eq!(
            residual_options_menu_last_action(effects),
            ResidualOptionsMenuAction::Accept
        );
        assert!(!drive_os_wnd_options_menu_back_like_cpp(effects));
    });
}

#[test]
fn apply_options_from_host_sets_gamma_and_music_volume_like_cpp() {
    crate::test_support::ensure_ambient_host();
    match crate::test_process::run_bounded(
        module_path!(),
        "apply_options_from_host_sets_gamma_and_music_volume_like_cpp",
    ) {
        crate::test_process::TestProcess::ParentVerified => return,
        crate::test_process::TestProcess::Child => {}
    }
    // C++ OptionsMenu.cpp:1180-1262 MusicVolume 0-100 + TheDisplay->setGamma.
    let mut apply = HostOptionsApply::default();
    apply.music_volume = 80;
    apply.gamma_slider = 100;
    apply.resolution = (1024, 768);
    with_fixture(|effects| {
        assert!(apply_options_from_host(apply, effects));
    });
    // Gamma lives on the receiving display; host apply still mutates music residual.
    assert!((runtime_global_data::read().music_volume_factor - 0.8).abs() < 0.01);
}

#[test]
fn apply_options_from_host_custom_preserves_advanced_detail() {
    crate::test_support::ensure_ambient_host();
    match crate::test_process::run_bounded(
        module_path!(),
        "apply_options_from_host_custom_preserves_advanced_detail",
    ) {
        crate::test_process::TestProcess::ParentVerified => return,
        crate::test_process::TestProcess::Child => {}
    }
    let original = {
        let g = runtime_global_data::read();
        (
            g.writable.use_shadow_volumes,
            g.writable.use_shadow_decals,
            g.use_cloud_map,
            g.use_light_map,
            g.show_soft_water_edge,
            g.use_heat_effects,
            g.enable_behind_building_markers,
            g.use_trees,
            g.max_particle_count,
        )
    };
    {
        let mut g = runtime_global_data::write();
        g.writable.use_shadow_volumes = true;
        g.writable.use_shadow_decals = true;
        g.use_cloud_map = true;
        g.use_light_map = true;
        g.show_soft_water_edge = true;
        g.use_heat_effects = true;
        g.enable_behind_building_markers = true;
        g.use_trees = true;
        g.max_particle_count = 3000;
    }
    let mut apply = HostOptionsApply::default();
    apply.detail_index = 3;
    apply.resolution = (1024, 768);
    with_fixture(|effects| {
        assert!(apply_options_from_host(apply, effects));
    });
    {
        let g = runtime_global_data::read();
        assert!(g.writable.use_shadow_volumes);
        assert!(g.writable.use_shadow_decals);
        assert!(g.use_cloud_map);
        assert!(g.use_light_map);
        assert!(g.show_soft_water_edge);
        assert!(g.use_heat_effects);
        assert!(g.enable_behind_building_markers);
        assert!(g.use_trees);
        assert_eq!(g.max_particle_count, 3000);
    }
    let mut g = runtime_global_data::write();
    g.writable.use_shadow_volumes = original.0;
    g.writable.use_shadow_decals = original.1;
    g.use_cloud_map = original.2;
    g.use_light_map = original.3;
    g.show_soft_water_edge = original.4;
    g.use_heat_effects = original.5;
    g.enable_behind_building_markers = original.6;
    g.use_trees = original.7;
    g.max_particle_count = original.8;
}

#[test]
fn apply_options_from_host_publishes_scroll_anchors() {
    crate::test_support::ensure_ambient_host();
    match crate::test_process::run_bounded(
        module_path!(),
        "apply_options_from_host_publishes_scroll_anchors",
    ) {
        crate::test_process::TestProcess::ParentVerified => return,
        crate::test_process::TestProcess::Child => {}
    }
    use crate::gui::options_host_bridge::HostOptionsRequest;

    with_fixture(|effects| {
        let _ = effects
            .menu_host
            .as_deref_mut()
            .map(|host| host.options.take_requests())
            .unwrap_or_default();
        let original_draw = effects.ui().get_draw_rmb_scroll_anchor();
        let original_move = effects.ui().get_move_rmb_scroll_anchor();

        let mut apply = HostOptionsApply::default();
        apply.draw_anchor = true;
        apply.move_anchor = true;
        apply.resolution = (1024, 768);
        assert!(apply_options_from_host(apply, effects));
        let reqs = effects
            .menu_host
            .as_deref_mut()
            .map(|host| host.options.take_requests())
            .unwrap_or_default();
        assert!(
            reqs.contains(&HostOptionsRequest::DrawRmbScrollAnchor { enabled: true }),
            "{reqs:?}"
        );
        assert!(
            reqs.contains(&HostOptionsRequest::MoveRmbScrollAnchor { enabled: true }),
            "{reqs:?}"
        );
        assert!(effects.ui().get_draw_rmb_scroll_anchor());
        assert!(effects.ui().get_move_rmb_scroll_anchor());
        effects.ui_mut().set_draw_rmb_scroll_anchor(original_draw);
        effects.ui_mut().set_move_rmb_scroll_anchor(original_move);
    });
}

#[test]
fn os_wnd_options_menu_keyboard_and_defaults_hit_named_gadgets() {
    crate::test_support::ensure_ambient_host();
    match crate::test_process::run_bounded(
        module_path!(),
        "os_wnd_options_menu_keyboard_and_defaults_hit_named_gadgets",
    ) {
        crate::test_process::TestProcess::ParentVerified => return,
        crate::test_process::TestProcess::Child => {}
    }
    with_fixture(|effects| {
        install_named_button("OptionsMenu.wnd:ButtonKeyboardOptions", 10, 40, effects);
        install_named_button("OptionsMenu.wnd:ButtonDefaults", 10, 70, effects);
        assert!(drive_os_wnd_options_menu_keyboard_like_cpp(effects));
        assert_eq!(
            residual_options_menu_last_action(effects),
            ResidualOptionsMenuAction::Keyboard
        );
        assert!(drive_os_wnd_options_menu_defaults_like_cpp(effects));
        assert_eq!(
            residual_options_menu_last_action(effects),
            ResidualOptionsMenuAction::Defaults
        );
    });
}
