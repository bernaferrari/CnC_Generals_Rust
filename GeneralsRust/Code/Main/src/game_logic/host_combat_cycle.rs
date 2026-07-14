//! Host GLA Combat Cycle / Combat Bike residual (rider weapon switch).
//!
//! Residual slice (playability):
//! - `RiderChangeContain` capacity: Slots = **1**, infantry only residual.
//! - Retail WeaponSet Conditions=None has PRIMARY **NONE** until a rider is present.
//! - InitialPayload residual: default `GLAInfantryRebel` → WEAPON_RIDER2
//!   (`GLARebelBikerMachineGun`).
//! - Rider weapon switch residual (visible bike fires rider weapon):
//!   - Rider2 Rebel: `GLARebelBikerMachineGun` (8 dmg / 150 range / 100ms / clip 6)
//!   - Rider3 TunnelDefender / RPG: `TunnelDefenderBikerRocketWeapon`
//!     (40 dmg / 175 range / min 5 / AA residual)
//!   - Rider4 Jarmen Kell: `GLABikerKellSniperRifle` (180 dmg / 225 range / 750ms)
//!   - Rider5 Terrorist: suicide residual (binds short-range self-detonation residual;
//!     real area damage via `SuicideBikeBomb` residual 700/20 + 100/50)
//!   - Rider1 Worker / Rider6 Hijacker / Rider7 Saboteur: no combat weapon residual
//! - Empty bike residual: primary weapon cleared (PRIMARY NONE).
//!
//! Fail-closed honesty:
//! - Not full RiderChangeContain model condition / STATUS_RIDER* death OCL matrix
//! - Not full ScuttleDelay 1500ms TOPPLED / UseRiderStealth matrix
//! - Not Jarmen Kell secondary pilot-sniper AutoChooseSources=NONE matrix
//! - Not network rider / weapon-set replication (network deferred)

use super::Weapon;

/// Retail default (Rebel) biker MG.
pub const REBEL_BIKER_MG: &str = "GLARebelBikerMachineGun";
/// Retail RPG trooper biker rocket.
pub const TUNNEL_DEFENDER_BIKER_ROCKET: &str = "TunnelDefenderBikerRocketWeapon";
/// Retail Jarmen Kell biker sniper.
pub const BIKER_KELL_SNIPER: &str = "GLABikerKellSniperRifle";
/// Retail terrorist self-kill trigger residual name.
pub const TERRORIST_SUICIDE_WEAPON: &str = "TerroristSuicideWeapon";
/// Retail death weapon for terrorist bike residual area.
pub const SUICIDE_BIKE_BOMB: &str = "SuicideBikeBomb";

/// C++ RiderChangeContain Slots = 1.
pub const COMBAT_CYCLE_TRANSPORT_SLOTS: usize = 1;

/// Rebel biker MG residual.
pub const REBEL_MG_DAMAGE: f32 = 8.0;
pub const REBEL_MG_RANGE: f32 = 150.0;
/// 100ms → 3 frames @ 30 FPS.
pub const REBEL_MG_DELAY_FRAMES: u32 = 3;
pub const REBEL_MG_CLIP: u32 = 6;

/// Tunnel defender biker rocket residual.
pub const RPG_DAMAGE: f32 = 40.0;
pub const RPG_RANGE: f32 = 175.0;
pub const RPG_MIN_RANGE: f32 = 5.0;
pub const RPG_SPLASH: f32 = 5.0;
/// 1000ms → 30 frames @ 30 FPS.
pub const RPG_DELAY_FRAMES: u32 = 30;

/// Jarmen Kell biker sniper residual.
pub const KELL_DAMAGE: f32 = 180.0;
pub const KELL_RANGE: f32 = 225.0;
/// 750ms → 23 frames @ 30 FPS (ceil 22.5).
pub const KELL_DELAY_FRAMES: u32 = 23;

/// SuicideBikeBomb residual area.
pub const SUICIDE_PRIMARY_DAMAGE: f32 = 700.0;
pub const SUICIDE_PRIMARY_RADIUS: f32 = 20.0;
pub const SUICIDE_SECONDARY_DAMAGE: f32 = 100.0;
pub const SUICIDE_SECONDARY_RADIUS: f32 = 50.0;
/// Terrorist residual attack range (must get close; retail AttackRange 1–5).
pub const SUICIDE_ATTACK_RANGE: f32 = 5.0;

/// Residual fire audio.
pub const COMBAT_CYCLE_REBEL_AUDIO: &str = "CombatCycleRebelWeapon";
pub const COMBAT_CYCLE_RPG_AUDIO: &str = "RPGTrooperWeapon";
pub const COMBAT_CYCLE_KELL_AUDIO: &str = "JarmenKellWeapon";
pub const COMBAT_CYCLE_SUICIDE_AUDIO: &str = "CarBomberDie";

/// Rider residual class mapped from RiderChangeContain Rider1..Rider7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CombatCycleRider {
    /// No rider / empty bike → PRIMARY NONE.
    #[default]
    None = 0,
    /// Rider1 Worker — no combat weapon residual.
    Worker = 1,
    /// Rider2 Rebel — machine gun residual.
    Rebel = 2,
    /// Rider3 TunnelDefender — RPG residual.
    TunnelDefender = 3,
    /// Rider4 Jarmen Kell — sniper residual.
    JarmenKell = 4,
    /// Rider5 Terrorist — suicide residual.
    Terrorist = 5,
    /// Rider6 Hijacker — no combat weapon residual.
    Hijacker = 6,
    /// Rider7 Saboteur — no combat weapon residual.
    Saboteur = 7,
}

impl CombatCycleRider {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Worker,
            2 => Self::Rebel,
            3 => Self::TunnelDefender,
            4 => Self::JarmenKell,
            5 => Self::Terrorist,
            6 => Self::Hijacker,
            7 => Self::Saboteur,
            _ => Self::None,
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Worker => 1,
            Self::Rebel => 2,
            Self::TunnelDefender => 3,
            Self::JarmenKell => 4,
            Self::Terrorist => 5,
            Self::Hijacker => 6,
            Self::Saboteur => 7,
        }
    }

    /// Whether residual has a combat primary weapon (can attack).
    pub fn has_combat_weapon(self) -> bool {
        matches!(
            self,
            Self::Rebel | Self::TunnelDefender | Self::JarmenKell | Self::Terrorist
        )
    }
}

/// Whether template is a residual Combat Cycle / Combat Bike vehicle.
///
/// Fail-closed: name residual (not full RiderChange / W3D rider anim matrix).
/// Excludes weapons / projectiles / command tokens.
pub fn is_combat_cycle_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("missile")
        || n.contains("projectile")
        || n.contains("shell")
        || n.starts_with("upgrade")
        || n.contains("command")
        || n.contains("voice")
        || n.contains("fx_")
        || n.contains("ocl_")
    {
        return false;
    }
    n.contains("combatbike")
        || n.contains("combat_bike")
        || n.contains("combatcycle")
        || n.contains("combat_cycle")
        || n == "gla_combatbike"
        || n == "testcombatbike"
        || n == "testcombatcycle"
}

/// Infer rider residual class from infantry template name.
///
/// Fail-closed: name residual mapping (not full RiderChangeContain list parity
/// for every general variant).
pub fn rider_from_template_name(template_name: &str) -> CombatCycleRider {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return CombatCycleRider::None;
    }
    // Order matters: more specific tokens first.
    if n.contains("jarmen") || n.contains("kell") {
        return CombatCycleRider::JarmenKell;
    }
    if n.contains("terrorist") {
        return CombatCycleRider::Terrorist;
    }
    if n.contains("tunneldefender")
        || n.contains("tunnel_defender")
        || n.contains("rpg")
        || (n.contains("defender") && n.contains("tunnel"))
    {
        return CombatCycleRider::TunnelDefender;
    }
    if n.contains("hijacker") {
        return CombatCycleRider::Hijacker;
    }
    if n.contains("saboteur") {
        return CombatCycleRider::Saboteur;
    }
    if n.contains("worker") {
        return CombatCycleRider::Worker;
    }
    if n.contains("rebel") {
        return CombatCycleRider::Rebel;
    }
    // Unknown infantry residual: treat as no combat weapon (fail-closed).
    CombatCycleRider::None
}

/// Default initial rider residual (retail InitialPayload = GLAInfantryRebel 1).
pub fn default_spawn_rider() -> CombatCycleRider {
    CombatCycleRider::Rebel
}

/// Default spawn for CombatBikeRocket residual (tunnel defender payload).
pub fn default_spawn_rider_for_template(template_name: &str) -> CombatCycleRider {
    let n = template_name.to_ascii_lowercase();
    if n.contains("terrorist") {
        CombatCycleRider::Terrorist
    } else if n.contains("rocket") {
        CombatCycleRider::TunnelDefender
    } else {
        CombatCycleRider::Rebel
    }
}

/// Weapon template name for rider residual (None when no combat weapon).
pub fn combat_cycle_weapon_name_for_rider(rider: CombatCycleRider) -> Option<&'static str> {
    match rider {
        CombatCycleRider::Rebel => Some(REBEL_BIKER_MG),
        CombatCycleRider::TunnelDefender => Some(TUNNEL_DEFENDER_BIKER_ROCKET),
        CombatCycleRider::JarmenKell => Some(BIKER_KELL_SNIPER),
        CombatCycleRider::Terrorist => Some(TERRORIST_SUICIDE_WEAPON),
        CombatCycleRider::None
        | CombatCycleRider::Worker
        | CombatCycleRider::Hijacker
        | CombatCycleRider::Saboteur => None,
    }
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// Build residual Weapon for a rider class (None when empty / non-combat rider).
pub fn combat_cycle_weapon_for_rider(rider: CombatCycleRider) -> Option<Weapon> {
    match rider {
        CombatCycleRider::Rebel => Some(Weapon {
            damage: REBEL_MG_DAMAGE,
            range: REBEL_MG_RANGE,
            min_range: 0.0,
            reload_time: delay_frames_to_reload_secs(REBEL_MG_DELAY_FRAMES),
            last_fire_time: 0.0,
            ammo: Some(REBEL_MG_CLIP),
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 999_999.0,
            pre_attack_delay: 0.0,
        }),
        CombatCycleRider::TunnelDefender => Some(Weapon {
            damage: RPG_DAMAGE,
            range: RPG_RANGE,
            min_range: RPG_MIN_RANGE,
            reload_time: delay_frames_to_reload_secs(RPG_DELAY_FRAMES),
            last_fire_time: 0.0,
            ammo: None,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: 600.0,
            pre_attack_delay: 0.0,
        }),
        CombatCycleRider::JarmenKell => Some(Weapon {
            damage: KELL_DAMAGE,
            range: KELL_RANGE,
            min_range: 0.0,
            reload_time: delay_frames_to_reload_secs(KELL_DELAY_FRAMES),
            last_fire_time: 0.0,
            ammo: None,
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 999_999.0,
            pre_attack_delay: 0.0,
        }),
        CombatCycleRider::Terrorist => Some(Weapon {
            // Host residual: use suicide primary as attack damage flag;
            // real area is applied via SuicideBikeBomb residual path.
            damage: SUICIDE_PRIMARY_DAMAGE,
            range: SUICIDE_ATTACK_RANGE,
            min_range: 0.0,
            reload_time: 0.05,
            last_fire_time: 0.0,
            ammo: Some(1),
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 999_999.0,
            pre_attack_delay: 0.0,
        }),
        CombatCycleRider::None
        | CombatCycleRider::Worker
        | CombatCycleRider::Hijacker
        | CombatCycleRider::Saboteur => None,
    }
}

/// Residual fire audio for rider class.
pub fn combat_cycle_audio_for_rider(rider: CombatCycleRider) -> Option<&'static str> {
    match rider {
        CombatCycleRider::Rebel => Some(COMBAT_CYCLE_REBEL_AUDIO),
        CombatCycleRider::TunnelDefender => Some(COMBAT_CYCLE_RPG_AUDIO),
        CombatCycleRider::JarmenKell => Some(COMBAT_CYCLE_KELL_AUDIO),
        CombatCycleRider::Terrorist => Some(COMBAT_CYCLE_SUICIDE_AUDIO),
        _ => None,
    }
}

/// Whether residual fire should apply Combat Cycle residual path.
pub fn should_apply_combat_cycle_residual(is_combat_cycle: bool) -> bool {
    is_combat_cycle
}

/// Whether residual fire is suicide detonation residual.
pub fn is_terrorist_suicide_rider(rider: CombatCycleRider) -> bool {
    matches!(rider, CombatCycleRider::Terrorist)
}

/// RPG splash residual damage at distance from impact.
pub fn rpg_splash_damage_at(is_intended_target: bool, distance_from_impact: f32) -> f32 {
    if is_intended_target {
        return RPG_DAMAGE;
    }
    if distance_from_impact <= RPG_SPLASH {
        RPG_DAMAGE
    } else {
        0.0
    }
}

/// SuicideBikeBomb residual damage at distance from bike.
pub fn suicide_bike_damage_at(distance_from_bike: f32) -> f32 {
    if distance_from_bike <= SUICIDE_PRIMARY_RADIUS {
        SUICIDE_PRIMARY_DAMAGE
    } else if distance_from_bike <= SUICIDE_SECONDARY_RADIUS {
        SUICIDE_SECONDARY_DAMAGE
    } else {
        0.0
    }
}

/// Legal residual target for combat cycle fire.
pub fn is_legal_combat_cycle_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind
}

// --- Wave 69 residual honesty peels (retail rider weapons / body) ---

/// Logic frames per second residual.
pub const COMBAT_CYCLE_LOGIC_FPS: f32 = 30.0;

/// Convert residual msec → logic frames @ 30 FPS.
pub fn combat_cycle_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * COMBAT_CYCLE_LOGIC_FPS / 1000.0).round() as u32
}

/// Retail rebel biker MG DelayBetweenShots residual (msec).
pub const REBEL_MG_DELAY_MS: u32 = 100;
/// Retail rebel ClipReloadTime residual (msec).
pub const REBEL_MG_CLIP_RELOAD_MS: u32 = 700;
/// Retail rebel DamageType residual.
pub const REBEL_MG_DAMAGE_TYPE: &str = "SMALL_ARMS";
/// Retail RPG DelayBetweenShots residual (msec).
pub const RPG_DELAY_MS: u32 = 1_000;
/// Retail RPG DamageType residual.
pub const RPG_DAMAGE_TYPE: &str = "INFANTRY_MISSILE";
/// Retail Kell DelayBetweenShots residual (msec).
pub const KELL_DELAY_MS: u32 = 750;
/// Retail Kell DamageType residual.
pub const KELL_DAMAGE_TYPE: &str = "SNIPER";
/// Retail SuicideBikeBomb DamageType residual.
pub const SUICIDE_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail SuicideBikeBomb DeathType residual.
pub const SUICIDE_DEATH_TYPE: &str = "SUICIDED";

/// Retail Combat Bike body residual (GLAVehicleCombatBike).
pub const COMBAT_CYCLE_MAX_HEALTH: f32 = 100.0;
pub const COMBAT_CYCLE_BUILD_COST: u32 = 500;
pub const COMBAT_CYCLE_BUILD_TIME_SEC: f32 = 4.0;
pub const COMBAT_CYCLE_BUILD_TIME_FRAMES: u32 = 120;
pub const COMBAT_CYCLE_VISION_RANGE: f32 = 180.0;
pub const COMBAT_CYCLE_SHROUD_CLEARING_RANGE: f32 = 300.0;
pub const COMBAT_CYCLE_LOCOMOTOR_SPEED: f32 = 120.0;
pub const COMBAT_CYCLE_LOCOMOTOR_SPEED_DAMAGED: f32 = 90.0;
/// Retail InitialPayload residual name.
pub const COMBAT_CYCLE_INITIAL_PAYLOAD: &str = "GLAInfantryRebel";

/// Wave 69 residual honesty: rider weapon residual peel.
pub fn honesty_combat_cycle_weapon_residual_ok() -> bool {
    REBEL_BIKER_MG == "GLARebelBikerMachineGun"
        && TUNNEL_DEFENDER_BIKER_ROCKET == "TunnelDefenderBikerRocketWeapon"
        && BIKER_KELL_SNIPER == "GLABikerKellSniperRifle"
        && SUICIDE_BIKE_BOMB == "SuicideBikeBomb"
        && (REBEL_MG_DAMAGE - 8.0).abs() < 0.01
        && (REBEL_MG_RANGE - 150.0).abs() < 0.01
        && REBEL_MG_DELAY_MS == 100
        && REBEL_MG_DELAY_FRAMES == combat_cycle_ms_to_frames(REBEL_MG_DELAY_MS)
        && REBEL_MG_DELAY_FRAMES == 3
        && REBEL_MG_CLIP == 6
        && REBEL_MG_CLIP_RELOAD_MS == 700
        && REBEL_MG_DAMAGE_TYPE == "SMALL_ARMS"
        && (RPG_DAMAGE - 40.0).abs() < 0.01
        && (RPG_RANGE - 175.0).abs() < 0.01
        && (RPG_MIN_RANGE - 5.0).abs() < 0.01
        && (RPG_SPLASH - 5.0).abs() < 0.01
        && RPG_DELAY_MS == 1_000
        && RPG_DELAY_FRAMES == combat_cycle_ms_to_frames(RPG_DELAY_MS)
        && RPG_DELAY_FRAMES == 30
        && RPG_DAMAGE_TYPE == "INFANTRY_MISSILE"
        && (KELL_DAMAGE - 180.0).abs() < 0.01
        && (KELL_RANGE - 225.0).abs() < 0.01
        && KELL_DELAY_MS == 750
        && KELL_DELAY_FRAMES == combat_cycle_ms_to_frames(KELL_DELAY_MS)
        && KELL_DELAY_FRAMES == 23
        && KELL_DAMAGE_TYPE == "SNIPER"
        && (SUICIDE_PRIMARY_DAMAGE - 700.0).abs() < 0.01
        && (SUICIDE_PRIMARY_RADIUS - 20.0).abs() < 0.01
        && (SUICIDE_SECONDARY_DAMAGE - 100.0).abs() < 0.01
        && (SUICIDE_SECONDARY_RADIUS - 50.0).abs() < 0.01
        && (SUICIDE_ATTACK_RANGE - 5.0).abs() < 0.01
        && SUICIDE_DAMAGE_TYPE == "EXPLOSION"
        && SUICIDE_DEATH_TYPE == "SUICIDED"
        && combat_cycle_weapon_for_rider(CombatCycleRider::Rebel).is_some()
        && combat_cycle_weapon_for_rider(CombatCycleRider::Worker).is_none()
        && (suicide_bike_damage_at(10.0) - 700.0).abs() < 0.01
}

/// Wave 69 residual honesty: combat cycle body residual peel.
pub fn honesty_combat_cycle_body_residual_ok() -> bool {
    (COMBAT_CYCLE_MAX_HEALTH - 100.0).abs() < 0.01
        && COMBAT_CYCLE_BUILD_COST == 500
        && (COMBAT_CYCLE_BUILD_TIME_SEC - 4.0).abs() < 0.01
        && COMBAT_CYCLE_BUILD_TIME_FRAMES
            == ((COMBAT_CYCLE_BUILD_TIME_SEC * COMBAT_CYCLE_LOGIC_FPS).round() as u32)
        && COMBAT_CYCLE_BUILD_TIME_FRAMES == 120
        && (COMBAT_CYCLE_VISION_RANGE - 180.0).abs() < 0.01
        && (COMBAT_CYCLE_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && COMBAT_CYCLE_TRANSPORT_SLOTS == 1
        && (COMBAT_CYCLE_LOCOMOTOR_SPEED - 120.0).abs() < 0.01
        && (COMBAT_CYCLE_LOCOMOTOR_SPEED_DAMAGED - 90.0).abs() < 0.01
        && COMBAT_CYCLE_INITIAL_PAYLOAD == "GLAInfantryRebel"
        && default_spawn_rider() == CombatCycleRider::Rebel
        && is_combat_cycle_template("GLAVehicleCombatBike")
        && !is_combat_cycle_template("GLARebelBikerMachineGun")
}

/// Combined Wave 69 Combat Cycle residual honesty pack.
pub fn honesty_combat_cycle_residual_pack_ok() -> bool {
    honesty_combat_cycle_weapon_residual_ok() && honesty_combat_cycle_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combat_cycle_name_matrix() {
        assert!(is_combat_cycle_template("GLAVehicleCombatBike"));
        assert!(is_combat_cycle_template("GLAVehicleCombatBikeRocket"));
        assert!(is_combat_cycle_template("GLAVehicleCombatBikeTerrorist"));
        assert!(is_combat_cycle_template("Chem_GLAVehicleCombatBike"));
        assert!(is_combat_cycle_template("Demo_GLAVehicleCombatBike"));
        assert!(is_combat_cycle_template("Slth_GLAVehicleCombatBike"));
        assert!(is_combat_cycle_template("TestCombatBike"));
        assert!(is_combat_cycle_template("TestCombatCycle"));
        assert!(!is_combat_cycle_template("GLARebelBikerMachineGun"));
        assert!(!is_combat_cycle_template("TunnelDefenderBikerRocketWeapon"));
        assert!(!is_combat_cycle_template("GLAVehicleRocketBuggy"));
        assert!(!is_combat_cycle_template("USA_Ranger"));
        assert!(!is_combat_cycle_template(
            "Command_ConstructGLAVehicleCombatBike"
        ));
    }

    #[test]
    fn rider_template_mapping() {
        assert_eq!(
            rider_from_template_name("GLAInfantryRebel"),
            CombatCycleRider::Rebel
        );
        assert_eq!(
            rider_from_template_name("GLAInfantryTunnelDefender"),
            CombatCycleRider::TunnelDefender
        );
        assert_eq!(
            rider_from_template_name("GLAInfantryJarmenKell"),
            CombatCycleRider::JarmenKell
        );
        assert_eq!(
            rider_from_template_name("GLAInfantryTerrorist"),
            CombatCycleRider::Terrorist
        );
        assert_eq!(
            rider_from_template_name("GLAInfantryWorker"),
            CombatCycleRider::Worker
        );
        assert_eq!(
            rider_from_template_name("GLAInfantryHijacker"),
            CombatCycleRider::Hijacker
        );
        assert_eq!(
            rider_from_template_name("GLAInfantrySaboteur"),
            CombatCycleRider::Saboteur
        );
        assert_eq!(
            rider_from_template_name("UnknownUnit"),
            CombatCycleRider::None
        );
        assert_eq!(default_spawn_rider(), CombatCycleRider::Rebel);
        assert_eq!(
            default_spawn_rider_for_template("GLAVehicleCombatBikeRocket"),
            CombatCycleRider::TunnelDefender
        );
        assert_eq!(
            default_spawn_rider_for_template("GLAVehicleCombatBikeTerrorist"),
            CombatCycleRider::Terrorist
        );
    }

    #[test]
    fn rider_weapons_residual() {
        assert!(combat_cycle_weapon_for_rider(CombatCycleRider::None).is_none());
        assert!(combat_cycle_weapon_for_rider(CombatCycleRider::Worker).is_none());
        let rebel = combat_cycle_weapon_for_rider(CombatCycleRider::Rebel).expect("rebel");
        assert!((rebel.damage - 8.0).abs() < 0.01);
        assert!((rebel.range - 150.0).abs() < 0.01);
        assert!(!rebel.can_target_air);
        let rpg = combat_cycle_weapon_for_rider(CombatCycleRider::TunnelDefender).expect("rpg");
        assert!((rpg.damage - 40.0).abs() < 0.01);
        assert!((rpg.min_range - 5.0).abs() < 0.01);
        assert!(rpg.can_target_air);
        let kell = combat_cycle_weapon_for_rider(CombatCycleRider::JarmenKell).expect("kell");
        assert!((kell.damage - 180.0).abs() < 0.01);
        assert!((kell.range - 225.0).abs() < 0.01);
        let terror = combat_cycle_weapon_for_rider(CombatCycleRider::Terrorist).expect("terror");
        assert!((terror.range - 5.0).abs() < 0.01);
        assert!(is_terrorist_suicide_rider(CombatCycleRider::Terrorist));
        assert!(!is_terrorist_suicide_rider(CombatCycleRider::Rebel));
        assert!((rpg_splash_damage_at(true, 0.0) - 40.0).abs() < 0.01);
        assert!((suicide_bike_damage_at(10.0) - 700.0).abs() < 0.01);
        assert!((suicide_bike_damage_at(30.0) - 100.0).abs() < 0.01);
        assert!((suicide_bike_damage_at(60.0)).abs() < 0.01);
    }

    #[test]
    fn transport_slots() {
        assert_eq!(COMBAT_CYCLE_TRANSPORT_SLOTS, 1);
    }

    #[test]
    fn combat_cycle_residual_pack_honesty_wave69() {
        assert_eq!(combat_cycle_ms_to_frames(100), 3);
        assert_eq!(combat_cycle_ms_to_frames(750), 23);
        assert_eq!(combat_cycle_ms_to_frames(1000), 30);
        assert!(honesty_combat_cycle_weapon_residual_ok());
        assert!(honesty_combat_cycle_body_residual_ok());
        assert!(honesty_combat_cycle_residual_pack_ok());
        assert_eq!(COMBAT_CYCLE_BUILD_TIME_FRAMES, 120);
        assert_eq!(REBEL_MG_DAMAGE_TYPE, "SMALL_ARMS");
        assert_eq!(SUICIDE_DEATH_TYPE, "SUICIDED");
    }
}
