//! Host USA Pilot residual (eject recrew of unmanned vehicles + EjectPilotDie).
//!
//! Residual slice (playability):
//! - `AmericaInfantryPilot` / AirF_ / CINE_ / TestPilot enter unmanned ground
//!   vehicles (DISABLED_UNMANNED residual from Jarmen Kell snipe / neutron) →
//!   recrew: clear unmanned, transfer team to pilot's team, transfer pilot
//!   veterancy (retail `VeterancyCrateCollide` IsPilot + AddsOwnerVeterancy),
//!   consume pilot.
//! - Pilots spawn residual at least VETERAN (VeterancyGainCreate StartingLevel).
//! - **EjectPilotDie residual**: eligible USA ground vehicles (Humvee / Tomahawk /
//!   Crusader / Paladin / Avenger / Microwave + general variants) spawn
//!   `AmericaInfantryPilot` on death via OCL_EjectPilotOnGround residual path.
//!   Fail-closed: unmanned vehicles do not eject (no pilot left).
//! - **VeterancyLevels residual**: retail `VeterancyLevels = ALL -REGULAR`
//!   ("only vet+ gives pilot"). Rookie / LEVEL_REGULAR vehicles do **not** eject.
//! - **InvulnerableTime residual**: OCL_EjectPilotOnGround `InvulnerableTime = 2000`
//!   ms → **60 frames**. Post-eject pilot residual blocks damage (host of C++
//!   `goInvulnerable` / undetected-defector relationship shield).
//! - **PilotFindVehicleUpdate residual**: AI-only idle pilot auto-scan
//!   (ScanRate **1000**ms → **30** frames, ScanRange **300**, MinHealth **0.5**)
//!   toward nearest recrewable unmanned vehicle → Enter residual.
//!   C++ human players sleep forever (no auto-scan).
//! - **Base-center fallback residual**: when no vehicle found, AI pilot moves once
//!   toward team command-center / base (`m_didMoveToBase`).
//! - **AutoFindHealingUpdate residual**: AI-only idle injured USA infantry
//!   auto-scan (ScanRate **1000**ms → **30** frames, ScanRange **300**,
//!   NeverHeal **0.85**, AlwaysHeal **0.25**) toward nearest HealPad →
//!   SeekingHealing residual. Templates: Pilot / Ranger / MissileDefender /
//!   Pathfinder / ColonelBurton residual family.
//! - **Air OCL parachute residual**: when dying vehicle is significantly above
//!   terrain (C++ `isSignificantlyAboveTerrain` / OCL_EjectPilotViaParachute),
//!   pilot spawns elevated + parachuting residual sink until ground.
//!
//! Fail-closed honesty:
//! - Not full AmericaParachute container OpenClose / fall-physics matrix
//! - Not full PilotFindVehicleUpdate CollideModule wouldLikeToCollideWith matrix
//! - Not full AutoFindHealingUpdate AlwaysHeal busy-interrupt path
//!   (C++ early-return makes busy path unreachable — host matches idle-only)
//! - Not full defector FX flash / UNDETECTED_DEFECTOR relationship matrix
//! - Not network recrew / pilot-eject replication (network deferred)

use super::VeterancyLevel;
use serde::{Deserialize, Serialize};

/// Retail pilot template family residual.
pub const PILOT_RECREW_AUDIO: &str = "PilotEnterVehicle";

/// Retail OCL_EjectPilotOnGround / OCL_EjectPilotViaParachute ObjectNames residual.
pub const EJECT_PILOT_TEMPLATE: &str = "AmericaInfantryPilot";

/// Residual eject audio (VoiceEject / SoundEject fail-closed host cue).
pub const PILOT_EJECT_AUDIO: &str = "VoiceEject";

/// Retail OCL_EjectPilotOnGround InvulnerableTime (ms).
pub const EJECT_PILOT_INVULNERABLE_MS: u32 = 2000;

/// Logic frames per second (host fixed step).
pub const EJECT_PILOT_LOGIC_FPS: f32 = 30.0;

/// Retail InvulnerableTime → frames at 30 FPS (2000 / (1000/30) = 60).
pub const EJECT_PILOT_INVULNERABLE_FRAMES: u32 = 60;

// --- PilotFindVehicleUpdate residual (AmericaInfantryPilot) ---

/// Retail PilotFindVehicleUpdate ScanRate (ms).
pub const PILOT_FIND_VEHICLE_SCAN_RATE_MS: u32 = 1000;

/// ScanRate → frames at 30 FPS (1000 / (1000/30) = 30).
pub const PILOT_FIND_VEHICLE_SCAN_FRAMES: u32 = 30;

/// Retail PilotFindVehicleUpdate ScanRange.
pub const PILOT_FIND_VEHICLE_SCAN_RANGE: f32 = 300.0;

/// Retail PilotFindVehicleUpdate MinHealth (don't enter vehicle below this ratio).
pub const PILOT_FIND_VEHICLE_MIN_HEALTH: f32 = 0.5;

// --- AutoFindHealingUpdate residual (AmericaInfantryPilot ModuleTag_06) ---

/// Retail AutoFindHealingUpdate ScanRate (ms).
pub const AUTO_FIND_HEALING_SCAN_RATE_MS: u32 = 1000;

/// ScanRate → frames at 30 FPS (1000 / (1000/30) = 30).
pub const AUTO_FIND_HEALING_SCAN_FRAMES: u32 = 30;

/// Retail AutoFindHealingUpdate ScanRange.
pub const AUTO_FIND_HEALING_SCAN_RANGE: f32 = 300.0;

/// Retail AutoFindHealingUpdate NeverHeal (don't heal above this ratio).
pub const AUTO_FIND_HEALING_NEVER_HEAL: f32 = 0.85;

/// Retail AutoFindHealingUpdate AlwaysHeal (busy-interrupt threshold residual).
/// Host residual fail-closed: only idle units scan (C++ "for now, only heal if idle"
/// early-return makes the AlwaysHeal branch unreachable in retail).
pub const AUTO_FIND_HEALING_ALWAYS_HEAL: f32 = 0.25;

// --- EjectPilotDie air OCL / isSignificantlyAboveTerrain residual ---

/// Retail GameData.ini Gravity residual.
pub const HOST_GRAVITY: f32 = -64.0;

/// C++ `Thing::isSignificantlyAboveTerrain` threshold: `-(3*3)*gravity`.
/// With Gravity=-64 → height > **576** is significantly airborne.
pub fn significantly_above_terrain_threshold() -> f32 {
    -(3.0 * 3.0) * HOST_GRAVITY
}

/// Host residual parachute sink (world units per logic frame).
/// Fail-closed: not full AmericaParachute OpenClose / PhysicsUpdate matrix.
pub const EJECT_PARACHUTE_SINK_PER_FRAME: f32 = 20.0;

/// Residual audio when air-ejected pilot lands (host of parachute open residual).
pub const PILOT_PARACHUTE_LAND_AUDIO: &str = "ParadropLanding";

/// Convert InvulnerableTime ms → logic frames (30 FPS residual).
pub fn eject_pilot_invulnerable_frames_from_ms(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / EJECT_PILOT_LOGIC_FPS)).round() as u32
}

/// Absolute host frame when post-eject InvulnerableTime residual expires.
pub fn eject_pilot_invulnerable_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(EJECT_PILOT_INVULNERABLE_FRAMES.max(1))
}

/// Whether template is a residual USA Pilot infantry.
///
/// Fail-closed: name residual. Excludes weapons / science / debris / pathfinder.
pub fn is_pilot_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("pathfinder")
        || n.contains("ranger")
        || n.contains("colonel")
        || n.contains("burton")
        || n.contains("commandset")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testpilot" || n == "usa_pilot" || n == "americainfantrypilot" {
        return true;
    }
    // AmericaInfantryPilot / AirF_AmericaInfantryPilot / CINE_AmericaInfantryPilot
    n.contains("infantrypilot") || n.ends_with("pilot") && n.contains("america")
}

/// Residual pilot starting veterancy (VeterancyGainCreate StartingLevel = VETERAN).
pub fn pilot_default_veterancy() -> VeterancyLevel {
    VeterancyLevel::Veteran
}

/// Whether template is a residual USA vehicle with EjectPilotDie module.
///
/// Retail AmericaVehicle.ini / general variants: Humvee, Tomahawk, Crusader,
/// Paladin, Avenger, Microwave. Fail-closed name residual (not full DieMux).
pub fn is_eject_pilot_eligible_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Exclude drones / hulks / weapons / infantry pilots themselves.
    if n.contains("drone")
        || n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("infantry")
        || n.contains("pilot")
        || n.contains("dozer")
        || n.contains("sentry")
        || n.contains("chinook")
        || n.contains("comanche")
        || n.contains("raptor")
        || n.contains("stealthfighter")
        || n.contains("aurora")
        || n.contains("jet")
        || n.contains("helicopter")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testejectvehicle"
        || n == "testejectpilotvehicle"
        || n == "goldenhumvee"
        || n == "usa_humvee"
        || n == "usa_crusader"
        || n == "usa_paladin"
        || n == "usa_tomahawk"
        || n == "usa_avenger"
        || n == "usa_microwave"
    {
        return true;
    }
    n.contains("humvee")
        || n.contains("tomahawk")
        || n.contains("tankcrusader")
        || n.contains("tankpaladin")
        || n.contains("tankavenger")
        || n.contains("tankmicrowave")
        || (n.contains("crusader") && n.contains("tank"))
        || (n.contains("paladin") && n.contains("tank"))
        || (n.contains("avenger") && n.contains("tank"))
        || (n.contains("microwave") && (n.contains("tank") || n.contains("vehicle")))
}

/// Retail `VeterancyLevels = ALL -REGULAR` residual gate.
///
/// C++ DieMuxData::isDieApplicable → getVeterancyLevelFlag(m_veterancyLevels, level).
/// LEVEL_REGULAR / Rookie is excluded; Veteran / Elite / Heroic eject.
pub fn meets_eject_pilot_veterancy_gate(level: VeterancyLevel) -> bool {
    !matches!(level, VeterancyLevel::Rookie)
}

/// Whether residual EjectPilotDie should fire on death.
///
/// Fail-closed: eligible template, not unmanned, not under construction,
/// vehicle kind residual (not structure / aircraft), VeterancyLevels ALL -REGULAR.
pub fn can_eject_pilot_on_death(
    is_eligible_template: bool,
    is_unmanned: bool,
    under_construction: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    meets_veterancy_gate: bool,
) -> bool {
    is_eligible_template
        && !is_unmanned
        && !under_construction
        && is_vehicle
        && !is_aircraft
        && meets_veterancy_gate
}

/// Whether PilotFindVehicleUpdate residual may auto-scan this pilot.
///
/// C++: AI-only (`PLAYER_HUMAN` → UPDATE_SLEEP_FOREVER); idle AI only.
pub fn pilot_find_vehicle_scan_eligible(
    is_pilot: bool,
    is_alive: bool,
    is_idle: bool,
    is_ai_controlled: bool,
) -> bool {
    is_pilot && is_alive && is_idle && is_ai_controlled
}

/// Whether vehicle health meets PilotFindVehicleUpdate MinHealth residual.
///
/// C++ skips targets with `health < maxHealth * MinHealth` (default 0.5).
pub fn vehicle_meets_pilot_find_min_health(health: f32, max_health: f32, min_ratio: f32) -> bool {
    if max_health <= 0.0 {
        return false;
    }
    health >= max_health * min_ratio
}

/// Whether a candidate vehicle is a valid PilotFindVehicle target residual.
///
/// Host residual: recrewable unmanned path (not full VeterancyCrate same-team
/// exp-gain matrix). MinHealth + range gates preserved.
pub fn is_pilot_find_vehicle_target(
    recrewable: bool,
    health_ok: bool,
    in_scan_range: bool,
) -> bool {
    recrewable && health_ok && in_scan_range
}

/// Whether current frame is a PilotFindVehicle scan tick residual.
pub fn pilot_find_vehicle_scan_frame(frame: u32) -> bool {
    frame.is_multiple_of(PILOT_FIND_VEHICLE_SCAN_FRAMES)
}

/// Whether PilotFindVehicle residual should issue base-center fallback.
///
/// C++: when `scanClosestTarget()` is null and `!m_didMoveToBase` and
/// `getAiBaseCenter` succeeds → `aiMoveToPosition` once.
pub fn should_pilot_base_center_fallback(
    found_vehicle: bool,
    did_move_to_base: bool,
    has_base_center: bool,
) -> bool {
    !found_vehicle && !did_move_to_base && has_base_center
}

/// Whether template has residual AutoFindHealingUpdate module.
///
/// Retail America infantry: Pilot / Ranger / MissileDefender / Pathfinder /
/// ColonelBurton (+ general variants). Fail-closed name residual (not full INI
/// module parse).
pub fn is_auto_find_healing_template(template_name: &str) -> bool {
    if is_pilot_template(template_name) {
        return true;
    }
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missileweapon")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("commandset")
        || n.contains("flashbang")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testranger"
        || n == "usa_ranger"
        || n == "goldenranger"
        || n == "testmissiledefender"
        || n == "usa_missiledefender"
        || n == "testpathfinder"
        || n == "usa_pathfinder"
        || n == "testcolonelburton"
        || n == "usa_colonelburton"
        || n == "testburton"
    {
        return true;
    }
    // AmericaInfantryRanger / *MissileDefender / *Pathfinder / *ColonelBurton
    n.contains("infantryranger")
        || (n.contains("ranger") && n.contains("america"))
        || n.contains("missiledefender")
        || n.contains("infantrypathfinder")
        || (n.contains("pathfinder") && (n.contains("america") || n.contains("usa")))
        || n.contains("colonelburton")
}

/// C++ Thing::isSignificantlyAboveTerrain residual.
///
/// Host: height_above_terrain > -(3*3)*Gravity (576 with Gravity=-64).
pub fn is_significantly_above_terrain(height_above_terrain: f32) -> bool {
    height_above_terrain > significantly_above_terrain_threshold()
}

/// Whether EjectPilotDie should use air OCL (OCL_EjectPilotViaParachute residual).
///
/// C++: `isSignificantlyAboveTerrain() ? m_oclInAir : m_oclOnGround`.
/// Host also accepts `airborne_target` residual flag.
pub fn uses_air_eject_ocl(height_above_terrain: f32, airborne_target: bool) -> bool {
    airborne_target || is_significantly_above_terrain(height_above_terrain)
}

/// Residual air-eject spawn height (keep elevated; floor at threshold+1).
pub fn air_eject_spawn_height(death_height: f32) -> f32 {
    death_height.max(significantly_above_terrain_threshold() + 1.0)
}

/// Advance parachute residual sink toward ground (y height axis).
///
/// Returns (new_height, landed). Fail-closed linear sink (not full parachute physics).
pub fn tick_parachute_height(current_height: f32, ground_height: f32) -> (f32, bool) {
    if current_height <= ground_height + 0.01 {
        return (ground_height, true);
    }
    let next = (current_height - EJECT_PARACHUTE_SINK_PER_FRAME).max(ground_height);
    let landed = next <= ground_height + 0.01;
    (if landed { ground_height } else { next }, landed)
}

/// Whether AutoFindHealingUpdate residual may auto-scan this unit.
///
/// C++: human players return early (no scan); AI idle only (busy path fail-closed).
/// `has_auto_find_healing` covers pilot + residual USA infantry module templates.
pub fn auto_find_healing_scan_eligible(
    has_auto_find_healing: bool,
    is_alive: bool,
    is_idle: bool,
    is_ai_controlled: bool,
) -> bool {
    has_auto_find_healing && is_alive && is_idle && is_ai_controlled
}

/// Whether health is low enough to seek healing residual.
///
/// C++ skips scan when `health > maxHealth * NeverHeal` (retail pilot NeverHeal 0.85).
pub fn health_needs_auto_find_healing(health: f32, max_health: f32, never_heal: f32) -> bool {
    if max_health <= 0.0 {
        return false;
    }
    health <= max_health * never_heal
}

/// Whether current frame is an AutoFindHealing scan tick residual.
pub fn auto_find_healing_scan_frame(frame: u32) -> bool {
    frame.is_multiple_of(AUTO_FIND_HEALING_SCAN_FRAMES)
}

/// Whether a candidate is a valid AutoFindHealing HealPad residual target.
pub fn is_auto_find_healing_target(is_heal_pad: bool, is_alive: bool, in_scan_range: bool) -> bool {
    is_heal_pad && is_alive && in_scan_range
}

/// Rank for residual veterancy transfer (higher wins).
pub fn veterancy_rank(level: VeterancyLevel) -> u8 {
    match level {
        VeterancyLevel::Rookie => 0,
        VeterancyLevel::Veteran => 1,
        VeterancyLevel::Elite => 2,
        VeterancyLevel::Heroic => 3,
    }
}

/// Whether residual vehicle can be recrewed by a pilot.
///
/// Fail-closed: live ground vehicle, unmanned, not under construction, not aircraft.
pub fn is_recrewable_unmanned_vehicle(
    is_alive: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    is_unmanned: bool,
    under_construction: bool,
    is_dozer: bool,
) -> bool {
    // Retail VeterancyCrateCollide ForbiddenKindOf = DOZER residual.
    is_alive && is_vehicle && !is_aircraft && is_unmanned && !under_construction && !is_dozer
}

/// Whether an enter command should take the pilot recrew residual path.
pub fn should_recrew_on_enter(is_pilot: bool, vehicle_recrewable: bool) -> bool {
    is_pilot && vehicle_recrewable
}

/// Merged veterancy after recrew: max(vehicle, pilot).
pub fn merged_recrew_veterancy(
    vehicle_level: VeterancyLevel,
    pilot_level: VeterancyLevel,
) -> VeterancyLevel {
    if veterancy_rank(pilot_level) >= veterancy_rank(vehicle_level) {
        pilot_level
    } else {
        vehicle_level
    }
}

/// Host residual honesty counters for USA Pilot recrew + EjectPilotDie.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostUsaPilotRegistry {
    /// Successful unmanned-vehicle recrews (pilot consumed).
    pub recrews: u32,
    /// Veterancy promotions applied onto recrewed vehicles.
    pub veterancy_transfers: u32,
    /// Successful EjectPilotDie residual pilot spawns on vehicle death.
    #[serde(default)]
    pub ejections: u32,
    /// Post-eject InvulnerableTime residual grants applied.
    #[serde(default)]
    pub invulnerable_grants: u32,
    /// Residual damage attempts blocked by InvulnerableTime.
    #[serde(default)]
    pub invulnerable_blocks: u32,
    /// PilotFindVehicleUpdate residual auto-scan Enter orders issued.
    #[serde(default)]
    pub find_vehicle_orders: u32,
    /// Rookie / REGULAR deaths skipped by VeterancyLevels gate residual.
    #[serde(default)]
    pub eject_veterancy_blocks: u32,
    /// PilotFindVehicleUpdate base-center fallback residual moves issued.
    #[serde(default)]
    pub base_center_moves: u32,
    /// AutoFindHealingUpdate residual SeekingHealing orders issued.
    #[serde(default)]
    pub auto_heal_orders: u32,
    /// EjectPilotDie air OCL parachute residual spawns (significantly above terrain).
    #[serde(default)]
    pub air_ejections: u32,
    /// Air-ejected pilot parachute residual landings completed.
    #[serde(default)]
    pub parachute_lands: u32,
    /// AutoFindHealingUpdate residual orders issued by non-pilot infantry.
    #[serde(default)]
    pub infantry_auto_heal_orders: u32,
}

impl HostUsaPilotRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_recrew(&mut self, transferred_veterancy: bool) {
        self.recrews = self.recrews.saturating_add(1);
        if transferred_veterancy {
            self.veterancy_transfers = self.veterancy_transfers.saturating_add(1);
        }
    }

    pub fn record_ejection(&mut self) {
        self.ejections = self.ejections.saturating_add(1);
    }

    pub fn record_invulnerable_grant(&mut self) {
        self.invulnerable_grants = self.invulnerable_grants.saturating_add(1);
    }

    pub fn record_invulnerable_block(&mut self) {
        self.invulnerable_blocks = self.invulnerable_blocks.saturating_add(1);
    }

    pub fn record_find_vehicle_order(&mut self) {
        self.find_vehicle_orders = self.find_vehicle_orders.saturating_add(1);
    }

    pub fn record_eject_veterancy_block(&mut self) {
        self.eject_veterancy_blocks = self.eject_veterancy_blocks.saturating_add(1);
    }

    pub fn record_base_center_move(&mut self) {
        self.base_center_moves = self.base_center_moves.saturating_add(1);
    }

    pub fn record_auto_heal_order(&mut self) {
        self.auto_heal_orders = self.auto_heal_orders.saturating_add(1);
    }

    pub fn record_air_ejection(&mut self) {
        // Caller also records a normal ejection; this only tags air OCL residual.
        self.air_ejections = self.air_ejections.saturating_add(1);
    }

    pub fn record_parachute_land(&mut self) {
        self.parachute_lands = self.parachute_lands.saturating_add(1);
    }

    pub fn record_infantry_auto_heal_order(&mut self) {
        // Caller may also use record_auto_heal_order for pilot; this tags non-pilot.
        self.infantry_auto_heal_orders = self.infantry_auto_heal_orders.saturating_add(1);
        self.auto_heal_orders = self.auto_heal_orders.saturating_add(1);
    }

    /// Residual honesty: at least one recrew completed.
    pub fn honesty_recrew_ok(&self) -> bool {
        self.recrews > 0
    }

    /// Residual honesty: recrew path with veterancy transfer observed.
    pub fn honesty_veterancy_transfer_ok(&self) -> bool {
        self.recrews > 0 && self.veterancy_transfers > 0
    }

    /// Residual honesty: at least one EjectPilotDie pilot spawn.
    pub fn honesty_eject_ok(&self) -> bool {
        self.ejections > 0
    }

    /// Residual honesty: InvulnerableTime residual granted on eject.
    pub fn honesty_invulnerable_ok(&self) -> bool {
        self.invulnerable_grants > 0
    }

    /// Residual honesty: InvulnerableTime blocked at least one damage attempt.
    pub fn honesty_invulnerable_block_ok(&self) -> bool {
        self.invulnerable_blocks > 0
    }

    /// Residual honesty: PilotFindVehicleUpdate issued at least one Enter order.
    pub fn honesty_find_vehicle_ok(&self) -> bool {
        self.find_vehicle_orders > 0
    }

    /// Residual honesty: VeterancyLevels REGULAR gate blocked at least one eject.
    pub fn honesty_eject_veterancy_gate_ok(&self) -> bool {
        self.eject_veterancy_blocks > 0
    }

    /// Residual honesty: PilotFindVehicle base-center fallback issued at least once.
    pub fn honesty_base_center_ok(&self) -> bool {
        self.base_center_moves > 0
    }

    /// Residual honesty: AutoFindHealingUpdate issued at least one SeekingHealing order.
    pub fn honesty_auto_heal_ok(&self) -> bool {
        self.auto_heal_orders > 0
    }

    /// Residual honesty: at least one air OCL parachute eject residual.
    pub fn honesty_air_eject_ok(&self) -> bool {
        self.air_ejections > 0
    }

    /// Residual honesty: at least one parachute residual landing completed.
    pub fn honesty_parachute_land_ok(&self) -> bool {
        self.parachute_lands > 0
    }

    /// Residual honesty: non-pilot infantry AutoFindHealing residual issued.
    pub fn honesty_infantry_auto_heal_ok(&self) -> bool {
        self.infantry_auto_heal_orders > 0
    }

    /// Combined pilot residual honesty (recrew or eject path).
    pub fn honesty_pilot_ok(&self) -> bool {
        self.honesty_recrew_ok()
            || self.honesty_eject_ok()
            || self.honesty_find_vehicle_ok()
            || self.honesty_base_center_ok()
            || self.honesty_auto_heal_ok()
            || self.honesty_air_eject_ok()
            || self.honesty_parachute_land_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pilot_name_matrix() {
        assert!(is_pilot_template("AmericaInfantryPilot"));
        assert!(is_pilot_template("AirF_AmericaInfantryPilot"));
        assert!(is_pilot_template("CINE_AmericaInfantryPilot"));
        assert!(is_pilot_template("TestPilot"));
        assert!(is_pilot_template("USA_Pilot"));
        assert!(!is_pilot_template("AmericaInfantryRanger"));
        assert!(!is_pilot_template("AmericaInfantryPathfinder"));
        assert!(!is_pilot_template("AmericaInfantryColonelBurton"));
        assert!(!is_pilot_template("Upgrade_AmericaPilot"));
        assert!(!is_pilot_template("GLAInfantryWorker"));
    }

    #[test]
    fn recrewable_gate() {
        assert!(is_recrewable_unmanned_vehicle(
            true, true, false, true, false, false
        ));
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, false, false, false, false
        )); // manned
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, true, true, false, false
        )); // aircraft
        assert!(!is_recrewable_unmanned_vehicle(
            true, true, false, true, false, true
        )); // dozer forbidden
        assert!(!is_recrewable_unmanned_vehicle(
            false, true, false, true, false, false
        )); // dead
    }

    #[test]
    fn veterancy_merge() {
        assert_eq!(
            merged_recrew_veterancy(VeterancyLevel::Rookie, VeterancyLevel::Veteran),
            VeterancyLevel::Veteran
        );
        assert_eq!(
            merged_recrew_veterancy(VeterancyLevel::Heroic, VeterancyLevel::Veteran),
            VeterancyLevel::Heroic
        );
        assert_eq!(pilot_default_veterancy(), VeterancyLevel::Veteran);
    }

    #[test]
    fn honesty_flags() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_pilot_ok());
        reg.record_recrew(true);
        assert!(reg.honesty_recrew_ok());
        assert!(reg.honesty_veterancy_transfer_ok());
        assert!(reg.honesty_pilot_ok());
        assert_eq!(reg.recrews, 1);
        assert_eq!(reg.veterancy_transfers, 1);
    }

    #[test]
    fn eject_pilot_template_matrix() {
        assert!(is_eject_pilot_eligible_template("AmericaVehicleHumvee"));
        assert!(is_eject_pilot_eligible_template("AmericaVehicleTomahawk"));
        assert!(is_eject_pilot_eligible_template("AmericaTankCrusader"));
        assert!(is_eject_pilot_eligible_template("AmericaTankPaladin"));
        assert!(is_eject_pilot_eligible_template("AmericaTankAvenger"));
        assert!(is_eject_pilot_eligible_template("AmericaTankMicrowave"));
        assert!(is_eject_pilot_eligible_template("SupW_AmericaTankCrusader"));
        assert!(is_eject_pilot_eligible_template("Lazr_AmericaTankPaladin"));
        assert!(is_eject_pilot_eligible_template("AirF_AmericaVehicleHumvee"));
        assert!(is_eject_pilot_eligible_template("TestEjectVehicle"));
        assert!(!is_eject_pilot_eligible_template("AmericaVehicleDozer"));
        assert!(!is_eject_pilot_eligible_template("AmericaVehicleScoutDrone"));
        assert!(!is_eject_pilot_eligible_template("AmericaInfantryPilot"));
        assert!(!is_eject_pilot_eligible_template("AmericaJetRaptor"));
        assert!(!is_eject_pilot_eligible_template("TestTank"));
        assert!(!is_eject_pilot_eligible_template("GLATankScorpion"));
    }

    #[test]
    fn eject_on_death_gate() {
        assert!(can_eject_pilot_on_death(
            true, false, false, true, false, true
        ));
        assert!(!can_eject_pilot_on_death(
            true, true, false, true, false, true
        )); // unmanned
        assert!(!can_eject_pilot_on_death(
            true, false, true, true, false, true
        )); // construction
        assert!(!can_eject_pilot_on_death(
            false, false, false, true, false, true
        )); // ineligible
        assert!(!can_eject_pilot_on_death(
            true, false, false, false, false, true
        )); // not vehicle
        assert!(!can_eject_pilot_on_death(
            true, false, false, true, true, true
        )); // aircraft
        assert!(!can_eject_pilot_on_death(
            true, false, false, true, false, false
        )); // REGULAR / Rookie blocked
    }

    #[test]
    fn eject_veterancy_levels_all_minus_regular() {
        assert!(!meets_eject_pilot_veterancy_gate(VeterancyLevel::Rookie));
        assert!(meets_eject_pilot_veterancy_gate(VeterancyLevel::Veteran));
        assert!(meets_eject_pilot_veterancy_gate(VeterancyLevel::Elite));
        assert!(meets_eject_pilot_veterancy_gate(VeterancyLevel::Heroic));
    }

    #[test]
    fn pilot_find_vehicle_gates() {
        assert!(pilot_find_vehicle_scan_eligible(
            true, true, true, true
        ));
        assert!(!pilot_find_vehicle_scan_eligible(
            true, true, true, false
        )); // human
        assert!(!pilot_find_vehicle_scan_eligible(
            true, true, false, true
        )); // not idle
        assert!(!pilot_find_vehicle_scan_eligible(
            false, true, true, true
        )); // not pilot

        assert!(vehicle_meets_pilot_find_min_health(50.0, 100.0, 0.5));
        assert!(vehicle_meets_pilot_find_min_health(100.0, 100.0, 0.5));
        assert!(!vehicle_meets_pilot_find_min_health(49.0, 100.0, 0.5));
        assert!(!vehicle_meets_pilot_find_min_health(10.0, 0.0, 0.5));

        assert!(is_pilot_find_vehicle_target(true, true, true));
        assert!(!is_pilot_find_vehicle_target(true, false, true)); // low HP
        assert!(!is_pilot_find_vehicle_target(false, true, true)); // not recrewable
        assert!(!is_pilot_find_vehicle_target(true, true, false)); // out of range

        assert!(pilot_find_vehicle_scan_frame(0));
        assert!(pilot_find_vehicle_scan_frame(30));
        assert!(!pilot_find_vehicle_scan_frame(1));
        assert_eq!(PILOT_FIND_VEHICLE_SCAN_FRAMES, 30);
        assert!((PILOT_FIND_VEHICLE_SCAN_RANGE - 300.0).abs() < 0.001);
        assert!((PILOT_FIND_VEHICLE_MIN_HEALTH - 0.5).abs() < 0.001);
    }

    #[test]
    fn find_vehicle_and_vet_gate_honesty() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_find_vehicle_ok());
        assert!(!reg.honesty_eject_veterancy_gate_ok());
        reg.record_find_vehicle_order();
        assert!(reg.honesty_find_vehicle_ok());
        assert!(reg.honesty_pilot_ok());
        reg.record_eject_veterancy_block();
        assert!(reg.honesty_eject_veterancy_gate_ok());
        assert_eq!(reg.find_vehicle_orders, 1);
        assert_eq!(reg.eject_veterancy_blocks, 1);
    }

    #[test]
    fn pilot_base_center_fallback_gate() {
        assert!(should_pilot_base_center_fallback(false, false, true));
        assert!(!should_pilot_base_center_fallback(true, false, true)); // found vehicle
        assert!(!should_pilot_base_center_fallback(false, true, true)); // already moved
        assert!(!should_pilot_base_center_fallback(false, false, false)); // no base
    }

    #[test]
    fn auto_find_healing_gates() {
        assert!(auto_find_healing_scan_eligible(true, true, true, true));
        assert!(!auto_find_healing_scan_eligible(true, true, true, false)); // human
        assert!(!auto_find_healing_scan_eligible(true, true, false, true)); // not idle
        assert!(!auto_find_healing_scan_eligible(false, true, true, true)); // no module

        assert!(is_auto_find_healing_template("AmericaInfantryPilot"));
        assert!(is_auto_find_healing_template("AmericaInfantryRanger"));
        assert!(is_auto_find_healing_template("AmericaInfantryMissileDefender"));
        assert!(is_auto_find_healing_template("AmericaInfantryPathfinder"));
        assert!(is_auto_find_healing_template("AmericaInfantryColonelBurton"));
        assert!(is_auto_find_healing_template("AirF_AmericaInfantryRanger"));
        assert!(is_auto_find_healing_template("TestRanger"));
        assert!(!is_auto_find_healing_template("GLAInfantryRebel"));
        assert!(!is_auto_find_healing_template("ChinaInfantryRedguard"));
        assert!(!is_auto_find_healing_template("RangerFlashBangGrenadeWeapon"));
        assert!(!is_auto_find_healing_template("Upgrade_AmericaRangerFlashBangGrenade"));

        assert!(health_needs_auto_find_healing(85.0, 100.0, 0.85));
        assert!(health_needs_auto_find_healing(50.0, 100.0, 0.85));
        assert!(!health_needs_auto_find_healing(86.0, 100.0, 0.85));
        assert!(!health_needs_auto_find_healing(100.0, 100.0, 0.85));
        assert!(!health_needs_auto_find_healing(10.0, 0.0, 0.85));

        assert!(is_auto_find_healing_target(true, true, true));
        assert!(!is_auto_find_healing_target(false, true, true));
        assert!(!is_auto_find_healing_target(true, false, true));
        assert!(!is_auto_find_healing_target(true, true, false));

        assert!(auto_find_healing_scan_frame(0));
        assert!(auto_find_healing_scan_frame(30));
        assert!(!auto_find_healing_scan_frame(1));
        assert_eq!(AUTO_FIND_HEALING_SCAN_FRAMES, 30);
        assert!((AUTO_FIND_HEALING_SCAN_RANGE - 300.0).abs() < 0.001);
        assert!((AUTO_FIND_HEALING_NEVER_HEAL - 0.85).abs() < 0.001);
        assert!((AUTO_FIND_HEALING_ALWAYS_HEAL - 0.25).abs() < 0.001);
    }

    #[test]
    fn air_eject_parachute_gates() {
        let thr = significantly_above_terrain_threshold();
        assert!((thr - 576.0).abs() < 0.001);
        assert!(!is_significantly_above_terrain(0.0));
        assert!(!is_significantly_above_terrain(thr));
        assert!(is_significantly_above_terrain(thr + 1.0));
        assert!(uses_air_eject_ocl(thr + 1.0, false));
        assert!(uses_air_eject_ocl(0.0, true)); // airborne_target residual
        assert!(!uses_air_eject_ocl(0.0, false));
        assert!((air_eject_spawn_height(0.0) - (thr + 1.0)).abs() < 0.001);
        assert!((air_eject_spawn_height(700.0) - 700.0).abs() < 0.001);

        let (h1, landed1) = tick_parachute_height(thr + 1.0, 0.0);
        assert!(!landed1);
        assert!(h1 < thr + 1.0);
        // Sink until land.
        let mut h = thr + 1.0;
        let mut landed = false;
        for _ in 0..100 {
            let (nh, l) = tick_parachute_height(h, 0.0);
            h = nh;
            if l {
                landed = true;
                break;
            }
        }
        assert!(landed);
        assert!((h - 0.0).abs() < 0.01);
        let (ground, land_ground) = tick_parachute_height(0.0, 0.0);
        assert!(land_ground);
        assert!((ground - 0.0).abs() < 0.01);
    }

    #[test]
    fn base_center_and_auto_heal_honesty() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_base_center_ok());
        assert!(!reg.honesty_auto_heal_ok());
        reg.record_base_center_move();
        assert!(reg.honesty_base_center_ok());
        assert!(reg.honesty_pilot_ok());
        reg.record_auto_heal_order();
        assert!(reg.honesty_auto_heal_ok());
        assert_eq!(reg.base_center_moves, 1);
        assert_eq!(reg.auto_heal_orders, 1);
        reg.record_air_ejection();
        assert!(reg.honesty_air_eject_ok());
        assert_eq!(reg.air_ejections, 1);
        reg.record_parachute_land();
        assert!(reg.honesty_parachute_land_ok());
        assert_eq!(reg.parachute_lands, 1);
        reg.record_infantry_auto_heal_order();
        assert!(reg.honesty_infantry_auto_heal_ok());
        assert_eq!(reg.infantry_auto_heal_orders, 1);
        assert_eq!(reg.auto_heal_orders, 2);
    }

    #[test]
    fn eject_honesty_alone_is_pilot_ok() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_pilot_ok());
        reg.record_ejection();
        assert!(reg.honesty_eject_ok());
        assert!(reg.honesty_pilot_ok());
        assert_eq!(reg.ejections, 1);
    }

    #[test]
    fn invulnerable_time_frames_match_retail() {
        assert_eq!(EJECT_PILOT_INVULNERABLE_MS, 2000);
        assert_eq!(EJECT_PILOT_INVULNERABLE_FRAMES, 60);
        assert_eq!(eject_pilot_invulnerable_frames_from_ms(2000), 60);
        assert_eq!(eject_pilot_invulnerable_until_frame(10), 70);
        assert_eq!(eject_pilot_invulnerable_frames_from_ms(0), 0);
    }

    #[test]
    fn invulnerable_honesty_flags() {
        let mut reg = HostUsaPilotRegistry::new();
        assert!(!reg.honesty_invulnerable_ok());
        reg.record_invulnerable_grant();
        assert!(reg.honesty_invulnerable_ok());
        assert!(!reg.honesty_invulnerable_block_ok());
        reg.record_invulnerable_block();
        assert!(reg.honesty_invulnerable_block_ok());
        assert_eq!(reg.invulnerable_grants, 1);
        assert_eq!(reg.invulnerable_blocks, 1);
    }
}
