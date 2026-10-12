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
    Arc::new(EntryTemplate { base, modules })
}
fn receiving_damage_world() -> NativeMatch {
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
    let source_template = damage_actor(true);
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
fn self_damage_uses_exact_source_and_rejects_foreign_or_shockwave_before_mutation() {
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
