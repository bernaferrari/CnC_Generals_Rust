//! Host SCIENCE_StealthFighter production unlock residual.
//!
//! Residual slice (playability):
//! - Retail AmericaJetStealthFighter (and science-gated variants) require
//!   `Prerequisites Science = SCIENCE_StealthFighter` before airfield production.
//! - Unlocking SCIENCE_StealthFighter allows host `enqueue_production` of gated
//!   stealth aircraft from an Airfield.
//! - Without the science, production of gated templates fails (fail-closed).
//! - Map/script `create_object` is **not** gated — only the production queue path
//!   mirrors the retail science Prerequisite on construct.
//!
//! Airforce General residual: `AirF_AmericaJetStealthFighter` does **not** list
//! SCIENCE_StealthFighter in retail Prerequisites (airfield only) — host residual
//! leaves AirF free to produce without the science.
//!
//! Fail-closed honesty:
//! - Not full PrerequisiteSciences rank tree (SCIENCE_AMERICA / SCIENCE_Rank1)
//! - Not full control-bar CommandButton science visibility matrix
//! - Not full INI Object/Science load path for every faction general variant
//! - Not multiplayer science replication (network deferred)

use serde::{Deserialize, Serialize};

/// Retail science that gates Stealth Fighter production.
pub const SCIENCE_STEALTH_FIGHTER: &str = "SCIENCE_StealthFighter";

/// Canonical retail USA Stealth Fighter object name.
pub const AMERICA_JET_STEALTH_FIGHTER: &str = "AmericaJetStealthFighter";

/// Host residual alias used by some USA seed tables / HUD labels.
pub const USA_STEALTH_FIGHTER: &str = "USA_StealthFighter";

/// Retail BuildCost residual (AmericaAir.ini AmericaJetStealthFighter).
pub const STEALTH_FIGHTER_BUILD_COST: u32 = 1600;

/// Retail BuildTime residual seconds (AmericaAir.ini = 25.0).
pub const STEALTH_FIGHTER_BUILD_TIME: f32 = 25.0;

/// Normalize science / template identity (alphanumeric lower).
pub fn normalize_identity(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Whether a science / purchase name is SCIENCE_StealthFighter residual.
pub fn is_stealth_fighter_science(name: &str) -> bool {
    let n = normalize_identity(name);
    n == "sciencestealthfighter" || n == "stealthfighter"
}

/// Whether a unit template requires SCIENCE_StealthFighter for production.
///
/// Retail Prerequisites Science = SCIENCE_StealthFighter applies to:
/// - AmericaJetStealthFighter
/// - SupW_AmericaJetStealthFighter
/// - Lazr_AmericaJetStealthFighter
/// - CINE_AmericaJetStealthFighter*
/// Host residual also gates USA_StealthFighter / USA residual aliases.
///
/// Explicitly **not** gated: AirF_AmericaJetStealthFighter (Airforce General free).
pub fn requires_stealth_fighter_science(template_name: &str) -> bool {
    let n = normalize_identity(template_name);
    if n.is_empty() {
        return false;
    }
    // Airforce General residual: no science Prerequisite in retail.
    if n.starts_with("airf") {
        return false;
    }
    // Canonical + general variants that carry Science = SCIENCE_StealthFighter.
    if n.contains("stealthfighter") || n.contains("jetstealth") {
        return true;
    }
    false
}

/// Production gate: science-gated templates require unlock; others always ok.
pub fn player_may_produce_stealth_aircraft(has_science: bool, template_name: &str) -> bool {
    if !requires_stealth_fighter_science(template_name) {
        return true;
    }
    has_science
}

/// Host residual honesty registry for Stealth Fighter science → production.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostStealthFighterRegistry {
    /// Times SCIENCE_StealthFighter was unlocked on a player (session residual).
    pub science_unlock_count: u32,
    /// Times a science-gated stealth aircraft was accepted into a production queue.
    pub production_enqueue_count: u32,
    /// Times a science-gated stealth aircraft finished production and spawned.
    pub production_spawn_count: u32,
    /// Times production was rejected solely due to missing science.
    pub production_denied_count: u32,
}

impl HostStealthFighterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_science_unlock(&mut self) {
        self.science_unlock_count = self.science_unlock_count.saturating_add(1);
    }

    pub fn record_production_enqueue(&mut self) {
        self.production_enqueue_count = self.production_enqueue_count.saturating_add(1);
    }

    pub fn record_production_spawn(&mut self) {
        self.production_spawn_count = self.production_spawn_count.saturating_add(1);
    }

    pub fn record_production_denied(&mut self) {
        self.production_denied_count = self.production_denied_count.saturating_add(1);
    }

    /// Residual honesty: science was unlocked at least once.
    pub fn honesty_unlock_ok(&self) -> bool {
        self.science_unlock_count > 0
    }

    /// Residual honesty: science-gated production was accepted at least once.
    pub fn honesty_produce_ok(&self) -> bool {
        self.production_enqueue_count > 0
    }

    /// Residual honesty: deny path observed (missing science rejected produce).
    pub fn honesty_deny_ok(&self) -> bool {
        self.production_denied_count > 0
    }

    /// Residual honesty: production finished and unit spawned.
    pub fn honesty_spawn_ok(&self) -> bool {
        self.production_spawn_count > 0
    }

    /// Combined residual honesty (unlock + produce). Spawn optional for queue-only tests.
    pub fn honesty_ok(&self) -> bool {
        self.honesty_unlock_ok() && self.honesty_produce_ok()
    }

    /// Full host path: unlock → queue → spawn.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_ok() && self.honesty_spawn_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn science_name_recognition() {
        assert!(is_stealth_fighter_science(SCIENCE_STEALTH_FIGHTER));
        assert!(is_stealth_fighter_science("SCIENCE_StealthFighter"));
        assert!(is_stealth_fighter_science("stealthfighter"));
        assert!(!is_stealth_fighter_science("SCIENCE_StealthFighter_x"));
        assert!(!is_stealth_fighter_science("SCIENCE_CashBounty1"));
        assert!(!is_stealth_fighter_science("SCIENCE_Paladin"));
    }

    #[test]
    fn template_science_gate_matrix() {
        assert!(requires_stealth_fighter_science(AMERICA_JET_STEALTH_FIGHTER));
        assert!(requires_stealth_fighter_science("SupW_AmericaJetStealthFighter"));
        assert!(requires_stealth_fighter_science("Lazr_AmericaJetStealthFighter"));
        assert!(requires_stealth_fighter_science("CINE_AmericaJetStealthFighter"));
        assert!(requires_stealth_fighter_science(USA_STEALTH_FIGHTER));
        // Airforce General free residual.
        assert!(!requires_stealth_fighter_science("AirF_AmericaJetStealthFighter"));
        assert!(!requires_stealth_fighter_science("USA_Raptor"));
        assert!(!requires_stealth_fighter_science("TestAircraft"));
    }

    #[test]
    fn production_gate_requires_science() {
        assert!(!player_may_produce_stealth_aircraft(false, AMERICA_JET_STEALTH_FIGHTER));
        assert!(player_may_produce_stealth_aircraft(true, AMERICA_JET_STEALTH_FIGHTER));
        assert!(player_may_produce_stealth_aircraft(
            false,
            "AirF_AmericaJetStealthFighter"
        ));
        assert!(player_may_produce_stealth_aircraft(false, "USA_Raptor"));
    }

    #[test]
    fn honesty_tracks_unlock_produce_spawn() {
        let mut reg = HostStealthFighterRegistry::new();
        assert!(!reg.honesty_ok());
        reg.record_science_unlock();
        assert!(reg.honesty_unlock_ok());
        assert!(!reg.honesty_ok());
        reg.record_production_enqueue();
        assert!(reg.honesty_ok());
        assert!(!reg.honesty_host_path_ok());
        reg.record_production_spawn();
        assert!(reg.honesty_host_path_ok());
        reg.record_production_denied();
        assert!(reg.honesty_deny_ok());
    }
}
