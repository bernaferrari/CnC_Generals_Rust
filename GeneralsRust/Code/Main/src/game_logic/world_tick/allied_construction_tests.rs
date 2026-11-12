//! CPP ActionManager.cpp:440-485 admits an ALLIES resume; DozerAIUpdate.cpp
//! 476-536 contributes only for the site's controller, at the builder's
//! calcTimeToBuild, one logic frame at a time.
use super::*;

fn world() -> GameLogic {
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
    // ScoreKeeper.cpp97-119 counts only STRUCTURE+SCORE/SCORE_CREATE; only an
    // authored, mobile DozerAIUpdate reaches its BUILD dock (DozerAIUpdate.cpp).
    crate::fixture_content::admit_fixture_objects(
        &mut world,
        &[
            crate::fixture_content::FixtureObject::new("AlliedTimingSite")
                .kind_of("STRUCTURE SCORE_CREATE")
                .health(120.0)
                .field("BuildTime = 1.0"),
            crate::fixture_content::FixtureObject::new("AlliedTimingDozer")
                .kind_of("VEHICLE DOZER")
                .health(100.0)
                .walker()
                .dozer(),
        ],
    );
    world
}

fn resumed(world: &mut GameLogic, site_owner: u32) -> (ObjectId, ObjectId) {
    let site = world
        .create_object_under_construction_for_player("AlliedTimingSite", site_owner, Vec3::ZERO, 0.0)
        .unwrap()
        .expect("ObjectId");
    let builder = world
        .create_object_for_player("AlliedTimingDozer", 0, Vec3::new(2.0, 0.0, 0.0))
        .unwrap();
    assert!(world.can_resume_construction_of(builder, site));
    assert!(world.resume_construction(&[builder], site).expect("result"));
    (site, builder)
}

fn first_piece(world: &mut GameLogic, site: ObjectId) {
    for _ in 0..600 {
        world.update_with_dt(1.0 / 30.0);
        if world.host_object(site).unwrap().construction_percent > 0.0 {
            return;
        }
    }
    panic!("the builder never contributed a BUILD frame");
}

#[test]
fn allied_resume_is_admitted_but_build_fails_on_controller_mismatch() {
    let mut world = world();
    let (site, _) = resumed(&mut world, 1);
    let hp = world.host_object(site).unwrap().health.current;
    let cash = (
        world.get_player(0).unwrap().resources.supplies,
        world.get_player(1).unwrap().resources.supplies,
    );
    for _ in 0..300 {
        world.update_with_dt(1.0 / 30.0);
    }
    // DozerAIUpdate.cpp:478: "Somehow I have changed sides in mid build!"
    let current = world.host_object(site).unwrap();
    assert_eq!(current.construction_percent, 0.0);
    assert_eq!(current.health.current, hp);
    assert!(current.status.under_construction);
    assert_eq!(current.owner_player_id, Some(1));
    assert_eq!(world.get_player(1).unwrap().statistics.structures_built(), 0);
    assert_eq!(
        (
            world.get_player(0).unwrap().resources.supplies,
            world.get_player(1).unwrap().resources.supplies
        ),
        cash
    );
}

#[test]
fn builder_frames_use_builder_handicap_and_power() {
    let mut first = world();
    let mut peer = world();
    peer.players.get_mut(&0).unwrap().power_produced = 10;
    let (site, builder) = resumed(&mut first, 0);
    let (peer_site, _) = resumed(&mut peer, 0);
    let cash = first.get_player(0).unwrap().resources.supplies;
    first_piece(&mut first, site);
    first_piece(&mut peer, peer_site);
    let current = first.host_object(site).unwrap();
    // Int(30*2) / 0.5 = 120 frames; full power is Int(30*2) = 60.
    assert!((current.construction_percent - 1.0 / 120.0).abs() < 0.00001);
    assert!((current.health.current - 2.0).abs() < 0.0001);
    assert_eq!(current.producer_id, Some(builder));
    assert_eq!(current.builder_id, Some(builder));
    assert!(
        (peer.host_object(peer_site).unwrap().construction_percent - 1.0 / 60.0).abs() < 0.00001
    );
    for _ in 0..600 {
        if !first.host_object(site).unwrap().status.under_construction {
            break;
        }
        first.update_with_dt(1.0 / 30.0);
    }
    assert!(!first.host_object(site).unwrap().status.under_construction);
    assert_eq!(first.get_player(0).unwrap().statistics.structures_built(), 1);
    assert_eq!(first.get_player(0).unwrap().resources.supplies, cash);
}

#[test]
fn active_builder_is_exclusive_and_enemy_cannot_resume() {
    let mut world = world();
    let (site, builder) = resumed(&mut world, 0);
    let second = world
        .create_object_for_player("AlliedTimingDozer", 0, Vec3::new(-2.0, 0.0, 0.0))
        .unwrap();
    // ActionManager.cpp:475-478 rejects only once the builder's current task
    // is BUILD on this site, which DozerActionState::onEnter sets (cpp:1205).
    first_piece(&mut world, site);
    assert!(!world.can_resume_construction_of(second, site));
    assert_eq!(world.host_object(site).unwrap().builder_id, Some(builder));
    let enemy = world
        .create_object_for_player("AlliedTimingDozer", 1, Vec3::new(-2.0, 0.0, 0.0))
        .unwrap();
    world.players.get_mut(&1).unwrap().alliance_team = 8;
    assert!(!world.can_resume_construction_of(enemy, site));
}
