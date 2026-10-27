use game_engine::common::ini::INILoadType;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::player_template::PlayerTemplateStore;

#[test]
fn two_application_player_templates_keep_their_names_and_fields_across_threads() {
    let mut first = PlayerTemplateStore::new_owned();
    let mut peer = PlayerTemplateStore::new_owned();
    NameKeyGenerator::init();
    assert_eq!(NameKeyGenerator::name_to_key("before"), 1);
    first.apply_source(
        "PlayerTemplate FactionAmerica\n Side = America\n StartMoney = 1200\n ProductionCostChange = VehicleA 25%\n PurchaseScienceCommandSetRank1 = FirstRank\nEnd\n",
        INILoadType::Overwrite,
    ).unwrap();
    peer.apply_source(
        "PlayerTemplate PeerOnly\n Side = China\nEnd\nPlayerTemplate FactionAmerica\n Side = America\n StartMoney = 2400\n ProductionCostChange = VehicleA 50%\n PurchaseScienceCommandSetRank1 = PeerRank\nEnd\n",
        INILoadType::Overwrite,
    ).unwrap();
    assert_eq!(NameKeyGenerator::name_to_key("after"), 2);
    assert_eq!(first.find_template_index("FactionAmerica"), Some(0));
    assert_eq!(peer.find_template_index("FactionAmerica"), Some(1));
    let mut first = std::thread::spawn(move || {
        NameKeyGenerator::init();
        NameKeyGenerator::name_to_key("foreign");
        first
            .apply_source(
                "PlayerTemplate FactionAmerica\n StartMoney = 1600\nEnd\n",
                INILoadType::Overwrite,
            )
            .unwrap();
        first
    })
    .join()
    .unwrap();
    let template = first.find_template("FactionAmerica").unwrap();
    assert_eq!(template.get_money().count_money(), 1600);
    assert_eq!(
        template.get_purchase_science_command_set_rank1(),
        "FirstRank"
    );
    assert_eq!(
        template.get_production_cost_change_percent("VehicleA"),
        0.25
    );
    let old_alias = first.name_to_key("FactionAmericaTankCommand");
    assert_eq!(
        first.find_player_template(old_alias).unwrap().get_name(),
        "FactionAmerica"
    );
    let peer_template = peer.find_template("FactionAmerica").unwrap();
    assert_eq!(peer_template.get_money().count_money(), 2400);
    assert_eq!(
        peer_template.get_purchase_science_command_set_rank1(),
        "PeerRank"
    );
    assert_eq!(
        peer_template.get_production_cost_change_percent("VehicleA"),
        0.5
    );

    first.apply_companion_source(
        "Object Example\n Behavior = DummyBehavior ModuleTag_1\n  PlayerTemplate = FactionAmerica\n  StartMoney = 9999\n End\nEnd\nPlayerTemplate FactionAmerica\n StartMoney = 1800\nEnd\n",
    ).unwrap();
    assert_eq!(
        first
            .find_template("FactionAmerica")
            .unwrap()
            .get_money()
            .count_money(),
        1800
    );
    assert!(first.apply_companion_source(
        "PlayerTemplate FactionAmerica\n StartMoney = 7000\nEnd\nPlayerTemplate Broken\n StartMoney = 50\n"
    ).is_err());
    assert_eq!(
        first
            .find_template("FactionAmerica")
            .unwrap()
            .get_money()
            .count_money(),
        1800
    );
    assert!(first.find_template("Broken").is_none());
    assert_eq!(
        peer.find_template("FactionAmerica")
            .unwrap()
            .get_money()
            .count_money(),
        2400
    );
}
