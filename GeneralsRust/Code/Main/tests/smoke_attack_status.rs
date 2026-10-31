#[path = "../src/executable_smoke/command_status.rs"]
mod command_status;

#[test]
fn attack_latch_requires_success_and_remains_set_after_later_failures() {
    let mut accepted = false;
    for status in [
        "",
        "attack_begin",
        "attack_fail_no_enemy",
        "attack_fail_no_selection",
    ] {
        command_status::record_attack_success(&mut accepted, status);
        assert!(
            !accepted,
            "{status} must not count as a successful attack status"
        );
    }
    command_status::record_attack_success(&mut accepted, "attack_ok:42");
    assert!(accepted);
    command_status::record_attack_success(&mut accepted, "attack_fail_no_enemy");
    assert!(
        accepted,
        "a later failure must not erase an earlier successful attack status"
    );
}
