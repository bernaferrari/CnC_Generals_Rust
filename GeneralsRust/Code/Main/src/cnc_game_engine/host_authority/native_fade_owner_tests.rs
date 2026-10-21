use super::*;

#[test]
fn staged_native_fade_owner_preflight_classifies_a_loan_as_staging_not_committed() {
    let mut candidate = crate::game_logic::GameLogic::new();
    assert!(CnCGameEngine::validate_staged_native_fade(&candidate).is_ok());
    candidate
        .with_owned_script_engine(|_, candidate| {
            let error = CnCGameEngine::validate_staged_native_fade(candidate).unwrap_err();
            assert!(
                matches!(
                    &error,
                    crate::cnc_game_engine::host_authority::HostLoadFailure::Staging(_)
                ),
                "unavailable detached fade owner must not discard the still-valid receiving match"
            );
            assert!(!error.requires_match_reset());
        })
        .unwrap();
    assert!(CnCGameEngine::validate_staged_native_fade(&candidate).is_ok());
}
