use game_engine::common::rts::{ProductionPrerequisite, ThingTemplateHandle};
use game_engine::common::thing::thing_factory::{ThingFactory, get_thing_factory};
use gamelogic::scripting::{clear_host_script_query_snapshot, merge_host_script_query_snapshot};
use generals_main::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
use glam::Vec3;

fn has_facility(world: &GameLogic) -> bool {
    world.inject_host_script_query_snapshot();
    let mut found = false;
    merge_host_script_query_snapshot(|snapshot| {
        found = snapshot.player_census["Alpha"].has_any_build_facility
    });
    clear_host_script_query_snapshot();
    found
}

#[test]
fn facility_census_resolves_handles_and_unresolved_names_for_each_world() {
    let mut factory = ThingFactory::new();
    let facility = factory.new_template("FacilityOne");
    let id = facility.get_template_id();
    let peer_id = factory.new_template("FacilityTwo").get_template_id();
    *get_thing_factory().unwrap() = Some(factory);
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "A", true).with_script_identity("Alpha"));
    let mut building = ThingTemplate::new("FacilityOne");
    building.add_kind_of(KindOf::Structure).set_health(100.0);
    world.templates.insert(building.name.clone(), building);
    let mut worker = ThingTemplate::new("Worker");
    let mut prereq = ProductionPrerequisite::new();
    prereq.add_unit_prereq("FacilityOne".into(), false);
    prereq
        .resolve_names_with(|_| Ok(Some(ThingTemplateHandle(id as u32))))
        .unwrap();
    worker.set_production_prerequisites(vec![prereq]);
    world.templates.insert(worker.name.clone(), worker);
    let mut peer = GameLogic::new();
    peer.add_player(Player::new(1, Team::USA, "A", true).with_script_identity("Alpha"));
    peer.templates
        .insert("FacilityOne".into(), ThingTemplate::new("FacilityOne"));
    let id = world
        .create_object("FacilityOne", Team::USA, Vec3::ZERO)
        .unwrap();
    world.host_object_mut(id).unwrap().owner_player_id = Some(1);
    assert!(has_facility(&world));
    assert!(!has_facility(&peer));
    let mut peer_building = ThingTemplate::new("FacilityTwo");
    peer_building
        .add_kind_of(KindOf::Structure)
        .set_health(100.0);
    peer.templates
        .insert(peer_building.name.clone(), peer_building);
    let mut peer_worker = ThingTemplate::new("PeerWorker");
    let mut peer_prereq = ProductionPrerequisite::new();
    peer_prereq.add_unit_prereq("FacilityTwo".into(), false);
    peer_prereq
        .resolve_names_with(|_| Ok(Some(ThingTemplateHandle(peer_id as u32))))
        .unwrap();
    peer_worker.set_production_prerequisites(vec![peer_prereq]);
    peer.templates.insert(peer_worker.name.clone(), peer_worker);
    let peer_object = peer
        .create_object("FacilityTwo", Team::USA, Vec3::ZERO)
        .unwrap();
    peer.host_object_mut(peer_object).unwrap().owner_player_id = Some(1);
    assert!(has_facility(&peer));
    world
        .templates
        .get_mut("Worker")
        .unwrap()
        .set_production_prerequisites(Vec::new());
    assert!(!has_facility(&world));
    assert!(has_facility(&peer));
    let mut unresolved = ProductionPrerequisite::new();
    unresolved.add_unit_prereq("FacilityOne".into(), false);
    world
        .templates
        .get_mut("Worker")
        .unwrap()
        .set_production_prerequisites(vec![unresolved]);
    assert!(has_facility(&world));
    *get_thing_factory().unwrap() = None;
}
