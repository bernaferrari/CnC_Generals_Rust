//! Real save-file round trips retain receiving-world resources and positions.
use game_engine::common::{ini::INILoadType, rts::player_template::PlayerTemplateStore};
use generals_main::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use generals_main::save_load::SaveFileManager;
use generals_main::skirmish_config::{
    SkirmishPlayerTemplateSelection, apply_skirmish_config, golden_skirmish_config,
};
use glam::Vec3;

fn empty_world() -> GameLogic {
    let mut definitions = PlayerTemplateStore::new_owned();
    definitions.apply_source("PlayerTemplate FactionAmerica\n Side = America\n StartMoney = 10000\n StartingBuilding = AmericaCommandCenter\nEnd\n", INILoadType::Overwrite).unwrap();
    let mut config = golden_skirmish_config("SaveModeMap");
    config.slots.truncate(1);
    config.slots[0].player_template = SkirmishPlayerTemplateSelection::Exact {
        template_name: "FactionAmerica".into(),
        template_index: 0,
    };
    let mut world = GameLogic::new();
    world
        .admit_player_template_definitions(definitions)
        .unwrap();
    apply_skirmish_config(&mut world, &config).unwrap();
    world.add_player(Player::new(2, Team::China, "Second", true));
    let mut template = ThingTemplate::new("SaveTestUnit");
    template.set_health(100.0).add_kind_of(KindOf::Infantry);
    world.templates.insert(template.name.clone(), template);
    world
}

#[test]
fn save_file_roundtrip_and_second_save_keep_worlds_independent() {
    let mut source = empty_world();
    source.get_player_mut(0).unwrap().resources.supplies = 7234;
    source.get_player_mut(2).unwrap().resources.supplies = 4567;
    let first = source
        .create_object_for_player("SaveTestUnit", 0, Vec3::new(31.0, 2.0, 49.0))
        .unwrap();
    let second = source
        .create_object_for_player("SaveTestUnit", 2, Vec3::new(-7.0, 3.0, 12.0))
        .unwrap();
    source.set_current_frame(123);

    let root = tempfile::tempdir().unwrap();
    let mut saves = SaveFileManager::with_save_directory(root.path());
    saves.init().unwrap();
    saves.quick_save(&source).unwrap();
    let decoded = saves.load_game_snapshot("quicksave").unwrap();
    let mut loaded = empty_world();
    saves
        .restore_game_snapshot(&decoded.snapshot, &mut loaded)
        .unwrap();
    assert_eq!(loaded.get_player(0).unwrap().resources.supplies, 7234);
    assert_eq!(loaded.get_player(2).unwrap().resources.supplies, 4567);
    assert_eq!(
        loaded.host_object(first).unwrap().get_position(),
        Vec3::new(31.0, 2.0, 49.0)
    );
    assert_eq!(
        loaded.host_object(second).unwrap().get_position(),
        Vec3::new(-7.0, 3.0, 12.0)
    );
    assert_eq!(loaded.get_current_frame(), 123);

    loaded.get_player_mut(0).unwrap().resources.supplies = 3210;
    assert!(loaded.set_registered_object_position(first, Vec3::new(4.0, 5.0, 6.0)));
    saves.quick_save(&loaded).unwrap();
    let decoded = saves.load_game_snapshot("quicksave").unwrap();
    let mut reloaded = empty_world();
    saves
        .restore_game_snapshot(&decoded.snapshot, &mut reloaded)
        .unwrap();
    assert_eq!(reloaded.get_player(0).unwrap().resources.supplies, 3210);
    assert_eq!(reloaded.get_player(2).unwrap().resources.supplies, 4567);
    assert_eq!(
        reloaded.host_object(first).unwrap().get_position(),
        Vec3::new(4.0, 5.0, 6.0)
    );
    assert_eq!(
        reloaded.host_object(second).unwrap().get_position(),
        Vec3::new(-7.0, 3.0, 12.0)
    );
    assert_eq!(source.get_player(0).unwrap().resources.supplies, 7234);
    assert_eq!(
        source.host_object(first).unwrap().get_position(),
        Vec3::new(31.0, 2.0, 49.0)
    );
}
