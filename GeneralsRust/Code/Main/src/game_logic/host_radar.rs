//! Host CommandCenter / RadarVan radar-online residual.
//!
//! Residual slice (playability):
//! - Owning an alive, constructed America/China Command Center (not Fake*,
//!   not GLA) grants radar (retail America GrantUpgradeCreate+RadarUpgrade /
//!   China CC RadarUpgrade TriggeredBy Upgrade_ChinaRadar). GLACommandCenter
//!   has no RadarUpgrade — radar comes from the Radar Van only.
//! - Owning an alive Radar Van grants radar (retail GLAVehicleRadarVan
//!   GrantUpgradeCreate Upgrade_GLARadar + RadarUpgrade DisableProof).
//! - Player radar state drives minimap / control-bar radar online (C++
//!   `Player::hasRadar()` residual).
//!
//! Wave 63 residual pack (retail INI honesty):
//! - Provider residual: America/China CommandCenter (not Fake*, not GLA)
//!   + RadarVan grant radar online.
//! - Radar Van body residual: MaxHealth **200**, Vision **200**, Shroud **500**,
//!   BuildCost **500**, BuildTime **10**s → **300**f, TransportSlotCount **3**.
//! - Grant residual: Upgrade_GLARadar + RadarUpgrade DisableProof **Yes**.
//! - Scan residual: SpecialPowerRadarVanScan Reload **30000**ms → **900**f,
//!   RadiusCursor **150**, Upgrade_GLARadarVanScan unpause gate.
//!
//! Fail-closed honesty:
//! - Not full RadarUpgrade / RadarUpdate extend-animation / grant-upgrade matrix
//! - Disable-proof vs brownout: `Player::has_radar` honors `disable_proof_radar_count`
//! - leftover Object::onDisabledEdge: EMP/hacked/held applied RadarUpgrade removeRadar
//! - Not full capture / sabotage / shared-allied radar edge cases
//! - Fake / GLA command centers residual-skip (no RadarUpgrade on GLACommandCenter)

use game_engine::common::system::radar::{Coord3D, RadarEventType, RadarSystem};
use serde::{Deserialize, Serialize};

/// Leftover `Object::on_disabled_edge` radar add/remove pending apply.
#[derive(Debug, Clone, Copy)]
pub struct RadarDisabledEdge {
    pub player_id: Option<u32>,
    pub becoming_disabled: bool,
    pub disable_proof: bool,
    pub module_index: usize,
}

/// Immutable authored RadarUpgrade/UpgradeMux data, in behavior-module order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RadarUpgradeRules {
    pub triggered_by: Vec<String>,
    pub conflicts_with: Vec<String>,
    pub removes_upgrades: Vec<String>,
    pub fx_list: Option<String>,
    pub requires_all: bool,
    pub disable_proof: bool,
    pub valid: bool,
}

impl RadarUpgradeRules {
    pub fn from_definition(module: &crate::assets::ini_parser::BehaviorModuleDefinition) -> Self {
        fn names(value: Option<&str>) -> Vec<String> {
            value
                .unwrap_or_default()
                .split_whitespace()
                .filter(|name| !name.eq_ignore_ascii_case("NONE"))
                .map(str::to_owned)
                .collect()
        }
        fn boolean(value: Option<&str>) -> Option<bool> {
            match value.map(str::trim) {
                None => Some(false),
                Some(value) if value.eq_ignore_ascii_case("yes") => Some(true),
                Some(value) if value.eq_ignore_ascii_case("no") => Some(false),
                _ => None,
            }
        }
        let all = boolean(module.attribute("RequiresAllTriggers"));
        let proof = boolean(module.attribute("DisableProof"));
        Self {
            triggered_by: names(module.attribute("TriggeredBy")),
            conflicts_with: names(module.attribute("ConflictsWith")),
            removes_upgrades: names(module.attribute("RemovesUpgrades")),
            fx_list: module
                .attribute("FXListUpgrade")
                .map(str::trim)
                .filter(|name| !name.is_empty() && !name.eq_ignore_ascii_case("NONE"))
                .map(str::to_owned),
            requires_all: all.unwrap_or(false),
            disable_proof: proof.unwrap_or(false),
            valid: all.is_some() && proof.is_some(),
        }
    }

    pub fn would_upgrade(&self, mut has: impl FnMut(&str) -> bool) -> bool {
        self.valid
            && !self.triggered_by.is_empty()
            && !self.conflicts_with.iter().any(|name| has(name))
            && if self.requires_all {
                self.triggered_by.iter().all(|name| has(name))
            } else {
                self.triggered_by.iter().any(|name| has(name))
            }
    }
}

/// Logic frames per second (host fixed step).
pub const RADAR_LOGIC_FPS: f32 = 30.0;

/// Audio residual when radar comes online (MiscAudio RadarNotifyOnlineSound = RadarOn).
pub const RADAR_ONLINE_AUDIO: &str = "RadarOn";

/// Audio residual when radar goes offline (MiscAudio RadarNotifyOfflineSound = RadarOff).
pub const RADAR_OFFLINE_AUDIO: &str = "RadarOff";

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

/// C++ `Radar::createEvent` default live time (Radar.cpp color table / 4s).
pub const RADAR_EVENT_SECONDS_TO_LIVE: f32 = 4.0;

/// Host world (Y-up XZ) → leftover radar plane (Z-up XY).
pub fn host_world_to_radar_coord(pos: glam::Vec3) -> Coord3D {
    Coord3D::new(pos.x, pos.z, pos.y)
}

/// Leftover radar plane (Z-up XY) → host world (Y-up XZ).
pub fn radar_coord_to_host_world(loc: Coord3D) -> glam::Vec3 {
    glam::Vec3::new(loc.x, loc.z, loc.y)
}

pub fn pack_player_color_argb(rgb: (u8, u8, u8)) -> u32 {
    0xFF00_0000 | ((rgb.0 as u32) << 16) | ((rgb.1 as u32) << 8) | (rgb.2 as u32)
}

/// C++ `TheGlobalData->m_timeOfDay == TIME_OF_DAY_NIGHT`.
pub fn host_time_of_day_is_night() -> bool {
    use game_engine::common::ini::ini_game_data::{TimeOfDay, get_global_data};
    get_global_data()
        .map(|data| matches!(data.read().time_of_day, TimeOfDay::Night))
        .unwrap_or(false)
}

/// Host residual honesty + radar-online bookkeeping.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostRadarRegistry {
    #[serde(skip)]
    pub(crate) providers: std::collections::HashMap<super::ObjectId, Vec<Vec<(u32, bool)>>>,
    #[serde(skip)]
    pub(crate) client_quiet: bool,
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
    fn radar_event_helpers_match_cpp_defaults() {
        assert!((RADAR_EVENT_SECONDS_TO_LIVE - 4.0).abs() < f32::EPSILON);
        assert_eq!(pack_player_color_argb((0x12, 0x34, 0x56)), 0xFF12_3456);
        let loc = host_world_to_radar_coord(glam::Vec3::new(1.0, 2.0, 3.0));
        assert!((loc.x - 1.0).abs() < f32::EPSILON);
        assert!((loc.y - 3.0).abs() < f32::EPSILON);
        assert!((loc.z - 2.0).abs() < f32::EPSILON);
        let back = radar_coord_to_host_world(loc);
        assert!((back.x - 1.0).abs() < f32::EPSILON);
        assert!((back.y - 2.0).abs() < f32::EPSILON);
        assert!((back.z - 3.0).abs() < f32::EPSILON);
    }

    #[test]
    fn spacebar_last_event_uses_the_radar_not_hud_queue() {
        let mut logic = crate::game_logic::GameLogic::new();
        {
            let radar = logic.radar_mut();
            radar.reset();
            radar.new_map(
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(1024.0, 1024.0, 100.0),
                &[],
            );
            radar.create_event(
                &host_world_to_radar_coord(glam::Vec3::new(150.0, 4.0, 250.0)),
                RadarEventType::UnderAttack,
                4.0,
            );
            radar.create_event(
                &host_world_to_radar_coord(glam::Vec3::new(1.0, 0.0, 1.0)),
                RadarEventType::BeaconPulse,
                0.5,
            );
        }
        let pos = logic
            .last_the_radar_event_host_position()
            .expect("TheRadar last event");
        assert!((pos.x - 150.0).abs() < f32::EPSILON);
        assert!((pos.y - 4.0).abs() < f32::EPSILON);
        assert!((pos.z - 250.0).abs() < f32::EPSILON);
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
    fn radar_edge_audio_uses_retail_misc_audio_event_names() {
        // Retail MiscAudio.ini: RadarNotifyOnlineSound = RadarOn, Offline = RadarOff.
        // SoundEffects.ini defines AudioEvent RadarOn / RadarOff; no RadarOnline/Offline.
        assert_eq!(RADAR_ONLINE_AUDIO, "RadarOn");
        assert_eq!(RADAR_OFFLINE_AUDIO, "RadarOff");
    }
}
