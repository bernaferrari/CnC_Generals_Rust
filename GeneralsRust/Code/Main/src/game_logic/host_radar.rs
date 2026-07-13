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
//! Fail-closed honesty:
//! - Not full RadarUpgrade / RadarUpdate extend-animation / grant-upgrade matrix
//! - Not full disable-proof vs power-brownout remove/addRadar on disable path
//! - Not full capture / sabotage / shared-allied radar edge cases
//! - Fake command centers residual-skip (`*Fake*CommandCenter*`)

use serde::{Deserialize, Serialize};

/// Audio residual when radar comes online (MiscAudio RadarNotifyOnlineSound).
pub const RADAR_ONLINE_AUDIO: &str = "RadarOnline";

/// Audio residual when radar goes offline (MiscAudio RadarNotifyOfflineSound).
pub const RADAR_OFFLINE_AUDIO: &str = "RadarOffline";

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
}
