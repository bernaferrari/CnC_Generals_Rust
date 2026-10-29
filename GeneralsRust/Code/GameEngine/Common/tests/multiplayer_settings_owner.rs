use game_engine::common::ini::{
    INI, INILoadType,
    ini_multiplayer::{MultiplayerSettings, with_multiplayer_settings},
};

fn parse(settings: &mut MultiplayerSettings, source: &str) {
    let mut ini = INI::new();
    ini.with_inline_source(source, |ini| {
        ini.parse_current_file_into_multiplayer_settings(settings)
    })
    .unwrap();
}

#[test]
fn ordered_multiplayer_definitions_stay_with_the_receiving_owner() {
    let ambient_before = with_multiplayer_settings(|settings| {
        (
            settings.get_num_colors(),
            settings.starting_money_choices.len(),
        )
    });
    let mut first = MultiplayerSettings::new();
    let mut peer = MultiplayerSettings::new();
    assert_eq!(first.max_beacons_per_player, 3);
    assert!(first.is_shroud_in_multiplayer);
    assert_eq!(first.get_color_value(-1), Some(0xffff_ffff));
    parse(
        &mut first,
        "OnlineChatColors\n Default = R:255 G:255 B:255\nEnd\n\
         MultiplayerSettings\n MaxBeaconsPerPlayer = 4\nEnd\n\
         MultiplayerColor TeamRed\n RGBColor = R:1 G:2 B:3\n RGBNightColor = R:4 G:5 B:6\nEnd\n\
         MultiplayerStartingMoneyChoice\n Value = 10000\n Default = Yes\nEnd\n",
    );
    parse(
        &mut peer,
        "MultiplayerSettings\n MaxBeaconsPerPlayer = 9\n UseShroud = No\nEnd\n\
         MultiplayerColor TeamRed\n RGBColor = R:10 G:20 B:30\nEnd\n",
    );
    parse(
        &mut first,
        "MultiplayerColor TeamRed\n RGBColor = R:7 G:8 B:9\nEnd\n\
         MultiplayerColor TeamBlue\n RGBColor = R:2 G:3 B:4\nEnd\n",
    );

    assert_eq!(first.max_beacons_per_player, 4);
    assert_eq!(first.get_num_colors(), 2);
    assert_eq!(first.get_color_value(0), Some(0xff07_0809));
    assert_eq!(first.color_definitions[0].night_color, 0xff04_0506);
    assert_eq!(first.get_color_value(1), Some(0xff02_0304));
    assert_eq!(first.starting_money_choices.len(), 1);
    assert!(first.got_default_starting_money);
    assert_eq!(peer.max_beacons_per_player, 9);
    assert!(!peer.is_shroud_in_multiplayer);
    assert_eq!(peer.get_num_colors(), 1);
    assert_eq!(peer.get_color_value(0), Some(0xff0a_141e));
    assert!(peer.starting_money_choices.is_empty());
    assert_eq!(
        ambient_before,
        with_multiplayer_settings(|settings| {
            (
                settings.get_num_colors(),
                settings.starting_money_choices.len(),
            )
        })
    );

    let mut ini = INI::new();
    assert!(
        ini.with_inline_source_and_load_type(
            "MultiplayerSettings\n MaxBeaconsPerPlayer = 15\nEnd\n",
            INILoadType::CreateOverrides,
            |ini| ini.parse_current_file_into_multiplayer_settings(&mut peer),
        )
        .is_err()
    );
    assert_eq!(peer.max_beacons_per_player, 9);
}
