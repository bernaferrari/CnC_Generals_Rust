//! Ranking for C++ WeaponSet::chooseBestWeaponForTarget.

use super::{WeaponFireStatus, WeaponSlotType};
use crate::damage::DamageType;

/// The metric used after putting ready weapons ahead of backups.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponChoiceCriteria {
    PreferMostDamage,
    PreferLongestRange,
}

/// Observations for an eligible slot. The score is damage or range according
/// to the selector's criteria.
pub struct WeaponCandidate {
    pub slot: WeaponSlotType,
    pub score: f32,
    pub status: WeaponFireStatus,
    pub preferred: bool,
    pub turret_aiming: bool,
}

/// Visit eligible slots in descending ordinal order.
pub struct WeaponSelection {
    criteria: WeaponChoiceCriteria,
    ready: Option<(WeaponSlotType, f32)>,
    backup: Option<(WeaponSlotType, f32)>,
}

impl WeaponSelection {
    pub fn new(criteria: WeaponChoiceCriteria) -> Self {
        Self {
            criteria,
            ready: None,
            backup: None,
        }
    }

    pub fn consider(&mut self, candidate: WeaponCandidate) {
        let (score, ready) = if candidate.preferred {
            (1.0e10, candidate.status != WeaponFireStatus::OutOfAmmo)
        } else {
            (
                candidate.score,
                candidate.status == WeaponFireStatus::ReadyToFire && !candidate.turret_aiming,
            )
        };
        let tier = if ready {
            &mut self.ready
        } else {
            &mut self.backup
        };
        let best = tier.map_or(0.0, |(_, score)| score);
        // Among equal scores, damage takes the later (lower) slot and range
        // keeps the earlier (higher) slot. Both start with a score of zero.
        let outranks = match self.criteria {
            WeaponChoiceCriteria::PreferMostDamage => score >= best,
            WeaponChoiceCriteria::PreferLongestRange => score > best,
        };
        if outranks {
            *tier = Some((candidate.slot, score));
        }
    }

    pub fn selected_slot(&self) -> Option<WeaponSlotType> {
        self.ready.or(self.backup).map(|(slot, _)| slot)
    }
}

/// C++ permits zero damage for unresistable weapons.
pub fn weapon_damage_is_eligible(damage: f32, damage_type: DamageType) -> bool {
    !(damage <= 0.0 && damage_type != DamageType::Unresistable)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(slot: WeaponSlotType, score: f32, status: WeaponFireStatus) -> WeaponCandidate {
        WeaponCandidate {
            slot,
            score,
            status,
            preferred: false,
            turret_aiming: false,
        }
    }

    #[test]
    fn equal_damage_prefers_primary_and_equal_range_prefers_tertiary() {
        let mut damage = WeaponSelection::new(WeaponChoiceCriteria::PreferMostDamage);
        let mut range = WeaponSelection::new(WeaponChoiceCriteria::PreferLongestRange);
        for slot in WeaponSlotType::DESCENDING {
            damage.consider(candidate(slot, 30.0, WeaponFireStatus::ReadyToFire));
            range.consider(candidate(slot, 30.0, WeaponFireStatus::ReadyToFire));
        }
        assert_eq!(damage.selected_slot(), Some(WeaponSlotType::Primary));
        assert_eq!(range.selected_slot(), Some(WeaponSlotType::Tertiary));
    }

    #[test]
    fn aiming_turret_becomes_backup_and_a_preferred_reloading_weapon_is_ready() {
        let mut selection = WeaponSelection::new(WeaponChoiceCriteria::PreferMostDamage);
        assert_eq!(selection.selected_slot(), None);

        let mut turret = candidate(
            WeaponSlotType::Tertiary,
            90.0,
            WeaponFireStatus::ReadyToFire,
        );
        turret.turret_aiming = true;
        selection.consider(turret);
        assert_eq!(selection.selected_slot(), Some(WeaponSlotType::Tertiary));

        selection.consider(candidate(
            WeaponSlotType::Secondary,
            10.0,
            WeaponFireStatus::ReadyToFire,
        ));
        assert_eq!(selection.selected_slot(), Some(WeaponSlotType::Secondary));

        let mut preferred = candidate(
            WeaponSlotType::Primary,
            1.0,
            WeaponFireStatus::ReloadingClip,
        );
        preferred.preferred = true;
        selection.consider(preferred);
        assert_eq!(selection.selected_slot(), Some(WeaponSlotType::Primary));
    }

    #[test]
    fn preferred_out_of_ammo_stays_in_the_backup_tier() {
        let mut selection = WeaponSelection::new(WeaponChoiceCriteria::PreferMostDamage);
        let mut preferred = candidate(WeaponSlotType::Secondary, 90.0, WeaponFireStatus::OutOfAmmo);
        preferred.preferred = true;
        selection.consider(preferred);
        assert_eq!(selection.selected_slot(), Some(WeaponSlotType::Secondary));
        selection.consider(candidate(
            WeaponSlotType::Primary,
            1.0,
            WeaponFireStatus::ReadyToFire,
        ));
        assert_eq!(selection.selected_slot(), Some(WeaponSlotType::Primary));
    }

    #[test]
    fn zero_damage_gate_keeps_unresistable() {
        assert!(!weapon_damage_is_eligible(0.0, DamageType::Explosion));
        assert!(weapon_damage_is_eligible(0.0, DamageType::Unresistable));
        assert!(weapon_damage_is_eligible(1.0, DamageType::Explosion));
    }
}
