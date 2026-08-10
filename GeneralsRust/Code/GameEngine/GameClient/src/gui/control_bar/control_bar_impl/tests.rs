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
        control_bar.sync_upgrades_and_specials_from_presentation(
            &[UPGRADE.to_string()],
            None,
            false,
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
            let mut center = game_engine::common::ini::ini_upgrade::get_upgrade_center_mut()
                .expect("INI upgrade center");
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
        control_bar.sync_upgrades_and_specials_from_presentation(
            &[UPGRADE.to_string()],
            None,
            false,
            0.0,
        );

        let cameos = &control_bar.get_portrait_state().upgrade_cameos;
        assert_eq!(cameos.len(), 1);
        assert_eq!(cameos[0].upgrade_name, UPGRADE);
        assert_eq!(cameos[0].button_image, IMAGE);
        assert_ne!(cameos[0].button_image, UPGRADE);
    }

    #[test]
    fn unknown_upgrade_cameo_keeps_fail_closed_name_placeholder() {
        const UPGRADE: &str = "Upgrade_UnknownCameoNoArtRegistered";
        let mut control_bar = ControlBar::new();
        control_bar.sync_upgrades_and_specials_from_presentation(
            &[UPGRADE.to_string()],
            None,
            false,
            0.0,
        );
        let cameos = &control_bar.get_portrait_state().upgrade_cameos;
        assert_eq!(cameos.len(), 1);
        assert_eq!(cameos[0].upgrade_name, UPGRADE);
        assert_eq!(cameos[0].button_image, UPGRADE);
    }
}
