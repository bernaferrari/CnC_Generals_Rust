//! Real installed ActiveBody/AutoHeal hit, released callbacks and peer identity.
use super::*;
use crate::damage::{DamageInfo, DamageType};
use crate::modules::ai_world::AiWorld;
use crate::object::behavior::auto_heal_behavior::{
    AutoHealBehaviorModule, AutoHealBehaviorModuleData,
};
use crate::object::body::active_body::{ActiveBodyModuleData, BodyDefinitionCatalog};
use std::collections::HashMap;
pub(super) fn damage_actor(heals: bool) -> Arc<dyn ThingTemplate> {
    damage_actor_with_physics(heals, false)
}
fn damage_actor_with_physics(heals: bool, physics: bool) -> Arc<dyn ThingTemplate> {
    let mut base = DefaultThingTemplate::new(
        if heals {
            "OwnedDamageHealer"
        } else {
            "OwnedDamagePeer"
        }
        .into(),
    );
    base.add_kind_of(KindOf::Infantry);
    let body = ActiveBodyModuleData {
        max_health: 100.0,
        initial_health: 100.0,
        ..Default::default()
    };
    let mut modules = vec![TemplateModuleInfo {
        name: "ActiveBody".into(),
        module_tag: "OwnedDamageBody".into(),
        data: Arc::new(body),
        interface_mask: ModuleInterfaceType::BODY,
    }];
    if heals {
        let heal = AutoHealBehaviorModuleData {
            initially_active: true,
            start_healing_delay: 11,
            healing_delay: 60,
            ..Default::default()
        };
        modules.push(TemplateModuleInfo {
            name: "AutoHealBehavior".into(),
            module_tag: "OwnedDamageHeal".into(),
            data: Arc::new(heal),
            interface_mask: ModuleInterfaceType::UPDATE | ModuleInterfaceType::DAMAGE,
        });
    }
    if physics {
        modules.push(TemplateModuleInfo {
            name: "PhysicsBehavior".into(),
            module_tag: "OwnedShockPhysics".into(),
            data: Arc::new(
                crate::object::behavior::physics_update::PhysicsBehaviorModuleData {
                    mass: 2.0,
                    shock_resistance: 0.25,
                    shock_max_yaw: 0.3,
                    shock_max_pitch: 0.2,
                    shock_max_roll: 0.1,
                    ..Default::default()
                },
            ),
            interface_mask: ModuleInterfaceType::UPDATE,
        });
    }
    Arc::new(EntryTemplate { base, modules })
}
fn receiving_damage_world() -> NativeMatch {
    receiving_damage_world_with_physics(false)
}
fn receiving_damage_world_with_physics(physics: bool) -> NativeMatch {
    let mut game = NativeMatch::new(150.0);
    let mut factory = ObjectFactory::new();
    let definitions = Arc::new(
        BodyDefinitionCatalog::new(
            Some(&game_engine::common::global_data::GlobalData::default()),
            HashMap::new(),
            HashMap::new(),
        )
        .unwrap(),
    );
    let source_template = damage_actor_with_physics(true, physics);
    let target_template = damage_actor(false);
    let mut catalog = game_engine::common::thing::module_factory::ModuleFactory::new();
    crate::contain_module_overrides::register_native_module_overrides(&mut catalog).unwrap();
    for template in [&source_template, &target_template] {
        for entry in template.get_behavior_module_info() {
            catalog.add_module_internal(
                None,
                None,
                game_engine::common::thing::module::ModuleType::Behavior,
                &entry.name,
                entry.interface_flags(),
            );
        }
    }
    let mut lookup = |_name: &str| Ok(None);
    factory
        .prepare_saved_object(
            source_template,
            1,
            &mut lookup,
            Arc::clone(&definitions),
            &catalog,
        )
        .unwrap();
    factory
        .prepare_saved_object(target_template, 2, &mut lookup, definitions, &catalog)
        .unwrap();
    game.source = factory.get_object(1).unwrap().get_base_object().unwrap();
    game.target = factory.get_object(2).unwrap().get_base_object().unwrap();
    game.factory = factory;
    game.scheduler = crate::system::game_logic::GameLogic::new();
    let source = Arc::clone(&game.source);
    if physics {
        Object::admit_update_schedule_from_handle(&source, &mut game.scheduler).unwrap();
    }
    game.operate(17, |world| world.admit_prepared_auto_heal(&source).unwrap());
    game
}
fn health(object: &Arc<RwLock<Object>>) -> f32 {
    object.read().unwrap().get_health()
}
fn recorded(object: &Arc<RwLock<Object>>) -> (u32, u32) {
    let body = object.read().unwrap().get_body_module().unwrap();
    let body = body.lock().unwrap();
    (
        body.get_last_damage_timestamp(),
        body.get_last_damage_info().unwrap().input.source_id,
    )
}
fn healing_wake(game: &NativeMatch) -> u32 {
    let proxy = game
        .source
        .read()
        .unwrap()
        .named_update_registrations("AutoHealBehavior")
        .pop()
        .unwrap();
    game.scheduler.sleepy_entry_for(&proxy).unwrap().1
}
fn hit(source: u32, amount: f32) -> DamageInfo {
    let mut damage = DamageInfo::default();
    damage.input.source_id = source;
    damage.input.damage_type = DamageType::Unresistable;
    damage.input.amount = amount;
    damage
}
#[test]
fn actual_damage_releases_body_owner_before_installed_autoheal_wake() {
    let mut first = receiving_damage_world();
    let peer = receiving_damage_world();
    let peer_wake = healing_wake(&peer);
    let owner = Arc::clone(&first.source);
    let peer_guard = peer.source.write().unwrap();
    let mut damage = hit(2, 25.0);
    first.operate(100, |world| {
        assert_eq!(
            world.attempt_damage_owned(&owner, &mut damage).unwrap(),
            25.0
        )
    });
    assert_eq!(health(&first.source), 75.0);
    assert_eq!(recorded(&first.source), (100, 2));
    assert_eq!(
        healing_wake(&first),
        111,
        "actual installed onDamage must run after HP commit with supplied frame"
    );
    drop(peer_guard);
    assert_eq!(health(&peer.source), 100.0);
    assert_eq!(healing_wake(&peer), peer_wake);
    let entry = first
        .source
        .read()
        .unwrap()
        .module_by_name(&"AutoHealBehavior".into())
        .unwrap();
    entry.with_module(|module| {
        assert_eq!(
            module
                .as_any_mut()
                .downcast_mut::<AutoHealBehaviorModule>()
                .unwrap()
                .behavior()
                .last_wake_sleep(),
            Some(crate::modules::UpdateSleepTime::Frames(11))
        )
    });
}
#[test]
fn self_damage_uses_exact_source_and_rejects_foreign_or_invalid_shock_before_mutation() {
    let mut first = receiving_damage_world();
    let peer = receiving_damage_world();
    let owner = Arc::clone(&first.source);
    let foreign = Arc::clone(&peer.source);
    let mut damage = hit(1, 10.0);
    first.operate(201, |world| {
        assert_eq!(
            world.attempt_damage_owned(&owner, &mut damage).unwrap(),
            10.0
        )
    });
    assert_eq!(health(&first.source), 90.0);
    assert_eq!(recorded(&first.source), (201, 1));
    assert_eq!(healing_wake(&first), 212);
    let previous = healing_wake(&first);
    first.operate(202, |world| {
        assert!(
            world
                .attempt_damage_owned(&foreign, &mut hit(1, 10.0))
                .is_err()
        )
    });
    first.operate(203, |world| {
        assert!(
            world
                .attempt_damage_owned(&owner, &mut {
                    let mut damage = hit(1, 200.0);
                    damage.input.shock_wave_amount = 1.0;
                    damage.input.shock_wave_radius = 10.0;
                    damage.input.shock_wave_vector.x = f32::NAN;
                    damage
                })
                .is_err()
        )
    });
    assert_eq!(health(&first.source), 90.0);
    assert_eq!(recorded(&first.source), (201, 1));
    assert_eq!(healing_wake(&first), previous);
    assert_eq!(health(&peer.source), 100.0);
}

#[test]
fn caller_held_actual_owner_body_or_callback_rejects_before_health_commit() {
    let mut game = receiving_damage_world();
    let owner = Arc::clone(&game.source);
    let wake = healing_wake(&game);
    {
        let held = owner.write().unwrap();
        game.operate(300, |world| {
            assert!(
                world
                    .attempt_damage_owned(&owner, &mut hit(1, 10.0))
                    .is_err()
            )
        });
        drop(held);
    }
    {
        let body = owner.read().unwrap().get_body_module().unwrap();
        let held = body.lock().unwrap();
        game.operate(301, |world| {
            assert!(
                world
                    .attempt_damage_owned(&owner, &mut hit(1, 10.0))
                    .is_err()
            )
        });
        assert_eq!(held.get_health(), 100.0);
    }
    let entry = owner
        .read()
        .unwrap()
        .module_by_name(&"AutoHealBehavior".into())
        .unwrap();
    entry.with_module(|_| {
        game.operate(302, |world| {
            assert!(
                world
                    .attempt_damage_owned(&owner, &mut hit(1, 10.0))
                    .is_err()
            )
        })
    });
    assert_eq!(health(&game.source), 100.0);
    assert_eq!(healing_wake(&game), wake);
}

#[test]
fn no_die_module_marks_dead_but_does_not_invent_destruction() {
    // CPP Object::onDie iterates zero die interfaces then runs its tail.
    let mut game = receiving_damage_world();
    let owner = Arc::clone(&game.source);
    let mut damage = hit(1, 150.0);
    game.operate(410, |world| {
        world.attempt_damage_owned(&owner, &mut damage).unwrap()
    });
    let owner = owner.read().unwrap();
    assert_eq!(owner.get_health(), 0.0);
    assert!(owner.is_effectively_dead());
    assert!(!owner.is_destroyed());
    assert_eq!(
        owner
            .behavior_modules()
            .iter()
            .filter(|entry| entry.interface_mask().0 & ModuleInterfaceType::DIE.0 != 0)
            .count(),
        0
    );
}

#[test]
fn shock_follows_real_body_callback_and_wakes_exact_installed_physics() {
    let mut game = receiving_damage_world_with_physics(true);
    let peer = receiving_damage_world_with_physics(true);
    let owner = Arc::clone(&game.source);
    let physics = owner.read().unwrap().get_physics().unwrap();
    let peer_physics = peer.source.read().unwrap().get_physics().unwrap();
    let mut damage = hit(1, 10.0);
    damage.input.shock_wave_amount = 8.0;
    damage.input.shock_wave_radius = 10.0;
    damage.input.shock_wave_taper_off = 0.5;
    damage.input.shock_wave_vector = Coord3D::new(3.0, 4.0, 0.0);
    game.operate(211, |world| {
        assert_eq!(
            world.attempt_damage_owned(&owner, &mut damage).unwrap(),
            10.0
        )
    });
    assert_eq!(health(&owner), 90.0);
    assert_eq!(recorded(&owner), (211, 1));
    assert_eq!(healing_wake(&game), 222);
    let expected = Coord3D::new(1.35, 1.8, 2.25);
    assert!((physics.accumulated_force_acceleration() - expected).length() < 0.00001);
    assert!(physics.installed_is_stunned().unwrap());
    let proxy = physics
        .installed_update_proxy(&owner.read().unwrap())
        .unwrap();
    assert_eq!(game.scheduler.sleepy_entry_for(&proxy).unwrap().1, 212);
    assert_eq!(peer_physics.accumulated_force_acceleration(), Coord3D::ZERO);
    assert!(!peer_physics.installed_is_stunned().unwrap());
}

#[test]
fn unavailable_shock_module_rejects_before_body_and_restored_impulse_uses_receiving_owner() {
    use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;
    let mut game = receiving_damage_world_with_physics(true);
    let peer = receiving_damage_world_with_physics(true);
    let owner = Arc::clone(&game.source);
    let physics = owner.read().unwrap().get_physics().unwrap();
    let mut damage = hit(1, 10.0);
    damage.input.shock_wave_amount = 8.0;
    damage.input.shock_wave_radius = 10.0;
    damage.input.shock_wave_vector = Coord3D::new(3.0, 4.0, 0.0);
    {
        let _loan = physics.try_access().unwrap();
        game.operate(221, |world| {
            assert!(world.attempt_damage_owned(&owner, &mut damage).is_err())
        });
    }
    assert_eq!(health(&owner), 100.0);
    assert_eq!(physics.accumulated_force_acceleration(), Coord3D::ZERO);
    game.operate(222, |world| {
        world.attempt_damage_owned(&owner, &mut damage).unwrap()
    });
    let module = owner
        .read()
        .unwrap()
        .module_by_name(&"PhysicsBehavior".into())
        .unwrap();
    let receiver = peer
        .source
        .read()
        .unwrap()
        .module_by_name(&"PhysicsBehavior".into())
        .unwrap();
    let mut saved = Cursor::new(Vec::new());
    module.with_module(|m| m.xfer(&mut XferSave::new(&mut saved, 1)).unwrap());
    let bytes = saved.into_inner();
    receiver.with_module(|m| {
        m.xfer(&mut XferLoad::new(Cursor::new(bytes.clone()), 1))
            .unwrap()
    });
    let mut again = Cursor::new(Vec::new());
    receiver.with_module(|m| m.xfer(&mut XferSave::new(&mut again, 1)).unwrap());
    assert_eq!(again.into_inner(), bytes);
    let receiver_physics = peer.source.read().unwrap().get_physics().unwrap();
    assert_eq!(
        receiver_physics.accumulated_force_acceleration(),
        physics.accumulated_force_acceleration()
    );
    assert!(receiver_physics.installed_is_stunned().unwrap());
    assert_eq!(health(&peer.source), 100.0);
}
