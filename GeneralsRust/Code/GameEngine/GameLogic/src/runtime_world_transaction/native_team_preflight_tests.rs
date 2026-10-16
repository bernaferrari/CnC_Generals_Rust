use super::*;

#[test]
fn native_team_preflight_accepts_semantically_empty_guard_receipts() {
    let receipts = vec![
        TeamFactoryDeferredEffects::empty_for_world_transaction_test(),
        TeamFactoryDeferredEffects::empty_for_world_transaction_test(),
    ];
    assert_eq!(receipts.len(), 2);
    assert!(validate_native_team_effects(&receipts).is_ok());
}

#[test]
fn native_team_preflight_rejects_create_payload_without_executing_it() {
    let receipts =
        vec![TeamFactoryDeferredEffects::pending_for_world_transaction_test(true, false)];
    let error = validate_native_team_effects(&receipts).unwrap_err();
    assert!(error.contains("1 create, 0 generic"));
    assert_eq!(receipts[0].pending_callback_counts(), (1, 0));
}

#[test]
fn native_team_preflight_rejects_foreign_generic_identity_without_evaluation() {
    let receipts =
        vec![TeamFactoryDeferredEffects::pending_for_world_transaction_test(false, true)];
    let error = validate_native_team_effects(&receipts).unwrap_err();
    assert!(error.contains("0 create, 1 generic"));
    assert_eq!(receipts[0].pending_callback_counts(), (0, 1));
}

#[test]
fn native_team_preflight_checks_all_receipts_and_both_payload_kinds() {
    let receipts = vec![
        TeamFactoryDeferredEffects::empty_for_world_transaction_test(),
        TeamFactoryDeferredEffects::pending_for_world_transaction_test(true, true),
        TeamFactoryDeferredEffects::pending_for_world_transaction_test(true, false),
    ];
    let error = validate_native_team_effects(&receipts).unwrap_err();
    assert!(error.contains("2 create, 1 generic"));
}
