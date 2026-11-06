use game_engine::common::{ini::INILoadType, rts::player_template::PlayerTemplateStore};
use generals_main::game_logic::{GameLogic, GameMode, PlayerTemplateIdentity};
use generals_main::presentation_frame::PresentationFrame;

#[test]
fn selected_world_preserves_sparse_upgrade_cameo_positions() {
    let mut templates = PlayerTemplateStore::new_owned();
    templates
        .apply_source(
            "PlayerTemplate FactionAmerica\n Side = America\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    let mut world = GameLogic::new();
    world.admit_player_template_definitions(templates).unwrap();
    world.admit_object_ini_sources([(
        "cameo-slots.ini".into(),
        "Object SparseCameoObject\n KindOf = SELECTABLE\n UpgradeCameo2 = Upgrade_Second\n UpgradeCameo5 = Upgrade_Fifth\n Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
    )]).unwrap();
    assert!(world.start_new_game_with_player_template(
        GameMode::SinglePlayer,
        0,
        PlayerTemplateIdentity {
            template_name: "FactionAmerica".into(),
            template_index: Some(0)
        },
    ).unwrap());
    let id = world
        .create_object_for_player("SparseCameoObject", 0, glam::Vec3::ZERO)
        .unwrap();
    world.host_object_mut(id).unwrap().selected = true;
    world.get_player_mut(0).unwrap().selected_objects = vec![id];
    let frame = PresentationFrame::build_from_logic(&mut world, 0);
    let mut catalog = gamelogic::upgrade::center::UpgradeCenter::new();
    catalog.new_upgrade("Upgrade_Second".into());
    catalog.new_upgrade("Upgrade_Fifth".into());
    let mut bar = game_client::gui::control_bar::ControlBar::new();
    bar.set_receiving_upgrade_definitions(&catalog, None);
    let mut client = game_client::core::GameClient::new().unwrap();
    client.set_in_game_ui_subsystem(Default::default());
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let windows: Vec<_> = (1..=5)
                .map(|index| {
                    let window = manager.create_window(None, 0, 0, 40, 40, effects).unwrap();
                    window
                        .borrow_mut()
                        .set_name(&format!("ControlBar.wnd:UnitUpgrade{index}"));
                    window
                })
                .collect();
            frame.apply_to_control_bar(&mut bar, manager, effects);
            assert!(windows[0].borrow().is_hidden());
            assert!(!windows[1].borrow().is_hidden());
            assert!(windows[2].borrow().is_hidden());
            assert!(windows[3].borrow().is_hidden());
            assert!(!windows[4].borrow().is_hidden());
        })
    });
    let slots = &bar.get_portrait_state().upgrade_cameos;
    assert_eq!(slots.len(), 5);
    assert!(!slots[0].is_visible);
    assert_eq!(slots[1].upgrade_name, "Upgrade_Second");
    assert!(slots[1].is_visible);
    assert!(!slots[2].is_visible && !slots[3].is_visible);
    assert_eq!(slots[4].upgrade_name, "Upgrade_Fifth");
    assert!(slots[4].is_visible);
}
