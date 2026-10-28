use generals_main::command_executor::CommandExecutor;
use generals_main::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use generals_main::game_logic::{GameLogic, ObjectId, Player, Team};
use generals_main::save_load::{Snapshot, SnapshotBuilder, WorldSnapshot, XferLoad, XferSave};
use glam::Vec3;
use std::io::Cursor;
use std::time::SystemTime;

fn world() -> GameLogic {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "OpenContainRally.ini".into(),
            "Object RallyTransport\n\
             KindOf = VEHICLE SELECTABLE AUTO_RALLYPOINT\n\
             Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\n\
             Behavior = TransportContain Cargo\n Slots = 4\n AllowInsideKindOf = INFANTRY\n NumberOfExitPaths = 1\n End\n\
             End\n\
             Object RallyFactory\n\
             KindOf = STRUCTURE SELECTABLE AUTO_RALLYPOINT\n\
             Body = ActiveBody BodyTag\n MaxHealth = 500\n InitialHealth = 500\n End\n\
             Behavior = DefaultProductionExitUpdate ExitTag\n End\n\
             Behavior = HealContain Cargo\n ContainMax = 4\n AllowInsideKindOf = INFANTRY\n NumberOfExitPaths = 1\n End\n\
             End\n\
             Object RallyPassenger\n KindOf = INFANTRY SELECTABLE\n\
             Body = ActiveBody BodyTag\n MaxHealth = 100\n InitialHealth = 100\n End\n End\n"
                .into(),
        )])
        .expect("admit receiving OpenContain definitions");
    world.add_player(Player::new(1, Team::USA, "Owner", true));
    world
}

#[test]
fn production_exit_rally_does_not_become_open_contain_exit_rally() {
    let mut world = world();
    let factory = world
        .create_object_for_player("RallyFactory", 1, Vec3::ZERO)
        .unwrap();
    let passenger = world
        .create_object_for_player("RallyPassenger", 1, Vec3::ZERO)
        .unwrap();
    assert!(world.host_object_mut(factory).unwrap().add_occupant(passenger));
    world.host_object_mut(passenger).unwrap().contained_by = Some(factory);
    let rally = Vec3::new(70.0, 0.0, 24.0);
    set_rally(&mut world, factory, rally);
    let owner = world.host_object(factory).unwrap();
    assert_eq!(owner.building_data.as_ref().and_then(|b| b.rally_point), Some(rally));
    assert_eq!(owner.open_contain_rally_point, None);

    let path = exit_path(&mut world, passenger, factory);
    assert!(path.len() >= 3);
    assert_ne!(path.last().copied(), Some(rally));
}

fn spawn_pair(world: &mut GameLogic) -> (ObjectId, ObjectId) {
    let container = world
        .create_object_for_player("RallyTransport", 1, Vec3::ZERO)
        .expect("receiving transport");
    let passenger = world
        .create_object_for_player("RallyPassenger", 1, Vec3::new(2.0, 0.0, 0.0))
        .expect("receiving passenger");
    assert!(
        world
            .host_object_mut(container)
            .expect("transport")
            .add_occupant(passenger)
    );
    world
        .host_object_mut(passenger)
        .expect("passenger")
        .contained_by = Some(container);
    (container, passenger)
}

fn set_rally(world: &mut GameLogic, container: ObjectId, point: Vec3) {
    let command = GameCommand {
        command_type: CommandType::SetRallyPoint { location: point },
        player_id: 1,
        command_id: 1,
        timestamp: SystemTime::now(),
        selected_units: vec![container],
        modifier_keys: ModifierKeys::default(),
    };
    assert_eq!(
        CommandExecutor::new(world, 1)
            .execute_command(command)
            .expect("SetRallyPoint command"),
        CommandResult::Success
    );
}

fn exit_path(world: &mut GameLogic, passenger: ObjectId, container: ObjectId) -> Vec<Vec3> {
    assert!(world.unit_command_exit_via_open_contain(passenger, container));
    world
        .host_object(passenger)
        .expect("exited passenger")
        .movement
        .path
        .clone()
}

#[test]
fn open_contain_rally_is_owned_by_same_id_receivers_and_restores_exit_continuation() {
    let mut first = world();
    let (first_container, first_passenger) = spawn_pair(&mut first);
    let mut peer = world();
    let (peer_container, peer_passenger) = spawn_pair(&mut peer);
    assert_eq!(
        first_container, peer_container,
        "same ObjectId across worlds"
    );
    assert_eq!(first_passenger, peer_passenger, "same passenger ObjectId");

    let first_rally = Vec3::new(70.0, 0.0, 24.0);
    let peer_rally = Vec3::new(-55.0, 0.0, 91.0);
    set_rally(&mut first, first_container, first_rally);
    set_rally(&mut peer, peer_container, peer_rally);
    assert_eq!(
        first
            .host_object(first_container)
            .unwrap()
            .open_contain_rally_point,
        Some(first_rally)
    );
    assert_eq!(
        peer.host_object(peer_container)
            .unwrap()
            .open_contain_rally_point,
        Some(peer_rally)
    );
    assert!(
        first
            .host_object(first_container)
            .unwrap()
            .building_data
            .is_none()
    );

    let builder = SnapshotBuilder::new();
    let mut saved = builder
        .create_world_snapshot(&first)
        .expect("capture first owner's container state");
    let mut bytes = Cursor::new(Vec::new());
    saved.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(Cursor::new(bytes.into_inner())))
        .unwrap();
    let mut restored = world();
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .expect("restore receiving OpenContain state");
    assert_eq!(
        restored
            .host_object(first_container)
            .unwrap()
            .open_contain_rally_point,
        Some(first_rally)
    );

    for (world, passenger, container, rally) in [
        (&mut first, first_passenger, first_container, first_rally),
        (&mut peer, peer_passenger, peer_container, peer_rally),
        (&mut restored, first_passenger, first_container, first_rally),
    ] {
        let path = exit_path(world, passenger, container);
        assert!(
            path.len() >= 4,
            "exit path retains duplicate end and rally: {path:?}"
        );
        assert_eq!(
            path[path.len() - 1],
            rally,
            "rally follows both exit-end points"
        );
        assert_eq!(path[path.len() - 2], path[path.len() - 3]);
    }

    assert_eq!(
        peer.host_object(peer_container)
            .unwrap()
            .open_contain_rally_point,
        Some(peer_rally),
        "restoring/exiting the first same-ID world cannot change the peer"
    );
}
