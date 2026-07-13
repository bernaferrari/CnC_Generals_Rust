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
//! Fail-closed honesty:
//! - Not full OCL GPSScrambler_InvisibleMarker grow-from-StartRadius pulse scan
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

/// Retail GrantStealthBehavior StartRadius (grow path deferred; residual uses Final).
pub const GPS_SCRAMBLER_START_RADIUS: f32 = 20.0;

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound).
pub const GPS_SCRAMBLER_ACTIVATE_AUDIO: &str = "GPSScrambleActivate";

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
        assert!(is_legal_gps_scrambler_target(true, false, true, true, false, false));
        assert!(is_legal_gps_scrambler_target(false, true, true, true, false, false));
        assert!(!is_legal_gps_scrambler_target(false, false, true, true, false, false)); // structure
        assert!(!is_legal_gps_scrambler_target(true, false, false, true, false, false)); // dead
        assert!(!is_legal_gps_scrambler_target(true, false, true, false, false, false)); // enemy
        assert!(!is_legal_gps_scrambler_target(true, false, true, true, true, false)); // constructing
        assert!(!is_legal_gps_scrambler_target(true, false, true, true, false, true)); // bombtruck
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
}
