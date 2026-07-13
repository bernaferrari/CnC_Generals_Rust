//! Host America Microwave Tank residual (disable buildings + clear garrison).
//!
//! Residual slice (playability):
//! - AmericaTankMicrowave / *Microwave* residual sources continuously subdue
//!   enemy/neutral structures they are actively attacking within residual
//!   MicrowaveTankBuildingDisabler AttackRange **200** (SUBDUAL_BUILDING →
//!   DISABLED_SUBDUED). Subdued structures count as `is_disabled()` so
//!   production / powered functions stop while cooked.
//! - Garrison clear residual (KILL_GARRISONED / MicrowaveTankBuildingClearer)
//!   is applied via the existing combat path (`host_bunker_buster` clearer
//!   residual): floor(damage) occupants killed, no structure HP damage.
//!
//! Fail-closed honesty:
//! - Not full subdual damage accumulate / SubdualDamageHelper heal drain
//! - Not full MicrowaveDisableStream laser attach / FireWeaponUpdate emitter
//!   infantry MICROWAVE damage field (MicrowaveTankEmitterWeapon)
//! - Not full vehicle disabler (retail WeaponSet has VehicleDisabler commented out)
//! - Not network microwave replication (network deferred)

use serde::{Deserialize, Serialize};

/// Retail MicrowaveTankBuildingDisabler AttackRange residual.
pub const HOST_MICROWAVE_DISABLE_RANGE: f32 = 200.0;

/// Retail MicrowaveTankBuildingClearer AttackRange residual (secondary).
pub const HOST_MICROWAVE_CLEAR_RANGE: f32 = 125.0;

/// Retail MicrowaveTankBuildingDisabler PrimaryDamage residual (subdual/pulse).
/// Fail-closed continuous residual does not accumulate; used for honesty docs.
pub const HOST_MICROWAVE_SUBDUAL_PULSE: f32 = 50.0;

/// Retail MicrowaveTankBuildingClearer PrimaryDamage residual (= 1 occupant).
pub const HOST_MICROWAVE_CLEAR_PER_SHOT: f32 = 1.0;

/// Activate / cook audio residual.
pub const MICROWAVE_DISABLE_AUDIO: &str = "MicrowaveWeaponLoop";

/// Whether template is a residual Microwave Tank source.
///
/// Fail-closed: name residual (not full INI WeaponSet / FireWeaponUpdate matrix).
pub fn is_microwave_tank(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testmicrowave" || n == "testmicrowavetank" {
        return true;
    }
    // AmericaTankMicrowave / USA_MicrowaveTank / Lazr_AmericaTankMicrowave / …
    if n.contains("microwave") {
        // Skip pure weapon / stream FX object names.
        if n.contains("stream")
            || n.contains("emitter")
            || n.contains("weapon")
            || n.contains("fx")
            || n.contains("particle")
        {
            return false;
        }
        return true;
    }
    false
}

/// Whether residual target can be subdued by a Microwave building disabler.
///
/// Retail: SUBDUAL_BUILDING on structures; RadiusDamageAffects ALLIES ENEMIES NEUTRALS
/// but residual only cooks enemy/neutral (fail-closed vs ally disable griefing).
pub fn is_legal_microwave_disable_target(
    is_structure: bool,
    is_alive: bool,
    enemy_or_neutral: bool,
    under_construction: bool,
) -> bool {
    is_structure && is_alive && enemy_or_neutral && !under_construction
}

/// True when microwave team vs target team is residual-hostile (enemy) or Neutral victim.
pub fn is_microwave_hostile_team(
    microwave_team_is_neutral: bool,
    same_team: bool,
    target_is_neutral: bool,
) -> bool {
    if microwave_team_is_neutral {
        // Neutral microwave residual does not cook anyone (fail-closed).
        return false;
    }
    !same_team || target_is_neutral
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_microwave_range_2d(src: (f32, f32), dst: (f32, f32), range: f32) -> bool {
    let dx = src.0 - dst.0;
    let dy = src.1 - dst.1;
    dx * dx + dy * dy <= range * range
}

/// Whether residual microwave should cook this structure target (attacking + range).
pub fn should_microwave_disable(
    is_microwave: bool,
    microwave_alive: bool,
    microwave_attacking: bool,
    has_target: bool,
    in_range: bool,
    legal_target: bool,
) -> bool {
    is_microwave
        && microwave_alive
        && microwave_attacking
        && has_target
        && in_range
        && legal_target
}

/// Host residual honesty counters for microwave tank.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostMicrowaveRegistry {
    /// Times residual applied DISABLED_SUBDUED to a structure (new grants).
    pub disable_grants: u32,
    /// Frames / ticks where at least one structure remained cooked.
    pub disable_ticks: u32,
    /// Structures currently cooked at last update (diagnostic).
    pub currently_disabled: u32,
}

impl HostMicrowaveRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn record_disable_grant(&mut self) {
        self.disable_grants = self.disable_grants.saturating_add(1);
        self.disable_ticks = self.disable_ticks.saturating_add(1);
    }

    pub fn record_disable_refresh(&mut self) {
        self.disable_ticks = self.disable_ticks.saturating_add(1);
    }

    pub fn set_currently_disabled(&mut self, count: u32) {
        self.currently_disabled = count;
    }

    /// Residual honesty: at least one structure was disabled by microwave.
    pub fn honesty_disable_ok(&self) -> bool {
        self.disable_grants > 0
    }

    /// Combined host path honesty for microwave disable residual.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_disable_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn microwave_name_matrix() {
        assert!(is_microwave_tank("AmericaTankMicrowave"));
        assert!(is_microwave_tank("USA_MicrowaveTank"));
        assert!(is_microwave_tank("Lazr_AmericaTankMicrowave"));
        assert!(is_microwave_tank("TestMicrowave"));
        assert!(is_microwave_tank("TestMicrowaveTank"));
        assert!(!is_microwave_tank("MicrowaveDisableStream"));
        assert!(!is_microwave_tank("MicrowaveTankEmitterWeapon"));
        assert!(!is_microwave_tank("USA_Ranger"));
        assert!(!is_microwave_tank("ChinaTankECM"));
        assert!(!is_microwave_tank("TestTank"));
    }

    #[test]
    fn legal_target_and_team_filters() {
        // structure, alive, enemy_or_neutral, under_construction
        assert!(is_legal_microwave_disable_target(true, true, true, false));
        assert!(!is_legal_microwave_disable_target(false, true, true, false));
        assert!(!is_legal_microwave_disable_target(true, false, true, false));
        assert!(!is_legal_microwave_disable_target(true, true, false, false));
        assert!(!is_legal_microwave_disable_target(true, true, true, true));

        assert!(is_microwave_hostile_team(false, false, false)); // enemy
        assert!(is_microwave_hostile_team(false, false, true)); // neutral victim
        assert!(!is_microwave_hostile_team(false, true, false)); // ally
        assert!(!is_microwave_hostile_team(true, false, false)); // neutral microwave
    }

    #[test]
    fn range_and_should_disable() {
        assert!(HOST_MICROWAVE_DISABLE_RANGE > HOST_MICROWAVE_CLEAR_RANGE);
        assert!(in_microwave_range_2d((0.0, 0.0), (150.0, 0.0), 200.0));
        assert!(!in_microwave_range_2d((0.0, 0.0), (250.0, 0.0), 200.0));
        assert!(should_microwave_disable(
            true, true, true, true, true, true
        ));
        assert!(!should_microwave_disable(
            true, true, false, true, true, true
        ));
        assert!(!should_microwave_disable(
            false, true, true, true, true, true
        ));
    }

    #[test]
    fn honesty_tracks_disable_grants() {
        let mut reg = HostMicrowaveRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        reg.record_disable_grant();
        assert!(reg.honesty_disable_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.disable_grants, 1);
    }
}
