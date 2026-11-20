//! Actual synthesized definition -> owned object -> construction command admission.
//! C++ ActionManager.cpp439 requires DOZER, not a name or the local Worker bit.
use super::*;
use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use glam::Vec3;

/// The retail constructors, authored as INI (Object.ini `Dozer`/`Worker`
/// DozerAIUpdate + locomotor + CommandSet) and admitted before NewGame, plus
/// the structure their CommandSet builds. C++ ActionManager.cpp:439 and
/// BuildAssistant::buildObjectNow only admit a DOZER whose live CommandSet
/// offers the template and whose AI is a DozerAIUpdate.
const CATALOG_ACTIONS: &str = "CommandButton CatalogScaffoldBuildButton\n Command = DOZER_CONSTRUCT\n Object = CatalogScaffold\nEnd\nCommandSet CatalogScaffoldBuildSet\n 1 = CatalogScaffoldBuildButton\nEnd\n";

fn catalog_builder(name: &str, kinds: &str) -> crate::fixture_content::FixtureObject {
    crate::fixture_content::FixtureObject::new(name)
        .kind_of(kinds)
        .health(300.0)
        .build_cost(1000)
        .field("CommandSet = CatalogScaffoldBuildSet")
        .walker()
        .dozer()
}

fn standard_objects() -> Vec<crate::fixture_content::FixtureObject> {
    standard_objects_without("")
}

fn standard_objects_without(skip: &str) -> Vec<crate::fixture_content::FixtureObject> {
    use crate::fixture_content::FixtureObject;
    let mut objects = vec![FixtureObject::new("CatalogScaffold")
        .kind_of("STRUCTURE IMMOBILE SELECTABLE ALWAYS_VISIBLE")
        .health(1000.0)
        .build_cost(300)
        .build_time(0.1)];
    for (name, kinds) in [
        ("USA_Dozer", "VEHICLE DOZER SELECTABLE"),
        ("AmericaVehicleDozer", "VEHICLE DOZER SELECTABLE"),
        ("China_Dozer", "VEHICLE DOZER SELECTABLE"),
        ("GLA_Worker", "INFANTRY DOZER SELECTABLE"),
        ("GLAInfantryWorker", "INFANTRY DOZER SELECTABLE"),
    ] {
        if name != skip {
            objects.push(catalog_builder(name, kinds));
        }
    }
    objects
}

fn world(team: Team) -> GameLogic {
    world_from(team, standard_objects())
}

/// Content admission runs once per world and precedes every Object, so a test
/// that needs a different authored roster builds its world from that roster.
fn world_from(team: Team, objects: Vec<crate::fixture_content::FixtureObject>) -> GameLogic {
    let mut world = GameLogic::new();
    crate::fixture_content::admit_fixture_content_with_actions(
        &mut world,
        &objects,
        &[],
        CATALOG_ACTIONS,
    );
    let mut player = Player::new(0, team, "ConstructorOwner", true);
    player.resources.supplies = 1000;
    player.resources.power = 100;
    player.power_available = 100;
    world.add_player(player);
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
    assert_eq!(world.unit_owner_if_can_construct(builder),
        Some((0, object.team))
    );
    let cash = world.get_player(0).unwrap().effective_supplies();
    assert_eq!(
        construct(world, builder), CommandResult::Success, "actual {name} DozerConstruct");
    assert_eq!(world.get_player(0).unwrap().effective_supplies(),
        cash - 300
    );
    let site = world
        .host_objects()
        .registered_values()
        .find(|o| o.template_name == "CatalogScaffold")
        .unwrap();
    assert!(site.status.under_construction);
    assert_eq!(site.owner_player_id, Some(0));
    assert_eq!(site.builder_id, Some(builder));
    assert_eq!(site.producer_id, Some(builder));
    assert_eq!(site.health.current, 1.0);
}

#[test]
fn authored_starting_catalog_builders_admit_actual_construction() {
    for (team, name) in [
        (Team::USA, "USA_Dozer"),
        (Team::China, "China_Dozer"),
        (Team::GLA, "GLA_Worker"),
    ] {
        let mut owner = world(team);
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
    // C++ ActionManager.cpp:439 needs a DozerAIUpdate builder: neither a name
    // containing Dozer/Worker nor a HARVESTER kind promotes a mobile vehicle.
    for (name, kinds) in [
        ("DozerWorkerNamedOnly", "VEHICLE SELECTABLE"),
        ("DozerHarvesterNamedOnly", "VEHICLE HARVESTER SELECTABLE"),
    ] {
        let mut objects = standard_objects();
        objects.push(
            crate::fixture_content::FixtureObject::new(name)
                .kind_of(kinds)
                .health(200.0)
                .walker(),
        );
        let mut owner = world_from(Team::USA, objects);
        let id = owner
            .create_object_for_player(name, 0, Vec3::new(55.0, 0.0, 55.0))
            .unwrap();
        let object = owner.host_object(id).unwrap();
        assert!(object.can_move());
        assert!(!object.can_construct());
        assert!(!object.can_repair());
        assert!(construct(&mut owner, id) == CommandResult::InvalidCommand);
        assert_eq!(owner.get_player(0).unwrap().effective_supplies(), 1000);
        assert!(
            !owner
                .host_objects()
                .registered_values()
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
    let peer_count = peer.host_objects().registered_len();
    assert!(construct(&mut owner, id) == CommandResult::Success);
    assert_eq!(owner.get_player(0).unwrap().effective_supplies(), 700);
    assert_eq!(peer.get_player(0).unwrap().effective_supplies(), 1000);
    assert_eq!(peer.host_objects().registered_len(), peer_count);
    assert_eq!(
        peer.host_object(peer_id).unwrap().dozer_task_build_target(),
        None
    );
    assert!(construct(&mut peer, peer_id) == CommandResult::Success);
    assert_eq!(peer.get_player(0).unwrap().effective_supplies(), 700);
    assert_eq!(owner.get_player(0).unwrap().effective_supplies(), 700);
}

#[test]
fn constructor_uses_the_last_equivalent_button_before_checking_buildability() {
    let mut objects = standard_objects();
    objects.push(
        crate::fixture_content::FixtureObject::new("CatalogScaffoldVariant")
            .kind_of("STRUCTURE IMMOBILE SELECTABLE")
            .health(1000.0)
            .field("Buildable = No")
            .field("BuildVariations = CatalogScaffold"),
    );
    let mut world = world_from(Team::USA, objects);
    world
        .admit_fixture_command_action_source(
            "CommandButton CatalogVariantButton\n Command = DOZER_CONSTRUCT\n Object = CatalogScaffoldVariant\nEnd\n\
             CommandSet CatalogVariantSet\n 1 = CatalogScaffoldBuildButton\n 2 = CatalogVariantButton\nEnd\n",
        )
        .unwrap();
    let builder = world
        .create_object_for_player("AmericaVehicleDozer", 0, Vec3::new(55.0, 0.0, 55.0))
        .unwrap();
    world.host_object_mut(builder).unwrap().command_set_override = Some("CatalogVariantSet".into());

    assert_eq!(construct(&mut world, builder), CommandResult::InvalidCommand);
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 1000);

    world.set_control_bar_override("CatalogVariantSet", 1, None);
    assert_eq!(construct(&mut world, builder), CommandResult::Success);
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 700);
}

#[test]
fn ensured_faction_catalog_does_not_reclassify_existing_authored_definition() {
    let mut objects = standard_objects_without("AmericaVehicleDozer");
    objects.push(
        crate::fixture_content::FixtureObject::new("AmericaVehicleDozer")
            .kind_of("VEHICLE SELECTABLE")
            .health(200.0)
            .walker(),
    );
    let mut owner = world_from(Team::USA, objects);
    owner.ensure_ai_faction_templates(Team::USA);
    let object = owner
        .create_object_for_player("AmericaVehicleDozer", 0, Vec3::ZERO)
        .unwrap();
    assert!(!owner.host_object(object).unwrap().is_kind_of(KindOf::Dozer));
    assert!(!owner.host_object(object).unwrap().can_construct());
}

#[test]
fn synthesized_builders_author_dozer_kindof_not_name() {
    // hq-v16h8: C++ ActionManager requires KindOf::Dozer, never a name peel.
    for (team, name) in [
        (Team::USA, "USA_Dozer"),
        (Team::USA, "AmericaVehicleDozer"),
        (Team::China, "China_Dozer"),
        (Team::GLA, "GLAInfantryWorker"),
        (Team::GLA, "GLA_Worker"),
    ] {
        let mut owner = world(team);
        owner.ensure_ai_faction_templates(team);
        owner
            .templates
            .extend(crate::game_logic::units::create_unit_templates());
        let id = owner
            .create_object_for_player(name, 0, Vec3::new(10.0, 0.0, 10.0))
            .unwrap_or_else(|| panic!("spawn {name}"));
        let object = owner.host_object(id).unwrap();
        assert!(
            object.is_kind_of(KindOf::Dozer),
            "{name} must author KindOf::Dozer"
        );
        assert!(object.can_construct(), "{name} can_construct via Dozer kind");
        assert!(
            !name.to_ascii_lowercase().contains("dozer") || object.is_kind_of(KindOf::Dozer),
            "name alone is not the gate"
        );
    }
}

