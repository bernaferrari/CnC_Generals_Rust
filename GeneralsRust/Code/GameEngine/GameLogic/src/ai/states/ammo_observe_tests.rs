//! hq-2qedt.79: AI attack/hunt ammo checks are WeaponSet.cpp:968-980
//! getStatus() calls on the owning object, so they must go through the
//! mutable-owner `observe_out_of_ammo_at` under a write guard, never the
//! `&self` read-lock `is_out_of_ammo()` side-effect path.

const ATTACK_SRC: &str = include_str!("attack.rs");
const HUNT_SRC: &str = include_str!("hunt.rs");

fn count(src: &str, needle: &str) -> usize {
    src.matches(needle).count()
}

#[test]
fn ai_attack_and_hunt_ammo_checks_use_write_guard_observe_not_read_lock_status() {
    for (name, src) in [("attack.rs", ATTACK_SRC), ("hunt.rs", HUNT_SRC)] {
        assert_eq!(
            count(src, ".is_out_of_ammo()"),
            0,
            "{name}: read-lock is_out_of_ammo()"
        );
        assert_eq!(
            count(src, ".is_out_of_ammo_at("),
            0,
            "{name}: read-lock is_out_of_ammo_at()"
        );
    }
    assert_eq!(count(ATTACK_SRC, ".observe_out_of_ammo_at("), 5);
    assert_eq!(count(HUNT_SRC, ".observe_out_of_ammo_at("), 1);
}
