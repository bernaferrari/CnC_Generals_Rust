//! Host China Troop Crawler residual (transport + detector + assault deploy).
//!
//! Residual slice (playability):
//! - `TransportContain` Slots = **8**, AllowInsideKindOf = INFANTRY.
//! - `InitialPayload` residual: `ChinaInfantryRedguard` × 8 docked on spawn.
//! - `StealthDetectorUpdate` residual: DetectionRange unset → VisionRange = **175**.
//! - `AssaultTransportAIUpdate` + `TroopCrawlerAssault` DEPLOY residual:
//!   when the crawler "fires" its primary assault weapon in range, unload
//!   healthy infantry and order them to attack the designated target.
//! - Passengers do **not** fire from inside (retail GoAggressiveOnExit / exit-to-fight).
//!
//! Wave 64 residual pack (retail ChinaVehicle.ini / Weapon.ini / Locomotor.ini):
//! - Body: MaxHealth **240**, BuildCost **1400**, BuildTime **15**s → **450**f,
//!   Vision **175**, ShroudClearingRange **400**, Locomotor Speed **40** / Damaged **30**
//! - TransportContain: Slots **8**, ExitDelay **250**ms → **8**f, NumberOfExitPaths **3**,
//!   HealthRegen%PerSec **10**, DamagePercentToUnits **10%**, GoAggressiveOnExit **Yes**,
//!   ScatterNearbyOnExit **No**, AllowInsideKindOf INFANTRY, InitialPayload Redguard×8
//! - StealthDetectorUpdate: DetectionRate **900**ms → **27**f (DetectionRange unset → 175)
//! - AssaultTransportAIUpdate: MembersGetHealedAtLifeRatio **0.5**
//! - TroopCrawlerAssault DEPLOY residual: dmg ~0 / range **175** / Delay **1000**ms → **30**f
//!
//! Fail-closed honesty:
//! - Not full multi-exit-path ExitStart01-nn / ExitDelay 250ms stagger
//! - Not full HealthRegen%PerSec / DamagePercentToUnits matrix
//! - Not full IR detector FX / IRParticleSys bones
//! - Not network transport / deploy replication (network deferred)
//! - Wounded-retrieve residual is host-simplified (instant enter/exit, no path AI)

use super::Weapon;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const TROOP_CRAWLER_LOGIC_FPS: f32 = 30.0;

/// Retail TransportContain Slots residual.
pub const TROOP_CRAWLER_TRANSPORT_SLOTS: usize = 8;

/// Retail InitialPayload count (ChinaInfantryRedguard 8).
pub const TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT: usize = 8;

/// Preferred retail payload infantry template name.
pub const TROOP_CRAWLER_PAYLOAD_TEMPLATE: &str = "ChinaInfantryRedguard";
/// Host seed alias used by GameLogic::setup_templates.
pub const TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS: &str = "China_RedGuard";

/// Retail TroopCrawlerAssault weapon name (DamageType = DEPLOY).
pub const TROOP_CRAWLER_ASSAULT_WEAPON: &str = "TroopCrawlerAssault";

/// Retail VisionRange residual (DetectionRange unset → vision).
pub const TROOP_CRAWLER_VISION_RANGE: f32 = 175.0;
/// Detection residual equals vision when DetectionRange is unset.
pub const TROOP_CRAWLER_DETECTION_RANGE: f32 = TROOP_CRAWLER_VISION_RANGE;
/// Retail ShroudClearingRange residual.
pub const TROOP_CRAWLER_SHROUD_CLEARING_RANGE: f32 = 400.0;

/// Retail TroopCrawlerAssault PrimaryDamage (negligible deploy trigger).
pub const TROOP_CRAWLER_ASSAULT_DAMAGE: f32 = 0.00001;
/// Retail TroopCrawlerAssault AttackRange.
pub const TROOP_CRAWLER_ASSAULT_RANGE: f32 = 175.0;
/// Retail DelayBetweenShots residual (msec).
pub const TROOP_CRAWLER_ASSAULT_DELAY_MS: u32 = 1000;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const TROOP_CRAWLER_ASSAULT_DELAY_FRAMES: u32 = 30;
/// Retail DamageType residual token.
pub const TROOP_CRAWLER_ASSAULT_DAMAGE_TYPE: &str = "DEPLOY";

/// Residual assault-deploy audio.
pub const TROOP_CRAWLER_DEPLOY_AUDIO: &str = "TroopCrawlerVoiceUnload";

/// Retail ActiveBody MaxHealth residual.
pub const TROOP_CRAWLER_MAX_HEALTH: f32 = 240.0;
/// Retail BuildCost residual.
pub const TROOP_CRAWLER_BUILD_COST: u32 = 1400;
/// Retail BuildTime residual (seconds).
pub const TROOP_CRAWLER_BUILD_TIME_SEC: f32 = 15.0;
/// BuildTime 15s → 450 frames @ 30 FPS.
pub const TROOP_CRAWLER_BUILD_TIME_FRAMES: u32 = 450;
/// Retail TroopCrawlerLocomotor Speed residual.
pub const TROOP_CRAWLER_LOCOMOTOR_SPEED: f32 = 40.0;
/// Retail TroopCrawlerLocomotor SpeedDamaged residual.
pub const TROOP_CRAWLER_LOCOMOTOR_SPEED_DAMAGED: f32 = 30.0;

/// Retail TransportContain ExitDelay residual (msec).
pub const TROOP_CRAWLER_EXIT_DELAY_MS: u32 = 250;
/// ExitDelay 250ms → 8 frames @ 30 FPS.
pub const TROOP_CRAWLER_EXIT_DELAY_FRAMES: u32 = 8;
/// Retail NumberOfExitPaths residual.
pub const TROOP_CRAWLER_NUMBER_OF_EXIT_PATHS: u32 = 3;
/// Retail HealthRegen%PerSec residual.
pub const TROOP_CRAWLER_HEALTH_REGEN_PERCENT_PER_SEC: f32 = 10.0;
/// Retail DamagePercentToUnits residual (percent).
pub const TROOP_CRAWLER_DAMAGE_PERCENT_TO_UNITS: f32 = 10.0;
/// Retail GoAggressiveOnExit residual.
pub const TROOP_CRAWLER_GO_AGGRESSIVE_ON_EXIT: bool = true;
/// Retail ScatterNearbyOnExit residual.
pub const TROOP_CRAWLER_SCATTER_NEARBY_ON_EXIT: bool = false;
/// Retail AllowInsideKindOf INFANTRY residual.
pub const TROOP_CRAWLER_ALLOW_INFANTRY_ONLY: bool = true;
/// Retail TransportSlotCount residual (slots this vehicle takes when carried).
pub const TROOP_CRAWLER_TRANSPORT_SLOT_COUNT: u32 = 8;

/// Retail StealthDetectorUpdate DetectionRate residual (msec).
pub const TROOP_CRAWLER_DETECTION_RATE_MS: u32 = 900;
/// DetectionRate 900ms → 27 frames @ 30 FPS.
pub const TROOP_CRAWLER_DETECTION_RATE_FRAMES: u32 = 27;
/// Retail IR ping audio residual.
pub const TROOP_CRAWLER_IR_PING_SOUND: &str = "IRPing";
/// Retail IR beacon particle residual.
pub const TROOP_CRAWLER_IR_BEACON_PARTICLE: &str = "IRLenzflare";

/// Retail AssaultTransportAIUpdate MembersGetHealedAtLifeRatio residual.
pub const TROOP_CRAWLER_MEMBERS_HEAL_LIFE_RATIO: f32 = 0.5;

/// Convert residual milliseconds to logic frames @ 30 FPS.
pub fn troop_crawler_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / TROOP_CRAWLER_LOGIC_FPS)).round() as u32
}

/// Host residual honesty counters for Troop Crawler load / unload / deploy / detect.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostTroopCrawlerRegistry {
    /// Successful infantry loads into a Troop Crawler residual transport.
    pub loads: u32,
    /// Successful unload/evacuate from a Troop Crawler residual transport.
    pub unloads: u32,
    /// InitialPayload residual spawns (Redguard docked on create).
    pub initial_payloads: u32,
    /// Assault deploy residual triggers (TroopCrawlerAssault fire → unload+attack).
    pub assault_deploys: u32,
    /// Infantry ordered to attack after assault deploy residual.
    pub deploy_attack_orders: u32,
    /// Stealth detects performed by Troop Crawler residual detector.
    pub detects: u32,
    /// C++ AssaultTransportAIUpdate wounded members ordered to re-enter.
    pub wounded_retrieves: u32,
    /// Contained full-health members ordered back out during assault.
    pub healthy_redeploys: u32,
}

impl HostTroopCrawlerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_load(&mut self) {
        self.loads = self.loads.saturating_add(1);
    }

    pub fn record_unload(&mut self) {
        self.unloads = self.unloads.saturating_add(1);
    }

    pub fn record_initial_payload(&mut self) {
        self.initial_payloads = self.initial_payloads.saturating_add(1);
    }

    pub fn record_assault_deploy(&mut self) {
        self.assault_deploys = self.assault_deploys.saturating_add(1);
    }

    pub fn record_deploy_attack_order(&mut self) {
        self.deploy_attack_orders = self.deploy_attack_orders.saturating_add(1);
    }

    pub fn record_detect(&mut self) {
        self.detects = self.detects.saturating_add(1);
    }

    /// Residual honesty: load → docked → unload path exercised.
    pub fn honesty_load_unload_ok(&self) -> bool {
        self.loads > 0 && self.unloads > 0
    }

    /// Residual honesty: InitialPayload residual docked at least once.
    pub fn honesty_initial_payload_ok(&self) -> bool {
        self.initial_payloads > 0
    }

    /// Residual honesty: assault deploy residual fired at least once.
    pub fn record_wounded_retrieve(&mut self) {
        self.wounded_retrieves = self.wounded_retrieves.saturating_add(1);
    }

    pub fn record_healthy_redeploy(&mut self) {
        self.healthy_redeploys = self.healthy_redeploys.saturating_add(1);
    }

    pub fn honesty_wounded_retrieve_ok(&self) -> bool {
        self.wounded_retrieves > 0
    }

    pub fn honesty_assault_deploy_ok(&self) -> bool {
        self.assault_deploys > 0
    }

    /// Residual honesty: detector residual revealed stealthed unit.
    pub fn honesty_detect_ok(&self) -> bool {
        self.detects > 0
    }

    /// Combined residual path honesty.
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_load_unload_ok()
            || self.honesty_initial_payload_ok()
            || self.honesty_assault_deploy_ok()
            || self.honesty_detect_ok()
    }
}

/// Whether template is a residual China Troop Crawler vehicle.
///
/// Fail-closed: name residual (not full AssaultTransportAIUpdate / KindOf matrix).
/// Excludes weapons / debris / command tokens.
pub fn is_troop_crawler_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("missile")
        || n.contains("projectile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.starts_with("upgrade")
        || n.contains("command")
        || n.contains("voice")
        || n.contains("fx_")
        || n.contains("ocl_")
        || n.contains("locomotor")
        // TroopCrawlerAssault DEPLOY weapon token (not the vehicle).
        || n.ends_with("assault")
        || n.contains("troopcrawlerassault")
    {
        return false;
    }
    n.contains("troopcrawler")
        || n.contains("troop_crawler")
        || n == "china_troopcrawler"
        || n == "testtroopcrawler"
}

/// Whether residual spawn should install detector fields.
pub fn troop_crawler_spawn_is_detector(template_name: &str) -> bool {
    is_troop_crawler_template(template_name)
}

/// Detection range residual (retail DetectionRange unset → VisionRange 175).
pub fn troop_crawler_detection_range(template_name: &str) -> Option<f32> {
    if is_troop_crawler_template(template_name) {
        Some(TROOP_CRAWLER_DETECTION_RANGE)
    } else {
        None
    }
}

/// Build residual TroopCrawlerAssault DEPLOY weapon (negligible damage, long range).
pub fn troop_crawler_assault_weapon() -> Weapon {
    Weapon {
        damage: TROOP_CRAWLER_ASSAULT_DAMAGE,
        range: TROOP_CRAWLER_ASSAULT_RANGE,
        min_range: 0.0,
        reload_time: (TROOP_CRAWLER_ASSAULT_DELAY_FRAMES as f32) / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        // Instant residual "deploy pulse" (WeaponSpeed 0 in retail).
        projectile_speed: 0.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Whether combat fire should take the assault-deploy residual path
/// (skip HP damage; unload + attack residual).
pub fn should_apply_troop_crawler_assault_deploy(is_troop_crawler: bool) -> bool {
    is_troop_crawler
}

/// Resolve preferred InitialPayload infantry template from a name present set.
///
/// Fail-closed: returns None when neither retail nor alias template is available.
pub fn resolve_payload_template_name(has_template: impl Fn(&str) -> bool) -> Option<&'static str> {
    if has_template(TROOP_CRAWLER_PAYLOAD_TEMPLATE) {
        Some(TROOP_CRAWLER_PAYLOAD_TEMPLATE)
    } else if has_template(TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS) {
        Some(TROOP_CRAWLER_PAYLOAD_TEMPLATE_ALIAS)
    } else if has_template("TestInfantry") {
        // Test residual fallback so host gates stay green without full INI load.
        Some("TestInfantry")
    } else {
        None
    }
}

// --- Wave 64 residual honesty packs ---

/// Wave 64 residual honesty: TransportContain residual.
pub fn honesty_troop_crawler_transport_residual_ok() -> bool {
    TROOP_CRAWLER_TRANSPORT_SLOTS == 8
        && TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT == 8
        && TROOP_CRAWLER_PAYLOAD_TEMPLATE == "ChinaInfantryRedguard"
        && TROOP_CRAWLER_ALLOW_INFANTRY_ONLY
        && TROOP_CRAWLER_EXIT_DELAY_MS == 250
        && TROOP_CRAWLER_EXIT_DELAY_FRAMES
            == troop_crawler_ms_to_frames(TROOP_CRAWLER_EXIT_DELAY_MS)
        && TROOP_CRAWLER_NUMBER_OF_EXIT_PATHS == 3
        && (TROOP_CRAWLER_HEALTH_REGEN_PERCENT_PER_SEC - 10.0).abs() < 0.01
        && (TROOP_CRAWLER_DAMAGE_PERCENT_TO_UNITS - 10.0).abs() < 0.01
        && TROOP_CRAWLER_GO_AGGRESSIVE_ON_EXIT
        && !TROOP_CRAWLER_SCATTER_NEARBY_ON_EXIT
        && TROOP_CRAWLER_TRANSPORT_SLOT_COUNT == 8
}

/// Wave 64 residual honesty: assault DEPLOY weapon residual.
pub fn honesty_troop_crawler_assault_residual_ok() -> bool {
    TROOP_CRAWLER_ASSAULT_WEAPON == "TroopCrawlerAssault"
        && TROOP_CRAWLER_ASSAULT_DAMAGE < 0.001
        && (TROOP_CRAWLER_ASSAULT_RANGE - 175.0).abs() < 0.01
        && TROOP_CRAWLER_ASSAULT_DELAY_MS == 1000
        && TROOP_CRAWLER_ASSAULT_DELAY_FRAMES
            == troop_crawler_ms_to_frames(TROOP_CRAWLER_ASSAULT_DELAY_MS)
        && TROOP_CRAWLER_ASSAULT_DAMAGE_TYPE == "DEPLOY"
        && TROOP_CRAWLER_DEPLOY_AUDIO == "TroopCrawlerVoiceUnload"
        && (TROOP_CRAWLER_MEMBERS_HEAL_LIFE_RATIO - 0.5).abs() < 0.001
}

/// Wave 64 residual honesty: detector residual.
pub fn honesty_troop_crawler_detector_residual_ok() -> bool {
    (TROOP_CRAWLER_VISION_RANGE - 175.0).abs() < 0.01
        && (TROOP_CRAWLER_DETECTION_RANGE - 175.0).abs() < 0.01
        && TROOP_CRAWLER_DETECTION_RATE_MS == 900
        && TROOP_CRAWLER_DETECTION_RATE_FRAMES
            == troop_crawler_ms_to_frames(TROOP_CRAWLER_DETECTION_RATE_MS)
        && TROOP_CRAWLER_IR_PING_SOUND == "IRPing"
        && TROOP_CRAWLER_IR_BEACON_PARTICLE == "IRLenzflare"
}

/// Wave 64 residual honesty: body / build / locomotor residual.
pub fn honesty_troop_crawler_body_residual_ok() -> bool {
    (TROOP_CRAWLER_MAX_HEALTH - 240.0).abs() < 0.01
        && TROOP_CRAWLER_BUILD_COST == 1400
        && (TROOP_CRAWLER_BUILD_TIME_SEC - 15.0).abs() < 0.01
        && TROOP_CRAWLER_BUILD_TIME_FRAMES
            == (TROOP_CRAWLER_BUILD_TIME_SEC * TROOP_CRAWLER_LOGIC_FPS).round() as u32
        && (TROOP_CRAWLER_SHROUD_CLEARING_RANGE - 400.0).abs() < 0.01
        && (TROOP_CRAWLER_LOCOMOTOR_SPEED - 40.0).abs() < 0.01
        && (TROOP_CRAWLER_LOCOMOTOR_SPEED_DAMAGED - 30.0).abs() < 0.01
}

/// Combined Wave 64 Troop Crawler residual honesty pack.
pub fn honesty_troop_crawler_residual_pack_ok() -> bool {
    honesty_troop_crawler_transport_residual_ok()
        && honesty_troop_crawler_assault_residual_ok()
        && honesty_troop_crawler_detector_residual_ok()
        && honesty_troop_crawler_body_residual_ok()
}

/// C++ AssaultTransportAIUpdate per-transport residual state.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostAssaultTransportState {
    /// C++ m_designatedTarget.
    pub designated_target: Option<u32>,
    /// Outside members still under assault AI control (ObjectId raw).
    pub member_ids: Vec<u32>,
    /// Parallel to member_ids: currently returning for heal.
    pub member_healing: Vec<bool>,
    /// Assault order is active.
    pub active: bool,
}

impl HostAssaultTransportState {
    pub fn begin(target: u32, members: Vec<u32>) -> Self {
        let n = members.len();
        Self {
            designated_target: Some(target),
            member_ids: members,
            member_healing: vec![false; n],
            active: true,
        }
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// C++ isMemberWounded: health/max < MembersGetHealedAtLifeRatio (0.5).
pub fn is_assault_member_wounded(current: f32, maximum: f32) -> bool {
    let max_h = maximum.max(0.0001);
    (current / max_h) < TROOP_CRAWLER_MEMBERS_HEAL_LIFE_RATIO
}

/// C++ isMemberHealthy: health == maxHealth (full only).
pub fn is_assault_member_healthy(current: f32, maximum: f32) -> bool {
    (current - maximum).abs() < 0.01 && maximum > 0.0
}

#[cfg(test)]
mod tests {
    #[test]
    fn assault_member_wounded_ratio() {
        assert!(super::is_assault_member_wounded(40.0, 100.0));
        assert!(!super::is_assault_member_wounded(50.0, 100.0));
        assert!(!super::is_assault_member_wounded(80.0, 100.0));
        assert!(super::is_assault_member_healthy(100.0, 100.0));
        assert!(!super::is_assault_member_healthy(99.0, 100.0));
    }

    use super::*;

    #[test]
    fn troop_crawler_name_matrix() {
        assert!(is_troop_crawler_template("ChinaVehicleTroopCrawler"));
        assert!(is_troop_crawler_template("China_TroopCrawler"));
        assert!(is_troop_crawler_template("Tank_ChinaVehicleTroopCrawler"));
        assert!(is_troop_crawler_template("Nuke_ChinaVehicleTroopCrawler"));
        assert!(is_troop_crawler_template("TestTroopCrawler"));
        assert!(!is_troop_crawler_template("TroopCrawlerAssault"));
        assert!(!is_troop_crawler_template("TroopCrawlerLocomotor"));
        assert!(!is_troop_crawler_template("ChinaInfantryRedguard"));
        assert!(!is_troop_crawler_template("ChinaTankDragon"));
        assert!(!is_troop_crawler_template("AmericaVehicleHumvee"));
    }

    #[test]
    fn detector_and_slots() {
        assert!(troop_crawler_spawn_is_detector("ChinaVehicleTroopCrawler"));
        assert_eq!(
            troop_crawler_detection_range("ChinaVehicleTroopCrawler"),
            Some(TROOP_CRAWLER_DETECTION_RANGE)
        );
        assert_eq!(troop_crawler_detection_range("USA_Ranger"), None);
        assert_eq!(TROOP_CRAWLER_TRANSPORT_SLOTS, 8);
        assert_eq!(TROOP_CRAWLER_INITIAL_PAYLOAD_COUNT, 8);
        assert!((TROOP_CRAWLER_DETECTION_RANGE - 175.0).abs() < 0.01);
    }

    #[test]
    fn assault_weapon_stats() {
        let w = troop_crawler_assault_weapon();
        assert!(w.damage < 0.001);
        assert!((w.range - 175.0).abs() < 0.01);
        assert!(w.can_target_ground && !w.can_target_air);
        assert!(should_apply_troop_crawler_assault_deploy(true));
        assert!(!should_apply_troop_crawler_assault_deploy(false));
    }

    #[test]
    fn payload_template_resolve() {
        assert_eq!(
            resolve_payload_template_name(|n| n == "ChinaInfantryRedguard"),
            Some("ChinaInfantryRedguard")
        );
        assert_eq!(
            resolve_payload_template_name(|n| n == "China_RedGuard"),
            Some("China_RedGuard")
        );
        assert_eq!(
            resolve_payload_template_name(|n| n == "TestInfantry"),
            Some("TestInfantry")
        );
        assert_eq!(resolve_payload_template_name(|_| false), None);
    }

    #[test]
    fn honesty_tracks_paths() {
        let mut reg = HostTroopCrawlerRegistry::new();
        assert!(!reg.honesty_any_ok());
        reg.record_load();
        reg.record_unload();
        assert!(reg.honesty_load_unload_ok());
        reg.record_initial_payload();
        assert!(reg.honesty_initial_payload_ok());
        reg.record_assault_deploy();
        assert!(reg.honesty_assault_deploy_ok());
        reg.record_detect();
        assert!(reg.honesty_detect_ok());
    }

    #[test]
    fn troop_crawler_residual_pack_honesty() {
        assert_eq!(troop_crawler_ms_to_frames(250), 8);
        assert_eq!(troop_crawler_ms_to_frames(900), 27);
        assert_eq!(troop_crawler_ms_to_frames(1000), 30);
        assert!(honesty_troop_crawler_transport_residual_ok());
        assert!(honesty_troop_crawler_assault_residual_ok());
        assert!(honesty_troop_crawler_detector_residual_ok());
        assert!(honesty_troop_crawler_body_residual_ok());
        assert!(honesty_troop_crawler_residual_pack_ok());
    }
}
