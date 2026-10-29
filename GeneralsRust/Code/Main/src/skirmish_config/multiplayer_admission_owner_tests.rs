use super::*;
use game_engine::common::ini::ini_multiplayer::MultiplayerSettings;
use game_engine::common::system::skirmish_game_info::SkirmishGameInfo;

fn palette(colors: &[(u32, u32)]) -> MultiplayerSettings {
    let mut settings = MultiplayerSettings::new();
    for (index, &(day, night)) in colors.iter().enumerate() {
        let color =
            settings.new_multiplayer_color_definition(format!("Owned{index}").as_str().into());
        color.set_color(day);
        color.set_night_color(night);
    }
    settings
}

#[test]
fn receiving_palette_controls_actual_admission_and_retained_original_record() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_palette_controls_actual_admission_and_retained_original_record",
        || {
            lobby_definition_tests::admit_slot_definitions();
            let first = palette(&[(0x010203, 0x111213), (0x040506, 0x141516)]);
            let second = palette(&[(0x212223, 0x313233), (0x242526, 0x343536)]);
            let mut config = golden_skirmish_config("OwnedPaletteAdmission.map");
            config.random_seed = 73;
            let mut source = SkirmishGameInfo::default();
            source.in_game = true;
            source.in_progress = true;
            source.map_name = config.map.clone();
            source.seed = config.random_seed as i32;
            source.starting_cash = config.rules.starting_cash as u32;
            for slot in &mut source.slots {
                slot.color = -1;
                slot.orig_color = -1;
            }
            source.slots[0].state = 5;
            source.slots[1].state = 3;
            for slot in &config.slots {
                source.slots[slot.slot_index].team_number = slot.team;
            }
            config.source_game_info = Some((source, 0));

            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            apply_skirmish_config_with_multiplayer_settings(&mut a, &config, &first).unwrap();
            apply_skirmish_config_with_multiplayer_settings(&mut b, &config, &second).unwrap();
            let saved_a = a.initial_skirmish_admission.as_ref().unwrap();
            let saved_b = b.initial_skirmish_admission.as_ref().unwrap();
            let info_a = saved_a
                .source
                .as_ref()
                .expect("first original record retained");
            let info_b = saved_b
                .source
                .as_ref()
                .expect("second original record retained");
            for id in 0..2 {
                let index = info_a.slots[id].color;
                assert_eq!(
                    index, info_b.slots[id].color,
                    "palette values must not alter RNG"
                );
                assert_eq!(info_a.slots[id].orig_color, -1);
                let unpack = |packed: u32| {
                    (
                        ((packed >> 16) & 255) as u8,
                        ((packed >> 8) & 255) as u8,
                        (packed & 255) as u8,
                    )
                };
                assert_eq!(
                    a.get_player(id as u32).unwrap().color_rgb,
                    unpack(first.get_color_value(index).unwrap())
                );
                assert_eq!(
                    b.get_player(id as u32).unwrap().color_rgb,
                    unpack(second.get_color_value(index).unwrap())
                );
                assert_eq!(
                    a.get_player(id as u32).unwrap().color_night_rgb,
                    unpack(first.color_definitions[index as usize].night_color)
                );
                assert_eq!(
                    b.get_player(id as u32).unwrap().color_night_rgb,
                    unpack(second.color_definitions[index as usize].night_color)
                );
            }
            assert_ne!(info_a.slots[0].color, info_a.slots[1].color);
            assert_eq!(a.logic_random.seed_words(), b.logic_random.seed_words());
            let observer_a = a.replay_observer_player_id().expect("first observer");
            let observer_b = b.replay_observer_player_id().expect("second observer");
            assert_eq!(observer_a, observer_b);
            assert_eq!(a.get_player(observer_a).unwrap().color_rgb, (1, 2, 3));
            assert_eq!(
                a.get_player(observer_a).unwrap().color_night_rgb,
                (17, 18, 19)
            );
            assert_eq!(b.get_player(observer_b).unwrap().color_rgb, (33, 34, 35));
            assert_eq!(
                b.get_player(observer_b).unwrap().color_night_rgb,
                (49, 50, 51)
            );
            for (world, day, night) in [
                (&a, 0xff01_0203, 0xff11_1213),
                (&b, 0xff21_2223, 0xff31_3233),
            ] {
                let sides = &world.prepared_map_sides;
                let side = sides
                    .find_side_info("ReplayObserver")
                    .and_then(|index| sides.get_side_info(index))
                    .unwrap()
                    .get_dict();
                assert_eq!(
                    side.get_int(game_engine::common::well_known_keys::key_player_color()) as u32,
                    day
                );
                assert_eq!(
                    side.get_int(game_engine::common::well_known_keys::key_player_night_color())
                        as u32,
                    night
                );
            }
            let before = a.initial_skirmish_admission.clone();
            assert!(
                apply_skirmish_config_with_multiplayer_settings(
                    &mut a,
                    &config,
                    &palette(&[(0x112233, 0x445566)])
                )
                .is_err()
            );
            assert_eq!(
                a.initial_skirmish_admission, before,
                "failed admission must not reset the receiving world"
            );
        },
    );
}
