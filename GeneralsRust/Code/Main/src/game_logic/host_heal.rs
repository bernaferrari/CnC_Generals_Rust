//! Host infantry heal residual (USA Ambulance AutoHeal + HealPad honesty).
//!
//! Residual slice (playability):
//! - AmericaVehicleMedic / Ambulance: C++ `AutoHealBehavior` radius pulse residual —
//!   heals damaged **ally infantry** in radius over time (INI ModuleTag_22:
//!   HealingAmount=4, HealingDelay=1000ms, Radius=100, KindOf=INFANTRY, StartsActive=Yes).
//! - HealPad `GetHealed` / `AIState::SeekingHealing` residual honesty counters
//!   (heal application lives in `GameLogic::update_support_states`).
//!
//! Fail-closed honesty:
//! - Not full sole-benefactor exclusivity / multi-ambulance reject matrix
//! - Not full vehicle AutoHeal ModuleTag_23 (VEHICLE, ForbiddenKindOf=AIRCRAFT)
//! - Not full TransportContain HealthRegen%PerSec while embarked
//! - Not full particle / world-anim heal pulse FX
//! - Not network heal replication (network deferred)

/// Retail ambulance infantry pulse amount residual (AutoHealBehavior HealingAmount).
pub const AMBULANCE_INFANTRY_HEAL_AMOUNT: f32 = 4.0;

/// Retail ambulance heal delay residual in seconds (HealingDelay = 1000 ms).
pub const AMBULANCE_HEAL_DELAY_SEC: f32 = 1.0;

/// Continuous residual rate equivalent to amount/delay pulse average.
pub const HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC: f32 =
    AMBULANCE_INFANTRY_HEAL_AMOUNT / AMBULANCE_HEAL_DELAY_SEC;

/// Retail ambulance AutoHeal radius residual (AmericaVehicleMedic Radius = 100).
pub const HOST_AMBULANCE_HEAL_RADIUS: f32 = 100.0;

/// Whether template is a residual ambulance / medic healer unit.
///
/// Fail-closed: name-based residual (not full INI AutoHealBehavior module matrix).
pub fn is_ambulance_healer(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("ambulance") || n.contains("vehiclemedic") || n.ends_with("medic")
}

/// Whether residual target can receive ambulance infantry AutoHeal.
pub fn is_legal_ambulance_infantry_heal_target(
    is_infantry: bool,
    is_alive: bool,
    is_damaged: bool,
    same_team: bool,
    is_self: bool,
) -> bool {
    is_infantry && is_alive && is_damaged && same_team && !is_self
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_heal_radius_2d(healer_pos: (f32, f32), target_pos: (f32, f32), radius: f32) -> bool {
    let dx = healer_pos.0 - target_pos.0;
    let dy = healer_pos.1 - target_pos.1;
    dx * dx + dy * dy <= radius * radius
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambulance_healer_name_matrix() {
        assert!(is_ambulance_healer("AmericaVehicleMedic"));
        assert!(is_ambulance_healer("USA_Ambulance"));
        assert!(is_ambulance_healer("SupW_AmericaVehicleMedic"));
        assert!(is_ambulance_healer("Lazr_AmericaVehicleMedic"));
        assert!(!is_ambulance_healer("USA_Ranger"));
        assert!(!is_ambulance_healer("TestInfantry"));
        assert!(!is_ambulance_healer("USA_Dozer"));
        assert!(!is_ambulance_healer("ChinaInfantryRedGuard"));
    }

    #[test]
    fn legal_infantry_heal_target_matrix() {
        assert!(is_legal_ambulance_infantry_heal_target(
            true, true, true, true, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            false, true, true, true, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            true, false, true, true, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            true, true, false, true, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            true, true, true, false, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            true, true, true, true, true
        ));
    }

    #[test]
    fn heal_radius_and_rate_positive() {
        assert!(HOST_AMBULANCE_HEAL_RADIUS > 0.0);
        assert!(HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC > 0.0);
        assert!(in_heal_radius_2d((0.0, 0.0), (50.0, 0.0), 100.0));
        assert!(!in_heal_radius_2d((0.0, 0.0), (150.0, 0.0), 100.0));
    }
}
