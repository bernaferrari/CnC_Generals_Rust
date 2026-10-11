//! CPP Player.cpp1729 preserves prototype admission order independently of IDs.
use super::*;

#[test]
fn owned_prototypes_preserve_order_controller_identity_replacement_and_reset() {
    let mut source = TeamFactory::new();
    let mut peer = TeamFactory::new();
    for factory in [&mut source, &mut peer] {
        factory
            .init_team_owned("Zulu".into(), "Owner".into(), false, None, Some(7))
            .unwrap();
        factory
            .init_team_owned("Alpha".into(), "Owner".into(), true, None, Some(7))
            .unwrap();
        factory
            .init_team_owned("Other".into(), "OtherOwner".into(), false, None, Some(8))
            .unwrap();
    }
    assert_eq!(source.prototype_names(), ["Zulu", "Alpha", "Other"]);
    assert_eq!(
        source
            .owned_team_prototypes_for_controller(7)
            .iter()
            .map(|prototype| prototype.get_name().to_string())
            .collect::<Vec<_>>(),
        ["Zulu", "Alpha"]
    );
    assert_eq!(
        source
            .find_existing_team_by_name("Alpha")
            .unwrap()
            .read()
            .unwrap()
            .get_controlling_player_id(),
        Some(7)
    );
    let mut replacement = (*source.find_team_prototype("Zulu").unwrap()).clone();
    replacement.set_production_priority(29);
    source.replace_team_prototype(replacement);
    assert_eq!(source.prototype_names(), ["Zulu", "Alpha", "Other"]);
    source
        .restore_prototype_admission_order(&["Alpha".into(), "Zulu".into(), "Other".into()])
        .unwrap();
    assert_eq!(
        source
            .owned_team_prototypes_for_controller(7)
            .iter()
            .map(|prototype| prototype.get_name().to_string())
            .collect::<Vec<_>>(),
        ["Alpha", "Zulu"]
    );
    assert_eq!(peer.prototype_names(), ["Zulu", "Alpha", "Other"]);
    let before = source.prototype_names();
    assert!(
        source
            .restore_prototype_admission_order(&["Zulu".into(), "Zulu".into()])
            .is_err()
    );
    assert!(
        source
            .restore_prototype_admission_order(&["ForeignMissing".into()])
            .is_err()
    );
    assert_eq!(source.prototype_names(), before);
    source.reset();
    assert!(source.prototype_names().is_empty());
    assert!(source.owned_team_prototypes_for_controller(7).is_empty());
    assert_eq!(peer.prototype_names(), ["Zulu", "Alpha", "Other"]);
}

#[test]
fn owned_team_create_effects_do_not_escape_to_compatibility_pending_queue() {
    let mut owner = TeamFactory::new();
    owner
        .init_team_owned("Callback".into(), "Owner".into(), true, None, Some(7))
        .unwrap();
    let mut prototype = (*owner.find_team_prototype("Callback").unwrap()).clone();
    prototype.set_execute_actions_on_create(true);
    prototype.set_production_condition("CreateCallback".into());
    owner.replace_team_prototype(prototype);
    let (first, effects) = owner
        .create_inactive_team_with_owned_effects("Callback")
        .unwrap();
    assert_eq!(effects, ["CreateCallback"]);
    assert!(owner.drain_pending_create_action_scripts().is_empty());
    let (same, effects) = owner
        .create_inactive_team_with_owned_effects("Callback")
        .unwrap();
    assert!(Arc::ptr_eq(&first, &same));
    assert_eq!(
        effects,
        ["CreateCallback"],
        "singleton reuse still executes callback"
    );
    assert!(owner.drain_pending_create_action_scripts().is_empty());
}

#[test]
fn owned_singleton_init_returns_effects_before_next_prototype_admission() {
    let mut dict = Dict::new();
    dict.set_ascii_string(key_team_production_condition(), "FirstInit");
    dict.set_bool(key_team_executes_actions_on_create(), true);
    let mut owner = TeamFactory::new();
    let mut peer = TeamFactory::new();
    let (_, effects) = owner
        .init_team_owned_with_effects("First".into(), "Owner".into(), true, Some(&dict), Some(7))
        .unwrap();
    assert_eq!(effects, ["FirstInit"]);
    assert_eq!(owner.prototype_names(), ["First"]);
    let team = owner.find_team_instances("First")[0].clone();
    assert_eq!(team.read().unwrap().get_controlling_player_id(), Some(7));
    assert!(!team.read().unwrap().is_active());
    assert!(owner.drain_pending_create_action_scripts().is_empty());
    assert!(peer.prototype_names().is_empty());
    // A normal prototype does not instantiate or execute during initTeam.
    let (_, effects) = peer
        .init_team_owned_with_effects("First".into(), "Peer".into(), false, Some(&dict), Some(9))
        .unwrap();
    assert!(effects.is_empty());
    assert!(peer.find_team_instances("First").is_empty());
    assert!(peer.drain_pending_create_action_scripts().is_empty());
}
