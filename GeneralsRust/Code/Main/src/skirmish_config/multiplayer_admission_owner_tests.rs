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

#[test]
fn original_game_info_import_uses_receiving_palette_and_shroud() {
    use game_engine::common::ini::INILoadType;
    use game_engine::common::rts::player_template::PlayerTemplateStore;
    let mut store = PlayerTemplateStore::new_owned();
    store
        .apply_source(
            "PlayerTemplate FactionAmerica\n Side = America\n StartingBuilding = AmericaCommandCenter\nEnd\n",
            INILoadType::Overwrite,
        )
        .unwrap();
    let mut settings = palette(&[(0x123456, 0xabcdef)]);
    settings.is_shroud_in_multiplayer = false;
    let mut source = SkirmishGameInfo::default();
    source.in_game = true;
    source.in_progress = true;
    source.map_name = "OriginalOwned.map".into();
    source.slots[0].state = 5;
    source.slots[0].player_template = store.find_template_index("FactionAmerica").unwrap() as i32;
    source.slots[0].color = 0;
    let imported = saved_admission::SavedSkirmishAdmission::from_original_with_settings(
        source,
        &store,
        Some(&settings),
    )
    .unwrap();
    assert_eq!(imported.config.slots[0].color_rgb, (0x12, 0x34, 0x56));
    assert!(!imported.config.rules.fog_of_war);
    assert!(
        imported
            .validate_definitions_with_settings(Some(&store), Some(&palette(&[(0x010203, 0)])))
            .is_err()
    );
}

#[cfg(feature = "game_client")]
#[test]
fn client_setup_admits_each_worlds_shroud_and_exact_palette_selection() {
    use game_client::gui::skirmish_setup::{SkirmishSetup, snapshot_skirmish_lobby};
    use game_client::SlotState;
    use game_engine::common::message_stream::ClientSkirmishLaunch;
    use std::sync::Arc;

    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "client_setup_admits_each_worlds_shroud_and_exact_palette_selection",
        || {
            lobby_definition_tests::admit_slot_definitions();
            let definitions =
                (*game_engine::common::rts::player_template::get_player_template_store()).clone();
            let template = definitions.find_template_index("FactionAmerica").unwrap() as i32;
            let mut setup = SkirmishSetup::default();
            setup.set_selected_map("OwnerSetup.map".into());
            let info = setup.game_info_mut().game_info_mut();
            info.enter_game();
            info.set_map("OwnerSetup.map".into());
            info.set_seed(73);
            let slot = info.get_slot_mut(0).unwrap();
            slot.set_state(SlotState::Player, "Commander".into(), 1);
            slot.set_player_template(template);
            slot.set_color(1);
            slot.set_start_pos(0);
            slot.set_team_number(0);
            slot.save_off_original_info();
            info.set_local_ip(1);
            info.start_game(0);

            let mut first = palette(&[(0x123456, 0x010203), (0x123456, 0x111213)]);
            first.is_shroud_in_multiplayer = true;
            let mut second = palette(&[(0xabcdef, 0x212223), (0xabcdef, 0x313233)]);
            second.is_shroud_in_multiplayer = false;
            let common = game_engine::common::common_services::common_services();
            let mut a = GameLogic::new();
            let mut b = GameLogic::new();
            a.player_template_definitions = Some(definitions.clone());
            b.player_template_definitions = Some(definitions);
            let object_source = r#"
Object AmericaCommandCenter
  KindOf = STRUCTURE COMMANDCENTER
  Body = ActiveBody CommandCenterBody
    MaxHealth = 5000
    InitialHealth = 5000
  End
End
Object OwnerBuilding
  KindOf = STRUCTURE SELECTABLE
  Geometry = CYLINDER
  GeometryMajorRadius = 10
  GeometryHeight = 10
  Body = ActiveBody OwnerBuildingBody
    MaxHealth = 100
    InitialHealth = 100
  End
End
"#;
            for world in [&mut a, &mut b] {
                world.admit_object_ini_sources([(
                    "owner-setup-objects.ini".into(), object_source.to_owned(),
                )]).expect("admit ordered authored objects to receiving world");
            }
            a.multiplayer_definitions = Some(Arc::new(first.clone()));
            b.multiplayer_definitions = Some(Arc::new(second.clone()));

            for (world, ambient, day, night, fog) in [
                (&mut a, &second, (0x12, 0x34, 0x56), (0x11, 0x12, 0x13), true),
                (&mut b, &first, (0xab, 0xcd, 0xef), (0x31, 0x32, 0x33), false),
            ] {
                *common.ini.multiplayer.write() = ambient.clone();
                let settings = world.multiplayer_definitions.clone().unwrap();
                let config = config_from_client_skirmish_setup_with_definitions(
                    &setup, None, world.player_template_definitions.as_ref(), &settings,
                ).unwrap();
                assert_eq!(config.slots[0].color_rgb, day);
                assert_eq!(config.rules.fog_of_war, fog);
                apply_skirmish_config(world, &config).unwrap();
                assert!(world.templates.contains_key("OwnerBuilding"), "authored placement template survives NewGame");
                let player = world.get_player(0).unwrap();
                assert_eq!(player.color_rgb, day);
                assert_eq!(player.color_night_rgb, night);
                let original = world.initial_skirmish_admission.as_ref().unwrap()
                    .source.as_ref().expect("original client GameInfo retained");
                assert_eq!(original.slots[0].color, 1);
                assert_eq!(original.slots[0].orig_color, 1);
                world.world_services.shroud().lock().unwrap().init_shroud_grid(128.0, 128.0);
                let position = glam::Vec3::new(40.0, 0.0, 40.0);
                assert!(!world.is_build_location_shroud_clear(0, position));
                let result = world.legal_build_code_at(Team::USA, position, "OwnerBuilding");
                let shrouded = crate::game_logic::host_production_buildable_command_residual::LBC_SHROUD;
                assert_eq!(result == shrouded, fog, "fog must affect actual build legality");
            }

            let launch = ClientSkirmishLaunch {
                map: "OwnerSetup.map".into(),
                game_info: snapshot_skirmish_lobby(&setup),
                local_slot: 0,
                old_factions_only: false,
                fog_of_war: false,
                game_mode_code: 2,
                max_fps: 30,
            };
            let converted = config_from_skirmish_launch_with_definitions(
                &launch, a.player_template_definitions.as_ref(), &first,
            ).unwrap();
            assert!(!converted.rules.fog_of_war, "launch descriptor owns its fog value");
            assert_eq!(converted.slots[0].color_rgb, (0x12, 0x34, 0x56));
        },
    );
}
