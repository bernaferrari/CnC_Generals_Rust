//! C++ ActionManager.cpp:699-709 rejects dead/self targets before isAbleToAttack.
use super::*;
use crate::common::Coord3D;
use crate::object::object_factory::{ObjectCreationFlags, ObjectFactory};

fn prepared_pair() -> (ObjectFactory, Arc<RwLock<Object>>, Arc<RwLock<Object>>) {
    let mut factory = ObjectFactory::new();
    let mut identity_owner = crate::system::game_logic::GameLogic::new();
    let mut objects = Vec::new();
    for name in ["AttackOrderSource", "AttackOrderTarget"] {
        let id = factory
            .prepare_object(
                &mut identity_owner,
                Arc::new(crate::common::DefaultThingTemplate::new(name.into())),
                Coord3D::ZERO,
                None,
                ObjectCreationFlags::NO_AI,
            )
            .unwrap();
        objects.push(factory.get_object(id).unwrap().get_base_object().unwrap());
    }
    (factory, objects.remove(0), objects.remove(0))
}

#[test]
fn attack_self_rejects_before_observing_ability() {
    let (_factory, source, _target) = prepared_pair();
    let source = source.read().unwrap();
    let query = |victim: &Object| {
        TheActionManager::get_can_attack_object_inner(
            &source,
            victim,
            CommandSourceType::FromPlayer,
            AbleToAttackType::NewTarget,
            None,
            || panic!("C++ sanity rejection must precede ability callback"),
        )
        .unwrap()
    };
    assert_eq!(query(&source), ATTACKRESULT_NOT_POSSIBLE);
}

#[test]
fn attack_ability_service_failure_is_not_a_negative_predicate() {
    let (_factory, source, target) = prepared_pair();
    let source = source.read().unwrap();
    let target = target.read().unwrap();
    let result = TheActionManager::get_can_attack_object_inner(
        &source,
        &target,
        CommandSourceType::FromPlayer,
        AbleToAttackType::NewTarget,
        None,
        || Err("required ability service unavailable".into()),
    );
    assert_eq!(result, Err("required ability service unavailable".into()));
}
