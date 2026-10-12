//! Actual receiving catalogs and the driving GameLogic identity counter.
use super::*;
use crate::object::object_factory::ObjectCreationFlags;
use crate::system::game_logic::GameLogic;

fn birth(
    candidate: &mut Candidate,
    logic: &mut GameLogic,
    definitions: &Definitions,
) -> Arc<RwLock<Object>> {
    definitions
        .borrowed(17)
        .prepare_new_object(
            &mut candidate.factory,
            logic,
            "FreshBody",
            Coord3D::new(20.0, 30.0, 0.0),
            None,
            ObjectCreationFlags::NO_AI,
        )
        .unwrap()
}

#[test]
fn restored_and_new_objects_share_only_the_driving_logic_counter() {
    let definitions = Definitions::new(75.0);
    let peer_definitions = Definitions::new(4.0);
    let mut candidate = Candidate::new();
    let mut peer = Candidate::new();
    let mut logic = GameLogic::new();
    let mut peer_logic = GameLogic::new();
    let saved = definitions
        .borrowed(17)
        .prepare_object(&mut candidate.factory, "FreshBody", 7)
        .unwrap();
    assert_eq!(
        logic.get_object_id_counter(),
        1,
        "detached save decoding cannot admit into an unrelated clock/identity owner"
    );
    assert!(logic.find_object_by_id(7).is_none());
    logic.track_object_in_update_list(saved.clone()).unwrap();
    assert_eq!(logic.get_object_id_counter(), 8);
    let new = birth(&mut candidate, &mut logic, &definitions);
    assert_eq!(new.read().unwrap().get_id(), 8);
    let peer_new = birth(&mut peer, &mut peer_logic, &peer_definitions);
    assert_eq!(peer_new.read().unwrap().get_id(), 1);
    assert_eq!(
        peer_new
            .read()
            .unwrap()
            .get_body()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        4.0
    );
    assert_eq!(logic.get_object_id_counter(), 9);
    let older = definitions
        .borrowed(17)
        .prepare_object(&mut candidate.factory, "FreshBody", 3)
        .unwrap();
    logic.track_object_in_update_list(older.clone()).unwrap();
    assert_eq!(
        logic.get_object_id_counter(),
        9,
        "older saved ids cannot move the live counter backwards"
    );
    let next = birth(&mut candidate, &mut logic, &definitions);
    assert_eq!(next.read().unwrap().get_id(), 9);
    assert_eq!(peer_logic.get_object_id_counter(), 2);
    assert!(Arc::ptr_eq(&logic.find_object_by_id(7).unwrap(), &saved));
    assert!(Arc::ptr_eq(&logic.find_object_by_id(8).unwrap(), &new));
    assert!(Arc::ptr_eq(&logic.find_object_by_id(3).unwrap(), &older));
    assert!(Arc::ptr_eq(
        &peer_logic.find_object_by_id(1).unwrap(),
        &peer_new
    ));
    assert_eq!(
        saved
            .read()
            .unwrap()
            .get_body()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        75.0
    );
}

#[test]
fn duplicate_or_foreign_saved_identity_cannot_replace_the_admitted_owner() {
    let definitions = Definitions::new(75.0);
    let peer_definitions = Definitions::new(4.0);
    let mut candidate = Candidate::new();
    let mut peer = Candidate::new();
    let mut logic = GameLogic::new();
    let owner = definitions
        .borrowed(17)
        .prepare_object(&mut candidate.factory, "FreshBody", 7)
        .unwrap();
    let foreign = peer_definitions
        .borrowed(17)
        .prepare_object(&mut peer.factory, "FreshBody", 7)
        .unwrap();
    logic.track_object_in_update_list(owner.clone()).unwrap();
    assert_eq!(logic.track_object_in_update_list(owner.clone()).unwrap(), 7);
    assert!(logic.track_object_in_update_list(foreign.clone()).is_err());
    assert!(
        definitions
            .borrowed(17)
            .prepare_object(&mut candidate.factory, "FreshBody", 7)
            .is_err()
    );
    assert_eq!(logic.get_object_id_counter(), 8);
    assert!(Arc::ptr_eq(&logic.find_object_by_id(7).unwrap(), &owner));
    assert!(Arc::ptr_eq(
        &candidate
            .factory
            .get_object(7)
            .unwrap()
            .get_base_object()
            .unwrap(),
        &owner
    ));
    assert_eq!(
        foreign
            .read()
            .unwrap()
            .get_body()
            .unwrap()
            .lock()
            .unwrap()
            .get_health(),
        4.0
    );
}

#[test]
fn failed_native_constructor_consumes_one_id_without_admitting_an_object() {
    let definitions = Definitions::new(75.0);
    let mut missing = Definitions::new(19.0);
    missing.module_factory = game_engine::common::thing::module_factory::ModuleFactory::new();
    let mut candidate = Candidate::new();
    let mut logic = GameLogic::new();
    let error = missing
        .borrowed(17)
        .prepare_new_object(
            &mut candidate.factory,
            &mut logic,
            "FreshBody",
            Coord3D::ZERO,
            None,
            ObjectCreationFlags::NO_AI,
        )
        .unwrap_err();
    assert!(error.contains("ActiveBody"), "{error}");
    assert_eq!(logic.get_object_id_counter(), 2);
    assert!(logic.find_object_by_id(1).is_none());
    assert!(candidate.factory.get_object(1).is_none());
    let owner = birth(&mut candidate, &mut logic, &definitions);
    assert_eq!(owner.read().unwrap().get_id(), 2);
    assert_eq!(logic.get_object_id_counter(), 3);
    assert!(Arc::ptr_eq(&logic.find_object_by_id(2).unwrap(), &owner));
}
