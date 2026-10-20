//! Actual synthesized definition -> owned object -> construction command admission.
//! C++ ActionManager.cpp439 requires DOZER, not a name or the local Worker bit.
use super::*;
use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use glam::Vec3;

fn world(team: Team) -> GameLogic {
    let mut world = GameLogic::new();
    let mut player = Player::new(0, team, "ConstructorOwner", true);
    player.resources.supplies = 1000;
    player.resources.power = 100;
    player.power_available = 100;
    world.add_player(player);
    let mut scaffold = ThingTemplate::new("CatalogScaffold");
    scaffold
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::Immobile)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0)
        .set_cost(300, 0);
    scaffold.build_time = 0.1;
    scaffold.always_visible = true;
    world.templates.insert(scaffold.name.clone(), scaffold);
    world
}

fn construct(world: &mut GameLogic, builder: ObjectId) -> CommandResult {
    CommandExecutor::new(world, 0)
        .execute_command(GameCommand {
            command_type: CommandType::DozerConstruct {
                template_name: "CatalogScaffold".into(),
                location: Vec3::new(60.0, 0.0, 60.0),
                orientation: 0.25,
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::UNIX_EPOCH,
            selected_units: vec![builder],
            modifier_keys: ModifierKeys::default(),
        })
        .unwrap()
}

fn assert_constructor_admission(world: &mut GameLogic, name: &str) {
    let builder = world
        .create_object_for_player(name, 0, Vec3::new(55.0, 0.0, 55.0))
        .unwrap();
    let object = world.host_object(builder).unwrap();
    assert!(
        object.is_kind_of(KindOf::Dozer),
        "synthesized constructor {name} must author DOZER"
    );
    assert!(
        object.can_construct(),
        "actual {name} construction permission"
    );
    assert!(object.can_repair(), "actual {name} repair permission");
    assert_eq!(
        world.unit_owner_if_can_construct(builder),
        Some((0, object.team))
    );
    let cash = world.get_player(0).unwrap().effective_supplies();
    assert_eq!(
        construct(world, builder),
        CommandResult::Success,
        "actual {name} DozerConstruct"
    );
    assert_eq!(
        world.get_player(0).unwrap().effective_supplies(),
        cash - 300
    );
    let site = world
        .host_objects()
        .values()
        .find(|o| o.template_name == "CatalogScaffold")
        .unwrap();
    assert!(site.status.under_construction);
    assert_eq!(site.owner_player_id, Some(0));
    assert_eq!(site.builder_id, Some(builder));
    assert_eq!(site.producer_id, Some(builder));
    assert_eq!(site.health.current, 1.0);
}

#[test]
fn synthesized_starting_catalog_builders_admit_actual_construction() {
    for (team, name) in [
        (Team::USA, "USA_Dozer"),
        (Team::China, "China_Dozer"),
        (Team::GLA, "GLA_Worker"),
    ] {
        let mut owner = world(team);
        // This existing public starter catalog is independently callable; do
        // not mislabel its definition test as the native content-load path.
        owner
            .templates
            .extend(crate::game_logic::units::create_unit_templates());
        assert_constructor_admission(&mut owner, name);
    }
}

#[test]
fn ensured_faction_catalog_builders_admit_actual_construction() {
    for (team, name) in [
        (Team::USA, "USA_Dozer"),
        (Team::USA, "AmericaVehicleDozer"),
        (Team::GLA, "GLAInfantryWorker"),
    ] {
        let mut owner = world(team);
        owner.ensure_ai_faction_templates(team);
        assert_constructor_admission(&mut owner, name);
    }
}

#[test]
fn constructor_admission_does_not_promote_worker_harvester_or_names() {
    for (name, kind) in [
        ("DozerWorkerNamedOnly", KindOf::Worker),
        ("DozerHarvesterNamedOnly", KindOf::Harvester),
    ] {
        let mut owner = world(Team::USA);
        let mut template = ThingTemplate::new(name);
        template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(kind)
            .add_kind_of(KindOf::Selectable)
            .set_health(200.0);
        owner.templates.insert(name.into(), template);
        let id = owner
            .create_object_for_player(name, 0, Vec3::new(55.0, 0.0, 55.0))
            .unwrap();
        let object = owner.host_object(id).unwrap();
        assert!(object.can_move());
        assert!(!object.can_construct());
        assert!(!object.can_repair());
        assert_eq!(construct(&mut owner, id), CommandResult::InvalidCommand);
        assert_eq!(owner.get_player(0).unwrap().effective_supplies(), 1000);
        assert!(
            !owner
                .host_objects()
                .values()
                .any(|o| o.template_name == "CatalogScaffold")
        );
    }
}

#[test]
fn same_id_constructor_admission_preserves_peer_cash_and_task() {
    let mut owner = world(Team::USA);
    let mut peer = world(Team::USA);
    owner.ensure_ai_faction_templates(Team::USA);
    peer.ensure_ai_faction_templates(Team::USA);
    let id = owner
        .create_object_for_player("AmericaVehicleDozer", 0, Vec3::new(55.0, 0.0, 55.0))
        .unwrap();
    let peer_id = peer
        .create_object_for_player("AmericaVehicleDozer", 0, Vec3::new(55.0, 0.0, 55.0))
        .unwrap();
    assert_eq!(id, peer_id);
    let peer_count = peer.host_objects().len();
    assert_eq!(construct(&mut owner, id), CommandResult::Success);
    assert_eq!(owner.get_player(0).unwrap().effective_supplies(), 700);
    assert_eq!(peer.get_player(0).unwrap().effective_supplies(), 1000);
    assert_eq!(peer.host_objects().len(), peer_count);
    assert_eq!(
        peer.host_object(peer_id).unwrap().dozer_task_build_target,
        None
    );
    assert_eq!(construct(&mut peer, peer_id), CommandResult::Success);
    assert_eq!(peer.get_player(0).unwrap().effective_supplies(), 700);
    assert_eq!(owner.get_player(0).unwrap().effective_supplies(), 700);
}

#[test]
fn ensured_faction_catalog_does_not_reclassify_existing_authored_definition() {
    let mut owner = world(Team::USA);
    let mut template = ThingTemplate::new("AmericaVehicleDozer");
    template.add_kind_of(KindOf::Vehicle).set_health(200.0);
    owner.templates.insert(template.name.clone(), template);
    owner.ensure_ai_faction_templates(Team::USA);
    let object = owner
        .create_object_for_player("AmericaVehicleDozer", 0, Vec3::ZERO)
        .unwrap();
    assert!(!owner.host_object(object).unwrap().is_kind_of(KindOf::Dozer));
    assert!(!owner.host_object(object).unwrap().can_construct());
}
