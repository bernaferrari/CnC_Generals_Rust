//! ActiveBody changes health synchronously (ActiveBody.cpp:497–594, 798–833).
use crate::fixture_content::{apply_fixture_damage, set_fixture_health};
use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::host_health_log::HostHealthEvent;
use crate::game_logic::{GameLogic, KindOf, ObjectId, Player, Team, ThingTemplate};
use crate::presentation_frame::{PresentationEvent, PresentationFrame};
use glam::Vec3;

fn injured_world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(0, Team::USA, "Body owner", true));
    let mut template = ThingTemplate::new("OwnerHealingVehicle");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    template.is_trainable = true;
    world.templates.insert(template.name.clone(), template);
    admit_injured_body(world)
}

pub(super) fn admit_injured_body(mut world: GameLogic) -> (GameLogic, ObjectId) {
    let id = world
        .create_object("OwnerHealingVehicle", Team::USA, Vec3::ZERO)
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    object.internal_change_health(40.0 - object.health.current);
    object.previous_health = 40.0;
    (world, id)
}

#[test]
fn fixed_frame_auto_heal_commits_before_publication() {
    let (mut world, id) = injured_world();
    world.host_object_mut(id).unwrap().default_auto_heal =
        Some(crate::game_logic::host_heal::HostDefaultAutoHealData::new());
    let before = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let frame = world.get_frame();
    world.tick_logic_frame_with_boundary(1.0 / 30.0, None, None, |_| {});
    assert_eq!(world.get_frame(), frame + 1);
    let object = world.host_object(id).unwrap();
    assert_eq!(
        (object.health.current, object.previous_health),
        (42.0, 40.0)
    );
    assert_eq!(world.health_events.snapshot_heal()[0].health, 42.0);
    world.complete_presentation_observations();
    world.complete_presentation_observations();
    let after = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert_eq!(before.objects[0].health_current, 40.0);
    assert_eq!(after.objects[0].health_current, 42.0);
    assert_eq!(
        after
            .events
            .iter()
            .filter(|event| matches!(
                event, PresentationEvent::HealApplied { target, health: 42.0 } if *target == id
            ))
            .count(),
        1
    );
}

#[test]
fn health_receipts_preserve_the_order_of_completed_body_operations() {
    for heal_first in [true, false] {
        let (mut world, id) = injured_world();
        world
            .with_host_logic_step(|owner| {
                if heal_first {
                    set_fixture_health(owner, id, 80.0);
                    apply_fixture_damage(owner, id, 10.0);
                } else {
                    apply_fixture_damage(owner, id, 10.0);
                    set_fixture_health(owner, id, 80.0);
                }
                let expected = if heal_first { 70.0 } else { 80.0 };
                assert_eq!(owner.host_authoritative_health(id), Some(expected));
            })
            .unwrap();
        let (expected, previous, post_hit) = if heal_first {
            (70.0, 80.0, 70.0)
        } else {
            (80.0, 30.0, 30.0)
        };
        let receipts = world.health_events.snapshot_ordered();
        let [first, second] = receipts.as_slice() else {
            panic!("expected two body operations: {receipts:?}");
        };
        let (health_event, hit_event) = if heal_first {
            (first, second)
        } else {
            (second, first)
        };
        let (HostHealthEvent::Heal(heal), HostHealthEvent::Damage(hit)) = (health_event, hit_event)
        else {
            panic!("unexpected order: {receipts:?}");
        };
        assert_eq!((heal.target, heal.health), (id, 80.0));
        assert_eq!(
            (hit.target, hit.amount, hit.body_observation().current),
            (id, 10.0, post_hit)
        );
        for _ in 0..2 {
            world.complete_presentation_observations();
            let object = world.host_object(id).unwrap();
            assert_eq!(
                (object.health.current, object.previous_health),
                (expected, previous)
            );
        }
        assert_eq!(world.health_events.snapshot_last_damage(), [*hit]);
        assert_eq!(world.health_events.snapshot_last_heal(), [*heal]);
        let published = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
        assert_eq!(published.objects[0].health_current, expected);
    }
}

#[test]
fn successive_body_loans_see_each_heal_and_the_damage_state_change() {
    let (mut world, id) = injured_world();
    world
        .host_object_mut(id)
        .unwrap()
        .internal_change_health(-10.0);
    world
        .with_host_logic_step(|owner| {
            set_fixture_health(owner, id, 60.0);
            assert_eq!(owner.host_authoritative_health(id), Some(60.0));
            set_fixture_health(owner, id, 90.0);
            let object = owner.host_object(id).unwrap();
            assert_eq!(
                (object.health.current, object.previous_health),
                (90.0, 60.0)
            );
            assert_eq!(object.body_damage_state, HostBodyDamageType::Pristine);
            set_fixture_health(owner, id, 190.0);
            assert_eq!(owner.host_authoritative_health(id), Some(100.0));
            assert_eq!(owner.host_object(id).unwrap().previous_health, 90.0);
        })
        .unwrap();
    assert_eq!(world.host_object_mut(id).unwrap().health.current, 100.0);
    world.complete_presentation_observations();
    world.complete_presentation_observations();
    let frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert_eq!(frame.objects[0].health_current, 100.0);
    assert_eq!(
        frame.objects[0].body_damage_state,
        HostBodyDamageType::Pristine as u8
    );
}

#[test]
fn health_publication_keeps_the_later_body_cap_and_experience() {
    let (mut world, id) = injured_world();
    set_fixture_health(&mut world, id, 90.0);
    world
        .set_owned_body_max_health_with_effects(
            id,
            60.0,
            crate::game_logic::object::MaxHealthChange::KeepCurrent,
            &mut Default::default(),
        )
        .unwrap();
    world.award_experience(id, 12.0);
    world.complete_presentation_observations();
    let frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let row = &frame.objects[0];
    assert_eq!(
        (row.id, row.health_current, row.health_max),
        (id, 60.0, 60.0)
    );
    assert_eq!(world.host_object(id).unwrap().experience.current, 12.0);
    assert!(frame.events.iter().any(|event| matches!(
        event, PresentationEvent::HealApplied { target, health: 90.0 } if *target == id
    )));
    world.complete_presentation_observations();
    assert_eq!(world.host_object(id).unwrap().health.current, 60.0);
}

#[test]
fn first_frame_for_a_new_body_keeps_its_own_effects() {
    let (mut world, original) = injured_world();
    let old = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let new = world
        .create_object("OwnerHealingVehicle", Team::USA, Vec3::X)
        .unwrap();
    set_fixture_health(&mut world, original, 80.0);
    apply_fixture_damage(&mut world, new, 10.0);
    apply_fixture_damage(&mut world, original, 10.0);
    world.award_experience(new, 12.0);
    world.complete_presentation_observations();
    world.complete_presentation_observations();
    let frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    let health: Vec<_> = frame
        .objects
        .iter()
        .map(|row| (row.id, row.health_current))
        .collect();
    assert_eq!(health, [(original, 70.0), (new, 90.0)]);
    assert_eq!(old.renderable_object_ids(), [original]);
    assert_eq!(old.objects[0].health_current, 40.0);
    assert_eq!(world.host_object(new).unwrap().experience.current, 12.0);
    let hits: Vec<_> = frame
        .events
        .iter()
        .filter_map(|event| match event {
            PresentationEvent::DamageApplied { target, amount, .. } => Some((*target, *amount)),
            _ => None,
        })
        .collect();
    assert_eq!(hits, [(new, 10.0), (original, 10.0)]);
}

#[test]
fn damage_receipt_retains_the_cap_at_the_damage_point() {
    let (mut world, id) = injured_world();
    apply_fixture_damage(&mut world, id, 1.0);
    let hits = world.health_events.snapshot_damage();
    let [hit] = hits.as_slice() else {
        panic!("expected one completed damage receipt");
    };
    assert_eq!(
        (
            hit.body_observation().current,
            hit.body_observation().maximum
        ),
        (39.0, 100.0)
    );
    world.host_object_mut(id).unwrap().set_body_max_health(50.0);
    world.complete_presentation_observations();
    assert_eq!(world.health_events.snapshot_last_damage(), [*hit]);
    let frame = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert_eq!(
        (frame.objects[0].health_current, frame.objects[0].health_max),
        (39.0, 50.0)
    );
}

#[test]
fn later_absolute_health_cannot_cancel_an_admitted_death() {
    let (mut world, id) = injured_world();
    apply_fixture_damage(&mut world, id, 100.0);
    set_fixture_health(&mut world, id, 80.0);
    world.complete_presentation_observations();
    let object = world.host_object(id).unwrap();
    assert!(object.status.on_die_started && object.status.destroyed && !object.is_alive());
    assert!(world.objects.ordered_ids().contains(&id));
    assert!(world.has_pending_destroy_work());
    let pending = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert!(pending.objects[0].destroyed);
    assert!(pending.events.iter().any(|event| matches!(
        event, PresentationEvent::DamageApplied { target, destroyed: true, .. } if *target == id
    )));
    world
        .process_destroy_list_with_effects_checked(&mut Default::default())
        .unwrap();
    assert!(world.host_object(id).is_none());
    let retired = PresentationFrame::try_build_from_logic(&mut world, 0).unwrap();
    assert!(retired.objects.is_empty());
}

#[test]
fn body_damage_marks_lethal_health_before_frame_completion() {
    let (mut world, id) = injured_world();
    let (object, health_events) = world.host_object_and_health_events_mut(id).unwrap();
    assert!(object.take_damage(999.0, 0, health_events));
    assert!(object.status.destroyed && !object.is_alive());
    assert_eq!(object.health.current, 0.0);
    assert_eq!(object.previous_health, 40.0);
    let receipt = world.health_events.snapshot_damage();
    assert_eq!(receipt.len(), 1);
    assert!(receipt[0].destroyed);
    assert_eq!(receipt[0].body_observation().current, 0.0);
}

#[test]
fn formation_speed_uses_damage_applied_by_the_same_step() {
    use crate::command_executor::CommandExecutor;
    use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
    use crate::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};
    use gamelogic::scripting::{
        HostScriptKillDeleteDamageRequest, request_host_script_kill_delete_damage,
    };
    use glam::Vec3;

    let mut logic = GameLogic::new();
    logic.add_player(Player::new(0, Team::USA, "USA", true));
    logic.scripts_loaded = true;
    let mut template = ThingTemplate::new("OwnerGroupSpeedVehicle");
    template
        .add_kind_of(KindOf::Vehicle)
        .add_kind_of(KindOf::Selectable)
        .set_health(100.0);
    logic.templates.insert(template.name.clone(), template);

    let fast = logic
        .create_object("OwnerGroupSpeedVehicle", Team::USA, Vec3::ZERO)
        .expect("fast member");
    let slow = logic
        .create_object(
            "OwnerGroupSpeedVehicle",
            Team::USA,
            Vec3::new(20.0, 0.0, 0.0),
        )
        .expect("slow member");
    {
        let object = logic.host_object_mut(fast).expect("fast");
        object.movement.max_speed = 20.0;
        object.movement.max_speed_damaged = 20.0;
        object.health.current = 100.0;
        object.health.maximum = 100.0;
        object.refresh_model_condition_bits();
    }
    {
        let object = logic.host_object_mut(slow).expect("slow");
        object.movement.max_speed = 10.0;
        object.movement.max_speed_damaged = 10.0;
        object.health.current = 100.0;
        object.health.maximum = 100.0;
        object.name = "SlowOwnerPhaseMember".into();
        object.refresh_model_condition_bits();
    }
    // A named map object enters the ScriptEngine named-object cache
    // (ScriptEngine::addObjectToCache); script requests resolve through it.
    assert!(logic.add_receiving_named_script_object(slow));

    // Use the actual command executor to establish the authored formation
    // stamp; the later Move is processed by GameLogic::process_commands.
    let formation = CommandExecutor::new(&mut logic, 0).execute_create_formation(&[fast, slow]);
    assert!(formation == CommandResult::Success);

    let before = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let old_slow = before.objects.iter().find(|row| row.id == slow).unwrap();
    // The NAMED_DAMAGE request queues on this world's own script services,
    // the queue its next ScriptEngine pass drains.
    logic.with_own_services(|| {
        request_host_script_kill_delete_damage(HostScriptKillDeleteDamageRequest::NamedDamage {
            unit: "SlowOwnerPhaseMember".into(),
            amount: 70,
        })
    });
    logic.queue_command(GameCommand {
        command_type: CommandType::Move {
            destination: Vec3::new(200.0, 0.0, 0.0),
        },
        player_id: 0,
        command_id: 1,
        timestamp: std::time::SystemTime::now(),
        selected_units: vec![fast, slow],
        modifier_keys: ModifierKeys::default(),
    });

    // The advanced update runs script damage before the queued Move computes
    // the formation's speed.
    logic.tick_logic_frame_with_boundary(1.0 / 30.0, None, None, |_| {});

    let fast_object = logic.host_objects().lookup(&fast).expect("fast after move");
    let slow_object = logic
        .host_objects()
        .lookup(&slow)
        .expect("slow after damage");
    assert_eq!(
        slow_object.health.current, 30.0,
        "named damage applied on host"
    );
    assert_eq!(
        fast_object.group_speed_factor, 1.0,
        "the really-damaged 10-speed member is excluded; healthy group speed is 20"
    );
    assert!(
        logic
            .health_events
            .snapshot_damage()
            .iter()
            .any(|event| event.target == slow),
        "the in-step script damage passed through the production damage channel"
    );

    assert_eq!(logic.host_authoritative_health(slow), Some(30.0));
    assert_eq!(old_slow.health_current, 100.0);
    logic.complete_presentation_observations();
    let after = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
    let slow_row = after.objects.iter().find(|row| row.id == slow).unwrap();
    assert_eq!(slow_row.health_current, 30.0);
}
