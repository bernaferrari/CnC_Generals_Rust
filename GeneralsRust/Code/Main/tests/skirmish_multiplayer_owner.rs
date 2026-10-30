use game_engine::common::ini::{INILoadType, ini_multiplayer::MultiplayerSettings};
use game_engine::common::rts::player_template::PlayerTemplateStore;
use game_engine::common::system::skirmish_game_info::SkirmishGameInfo;
use generals_main::game_logic::GameLogic;
use generals_main::save_load::SnapshotBuilder;
use generals_main::skirmish_config::{
    GameRulesSnapshot, SkirmishMatchConfig, SkirmishPlayerTemplateSelection, SkirmishSlotConfig,
    apply_skirmish_config_with_multiplayer_settings,
};

type Color = (u8, u8, u8);
type DayNight = (Color, Color);

fn player_templates() -> PlayerTemplateStore {
    let mut store = PlayerTemplateStore::new_owned();
    store
        .apply_source(
            "PlayerTemplate FactionAmerica\n Side = America\n BaseSide = America\n StartingBuilding = AmericaCommandCenter\nEnd\n\
             PlayerTemplate FactionGLA\n Side = GLA\n BaseSide = GLA\n StartingBuilding = GLACommandCenter\nEnd\n\
             PlayerTemplate FactionObserver\n IsObserver = Yes\n Side = Observer\n BaseSide = Observer\nEnd\n\
             PlayerTemplate FactionCivilian\n Side = Civilian\n BaseSide = Civilian\nEnd\n",
            INILoadType::Overwrite,
        )
        .expect("parse owned player templates");
    store
}

fn palette(colors: &[(u32, u32)]) -> MultiplayerSettings {
    let mut settings = MultiplayerSettings::new();
    for (index, &(day, night)) in colors.iter().enumerate() {
        let definition =
            settings.new_multiplayer_color_definition(format!("Owned{index}").as_str().into());
        definition.set_color(day);
        definition.set_night_color(night);
    }
    settings
}

fn config(store: &PlayerTemplateStore) -> SkirmishMatchConfig {
    let mut source = SkirmishGameInfo::default();
    source.in_game = true;
    source.in_progress = true;
    source.map_name = "OwnedPaletteAdmission.map".into();
    source.seed = 73;
    source.starting_cash = 10_000;
    for slot in &mut source.slots {
        slot.color = -1;
        slot.orig_color = -1;
    }
    let slots = [
        ("FactionAmerica", "USA", true),
        ("FactionGLA", "GLA", false),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (name, faction, is_human))| {
        let template_index = store.find_template_index(name).unwrap() as i32;
        source.slots[index].state = if is_human { 5 } else { 3 };
        source.slots[index].team_number = index as i32;
        SkirmishSlotConfig {
            slot_index: index,
            is_human,
            is_active: true,
            player_template: SkirmishPlayerTemplateSelection::Exact {
                template_name: name.into(),
                template_index,
            },
            faction: faction.into(),
            color_rgb: (255, 255, 255),
            team: index as i32,
            start_position: index as i32,
            player_name: format!("Player {index}"),
            ai_difficulty: (!is_human).then(|| "Medium".into()),
        }
    })
    .collect();
    SkirmishMatchConfig {
        source_game_info: Some((source, 0)),
        map: "OwnedPaletteAdmission.map".into(),
        random_seed: 73,
        old_factions_only: false,
        rules: GameRulesSnapshot::default_rules(),
        slots,
    }
}

fn colors(world: &GameLogic, id: u32) -> DayNight {
    let player = world.get_player(id).expect("admitted player");
    (player.color_rgb, player.color_night_rgb)
}

#[test]
fn skirmish_admission_uses_each_worlds_multiplayer_palette() {
    let first_definitions = player_templates();
    let second_definitions = player_templates();
    let first_config = config(&first_definitions);
    let second_config = config(&second_definitions);
    let first_palette = palette(&[(0x010203, 0x111213), (0x040506, 0x141516)]);
    let second_palette = palette(&[(0x212223, 0x313233), (0x242526, 0x343536)]);
    let first_choices: [DayNight; 2] = [((1, 2, 3), (17, 18, 19)), ((4, 5, 6), (20, 21, 22))];
    let second_choices: [DayNight; 2] =
        [((33, 34, 35), (49, 50, 51)), ((36, 37, 38), (52, 53, 54))];

    let mut first = GameLogic::new();
    first
        .admit_player_template_definitions(first_definitions)
        .expect("admit first player templates");
    apply_skirmish_config_with_multiplayer_settings(&mut first, &first_config, &first_palette)
        .expect("admit first skirmish");
    let first_before = [colors(&first, 0), colors(&first, 1)];
    let first_observer = first.replay_observer_player_id().expect("first observer");
    let first_observer_before = colors(&first, first_observer);

    let mut second = GameLogic::new();
    second
        .admit_player_template_definitions(second_definitions)
        .expect("admit second player templates");
    apply_skirmish_config_with_multiplayer_settings(&mut second, &second_config, &second_palette)
        .expect("admit second skirmish");
    let second_observer = second.replay_observer_player_id().expect("second observer");
    assert_eq!(first_observer, second_observer);
    assert_eq!(first_observer_before, first_choices[0]);
    assert_eq!(colors(&second, second_observer), second_choices[0]);
    let mut indices = Vec::new();
    for id in 0..2 {
        let first_index = first_choices
            .iter()
            .position(|c| *c == colors(&first, id))
            .unwrap();
        let second_index = second_choices
            .iter()
            .position(|c| *c == colors(&second, id))
            .unwrap();
        assert_eq!(
            first_index, second_index,
            "same seed selects corresponding colors"
        );
        indices.push(first_index);
    }
    assert_ne!(
        indices[0], indices[1],
        "occupied slots receive distinct colors"
    );
    assert_eq!([colors(&first, 0), colors(&first, 1)], first_before);
    assert_eq!(colors(&first, first_observer), first_observer_before);

    let second_before = [colors(&second, 0), colors(&second, 1)];
    let too_small = palette(&[(0x112233, 0x445566)]);
    assert!(
        apply_skirmish_config_with_multiplayer_settings(&mut first, &first_config, &too_small)
            .is_err()
    );
    assert_eq!([colors(&first, 0), colors(&first, 1)], first_before);
    assert_eq!(colors(&first, first_observer), first_observer_before);
    assert_eq!([colors(&second, 0), colors(&second, 1)], second_before);
    assert_eq!(colors(&second, second_observer), second_choices[0]);

    // The save records selected indices, so restoring needs the receiving
    // world's authored definitions rather than another session's palette.
    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&first)
        .expect("snapshot first world");
    assert!(
        builder
            .restore_from_snapshot(&snapshot, &mut second)
            .is_err(),
        "a receiving palette disagreement must reject the save"
    );
    assert_eq!([colors(&second, 0), colors(&second, 1)], second_before);
    assert_eq!(colors(&second, second_observer), second_choices[0]);
    let restore_definitions = player_templates();
    let restore_config = config(&restore_definitions);
    let mut restored = GameLogic::new();
    restored
        .admit_player_template_definitions(restore_definitions)
        .expect("admit restore player templates");
    apply_skirmish_config_with_multiplayer_settings(&mut restored, &restore_config, &first_palette)
        .expect("admit restore settings");
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .expect("restore with the receiving palette");
    assert_eq!([colors(&restored, 0), colors(&restored, 1)], first_before);
    let restored_observer = restored
        .replay_observer_player_id()
        .expect("restored observer");
    assert_eq!(colors(&restored, restored_observer), first_observer_before);
    assert_eq!([colors(&second, 0), colors(&second, 1)], second_before);
}
