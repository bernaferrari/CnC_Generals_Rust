//! Host GLA GPS Scrambler special-power residual — ally stealth grant in radius.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(GpsScrambler)` at a world location grants STEALTHED status to
//!   same-team **VEHICLE|INFANTRY** in radius (retail SuperweaponGPSScrambler →
//!   SUPERWEAPON_GPSScrambler → GPSScrambler_InvisibleMarker GrantStealthBehavior
//!   receiveGrant() → OBJECT_STATUS_CAN_STEALTH + OBJECT_STATUS_STEALTHED).
//! - FinalRadius residual 100 (RadiusCursorRadius / GrantStealth FinalRadius).
//! - Stealthed-and-undetected units are not enemy-targetable / not visible to enemies
//!   (existing host stealth gates). Attack still breaks stealth (STEALTH_NOT_WHILE_ATTACKING).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Wave 54 residual pack (retail INI honesty):
//! - GrantStealthBehavior grow-radius pulse: StartRadius **20**, FinalRadius **100**,
//!   RadiusGrowRate **10** / frame → **8** grow updates to final
//! - KindOf VEHICLE | INFANTRY
//! - SuperweaponGPSScrambler ReloadTime **240000**ms → **7200**f,
//!   RadiusCursorRadius **100**, RequiredScience SCIENCE_GPSScrambler
//! - Slth_SuperweaponGPSScrambler ReloadTime **180000**ms → **5400**f
//! - OCL SUPERWEAPON_GPSScrambler → GPSScrambler_InvisibleMarker
//!
//! Fail-closed honesty:
//! - Not full OCL GPSScrambler_InvisibleMarker particle (GPSMicrowaveScambler /
//!   GPSRotisserie / gpsScrambleCloud) GPU path
//! - Not full StealthUpdate module matrix (only units with getStealth in C++; host residual
//!   grants to VEHICLE|INFANTRY KindOf, skips bomb-truck disguise residual by name)
//! - Not full ally relationship filter (uses same-team residual)
//! - Not full particle / flashAsSelected drawable path
//! - Not network GPS Scrambler replication (network deferred)
//!
//! Note: Older module comments claiming "disables enemy radar" are incorrect for ZH
//! retail — GPS Scrambler is GrantStealth on allies, not radar jam.

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const GPS_SCRAMBLER_LOGIC_FPS: f32 = 30.0;

/// Retail SuperweaponGPSScrambler RadiusCursorRadius / GrantStealth FinalRadius.
pub const HOST_GPS_SCRAMBLER_RADIUS: f32 = 100.0;
/// Alias for RadiusCursorRadius residual.
pub const GPS_SCRAMBLER_RADIUS_CURSOR: f32 = 100.0;

/// Retail GrantStealthBehavior StartRadius (grow path residual).
pub const GPS_SCRAMBLER_START_RADIUS: f32 = 20.0;
/// Retail GrantStealthBehavior FinalRadius residual.
pub const GPS_SCRAMBLER_FINAL_RADIUS: f32 = 100.0;
/// Retail GrantStealthBehavior RadiusGrowRate residual (distance per logic frame).
pub const GPS_SCRAMBLER_RADIUS_GROW_RATE: f32 = 10.0;
/// Grow updates to reach final: (100 - 20) / 10 = 8.
/// C++ first update: start(20) + rate → 30 … final update clamps to 100.
pub const GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL: u32 = 8;

/// Retail SuperweaponGPSScrambler ReloadTime residual (msec).
pub const GPS_SCRAMBLER_RELOAD_MS: u32 = 240_000;
/// ReloadTime 240000ms → 7200 frames @ 30 FPS.
pub const GPS_SCRAMBLER_RELOAD_FRAMES: u32 = 7_200;
/// Retail Slth_SuperweaponGPSScrambler ReloadTime residual (msec).
pub const GPS_SCRAMBLER_SLTH_RELOAD_MS: u32 = 180_000;
/// Slth reload 180000ms → 5400 frames @ 30 FPS.
pub const GPS_SCRAMBLER_SLTH_RELOAD_FRAMES: u32 = 5_400;

/// Retail special power / OCL / marker names.
pub const SUPERWEAPON_GPS_SCRAMBLER: &str = "SuperweaponGPSScrambler";
pub const SUPERWEAPON_GPS_SCRAMBLER_OCL: &str = "SUPERWEAPON_GPSScrambler";
pub const GPS_SCRAMBLER_INVISIBLE_MARKER: &str = "GPSScrambler_InvisibleMarker";
pub const SCIENCE_GPS_SCRAMBLER: &str = "SCIENCE_GPSScrambler";
pub const SLTH_SUPERWEAPON_GPS_SCRAMBLER: &str = "Slth_SuperweaponGPSScrambler";
pub const SLTH_SCIENCE_GPS_SCRAMBLER: &str = "Slth_SCIENCE_GPSScrambler";

/// Retail RadiusParticleSystemName residual.
pub const GPS_SCRAMBLER_RADIUS_PARTICLE: &str = "ParticleUplinkCannon_LaserBaseReadyToFire";

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound).
pub const GPS_SCRAMBLER_ACTIVATE_AUDIO: &str = "GPSScrambleActivate";

// --- Wave 78: GPS Scrambler science / marker / particle residual deepen ---
/// Retail SCIENCE_GPSScrambler SciencePurchasePointCost residual.
pub const GPS_SCRAMBLER_SCIENCE_POINT_COST: u32 = 1;
/// Retail SCIENCE_GPSScrambler PrerequisiteSciences residual tokens.
pub const GPS_SCRAMBLER_PREREQ_SCIENCES: [&str; 2] = ["SCIENCE_GLA", "SCIENCE_Rank5"];
/// Retail Slth_SCIENCE_GPSScrambler PrerequisiteSciences residual tokens (Rank3).
pub const GPS_SCRAMBLER_SLTH_PREREQ_SCIENCES: [&str; 2] = ["SCIENCE_GLA", "SCIENCE_Rank3"];
/// Retail GrantStealthBehavior KindOf residual tokens.
pub const GPS_SCRAMBLER_GRANT_KIND_OF: [&str; 2] = ["VEHICLE", "INFANTRY"];
/// Retail GPSScrambler_InvisibleMarker KindOf residual substring honesty.
pub const GPS_SCRAMBLER_MARKER_KIND_OF: &str = "NO_COLLIDE IMMOBILE UNATTACKABLE";
/// Retail ImmortalBody MaxHealth residual on invisible marker.
pub const GPS_SCRAMBLER_MARKER_MAX_HEALTH: f32 = 1.0;
/// Retail W3DModelDraw particle residual names on GPSScrambler_InvisibleMarker.
pub const GPS_SCRAMBLER_PARTICLE_MICROWAVE: &str = "GPSMicrowaveScambler";
/// Retail GPSRotisserie particle residual.
pub const GPS_SCRAMBLER_PARTICLE_ROTISSERIE: &str = "GPSRotisserie";
/// Retail gpsScrambleCloud particle residual.
pub const GPS_SCRAMBLER_PARTICLE_CLOUD: &str = "gpsScrambleCloud";
/// Retail Enum residual for SuperweaponGPSScrambler.
pub const GPS_SCRAMBLER_ENUM: &str = "SPECIAL_GPS_SCRAMBLER";
/// Retail Enum residual for Slth_SuperweaponGPSScrambler.
pub const GPS_SCRAMBLER_SLTH_ENUM: &str = "SLTH_SPECIAL_GPS_SCRAMBLER";

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn gps_scrambler_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / GPS_SCRAMBLER_LOGIC_FPS)).round() as u32
}

/// Grow-radius pulse residual: scan radius after `update_index` GrantStealth updates.
///
/// C++ GrantStealthBehavior::update: m_currentScanRadius starts at StartRadius,
/// each update does `+= RadiusGrowRate` then clamps to FinalRadius.
/// `update_index` is 0-based (first update = 0 → Start + Rate).
pub fn gps_scrambler_scan_radius_after_updates(update_index: u32) -> f32 {
    let r =
        GPS_SCRAMBLER_START_RADIUS + (update_index as f32 + 1.0) * GPS_SCRAMBLER_RADIUS_GROW_RATE;
    r.min(GPS_SCRAMBLER_FINAL_RADIUS)
}

/// Whether residual grow pulse has reached FinalRadius after `update_index` updates.
pub fn gps_scrambler_grow_is_final(update_index: u32) -> bool {
    gps_scrambler_scan_radius_after_updates(update_index) >= GPS_SCRAMBLER_FINAL_RADIUS - 0.001
}

/// Whether residual target can receive GPS Scrambler stealth grant.
///
/// Retail GrantStealthBehavior + receiveGrant:
/// - allies (host residual: same-team)
/// - alive
/// - KindOf VEHICLE | INFANTRY
/// - not under construction residual
/// - not bomb-truck disguise residual (StealthUpdate::canDisguise skip)
pub fn is_legal_gps_scrambler_target(
    is_vehicle: bool,
    is_infantry: bool,
    is_alive: bool,
    same_team: bool,
    under_construction: bool,
    is_disguise_unit: bool,
) -> bool {
    if !is_alive || under_construction || is_disguise_unit {
        return false;
    }
    if !same_team {
        return false;
    }
    is_vehicle || is_infantry
}

/// Name residual for bomb-truck / disguise units that C++ receiveGrant skips.
pub fn is_gps_scrambler_disguise_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("bombtruck") || n.contains("disguise") || n.contains("hijacker")
}

/// 2D distance check residual (C++ FROM_CENTER_2D / FinalRadius).
pub fn in_gps_scrambler_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// Wave 54 residual honesty: grow-radius pulse residual.
pub fn honesty_gps_scrambler_grow_radius_residual_ok() -> bool {
    (GPS_SCRAMBLER_START_RADIUS - 20.0).abs() < 0.01
        && (GPS_SCRAMBLER_FINAL_RADIUS - 100.0).abs() < 0.01
        && (HOST_GPS_SCRAMBLER_RADIUS - 100.0).abs() < 0.01
        && (GPS_SCRAMBLER_RADIUS_CURSOR - 100.0).abs() < 0.01
        && (GPS_SCRAMBLER_RADIUS_GROW_RATE - 10.0).abs() < 0.01
        && GPS_SCRAMBLER_GROW_UPDATES_TO_FINAL == 8
        && (gps_scrambler_scan_radius_after_updates(0) - 30.0).abs() < 0.01
        && (gps_scrambler_scan_radius_after_updates(7) - 100.0).abs() < 0.01
        && gps_scrambler_grow_is_final(7)
        && !gps_scrambler_grow_is_final(6)
        && GPS_SCRAMBLER_RADIUS_PARTICLE == "ParticleUplinkCannon_LaserBaseReadyToFire"
}

/// Wave 54 residual honesty: reload / OCL / science residual.
pub fn honesty_gps_scrambler_reload_ocl_residual_ok() -> bool {
    GPS_SCRAMBLER_RELOAD_MS == 240_000
        && GPS_SCRAMBLER_RELOAD_FRAMES == gps_scrambler_ms_to_frames(GPS_SCRAMBLER_RELOAD_MS)
        && GPS_SCRAMBLER_SLTH_RELOAD_MS == 180_000
        && GPS_SCRAMBLER_SLTH_RELOAD_FRAMES
            == gps_scrambler_ms_to_frames(GPS_SCRAMBLER_SLTH_RELOAD_MS)
        && SUPERWEAPON_GPS_SCRAMBLER == "SuperweaponGPSScrambler"
        && SUPERWEAPON_GPS_SCRAMBLER_OCL == "SUPERWEAPON_GPSScrambler"
        && GPS_SCRAMBLER_INVISIBLE_MARKER == "GPSScrambler_InvisibleMarker"
        && SCIENCE_GPS_SCRAMBLER == "SCIENCE_GPSScrambler"
        && SLTH_SUPERWEAPON_GPS_SCRAMBLER == "Slth_SuperweaponGPSScrambler"
        && SLTH_SCIENCE_GPS_SCRAMBLER == "Slth_SCIENCE_GPSScrambler"
        && GPS_SCRAMBLER_ACTIVATE_AUDIO == "GPSScrambleActivate"
}

/// Combined Wave 54 GPS Scrambler residual honesty pack.
pub fn honesty_gps_scrambler_residual_pack_ok() -> bool {
    honesty_gps_scrambler_grow_radius_residual_ok()
        && honesty_gps_scrambler_reload_ocl_residual_ok()
}

/// Wave 78 residual honesty: GPS science / marker particle / KindOf residual deepen.
///
/// Fail-closed: not full particle GPU path / full StealthUpdate module matrix.
pub fn honesty_gps_scrambler_residual_pack_wave78() -> bool {
    GPS_SCRAMBLER_SCIENCE_POINT_COST == 1
        && GPS_SCRAMBLER_PREREQ_SCIENCES == ["SCIENCE_GLA", "SCIENCE_Rank5"]
        && GPS_SCRAMBLER_SLTH_PREREQ_SCIENCES == ["SCIENCE_GLA", "SCIENCE_Rank3"]
        && GPS_SCRAMBLER_GRANT_KIND_OF == ["VEHICLE", "INFANTRY"]
        && GPS_SCRAMBLER_MARKER_KIND_OF.contains("NO_COLLIDE")
        && GPS_SCRAMBLER_MARKER_KIND_OF.contains("IMMOBILE")
        && GPS_SCRAMBLER_MARKER_KIND_OF.contains("UNATTACKABLE")
        && (GPS_SCRAMBLER_MARKER_MAX_HEALTH - 1.0).abs() < 0.01
        && GPS_SCRAMBLER_PARTICLE_MICROWAVE == "GPSMicrowaveScambler"
        && GPS_SCRAMBLER_PARTICLE_ROTISSERIE == "GPSRotisserie"
        && GPS_SCRAMBLER_PARTICLE_CLOUD == "gpsScrambleCloud"
        && GPS_SCRAMBLER_ENUM == "SPECIAL_GPS_SCRAMBLER"
        && GPS_SCRAMBLER_SLTH_ENUM == "SLTH_SPECIAL_GPS_SCRAMBLER"
        // Slth is faster reload and lower rank prereq residual.
        && GPS_SCRAMBLER_SLTH_RELOAD_MS < GPS_SCRAMBLER_RELOAD_MS
        && GPS_SCRAMBLER_SLTH_PREREQ_SCIENCES[1] == "SCIENCE_Rank3"
        && GPS_SCRAMBLER_PREREQ_SCIENCES[1] == "SCIENCE_Rank5"
        && honesty_gps_scrambler_residual_pack_ok()
}

/// One active residual GPS Scrambler activation bookkeeping entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostGpsScrambler {
    pub id: u32,
    pub player_id: u32,
    pub location: Vec3,
    pub radius: f32,
    pub activate_frame: u32,
    pub caster_id: Option<ObjectId>,
    /// Ally units that received STEALTHED residual this activation.
    pub grants: u32,
}

/// Host residual registry for GPS Scrambler special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostGpsScramblerRegistry {
    next_id: u32,
    /// Recent activations (bookkeeping).
    activations: Vec<HostGpsScrambler>,
    /// Total activations (honesty).
    pub activation_count: u32,
    /// Total stealth grants applied.
    pub grant_count: u32,
}

impl HostGpsScramblerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }

    pub fn grant_count(&self) -> u32 {
        self.grant_count
    }

    pub fn activations(&self) -> &[HostGpsScrambler] {
        &self.activations
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a successful residual GPS Scrambler activation.
    pub fn record_activation(&mut self, entry: HostGpsScrambler) {
        self.activation_count = self.activation_count.saturating_add(1);
        self.grant_count = self.grant_count.saturating_add(entry.grants);
        self.activations.push(entry);
        // Keep bookkeeping bounded (residual, not full history Xfer).
        if self.activations.len() > 32 {
            let drain = self.activations.len() - 32;
            self.activations.drain(0..drain);
        }
    }

    /// Residual honesty: at least one GPS Scrambler activated.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    /// Residual honesty: at least one unit received stealth grant.
    pub fn honesty_grant_ok(&self) -> bool {
        self.grant_count > 0
    }

    /// Combined host path: activated and granted stealth at least once.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_grant_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gps_scrambler_constants_match_retail_residual() {
        assert!((HOST_GPS_SCRAMBLER_RADIUS - 100.0).abs() < 0.01);
        assert!((GPS_SCRAMBLER_START_RADIUS - 20.0).abs() < 0.01);
        assert!(!GPS_SCRAMBLER_ACTIVATE_AUDIO.is_empty());
    }

    #[test]
    fn legal_gps_scrambler_target_matrix() {
        // vehicle, infantry, alive, same_team, under_construction, disguise
        assert!(is_legal_gps_scrambler_target(
            true, false, true, true, false, false
        ));
        assert!(is_legal_gps_scrambler_target(
            false, true, true, true, false, false
        ));
        assert!(!is_legal_gps_scrambler_target(
            false, false, true, true, false, false
        )); // structure
        assert!(!is_legal_gps_scrambler_target(
            true, false, false, true, false, false
        )); // dead
        assert!(!is_legal_gps_scrambler_target(
            true, false, true, false, false, false
        )); // enemy
        assert!(!is_legal_gps_scrambler_target(
            true, false, true, true, true, false
        )); // constructing
        assert!(!is_legal_gps_scrambler_target(
            true, false, true, true, false, true
        )); // bombtruck
    }

    #[test]
    fn disguise_name_residual() {
        assert!(is_gps_scrambler_disguise_name("GLAVehicleBombTruck"));
        assert!(is_gps_scrambler_disguise_name("Demo_GLAVehicleBombTruck"));
        assert!(!is_gps_scrambler_disguise_name("GLAVehicleQuadCannon"));
        assert!(!is_gps_scrambler_disguise_name("USA_Ranger"));
    }

    #[test]
    fn gps_scrambler_radius_check() {
        assert!(in_gps_scrambler_radius_2d((0.0, 0.0), (50.0, 0.0), 100.0));
        assert!(!in_gps_scrambler_radius_2d((0.0, 0.0), (150.0, 0.0), 100.0));
    }

    #[test]
    fn honesty_registry_records_grants() {
        let mut reg = HostGpsScramblerRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostGpsScrambler {
            id,
            player_id: 2,
            location: Vec3::ZERO,
            radius: HOST_GPS_SCRAMBLER_RADIUS,
            activate_frame: 0,
            caster_id: None,
            grants: 3,
        });
        assert!(reg.honesty_activate_ok());
        assert!(reg.honesty_grant_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.activation_count(), 1);
        assert_eq!(reg.grant_count(), 3);
    }

    #[test]
    fn gps_scrambler_residual_pack_honesty() {
        assert!(honesty_gps_scrambler_residual_pack_ok());
        assert_eq!(gps_scrambler_ms_to_frames(240_000), 7_200);
        assert_eq!(gps_scrambler_ms_to_frames(180_000), 5_400);
        // Grow sequence honesty: 30,40,...,100 over 8 updates
        for i in 0..7 {
            assert!(!gps_scrambler_grow_is_final(i));
        }
        assert!(gps_scrambler_grow_is_final(7));
        assert!((gps_scrambler_scan_radius_after_updates(100) - 100.0).abs() < 0.01);
    }

    #[test]
    fn gps_scrambler_residual_pack_wave78_honesty() {
        assert!(honesty_gps_scrambler_residual_pack_wave78());
        assert_eq!(GPS_SCRAMBLER_SCIENCE_POINT_COST, 1);
        assert_eq!(
            GPS_SCRAMBLER_PREREQ_SCIENCES,
            ["SCIENCE_GLA", "SCIENCE_Rank5"]
        );
        assert_eq!(
            GPS_SCRAMBLER_SLTH_PREREQ_SCIENCES,
            ["SCIENCE_GLA", "SCIENCE_Rank3"]
        );
        assert_eq!(GPS_SCRAMBLER_PARTICLE_MICROWAVE, "GPSMicrowaveScambler");
        assert_eq!(GPS_SCRAMBLER_PARTICLE_CLOUD, "gpsScrambleCloud");
        assert_eq!(GPS_SCRAMBLER_ENUM, "SPECIAL_GPS_SCRAMBLER");
        assert!(GPS_SCRAMBLER_SLTH_RELOAD_MS < GPS_SCRAMBLER_RELOAD_MS);
    }
}
