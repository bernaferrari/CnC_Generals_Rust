//! CPP GameLogic1449–1450: synthetic observer uses authored color zero.
use super::*;

#[test]
fn synthetic_observer_projects_authored_zero_day_and_night_to_host_sides_and_core() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "synthetic_observer_projects_authored_zero_day_and_night_to_host_sides_and_core",
        || {
            crate::skirmish_config::lobby_definition_tests::admit_slot_definitions();
            game_engine::common::ini::ini_multiplayer::with_multiplayer_settings(|settings| {
                assert!(
                    settings.color_definitions.is_empty(),
                    "isolated definition witness must author actual index zero"
                );
            });
            let mut ini = game_engine::common::ini::INI::new();
            ini.with_inline_source("MultiplayerColor SyntheticAuthoredZero\n RGBColor = R:31 G:47 B:59\n RGBNightColor = R:71 G:83 B:97\nEnd\nMultiplayerColor OccupiedObserverSeparate\n RGBColor = R:101 G:103 B:107\n RGBNightColor = R:109 G:113 B:127\nEnd\n", |ini| ini.parse_current_file()).unwrap();
            let mut world = GameLogic::new();
            let observer = world.ensure_replay_observer_player();
            let actual = world.get_player(observer).unwrap();
            assert_eq!(actual.color_rgb, (31, 47, 59));
            assert_eq!(actual.color_night_rgb, (71, 83, 97));
            assert!(actual.is_replay_observer());
            world.install_replay_observer_side();
            let sides = &world.prepared_map_sides;
            let side = sides
                .find_side_info("ReplayObserver")
                .and_then(|i| sides.get_side_info(i))
                .unwrap()
                .get_dict();
            assert_eq!(side.get_int(key_player_color()) as u32, 0xff1f_2f3b);
            assert_eq!(side.get_int(key_player_night_color()) as u32, 0xff47_5361);
            let owned = world.get_player(observer).unwrap();
            assert_eq!(owned.color_rgb, (31, 47, 59));
            assert_eq!(owned.color_night_rgb, (71, 83, 97));
            let other = GameLogic::new();
            assert!(
                other.replay_observer_player_id().is_none(),
                "constructor must remain inert"
            );
            assert_eq!(world.ensure_replay_observer_player(), observer);
            assert_eq!(
                world.get_player(observer).unwrap().color_night_rgb,
                (71, 83, 97)
            );
            assert_eq!(world.prepared_map_sides.get_num_sides(), 1);
        },
    );
}
