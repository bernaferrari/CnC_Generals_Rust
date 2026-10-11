//! TMAI object-goal identity and continuation, independent of snapshot pose copies.
use super::*;
use crate::game_logic::{AIState, KindOf, Player, Team, ThingTemplate};
use crate::save_load::snapshot::SnapshotBuilder;

fn ranger_logic() -> GameLogic {
    let mut world = GameLogic::new();
    let mut template = ThingTemplate::new("Ranger");
    template.set_health(100.0).add_kind_of(KindOf::Infantry);
    world.templates.insert("Ranger".into(), template);
    world.add_player(Player::new(0, Team::USA, "USA", false));
    world
}

fn encoded_goal_tail(
    version: u32,
    ai: AiTeamPersistPayload,
    movement_runtime: Vec<MovementRuntimePersist>,
) -> Vec<u8> {
    let encoded = if version == 10 {
        bincode_legacy::serialize(&ai).unwrap()
    } else {
        bincode_legacy::serialize(&AiTeamPersistPayloadV11 {
            ai,
            movement_runtime,
        })
        .unwrap()
    };
    let mut tail = TMAI_MAGIC.to_vec();
    append_u32(&mut tail, version);
    append_u32(&mut tail, encoded.len() as u32);
    tail.extend(encoded);
    tail
}

#[test]
fn movement_goal_v11_round_trip_retains_missing_target_and_future_goal() {
    let mut source = ranger_logic();
    let id = source
        .create_object("Ranger", Team::USA, Vec3::ZERO)
        .unwrap();
    let target = source
        .create_object("Ranger", Team::USA, Vec3::new(80.0, 0.0, 20.0))
        .unwrap();
    let goal = source.host_object(target).unwrap().get_position();
    {
        let object = source.host_object_mut(id).unwrap();
        object.set_ai_state(AIState::Moving);
        object.set_movement_goal_object(Some(target));
        object.requested_destination = Some(goal);
    }
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    // Decode the actual append envelope, rather than merely cloning Object.
    let suffix = find_tmai_suffix(&snapshot.lifecycle_tail).unwrap();
    assert_eq!(u32::from_le_bytes(suffix[..4].try_into().unwrap()), 11);
    let mut restored = ranger_logic();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert_eq!(
        restored.host_object(id).unwrap().movement_goal_object(),
        Some(target)
    );
    let future = Vec3::new(190.0, 0.0, 30.0);
    restored
        .host_object_mut(target)
        .unwrap()
        .set_position(future);
    restored.refresh_movement_goal_objects(&[id]);
    assert_eq!(
        restored.host_object(id).unwrap().requested_destination,
        Some(future)
    );
    restored.objects.remove(&target);
    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &restored);
    apply_from_lifecycle_tail(&bytes, &mut restored).unwrap();
    restored.refresh_movement_goal_objects(&[id]);
    assert_eq!(
        restored.host_object(id).unwrap().movement_goal_object(),
        Some(target)
    );
    assert_eq!(
        restored.host_object(id).unwrap().requested_destination,
        Some(future)
    );
}

#[test]
fn movement_goal_v10_defaults_and_invalid_v11_receivers_reject_before_mutation() {
    let mut world = ranger_logic();
    let id = world
        .create_object("Ranger", Team::USA, Vec3::ZERO)
        .unwrap();
    world
        .host_object_mut(id)
        .unwrap()
        .set_movement_goal_object(Some(ObjectId(999)));
    world.host_object_mut(id).unwrap().path_goal_position = Some(Vec3::new(10.0, 0.0, 20.0));
    world.host_object_mut(id).unwrap().path_timestamp = 37;
    let old = encoded_goal_tail(10, capture(&world), Vec::new());
    apply_from_lifecycle_tail(&old, &mut world).unwrap();
    assert_eq!(world.host_object(id).unwrap().movement_goal_object(), None);
    assert_eq!(
        world.host_object(id).unwrap().path_goal_position,
        Some(Vec3::new(10.0, 0.0, 20.0))
    );
    assert_eq!(world.host_object(id).unwrap().path_timestamp, 37);
    let row = |receiver, goal_object| MovementRuntimePersist {
        receiver,
        goal_object,
        path_goal_position: None,
        path_timestamp: 0,
        wander_width_override: None,
    };
    for goals in [
        vec![row(id.0, Some(999)), row(id.0, Some(998))],
        vec![row(0, Some(999))],
        vec![row(u32::MAX, Some(999))],
        vec![row(id.0, Some(0))],
        vec![MovementRuntimePersist {
            path_goal_position: Some([f32::NAN, 0.0, 0.0]),
            ..row(id.0, None)
        }],
        vec![MovementRuntimePersist {
            wander_width_override: Some(f32::INFINITY),
            ..row(id.0, None)
        }],
    ] {
        world
            .host_object_mut(id)
            .unwrap()
            .set_movement_goal_object(Some(ObjectId(777)));
        let bytes = encoded_goal_tail(11, capture(&world), goals);
        assert!(apply_from_lifecycle_tail(&bytes, &mut world).is_err());
        assert_eq!(
            world.host_object(id).unwrap().movement_goal_object(),
            Some(ObjectId(777))
        );
    }
}

#[test]
fn ordinary_positional_movement_runtime_round_trips_exact_prior_path_goal() {
    let mut world = ranger_logic();
    let id = world
        .create_object("Ranger", Team::USA, Vec3::ZERO)
        .unwrap();
    {
        let mover = world.host_object_mut(id).unwrap();
        mover.set_ai_state(AIState::Moving);
        mover.requested_destination = Some(Vec3::new(90.0, 0.0, 50.0));
        mover.path_goal_position = Some(Vec3::new(70.0, 0.0, 40.0));
        mover.path_timestamp = 123;
    }
    let mut bytes = Vec::new();
    append_to_lifecycle_tail(&mut bytes, &world);
    world.host_object_mut(id).unwrap().path_goal_position = None;
    world.host_object_mut(id).unwrap().path_timestamp = 0;
    apply_from_lifecycle_tail(&bytes, &mut world).unwrap();
    let mover = world.host_object(id).unwrap();
    assert_eq!(mover.movement_goal_object(), None);
    assert_eq!(
        mover.requested_destination,
        Some(Vec3::new(90.0, 0.0, 50.0))
    );
    assert_eq!(mover.path_goal_position, Some(Vec3::new(70.0, 0.0, 40.0)));
    assert_eq!(mover.path_timestamp, 123);
}
