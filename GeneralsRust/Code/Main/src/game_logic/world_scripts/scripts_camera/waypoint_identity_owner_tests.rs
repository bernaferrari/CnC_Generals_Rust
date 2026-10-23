//! Actual MapData admission -> owned script command -> movement -> condition -> save.
use super::*;
use gamelogic::scripting::core::{Condition, ConditionType, Parameter, ParameterType};
use gamelogic::system::map_loader::{Coord3D, MapData, MapWaypoint};

fn world(terminal_labels: [&str; 3]) -> GameLogic {
    world_with_terminal(terminal_labels, 2)
}
fn world_with_terminal(terminal_labels: [&str; 3], terminal_id: u32) -> GameLogic {
    let mut world = GameLogic::new();
    world.admit_receiving_module_content(crate::assets::ReceivingObjectContent::from_sources([
        ("waypoint_identity.ini".into(),"Object WaypointWalker\nKindOf = INFANTRY SELECTABLE\nLocomotor = SET_NORMAL BasicHumanLocomotor\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nEnd\nObject WaypointAir\nKindOf = VEHICLE AIRCRAFT\nLocomotor = SET_NORMAL ComancheLocomotor\nBody = ActiveBody Body\nMaxHealth = 100\nInitialHealth = 100\nEnd\nBehavior = AIUpdateInterface AI\nEnd\nEnd\n".into())
    ]).unwrap()).unwrap();
    world.add_player(Player::new(1, Team::USA, "Waypoint Owner", true));
    world.override_world_size(256., 256.);
    let mut map = MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![0; 64 * 64];
    let row = |id, name: &str, x, label1: &str, label2: &str, label3: &str| MapWaypoint {
        id,
        name: name.into(),
        location: Coord3D::new(x, 20., 0.),
        path_label1: label1.into(),
        path_label2: label2.into(),
        path_label3: label3.into(),
        bi_directional: false,
    };
    map.waypoints = vec![
        row(1, "Start", 20., "Route", "", ""),
        row(
            terminal_id,
            "",
            40.,
            terminal_labels[0],
            terminal_labels[1],
            terminal_labels[2],
        ),
        // Equal coordinates do not turn this unrelated waypoint into the path's
        // terminal identity. The old nearest-at-last producer picked it.
        row(3, "Destination", 40., "Route", "Wrong", "Wrong"),
    ];
    map.waypoint_links.push((1, terminal_id));
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    world
}
fn actor(world: &mut GameLogic) -> ObjectId {
    let id = world
        .create_object_for_player("WaypointWalker", 1, Vec3::new(10., 0., 20.))
        .unwrap();
    world.host_object_mut(id).unwrap().name = "Walker".into();
    assert!(world.host_object(id).unwrap().has_ai_update_interface());
    id
}
fn reached(world: &mut GameLogic, path: &str) -> bool {
    let mut condition = Condition::new(ConditionType::NamedReachedWaypointsEnd);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Walker".into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            path.into(),
        ))
        .unwrap();
    world.owned_script_condition_for_test(condition)
}
fn encoded_restore(source: &GameLogic, labels: [&str; 3]) -> GameLogic {
    encoded_restore_with_terminal(source, labels, 2)
}
fn encoded_restore_with_terminal(
    source: &GameLogic,
    labels: [&str; 3],
    terminal_id: u32,
) -> GameLogic {
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(source).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    let mut restored = world_with_terminal(labels, terminal_id);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    restored
}
#[test]
fn actual_empty_named_terminal_identity_survives_pending_and_completed_encoded_continuation() {
    let mut source = world(["", "", ""]);
    let id = actor(&mut source);
    let mut peer = world(["Foreign", "Peer", "Only"]);
    let peer_id = actor(&mut peer);
    assert_eq!(id, peer_id);
    {
        let owner = source.world_services.terrain();
        let terrain = owner.read().unwrap();
        let terminal = terrain
            .get_waypoint_by_id(2)
            .expect("actual empty-name waypoint admitted");
        assert_eq!(terminal.get_name().as_str(), "");
    }
    assert!(
        !reached(&mut source, ""),
        "no completed waypoint is not an empty-labelled completion"
    );
    source.apply_owned_named_waypoint("Walker", "Route", Some(false), None);
    assert_eq!(source.host_object(id).unwrap().pending_waypoint_id, Some(2));
    assert_eq!(
        source.host_object(id).unwrap().pending_waypoint_labels,
        ["", "", ""]
    );
    assert!(
        !reached(&mut source, ""),
        "accepted pending path is not complete"
    );
    let mut restored = encoded_restore(&source, ["", "", ""]);
    assert_eq!(
        restored.host_object(id).unwrap().pending_waypoint_id,
        Some(2)
    );
    let mut finished = false;
    for _ in 1..=900 {
        for owner in [&mut source, &mut restored] {
            advance_waypoint_world(owner, id);
        }
        let a = source.host_object(id).unwrap();
        let b = restored.host_object(id).unwrap();
        assert_eq!(
            (a.pending_waypoint_id, a.completed_waypoint_id),
            (b.pending_waypoint_id, b.completed_waypoint_id)
        );
        assert_eq!(a.get_position(), b.get_position());
        if a.completed_waypoint_id == Some(2) {
            finished = true;
            break;
        }
    }
    assert!(
        finished,
        "actual locomotor must publish the terminal identity: {}",
        waypoint_motion_debug(&source, id)
    );
    assert!(reached(&mut source, ""));
    assert!(reached(&mut restored, ""));
    assert!(
        !reached(&mut source, "Route"),
        "command label is not a terminal slot"
    );
    assert!(
        !reached(&mut source, "Wrong"),
        "equal-coordinate unrelated waypoint is not the terminal"
    );
    assert!(!reached(&mut peer, ""));
    assert_eq!(
        peer.host_object(peer_id).unwrap().completed_waypoint_id,
        None
    );
    let mut completed = encoded_restore(&source, ["", "", ""]);
    assert!(reached(&mut completed, ""));
    assert_eq!(
        completed.host_object(id).unwrap().completed_waypoint_id,
        Some(2)
    );
    completed.begin_host_waypoint_ai_observations(&[id]);
    completed.update_movement_for_test(&[id], 1. / 30.);
    assert!(
        !reached(&mut completed, ""),
        "next actual AI wake clears completion pulse"
    );
}
#[test]
fn actual_exact_path_exit_publishes_original_start_identity_including_empty_slots() {
    let mut owner = world(["", "", ""]);
    let id = actor(&mut owner);
    owner.apply_owned_named_waypoint("Walker", "Route", Some(true), None);
    assert_eq!(owner.host_object(id).unwrap().pending_waypoint_id, Some(1));
    assert!(!reached(&mut owner, "Route"));
    // An actual replacement script command exits Exact, rather than manually
    // stamping a completion. CPP Exact.onExit reports its admitted start.
    owner.apply_owned_named_waypoint("Walker", "Destination", None, None);
    assert_eq!(
        owner.host_object(id).unwrap().completed_waypoint_id,
        Some(1)
    );
    assert!(reached(&mut owner, "Route"));
    assert!(reached(&mut owner, ""));
    assert_eq!(
        owner.host_object(id).unwrap().completed_waypoint_labels,
        ["Route", "", ""]
    );
}

#[test]
fn actual_exact_stop_exits_once_and_cyclic_follow_is_not_a_terminal_completion() {
    let mut owner = world(["", "", ""]);
    let id = actor(&mut owner);
    owner.apply_owned_named_waypoint("Walker", "Route", Some(true), None);
    assert!(owner.unit_command_stop(id));
    assert_eq!(
        owner.host_object(id).unwrap().completed_waypoint_id,
        Some(1)
    );
    assert_eq!(owner.host_object(id).unwrap().pending_waypoint_id, None);
    assert!(reached(&mut owner, "Route"));
    let mut map = MapData::new();
    map.width = 64;
    map.height = 64;
    map.heightmap = vec![0; 64 * 64];
    map.waypoints = vec![
        MapWaypoint {
            id: 1,
            name: "RingA".into(),
            location: Coord3D::new(20., 20., 0.),
            path_label1: "Ring".into(),
            path_label2: "".into(),
            path_label3: "".into(),
            bi_directional: false,
        },
        MapWaypoint {
            id: 2,
            name: "RingB".into(),
            location: Coord3D::new(40., 20., 0.),
            path_label1: "Ring".into(),
            path_label2: "".into(),
            path_label3: "".into(),
            bi_directional: false,
        },
    ];
    map.waypoint_links = vec![(1, 2), (2, 1)];
    owner
        .world_services
        .terrain()
        .write()
        .unwrap()
        .load_map_data(map);
    let route = owner
        .host_script_waypoint_route_from("Ring", Vec3::new(10., 0., 20.))
        .unwrap();
    assert_eq!(route.points.len(), 2);
    assert!(
        route.completion(false).is_none(),
        "visited limit is not a null C++ next pointer"
    );
    assert_eq!(route.completion(true).unwrap().id, 1);
}

#[test]
fn actual_completed_magic_id_and_magic_labels_roundtrip_at_owner_boundaries() {
    let waypoint = u32::from_le_bytes(*b"TMAI");
    let labels = ["TMAI", "prefix TMAI suffix", ""];
    let mut source = world_with_terminal(labels, waypoint);
    let id = actor(&mut source);
    source.apply_owned_named_waypoint("Walker", "Route", Some(false), None);
    assert_eq!(
        source.host_object(id).unwrap().pending_waypoint_id,
        Some(waypoint)
    );
    let mut pending = encoded_restore_with_terminal(&source, labels, waypoint);
    assert_eq!(
        pending.host_object(id).unwrap().pending_waypoint_id,
        Some(waypoint)
    );
    let mut completed = false;
    for _ in 1..=900 {
        advance_waypoint_world(&mut pending, id);
        if pending.host_object(id).unwrap().completed_waypoint_id == Some(waypoint) {
            completed = true;
            break;
        }
    }
    assert!(completed, "{}", waypoint_motion_debug(&pending, id));
    // In v12 this completed ID ends the owner payload with literal TMAI bytes.
    // Those bytes must remain data, even alongside magic within label strings.
    let mut restored = encoded_restore_with_terminal(&pending, labels, waypoint);
    assert_eq!(
        restored.host_object(id).unwrap().completed_waypoint_id,
        Some(waypoint)
    );
    assert!(reached(&mut restored, "TMAI"));
    assert!(reached(&mut restored, "prefix TMAI suffix"));
}
#[test]
fn actual_air_scatter_exits_exact_before_quick_path_overwrites_its_mode() {
    let mut owner = world(["", "", ""]);
    let mut peer = world(["", "", ""]);
    let mut spawn = |world: &mut GameLogic| {
        let id = world
            .create_object_for_player("WaypointAir", 1, Vec3::new(10., 50., 20.))
            .unwrap();
        world.host_object_mut(id).unwrap().name = "Walker".into();
        id
    };
    let id = spawn(&mut owner);
    assert_eq!(id, spawn(&mut peer));
    for world in [&mut owner, &mut peer] {
        world.apply_owned_named_waypoint("Walker", "Route", Some(true), None);
    }
    assert!(owner.host_object(id).unwrap().is_exact_path);
    assert_eq!(owner.host_object(id).unwrap().ai_state, AIState::Moving);
    assert_eq!(owner.host_object(id).unwrap().pending_waypoint_id, Some(1));
    assert!(owner.unit_command_move_to_moving(id, Vec3::new(60., 50., 20.)));
    assert_eq!(
        owner.host_object(id).unwrap().completed_waypoint_id,
        Some(1)
    );
    assert_eq!(owner.host_object(id).unwrap().pending_waypoint_id, None);
    assert!(
        !owner.host_object(id).unwrap().is_exact_path,
        "actual flying quick path was accepted"
    );
    assert_eq!(peer.host_object(id).unwrap().completed_waypoint_id, None);
    assert_eq!(peer.host_object(id).unwrap().pending_waypoint_id, Some(1));
    for frame in 1..=120 {
        owner.set_current_frame(frame);
        owner.begin_host_waypoint_ai_observations(&[id]);
        owner.update_movement_for_test(&[id], 1. / 30.);
        assert_eq!(
            owner.host_object(id).unwrap().completed_waypoint_id,
            None,
            "unrelated scatter arrival must not republish the outgoing Exact receipt"
        );
    }
}

#[test]
fn actual_retained_dead_ai_clears_completed_waypoint_on_next_movement_wake() {
    let mut source = world(["", "", ""]);
    let id = actor(&mut source);
    let mut peer = world(["", "", ""]);
    let peer_id = actor(&mut peer);
    assert_eq!(id, peer_id);
    for owner in [&mut source, &mut peer] {
        owner.apply_owned_named_waypoint("Walker", "Route", Some(false), None);
        let mut finished = false;
        for _ in 1..=900 {
            advance_waypoint_world(owner, id);
            if owner.host_object(id).unwrap().completed_waypoint_id == Some(2) {
                finished = true;
                break;
            }
        }
        assert!(
            finished,
            "actual locomotor publishes completion before death: {}",
            waypoint_motion_debug(owner, id)
        );
        assert!(reached(owner, ""));
    }
    let token = source
        .host_object(id)
        .unwrap()
        .visual_draw_identity_token
        .clone();
    let mut effects = crate::game_logic::ScriptPresentationEffects::default();
    let result = source
        .apply_owned_kill_with_effects(
            id,
            crate::game_logic::combat::DamageType::Explosive,
            crate::game_logic::host_usa_pilot::HostDeathType::Exploded,
            &mut effects,
        )
        .expect("actual receiving Body death");
    assert!(result.destroyed);
    let corpse = source
        .host_object(id)
        .expect("no DestroyDie authored: Body remains registered");
    assert!(corpse.status.on_die_started);
    assert!(!corpse.is_alive());
    assert_eq!(corpse.completed_waypoint_id, Some(2));
    assert!(
        reached(&mut source, ""),
        "completion survives until next AI observation, not death itself"
    );
    // Drive the real indexed Movement obligation, rather than clearing the receipt directly.
    source.update_with_dt(1. / 30.);
    let corpse = source
        .host_object(id)
        .expect("retained corpse is not physically removed");
    assert!(std::sync::Arc::ptr_eq(
        &token,
        &corpse.visual_draw_identity_token
    ));
    assert_eq!(corpse.completed_waypoint_id, None);
    assert!(corpse.completed_waypoint_labels.is_empty());
    assert!(!reached(&mut source, ""));
    assert_eq!(
        peer.host_object(peer_id).unwrap().completed_waypoint_id,
        Some(2)
    );
    assert!(
        reached(&mut peer, ""),
        "same-ID peer has not taken its AI wake"
    );
}

fn advance_waypoint_world(world: &mut GameLogic, id: ObjectId) {
    let before = world.get_current_frame();
    world.update_with_dt(1. / 30.);
    assert!(
        world.get_current_frame() > before,
        "real fixed step must advance: {}",
        waypoint_motion_debug(world, id)
    );
    let actor = world.host_object(id).expect("waypoint receiver");
    if actor.completed_waypoint_id.is_some() {
        assert!(
            !actor.waiting_for_path,
            "completion cannot bypass unresolved path admission"
        );
    }
}

// Failure diagnostics retain the actual receiving order, pose and path evidence.
fn waypoint_motion_debug(world: &GameLogic, id: ObjectId) -> String {
    let actor = world
        .host_object(id)
        .expect("waypoint actor remains registered");
    format!(
        "frame={} pos={:?} state={:?} loco={:?} speed={} velocity={:?} target={:?} path={:?} index={} waiting={} requested={:?} pending={:?} completed={:?}",
        world.get_current_frame(),
        actor.get_position(),
        actor.ai_state,
        actor.cur_locomotor_name,
        actor.movement.max_speed,
        actor.movement.velocity,
        actor.movement.target_position,
        actor.movement.path,
        actor.movement.current_path_index,
        actor.waiting_for_path,
        actor.requested_destination,
        actor.pending_waypoint_id,
        actor.completed_waypoint_id
    )
}

#[test]
fn actual_ordinary_waypoint_cancellation_does_not_publish_terminal_arrival() {
    let mut owner = world(["", "", ""]);
    let id = actor(&mut owner);
    owner.apply_owned_named_waypoint("Walker", "Route", Some(false), None);
    assert_eq!(owner.host_object(id).unwrap().pending_waypoint_id, Some(2));
    assert!(owner.unit_command_move_to_moving(id, Vec3::new(80., 0., 20.)));
    assert_eq!(owner.host_object(id).unwrap().pending_waypoint_id, None);
    assert_eq!(owner.host_object(id).unwrap().completed_waypoint_id, None);
    assert!(
        !reached(&mut owner, ""),
        "ordinary Follow has no completion on interrupted exit"
    );
}
