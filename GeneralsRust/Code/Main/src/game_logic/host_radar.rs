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

use game_engine::common::system::radar::{Coord3D, RadarEventType, get_radar_system};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

/// Leftover `Object::on_disabled_edge` radar add/remove pending apply.
#[derive(Debug, Clone, Copy)]
pub struct LeftoverRadarDisabledEdge {
    pub player_id: Option<u32>,
    pub becoming_disabled: bool,
    pub disable_proof: bool,
}

thread_local! {
    static LEFTOVER_RADAR_DISABLED_EDGES: RefCell<Vec<LeftoverRadarDisabledEdge>> =
        const { RefCell::new(Vec::new()) };
}

/// Record leftover onDisabledEdge radar walk for the controlling player.
pub fn record_leftover_radar_disabled_edge(
    player_id: Option<u32>,
    becoming_disabled: bool,
    disable_proof: bool,
) {
    LEFTOVER_RADAR_DISABLED_EDGES.with(|log| {
        log.borrow_mut().push(LeftoverRadarDisabledEdge {
            player_id,
            becoming_disabled,
            disable_proof,
        });
    });
}

/// Drain leftover onDisabledEdge radar walks (Player::removeRadar/addRadar).
pub fn drain_leftover_radar_disabled_edges() -> Vec<LeftoverRadarDisabledEdge> {
    LEFTOVER_RADAR_DISABLED_EDGES.with(|log| std::mem::take(&mut *log.borrow_mut()))
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

/// C++ `TheRadar->getLastEventLoc` for spacebar / MSG_META_VIEW_LAST_RADAR_EVENT.
/// Beacon pulses never become the last event.
pub fn last_the_radar_event_host_position() -> Option<glam::Vec3> {
    get_radar_system()
        .read()
        .ok()
        .and_then(|radar| radar.get_last_event_loc())
        .map(radar_coord_to_host_world)
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

/// C++ `TheRadar->createEvent` — rotating triangle + last-event (not beacon).
pub fn host_create_radar_event(pos: glam::Vec3, event_type: RadarEventType) {
    host_create_radar_event_for(pos, event_type, RADAR_EVENT_SECONDS_TO_LIVE);
}

/// C++ `TheRadar->createEvent` with an explicit lifetime (beacon pulse = 0.5s).
pub fn host_create_radar_event_for(pos: glam::Vec3, event_type: RadarEventType, seconds: f32) {
    if let Ok(mut radar) = get_radar_system().write() {
        radar.create_event(&host_world_to_radar_coord(pos), event_type, seconds);
    }
}

/// C++ `TheRadar->createPlayerEvent` (battle-plan player colors).
pub fn host_create_player_radar_event(
    player_color: u32,
    pos: glam::Vec3,
    event_type: RadarEventType,
) {
    if let Ok(mut radar) = get_radar_system().write() {
        radar.create_player_event(
            player_color,
            &host_world_to_radar_coord(pos),
            event_type,
            RADAR_EVENT_SECONDS_TO_LIVE,
        );
    }
}

/// C++ `Radar::queueTerrainRefresh` (3s delay via leftover `update`).
pub fn host_radar_queue_terrain_refresh() {
    if let Ok(mut radar) = get_radar_system().write() {
        radar.queue_terrain_refresh();
    }
}

/// C++ `Radar::refreshTerrain` immediate rebuild (map load / wave-guide).
pub fn host_radar_refresh_terrain() {
    if let Ok(mut radar) = get_radar_system().write() {
        radar.refresh_terrain();
    }
}

/// True when template is a residual radar-providing Command Center (not fake).
///
/// Leftover/INI attach RadarUpgrade only to America/China CC (and Radar Van).
/// GLACommandCenter has no RadarUpgrade, so a GLA CC name is not a provider.
pub fn is_radar_command_center_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.contains("fake") || n.contains("gla") {
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

/// Leftover RadarUpgrade DisableProof for an applied module on this template.
/// `None` when the template has no RadarUpgrade (GLA CC, barracks, …).
pub fn leftover_radar_upgrade_disable_proof(template_name: &str) -> Option<bool> {
    if is_radar_van_template(template_name) {
        return Some(RADAR_VAN_DISABLE_PROOF);
    }
    if is_radar_command_center_template(template_name) {
        return Some(false);
    }
    None
}

/// Leftover `RadarUpgrade::isAlreadyUpgraded` residual.
/// America CC / RadarVan GrantUpgradeCreate apply on construction.
/// China CC RadarUpgrade stays unapplied until Upgrade_ChinaRadar is tagged.
pub fn leftover_radar_upgrade_is_applied(
    template_name: &str,
    has_required_research_tag: bool,
) -> bool {
    if leftover_radar_upgrade_disable_proof(template_name).is_none() {
        return false;
    }
    if crate::game_logic::host_upgrades::radar_provider_required_research_upgrade(template_name)
        .is_some()
    {
        return has_required_research_tag;
    }
    true
}

/// Leftover `Object::on_disabled_edge` radar walk (object_upgrade.rs:312-319).
/// `Some(disable_proof)` when an applied RadarUpgrade must add/remove radar.
pub fn leftover_on_disabled_edge_radar(
    template_name: &str,
    radar_upgrade_applied: bool,
) -> Option<bool> {
    if !radar_upgrade_applied {
        return None;
    }
    leftover_radar_upgrade_disable_proof(template_name)
}

/// C++ `Object::isDisabled` for radar (DisabledType mask, not UNDER_CONSTRUCTION).
pub fn is_disabled_for_radar(is_disabled: bool, under_construction: bool) -> bool {
    is_disabled && !under_construction
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
        // Fake / GLA CC residual-skip (no RadarUpgrade on GLACommandCenter).
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
    RADAR_ONLINE_AUDIO == "RadarOn"
        && RADAR_OFFLINE_AUDIO == "RadarOff"
        && is_radar_command_center_template("AmericaCommandCenter")
        && is_radar_command_center_template("ChinaCommandCenter")
        && !is_radar_command_center_template("GLA_CommandCenter")
        && !is_radar_command_center_template("GLACommandCenter")
        && !is_radar_command_center_template("Chem_GLACommandCenter")
        && !is_radar_command_center_template("FakeGLACommandCenter")
        && is_radar_van_template("GLAVehicleRadarVan")
        && is_radar_provider_template("USA_CommandCenter")
        && !is_radar_provider_template("GLACommandCenter")
        && is_legal_radar_provider(true, true, true, "USA_CommandCenter")
        && !is_legal_radar_provider(true, false, true, "USA_CommandCenter")
        && !is_legal_radar_provider(true, true, true, "GLACommandCenter")
        && is_legal_radar_provider(true, true, false, "GLAVehicleRadarVan")
        && leftover_radar_upgrade_disable_proof("GLACommandCenter").is_none()
        && leftover_radar_upgrade_disable_proof("AmericaCommandCenter") == Some(false)
        && leftover_radar_upgrade_disable_proof("GLAVehicleRadarVan") == Some(true)
        && leftover_on_disabled_edge_radar("AmericaCommandCenter", true) == Some(false)
        && leftover_on_disabled_edge_radar("GLACommandCenter", true).is_none()
        && leftover_on_disabled_edge_radar("GLAVehicleRadarVan", true) == Some(true)
        && leftover_on_disabled_edge_radar("AmericaCommandCenter", false).is_none()
        && is_disabled_for_radar(true, false)
        && !is_disabled_for_radar(true, true)
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
        assert!(!is_radar_command_center_template("GLA_CommandCenter"));
        assert!(!is_radar_command_center_template("GLACommandCenter"));
        assert!(!is_radar_command_center_template("Slth_GLACommandCenter"));
        assert!(!is_radar_command_center_template("FakeGLACommandCenter"));
        assert!(is_radar_van_template("GLAVehicleRadarVan"));
        assert!(is_radar_van_template("TestRadarVan"));
        assert!(is_radar_provider_template("TestCommandCenter"));
        assert!(!is_radar_provider_template("TestBarracks"));
        assert!(!is_radar_provider_template("GLACommandCenter"));
        assert_eq!(
            leftover_on_disabled_edge_radar("AmericaCommandCenter", true),
            Some(false)
        );
        assert!(leftover_on_disabled_edge_radar("GLACommandCenter", true).is_none());
        assert_eq!(
            leftover_on_disabled_edge_radar("GLAVehicleRadarVan", true),
            Some(true)
        );
    }

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
        {
            let radar_system = get_radar_system();
            let mut radar = radar_system.write().expect("radar write");
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
        let pos = last_the_radar_event_host_position().expect("TheRadar last event");
        assert!((pos.x - 150.0).abs() < f32::EPSILON);
        assert!((pos.y - 4.0).abs() < f32::EPSILON);
        assert!((pos.z - 250.0).abs() < f32::EPSILON);
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
        assert!(!is_legal_radar_provider(
            true,
            true,
            true,
            "GLACommandCenter"
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

    #[test]
    fn radar_edge_audio_uses_retail_misc_audio_event_names() {
        // Retail MiscAudio.ini: RadarNotifyOnlineSound = RadarOn, Offline = RadarOff.
        // SoundEffects.ini defines AudioEvent RadarOn / RadarOff; no RadarOnline/Offline.
        assert_eq!(RADAR_ONLINE_AUDIO, "RadarOn");
        assert_eq!(RADAR_OFFLINE_AUDIO, "RadarOff");
        assert!(honesty_radar_provider_residual_ok());
    }
}
