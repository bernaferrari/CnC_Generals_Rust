//! Actual admitted repair request and sole native module continuation.
use crate::game_logic::{GameLogic, ObjectId, Player, Team};
use crate::save_load::{XferData, XferLoad, XferSave};
use gamelogic::object::update::ai_update::dozer_ai_update::DozerTask;
use glam::Vec3;

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("repair-native-owner.ini".into(), "Object RepairDozer\nKindOf = VEHICLE DOZER SELECTABLE\nLocomotor = SET_NORMAL AmericaVehicleDozerLocomotor\nBody = ActiveBody Body\nMaxHealth = 200\nInitialHealth = 200\nEnd\nBehavior = DozerAIUpdate ActualRepairSource\nRepairHealthPercentPerSecond = 2\nEnd\nEnd\nObject RepairSite\nKindOf = STRUCTURE SELECTABLE\nBody = ActiveBody Body\nMaxHealth = 500\nInitialHealth = 500\nEnd\nEnd\n".into()),
    ]).unwrap()).unwrap();
    let mut player = Player::new(0, Team::USA, "RepairOwner", true);
    player.power_produced = 1;
    world.add_player(player);
    let dozer = world
        .create_object_for_player("RepairDozer", 0, Vec3::ZERO)
        .unwrap();
    let site = world
        .create_object_for_player("RepairSite", 0, Vec3::new(20., 0., 0.))
        .unwrap();
    world.host_object_mut(site).unwrap().health.current = 200.;
    (world, dozer, site)
}

#[test]
fn snapshot_round_trips_dozer_repair_task_and_continues_only_receiving_owner() {
    let (mut source, dozer, site) = world();
    let (peer, peer_dozer, peer_site) = world();
    assert_eq!((dozer, site), (peer_dozer, peer_site));
    source.frame = 88;
    source.dozer_new_task_repair(dozer, site).unwrap();
    assert_eq!(
        source.host_object(dozer).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
    assert_eq!(
        source.host_object(dozer).unwrap().dozer_tasks.requests[1].target,
        Some(site)
    );
    for _ in 0..120 {
        source.update_with_dt(1. / 30.);
        if source.host_object(dozer).unwrap().dozer_tasks.current == DozerTask::Repair {
            break;
        }
    }
    let owner = &source.host_object(dozer).unwrap().dozer_tasks;
    assert_eq!(
        owner.current,
        DozerTask::Repair,
        "actual original phase entry assigns current"
    );
    assert_eq!(owner.requests[1].order_frame, 88);
    let request = owner.requests[1].clone();
    let docks = owner.docks[1];
    let before = source.host_object(site).unwrap().health.current;
    let factory = crate::save_load::snapshot::SnapshotBuilder::new();
    let mut snapshot = factory.create_world_snapshot(&source).unwrap();
    let mut bytes = std::io::Cursor::new(Vec::new());
    snapshot.xfer(&mut XferSave::new(&mut bytes)).unwrap();
    let mut decoded = crate::save_load::snapshot::WorldSnapshot::default();
    decoded
        .xfer(&mut XferLoad::new(std::io::Cursor::new(bytes.into_inner())))
        .unwrap();
    let (mut receiving, receiving_dozer, receiving_site) = world();
    assert_eq!((dozer, site), (receiving_dozer, receiving_site));
    factory
        .restore_from_snapshot(&decoded, &mut receiving)
        .unwrap();
    let restored = &receiving.host_object(dozer).unwrap().dozer_tasks;
    assert_eq!(restored.current, DozerTask::Repair);
    assert_eq!(restored.requests[1].target, request.target);
    assert_eq!(restored.requests[1].order_frame, request.order_frame);
    for (expected, actual) in docks.iter().zip(restored.docks[1].iter()) {
        assert_eq!(actual.valid, expected.valid);
        assert_eq!(actual.location, expected.location);
    }
    assert_eq!(receiving.host_object(site).unwrap().health.current, before);
    for _ in 0..600 {
        receiving.update_with_dt(1. / 30.);
        if receiving.host_object(site).unwrap().health.current > before {
            break;
        }
    }
    assert!(
        receiving.host_object(site).unwrap().health.current > before,
        "actual receiving repair heals after native continuation"
    );
    assert_eq!(
        source.host_object(site).unwrap().health.current,
        before,
        "same-ID source remains saved"
    );
    assert_eq!(peer.host_object(peer_site).unwrap().health.current, 200.);
    assert_eq!(
        peer.host_object(peer_dozer).unwrap().dozer_tasks.current,
        DozerTask::Invalid
    );
    assert_eq!(
        peer.host_object(peer_dozer).unwrap().dozer_tasks.requests[1].target,
        None
    );
}
