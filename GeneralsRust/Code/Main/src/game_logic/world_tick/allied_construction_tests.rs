//! CPP DozerAIUpdate.cpp510-536: builder controls timing; site controls completion.
use super::*;

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.set_scoring_enabled(true);
    let mut builder = Player::new(0, Team::USA, "Builder", true);
    builder.alliance_team = 7;
    builder.map_side.handicap_build_time_buildings = 2.0;
    builder.power_produced = 0;
    builder.power_consumed = 10;
    let mut owner = Player::new(1, Team::China, "SiteOwner", false);
    owner.alliance_team = 7;
    owner.map_side.handicap_build_time_buildings = 0.25;
    owner.power_produced = 10;
    owner.power_consumed = 10;
    world.add_player(builder);
    world.add_player(owner);
    let rules = game_engine::common::ini::ini_game_data::ProductionEnergyRules::default()
        .overlay_ini_text("GameData\nLowEnergyPenaltyModifier=1\nMinLowEnergyProductionSpeed=0.5\nMaxLowEnergyProductionSpeed=1\nEnd\n")
        .unwrap();
    world.admit_production_energy_base(rules);
    let mut site = ThingTemplate::new("AlliedTimingSite");
    // ScoreKeeper.cpp97-119 counts only STRUCTURE+SCORE/SCORE_CREATE.
    site.add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::ScoreCreate)
        .set_health(120.0);
    site.build_time = 1.0;
    world.templates.insert("AlliedTimingSite".into(), site);
    let mut dozer = ThingTemplate::new("AlliedTimingDozer");
    dozer
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Dozer)
        .set_health(100.0);
    world.templates.insert("AlliedTimingDozer".into(), dozer);
    let site = world
        .create_object_under_construction_for_player("AlliedTimingSite", 1, Vec3::ZERO, 0.0)
        .unwrap();
    let builder = world
        .create_object_for_player("AlliedTimingDozer", 0, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    assert!(world.can_resume_construction_of(builder, site));
    assert!(world.resume_construction(&[builder], site));
    (world, site, builder)
}

#[test]
fn allied_resume_uses_builder_handicap_power_and_keeps_site_completion_owner() {
    let (mut first, site, builder) = world();
    let (mut peer, peer_site, _) = world();
    assert_eq!(site, peer_site);
    peer.players.get_mut(&0).unwrap().power_produced = 10;
    let peer_before = peer.host_object(peer_site).unwrap().health.current;
    let cash = (
        first.get_player(0).unwrap().resources.supplies,
        first.get_player(1).unwrap().resources.supplies,
    );
    first.frame = 1;
    first.update_construction(&[site], 1.0 / 30.0);
    let current = first.host_object(site).unwrap();
    // Int(30*2) / 0.5 = 120 builder frames, not the owner's Int(30*.25)=7.
    assert!((current.construction_percent - 1.0 / 120.0).abs() < 0.00001);
    assert!((current.health.current - 2.0).abs() < 0.00001);
    assert_eq!(current.owner_player_id, Some(1));
    assert_eq!(current.producer_id, Some(builder));
    assert_eq!(current.builder_id, Some(builder));
    assert_eq!(
        peer.host_object(peer_site).unwrap().health.current,
        peer_before
    );
    peer.frame = 1;
    peer.update_construction(&[peer_site], 1.0 / 30.0);
    assert!(
        (peer.host_object(peer_site).unwrap().construction_percent - 1.0 / 60.0).abs() < 0.00001
    );
    for frame in 2..=122 {
        first.frame = frame;
        first.update_construction(&[site], 1.0 / 30.0);
    }
    assert!(!first.host_object(site).unwrap().status.under_construction);
    assert_eq!(first.get_player(1).unwrap().statistics.structures_built, 1);
    assert_eq!(first.get_player(0).unwrap().statistics.structures_built, 0);
    assert_eq!(
        (
            first.get_player(0).unwrap().resources.supplies,
            first.get_player(1).unwrap().resources.supplies
        ),
        cash
    );
    assert!(
        peer.host_object(peer_site)
            .unwrap()
            .status
            .under_construction
    );
}

#[test]
fn exclusive_allied_builder_rejects_second_and_enemy_cannot_resume_or_contribute() {
    let (mut world, site, builder) = world();
    let second = world
        .create_object_for_player("AlliedTimingDozer", 0, Vec3::ZERO)
        .unwrap();
    assert!(!world.can_resume_construction_of(second, site));
    assert_eq!(world.host_object(site).unwrap().builder_id, Some(builder));
    world.players.get_mut(&0).unwrap().alliance_team = 8;
    assert!(!world.can_resume_construction_of(builder, site));
    let hp = world.host_object(site).unwrap().health.current;
    world.frame = 1;
    world.update_construction(&[site], 1.0 / 30.0);
    assert_eq!(world.host_object(site).unwrap().construction_percent, 0.0);
    assert_eq!(world.host_object(site).unwrap().health.current, hp);
}
