//! Host CommandCenter / RadarVan radar-online residual.
//!
//! Residual slice (playability):
//! - Owning an alive, constructed Command Center (not Fake*) grants radar to
//!   that player (retail America GrantUpgradeCreate+RadarUpgrade / China CC
//!   radar path residual; GLA CC name residual for playability).
//! - Owning an alive Radar Van grants radar (retail GLAVehicleRadarVan
//!   GrantUpgradeCreate Upgrade_GLARadar + RadarUpgrade DisableProof).
//! - Player radar state drives minimap / control-bar radar online (C++
//!   `Player::hasRadar()` residual).
//!
//! Wave 63 residual pack (retail INI honesty):
//! - Provider residual: CommandCenter (not Fake*) + RadarVan grant radar online.
//! - Radar Van body residual: MaxHealth **200**, Vision **200**, Shroud **500**,
//!   BuildCost **500**, BuildTime **10**s → **300**f, TransportSlotCount **3**.
//! - Grant residual: Upgrade_GLARadar + RadarUpgrade DisableProof **Yes**.
//! - Scan residual: SpecialPowerRadarVanScan Reload **30000**ms → **900**f,
//!   RadiusCursor **150**, Upgrade_GLARadarVanScan unpause gate.
//!
//! Fail-closed honesty:
//! - Not full RadarUpgrade / RadarUpdate extend-animation / grant-upgrade matrix
//! - Not full disable-proof vs power-brownout remove/addRadar on disable path
//! - Not full capture / sabotage / shared-allied radar edge cases
//! - Fake command centers residual-skip (`*Fake*CommandCenter*`)

use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const RADAR_LOGIC_FPS: f32 = 30.0;

/// Audio residual when radar comes online (MiscAudio RadarNotifyOnlineSound).
pub const RADAR_ONLINE_AUDIO: &str = "RadarOnline";

/// Audio residual when radar goes offline (MiscAudio RadarNotifyOfflineSound).
pub const RADAR_OFFLINE_AUDIO: &str = "RadarOffline";

/// Retail GrantUpgradeCreate / RadarUpgrade trigger residual.
pub const UPGRADE_GLA_RADAR: &str = "Upgrade_GLARadar";
/// Retail Radar Van Scan unlock residual.
pub const UPGRADE_GLA_RADAR_VAN_SCAN: &str = "Upgrade_GLARadarVanScan";
/// Retail SpecialPower residual name.
pub const SPECIAL_POWER_RADAR_VAN_SCAN: &str = "SpecialPowerRadarVanScan";
/// Retail OCL residual for scan.
pub const OCL_RADAR_VAN_SCAN: &str = "SUPERWEAPON_RadarVanScan";
/// Retail RadarUpgrade DisableProof residual.
pub const RADAR_VAN_DISABLE_PROOF: bool = true;

// --- Radar Van body residual (GLAVehicleRadarVan) ---

/// Retail MaxHealth residual.
pub const RADAR_VAN_MAX_HEALTH: f32 = 200.0;
/// Retail VisionRange residual.
pub const RADAR_VAN_VISION_RANGE: f32 = 200.0;
/// Retail ShroudClearingRange residual.
pub const RADAR_VAN_SHROUD_CLEARING_RANGE: f32 = 500.0;
/// Retail BuildCost residual.
pub const RADAR_VAN_BUILD_COST: u32 = 500;
/// Retail BuildTime residual (seconds).
pub const RADAR_VAN_BUILD_TIME_SEC: f32 = 10.0;
/// Retail BuildTime → frames @ 30 FPS.
pub const RADAR_VAN_BUILD_TIME_FRAMES: u32 = 300;
/// Retail TransportSlotCount residual.
pub const RADAR_VAN_TRANSPORT_SLOT_COUNT: u32 = 3;

// --- Scan special power residual ---

/// Retail SpecialPowerRadarVanScan ReloadTime residual (msec).
pub const RADAR_VAN_SCAN_RELOAD_MS: u32 = 30_000;
/// Retail ReloadTime → frames @ 30 FPS.
pub const RADAR_VAN_SCAN_RELOAD_FRAMES: u32 = 900;
/// Retail RadiusCursorRadius residual (aligns with RadarVanPing shroud).
pub const RADAR_VAN_SCAN_RADIUS_CURSOR: f32 = 150.0;
/// Retail InitiateAtLocationSound residual.
pub const RADAR_VAN_SCAN_AUDIO: &str = "RadarVanScan";

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn radar_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * RADAR_LOGIC_FPS / 1000.0).round() as u32
}

/// True when template is a residual radar-providing Command Center (not fake).
pub fn is_radar_command_center_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.contains("fake") {
        return false;
    }
    n.contains("commandcenter") || n.contains("headquarters")
}

/// True when template is a residual Radar Van provider.
pub fn is_radar_van_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("radarvan") || n.contains("radar_van") || n == "testradarvan"
}

/// True when template name is a residual radar provider (CC or RadarVan).
pub fn is_radar_provider_template(name: &str) -> bool {
    is_radar_command_center_template(name) || is_radar_van_template(name)
}

/// Whether a residual object can grant radar this frame.
///
/// Matches C++ RadarUpgrade gates (subset):
/// alive, construction complete (GrantUpgradeCreate ExemptStatus=UNDER_CONSTRUCTION),
/// not fake provider.
pub fn is_legal_radar_provider(
    is_alive: bool,
    is_constructed: bool,
    is_command_center_kind: bool,
    template_name: &str,
) -> bool {
    if !is_alive || !is_constructed {
        return false;
    }
    if is_radar_van_template(template_name) {
        return true;
    }
    if is_command_center_kind || is_radar_command_center_template(template_name) {
        // Fake CC residual-skip.
        return is_radar_command_center_template(template_name);
    }
    false
}

/// Host residual honesty + radar-online bookkeeping.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostRadarRegistry {
    /// Times a player transitioned from no-radar → has-radar.
    pub online_transitions: u32,
    /// Times a player transitioned from has-radar → no-radar.
    pub offline_transitions: u32,
    /// Peak concurrent radar-provider count observed on any player.
    pub max_provider_count: u32,
    /// True once any player was observed with has_radar after a residual update.
    pub any_player_online: bool,
}

impl HostRadarRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Record a recompute for one player. Returns `(came_online, went_offline)`.
    pub fn record_player_radar(
        &mut self,
        provider_count: u32,
        had_radar: bool,
        has_radar_now: bool,
    ) -> (bool, bool) {
        if provider_count > self.max_provider_count {
            self.max_provider_count = provider_count;
        }
        if has_radar_now {
            self.any_player_online = true;
        }
        let came_online = !had_radar && has_radar_now;
        let went_offline = had_radar && !has_radar_now;
        if came_online {
            self.online_transitions = self.online_transitions.saturating_add(1);
        }
        if went_offline {
            self.offline_transitions = self.offline_transitions.saturating_add(1);
        }
        (came_online, went_offline)
    }

    /// Residual honesty: at least one player radar came online via residual path.
    pub fn honesty_online_ok(&self) -> bool {
        self.online_transitions > 0 && self.any_player_online
    }

    /// Residual honesty: provider count was observed positive.
    pub fn honesty_provider_ok(&self) -> bool {
        self.max_provider_count > 0
    }

    /// Combined residual honesty (provider + online transition).
    pub fn honesty_ok(&self) -> bool {
        self.honesty_provider_ok() && self.honesty_online_ok()
    }
}

// --- Wave 63 residual honesty packs ---

/// Wave 63 residual honesty: radar provider + audio residual peel.
pub fn honesty_radar_provider_residual_ok() -> bool {
    RADAR_ONLINE_AUDIO == "RadarOnline"
        && RADAR_OFFLINE_AUDIO == "RadarOffline"
        && is_radar_command_center_template("AmericaCommandCenter")
        && is_radar_command_center_template("ChinaCommandCenter")
        && is_radar_command_center_template("GLA_CommandCenter")
        && !is_radar_command_center_template("FakeGLACommandCenter")
        && is_radar_van_template("GLAVehicleRadarVan")
        && is_radar_provider_template("USA_CommandCenter")
        && is_legal_radar_provider(true, true, true, "USA_CommandCenter")
        && !is_legal_radar_provider(true, false, true, "USA_CommandCenter")
        && is_legal_radar_provider(true, true, false, "GLAVehicleRadarVan")
}

/// Wave 63 residual honesty: Radar Van body residual peel.
pub fn honesty_radar_van_body_residual_ok() -> bool {
    (RADAR_VAN_MAX_HEALTH - 200.0).abs() < 0.01
        && (RADAR_VAN_VISION_RANGE - 200.0).abs() < 0.01
        && (RADAR_VAN_SHROUD_CLEARING_RANGE - 500.0).abs() < 0.01
        && RADAR_VAN_BUILD_COST == 500
        && (RADAR_VAN_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && RADAR_VAN_BUILD_TIME_FRAMES
            == ((RADAR_VAN_BUILD_TIME_SEC * RADAR_LOGIC_FPS).round() as u32)
        && RADAR_VAN_BUILD_TIME_FRAMES == 300
        && RADAR_VAN_TRANSPORT_SLOT_COUNT == 3
        && UPGRADE_GLA_RADAR == "Upgrade_GLARadar"
        && RADAR_VAN_DISABLE_PROOF
}

/// Wave 63 residual honesty: Radar Van Scan special-power residual peel.
pub fn honesty_radar_van_scan_residual_ok() -> bool {
    SPECIAL_POWER_RADAR_VAN_SCAN == "SpecialPowerRadarVanScan"
        && UPGRADE_GLA_RADAR_VAN_SCAN == "Upgrade_GLARadarVanScan"
        && OCL_RADAR_VAN_SCAN == "SUPERWEAPON_RadarVanScan"
        && RADAR_VAN_SCAN_RELOAD_MS == 30_000
        && RADAR_VAN_SCAN_RELOAD_FRAMES == radar_ms_to_frames(RADAR_VAN_SCAN_RELOAD_MS)
        && RADAR_VAN_SCAN_RELOAD_FRAMES == 900
        && (RADAR_VAN_SCAN_RADIUS_CURSOR - 150.0).abs() < 0.01
        && RADAR_VAN_SCAN_AUDIO == "RadarVanScan"
}

/// Combined Wave 63 radar residual honesty pack.
pub fn honesty_radar_residual_pack_ok() -> bool {
    honesty_radar_provider_residual_ok()
        && honesty_radar_van_body_residual_ok()
        && honesty_radar_van_scan_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_center_and_radar_van_templates_detected() {
        assert!(is_radar_command_center_template("USA_CommandCenter"));
        assert!(is_radar_command_center_template("AmericaCommandCenter"));
        assert!(is_radar_command_center_template("ChinaCommandCenter"));
        assert!(is_radar_command_center_template("GLA_CommandCenter"));
        assert!(!is_radar_command_center_template("FakeGLACommandCenter"));
        assert!(is_radar_van_template("GLAVehicleRadarVan"));
        assert!(is_radar_van_template("TestRadarVan"));
        assert!(is_radar_provider_template("TestCommandCenter"));
        assert!(!is_radar_provider_template("TestBarracks"));
    }

    #[test]
    fn legal_provider_requires_alive_constructed() {
        assert!(is_legal_radar_provider(
            true,
            true,
            true,
            "USA_CommandCenter"
        ));
        assert!(!is_legal_radar_provider(
            false,
            true,
            true,
            "USA_CommandCenter"
        ));
        assert!(!is_legal_radar_provider(
            true,
            false,
            true,
            "USA_CommandCenter"
        ));
        assert!(!is_legal_radar_provider(
            true,
            true,
            false,
            "FakeGLACommandCenter"
        ));
        assert!(is_legal_radar_provider(
            true,
            true,
            false,
            "GLAVehicleRadarVan"
        ));
    }

    #[test]
    fn registry_records_online_transition() {
        let mut reg = HostRadarRegistry::new();
        assert!(!reg.honesty_ok());
        let (on, off) = reg.record_player_radar(1, false, true);
        assert!(on);
        assert!(!off);
        assert!(reg.honesty_online_ok());
        assert!(reg.honesty_provider_ok());
        assert!(reg.honesty_ok());
        let (on2, off2) = reg.record_player_radar(0, true, false);
        assert!(!on2);
        assert!(off2);
        assert_eq!(reg.offline_transitions, 1);
    }

    #[test]
    fn radar_residual_pack_honesty_wave63() {
        assert!(honesty_radar_provider_residual_ok());
        assert!(honesty_radar_van_body_residual_ok());
        assert!(honesty_radar_van_scan_residual_ok());
        assert!(honesty_radar_residual_pack_ok());
        assert_eq!(radar_ms_to_frames(30_000), 900);
        assert_eq!(radar_ms_to_frames(0), 0);
        assert_eq!(RADAR_VAN_BUILD_TIME_FRAMES, 300);
        assert!(RADAR_VAN_DISABLE_PROOF);
        assert_eq!(SPECIAL_POWER_RADAR_VAN_SCAN, "SpecialPowerRadarVanScan");
    }
}
