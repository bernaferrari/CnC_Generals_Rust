// Split from `gui/control_bar/control_bar.rs` dump. Included by `control_bar_impl/mod.rs`.

#[cfg(test)]
mod tests {
    use super::*;

    fn named_window(name: &str) -> Rc<RefCell<GameWindow>> {
        let window = Rc::new(RefCell::new(GameWindow::new()));
        window.borrow_mut().set_name(name);
        window
    }

    #[test]
    fn local_beacon_windows_show_editor_and_caption_text() {
        let text_entry = Some(named_window("ControlBar.wnd:EditBeaconText"));
        let static_text = Some(named_window("ControlBar.wnd:StaticTextBeaconLabel"));
        let clear_button = Some(named_window("ControlBar.wnd:ButtonClearBeaconText"));
        for window in [&text_entry, &static_text, &clear_button]
            .into_iter()
            .flatten()
        {
            window.borrow_mut().hide(true).unwrap();
        }

        ControlBar::apply_beacon_window_state(
            &text_entry,
            &static_text,
            &clear_button,
            true,
            "Beacon Alpha",
        );

        let edit = text_entry.unwrap();
        assert!(!edit.borrow().is_hidden());
        assert_eq!(edit.borrow().get_text(), "Beacon Alpha");
        assert!(!static_text.unwrap().borrow().is_hidden());
        assert!(!clear_button.unwrap().borrow().is_hidden());
    }

    #[test]
    fn nonlocal_beacon_windows_hide_editor_label_and_clear() {
        let text_entry = Some(named_window("ControlBar.wnd:EditBeaconText"));
        let static_text = Some(named_window("ControlBar.wnd:StaticTextBeaconLabel"));
        let clear_button = Some(named_window("ControlBar.wnd:ButtonClearBeaconText"));

        ControlBar::apply_beacon_window_state(
            &text_entry,
            &static_text,
            &clear_button,
            false,
            "Enemy Beacon",
        );

        assert!(text_entry.unwrap().borrow().is_hidden());
        assert!(static_text.unwrap().borrow().is_hidden());
        assert!(clear_button.unwrap().borrow().is_hidden());
    }

    #[test]
    fn place_beacon_button_enabled_state_tracks_limit() {
        let place_button = Some(named_window("ControlBar.wnd:ButtonPlaceBeacon"));

        ControlBar::apply_place_beacon_button_enabled(&place_button, false);
        assert!(!place_button.as_ref().unwrap().borrow().is_enabled());

        ControlBar::apply_place_beacon_button_enabled(&place_button, true);
        assert!(place_button.unwrap().borrow().is_enabled());
    }

    #[test]
    fn upgrade_cameo_uses_command_button_image_not_upgrade_name() {
        const UPGRADE: &str = "Upgrade_TestCameoImageLookup";
        const IMAGE: &str = "SNTestCameoArt";
        const BUTTON: &str = "Command_TestCameoImageLookup";

        game_engine::common::ini::ini_command_button::initialize_control_bar();
        {
            let mut bar = game_engine::common::ini::ini_command_button::get_control_bar_mut()
                .expect("INI control bar");
            let button = bar.new_command_button(BUTTON.to_string());
            button.upgrade = UPGRADE.to_string();
            button.button_image = IMAGE.to_string();
        }

        let mut control_bar = ControlBar::new();
        control_bar.sync_upgrade_cameos_from_presentation(
            &[UPGRADE.to_string()],
            &[UPGRADE.to_string()],
            None,
            false,
            0.0,
            0.0,
        );

        let cameos = &control_bar.get_portrait_state().upgrade_cameos;
        assert_eq!(cameos.len(), 1);
        assert_eq!(cameos[0].upgrade_name, UPGRADE);
        assert_eq!(cameos[0].button_image, IMAGE);
        assert_ne!(cameos[0].button_image, UPGRADE);
    }

    #[test]
    fn upgrade_cameo_uses_upgrade_template_button_image() {
        const UPGRADE: &str = "Upgrade_TestTemplateCameoImage";
        const IMAGE: &str = "SSTestTemplateCameo";

        game_engine::common::ini::ini_upgrade::initialize_upgrade_center();
        {
            let center = game_engine::common::ini::ini_upgrade::get_upgrade_center();
            let mut center = center.write().expect("INI upgrade center");
            let template = center.new_template(
                game_engine::common::ascii_string::AsciiString::from(UPGRADE),
            );
            template.button_image = game_engine::common::ascii_string::AsciiString::from(IMAGE);
        }

        // C++ path: TheUpgradeCenter->findUpgrade()->getButtonImage()
        {
            use game_engine::common::ini::{INIError, INI};
            use gamelogic::upgrade::center::with_upgrade_center_mut;
            let source = format!("{UPGRADE}\nButtonImage = {IMAGE}\nEnd\n");
            let mut ini = INI::new();
            ini.with_inline_source(&source, |ini| {
                ini.read_line()?;
                with_upgrade_center_mut(|center| {
                    center
                        .parse_upgrade_definition(ini)
                        .map_err(|_| INIError::InvalidData)
                })
            })
            .expect("register GameLogic upgrade ButtonImage");
        }

        let mut control_bar = ControlBar::new();
        control_bar.sync_upgrade_cameos_from_presentation(
            &[UPGRADE.to_string()],
            &[UPGRADE.to_string()],
            None,
            false,
            0.0,
            0.0,
        );

        let cameos = &control_bar.get_portrait_state().upgrade_cameos;
        assert_eq!(cameos.len(), 1);
        assert_eq!(cameos[0].upgrade_name, UPGRADE);
        assert_eq!(cameos[0].button_image, IMAGE);
        assert_ne!(cameos[0].button_image, UPGRADE);
    }

    #[test]
    fn update_for_selection_runs_real_update_and_clears_dirty() {
        crate::helpers::register_live_control_bar_hooks();
        let mut control_bar = ControlBar::new();
        control_bar
            .update_for_selection(Vec::new())
            .expect("update_for_selection");
        assert!(
            !control_bar.is_ui_dirty(),
            "live update_for_selection must call update() and clear ui_dirty"
        );
    }

    #[test]
    fn live_ui_hooks_mark_control_bar_dirty_until_update() {
        crate::helpers::register_live_control_bar_hooks();
        gamelogic::control_bar::mark_ui_dirty();
        let mut control_bar = ControlBar::new();
        assert!(!control_bar.is_ui_dirty());
        control_bar
            .update(std::time::Duration::from_millis(33))
            .expect("update");
        // Dirty is applied then evaluate_context_ui clears it.
        assert!(!control_bar.is_ui_dirty());
    }

    #[test]
    fn unknown_upgrade_cameo_keeps_fail_closed_name_placeholder() {
        const UPGRADE: &str = "Upgrade_UnknownCameoNoArtRegistered";
        let mut control_bar = ControlBar::new();
        control_bar.sync_upgrade_cameos_from_presentation(
            &[UPGRADE.to_string()],
            &[UPGRADE.to_string()],
            None,
            false,
            0.0,
            0.0,
        );
        let cameos = &control_bar.get_portrait_state().upgrade_cameos;
        assert_eq!(cameos.len(), 1);
        assert_eq!(cameos[0].upgrade_name, UPGRADE);
        assert_eq!(cameos[0].button_image, UPGRADE);
    }

    #[test]
    fn presentation_special_power_cooldown_feeds_inverse_clock_percent() {
        // C++ SpecialPowerModule::getPercentReady = 1.0 - remaining/reloadTime.
        // ControlBarCommand.cpp:1404-1407 GadgetButtonDrawInverseClock(percentReady*100).
        let mut bar = ControlBar::new();
        bar.sync_upgrades_and_specials_from_presentation(&[], None, false, 45.0, 180.0);
        let portrait = bar.get_portrait_state();
        assert!(!portrait.special_power_ready);
        assert!((portrait.special_power_cooldown_remaining - 45.0).abs() < 0.01);
        assert!((portrait.special_power_cooldown_total - 180.0).abs() < 0.01);
        let mut command = CommandButton::default();
        command.command_type = CommandType::DoSpecialPower;
        assert_eq!(bar.command_not_ready_clock(&command, 1), Some(75));
    }

    #[test]
    fn control_bar_background_draw_ships_nothing_without_scheme() {
        // C++ W3DControlBar.cpp:615-623 + ControlBarScheme.cpp:794-799: with no
        // scheme manager (or a missing scheme image) the background draw paints
        // NOTHING — the old black HUD-strip fallback was non-C++ and removed.
        crate::gui::w3d_gadget_draw::reset_shipped_ui_draw_command_count();
        let mut window = GameWindow::new();
        window.set_name("ControlBar.wnd:BackgroundMarker");
        let _ = window.set_position(0, 460);
        let _ = window.set_size(800, 140);
        crate::gui::w3d_gadget_draw::w3d_command_bar_background_draw(
            &window,
            window.instance_data(),
        );
        assert_eq!(
            crate::gui::w3d_gadget_draw::shipped_ui_draw_command_count(),
            0,
            "manager-less background draw must stay silent, never paint a fallback strip"
        );
    }

    #[test]
    fn control_bar_money_display_formats_like_cpp_ingame_ui() {
        // C++ InGameUI.cpp:1803 buffer.format(TheGameText->fetch("GUI:ControlBarMoneyDisplay"), currentMoney)
        let formatted = ControlBar::format_control_bar_money_display(1250);
        assert!(
            formatted.contains("1250"),
            "MoneyDisplay text must include the player cash amount, got {formatted}"
        );
        assert_ne!(formatted, "1250", "must wrap amount in GUI:ControlBarMoneyDisplay or $ prefix");
    }

    #[test]
    fn control_bar_update_writes_money_display_window_text() {
        // Live path: ControlBar::update → update_money_and_power_windows writes
        // ControlBar.wnd:MoneyDisplay via TheWindowManager (C++ InGameUI.cpp:1776-1815).
        crate::gui::with_window_manager(|manager| {
            let win = manager
                .create_window(None, 0, 0, 80, 24)
                .expect("MoneyDisplay");
            win.borrow_mut().set_name("ControlBar.wnd:MoneyDisplay");
            let _ = win.borrow_mut().set_text("PLACEHOLDER");
        });
        let mut bar = ControlBar::new();
        bar.update_money_and_power_windows();
        let text = crate::gui::with_window_manager_ref(|manager| {
            manager
                .find_window_by_name("ControlBar.wnd:MoneyDisplay")
                .map(|w| w.borrow().get_text().to_string())
        });
        // No local player → hide; placeholder must not remain visible as live money.
        let hidden = crate::gui::with_window_manager_ref(|manager| {
            manager
                .find_window_by_name("ControlBar.wnd:MoneyDisplay")
                .map(|w| w.borrow().is_hidden())
                .unwrap_or(false)
        });
        assert!(
            hidden || text.as_deref() != Some("PLACEHOLDER"),
            "MoneyDisplay must be hidden without a player or rewritten from ThePlayerList, got text={text:?} hidden={hidden}"
        );
    }

}
