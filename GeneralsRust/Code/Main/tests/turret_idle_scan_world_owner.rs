//! Public Main update route for generic turret idle scanning.
//! The two sessions deliberately share an object id and are stepped
//! interleaved, so a process-wide idle-scan stream would couple their results.
use game_engine::common::random_value::SessionRandom;
use generals_main::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
use generals_main::save_load::SnapshotBuilder;
use glam::Vec3;

fn idle_scan_world(seed: u32) -> (GameLogic, generals_main::game_logic::ObjectId) {
    let mut world = GameLogic::new();
    let mut unit = ThingTemplate::new("OwnedIdleScanTurret");
    unit.set_health(100.0).add_kind_of(KindOf::Vehicle);
    world.templates.insert(unit.name.clone(), unit);
    let id = world
        .create_object("OwnedIdleScanTurret", Team::USA, Vec3::ZERO)
        .unwrap();

    // Public snapshot admission exposes the exact receiving session stream
    // without adding a test-only RNG mutator to GameLogic.
    let builder = SnapshotBuilder::new();
    let mut snapshot = builder.create_world_snapshot(&world).unwrap();
    snapshot.random_seed = u64::from(seed);
    let random = SessionRandom::default();
    random.seed_random(seed);
    snapshot.logic_rng_seed_words = random.seed_words();
    builder
        .restore_from_snapshot(&snapshot, &mut world)
        .unwrap();

    world
        .host_object_mut(id)
        .expect("created turret")
        .turret_enabled = true;
    world.set_turret_idle_scan_params(id, 0.2, 0.8, 1, 1);
    (world, id)
}

#[test]
fn ordinary_updates_keep_same_id_turret_scan_owned_by_each_world() {
    const OWNER_SEED: u32 = 0x51A7_CAFE;
    let (mut control, control_id) = idle_scan_world(OWNER_SEED);
    let (mut control_peer, control_peer_id) = idle_scan_world(0xB22D_F002);
    let (mut world, id) = idle_scan_world(OWNER_SEED);
    let (mut peer, peer_id) = idle_scan_world(0xB22D_F002);
    assert_eq!(id, peer_id);
    assert_eq!(id, control_id);
    assert_eq!(id, control_peer_id);

    // World::update drives the live Main step; post_command_update dispatches
    // tick_all_turret_state_machines in the normal combat phase. The fixed
    // interval admits the Idle -> IdleScan transition without fixture-only
    // access to tick_turret_state_machine.
    for _ in 0..3 {
        control.update();
        control_peer.update();
    }
    let builder = SnapshotBuilder::new();
    let control_rng = builder
        .create_world_snapshot(&control)
        .unwrap()
        .logic_rng_seed_words;
    let control_peer_rng = builder
        .create_world_snapshot(&control_peer)
        .unwrap()
        .logic_rng_seed_words;

    for _ in 0..3 {
        peer.update();
        world.update();
    }

    let actual = world.host_object(id).unwrap();
    let foreign = peer.host_object(peer_id).unwrap();
    assert_ne!(actual.turret_idle_scan_desired_angle_deg, 0.0);
    assert_eq!(
        actual.turret_idle_scan_desired_angle_deg,
        control
            .host_object(control_id)
            .unwrap()
            .turret_idle_scan_desired_angle_deg,
        "peer simulation must not perturb the receiving world scan"
    );
    assert_eq!(
        foreign.turret_idle_scan_desired_angle_deg,
        control_peer
            .host_object(control_peer_id)
            .unwrap()
            .turret_idle_scan_desired_angle_deg,
        "owner simulation must not perturb the peer world scan"
    );
    assert_eq!(
        builder
            .create_world_snapshot(&world)
            .unwrap()
            .logic_rng_seed_words,
        control_rng,
        "receiving world consumes the same RNG sequence with an interleaved peer"
    );
    assert_eq!(
        builder
            .create_world_snapshot(&peer)
            .unwrap()
            .logic_rng_seed_words,
        control_peer_rng,
        "peer world consumes its own same sequence during interleaved updates"
    );
}
