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
//! Fail-closed honesty:
//! - Not full sole-benefactor exclusivity / multi-tower reject matrix
//! - Not full ally relationship filter (uses same-team residual)
//! - Not full double-contain / stealthed FX suppress / POWERED underpower gate
//! - Not full player vs object UpgradeType switch matrix beyond residual tag
//! - Not full PulseFX / world-anim propaganda pulse
//! - Not network propaganda replication (network deferred)

/// Retail speaker-tower scan radius residual (ChinaSpeakerTower Radius = 150).
pub const HOST_PROPAGANDA_TOWER_RADIUS: f32 = 150.0;

/// Retail base heal percent of max health per second (HealPercentEachSecond = 2%).
pub const HOST_PROPAGANDA_HEAL_PERCENT_PER_SEC: f32 = 0.02;

/// Retail upgraded heal percent of max health per second (4%).
pub const HOST_PROPAGANDA_UPGRADED_HEAL_PERCENT_PER_SEC: f32 = 0.04;

/// Player upgrade residual that enables SUBLIMINAL + upgraded heal rate.
pub const UPGRADE_CHINA_SUBLIMINAL_MESSAGING: &str = "Upgrade_ChinaSubliminalMessaging";

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

/// Whether tower/player residual has subliminal upgrade active.
pub fn is_subliminal_upgrade_active(has_upgrade_tag: bool) -> bool {
    has_upgrade_tag
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
}
