//! CPP LifetimeUpdate.cpp70–75 kills at its scheduled ordinary frame;
//! PoisonedBehavior.cpp84–163 applies a source-less UNRESISTABLE pulse and
//! retains the original next pulse while a new poison dose extends duration.
//! Parsed known-range Main residual coverage; not arbitrary module installation.
use super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority;
use crate::game_logic::game_logic::world_tick::heli_motion_tests::isolated_at;
use crate::game_logic::host_usa_pilot::HostDeathType;

fn with_authority(test: &str, movement: bool, damage: bool, run: impl FnOnce()) {
    isolated_at(module_path!(), test, || {
        let mut authority = GameWorldAuthority::DEFAULT_OFF;
        authority.movement = movement;
        authority.damage = damage;
        crate::gameworld_shadow::with_gameworld_authority(authority, run);
    });
}

fn parsed_owner(world: &mut GameLogic, lifetime: bool) -> ObjectId {
    let (name, text, class) = if lifetime {
        (
            "TimerOwnershipTimedDemo",
            r#"
Object TimerOwnershipTimedDemo
  KindOf = SELECTABLE ATTACKABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
  End
  Behavior = LifetimeUpdate ModuleTag_Lifetime
    MinLifetime = 10000
    MaxLifetime = 10000
  End
End
"#,
            "LifetimeUpdate",
        )
    } else {
        (
            "TimerOwnershipPoisonVictim",
            r#"
Object TimerOwnershipPoisonVictim
  KindOf = SELECTABLE ATTACKABLE
  Body = ActiveBody ModuleTag_Body
    MaxHealth = 100
  End
  Behavior = PoisonedBehavior ModuleTag_Poison
    PoisonDamageInterval = 100
    PoisonDuration = 3000
  End
End
"#,
            "PoisonedBehavior",
        )
    };
    let mut parser = crate::assets::IniParser::new();
    assert_eq!(
        parser.parse_ini_content(text, "timer_owner.ini").unwrap(),
        1
    );
    let definition = parser.get_definition(name).unwrap();
    let data = definition
        .behavior_modules
        .iter()
        .find(|module| module.class_name == class)
        .unwrap();
    if lifetime {
        assert_eq!(data.attribute("MinLifetime"), Some("10000"));
        assert_eq!(data.attribute("MaxLifetime"), Some("10000"));
        assert_eq!(
            crate::game_logic::host_lifetime_update::lifetime_msec_range_for_template(name),
            Some((10000, 10000))
        );
    } else {
        assert_eq!(data.attribute("PoisonDamageInterval"), Some("100"));
        assert_eq!(data.attribute("PoisonDuration"), Some("3000"));
        assert_eq!(
            crate::game_logic::host_poisoned_behavior::poison_interval_frames(),
            3
        );
        assert_eq!(
            crate::game_logic::host_poisoned_behavior::poison_duration_frames(),
            90
        );
    }
    // Admission really constructs the owned body/timer. Main currently binds
    // Lifetime by its named retail range and Poison by fixed retail constants;
    // parsed arbitrary timer rules are an independent missing integration.
    let template = GameLogic::build_template_from_object_definition(name, definition, None);
    world.templates.insert(name.into(), template);
    let id = world.create_object(name, Team::USA, Vec3::ZERO).unwrap();
    let owner = world.host_object(id).unwrap();
    assert_eq!(owner.health.current, 100.0);
    assert_eq!(owner.health.maximum, 100.0);
    assert_eq!(owner.template_name, name);
    assert!(!owner.status.on_die_started);
    assert!(world.objects_to_destroy.iter().all(|event| event.id != id));
    id
}

fn coupled_update(
    world: &mut GameLogic,
    shadow: &mut crate::gameworld_shadow::GameWorldShadow,
) -> u32 {
    let before = world.getFrame();
    let _coupled = crate::gameworld_shadow::CoupledTickGuard::enter();
    assert!(crate::gameworld_shadow::gameworld_shadow_enabled());
    world.update();
    assert_eq!(world.getFrame(), before + 1);
    crate::gameworld_shadow::shadow_session_after_host_tick(shadow, world);
    before
}

fn lifetime_case(movement: bool, damage: bool) {
    let mut world = GameLogic::new();
    world.set_movement_authority(movement);
    world.set_damage_authority(damage);
    let start = world.getFrame();
    let id = parsed_owner(&mut world, true);
    let lifetime = world
        .host_object(id)
        .unwrap()
        .lifetime_update
        .as_ref()
        .unwrap();
    assert!(lifetime.active);
    assert_eq!(lifetime.expire_at_frame, start + 300);
    let due = lifetime.expire_at_frame;
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
    shadow.sync_from_host(&world);
    while world.getFrame() < due {
        let owner = world.host_object(id).unwrap();
        assert_eq!(owner.health.current, 100.0);
        assert!(
            !owner.status.on_die_started,
            "CPP Lifetime hasn't executed its due frame yet"
        );
        let before = coupled_update(&mut world, &mut shadow);
        assert!(before < due);
        let owner = world
            .host_object(id)
            .expect("Lifetime survives before due execution");
        assert!(
            !owner.status.on_die_started,
            "diagnostic timer cannot kill a frame early"
        );
        assert!(world.objects_to_destroy.iter().all(|event| event.id != id));
    }
    // Ordinary tick executes frame300, then advances the public clock to301.
    assert_eq!(coupled_update(&mut world, &mut shadow), due);
    assert_eq!(world.getFrame(), due + 1);
    assert!(
        world.host_object(id).is_none(),
        "the real initial-kill path reaches the original destruction queue"
    );
    assert!(shadow.entity_for_host(id).is_none());
    assert!(world.objects_to_destroy.iter().all(|event| event.id != id));
    coupled_update(&mut world, &mut shadow);
    assert!(
        world.host_object(id).is_none(),
        "no second timer execution resurrects the owner"
    );
}

#[test]
fn coupled_default_lifetime_ordinary_deadline_is_not_disabled() {
    with_authority(
        "coupled_default_lifetime_ordinary_deadline_is_not_disabled",
        false,
        false,
        || lifetime_case(false, false),
    );
}

#[test]
fn coupled_movement_lifetime_does_not_kill_one_frame_early() {
    with_authority(
        "coupled_movement_lifetime_does_not_kill_one_frame_early",
        true,
        false,
        || lifetime_case(true, false),
    );
}

#[test]
fn coupled_movement_damage_lifetime_keeps_initial_kill_deadline() {
    with_authority(
        "coupled_movement_damage_lifetime_keeps_initial_kill_deadline",
        true,
        true,
        || lifetime_case(true, true),
    );
}

fn infect(world: &mut GameLogic, id: ObjectId, amount: f32) {
    let frame = world.getFrame();
    assert!(
        frame > 0,
        "this packet excludes the preexisting frame0 deferred-anchor policy"
    );
    assert!(!world
        .host_object_mut(id)
        .unwrap()
        .take_damage_from_typed_death_at_frame(
            amount,
            None,
            DamageType::Toxin,
            HostDeathType::Poisoned,
            frame
        ));
    let owner = world.host_object(id).unwrap();
    let poison = owner.poisoned_behavior.as_ref().unwrap();
    assert!(owner.is_poison_tinted());
    assert!(poison.is_active());
    assert!(!poison.needs_frame_sync);
    assert_eq!(poison.poison_damage_amount, amount);
    assert_eq!(poison.death_type, HostDeathType::Poisoned);
    assert_eq!(poison.poison_overall_stop_frame, frame + 90);
}

fn poison_case(movement: bool, damage: bool) {
    let mut world = GameLogic::new();
    world.set_movement_authority(movement);
    world.set_damage_authority(damage);
    world.update();
    assert_eq!(world.getFrame(), 1);
    let id = parsed_owner(&mut world, false);
    let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
    shadow.sync_from_host(&world);
    infect(&mut world, id, 10.0);
    let owner = world.host_object(id).unwrap();
    assert_eq!(owner.health.current, 90.0);
    assert_eq!(
        owner
            .poisoned_behavior
            .as_ref()
            .unwrap()
            .poison_damage_frame,
        4
    );
    // At public clock4, frame4 is still awaiting ordinary execution. The
    // deadline assertion is after its execution (clock5), not a synthetic tick.
    while world.getFrame() < 4 {
        coupled_update(&mut world, &mut shadow);
        if world.getFrame() < 4 {
            assert_eq!(world.host_object(id).unwrap().health.current, 90.0);
        }
    }
    for executing_frame in 4..=7 {
        assert_eq!(world.getFrame(), executing_frame);
        assert_eq!(coupled_update(&mut world, &mut shadow), executing_frame);
        let owner = world.host_object(id).unwrap();
        let expected_ticks = if executing_frame < 7 { 1 } else { 2 };
        assert_eq!(
            owner.health.current,
            90.0 - 10.0 * expected_ticks as f32,
            "one CPP pulse every three ordinary frames, not every shadow synchronization"
        );
        assert_eq!(owner.last_damage_source, None);
        assert_eq!(owner.last_damage_info_type, Some(DamageType::Unresistable));
        let poison = owner.poisoned_behavior.as_ref().unwrap();
        assert_eq!(poison.tick_count, expected_ticks);
        assert_eq!(poison.total_dot_damage, 10.0 * expected_ticks as f32);
        assert_eq!(
            poison.poison_damage_amount, 10.0,
            "UNRESISTABLE doesn't reinfect"
        );
        assert_eq!(poison.poison_overall_stop_frame, 91);
        assert_eq!(
            poison.poison_damage_frame,
            if executing_frame < 7 { 7 } else { 10 }
        );
        assert!(owner.is_poison_tinted());
        assert!(!owner.status.on_die_started);
    }
}

#[test]
fn coupled_default_poison_keeps_owned_periodic_damage_and_tint() {
    with_authority(
        "coupled_default_poison_keeps_owned_periodic_damage_and_tint",
        false,
        false,
        || poison_case(false, false),
    );
}

#[test]
fn coupled_movement_poison_preserves_periodic_damage_not_repeated_sync_damage() {
    with_authority(
        "coupled_movement_poison_preserves_periodic_damage_not_repeated_sync_damage",
        true,
        false,
        || poison_case(true, false),
    );
}

#[test]
fn coupled_movement_damage_poison_applies_each_accepted_pulse_once() {
    with_authority(
        "coupled_movement_damage_poison_applies_each_accepted_pulse_once",
        true,
        true,
        || poison_case(true, true),
    );
}

#[test]
fn coupled_movement_damage_reinfection_replaces_amount_without_postponing_pulse() {
    with_authority(
        "coupled_movement_damage_reinfection_replaces_amount_without_postponing_pulse",
        true,
        true,
        || {
            let mut world = GameLogic::new();
            world.set_movement_authority(true);
            world.set_damage_authority(true);
            world.update();
            let id = parsed_owner(&mut world, false);
            let mut shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
            shadow.sync_from_host(&world);
            infect(&mut world, id, 10.0);
            while world.getFrame() < 5 {
                coupled_update(&mut world, &mut shadow);
            }
            assert_eq!(world.host_object(id).unwrap().health.current, 80.0);
            infect(&mut world, id, 5.0);
            assert_eq!(world.host_object(id).unwrap().health.current, 75.0);
            assert_eq!(
                world
                    .host_object(id)
                    .unwrap()
                    .poisoned_behavior
                    .as_ref()
                    .unwrap()
                    .poison_damage_frame,
                7
            );
            while world.getFrame() <= 7 {
                let before = coupled_update(&mut world, &mut shadow);
                let owner = world.host_object(id).unwrap();
                assert_eq!(owner.health.current, if before < 7 { 75.0 } else { 70.0 });
                let poison = owner.poisoned_behavior.as_ref().unwrap();
                assert_eq!(poison.poison_overall_stop_frame, 95);
                assert_eq!(poison.poison_damage_amount, 5.0);
                assert_eq!(poison.poison_damage_frame, if before < 7 { 7 } else { 10 });
                assert_eq!(poison.tick_count, if before < 7 { 1 } else { 2 });
                assert!(owner.is_poison_tinted());
            }
        },
    );
}

#[test]
fn coupled_default_same_id_poison_uses_each_owners_actual_dose_and_clock() {
    with_authority(
        "coupled_default_same_id_poison_uses_each_owners_actual_dose_and_clock",
        false,
        false,
        || {
            let mut first = GameLogic::new();
            first.update();
            let first_id = parsed_owner(&mut first, false);
            infect(&mut first, first_id, 10.0);
            let mut second = GameLogic::new();
            for _ in 0..5 {
                second.update();
            }
            let second_id = parsed_owner(&mut second, false);
            assert_eq!(first_id, second_id);
            infect(&mut second, second_id, 5.0);
            let mut first_shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
            let mut second_shadow = crate::gameworld_shadow::GameWorldShadow::new(64);
            first_shadow.sync_from_host(&first);
            second_shadow.sync_from_host(&second);
            for _ in 0..4 {
                coupled_update(&mut second, &mut second_shadow);
                coupled_update(&mut first, &mut first_shadow);
            }
            let a = first.host_object(first_id).unwrap();
            let b = second.host_object(second_id).unwrap();
            assert_eq!((first.getFrame(), second.getFrame()), (5, 9));
            assert_eq!(a.health.current, 80.0);
            assert_eq!(b.health.current, 90.0);
            assert_eq!(a.last_damage_timestamp, Some(4));
            assert_eq!(b.last_damage_timestamp, Some(8));
            assert_eq!(a.last_damage_info_type, Some(DamageType::Unresistable));
            assert_eq!(b.last_damage_info_type, Some(DamageType::Unresistable));
            assert_eq!(a.poisoned_behavior.as_ref().unwrap().poison_damage_frame, 7);
            assert_eq!(
                b.poisoned_behavior.as_ref().unwrap().poison_damage_frame,
                11
            );
            assert_eq!(
                a.poisoned_behavior
                    .as_ref()
                    .unwrap()
                    .poison_overall_stop_frame,
                91
            );
            assert_eq!(
                b.poisoned_behavior
                    .as_ref()
                    .unwrap()
                    .poison_overall_stop_frame,
                95
            );
            assert_eq!(a.poisoned_behavior.as_ref().unwrap().tick_count, 1);
            assert_eq!(b.poisoned_behavior.as_ref().unwrap().tick_count, 1);
            assert!(a.is_poison_tinted() && b.is_poison_tinted());
        },
    );
}

#[test]
fn admitted_lifetime_consumes_one_wake_without_rebuilding_its_deadline() {
    with_authority(
        "admitted_lifetime_consumes_one_wake_without_rebuilding_its_deadline",
        false,
        false,
        || {
            let mut world = GameLogic::new();
            let id = parsed_owner(&mut world, true);
            let due = world
                .host_object(id)
                .unwrap()
                .lifetime_update
                .as_ref()
                .unwrap()
                .expire_at_frame;
            // This is an admitted Object timer-operation control, not a claim
            // that the scheduler advanced the owning world to its due frame.
            // The ordinary public-update cases above prove actual dispatch.
            let owner = world.host_object_mut(id).unwrap();
            assert!(!owner.tick_lifetime_update(due - 1));
            assert!(owner.lifetime_update.as_ref().unwrap().active);
            assert!(owner.tick_lifetime_update(due));
            assert!(!owner.lifetime_update.as_ref().unwrap().active);
            assert_eq!(owner.lifetime_update.as_ref().unwrap().expire_at_frame, due);
            assert!(!owner.tick_lifetime_update(due));
            assert!(!owner.tick_lifetime_update(due + 1));
            assert_eq!(owner.lifetime_update.as_ref().unwrap().expire_at_frame, due);
            // Returning the initial-kill request doesn't perform final destroy.
            assert!(!owner.status.on_die_started);
            assert_eq!(owner.health.current, 100.0);
            assert!(world.objects_to_destroy.iter().all(|event| event.id != id));
        },
    );
}
