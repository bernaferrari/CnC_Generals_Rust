//! CPP Team.cpp:1315 / Object.cpp:853 / ScriptEngine.cpp:5933 roster contracts.
//! Fixtures construct only their owned definitions and instances, with no
//! published factory, player roster, object registry or process test guard.
use super::*;

fn factory(name: &str, singleton: bool) -> TeamFactory {
    let mut factory = TeamFactory::new();
    let mut prototype = TeamPrototype::new(name.into());
    prototype.set_singleton(singleton);
    factory.prototypes.insert(name.into(), Arc::new(prototype));
    factory
}

fn insert_team(
    factory: &mut TeamFactory,
    name: &str,
    id: TeamID,
    active: bool,
    arrivals: &[ObjectID],
) -> Arc<RwLock<Team>> {
    let mut team = Team::new(name.into(), id);
    if active {
        team.set_active();
    }
    for &member in arrivals {
        team.add_member(member);
    }
    let team = Arc::new(RwLock::new(team));
    factory.admit_team_instance(id, Arc::clone(&team));
    factory.unique_team_id = factory.unique_team_id.max(id + 1);
    team
}

#[test]
fn member_admission_prepends_without_reordering_duplicates() {
    let mut team = Team::new("Guard".into(), 1);
    team.add_member(11);
    team.add_member(22);
    team.add_member(33);
    team.add_member(22);
    assert_eq!(team.get_members(), &[33, 22, 11]);
    assert_eq!(
        team.cur_units, 0,
        "admission does not perform updateState recount"
    );
    team.remove_member(22);
    team.add_member(22);
    assert_eq!(team.get_members(), &[22, 33, 11]);
}

#[test]
fn ordinary_lookup_returns_newest_instance_in_member_list_order() {
    let mut factory = factory("Guard", false);
    insert_team(&mut factory, "Guard", 7, true, &[101, 102]);
    insert_team(&mut factory, "Guard", 9, false, &[201, 202, 203]);
    insert_team(&mut factory, "Other", 12, true, &[999]);
    assert_eq!(
        factory.script_team_member_ids("Guard", false).unwrap(),
        Some(vec![203, 202, 201])
    );
}

#[test]
fn inactive_newest_singleton_does_not_fall_back_to_older_active_instance() {
    let mut factory = factory("Guard", true);
    insert_team(&mut factory, "Guard", 7, true, &[101]);
    let newest = insert_team(&mut factory, "Guard", 9, false, &[201]);
    assert_eq!(
        factory.script_team_member_ids("Guard", false).unwrap(),
        None
    );
    newest.write().unwrap().set_active();
    assert_eq!(
        factory.script_team_member_ids("Guard", false).unwrap(),
        Some(vec![201])
    );
}

#[test]
fn contextual_singleton_bypasses_activity_gate() {
    let mut factory = factory("Guard", true);
    insert_team(&mut factory, "Guard", 7, false, &[101]);
    assert_eq!(
        factory.script_team_member_ids("Guard", false).unwrap(),
        None
    );
    assert_eq!(
        factory.script_team_member_ids("Guard", true).unwrap(),
        Some(vec![101])
    );
}

#[test]
fn unique_contextual_instance_needs_no_prototype() {
    let mut factory = TeamFactory::new();
    insert_team(&mut factory, "Calling", 7, false, &[101, 102]);
    assert_eq!(
        factory.script_team_member_ids("Calling", false).unwrap(),
        None
    );
    assert_eq!(
        factory.script_team_member_ids("Calling", true).unwrap(),
        Some(vec![102, 101])
    );
    assert!(factory.prototypes.is_empty());
}

#[test]
fn ambiguous_context_requires_instance_identity_without_mutation() {
    let mut factory = factory("Guard", false);
    let first = insert_team(&mut factory, "Guard", 7, true, &[101]);
    let second = insert_team(&mut factory, "Guard", 9, true, &[201]);
    let next_id = factory.get_next_team_id();
    let result = factory.script_team_member_ids("Guard", true);
    assert!(
        matches!(result, Err(crate::GameLogicError::Configuration(message)) if message.contains("contextual team ID"))
    );
    assert_eq!(factory.get_next_team_id(), next_id);
    assert_eq!(factory.teams.len(), 2);
    assert_eq!(first.read().unwrap().get_members(), &[101]);
    assert_eq!(second.read().unwrap().get_members(), &[201]);
}

#[test]
fn missing_lookup_never_creates_an_instance() {
    let factory = factory("Declared", false);
    let next_id = factory.get_next_team_id();
    for name in ["Declared", "Missing", ""] {
        for contextual in [false, true] {
            assert_eq!(
                factory.script_team_member_ids(name, contextual).unwrap(),
                None
            );
        }
    }
    assert_eq!(factory.get_next_team_id(), next_id);
    assert!(factory.teams.is_empty());
    assert_eq!(factory.prototypes.len(), 1);
}

#[test]
fn script_lookup_preserves_exact_name_without_case_or_whitespace_aliases() {
    let mut factory = factory("Guard", false);
    insert_team(&mut factory, "Guard", 7, true, &[101]);
    for name in ["guard", "GUARD", " Guard", "Guard "] {
        for contextual in [false, true] {
            assert_eq!(
                factory.script_team_member_ids(name, contextual).unwrap(),
                None
            );
        }
    }
    assert_eq!(
        factory.script_team_member_ids("Guard", false).unwrap(),
        Some(vec![101])
    );
}

#[test]
fn poisoned_owned_instance_is_an_error_instead_of_a_missing_team() {
    let mut factory = factory("Guard", false);
    let team = insert_team(&mut factory, "Guard", 7, true, &[101]);
    let poisoned = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = team.write().unwrap();
        panic!("intentional owned roster poison");
    }));
    assert!(poisoned.is_err());
    for contextual in [false, true] {
        assert!(matches!(
            factory.script_team_member_ids("Guard", contextual),
            Err(crate::GameLogicError::Threading(_))
        ));
    }
}

#[test]
fn restored_ids_follow_admission_order_instead_of_numeric_order() {
    // CPP TeamPrototype::xfer (Team.cpp:1249-1258) admits missing saved
    // instances sequentially, prepending each even when saved IDs decrease.
    let mut factory = factory("Guard", false);
    insert_team(&mut factory, "Guard", 9, true, &[901]);
    insert_team(&mut factory, "Guard", 7, true, &[701, 702]);
    assert_eq!(factory.instance_order, vec![7, 9]);
    assert_eq!(
        factory.script_team_member_ids("Guard", false).unwrap(),
        Some(vec![702, 701])
    );
}

#[test]
fn restoring_existing_id_preserves_its_instance_list_position() {
    let mut factory = factory("Guard", false);
    let first = insert_team(&mut factory, "Guard", 9, true, &[901]);
    insert_team(&mut factory, "Guard", 7, true, &[701]);
    let prototype = factory.find_team_prototype("Guard").unwrap();
    // The production load callback returns an admitted ID before owner lookup.
    let restored = factory
        .create_team_on_prototype_with_id(&prototype, 9)
        .unwrap();
    assert!(Arc::ptr_eq(&first, &restored));
    assert_eq!(factory.instance_order, vec![7, 9]);
    assert_eq!(
        factory.script_team_member_ids("Guard", false).unwrap(),
        Some(vec![701])
    );
}

#[test]
fn finalized_owned_deletion_removes_only_that_instance_from_order() {
    let mut factory = factory("Guard", false);
    insert_team(&mut factory, "Guard", 9, true, &[901]);
    insert_team(&mut factory, "Guard", 7, true, &[701]);
    let deletion = factory.prepare_host_team_deletion(7).unwrap();
    assert!(factory.finalize_host_team_deletion(deletion));
    assert_eq!(factory.instance_order, vec![9]);
    assert_eq!(
        factory.script_team_member_ids("Guard", false).unwrap(),
        Some(vec![901])
    );
}

#[test]
fn init_and_reset_clear_admitted_instance_order() {
    // No prototypes: reset has no owning-player publication to unlink.
    let mut factory = TeamFactory::new();
    insert_team(&mut factory, "Calling", 9, true, &[901]);
    factory.init();
    assert!(factory.instance_order.is_empty());
    assert!(factory.teams.is_empty());
    insert_team(&mut factory, "Calling", 7, true, &[701]);
    factory.reset();
    assert!(factory.instance_order.is_empty());
    assert!(factory.teams.is_empty());
}
