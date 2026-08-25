use game_engine::common::ini::{
    ChallengeGeneralsLoadStatus, challenge_generals_load_status, ensure_challenge_generals_loaded,
    get_challenge_generals,
};

#[test]
fn retail_challenge_mode_loads_authoritative_locked_boss_persona() {
    ensure_challenge_generals_loaded()
        .expect("retail ChallengeMode.ini must load through the Common-owned loader");
    assert!(matches!(
        challenge_generals_load_status(),
        ChallengeGeneralsLoadStatus::Loaded { .. }
    ));

    let generals = get_challenge_generals();
    assert!(
        generals
            .get_general_by_template_name("FactionAmericaAirForceGeneral")
            .is_some_and(|persona| persona.is_starting_enabled())
    );
    assert!(
        generals
            .get_general_by_template_name("FactionBossGeneral")
            .is_some_and(|persona| !persona.is_starting_enabled())
    );
}
