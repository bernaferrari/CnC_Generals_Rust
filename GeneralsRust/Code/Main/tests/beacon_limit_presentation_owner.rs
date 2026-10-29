use game_engine::common::{ini::INILoadType, rts::player_template::PlayerTemplateStore};
use generals_main::game_logic::{GameLogic, GameMode, PlayerTemplateIdentity};
use generals_main::presentation_frame::PresentationFrame;

fn world() -> GameLogic {
    let mut templates = PlayerTemplateStore::new_owned();
    templates
        .apply_source(
            "PlayerTemplate FactionAmerica\n Side = America\n BeaconName = OwnedBeacon\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    let mut world = GameLogic::new();
    world.admit_player_template_definitions(templates).unwrap();
    world.admit_object_ini_sources([(
        "beacon-owner.ini".into(),
        "Object OwnedBeacon\n KindOf = SELECTABLE\n Body = ActiveBody BeaconBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\nObject OtherObject\n KindOf = SELECTABLE\n Body = ActiveBody ObjectBody\n MaxHealth = 100\n InitialHealth = 100\n End\nEnd\n".into(),
    )]).unwrap();
    assert!(world.start_new_game_with_player_template(
        GameMode::SinglePlayer,
        0,
        PlayerTemplateIdentity {
            template_name: "FactionAmerica".into(),
            template_index: Some(0)
        },
    ));
    world.admit_max_beacons_per_player(1);
    world
}

#[test]
fn native_beacon_button_uses_each_worlds_owned_template_and_team_members() {
    let mut first = world();
    let mut peer = world();
    let beacon = first
        .create_object_for_player("OwnedBeacon", 0, glam::Vec3::ZERO)
        .unwrap();
    let other = peer
        .create_object_for_player("OtherObject", 0, glam::Vec3::ZERO)
        .unwrap();
    assert_eq!(beacon, other, "worlds deliberately reuse object identities");
    let first_frame = PresentationFrame::build_from_logic(&mut first, 0);
    let peer_frame = PresentationFrame::build_from_logic(&mut peer, 0);
    assert_eq!(first_frame.local_beacon_button_enabled, Some(false));
    assert_eq!(peer_frame.local_beacon_button_enabled, Some(true));

    // C++ counts dead members but ignores members under construction.
    first
        .host_object_mut(beacon)
        .unwrap()
        .status
        .effectively_dead = true;
    assert_eq!(
        PresentationFrame::build_from_logic(&mut first, 0).local_beacon_button_enabled,
        Some(false)
    );
    first
        .host_object_mut(beacon)
        .unwrap()
        .status
        .under_construction = true;
    let next = PresentationFrame::build_from_logic(&mut first, 0);
    assert_eq!(next.local_beacon_button_enabled, Some(true));
    assert_eq!(peer_frame.local_beacon_button_enabled, Some(true));

    let mut client = game_client::core::GameClient::new().unwrap();
    client.set_in_game_ui_subsystem(
        game_client::core::subsystems::InGameUISubsystem::default(),
    );
    let mut bar = game_client::gui::control_bar::ControlBar::new();
    client.with_window_event_effects(None, |effects| {
        effects.with_manager(|manager, effects| {
            let button = manager.create_window(None, 0, 0, 20, 20, effects).unwrap();
            button
                .borrow_mut()
                .set_name("ControlBar.wnd:ButtonPlaceBeacon");
            first_frame.apply_to_control_bar(&mut bar, manager, effects);
            bar.update_owned(std::time::Duration::ZERO, manager, effects)
                .unwrap();
            assert!(!button.borrow().is_enabled());
            next.apply_to_control_bar(&mut bar, manager, effects);
            bar.update_owned(std::time::Duration::ZERO, manager, effects)
                .unwrap();
            assert!(button.borrow().is_enabled());
        });
    });
}
