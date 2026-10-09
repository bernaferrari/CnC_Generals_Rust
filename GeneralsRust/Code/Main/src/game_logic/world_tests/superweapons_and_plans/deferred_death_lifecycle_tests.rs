//! Object::onDie (CPP Object.cpp4548–4567) and GameLogic::destroyObject
//! (GameLogic.cpp3935–3968) are distinct synchronous lifecycle operations.
use super::*;

fn parsed_slow_death_owner() -> (GameLogic, ObjectId, u32) {
    const NAME: &str = "DeferredDestroyBoundaryInfantry";
    let mut world = GameLogic::new();
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser
            .parse_ini_content(
                r#"
Object DeferredDestroyBoundaryInfantry
  KindOf = INFANTRY SELECTABLE ATTACKABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
  End
  Behavior = SlowDeathBehavior ModuleTag_Death
    SinkDelay = 0
    SinkRate = 0
    DestructionDelay = 300
    DestructionDelayVariance = 0
  End
End
"#,
                "deferred_destroy_boundary.ini"
            )
            .unwrap(),
        1
    );
    let definition = parser.get_definition(NAME).unwrap();
    let death = definition
        .behavior_modules
        .iter()
        .find(|module| module.class_name == "SlowDeathBehavior")
        .unwrap();
    let attrs: Vec<_> = death
        .attributes
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let ini = crate::game_logic::host_slow_death::slow_death_ini_from_behavior_attrs(&attrs);
    assert_eq!(ini.destruction_delay_ms, 300);
    assert_eq!(ini.destruction_delay_variance_ms, 0);
    let template = GameLogic::build_template_from_object_definition(NAME, definition, None);
    assert!(template.is_kind_of(KindOf::Infantry));
    world.templates.insert(NAME.to_string(), template);
    let id = world.create_object(NAME, Team::USA, Vec3::ZERO).unwrap();
    // This explicitly invokes the real domain entry with parsed immutable
    // rules. The current Main AssetManager-only lookup cannot install this
    // unique definition without renderer startup: this is not authored
    // AssetManager→onDie factory integration evidence.
    let frame = world.getFrame();
    // CPP SlowDeath begins from the lethal ActiveBody/onDie boundary. Exercise
    // the actual damage kernel before the parsed domain callback; no HP writes.
    let (owner, health_events) = world.host_object_and_health_events_mut(id).unwrap();
    assert!(owner.take_damage_from_typed_death_at_frame(
        100.0,
        None,
        crate::game_logic::combat::DamageType::Unresistable,
        crate::game_logic::host_usa_pilot::HostDeathType::Normal,
        frame,
        health_events,
    ));
    assert_eq!(world.host_object(id).unwrap().health.current, 0.0);
    assert!(
        world
            .host_object_mut(id)
            .unwrap()
            .begin_slow_death_from_ini(frame, &ini)
    );
    world.mark_object_for_destruction(id, Some(Team::GLA));
    let owner = world.host_object(id).unwrap();
    assert!(owner.status.on_die_started);
    assert!(!owner.status.destroyed);
    let death = owner.slow_death.as_ref().unwrap();
    assert!(death.is_active());
    assert_eq!(death.begin_frame, frame);
    assert_eq!(death.destroy_at_frame, frame + 9);
    assert!(world.objects_to_destroy.iter().all(|event| event.id != id));
    (world, id, frame + 9)
}

#[test]
fn explicit_destroy_during_active_slow_death_enqueues_once_without_reentering_die() {
    let (mut world, id, due) = parsed_slow_death_owner();
    let begin = world
        .host_object(id)
        .unwrap()
        .slow_death
        .as_ref()
        .unwrap()
        .begin_frame;
    // CPP destroyObject has no onDie latch and no waiting-for-animation gate.
    world.destroy_object(id);
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == id)
            .count(),
        1
    );
    assert_eq!(world.objects_to_destroy.front().unwrap().killer, None);
    let owner = world.host_object(id).unwrap();
    assert!(owner.status.destroyed && owner.status.on_die_started);
    let death = owner.slow_death.as_ref().unwrap();
    assert_eq!(death.begin_frame, begin);
    assert_eq!(death.destroy_at_frame, due);
    assert!(
        death.is_active(),
        "explicit deletion did not fast-forward the timer"
    );
    world.destroy_object(id);
    world.mark_object_for_destruction(id, Some(Team::China));
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == id)
            .count(),
        1
    );
    assert_eq!(world.objects_to_destroy.front().unwrap().killer, None);
    world.process_destroy_list();
    assert!(world.host_object(id).is_none());
}

#[test]
fn ordinary_slow_death_final_frame_requests_final_deletion_without_restart() {
    let (mut world, id, due) = parsed_slow_death_owner();
    while world.getFrame() < due {
        let before = world.getFrame();
        // A repeated death-start notification is not final destruction.
        world.mark_object_for_destruction(id, Some(Team::GLA));
        assert!(world.objects_to_destroy.iter().all(|event| event.id != id));
        assert_eq!(
            world
                .host_object(id)
                .unwrap()
                .slow_death
                .as_ref()
                .unwrap()
                .destroy_at_frame,
            due
        );
        world.update();
        assert_eq!(
            world.getFrame(),
            before + 1,
            "the ordinary simulation actually advanced"
        );
        assert!(
            world.host_object(id).is_some(),
            "no deletion before the original deadline"
        );
    }
    // The next ordinary update executes the exact due frame before advancing.
    // CPP SlowDeathBehavior.cpp446–450 does FINAL then destroyObject.
    world.update();
    assert_eq!(world.getFrame(), due + 1);
    assert!(
        world.host_object(id).is_none(),
        "final timer completed but deletion was dropped: frame={}, remaining={:?}, queued={}",
        world.getFrame(),
        world.host_object(id).map(|owner| (
            owner.status.on_die_started,
            owner.status.destroyed,
            owner.slow_death.as_ref().map(|death| death.is_done())
        )),
        world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == id)
            .count()
    );
}

#[test]
fn explicit_destroy_after_producer_death_preserves_one_refund_and_queue_identity() {
    let mut world = GameLogic::new();
    ensure_test_player_for_team(&mut world, Team::USA);
    ensure_test_player_for_team(&mut world, Team::GLA);
    ensure_test_barracks_template(&mut world);
    ensure_test_infantry_template(&mut world);
    let id = world
        .create_object("TestBarracks", Team::USA, Vec3::ZERO)
        .unwrap();
    assert!(world.enqueue_production(id, "TestInfantry".to_string()));
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 99_900);
    world.mark_object_for_destruction(id, Some(Team::GLA));
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 100_000);
    assert_eq!(world.get_player(2).unwrap().effective_supplies(), 100_000);
    let owner = world.host_object(id).unwrap();
    assert!(owner.status.on_die_started);
    assert!(owner.structure_topple_data.as_ref().unwrap().is_active());
    // The host bare-structure topple selector is an existing approximation.
    // This case proves only explicit destruction after onDie, not a CPP
    // assertion that StructureTopple by itself must remove a building.
    world.destroy_object(id);
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == id)
            .count(),
        1
    );
    world.destroy_object(id);
    world.mark_object_for_destruction(id, Some(Team::GLA));
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == id)
            .count(),
        1
    );
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 100_000);
    assert_eq!(world.get_player(2).unwrap().effective_supplies(), 100_000);
    world.process_destroy_list();
    assert!(world.host_object(id).is_none());
    assert_eq!(world.get_player(0).unwrap().effective_supplies(), 100_000);
}

#[test]
fn repeated_alive_direct_destroy_queues_once_without_entering_die() {
    let mut world = GameLogic::new();
    ensure_test_infantry_template(&mut world);
    let id = world
        .create_object("TestInfantry", Team::USA, Vec3::ZERO)
        .unwrap();
    world.destroy_object(id);
    // CPP GameLogic3935 / Object722: direct deletion has no onDie.
    assert!(!world.host_object(id).unwrap().status.on_die_started);
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == id)
            .count(),
        1
    );
    world.destroy_object(id);
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .filter(|event| event.id == id)
            .count(),
        1
    );
    world.process_destroy_list();
    assert!(world.host_object(id).is_none());
}

#[test]
fn coupled_default_authority_keeps_host_slow_death_timer_until_original_deadline() {
    use crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority;
    use crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at;
    isolated_at(
        module_path!(),
        "coupled_default_authority_keeps_host_slow_death_timer_until_original_deadline",
        || {
            crate::gameworld_shadow::with_gameworld_authority(
                GameWorldAuthority::DEFAULT_OFF,
                || {
                    let (mut world, id, due) = parsed_slow_death_owner();
                    assert_eq!(
                        *world.gameworld_authority(),
                        GameWorldAuthority::DEFAULT_OFF
                    );
                    assert!(crate::gameworld_shadow::gameworld_shadow_enabled());
                    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
                    shadow.sync_from_host(&world);
                    // Ordinary update evaluates its current frame, then advances it.
                    // At frame9 the original deadline executes; the exposed clock is10.
                    while world.getFrame() <= due {
                        let before = world.getFrame();
                        let _coupled = crate::gameworld_shadow::CoupledTickGuard::enter();
                        assert!(!crate::gameworld_shadow::gameworld_movement_authority_live());
                        world.update();
                        assert_eq!(world.getFrame(), before + 1);
                        assert_eq!(
                            world.host_object(id).is_some(),
                            before < due,
                            "default host owns final deletion before the shadow boundary"
                        );
                        let _ = crate::gameworld_shadow::shadow_session_after_host_tick(
                            &mut shadow,
                            &mut world,
                        );
                        if before < due {
                            let owner = world
                                .host_object(id)
                                .expect("host survives before its original execution deadline");
                            assert!(owner.status.on_die_started);
                            assert!(!owner.status.destroyed);
                            assert_eq!(owner.slow_death.as_ref().unwrap().destroy_at_frame, due);
                            assert!(
                                owner.health.current > 0.0,
                                "diagnostic mirror must not consume final-death ForceKill with authority off"
                            );
                            assert!(world.objects_to_destroy.iter().all(|event| event.id != id));
                        } else {
                            assert!(
                                world.host_object(id).is_none(),
                                "coupling with default authority must not disable the host final-death timer"
                            );
                        }
                    }
                    assert_eq!(world.getFrame(), due + 1);
                    assert!(shadow.entity_for_host(id).is_none());
                },
            );
        },
    );
}

#[test]
fn declared_movement_authority_shadow_final_deletion_does_not_reenter_death_start() {
    use crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority;
    use crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at;
    isolated_at(
        module_path!(),
        "declared_movement_authority_shadow_final_deletion_does_not_reenter_death_start",
        || {
            crate::gameworld_shadow::with_gameworld_authority(
                GameWorldAuthority::DEFAULT_OFF,
                || {
                    let (mut world, id, due) = parsed_slow_death_owner();
                    let body_before_final_timer = world.host_object(id).unwrap().health.current;
                    // Existing actual world declaration plus synchronous borrowed
                    // context, matching the real engine's authority dispatch boundary.
                    world.set_movement_authority(true);
                    let authority = *world.gameworld_authority();
                    assert!(authority.movement);
                    crate::gameworld_shadow::with_gameworld_authority(authority, || {
                        assert!(crate::gameworld_shadow::gameworld_shadow_enabled());
                        let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
                        shadow.sync_from_host(&world);
                        while world.getFrame() < due {
                            let before = world.getFrame();
                            let _coupled = crate::gameworld_shadow::CoupledTickGuard::enter();
                            assert!(crate::gameworld_shadow::gameworld_movement_authority_live());
                            world.update();
                            assert_eq!(world.getFrame(), before + 1);
                            assert!(
                                world
                                    .host_object(id)
                                    .unwrap()
                                    .slow_death
                                    .as_ref()
                                    .unwrap()
                                    .is_active(),
                                "declared mirror authority peels the local timer"
                            );
                            let _ = crate::gameworld_shadow::shadow_session_after_host_tick(
                                &mut shadow,
                                &mut world,
                            );
                            if world.getFrame() < due {
                                assert!(world.host_object(id).is_some());
                                assert!(
                                    world.objects_to_destroy.iter().all(|event| event.id != id)
                                );
                            }
                        }
                        let owner = world.host_object(id).unwrap();
                        assert!(owner.status.on_die_started);
                        assert_eq!(
                            owner.health.current, body_before_final_timer,
                            "CPP final destroyObject preserves earlier body state"
                        );
                        assert_eq!(owner.slow_death.as_ref().unwrap().destroy_at_frame, due);
                        assert_eq!(
                            world
                                .objects_to_destroy
                                .iter()
                                .filter(|event| event.id == id)
                                .count(),
                            1,
                            "actual mirror completion queues final destruction despite the onDie latch"
                        );
                        world.destroy_object(id);
                        assert_eq!(
                            world
                                .objects_to_destroy
                                .iter()
                                .filter(|event| event.id == id)
                                .count(),
                            1
                        );
                        world.process_destroy_list();
                        assert!(world.host_object(id).is_none());
                    });
                },
            );
        },
    );
}

#[test]
fn parsed_active_slow_death_snapshot_preserves_latch_and_original_deadline_continuation() {
    use crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority;
    use crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at;
    use crate::save_load::snapshot::SnapshotBuilder;
    isolated_at(
        module_path!(),
        "parsed_active_slow_death_snapshot_preserves_latch_and_original_deadline_continuation",
        || {
            crate::gameworld_shadow::with_gameworld_authority(
                GameWorldAuthority::DEFAULT_OFF,
                || {
                    let (mut source, id, due) = parsed_slow_death_owner();
                    for _ in 0..4 {
                        let before = source.getFrame();
                        source.update();
                        assert_eq!(source.getFrame(), before + 1);
                    }
                    assert_eq!(source.getFrame(), 4);
                    let before = source.host_object(id).unwrap();
                    assert!(before.status.on_die_started);
                    assert!(!before.status.destroyed);
                    let captured_death =
                        serde_json::to_value(before.slow_death.as_ref().unwrap()).unwrap();
                    assert_eq!(before.slow_death.as_ref().unwrap().destroy_at_frame, due);
                    let builder = SnapshotBuilder::new();
                    let snapshot = builder
                        .create_world_snapshot(&source)
                        .expect("capture actual active death");
                    let mut loaded = GameLogic::new();
                    loaded.templates = source.templates.clone();
                    builder
                        .restore_from_snapshot(&snapshot, &mut loaded)
                        .expect("restore actual active death");
                    assert_eq!(loaded.getFrame(), source.getFrame());
                    let restored = loaded.host_object(id).unwrap();
                    assert_eq!(
                        serde_json::to_value(restored.slow_death.as_ref().unwrap()).unwrap(),
                        captured_death,
                        "actual runtime phase/rules/absolute deadlines survive the native snapshot"
                    );
                    assert!(!restored.status.destroyed);
                    assert!(
                        restored.status.on_die_started,
                        "native continuation must retain the once-only death-start latch, not rerun onDie after load"
                    );
                    while source.getFrame() <= due {
                        let before = source.getFrame();
                        source.update();
                        loaded.update();
                        assert_eq!(source.getFrame(), before + 1);
                        assert_eq!(loaded.getFrame(), source.getFrame());
                        if before < due {
                            for world in [&source, &loaded] {
                                let owner = world
                                    .host_object(id)
                                    .expect("no early deletion after snapshot");
                                assert!(owner.status.on_die_started);
                                assert_eq!(
                                    owner.slow_death.as_ref().unwrap().destroy_at_frame,
                                    due
                                );
                                assert!(
                                    world.objects_to_destroy.iter().all(|event| event.id != id)
                                );
                            }
                        } else {
                            assert!(source.host_object(id).is_none());
                            assert!(loaded.host_object(id).is_none());
                        }
                    }
                    assert_eq!(source.getFrame(), due + 1);
                },
            );
        },
    );
}

#[test]
fn explicit_container_delete_detaches_a_dying_slow_death_passenger_immediately() {
    let (mut world, child, due) = parsed_slow_death_owner();
    let mut template = ThingTemplate::new("DyingPassengerContainer");
    template.add_kind_of(KindOf::Vehicle).set_health(100.0);
    template.contain_module.kind = ContainModuleKind::Transport;
    template.contain_module.slots = Some(2);
    world.templates.insert(template.name.clone(), template);
    let container = world
        .create_object("DyingPassengerContainer", Team::USA, Vec3::ZERO)
        .unwrap();
    assert!(
        world
            .host_object_mut(container)
            .unwrap()
            .add_occupant(child)
    );
    world
        .host_object_mut(child)
        .unwrap()
        .set_contained_by(Some(container));
    world.destroy_object(container);
    assert!(
        world
            .host_object(container)
            .unwrap()
            .contained_units()
            .is_empty()
    );
    let passenger = world.host_object(child).unwrap();
    assert_eq!(passenger.contained_by, None);
    assert!(passenger.status.destroyed && passenger.status.on_die_started);
    assert_eq!(passenger.slow_death.as_ref().unwrap().destroy_at_frame, due);
    assert_eq!(
        world
            .objects_to_destroy
            .iter()
            .map(|e| e.id)
            .collect::<Vec<_>>(),
        vec![container, child]
    );
    world.destroy_object(child);
    assert_eq!(world.objects_to_destroy.len(), 2);
    world.process_destroy_list();
    assert!(world.host_object(child).is_none());
}
