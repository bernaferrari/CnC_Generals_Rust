use super::*;
use game_engine::common::rts::score_keeper::{KindOf, KindOfMaskType, ScoreableObject};
use game_engine::common::system::xfer_crc::XferCRC;
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use std::io::Cursor;

struct TestScoreObject {
    name: String,
    mask: KindOfMaskType,
    player_index: Option<Int>,
    under_construction: Bool,
}

impl TestScoreObject {
    fn unit(name: &str, player_index: Int) -> Self {
        let mut mask = KindOfMaskType::new();
        mask.set(KindOf::Vehicle);
        mask.set(KindOf::Score);
        Self {
            name: name.to_string(),
            mask,
            player_index: Some(player_index),
            under_construction: false,
        }
    }

    fn structure(name: &str, player_index: Int) -> Self {
        let mut mask = KindOfMaskType::new();
        mask.set(KindOf::Structure);
        mask.set(KindOf::Score);
        Self {
            name: name.to_string(),
            mask,
            player_index: Some(player_index),
            under_construction: false,
        }
    }
}

impl ScoreableObject for TestScoreObject {
    fn get_score_template_name(&self) -> &str {
        &self.name
    }

    fn get_score_kindof_mask(&self) -> KindOfMaskType {
        self.mask
    }

    fn get_score_controlling_player_index(&self) -> Option<i32> {
        self.player_index
    }

    fn is_score_under_construction(&self) -> bool {
        self.under_construction
    }
}

fn player_crc(player: &Player) -> u32 {
    let sink = Cursor::new(Vec::<u8>::new());
    let inner = XferSave::new(sink, 1);
    let mut xfer = XferCRC::new(inner);
    Snapshotable::crc(player, &mut xfer).unwrap();
    xfer.get_crc()
}

fn player_xfer_round_trip(mut source: Player, loaded_player_index: PlayerIndex) -> Player {
    let mut saved = Vec::new();
    {
        let cursor = Cursor::new(&mut saved);
        let mut xfer = XferSave::new(cursor, 1);
        Snapshotable::xfer(&mut source, &mut xfer).unwrap();
    }

    let mut loaded = Player::new(loaded_player_index);
    {
        let cursor = Cursor::new(saved);
        let mut xfer = XferLoad::new(cursor, 1);
        Snapshotable::xfer(&mut loaded, &mut xfer).unwrap();
    }
    loaded
}

#[test]
fn score_keeper_tracks_destroyed_objects_by_victim_player() {
    let mut keeper = ScoreKeeper::new_for_player(2);
    keeper.add_unit_built();
    keeper.add_building_built();
    keeper.add_money_earned(75);

    keeper.add_object_destroyed_obj(&TestScoreObject::unit("EnemyTank", 1));
    keeper.add_object_destroyed_obj(&TestScoreObject::unit("OwnTruck", 2));
    keeper.add_object_destroyed_obj(&TestScoreObject::structure("EnemyBarracks", 3));

    assert_eq!(keeper.units_destroyed_by_player[1], 1);
    assert_eq!(keeper.units_destroyed_by_player[2], 1);
    assert_eq!(keeper.buildings_destroyed_by_player[3], 1);
    assert_eq!(keeper.get_total_units_destroyed(), 2);
    assert_eq!(keeper.get_total_buildings_destroyed(), 1);
    assert_eq!(
        keeper.get_total_score(),
        100 + 100 + 75 + 100 + 100,
        "C++ score excludes destroyed objects owned by m_myPlayerIdx"
    );
}

#[test]
fn player_xfer_preserves_score_keeper_destroyed_arrays_and_score_fields() {
    let mut player = Player::new(4);
    player
        .score_keeper
        .add_object_destroyed_obj(&TestScoreObject::unit("EnemyTank", 1));
    player
        .score_keeper
        .add_object_destroyed_obj(&TestScoreObject::structure("EnemyBarracks", 3));
    player
        .score_keeper
        .add_object_lost_obj(&TestScoreObject::unit("OwnTruck", 4));
    player
        .score_keeper
        .add_object_built_obj(&TestScoreObject::unit("BuiltHumvee", 4));
    player
        .score_keeper
        .add_object_captured_obj(&TestScoreObject::structure("CapturedOilDerrick", 2));
    player.score_keeper.current_score = 1234;

    let loaded = player_xfer_round_trip(player, 0);

    assert_eq!(loaded.score_keeper.my_player_idx, 4);
    assert_eq!(loaded.score_keeper.units_destroyed_by_player[1], 1);
    assert_eq!(loaded.score_keeper.buildings_destroyed_by_player[3], 1);
    assert_eq!(loaded.score_keeper.get_total_units_destroyed(), 1);
    assert_eq!(loaded.score_keeper.get_total_buildings_destroyed(), 1);
    assert_eq!(loaded.score_keeper.objects_destroyed[1]["EnemyTank"], 1);
    assert_eq!(loaded.score_keeper.objects_destroyed[3]["EnemyBarracks"], 1);
    assert_eq!(loaded.score_keeper.objects_lost["OwnTruck"], 1);
    assert_eq!(loaded.score_keeper.objects_built["BuiltHumvee"], 1);
    assert_eq!(
        loaded.score_keeper.objects_captured["CapturedOilDerrick"],
        1
    );
    assert_eq!(loaded.score_keeper.get_current_score(), 1234);
    assert_eq!(loaded.score_keeper.faction_buildings_captured, 1);
}

#[test]
fn player_crc_matches_cpp_skill_and_science_surface() {
    let mut base = Player::new(0);
    base.skill_points = 10;
    base.science_purchase_points = 3;
    let base_crc = player_crc(&base);

    let mut save_only_change = Player::new(0);
    save_only_change.skill_points = 10;
    save_only_change.science_purchase_points = 3;
    save_only_change.money.set_money(50_000);
    save_only_change.general_name = "General AΩ".to_string();
    save_only_change.radar_count = 7;
    save_only_change.bombard_battle_plans = 2;

    assert_eq!(player_crc(&save_only_change), base_crc);

    let mut skill_change = Player::new(0);
    skill_change.skill_points = 11;
    skill_change.science_purchase_points = 3;

    assert_ne!(player_crc(&skill_change), base_crc);
}

#[test]
fn player_crc_includes_battle_plan_bonus_payload_like_cpp() {
    let base = Player::new(0);
    let mut with_bonus = Player::new(0);
    with_bonus.battle_plan_bonuses = Some(BattlePlanBonuses {
        armor_scalar: 1.25,
        sight_range_scalar: 1.5,
        bombardment: 1,
        hold_the_line: 0,
        search_and_destroy: 1,
        valid_kind_of: 0x12,
        invalid_kind_of: 0x40,
    });

    assert_ne!(player_crc(&with_bonus), player_crc(&base));
}

#[test]
fn player_template_new_defaults_playable_false_like_cpp() {
    let pt = PlayerTemplate::new("FactionTest".into());
    assert!(!pt.playable);
    assert!(!pt.is_playable_side());
    assert_eq!(pt.get_starting_unit(-1), "");
    assert_eq!(pt.get_starting_unit(0), "");
    assert_eq!(pt.get_starting_unit(9), "");
    assert_eq!(pt.get_starting_unit(10), "");
    assert_eq!(pt.get_starting_unit(MAX_MP_STARTING_UNITS as i32), "");
    assert_eq!(pt.starting_units.len(), MAX_MP_STARTING_UNITS);
    assert_ne!(pt.name_key, NAMEKEY_INVALID);
    assert_eq!(pt.get_name_key(), pt.name_key);
}

#[test]
fn player_template_is_playable_side_is_playable_flag_only() {
    let mut pt = PlayerTemplate::new("FactionBoss".into());
    pt.playable = true;
    pt.side = "Boss".into();
    assert!(pt.is_playable_side());
    assert!(!pt.is_playable_side_excluding_boss());

    pt.playable = false;
    assert!(!pt.is_playable_side());
    assert!(!pt.is_playable_side_excluding_boss());
}

#[test]
fn player_template_from_common_copies_name_key_playable_and_starting_units() {
    let mut common =
        game_engine::common::rts::player_template::PlayerTemplate::new("FactionAmerica".into());
    common.playable = true;
    common.side = "America".into();
    common.starting_building = "AmericaCommandCenter".into();
    common.starting_units[0] = "AmericaRanger".into();
    common.starting_units[1] = "AmericaDozer".into();
    common.starting_units[2] = "AmericaMissileDefender".into();

    let gl = PlayerTemplate::from_common(&common);
    assert_eq!(gl.name_key, common.name_key);
    assert_eq!(gl.get_name_key(), common.get_name_key());
    assert_eq!(gl.name, common.name);
    assert_eq!(gl.playable, common.playable);
    assert!(gl.is_playable_side());
    assert_eq!(gl.starting_units, common.starting_units);
    assert_eq!(gl.get_starting_unit(0), "AmericaRanger");
    assert_eq!(gl.get_starting_unit(1), "AmericaDozer");
    assert_eq!(gl.get_starting_unit(2), "AmericaMissileDefender");
    assert_eq!(gl.get_starting_unit(3), "");
    assert_eq!(gl.get_starting_unit(9), "");
    assert_eq!(gl.get_starting_unit(-1), "");
    assert_eq!(gl.get_starting_unit(10), "");
    assert_eq!(gl.starting_building, "AmericaCommandCenter");

    let mut unplayable = common.clone();
    unplayable.playable = false;
    unplayable.name_key = 42;
    let gl_unplayable = PlayerTemplate::from_common(&unplayable);
    assert_eq!(gl_unplayable.name_key, 42);
    assert!(!gl_unplayable.playable);
    assert!(!gl_unplayable.is_playable_side());
    assert_eq!(gl_unplayable.starting_units, unplayable.starting_units);
}

#[test]
fn player_template_from_common_copies_sciences_production_and_shortcut_fields() {
    let mut common =
        game_engine::common::rts::player_template::PlayerTemplate::new("FactionChina".into());
    common.intrinsic_sciences = vec!["SCIENCE_CHINA".into(), "SCIENCE_RedGuardTraining".into()];
    common.intrinsic_science_purchase_points = 3;
    let tank_key = NameKeyGenerator::name_to_key("ChinaTankOverlord");
    let gatling_key = NameKeyGenerator::name_to_key("ChinaVehicleTroopCrawler");
    common.production_cost_changes.insert(tank_key, 0.8);
    common.production_time_changes.insert(tank_key, 0.5);
    common.production_veterancy_levels.insert(
        gatling_key,
        game_engine::common::game_common::VeterancyLevel::Veteran,
    );
    common.special_power_shortcut_command_set = "SCIENCE_CHINA_SHORTCUT".into();
    common.special_power_shortcut_win_name = "ControlBar.wnd:SpecialPowerShortcutChina".into();
    common.special_power_shortcut_button_count = 5;
    common.purchase_science_command_set_rank1 = "SCIENCE_CHINA_RANK1".into();
    common.purchase_science_command_set_rank3 = "SCIENCE_CHINA_RANK3".into();
    common.purchase_science_command_set_rank8 = "SCIENCE_CHINA_RANK8".into();
    common.score_screen_image = "ChinaScoreScreen".into();
    common.load_screen_music = "Load_China".into();
    common.beacon_name = "ChinaBeacon".into();

    let gl = PlayerTemplate::from_common(&common);

    assert_eq!(gl.get_intrinsic_science_purchase_points(), 3);
    assert_eq!(
        gl.get_intrinsic_sciences(),
        &vec![
            NameKeyGenerator::name_to_key("SCIENCE_CHINA") as ScienceType,
            NameKeyGenerator::name_to_key("SCIENCE_RedGuardTraining") as ScienceType,
        ]
    );
    assert_eq!(
        gl.get_production_cost_changes().get(&tank_key).copied(),
        Some(0.8)
    );
    assert_eq!(
        gl.get_production_time_changes().get(&tank_key).copied(),
        Some(0.5)
    );
    assert_eq!(
        gl.get_production_veterancy_levels()
            .get(&gatling_key)
            .copied(),
        Some(VeterancyLevel::Veteran)
    );
    assert_eq!(
        gl.get_special_power_shortcut_command_set(),
        "SCIENCE_CHINA_SHORTCUT"
    );
    assert_eq!(
        gl.get_special_power_shortcut_win_name(),
        "ControlBar.wnd:SpecialPowerShortcutChina"
    );
    assert_eq!(gl.get_special_power_shortcut_button_count(), 5);
    assert_eq!(
        gl.get_purchase_science_command_set_rank1(),
        "SCIENCE_CHINA_RANK1"
    );
    assert_eq!(
        gl.get_purchase_science_command_set_rank3(),
        "SCIENCE_CHINA_RANK3"
    );
    assert_eq!(
        gl.get_purchase_science_command_set_rank8(),
        "SCIENCE_CHINA_RANK8"
    );
    assert_eq!(gl.get_score_screen(), "ChinaScoreScreen");
    assert_eq!(gl.get_load_screen_music(), "Load_China");
    assert_eq!(gl.get_beacon_template(), "ChinaBeacon");
    assert_eq!(gl.get_starting_unit(-1), "");
    assert_eq!(gl.get_starting_unit(10), "");
    assert_eq!(gl.get_starting_unit(MAX_MP_STARTING_UNITS as i32), "");
    assert!(!gl.is_playable_side());
}

#[test]
fn process_create_team_evicts_from_other_squads() {
    // C++ Player::processCreateTeamGameMessage (Player.cpp:3637-3647)
    let mut player = Player::new(0);
    player.init_from_dict_defaults();
    player.process_create_team_game_message(0, &[1, 2]);
    player.process_create_team_game_message(1, &[2, 3]);
    let squad0 = player.get_hotkey_squad_const(0).expect("squad 0");
    let squad1 = player.get_hotkey_squad_const(1).expect("squad 1");
    assert!(squad0.is_on_squad_by_id(1));
    assert!(!squad0.is_on_squad_by_id(2));
    assert!(squad1.is_on_squad_by_id(2));
    assert!(squad1.is_on_squad_by_id(3));
    player.process_select_team_game_message(1);
    assert_eq!(player.get_current_selection_ids(), vec![2, 3]);
}

#[test]
fn honor_kindof_filter_uses_retail_vehicle_and_aircraft_bits() {
    // C++ KindOf.h: VEHICLE=9, AIRCRAFT=10. Local ScoreKindOf Vehicle=5, Aircraft=6
    // would match KINDOF_CAN_CAST_REFLECTIONS / KINDOF_SHRUBBERY.
    assert_eq!(ScoreKeeper::score_kindof_retail_bit(KindOf::Vehicle), 9);
    assert_eq!(ScoreKeeper::score_kindof_retail_bit(KindOf::Aircraft), 10);

    let vehicle_bits = 1u64 << 9;
    let aircraft_bits = (1u64 << 9) | (1u64 << 10);
    let shrubbery_bits = 1u64 << 6;

    let mut vehicle_mask = KindOfMaskType::new();
    vehicle_mask.set(KindOf::Vehicle);
    let mut aircraft_mask = KindOfMaskType::new();
    aircraft_mask.set(KindOf::Aircraft);
    let none = KindOfMaskType::new();

    assert!(ScoreKeeper::kindof_matches_multi(
        vehicle_bits,
        &vehicle_mask,
        &aircraft_mask
    ));
    assert!(!ScoreKeeper::kindof_matches_multi(
        aircraft_bits,
        &vehicle_mask,
        &aircraft_mask
    ));
    assert!(ScoreKeeper::kindof_matches_multi(
        aircraft_bits,
        &aircraft_mask,
        &none
    ));
    assert!(!ScoreKeeper::kindof_matches_multi(
        shrubbery_bits,
        &aircraft_mask,
        &none
    ));
}

#[test]
fn score_keeper_mutators_respect_game_logic_scoring_enabled() {
    TheGameLogic::set_scoring_enabled(false);
    let mut keeper = ScoreKeeper::new_for_player(0);
    keeper.add_unit_built();
    keeper.add_building_built();
    keeper.add_object_built_obj(&TestScoreObject::unit("Humvee", 0));
    keeper.add_object_lost_obj(&TestScoreObject::unit("Humvee", 0));
    keeper.add_object_destroyed_obj(&TestScoreObject::unit("Enemy", 1));
    keeper.add_object_captured_obj(&TestScoreObject::structure("Oil", 2));
    keeper.add_money_earned(50);
    keeper.add_money_spent(25);
    assert_eq!(keeper.get_total_units_built(), 0);
    assert_eq!(keeper.get_total_buildings_built(), 0);
    assert_eq!(keeper.get_total_units_lost(), 0);
    assert_eq!(keeper.get_total_units_destroyed(), 0);
    assert_eq!(keeper.faction_buildings_captured, 0);
    assert_eq!(keeper.get_total_money_earned(), 50);
    assert_eq!(keeper.get_total_money_spent(), 25);
    TheGameLogic::set_scoring_enabled(true);
}

#[test]
fn add_object_built_template_counts_score_vehicle_into_objects_built() {
    TheGameLogic::set_scoring_enabled(true);
    let mut keeper = ScoreKeeper::new_for_player(0);
    let vehicle_score = (1u64 << 9) | (1u64 << 35);
    keeper.add_object_built_template("AmericaVehicleDozer", vehicle_score);
    assert_eq!(keeper.get_total_units_built(), 1);
    assert_eq!(keeper.get_total_objects_built("AmericaVehicleDozer"), 1);
    let civilian = 1u64 << 8; // infantry, no SCORE
    keeper.add_object_built_template("Civilian", civilian);
    assert_eq!(keeper.get_total_units_built(), 1);
}

struct TestBountyVictim {
    cost: Int,
    under_construction: Bool,
}

impl game_engine::common::rts::player::BountyObject for TestBountyVictim {
    fn calc_cost_to_build(&self) -> i32 {
        self.cost
    }
    fn is_under_construction(&self) -> bool {
        self.under_construction
    }
}

#[test]
fn do_bounty_for_kill_obj_uses_calc_cost_to_build_and_score_keeper() {
    let mut player = Player::new(0);
    player.set_cash_bounty(0.20);
    let victim = TestBountyVictim {
        cost: 1000,
        under_construction: false,
    };
    let bounty = player.do_bounty_for_kill_obj(&victim, &victim);
    assert_eq!(bounty, 200);
    assert_eq!(player.get_score_keeper().get_total_money_earned(), 200);
    assert_eq!(player.get_money().count_money(), 200);
}
