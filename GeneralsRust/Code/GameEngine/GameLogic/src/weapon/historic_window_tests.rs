//! Native impacts at the C++ unsigned HistoricBonus window boundaries.
//!
//! Weapon.cpp:1169–1186 trims with an UnsignedInt subtraction and `<=`.
//! Weapon.cpp:1216–1247 counts with another UnsignedInt subtraction and `>=`,
//! then either clears the history or appends this impact after counting.

use super::test_fixture::{AdmittedWeaponBodies, ScopedWeaponFixture};
use super::{Coord3D, HistoricWeaponFixture, Weapon, WeaponBonus, WeaponSlotType, WeaponTemplate};
use std::sync::Arc;

fn assert_two_native_impacts(
    name: &str,
    frames: [u32; 2],
    retention: u32,
    bonus_time: u32,
    bonus_count: i32,
    expected_history: usize,
) {
    // This fixture owns the native registry isolation guard and restores the
    // exact prior clock and retention, including assertion unwinding. Bodies
    // retire before those inputs are restored, through their actual owner.
    let fixture = HistoricWeaponFixture::new();
    let _inputs = ScopedWeaponFixture::new(&fixture._isolation);
    let bodies = AdmittedWeaponBodies::new();
    game_engine::common::global_data::write().historic_damage_limit = retention;

    let mut template = WeaponTemplate::new(name.to_owned());
    template.primary_damage = 10.0;
    template.historic_bonus_count = bonus_count;
    template.historic_bonus_time = bonus_time;
    template.historic_bonus_radius = 20.0;
    // Deliberately no bonus template: C++ createAndFireTempWeapon explicitly
    // returns for NULL (Weapon.cpp:1513–1521), and its caller still clears the
    // list on the threshold branch. This isolates the window branch without
    // claiming authored Firestorm creation or introducing a foreign store.
    assert!(template.historic_bonus_weapon.is_none());
    assert!(template.historic_bonus_weapon_name.is_empty());
    let mut weapon = Weapon::new(Arc::new(template), WeaponSlotType::Primary);
    let position = Coord3D::new(20.0, 0.0, 0.0);
    let bonus = WeaponBonus::default();

    for (index, frame) in frames.into_iter().enumerate() {
        crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .set_current_frame(u64::from(frame));
        let actual_damage = weapon
            .deal_damage_internal(
                bodies.source_id(),
                Some(bodies.target_id()),
                &position,
                &bonus,
                false,
            )
            .expect("canonical native impact must reach the admitted victim");
        assert_eq!(actual_damage, 10, "impact {index} at frame {frame}");
        assert_eq!(
            bodies.target_health(),
            100.0 - 10.0 * (index + 1) as f32,
            "actual victim health after impact {index} at frame {frame}"
        );
        if index == 0 {
            assert_eq!(weapon.template.historic_damage_len(), 1);
        }
    }
    assert_eq!(bodies.target_health(), 80.0);
    assert_eq!(bodies.source_health(), 100.0);
    assert_eq!(
        weapon.template.historic_damage_len(),
        expected_history,
        "C++ unsigned window result for frames {frames:?}, retention {retention}, bonus time {bonus_time}"
    );
}

#[test]
fn native_historic_retention_early_frame_wraps_expiration() {
    // 1u32 - 8u32 wraps to MAX-6, so the first sample at frame 1 expires.
    assert_two_native_impacts("HistoricEarlyRetention", [1, 1], 8, 0, 3, 1);
}

#[test]
fn native_historic_retention_ordinary_frame_keeps_recent_sample() {
    // Expiration is 2; both frame-10 impacts remain in the history.
    assert_two_native_impacts("HistoricOrdinaryRetention", [10, 10], 8, 0, 3, 2);
}

#[test]
fn native_historic_retention_expires_sample_at_inclusive_boundary() {
    // The first sample equals the second impact's expiration: 10 - 8 == 2.
    assert_two_native_impacts("HistoricInclusiveRetention", [2, 10], 8, 0, 3, 1);
}

#[test]
fn native_historic_retention_rollover_preserves_cpp_unsigned_comparison() {
    // After rollover, 1 - 8 == MAX-6; MAX-7 is removed by the original <=
    // comparison. This intentionally tests C++ behavior, not elapsed-time
    // normalization across wrap or a redesigned chronological ordering.
    assert_two_native_impacts("HistoricRolloverRetention", [u32::MAX - 7, 1], 8, 0, 3, 1);
}

#[test]
fn native_historic_count_window_underflow_does_not_count_recent_sample() {
    // Retention does not wrap (100 - 8 == 92), so the first sample survives.
    // The count cutoff does wrap (100 - 101 == MAX), and frame 100 is below
    // it. C++ appends the second sample instead of taking the clear branch.
    assert_two_native_impacts("HistoricCountUnderflow", [100, 100], 8, 101, 2, 2);
}
