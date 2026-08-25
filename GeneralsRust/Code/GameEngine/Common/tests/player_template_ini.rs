use game_engine::common::game_common::VeterancyLevel;
use game_engine::common::ini::ini::INI;
use game_engine::common::language::Language;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::player_template::{
    MAX_MP_STARTING_UNITS, PlayerTemplate, get_player_template_store, get_player_template_store_mut,
};
use std::sync::Mutex;

static TEST_LOCK: Mutex<()> = Mutex::new(());

fn reset_player_template_test_state() {
    Language::clear_localized_strings();
    get_player_template_store_mut().clear();
}

fn parse_player_templates(source: &str) {
    let mut ini = INI::new();
    ini.with_inline_source(source, |ini| {
        ini.parse_current_file()?;
        Ok(())
    })
    .expect("inline PlayerTemplate should parse via registered block");
}

#[test]
fn player_template_display_name_translates_full_label_token() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_player_template_test_state();
    Language::register_localized_string("GUI:America", "United States");
    Language::register_localized_string("INI:FactionChina", "China");

    parse_player_templates(
        r#"
PlayerTemplate FactionAmerica
  DisplayName = GUI:America
End

PlayerTemplate FactionChina
  DisplayName = INI:FactionChina
End
"#,
    );

    {
        let store = get_player_template_store();
        let template = store
            .find_template("FactionAmerica")
            .expect("template should be stored");
        assert_eq!(template.display_name, "United States");
        let template = store
            .find_template("FactionChina")
            .expect("second template should be stored");
        assert_eq!(template.display_name, "China");
    }

    reset_player_template_test_state();
}

#[test]
fn player_template_reparse_same_name_updates_existing_store_len_stays_one() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_player_template_test_state();

    parse_player_templates(
        r#"
PlayerTemplate FactionAmerica
  Side = America
  PlayableSide = Yes
  StartMoney = 1000
  StartingBuilding = AmericaCommandCenter
  StartingUnit0 = AmericaVehicleDozer
End

PlayerTemplate FactionAmerica
  StartMoney = 5000
  StartingUnit0 = AmericaInfantryRanger
End
"#,
    );

    {
        let store = get_player_template_store();
        assert_eq!(
            store.len(),
            1,
            "C++ findPlayerTemplate + initFromINI must not push a second template"
        );
        let template = store
            .find_template("FactionAmerica")
            .expect("re-parsed template should still be stored under the same name");
        assert_eq!(template.starting_money.count_money(), 5000);
        assert_eq!(template.starting_units[0], "AmericaInfantryRanger");
        // Fields omitted from the second block stay from the first (in-place initFromINI).
        assert_eq!(template.starting_building, "AmericaCommandCenter");
        assert_eq!(template.side, "America");
        assert!(template.playable);
    }

    reset_player_template_test_state();
}

#[test]
fn player_template_faction_america_like_fields_round_trip_from_store() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_player_template_test_state();

    parse_player_templates(
        r#"
PlayerTemplate FactionAmerica
  Side              = America
  BaseSide          = USA
  PlayableSide      = Yes
  StartMoney        = 0
  StartingBuilding  = AmericaCommandCenter
  StartingUnit0     = AmericaVehicleDozer
End
"#,
    );

    {
        let store = get_player_template_store();
        let template = store
            .find_template("FactionAmerica")
            .expect("FactionAmerica should be registered by parse_current_file");
        assert_eq!(template.side, "America");
        assert_eq!(template.base_side, "USA");
        assert!(template.playable);
        assert!(template.is_playable_side());
        assert_eq!(template.starting_money.count_money(), 0);
        assert_eq!(template.starting_building, "AmericaCommandCenter");
        assert_eq!(template.starting_units[0], "AmericaVehicleDozer");
        assert_eq!(template.get_starting_unit(0), "AmericaVehicleDozer");
        assert_eq!(template.get_starting_unit(9), "");
        assert_eq!(template.get_starting_unit(-1), "");
        assert_eq!(template.get_starting_unit(MAX_MP_STARTING_UNITS as i32), "");
    }

    reset_player_template_test_state();
}

#[test]
fn player_template_ini_parse_starting_units_money_production_and_store() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_player_template_test_state();

    parse_player_templates(
        r#"
PlayerTemplate FactionAmerica
  Side = America
  BaseSide = America
  PlayableSide = Yes
  StartMoney = 10000
  PreferredColor = R:0 G:0 B:255
  StartingBuilding = AmericaCommandCenter
  StartingUnit0 = AmericaRanger
  StartingUnit1 = AmericaDozer
  ProductionCostChange = AmericaTank 80%
  ProductionTimeChange = AmericaTank 50%
  ProductionVeterancyLevel = AmericaRanger VETERAN
  IsObserver = No
  OldFaction = Yes
End

PlayerTemplate FactionChina
  Side = China
  PlayableSide = Yes
  StartMoney = 8000
  StartingUnit0 = ChinaRedguard
  IsObserver = No
  OldFaction = Yes
End

PlayerTemplate FactionGLA
  Side = GLA
  PlayableSide = Yes
End

PlayerTemplate FactionAmericaAirForce
  Side = America
  PlayableSide = Yes
End

PlayerTemplate FactionObserver
  Side = Observer
  PlayableSide = No
  IsObserver = Yes
  OldFaction = No
End

PlayerTemplate FactionBoss
  Side = Boss
  PlayableSide = Yes
  IsObserver = No
End
"#,
    );

    {
        let store = get_player_template_store();
        assert_eq!(store.get_player_template_count(), 6);

        let america = store
            .find_template("FactionAmerica")
            .expect("FactionAmerica stored");

        assert_eq!(america.get_starting_unit(0), "AmericaRanger");
        assert_eq!(america.get_starting_unit(1), "AmericaDozer");
        assert_eq!(america.get_starting_unit(9), "");
        assert_eq!(america.get_starting_unit(-1), "");
        assert_eq!(america.get_starting_unit(10), "");
        assert_eq!(america.get_starting_building(), "AmericaCommandCenter");

        assert_eq!(america.get_money().count_money(), 10000);
        assert_eq!(america.get_preferred_color(), 0x0000FF);

        let tank_key = NameKeyGenerator::name_to_key("AmericaTank");
        let ranger_key = NameKeyGenerator::name_to_key("AmericaRanger");
        assert_eq!(
            america
                .get_production_cost_changes()
                .get(&tank_key)
                .copied(),
            Some(0.8)
        );
        assert_eq!(
            america
                .get_production_time_changes()
                .get(&tank_key)
                .copied(),
            Some(0.5)
        );
        assert_eq!(
            america
                .get_production_veterancy_levels()
                .get(&ranger_key)
                .copied(),
            Some(VeterancyLevel::Veteran)
        );

        assert!(america.is_playable_side());
        assert!(!america.is_observer());
        assert!(america.is_old_faction());

        let observer = store
            .find_template("FactionObserver")
            .expect("FactionObserver stored");
        assert!(observer.is_observer());
        assert!(!observer.is_playable_side());
        assert!(!observer.is_old_faction());

        let boss = store
            .find_template("FactionBoss")
            .expect("FactionBoss stored");
        assert!(boss.is_playable_side());
        assert!(!boss.is_playable_side_excluding_boss());

        let america_key = NameKeyGenerator::name_to_key("FactionAmerica");
        let found = store
            .find_player_template(america_key)
            .expect("find by namekey");
        assert_eq!(found.get_name_key(), america.get_name_key());
        assert_eq!(found.get_name(), "FactionAmerica");

        let remapped = store
            .find_player_template(NameKeyGenerator::name_to_key("FactionAmericaTankCommand"))
            .expect("old America namekey remaps");
        assert_eq!(remapped.get_name(), "FactionAmerica");

        assert_eq!(store.get_template_num_by_name("factionamerica"), 0);
        assert_eq!(store.get_template_num_by_name("FactionChina"), 1);
        assert_eq!(store.get_template_num_by_name("missing"), -1);

        assert!(store.get_nth_player_template_signed(-1).is_none());
        assert!(store.get_nth_player_template(0).is_some());
        assert!(store.get_nth_player_template(99).is_none());

        let mut sides = Vec::new();
        store.get_all_side_strings(&mut sides);
        assert_eq!(sides, vec!["America", "China", "GLA", "Observer", "Boss"]);
    }

    reset_player_template_test_state();
}

#[test]
fn parse_player_template_definition_finds_or_creates_by_namekey() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_player_template_test_state();

    parse_player_templates(
        r#"
PlayerTemplate FactionAmerica
  Side = America
  StartMoney = 1000
  StartingUnit0 = AmericaRanger
End

PlayerTemplate FactionAmerica
  Side = America
  StartMoney = 2500
  StartingUnit0 = AmericaMissileDefender
  StartingUnit1 = AmericaDozer
End
"#,
    );

    {
        let store = get_player_template_store();
        assert_eq!(store.get_player_template_count(), 1);
        let america = store
            .find_player_template(NameKeyGenerator::name_to_key("FactionAmerica"))
            .expect("namekey find after redefinition");
        assert_eq!(america.get_money().count_money(), 2500);
        assert_eq!(america.get_starting_unit(0), "AmericaMissileDefender");
        assert_eq!(america.get_starting_unit(1), "AmericaDozer");
        // PlayableSide omitted in both blocks → C++ ctor default `m_playableSide = false`.
        assert!(!america.playable);
        assert!(!america.is_playable_side());
    }

    reset_player_template_test_state();
}

#[test]
fn player_template_omitted_playable_side_stays_false() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_player_template_test_state();

    parse_player_templates(
        r#"
PlayerTemplate FactionCivilian
  Side = Civilian
  StartMoney = 0
  StartingUnit0 = CivilianTruck
End
"#,
    );

    {
        let store = get_player_template_store();
        let civilian = store
            .find_template("FactionCivilian")
            .expect("FactionCivilian stored");
        assert!(!civilian.playable);
        assert!(!civilian.is_playable_side());
        assert_eq!(civilian.get_starting_unit(0), "CivilianTruck");
        assert_eq!(civilian.get_starting_unit(9), "");
        assert_eq!(civilian.get_starting_unit(-1), "");
        assert_eq!(civilian.get_starting_unit(MAX_MP_STARTING_UNITS as i32), "");
        assert!(civilian.get_intrinsic_sciences().is_empty());
        assert_eq!(civilian.get_intrinsic_science_purchase_points(), 0);
        assert_eq!(civilian.get_special_power_shortcut_button_count(), 0);
        assert!(civilian.get_production_cost_changes().is_empty());
    }

    reset_player_template_test_state();
}

#[test]
fn player_template_ini_parses_sciences_production_and_shortcut_fields() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_player_template_test_state();

    parse_player_templates(
        r#"
PlayerTemplate FactionAmericaAirForceGeneral
  Side = America
  BaseSide = America
  PlayableSide = Yes
  IntrinsicSciences = SCIENCE_AMERICA SCIENCE_AirForce
  IntrinsicSciencePurchasePoints = 1
  PurchaseScienceCommandSetRank1 = SCIENCE_AMERICA_RANK1
  PurchaseScienceCommandSetRank3 = SCIENCE_AMERICA_RANK3
  PurchaseScienceCommandSetRank8 = SCIENCE_AMERICA_RANK8
  SpecialPowerShortcutCommandSet = SCIENCE_AMERICA_SHORTCUT
  SpecialPowerShortcutWinName = ControlBar.wnd:SpecialPowerShortcutUSA
  SpecialPowerShortcutButtonCount = 4
  ProductionCostChange = AmericaJetRaptor 75%
  ProductionTimeChange = AmericaJetRaptor 50%
  ProductionVeterancyLevel = AmericaJetRaptor ELITE
  ScoreScreenImage = AmericaScoreScreen
  LoadScreenMusic = Load_America
  BeaconName = AmericaBeacon
End
"#,
    );

    {
        let store = get_player_template_store();
        assert_eq!(store.get_player_template_count(), 1);
        let america = store
            .find_player_template(NameKeyGenerator::name_to_key(
                "FactionAmericaAirForceGeneral",
            ))
            .expect("namekey find");
        assert!(america.is_playable_side());
        assert_eq!(
            america.get_intrinsic_sciences(),
            ["SCIENCE_AMERICA", "SCIENCE_AirForce"]
        );
        assert_eq!(america.get_intrinsic_science_purchase_points(), 1);
        assert_eq!(
            america.get_purchase_science_command_set_rank1(),
            "SCIENCE_AMERICA_RANK1"
        );
        assert_eq!(
            america.get_special_power_shortcut_win_name(),
            "ControlBar.wnd:SpecialPowerShortcutUSA"
        );
        assert_eq!(america.get_special_power_shortcut_button_count(), 4);
        let raptor = NameKeyGenerator::name_to_key("AmericaJetRaptor");
        assert_eq!(
            america.get_production_cost_changes().get(&raptor).copied(),
            Some(0.75)
        );
        assert_eq!(
            america.get_production_time_changes().get(&raptor).copied(),
            Some(0.5)
        );
        assert_eq!(
            america
                .get_production_veterancy_levels()
                .get(&raptor)
                .copied(),
            Some(VeterancyLevel::Elite)
        );
        assert_eq!(america.get_score_screen(), "AmericaScoreScreen");
        assert_eq!(america.get_load_screen_music(), "Load_America");
        assert_eq!(america.get_beacon_template(), "AmericaBeacon");
        assert_eq!(america.get_starting_unit(-1), "");
        assert_eq!(america.get_starting_unit(10), "");
    }

    reset_player_template_test_state();
}

#[test]
fn player_template_store_init_clears_but_reset_retains() {
    let _guard = TEST_LOCK.lock().unwrap();
    reset_player_template_test_state();
    {
        let mut store = get_player_template_store_mut();
        store.add_template(PlayerTemplate::new("FactionAmerica".into()));
        store.reset();
        assert_eq!(store.get_player_template_count(), 1);
        store.init();
        assert_eq!(store.get_player_template_count(), 0);
    }
    reset_player_template_test_state();
}
