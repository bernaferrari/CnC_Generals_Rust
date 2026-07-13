//! Host China Propaganda / Speaker Tower residual (heal + weapon buff).
//!
//! Residual slice (playability):
//! - ChinaSpeakerTower / *PropagandaTower / ListeningOutpost / Emperor tanks:
//!   C++ `PropagandaTowerBehavior` radius pulse residual —
//!   heals damaged **same-team non-structure** units in radius over time and
//!   applies ENTHUSIASTIC (base) / SUBLIMINAL (upgrade) weapon-bonus flags.
//! - Retail ChinaSpeakerTower INI ModuleTag_06 residual:
//!   Radius=150, DelayBetweenUpdates=2000ms, HealPercentEachSecond=2%,
//!   UpgradedHealPercentEachSecond=4%, UpgradeRequired=Upgrade_ChinaSubliminalMessaging.
//!
//! Wave 52 residual pack:
//! - Radius / DelayBetweenUpdates frames / HealPercent residual honesty
//! - ENTHUSIASTIC / SUBLIMINAL WeaponBonusCondition discriminants + ROF 125%
//! - Sole-benefactor residual map (first-tower-wins per pulse; multi-tower reject)
//! - UpgradeRequired = Upgrade_ChinaSubliminalMessaging residual
//!
//! Fail-closed honesty:
//! - Not full C++ multi-tower ObjectTracker influence matrix beyond residual map
//! - Not full ally relationship filter (uses same-team residual)
//! - Not full double-contain / stealthed FX suppress / POWERED underpower gate
//! - Not full player vs object UpgradeType switch matrix beyond residual tag
//! - Not full PulseFX / world-anim propaganda pulse
//! - Not network propaganda replication (network deferred)

use super::ObjectId;
use std::collections::HashMap;

/// Logic frames per second residual (C++ LOGICFRAMES_PER_SECOND).
pub const PROPAGANDA_LOGIC_FPS: f32 = 30.0;

/// Retail speaker-tower scan radius residual (ChinaSpeakerTower Radius = 150).
pub const HOST_PROPAGANDA_TOWER_RADIUS: f32 = 150.0;

/// Retail DelayBetweenUpdates residual (msec).
pub const HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_MS: u32 = 2000;
/// DelayBetweenUpdates 2000ms → 60 frames @ 30 FPS (parseDurationUnsignedInt ceil).
pub const HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES: u32 = 60;

/// Retail base heal percent of max health per second (HealPercentEachSecond = 2%).
pub const HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC: f32 = 0.02;

/// Retail upgraded heal percent of max health per second (4%).
pub const HOST_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC: f32 = 0.04;

/// Player upgrade residual that enables SUBLIMINAL + upgraded heal rate.
pub const UPGRADE_CHINA_SUBLIMINAL_MESSAGING: &str = "Upgrade_ChinaSubliminalMessaging";

/// Retail PulseFX residual names (ChinaSpeakerTower ModuleTag_06).
pub const PROPAGANDA_PULSE_FX: &str = "FX_PropagandaTowerPropagandaPulse";
pub const PROPAGANDA_UPGRADED_PULSE_FX: &str = "FX_PropagandaTowerSubliminalPulse";

/// Retail GameData.ini WeaponBonus ENTHUSIASTIC RATE_OF_FIRE residual (125%).
pub const ENTHUSIASTIC_RATE_OF_FIRE_MULT: f32 = 1.25;
/// Retail GameData.ini WeaponBonus SUBLIMINAL RATE_OF_FIRE residual (125%).
pub const SUBLIMINAL_RATE_OF_FIRE_MULT: f32 = 1.25;

/// C++ `WeaponBonusConditionType` residual discriminants (ALLOW_DEMORALIZE off).
/// WEAPONBONUSCONDITION_ENTHUSIASTIC = 8, WEAPONBONUSCONDITION_SUBLIMINAL = 15.
pub const WEAPON_BONUS_ENTHUSIASTIC: u8 = 8;
pub const WEAPON_BONUS_SUBLIMINAL: u8 = 15;

/// Retail condition residual names.
pub const ENTHUSIASTIC_CONDITION_NAME: &str = "ENTHUSIASTIC";
pub const SUBLIMINAL_CONDITION_NAME: &str = "SUBLIMINAL";

/// Convert msec residual → logic frames @ 30 FPS (C++ parseDurationUnsignedInt ceil).
pub fn propaganda_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * (PROPAGANDA_LOGIC_FPS / 1000.0)).ceil() as u32
}

/// Whether template is a residual propaganda / speaker tower source.
///
/// Fail-closed: name-based residual (not full INI PropagandaTowerBehavior module matrix).
/// Excludes PropagandaCenter (research building, different module).
pub fn is_propaganda_tower(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.contains("propagandacenter") {
        return false;
    }
    n.contains("speakertower")
        || n.contains("propagandatower")
        || n.contains("listeningoutpost")
        || n.contains("tankemperor")
        || n.ends_with("emperor")
}

/// Whether residual target can receive propaganda heal/buff.
///
/// C++ filters: allies, alive, same map status, not STRUCTURE, optional AffectsSelf=false.
pub fn is_legal_propaganda_target(
    is_structure: bool,
    is_alive: bool,
    same_team: bool,
    is_self: bool,
    under_construction: bool,
) -> bool {
    !is_structure && is_alive && same_team && !is_self && !under_construction
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_propaganda_radius_2d(
    tower_pos: (f32, f32),
    target_pos: (f32, f32),
    radius: f32,
) -> bool {
    let dx = tower_pos.0 - target_pos.0;
    let dy = tower_pos.1 - target_pos.1;
    dx * dx + dy * dy <= radius * radius
}

/// Continuous residual heal amount for one tick given max health and upgrade state.
pub fn propaganda_heal_amount(max_health: f32, upgraded: bool, dt: f32) -> f32 {
    if max_health <= 0.0 || dt <= 0.0 {
        return 0.0;
    }
    let percent = if upgraded {
        HOST_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC
    } else {
        HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC
    };
    percent * max_health * dt
}

/// Heal amount residual for one DelayBetweenUpdates pulse (2s).
///
/// Host continuous path uses `propaganda_heal_amount`; this helper is the
/// retail pulse-window residual honesty (percent_per_sec * max * 2.0s).
pub fn propaganda_heal_amount_per_pulse(max_health: f32, upgraded: bool) -> f32 {
    propaganda_heal_amount(max_health, upgraded, 2.0)
}

/// Whether tower/player residual has subliminal upgrade active.
pub fn is_subliminal_upgrade_active(has_upgrade_tag: bool) -> bool {
    has_upgrade_tag
}

/// Residual weapon-bonus flag pair from upgrade coverage.
///
/// Base cover → ENTHUSIASTIC; upgraded cover → ENTHUSIASTIC + SUBLIMINAL.
pub fn propaganda_weapon_bonus_flags(upgraded: bool) -> (bool, bool) {
    // (enthusiastic, subliminal)
    (true, upgraded)
}

/// Residual sole-benefactor exclusivity map (ObjectId → tower_id).
///
/// Host residual: first-tower-wins per pulse/frame — a target accepts heal only
/// from the first propaganda source that claims it. Clears each pulse for next cycle.
/// Mirrors ambulance `HostAmbulanceHealExclusivity` pattern (Wave 48).
#[derive(Debug, Clone, Default)]
pub struct HostPropagandaHealExclusivity {
    /// Target ObjectId → claiming tower ObjectId.
    beneficiaries: HashMap<ObjectId, ObjectId>,
    /// Claims that won (first tower for a target).
    pub claims_granted: u32,
    /// Later towers rejected because target already claimed (multi-tower reject).
    pub claims_rejected: u32,
}

impl HostPropagandaHealExclusivity {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.beneficiaries.clear();
        // Keep historical honesty counters (do not zero).
    }

    pub fn clear_pulse(&mut self) {
        self.beneficiaries.clear();
    }

    pub fn reset_honesty(&mut self) {
        *self = Self::default();
    }

    /// First-tower-wins claim. Returns true if `tower` may heal `target`.
    pub fn try_claim(&mut self, target: ObjectId, tower: ObjectId) -> bool {
        match self.beneficiaries.get(&target) {
            Some(existing) if *existing == tower => true,
            Some(_) => {
                self.claims_rejected = self.claims_rejected.saturating_add(1);
                false
            }
            None => {
                self.beneficiaries.insert(target, tower);
                self.claims_granted = self.claims_granted.saturating_add(1);
                true
            }
        }
    }

    pub fn claimed_tower(&self, target: ObjectId) -> Option<ObjectId> {
        self.beneficiaries.get(&target).copied()
    }

    pub fn honesty_exclusivity_ok(&self) -> bool {
        self.claims_granted > 0
    }

    pub fn honesty_reject_ok(&self) -> bool {
        self.claims_rejected > 0
    }
}

/// Wave 52 residual honesty: radius / delay / heal% / flags / upgrade residual.
pub fn honesty_propaganda_residual_ok() -> bool {
    (HOST_PROPAGANDA_TOWER_RADIUS - 150.0).abs() < 0.01
        && HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_MS == 2000
        && HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES
            == propaganda_ms_to_frames(HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_MS)
        && (HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC - 0.02).abs() < 0.0001
        && (HOST_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC - 0.04).abs() < 0.0001
        && UPGRADE_CHINA_SUBLIMINAL_MESSAGING == "Upgrade_ChinaSubliminalMessaging"
        && (ENTHUSIASTIC_RATE_OF_FIRE_MULT - 1.25).abs() < 0.001
        && (SUBLIMINAL_RATE_OF_FIRE_MULT - 1.25).abs() < 0.001
        && WEAPON_BONUS_ENTHUSIASTIC == 8
        && WEAPON_BONUS_SUBLIMINAL == 15
        && ENTHUSIASTIC_CONDITION_NAME == "ENTHUSIASTIC"
        && SUBLIMINAL_CONDITION_NAME == "SUBLIMINAL"
        && PROPAGANDA_PULSE_FX == "FX_PropagandaTowerPropagandaPulse"
        && PROPAGANDA_UPGRADED_PULSE_FX == "FX_PropagandaTowerSubliminalPulse"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propaganda_tower_name_matrix() {
        assert!(is_propaganda_tower("ChinaSpeakerTower"));
        assert!(is_propaganda_tower("Tank_ChinaSpeakerTower"));
        assert!(is_propaganda_tower("ChinaTankOverlordPropagandaTower"));
        assert!(is_propaganda_tower("ChinaHelixPropagandaTower"));
        assert!(is_propaganda_tower("ChinaVehicleListeningOutpost"));
        assert!(is_propaganda_tower("Tank_ChinaTankEmperor"));
        assert!(is_propaganda_tower("Boss_SpeakerTower"));
        assert!(!is_propaganda_tower("ChinaPropagandaCenter"));
        assert!(!is_propaganda_tower("Tank_ChinaPropagandaCenter"));
        assert!(!is_propaganda_tower("USA_Ranger"));
        assert!(!is_propaganda_tower("AmericaVehicleMedic"));
        assert!(!is_propaganda_tower("TestInfantry"));
    }

    #[test]
    fn legal_propaganda_target_matrix() {
        assert!(is_legal_propaganda_target(false, true, true, false, false));
        assert!(!is_legal_propaganda_target(true, true, true, false, false));
        assert!(!is_legal_propaganda_target(false, false, true, false, false));
        assert!(!is_legal_propaganda_target(false, true, false, false, false));
        assert!(!is_legal_propaganda_target(false, true, true, true, false));
        assert!(!is_legal_propaganda_target(false, true, true, false, true));
    }

    #[test]
    fn propaganda_radius_heal_and_upgrade_math() {
        assert!(HOST_PROPAGANDA_TOWER_RADIUS > 0.0);
        assert!(HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC > 0.0);
        assert!(
            HOST_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC
                > HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC
        );
        assert!(in_propaganda_radius_2d((0.0, 0.0), (50.0, 0.0), 150.0));
        assert!(!in_propaganda_radius_2d((0.0, 0.0), (200.0, 0.0), 150.0));

        let base = propaganda_heal_amount(100.0, false, 1.0);
        let up = propaganda_heal_amount(100.0, true, 1.0);
        assert!((base - 2.0).abs() < f32::EPSILON);
        assert!((up - 4.0).abs() < f32::EPSILON);
        assert_eq!(propaganda_heal_amount(100.0, false, 0.0), 0.0);
        assert!(is_subliminal_upgrade_active(true));
        assert!(!is_subliminal_upgrade_active(false));
    }

    #[test]
    fn propaganda_residual_pack_honesty() {
        assert!(honesty_propaganda_residual_ok());
        // Radius=150, DelayBetweenUpdates=2000ms→60f, HealPercent 2%/4%.
        assert_eq!(HOST_PROPAGANDA_TOWER_RADIUS, 150.0);
        assert_eq!(HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_MS, 2000);
        assert_eq!(HOST_PROPAGANDA_DELAY_BETWEEN_UPDATES_FRAMES, 60);
        assert_eq!(propaganda_ms_to_frames(2000), 60);
        assert!((HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC - 0.02).abs() < 0.0001);
        assert!((HOST_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC - 0.04).abs() < 0.0001);
        // Pulse-window residual: 2% * 100 * 2s = 4 HP base; 4% * 100 * 2s = 8 HP upgraded.
        assert!((propaganda_heal_amount_per_pulse(100.0, false) - 4.0).abs() < 0.001);
        assert!((propaganda_heal_amount_per_pulse(100.0, true) - 8.0).abs() < 0.001);
        // ENTHUSIASTIC / SUBLIMINAL residual flag discriminants.
        assert_eq!(WEAPON_BONUS_ENTHUSIASTIC, 8);
        assert_eq!(WEAPON_BONUS_SUBLIMINAL, 15);
        assert_eq!(ENTHUSIASTIC_CONDITION_NAME, "ENTHUSIASTIC");
        assert_eq!(SUBLIMINAL_CONDITION_NAME, "SUBLIMINAL");
        assert!((ENTHUSIASTIC_RATE_OF_FIRE_MULT - 1.25).abs() < 0.001);
        assert!((SUBLIMINAL_RATE_OF_FIRE_MULT - 1.25).abs() < 0.001);
        assert_eq!(
            propaganda_weapon_bonus_flags(false),
            (true, false)
        );
        assert_eq!(propaganda_weapon_bonus_flags(true), (true, true));
        // UpgradeRequired residual.
        assert_eq!(
            UPGRADE_CHINA_SUBLIMINAL_MESSAGING,
            "Upgrade_ChinaSubliminalMessaging"
        );
    }

    #[test]
    fn sole_benefactor_first_tower_wins_residual_honesty() {
        let mut excl = HostPropagandaHealExclusivity::new();
        assert!(!excl.honesty_exclusivity_ok());
        let target = ObjectId(10);
        let tower_a = ObjectId(1);
        let tower_b = ObjectId(2);
        assert!(excl.try_claim(target, tower_a));
        assert!(excl.honesty_exclusivity_ok());
        assert_eq!(excl.claimed_tower(target), Some(tower_a));
        // Second tower rejected for same target (multi-tower reject residual).
        assert!(!excl.try_claim(target, tower_b));
        assert!(excl.honesty_reject_ok());
        // Same tower re-claims ok.
        assert!(excl.try_claim(target, tower_a));
        assert_eq!(excl.claims_granted, 1);
        assert_eq!(excl.claims_rejected, 1);
        excl.clear_pulse();
        assert!(excl.try_claim(target, tower_b));
        assert_eq!(excl.claimed_tower(target), Some(tower_b));
    }
}
