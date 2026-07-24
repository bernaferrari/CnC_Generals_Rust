//! Host America Countermeasures residual (CountermeasuresBehavior).
//!
//! Residual slice (playability):
//! - When `Upgrade_AmericaCountermeasures` is applied to an aircraft, incoming
//!   projectiles may be diverted with retail **EvasionRate 30%**.
//! - Available flares residual: VolleySize **4** × NumberOfVolleys **5** = **20**
//!   (Raptor ModuleTag_11 baseline; other airframes may use 3 volleys).
//! - ReloadTime **0** → must reload at airfield residual (fail-closed full dock).
//! - Diverted missiles deal **no** Direct residual damage (decoy path).
//!
//! C++ path: `CountermeasuresBehavior::reportMissileForCountermeasures` rolls
//! `GameLogicRandomValueReal(0,1) < m_evasionRate`, then sets projectile
//! diversion delay. Host residual collapses delay into immediate miss.
//!
//! Fail-closed honesty:
//! - CountermeasureFlare SpecialObject spawn residual closed (LifetimeUpdate 3s)
//! - Not full bone volley arc / VolleyVelocityFactor locomotor matrix
//! - Not full calculateCountermeasureToDivertTo closest-flare seeker
//! - Airfield reload residual: docked at friendly airfield restores full load
//!   (C++ JetAIUpdate → reloadCountermeasures; ReloadTime=0 / MustReloadAtAirfield)
//! - Not full active-flare lifetime list / bone volley FX
//! - Shell `playable_claim` stays false; network deferred

use super::ObjectId;
use crate::game_logic::host_rng_residual::HostRandomState;
use crate::game_logic::host_upgrades::UPGRADE_AMERICA_COUNTERMEASURES;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Retail FlareTemplateName residual.
pub const FLARE_TEMPLATE_NAME: &str = "CountermeasureFlare";
/// Retail FlareBoneBaseName residual.
pub const FLARE_BONE_BASE_NAME: &str = "Flare";
/// Retail CountermeasureFlare LifetimeUpdate Min/MaxLifetime = 3000 ms → 90f @ 30 FPS.
pub const FLARE_LIFETIME_MS: u32 = 3_000;
pub const FLARE_LIFETIME_FRAMES: u32 = (FLARE_LIFETIME_MS * 30 + 999) / 1000;
/// Retail CountermeasureFlare body residual.
pub const FLARE_MAX_HEALTH: f32 = 1.0;

/// Retail VolleySize residual (Raptor ModuleTag_11).
pub const VOLLEY_SIZE: u32 = 4;
/// Retail NumberOfVolleys residual (Raptor = 5; Comanche/Aurora often 3).
pub const NUMBER_OF_VOLLEYS: u32 = 5;
/// Available countermeasures at full load: volley_size * number_of_volleys.
pub const FULL_LOAD_COUNTERMEASURES: u32 = VOLLEY_SIZE * NUMBER_OF_VOLLEYS; // 20

/// Retail EvasionRate residual (30%).
pub const EVASION_RATE: f32 = 0.30;
/// Retail EvasionRate percent string residual.
pub const EVASION_RATE_STR: &str = "30%";

/// Retail DelayBetweenVolleys residual msec.
pub const DELAY_BETWEEN_VOLLEYS_MS: u32 = 1_000;
/// DelayBetweenVolleys frames residual (1000 ms → 30).
pub const DELAY_BETWEEN_VOLLEYS_FRAMES: u32 = 30;

/// Retail MissileDecoyDelay residual msec.
pub const MISSILE_DECOY_DELAY_MS: u32 = 200;
/// MissileDecoyDelay frames residual (200 ms → 6).
pub const MISSILE_DECOY_DELAY_FRAMES: u32 = 6;

/// Retail ReactionLaunchLatency residual msec (0 → immediate first volley residual).
pub const REACTION_LAUNCH_LATENCY_MS: u32 = 0;

/// Retail ReloadTime residual msec (0 → airfield-only reload residual).
pub const RELOAD_TIME_MS: u32 = 0;
/// Retail MustReloadAtAirfield residual (America air CountermeasuresBehavior).
pub const MUST_RELOAD_AT_AIRFIELD: bool = true;

/// Retail VolleyArcAngle residual degrees.
pub const VOLLEY_ARC_ANGLE_DEG: f32 = 90.0;
/// Retail VolleyVelocityFactor residual.
pub const VOLLEY_VELOCITY_FACTOR: f32 = 2.0;

/// Per-aircraft countermeasures residual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostCountermeasuresState {
    pub available: u32,
    pub active: u32,
    pub incoming_missiles: u32,
    pub diverted_missiles: u32,
    pub volleys_fired: u32,
}

impl Default for HostCountermeasuresState {
    fn default() -> Self {
        Self {
            available: FULL_LOAD_COUNTERMEASURES,
            active: 0,
            incoming_missiles: 0,
            diverted_missiles: 0,
            volleys_fired: 0,
        }
    }
}

impl HostCountermeasuresState {
    pub fn full_load() -> Self {
        Self::default()
    }

    /// Airfield reload residual (ReloadTime = 0 → only via this path).
    pub fn reload_at_airfield(&mut self) {
        self.available = FULL_LOAD_COUNTERMEASURES;
        self.active = 0;
        self.volleys_fired = 0;
    }

    /// True when any flares remain (available or currently active residual).
    pub fn has_flares(&self) -> bool {
        self.available.saturating_add(self.active) > 0
    }
}

/// Host registry of countermeasures residual by aircraft ObjectId.
/// Pending CountermeasureFlare SpecialObject spawn residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCountermeasureFlareSpawn {
    pub aircraft_id: ObjectId,
    pub frame: u32,
    pub volley_index: u32,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct HostCountermeasuresRegistry {
    states: HashMap<u32, HostCountermeasuresState>,
    total_reports: u32,
    total_diverts: u32,
    total_reloads: u32,
    pub flares_spawned: u32,
    pending_flare_spawns: Vec<PendingCountermeasureFlareSpawn>,
}

impl HostCountermeasuresRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.states.clear();
        self.total_reports = 0;
        self.total_diverts = 0;
        self.total_reloads = 0;
        self.flares_spawned = 0;
        self.pending_flare_spawns.clear();
    }

    /// Drain pending CountermeasureFlare spawn residuals.
    pub fn take_pending_flare_spawns(&mut self) -> Vec<PendingCountermeasureFlareSpawn> {
        std::mem::take(&mut self.pending_flare_spawns)
    }

    pub fn record_flare_spawned(&mut self, n: u32) {
        self.flares_spawned = self.flares_spawned.saturating_add(n);
    }

    pub fn honesty_flare_spawn_ok(&self) -> bool {
        self.flares_spawned > 0
    }

    /// LifetimeUpdate expired residual — free active flare slot bookkeeping.
    pub fn note_flare_expired(&mut self, aircraft_id: ObjectId) {
        if let Some(st) = self.states.get_mut(&aircraft_id.0) {
            st.active = st.active.saturating_sub(1);
        }
    }

    pub fn ensure(&mut self, aircraft_id: ObjectId) -> &mut HostCountermeasuresState {
        self.states
            .entry(aircraft_id.0)
            .or_insert_with(HostCountermeasuresState::full_load)
    }

    pub fn get(&self, aircraft_id: ObjectId) -> Option<&HostCountermeasuresState> {
        self.states.get(&aircraft_id.0)
    }

    pub fn reload_at_airfield(&mut self, aircraft_id: ObjectId) {
        self.ensure(aircraft_id).reload_at_airfield();
        self.total_reloads = self.total_reloads.saturating_add(1);
    }

    pub fn total_reports(&self) -> u32 {
        self.total_reports
    }

    pub fn total_diverts(&self) -> u32 {
        self.total_diverts
    }

    pub fn total_reloads(&self) -> u32 {
        self.total_reloads
    }

    pub fn honesty_divert_ok(&self) -> bool {
        self.total_diverts > 0
    }

    pub fn honesty_report_ok(&self) -> bool {
        self.total_reports > 0
    }
}

/// True when aircraft has Countermeasures upgrade residual tag.
#[inline]
pub fn aircraft_has_countermeasures_upgrade(
    applied_upgrades: &std::collections::HashSet<String>,
) -> bool {
    applied_upgrades.iter().any(|u| {
        let n = u.to_ascii_lowercase();
        n.contains("countermeasure") || u == UPGRADE_AMERICA_COUNTERMEASURES
    })
}

/// C++ reportMissileForCountermeasures residual: roll evasion, consume one flare.
///
/// Returns `true` when the missile is diverted (no Direct residual damage).
/// Deterministic RNG: seed from aircraft_id ^ projectile_id ^ frame.
pub fn try_divert_missile(
    reg: &mut HostCountermeasuresRegistry,
    aircraft_id: ObjectId,
    projectile_id: ObjectId,
    frame: u32,
    has_upgrade: bool,
) -> bool {
    if !has_upgrade {
        return false;
    }
    reg.total_reports = reg.total_reports.saturating_add(1);
    let st = reg.ensure(aircraft_id);
    st.incoming_missiles = st.incoming_missiles.saturating_add(1);
    if !st.has_flares() {
        return false;
    }
    // Deterministic GameLogicRandomValueReal residual.
    let seed = aircraft_id
        .0
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(projectile_id.0)
        .wrapping_add(frame.wrapping_mul(0x85EB_CA6B));
    let mut rng = HostRandomState::seeded(seed);
    let roll = rng.next_real(0.0, 1.0);
    if roll >= EVASION_RATE {
        return false;
    }
    // Launch one volley residual (VolleySize flares) when available.
    let flares = st.available.min(VOLLEY_SIZE);
    if flares > 0 {
        st.available = st.available.saturating_sub(flares);
        st.active = st.active.saturating_add(flares);
    }
    st.diverted_missiles = st.diverted_missiles.saturating_add(1);
    st.volleys_fired = st.volleys_fired.saturating_add(1);
    reg.total_diverts = reg.total_diverts.saturating_add(1);
    for vi in 0..flares {
        reg.pending_flare_spawns.push(PendingCountermeasureFlareSpawn {
            aircraft_id,
            frame,
            volley_index: vi,
        });
    }
    let _ = projectile_id;
    true
}

/// Wave residual honesty pack.
pub fn honesty_countermeasures_residual_pack_ok() -> bool {
    FLARE_TEMPLATE_NAME == "CountermeasureFlare"
        && FLARE_BONE_BASE_NAME == "Flare"
        && FLARE_LIFETIME_FRAMES == 90
        && VOLLEY_SIZE == 4
        && NUMBER_OF_VOLLEYS == 5
        && FULL_LOAD_COUNTERMEASURES == 20
        && (EVASION_RATE - 0.30).abs() < 1e-6
        && EVASION_RATE_STR == "30%"
        && DELAY_BETWEEN_VOLLEYS_MS == 1_000
        && DELAY_BETWEEN_VOLLEYS_FRAMES == 30
        && MISSILE_DECOY_DELAY_MS == 200
        && MISSILE_DECOY_DELAY_FRAMES == 6
        && REACTION_LAUNCH_LATENCY_MS == 0
        && RELOAD_TIME_MS == 0
        && MUST_RELOAD_AT_AIRFIELD
        && (VOLLEY_ARC_ANGLE_DEG - 90.0).abs() < 1e-3
        && (VOLLEY_VELOCITY_FACTOR - 2.0).abs() < 1e-3
        && UPGRADE_AMERICA_COUNTERMEASURES == "Upgrade_AmericaCountermeasures"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn countermeasures_residual_pack_honesty() {
        assert!(honesty_countermeasures_residual_pack_ok());
    }

    #[test]
    fn diversion_requires_upgrade_and_flares() {
        let mut reg = HostCountermeasuresRegistry::new();
        let air = ObjectId(1);
        let proj = ObjectId(99);
        assert!(!try_divert_missile(&mut reg, air, proj, 1, false));
        // With upgrade: some frames divert (30%). Exhaust flares.
        let mut any = false;
        for f in 0..200u32 {
            if try_divert_missile(&mut reg, air, ObjectId(100 + f), f, true) {
                any = true;
            }
        }
        assert!(any, "expected some diversions at 30% over 200 rolls");
        assert!(reg.honesty_report_ok());
        assert!(reg.honesty_divert_ok());
        // Exhaust remaining by force-zero available.
        if let Some(st) = reg.states.get_mut(&1) {
            st.available = 0;
            st.active = 0;
        }
        assert!(!try_divert_missile(
            &mut reg,
            air,
            ObjectId(9999),
            999,
            true
        ));
    }

    #[test]
    fn airfield_reload_restores_full_load() {
        let mut reg = HostCountermeasuresRegistry::new();
        let air = ObjectId(7);
        {
            let st = reg.ensure(air);
            st.available = 0;
            st.volleys_fired = 5;
        }
        assert_eq!(reg.get(air).map(|s| s.available), Some(0));
        reg.reload_at_airfield(air);
        assert_eq!(
            reg.get(air).map(|s| s.available),
            Some(FULL_LOAD_COUNTERMEASURES)
        );
        assert_eq!(reg.total_reloads(), 1);
    }

    #[test]
    fn upgrade_tag_detects_countermeasures() {
        let mut s = HashSet::new();
        assert!(!aircraft_has_countermeasures_upgrade(&s));
        s.insert(UPGRADE_AMERICA_COUNTERMEASURES.to_string());
        assert!(aircraft_has_countermeasures_upgrade(&s));
    }
}
