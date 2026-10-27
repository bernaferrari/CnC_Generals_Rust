//! Connected production ingress: CPP Dozer construct, ProductionUpdate, AI
//! movement/attack phases and GameState continuation. No global fixture setup.
use super::*;
use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use crate::game_logic::{ProductionExitMetadata, ProductionExitStyle};
use crate::save_load::SnapshotBuilder;

#[track_caller]
fn command(world: &mut GameLogic, units: &[ObjectId], kind: CommandType) {
    let label = format!("{kind:?}");
    let result = CommandExecutor::new(world, 0)
        .execute_command(GameCommand {
            command_type: kind,
            player_id: 0,
            command_id: 1,
            timestamp: std::time::UNIX_EPOCH,
            selected_units: units.to_vec(),
            modifier_keys: ModifierKeys::default(),
        })
        .unwrap();
    assert_eq!(result, CommandResult::Success, "actual command {label}");
}

fn frame(world: &mut GameLogic) {
    world.tick_logic_frame(LOGIC_FRAME_TIMESTEP, None, None);
}

fn authored_world(cash: u32) -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    for (id, team) in [(0, Team::USA), (1, Team::China)] {
        let mut player = Player::new(id, team, "Connected", id == 0);
        player.resources.supplies = cash;
        player.power_available = 100;
        player.resources.power = 100;
        // CPP map-side relationships, rather than faction labels, admit
        // an ordinary attack between these two owned player slots.
        player.set_map_relationship(1 - id, gamelogic::common::Relationship::Enemies);
        world.add_player(player);
    }
    admit_retail_production_energy_rules(&mut world);
    // Original Barracks requires its own CommandCenter, and the opposing
    // player must have a victory-counted base before the first real tick.
    for (name, player, position) in [
        ("AmericaCommandCenter", 0, Vec3::new(140.0, 0.0, 140.0)),
        ("ChinaCommandCenter", 1, Vec3::new(-140.0, 0.0, -140.0)),
    ] {
        let mut base = ThingTemplate::new(name);
        base.add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Immobile)
            .add_kind_of(KindOf::CommandCenter)
            .add_kind_of(KindOf::MpCountForVictory)
            .add_kind_of(KindOf::Attackable)
            .set_health(1000.0);
        base.always_visible = true;
        world.templates.insert(name.into(), base);
        world
            .create_object_for_player(name, player, position)
            .unwrap();
    }
    let mut dozer = ThingTemplate::new("ConnectedDozer");
    dozer
        .add_kind_of(KindOf::Dozer)
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(200.0);
    dozer.always_visible = true;
    dozer.set_locomotor_name(crate::game_logic::locomotor_bootstrap::AMERICA_DOZER_LOCOMOTOR);
    world.templates.insert(dozer.name.clone(), dozer);
    let mut barracks = ThingTemplate::new("AmericaBarracks");
    barracks
        .add_kind_of(KindOf::Structure)
        .add_kind_of(KindOf::FSBarracks)
        .add_kind_of(KindOf::Score)
        .add_kind_of(KindOf::Immobile)
        .add_kind_of(KindOf::MpCountForVictory)
        .add_kind_of(KindOf::Selectable)
        .set_health(1000.0)
        .set_cost(300, -10);
    barracks.build_time = 0.1;
    barracks.always_visible = true;
    // Authored QueueProductionExitUpdate: creation and natural rally points
    // are distinct, and the real egress path clears the factory footprint.
    barracks.production_exit_metadata = Some(ProductionExitMetadata {
        style: ProductionExitStyle::Queue,
        unit_create_point: [0.0, -25.0, 0.0],
        natural_rally_point: [36.0, -25.0, 0.0],
        exit_delay_frames: 9,
        allow_airborne_creation: false,
        initial_burst: 0,
        use_spawn_rally_point: false,
        grant_temporary_stealth_frames: 0,
    });
    world.templates.insert(barracks.name.clone(), barracks);
    let mut unit = ThingTemplate::new("AmericaInfantryRanger");
    unit.add_kind_of(KindOf::Infantry)
        .add_kind_of(KindOf::Selectable)
        .add_kind_of(KindOf::Attackable)
        .add_kind_of(KindOf::Score)
        .set_health(80.0)
        .set_cost(150, 0);
    unit.build_time = 0.1;
    unit.always_visible = true;
    unit.sight_range = 200.0;
    unit.set_locomotor_name(crate::game_logic::locomotor_bootstrap::BASIC_HUMAN_LOCOMOTOR);
    unit.set_primary_weapon(Weapon {
        damage: 20.0,
        range: 80.0,
        reload_time: 0.25,
        // CPP Weapon::fireWeapon without a ProjectileObject applies damage
        // synchronously; this authored gun uses the host direct-fire route.
        projectile_speed: 0.0,
        ..Weapon::default()
    });
    world.templates.insert(unit.name.clone(), unit);
    let dozer = world
        .create_object_for_player("ConnectedDozer", 0, Vec3::new(55.0, 0.0, 55.0))
        .unwrap();
    world.admit_fixture_command_action_source(
        "CommandButton ConnectedBarracksBuildButton\n Command = DOZER_CONSTRUCT\n Object = AmericaBarracks\nEnd\nCommandSet ConnectedBarracksBuildSet\n 1 = ConnectedBarracksBuildButton\nEnd\n",
    ).unwrap();
    world.host_object_mut(dozer).unwrap().command_set_override = Some("ConnectedBarracksBuildSet".into());
    (world, dozer)
}

fn construct_and_produce(world: &mut GameLogic, dozer: ObjectId) -> (ObjectId, ObjectId) {
    let structures_before = world.get_player(0).unwrap().statistics.structures_built();
    assert_eq!(
        world.unit_owner_if_can_construct(dozer),
        Some((0, Team::USA))
    );
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 1000);
    command(
        world,
        &[dozer],
        CommandType::DozerConstruct {
            template_name: "AmericaBarracks".into(),
            location: Vec3::new(60.0, 0.0, 60.0),
            orientation: 0.25,
        },
    );
    let barracks = world
        .objects
        .registered_values()
        .find(|object| object.template_name == "AmericaBarracks")
        .unwrap()
        .id;
    let site = world.host_object(barracks).unwrap();
    assert!(site.status.under_construction);
    assert_eq!(site.builder_id, Some(dozer));
    assert_eq!(site.producer_id, Some(dozer));
    assert_eq!(site.health.current, 1.0);
    assert_eq!(
        world.get_player(0).unwrap().effective_supplies(),
        1000 - 300
    );
    for _ in 0..240 {
        if !world
            .host_object(barracks)
            .unwrap()
            .status
            .under_construction
        {
            break;
        }
        frame(world);
    }
    let building = world.host_object(barracks).unwrap();
    assert!(
        !building.status.under_construction,
        "real dock/build operation must complete"
    );
    assert!(building.construction_power_admitted);
    assert_eq!(
        world.get_player(0).unwrap().statistics.structures_built(),
        structures_before + 1
    );
    command(
        world,
        &[barracks],
        CommandType::QueueUnitCreate {
            template_name: "AmericaInfantryRanger".into(),
            quantity: 1,
        },
    );
    for _ in 0..240 {
        let produced = world
            .objects
            .registered_values()
            .find(|object| object.template_name == "AmericaInfantryRanger")
            .map(|unit| (unit.id, unit.producer_id));
        if let Some((unit_id, producer_id)) = produced {
            assert_eq!(producer_id, Some(barracks));
            for _ in 0..240 {
                let unit = world.host_object(unit_id).unwrap();
                if unit.ai_state() == AIState::Idle && !unit.status.moving {
                    assert_eq!(unit.ignored_obstacle_id, None);
                    assert!(!unit.can_path_through_units);
                    return (barracks, unit_id);
                }
                frame(world);
            }
            panic!("real production egress must complete before the player move order");
        }
        frame(world);
    }
    panic!("actual production queue must create its unit");
}

#[test]
fn connected_construction_movement_combat_save_continues_without_peer_interference() {
    let (mut source, dozer) = authored_world(1000);
    let (barracks, unit) = construct_and_produce(&mut source, dozer);
    // Take the open lane away from the dozer's construction dock exit.
    command(
        &mut source,
        &[unit],
        CommandType::Move {
            destination: Vec3::new(90.0, 0.0, 20.0),
        },
    );
    let moving = source.host_object(unit).unwrap();
    assert_eq!(moving.ai_state(), AIState::Moving);
    assert!(moving.status.moving);
    let admitted_goal = *moving.movement.path.last().expect("actual admitted path");
    let arrival_band = moving
        .close_enough_dist
        .expect("authored locomotor stopping distance");
    assert!(arrival_band.is_finite() && arrival_band > 0.0);
    // CPP AIInternalMoveToState uses the accepted path's final node and
    // its locomotor stopping distance, not the unsnapped mouse coordinate.
    assert!(
        admitted_goal.distance(Vec3::new(90.0, 0.0, 20.0)) <= 4.0 * PATHFIND_CELL_SIZE_F_RESIDUAL
    );
    for _ in 0..3 {
        frame(&mut source);
    }
    let snapshotter = SnapshotBuilder::new();
    let movement_save = snapshotter.create_world_snapshot(&source).unwrap();
    let (mut receiver, receiver_dozer) = authored_world(3000);
    assert_eq!(receiver_dozer, dozer);
    snapshotter
        .restore_from_snapshot(&movement_save, &mut receiver)
        .unwrap();
    let (mut peer, peer_dozer) = authored_world(9000);
    assert_eq!(peer_dozer, dozer);
    let peer_cash = peer.get_player(0).unwrap().effective_supplies();
    for _ in 0..120 {
        frame(&mut source);
        frame(&mut peer);
        frame(&mut receiver);
        assert_eq!(
            source.host_object(unit).unwrap().get_position(),
            receiver.host_object(unit).unwrap().get_position()
        );
    }
    let arrived = source.host_object(unit).unwrap();
    assert_eq!(arrived.ai_state(), AIState::Idle);
    assert!(!arrived.status.moving);
    assert!(
        arrived.get_position().distance(admitted_goal) < arrival_band,
        "position={:?} admitted_goal={admitted_goal:?} CPP stopping band={arrival_band}",
        arrived.get_position(),
    );
    assert_eq!(peer.get_player(0).unwrap().effective_supplies(), peer_cash);
    let target = source
        .create_object_for_player("AmericaInfantryRanger", 1, Vec3::new(130.0, 0.0, 20.0))
        .unwrap();
    let receiver_target = receiver
        .create_object_for_player("AmericaInfantryRanger", 1, Vec3::new(130.0, 0.0, 20.0))
        .unwrap();
    assert_eq!(target, receiver_target);
    command(
        &mut source,
        &[unit],
        CommandType::Attack { target_id: target },
    );
    frame(&mut source);
    let combat_save = snapshotter.create_world_snapshot(&source).unwrap();
    snapshotter
        .restore_from_snapshot(&combat_save, &mut receiver)
        .unwrap();
    let mut witnessed_damage = false;
    for _ in 0..120 {
        frame(&mut source);
        frame(&mut peer);
        frame(&mut receiver);
        let left = source
            .host_object(target)
            .map(|object| (object.health.current, object.is_alive()));
        let right = receiver
            .host_object(target)
            .map(|object| (object.health.current, object.is_alive()));
        assert_eq!(
            left,
            right,
            "saved combat must continue at the same observation phase: frame={} source={:?} receiver={:?}",
            source.frame,
            source.host_object(unit).map(|o| (
                &o.attack_substate(),
                o.status.is_aiming_weapon,
                o.status.is_firing_weapon,
                o.fire_intent_count,
                &o.weapon
            )),
            receiver.host_object(unit).map(|o| (
                &o.attack_substate(),
                o.status.is_aiming_weapon,
                o.status.is_firing_weapon,
                o.fire_intent_count,
                &o.weapon
            )),
        );
        witnessed_damage |= left.is_none_or(|(health, _)| health < 80.0);
        assert_eq!(
            source.get_player(0).unwrap().effective_supplies(),
            receiver.get_player(0).unwrap().effective_supplies()
        );
    }
    assert!(
        witnessed_damage,
        "actual attack must reach damage: actor={:?} target={:?}",
        source.host_object(unit).map(|o| (
            &o.ai_state(),
            o.target,
            o.fire_intent_count,
            o.last_fire_damage,
            &o.weapon
        )),
        source
            .host_object(target)
            .map(|o| (o.health.current, &o.ai_state(), o.owner_player_id)),
    );
    assert!(
        source
            .host_object(target)
            .is_none_or(|object| !object.is_alive()),
        "actual lethal attack must reach death/cleanup"
    );
    assert_eq!(
        source.host_object(barracks).unwrap().construction_percent,
        receiver.host_object(barracks).unwrap().construction_percent
    );
    assert_eq!(peer.get_player(0).unwrap().effective_supplies(), peer_cash);
}
