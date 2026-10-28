use super::*;

#[test]
fn transferring_a_prototype_appends_to_new_owner_and_updates_all_siblings_in_place() {
    let mut factory = TeamFactory::new();
    factory.init_team_owned("OldFirst".into(), "OldOwner".into(), true, None, Some(1));
    let prototype = factory
        .init_team_owned(
            "SharedPrototype".into(),
            "OldOwner".into(),
            false,
            None,
            Some(1),
        )
        .expect("admit shared prototype");
    factory.init_team_owned(
        "DestinationFirst".into(),
        "NewOwner".into(),
        true,
        None,
        Some(2),
    );
    factory.init_team_owned(
        "DestinationLast".into(),
        "NewOwner".into(),
        true,
        None,
        Some(2),
    );
    let first = factory
        .create_inactive_team("SharedPrototype")
        .expect("create first sibling");
    let second = factory
        .create_inactive_team("SharedPrototype")
        .expect("create second sibling");
    let first_id = first.read().unwrap().get_id();
    let second_id = second.read().unwrap().get_id();
    first.write().unwrap().restore_owned_members(&[101]);
    second.write().unwrap().restore_owned_members(&[202]);
    let prototype_id = prototype.get_id();
    let factory_order_before = factory.prototype_names();

    assert_eq!(
        factory
            .owned_team_prototypes_for_controller(2)
            .iter()
            .map(|prototype| prototype.get_name().to_string())
            .collect::<Vec<_>>(),
        ["DestinationFirst", "DestinationLast"]
    );
    assert_eq!(
        factory.transfer_team_prototype_controller(first_id, 2),
        Some(TeamPrototypeControllerTransfer {
            all_member_ids: vec![202, 101],
            selected_member_ids: vec![101],
        })
    );

    let moved = factory
        .find_team_prototype("SharedPrototype")
        .expect("prototype remains registered");
    assert_eq!(moved.get_id(), prototype_id);
    assert!(Arc::ptr_eq(&prototype, &moved));
    assert!(Arc::ptr_eq(
        &factory.find_team_by_id(first_id).unwrap(),
        &first
    ));
    assert!(Arc::ptr_eq(
        &factory.find_team_by_id(second_id).unwrap(),
        &second
    ));
    assert_eq!(first.read().unwrap().get_controlling_player_id(), Some(2));
    assert_eq!(second.read().unwrap().get_controlling_player_id(), Some(2));
    assert_eq!(factory.prototype_names(), factory_order_before);
    assert_eq!(
        factory
            .owned_team_prototypes_for_controller(1)
            .iter()
            .map(|prototype| prototype.get_name().to_string())
            .collect::<Vec<_>>(),
        ["OldFirst"]
    );
    assert_eq!(
        factory
            .owned_team_prototypes_for_controller(2)
            .iter()
            .map(|prototype| prototype.get_name().to_string())
            .collect::<Vec<_>>(),
        ["DestinationFirst", "DestinationLast", "SharedPrototype"]
    );
}

#[test]
fn prototype_controller_restore_keeps_factory_order_and_restores_player_order() {
    let mut factory = TeamFactory::new();
    factory.init_team_owned(
        "SharedPrototype".into(),
        "OldOwner".into(),
        false,
        None,
        Some(1),
    );
    factory.init_team_owned(
        "DestinationFirst".into(),
        "NewOwner".into(),
        true,
        None,
        Some(2),
    );
    factory.init_team_owned(
        "DestinationLast".into(),
        "NewOwner".into(),
        true,
        None,
        Some(2),
    );
    let factory_order_before = factory.prototype_names();
    assert!(factory.restore_prototype_controller("SharedPrototype", Some(2)));
    assert!(factory.restore_player_prototype_orders(&[
        ("DestinationFirst".into(), 2, 0),
        ("DestinationLast".into(), 2, 1),
        ("SharedPrototype".into(), 2, 2),
    ]));

    assert_eq!(factory.owned_team_prototypes_for_controller(1).len(), 0);
    assert_eq!(
        factory
            .owned_team_prototypes_for_controller(2)
            .iter()
            .map(|prototype| prototype.get_name().to_string())
            .collect::<Vec<_>>(),
        ["DestinationFirst", "DestinationLast", "SharedPrototype"]
    );
    assert_eq!(factory.prototype_names(), factory_order_before);
}

#[test]
fn restoring_unknown_prototype_controller_is_rejected_without_changing_owner() {
    let mut factory = TeamFactory::new();
    factory.init_team_owned(
        "SharedPrototype".into(),
        "OldOwner".into(),
        false,
        None,
        Some(1),
    );
    assert!(!factory.restore_prototype_controller("Unknown", Some(2)));

    assert_eq!(
        factory
            .owned_team_prototypes_for_controller(1)
            .iter()
            .map(|prototype| prototype.get_name().to_string())
            .collect::<Vec<_>>(),
        ["SharedPrototype"]
    );
    assert!(factory.owned_team_prototypes_for_controller(2).is_empty());
}

#[test]
fn prototype_controller_transfer_keeps_unrelated_factory_callbacks_pending() {
    let handle = TeamFactoryHandle::new();
    let team_id = {
        let mut factory = handle.inner.inner.lock().unwrap();
        factory.init_team_owned("Owned".into(), "First".into(), true, None, Some(1));
        let team_id = factory.find_team_instances("Owned")[0]
            .read()
            .unwrap()
            .get_id();
        factory
            .pending_create_action_scripts
            .push("unrelated callback".into());
        team_id
    };

    assert!(
        handle
            .transfer_prototype_controller_without_flushing(team_id, 2)
            .unwrap()
            .is_some()
    );
    let factory = handle.inner.inner.lock().unwrap();
    assert_eq!(
        factory.admitted_prototype_controller("Owned"),
        Some(Some(2))
    );
    assert_eq!(
        factory.pending_create_action_scripts,
        ["unrelated callback"]
    );
}
