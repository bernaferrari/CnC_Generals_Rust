//! Host America Countermeasures (CountermeasuresBehavior).
//!
//! C++ `CountermeasuresBehavior::reportMissileForCountermeasures` rolls
//! `GameLogicRandomValueReal(0,1) < m_evasionRate`, then sets projectile
//! `framesTillDecoyed` (MissileDecoyDelay 200 ms → 6f). Volleys launch later
//! from `update` while airborne. Diverted missiles seek the closest flare of
//! the newest volley and deal no detonation damage.
//!
//! Retail pack:
//! - EvasionRate **30%**
//! - VolleySize **4** × NumberOfVolleys **5** = **20** (Raptor ModuleTag_11).
//!   Comanche / Aurora use **3** volleys (12 flares).
//! - ReloadTime **0** → airfield-only reload (MustReloadAtAirfield).
//! - DelayBetweenVolleys **1000** ms → 30f; ReactionLaunchLatency **0**.
//! - VolleyArcAngle **90°**, VolleyVelocityFactor **2.0**.

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
    /// C++ `m_reactionFrame`. Meaningful when `reaction_armed`.
    #[serde(default)]
    pub reaction_frame: u32,
    /// Distinguishes an armed reaction at frame 0 from C++'s 0=unset.
    #[serde(default)]
    pub reaction_armed: bool,
    /// C++ `m_nextVolleyFrame`. 0 = no continuation volley scheduled.
    #[serde(default)]
    pub next_volley_frame: u32,
    /// Per-airframe `NumberOfVolleys` (Raptor 5, Comanche/Aurora 3).
    #[serde(default = "default_number_of_volleys")]
    pub number_of_volleys: u32,
    /// C++ `m_counterMeasures` (newest at the back).
    #[serde(default)]
    pub flare_ids: Vec<ObjectId>,
}

fn default_number_of_volleys() -> u32 {
    NUMBER_OF_VOLLEYS
}

impl Default for HostCountermeasuresState {
    fn default() -> Self {
        Self::full_load_with_volleys(NUMBER_OF_VOLLEYS)
    }
}

impl HostCountermeasuresState {
    pub fn full_load() -> Self {
        Self::full_load_with_volleys(NUMBER_OF_VOLLEYS)
    }

    pub fn full_load_with_volleys(number_of_volleys: u32) -> Self {
        let n = number_of_volleys.max(1);
        Self {
            available: VOLLEY_SIZE.saturating_mul(n),
            active: 0,
            incoming_missiles: 0,
            diverted_missiles: 0,
            volleys_fired: 0,
            reaction_frame: 0,
            reaction_armed: false,
            next_volley_frame: 0,
            number_of_volleys: n,
            flare_ids: Vec::new(),
        }
    }

    pub fn full_load_count(&self) -> u32 {
        VOLLEY_SIZE.saturating_mul(self.number_of_volleys.max(1))
    }

    /// Airfield reload residual (ReloadTime = 0 → only via this path).
    pub fn reload_at_airfield(&mut self) {
        self.available = self.full_load_count();
        self.active = 0;
        self.volleys_fired = 0;
        self.reaction_frame = 0;
        self.reaction_armed = false;
        self.next_volley_frame = 0;
        self.flare_ids.clear();
    }

    /// True when any flares remain (available or currently active residual).
    pub fn has_flares(&self) -> bool {
        self.available.saturating_add(self.active) > 0
    }
}

/// Retail `NumberOfVolleys` for an airframe template.
/// Raptor ModuleTag_11 = 5; Comanche / Aurora = 3.
pub fn number_of_volleys_for_template(template_name: &str) -> u32 {
    let n = template_name.to_ascii_lowercase();
    if n.contains("comanche") || n.contains("aurora") {
        3
    } else {
        NUMBER_OF_VOLLEYS
    }
}

/// C++ `LOCOMOTORSET_SUPERSONIC` gate (Weapon.cpp:1148-1149).
pub fn victim_locomotor_is_supersonic(locomotor_set: Option<&str>) -> bool {
    locomotor_set
        .map(|s| s.to_ascii_uppercase().contains("SUPERSONIC"))
        .unwrap_or(false)
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

    /// Drop a destroyed flare id from the newest-volley seek list.
    pub fn forget_flare_id(&mut self, aircraft_id: ObjectId, flare_id: ObjectId) {
        if let Some(st) = self.states.get_mut(&aircraft_id.0) {
            st.flare_ids.retain(|id| *id != flare_id);
        }
    }

    /// Record a spawned flare id (C++ `m_counterMeasures.push_back`).
    pub fn record_flare_id(&mut self, aircraft_id: ObjectId, flare_id: ObjectId) {
        self.ensure(aircraft_id).flare_ids.push(flare_id);
    }

    pub fn aircraft_ids(&self) -> Vec<ObjectId> {
        self.states.keys().copied().map(ObjectId).collect()
    }

    /// Snapshot remaining same-frame flare spawns (host queues; C++ creates inline).
    pub fn pending_flare_spawns(&self) -> &[PendingCountermeasureFlareSpawn] {
        &self.pending_flare_spawns
    }

    /// Replace the flare registry entry without `ensure()` rebuilding a full load.
    pub fn restore_state(&mut self, aircraft_id: ObjectId, state: HostCountermeasuresState) {
        self.states.insert(aircraft_id.0, state);
    }

    pub fn restore_pending_flare_spawns(&mut self, pending: Vec<PendingCountermeasureFlareSpawn>) {
        self.pending_flare_spawns = pending;
    }

    pub fn ensure(&mut self, aircraft_id: ObjectId) -> &mut HostCountermeasuresState {
        self.states
            .entry(aircraft_id.0)
            .or_insert_with(HostCountermeasuresState::full_load)
    }

    pub fn ensure_for_template(
        &mut self,
        aircraft_id: ObjectId,
        template_name: &str,
    ) -> &mut HostCountermeasuresState {
        let n = number_of_volleys_for_template(template_name);
        self.states
            .entry(aircraft_id.0)
            .or_insert_with(|| HostCountermeasuresState::full_load_with_volleys(n))
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

/// C++ `reportMissileForCountermeasures`: increment incoming, roll evasion,
/// arm reaction. Does **not** consume flares or spawn a volley.
///
/// Returns `true` when the missile is marked for delayed diversion
/// (`setFramesTillCountermeasureDiversionOccurs`).
pub fn report_missile_for_countermeasures(
    reg: &mut HostCountermeasuresRegistry,
    aircraft_id: ObjectId,
    projectile_id: ObjectId,
    frame: u32,
    has_upgrade: bool,
) -> bool {
    report_missile_for_countermeasures_named(
        reg,
        aircraft_id,
        projectile_id,
        frame,
        has_upgrade,
        None,
    )
}

pub fn report_missile_for_countermeasures_named(
    reg: &mut HostCountermeasuresRegistry,
    aircraft_id: ObjectId,
    projectile_id: ObjectId,
    frame: u32,
    has_upgrade: bool,
    template_name: Option<&str>,
) -> bool {
    if !has_upgrade {
        return false;
    }
    reg.total_reports = reg.total_reports.saturating_add(1);
    let st = match template_name {
        Some(name) => reg.ensure_for_template(aircraft_id, name),
        None => reg.ensure(aircraft_id),
    };
    st.incoming_missiles = st.incoming_missiles.saturating_add(1);
    if !st.has_flares() {
        return false;
    }
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
    st.diverted_missiles = st.diverted_missiles.saturating_add(1);
    if st.active == 0 && !st.reaction_armed {
        st.reaction_frame = frame.saturating_add(REACTION_LAUNCH_LATENCY_MS);
        st.reaction_armed = true;
    }
    reg.total_diverts = reg.total_diverts.saturating_add(1);
    let _ = projectile_id;
    true
}

/// Back-compat name for C++ report (no flare consume).
pub fn try_divert_missile(
    reg: &mut HostCountermeasuresRegistry,
    aircraft_id: ObjectId,
    projectile_id: ObjectId,
    frame: u32,
    has_upgrade: bool,
) -> bool {
    report_missile_for_countermeasures(reg, aircraft_id, projectile_id, frame, has_upgrade)
}

/// C++ `CountermeasuresBehavior::launchVolley` — queue `VolleySize` flare spawns.
fn launch_volley(reg: &mut HostCountermeasuresRegistry, aircraft_id: ObjectId, frame: u32) {
    let st = reg.ensure(aircraft_id);
    let flares = st.available.min(VOLLEY_SIZE);
    if flares == 0 {
        return;
    }
    st.available = st.available.saturating_sub(flares);
    st.active = st.active.saturating_add(flares);
    st.volleys_fired = st.volleys_fired.saturating_add(1);
    for vi in 0..flares {
        reg.pending_flare_spawns
            .push(PendingCountermeasureFlareSpawn {
                aircraft_id,
                frame,
                volley_index: vi,
            });
    }
}

/// C++ `CountermeasuresBehavior::update` volley timers (airborne only).
pub fn update_countermeasures(
    reg: &mut HostCountermeasuresRegistry,
    aircraft_id: ObjectId,
    now: u32,
    airborne: bool,
) {
    let (available, reaction, reaction_armed, next) = {
        let Some(st) = reg.states.get(&aircraft_id.0) else {
            return;
        };
        (
            st.available,
            st.reaction_frame,
            st.reaction_armed,
            st.next_volley_frame,
        )
    };
    if !airborne || available == 0 {
        return;
    }
    if reaction_armed && reaction == now {
        launch_volley(reg, aircraft_id, now);
        if let Some(st) = reg.states.get_mut(&aircraft_id.0) {
            st.next_volley_frame = now.saturating_add(DELAY_BETWEEN_VOLLEYS_FRAMES);
            st.reaction_frame = 0;
            st.reaction_armed = false;
        }
    }
    let next = reg
        .states
        .get(&aircraft_id.0)
        .map(|s| s.next_volley_frame)
        .unwrap_or(next);
    if next != 0 && next == now {
        launch_volley(reg, aircraft_id, now);
        if let Some(st) = reg.states.get_mut(&aircraft_id.0) {
            st.next_volley_frame = now.saturating_add(DELAY_BETWEEN_VOLLEYS_FRAMES);
        }
    }
}

/// C++ `launchVolley` facing-rotated motive (host XZ = C++ XY).
/// `ratio = i/(size-1)*2-1`, `angle = ratio * 90°`, scale by
/// `(vel < 1 ? -10 : vel) * VolleyVelocityFactor`.
pub fn flare_volley_motive_force(
    facing_xz: (f32, f32),
    volley_index: u32,
    volley_size: u32,
    speed: f32,
) -> (f32, f32, f32) {
    let size = volley_size.max(1);
    let ratio = if size > 1 {
        (volley_index as f32) / ((size - 1) as f32) * 2.0 - 1.0
    } else {
        0.0
    };
    let angle = ratio * VOLLEY_ARC_ANGLE_DEG.to_radians();
    let (sin_a, cos_a) = angle.sin_cos();
    let (dx, dz) = facing_xz;
    let len = (dx * dx + dz * dz).sqrt();
    let (dx, dz) = if len > 1.0e-6 {
        (dx / len, dz / len)
    } else {
        (1.0, 0.0)
    };
    let rx = dx * cos_a - dz * sin_a;
    let rz = dx * sin_a + dz * cos_a;
    let velocity = if speed < 1.0 { -10.0 } else { speed };
    let scale = velocity * VOLLEY_VELOCITY_FACTOR;
    (rx * scale, 0.0, rz * scale)
}

/// C++ `calculateCountermeasureToDivertTo`: closest 2D of the newest volley.
pub fn calculate_countermeasure_to_divert_to(
    reg: &HostCountermeasuresRegistry,
    aircraft_id: ObjectId,
    aircraft_xz: (f32, f32),
    flare_xz: &[(ObjectId, f32, f32)],
) -> Option<ObjectId> {
    let st = reg.get(aircraft_id)?;
    let max_check = VOLLEY_SIZE.max(1) as usize;
    let newest: Vec<ObjectId> = st.flare_ids.iter().rev().copied().take(max_check).collect();
    let mut best_id = None;
    let mut best_d2 = f32::INFINITY;
    for (fid, fx, fz) in flare_xz {
        if !newest.iter().any(|id| id == fid) {
            continue;
        }
        let dx = fx - aircraft_xz.0;
        let dz = fz - aircraft_xz.1;
        let d2 = dx * dx + dz * dz;
        if d2 < best_d2 {
            best_d2 = d2;
            best_id = Some(*fid);
        }
    }
    best_id
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

    #[test]
    fn report_does_not_consume_flares() {
        let mut reg = HostCountermeasuresRegistry::new();
        let air = ObjectId(1);
        let mut diverted = 0u32;
        for f in 0..80u32 {
            if report_missile_for_countermeasures(&mut reg, air, ObjectId(200 + f), f, true) {
                diverted += 1;
            }
        }
        assert!(diverted > 0);
        let st = reg.get(air).unwrap();
        assert_eq!(st.available, FULL_LOAD_COUNTERMEASURES);
        assert_eq!(st.active, 0);
        assert_eq!(st.volleys_fired, 0);
        assert!(st.reaction_armed);
        assert!(reg.take_pending_flare_spawns().is_empty());
    }

    #[test]
    fn airborne_update_launches_timed_volleys() {
        let mut reg = HostCountermeasuresRegistry::new();
        let air = ObjectId(3);
        // Force a diverted report at frame 10.
        let mut frame = 10u32;
        let mut ok = false;
        for i in 0..64u32 {
            if report_missile_for_countermeasures(&mut reg, air, ObjectId(300 + i), frame, true) {
                ok = true;
                break;
            }
            frame += 1;
        }
        assert!(ok);
        let reaction = reg.get(air).unwrap().reaction_frame;
        assert_eq!(reaction, frame);
        // Extra evaded missiles must not dump extra volleys.
        let _ = report_missile_for_countermeasures(&mut reg, air, ObjectId(400), frame, true);
        update_countermeasures(&mut reg, air, reaction, false);
        assert!(reg.take_pending_flare_spawns().is_empty());
        update_countermeasures(&mut reg, air, reaction, true);
        let first = reg.take_pending_flare_spawns();
        assert_eq!(first.len(), VOLLEY_SIZE as usize);
        let st = reg.get(air).unwrap();
        assert_eq!(st.available, FULL_LOAD_COUNTERMEASURES - VOLLEY_SIZE);
        assert_eq!(st.volleys_fired, 1);
        assert_eq!(st.reaction_frame, 0);
        assert_eq!(
            st.next_volley_frame,
            reaction + DELAY_BETWEEN_VOLLEYS_FRAMES
        );
        let next_volley_frame = st.next_volley_frame;
        update_countermeasures(&mut reg, air, next_volley_frame, true);
        let second = reg.take_pending_flare_spawns();
        assert_eq!(second.len(), VOLLEY_SIZE as usize);
        assert_eq!(
            reg.get(air).unwrap().available,
            FULL_LOAD_COUNTERMEASURES - 2 * VOLLEY_SIZE
        );
    }

    #[test]
    fn comanche_and_aurora_use_three_volleys() {
        assert_eq!(number_of_volleys_for_template("AmericaJetRaptor"), 5);
        assert_eq!(number_of_volleys_for_template("AmericaVehicleComanche"), 3);
        assert_eq!(number_of_volleys_for_template("AmericaJetAurora"), 3);
        let mut reg = HostCountermeasuresRegistry::new();
        let air = ObjectId(8);
        let st = reg.ensure_for_template(air, "AmericaVehicleComanche");
        assert_eq!(st.available, 12);
        assert_eq!(st.number_of_volleys, 3);
    }

    #[test]
    fn flare_motive_is_ninety_degree_fan() {
        let (x0, y0, z0) = flare_volley_motive_force((1.0, 0.0), 0, 4, 20.0);
        let (x1, _, z1) = flare_volley_motive_force((1.0, 0.0), 1, 4, 20.0);
        let (x3, _, z3) = flare_volley_motive_force((1.0, 0.0), 3, 4, 20.0);
        assert!((y0).abs() < 1e-5);
        // i=0 → -90° from +X → -Z at 40.
        assert!((x0).abs() < 1e-4);
        assert!((z0 + 40.0).abs() < 1e-3);
        // i=3 → +90° from +X → +Z at 40.
        assert!((x3).abs() < 1e-4);
        assert!((z3 - 40.0).abs() < 1e-3);
        // Hover uses -10 * 2 = -20 along facing (straight back).
        let (hx, _, hz) = flare_volley_motive_force((1.0, 0.0), 0, 1, 0.2);
        assert!((hx + 20.0).abs() < 1e-3);
        assert!(hz.abs() < 1e-4);
        let _ = (x1, z1);
    }

    #[test]
    fn divert_picks_closest_newest_volley_flare() {
        let mut reg = HostCountermeasuresRegistry::new();
        let air = ObjectId(1);
        {
            let st = reg.ensure(air);
            st.flare_ids = vec![
                ObjectId(10),
                ObjectId(11),
                ObjectId(12),
                ObjectId(13),
                ObjectId(20),
                ObjectId(21),
                ObjectId(22),
                ObjectId(23),
            ];
        }
        let flares = [
            (ObjectId(10), 0.0, 0.0),
            (ObjectId(20), 100.0, 0.0),
            (ObjectId(21), 1.0, 0.0),
            (ObjectId(22), 50.0, 0.0),
            (ObjectId(23), 8.0, 0.0),
        ];
        let id = calculate_countermeasure_to_divert_to(&reg, air, (0.0, 0.0), &flares);
        assert_eq!(id, Some(ObjectId(21)));
    }

    #[test]
    fn supersonic_gate_matches_set_token() {
        assert!(victim_locomotor_is_supersonic(Some("SET_SUPERSONIC")));
        assert!(!victim_locomotor_is_supersonic(Some("SET_NORMAL")));
        assert!(!victim_locomotor_is_supersonic(None));
    }
}
