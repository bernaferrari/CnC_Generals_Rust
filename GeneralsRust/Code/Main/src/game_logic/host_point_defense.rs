//! PointDefenseLaserUpdate authored module definitions and object-owned state.
//!
//! The production update resolves each module's weapon in the owning world's
//! catalog and targets its authored primary and secondary KindOf masks.
//! The retail constants below remain reference data for existing consumers.
//! C++ PointDefenseLaserUpdate.cpp:134-214 performs one immediate rescan when
//! its random post-kill/out-of-range delay is zero, without recursively firing.
//! Snapshot and beam/FX parity still require runtime verification.

/// Logic frames per second residual.
pub const PDL_LOGIC_FPS: f32 = 30.0;

/// Retail PrimaryTargetTypes residual name list (all carriers).
pub const PDL_PRIMARY_TARGET_TYPES: &[&str] = &["BALLISTIC_MISSILE", "SMALL_MISSILE"];
/// Retail Paladin SecondaryTargetTypes residual name list.
pub const PDL_SECONDARY_TARGET_TYPES: &[&str] = &["INFANTRY"];

/// Retail PaladinPointDefenseLaser AttackRange residual.
pub const PALADIN_PDL_FIRE_RANGE: f32 = 65.0;

/// Retail Paladin PointDefenseLaserUpdate ScanRange residual.
pub const PALADIN_PDL_SCAN_RANGE: f32 = 120.0;

/// Retail Paladin PointDefenseLaserUpdate ScanRate residual (msec).
pub const PALADIN_PDL_SCAN_RATE_MS: u32 = 500;
/// ScanRate 500ms → 15 frames @ 30 FPS.
pub const PALADIN_PDL_SCAN_RATE_FRAMES: u32 = 15;

/// Retail Paladin PredictTargetVelocityFactor residual.
pub const PALADIN_PDL_VELOCITY_PREDICT: f32 = 3.0;

/// Retail PaladinPointDefenseLaser PrimaryDamage residual.
pub const PALADIN_PDL_DAMAGE: f32 = 100.0;

/// Retail PaladinPointDefenseLaser DelayBetweenShots residual (msec).
pub const PALADIN_PDL_DELAY_MS: u32 = 1000;
/// Retail PaladinPointDefenseLaser DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const PALADIN_PDL_DELAY_FRAMES: u32 = 30;

/// Retail AvengerPointDefenseLaser AttackRange residual.
pub const AVENGER_PDL_FIRE_RANGE: f32 = 100.0;

/// Retail Avenger PointDefenseLaserUpdate ScanRange residual (both lasers).
pub const AVENGER_PDL_SCAN_RANGE: f32 = 200.0;

/// Retail Avenger laser-one ScanRate residual (msec; 0 = every frame).
pub const AVENGER_PDL_SCAN_RATE_LASER_ONE_MS: u32 = 0;
/// Retail Avenger laser-two ScanRate residual (msec).
pub const AVENGER_PDL_SCAN_RATE_LASER_TWO_MS: u32 = 100;
/// Residual host scan cadence: min non-zero stream = 100ms → 3 frames.
pub const AVENGER_PDL_SCAN_RATE_FRAMES: u32 = 3;

/// Retail Avenger PredictTargetVelocityFactor residual.
pub const AVENGER_PDL_VELOCITY_PREDICT: f32 = 1.0;

/// Retail Avenger dual-laser DelayBetweenShots residual (msec each).
/// Each leftover PointDefenseLaserUpdate module has its own 500ms fire clock.
pub const AVENGER_PDL_DELAY_MS: u32 = 500;
/// Retail Avenger DelayBetweenShots 500ms → 15 frames per independent module.
pub const AVENGER_PDL_DELAY_FRAMES: u32 = 15;

/// Retail AvengerPointDefenseLaser PrimaryDamage residual.
pub const AVENGER_PDL_DAMAGE: f32 = 100.0;

/// Retail AirF_RaptorPointDefenseLaser / AirF_PointDefenseLaser AttackRange residual.
pub const KING_RAPTOR_PDL_FIRE_RANGE: f32 = 65.0;

/// Retail King Raptor PointDefenseLaserUpdate ScanRange residual.
pub const KING_RAPTOR_PDL_SCAN_RANGE: f32 = 200.0;

/// Retail AirF_RaptorPointDefenseLaser ScanRate residual (msec).
pub const KING_RAPTOR_PDL_SCAN_RATE_RAPTOR_MS: u32 = 10;
/// Retail AirF_PointDefenseLaser ScanRate residual (msec; 0 = every frame).
pub const KING_RAPTOR_PDL_SCAN_RATE_AIRF_MS: u32 = 0;
/// Residual host scan cadence: min non-zero stream = 10ms → 0 frames (round) /
/// host uses every-frame residual when < 1 frame (see `pdl_scan_rate_frames`).
pub const KING_RAPTOR_PDL_SCAN_RATE_FRAMES: u32 = 0;

/// Retail AirF_RaptorPointDefenseLaser PredictTargetVelocityFactor.
pub const KING_RAPTOR_PDL_VELOCITY_PREDICT_RAPTOR: f32 = 2.0;
/// Retail AirF_PointDefenseLaser PredictTargetVelocityFactor (King Raptor dual).
pub const KING_RAPTOR_PDL_VELOCITY_PREDICT_AIRF: f32 = 1.0;
/// Host residual collapses dual lasers → use higher Predict factor (**2.0**).
pub const KING_RAPTOR_PDL_VELOCITY_PREDICT: f32 = 2.0;

/// Retail AirF_RaptorPointDefenseLaser PrimaryDamage residual.
pub const KING_RAPTOR_PDL_DAMAGE: f32 = 100.0;

/// Retail dual lasers DelayBetweenShots residual (msec each).
pub const KING_RAPTOR_PDL_DELAY_MS: u32 = 250;
/// Retail dual lasers DelayBetweenShots 250ms each → residual collapse to ~4 frames.
/// (Two independent 250ms streams ≈ one shot every ~125ms @ 30 FPS.)
pub const KING_RAPTOR_PDL_DELAY_FRAMES: u32 = 4;

/// Retail AirF_AmericaVehicleChinook PointDefenseLaserUpdate ScanRange residual.
pub const COMBAT_CHINOOK_PDL_SCAN_RANGE: f32 = 250.0;

/// Retail Combat Chinook PointDefenseLaserUpdate ScanRate residual (msec).
pub const COMBAT_CHINOOK_PDL_SCAN_RATE_MS: u32 = 33;
/// ScanRate 33ms → 1 frame @ 30 FPS.
pub const COMBAT_CHINOOK_PDL_SCAN_RATE_FRAMES: u32 = 1;

/// Retail Combat Chinook PredictTargetVelocityFactor residual.
pub const COMBAT_CHINOOK_PDL_VELOCITY_PREDICT: f32 = 1.0;

/// Retail AirF_PointDefenseLaser AttackRange residual (Combat Chinook single laser).
pub const COMBAT_CHINOOK_PDL_FIRE_RANGE: f32 = 65.0;

/// Retail AirF_PointDefenseLaser PrimaryDamage residual.
pub const COMBAT_CHINOOK_PDL_DAMAGE: f32 = 100.0;

/// Retail AirF_PointDefenseLaser DelayBetweenShots residual (msec).
pub const COMBAT_CHINOOK_PDL_DELAY_MS: u32 = 250;
/// Retail AirF_PointDefenseLaser DelayBetweenShots 250ms → ~8 frames @ 30 FPS
/// (single laser; not dual-stream collapse).
pub const COMBAT_CHINOOK_PDL_DELAY_FRAMES: u32 = 8;

/// Activate / intercept audio residual (FXList WeaponFX_PaladinPointDefenseLaser).
pub const PDL_INTERCEPT_AUDIO: &str = "PaladinPointDefenseLaserPulse";
/// Retail PointDefenseLaserBeam LifetimeUpdate Min/MaxLifetime = 95 ms → 3f @ 30 FPS.
pub const PDL_LASER_BEAM_LIFETIME_MS: u32 = 95;
pub const PDL_LASER_BEAM_LIFETIME_FRAMES: u32 = (PDL_LASER_BEAM_LIFETIME_MS * 30 + 999) / 1000;
/// Default Paladin residual LaserName.
pub const PDL_LASER_BEAM_DEFAULT: &str = "PointDefenseLaserBeam";
pub const PDL_LASER_BEAM_AVENGER: &str = "AvengerPointDefenseLaserBeam";
pub const PDL_LASER_BEAM_AIRF: &str = "AirF_PointDefenseLaserBeam";
pub const PDL_LASER_BEAM_MAX_HEALTH: f32 = 1.0;

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn pdl_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / PDL_LOGIC_FPS)).round() as u32
}

/// Whether template is a residual King Raptor (Air Force General jet with PDL).
///
/// Retail: only `AirF_AmericaJetRaptor` has PointDefenseLaserUpdate — regular
/// `AmericaJetRaptor` does **not**. Fail-closed name residual.
pub fn is_king_raptor_carrier(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    // Explicit residual test / display names.
    if n == "testkingraptor" || n.contains("kingraptor") {
        return true;
    }
    // Projectile / laser beam objects are not the plane.
    if n.contains("missile")
        || n.contains("projectile")
        || n.contains("laserbeam")
        || n.contains("pointdefense")
        || n.contains("shell")
    {
        return false;
    }
    // Air Force General King Raptor: AirF_AmericaJetRaptor / AirF_*Raptor*
    // Fail-closed: regular AmericaJetRaptor (no AirF_ prefix) is NOT a PDL carrier.
    if n.starts_with("airf_") && n.contains("raptor") {
        return true;
    }
    false
}

/// Whether template is residual Air Force Combat Chinook with PointDefenseLaser.
///
/// Retail: only `AirF_AmericaVehicleChinook` has PDL — vanilla `AmericaVehicleChinook`
/// does **not**. Fail-closed name residual.
pub fn is_combat_chinook_pdl_carrier(template_name: &str) -> bool {
    crate::game_logic::host_combat_chinook::is_combat_chinook_template(template_name)
}

/// Mutable state of one authored PointDefenseLaserUpdate, owned by its object.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub(crate) struct PointDefenseState {
    pub target: Option<crate::game_logic::ObjectId>,
    pub in_range: bool,
    pub next_scan: u32,
    pub next_shot: u32,
}

/// Authored module fields; weapon data is resolved from the owning world.
#[derive(Debug, Clone)]
pub(crate) struct PointDefenseConfig {
    pub weapon: String,
    pub primary: u128,
    pub secondary: u128,
    pub scan_frames: u32,
    pub scan_range: f32,
}

pub(crate) fn authored_modules(object: &crate::game_logic::Object) -> Vec<PointDefenseConfig> {
    use game_engine::common::system::kind_of::KindOfMask;
    let mask = |value: Option<&str>| {
        value.and_then(|s| KindOfMask::parse_ini(KindOfMask::empty(), s).ok())
            .map(|m| m.bits()).unwrap_or(0)
    };
    object.thing().template.receiving_object_definition.as_ref()
        .into_iter().flat_map(|d| d.behavior_modules.iter())
        .filter(|m| m.class_name.eq_ignore_ascii_case("PointDefenseLaserUpdate"))
        .map(|m| PointDefenseConfig {
            weapon: m.attribute("WeaponTemplate").unwrap_or("").trim().to_ascii_lowercase(),
            primary: mask(m.attribute("PrimaryTargetTypes")),
            secondary: mask(m.attribute("SecondaryTargetTypes")),
            scan_frames: m.attribute("ScanRate").and_then(|s| s.trim().parse::<u32>().ok()).unwrap_or(0).saturating_mul(30).saturating_add(999) / 1000,
            scan_range: m.attribute("ScanRange").and_then(|s| s.trim().parse().ok()).unwrap_or(0.0),
        }).collect()
}

/// Whether the object authors a `PointDefenseLaserUpdate` behavior module in its
/// parsed INI definition (hq-2qedt.285). Combat Chinook PDL keys on this, not
/// the `AirF_AmericaVehicleChinook` name.
pub fn authors_point_defense_laser_update(object: &crate::game_logic::Object) -> bool {
    object
        .thing()
        .template
        .receiving_object_definition
        .as_ref()
        .is_some_and(|definition| {
            definition.behavior_modules.iter().any(|module| {
                module
                    .class_name
                    .eq_ignore_ascii_case("PointDefenseLaserUpdate")
            })
        })
}

/// A live carrier must actually author the behavior.
pub fn is_live_point_defense_carrier(object: &crate::game_logic::Object) -> bool {
    authors_point_defense_laser_update(object)
}

/// Whether template is a residual PointDefenseLaser carrier
/// (Paladin / Avenger / King Raptor / Combat Chinook).
///
/// Fail-closed: name residual (not full INI PointDefenseLaserUpdate module matrix).
pub fn is_point_defense_carrier(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    // Explicit residual test names.
    if n == "testpaladin"
        || n == "testavenger"
        || n == "testpdl"
        || n == "testkingraptor"
        || n == "testcombatchinook"
    {
        return true;
    }
    // AmericaTankPaladin / USA_Paladin / Lazr_AmericaTankPaladin / …
    if n.contains("paladin") {
        return true;
    }
    // AmericaVehicleAvenger / USA_Avenger / SupW_AmericaVehicleAvenger / …
    if n.contains("avenger") {
        return true;
    }
    // AirF_AmericaJetRaptor (King Raptor) dual residual lasers.
    if is_king_raptor_carrier(template_name) {
        return true;
    }
    // AirF_AmericaVehicleChinook single residual laser.
    if is_combat_chinook_pdl_carrier(template_name) {
        return true;
    }
    false
}

/// True when residual carrier uses Avenger dual-laser stats (range/delay).
pub fn is_avenger_carrier(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("avenger") || n == "testavenger"
}

/// Independent leftover `PointDefenseLaserUpdate` modules on this carrier.
/// Avenger authors two modules (500ms each). Paladin / Combat Chinook / King
/// Raptor live residual keep one stream (King Raptor collapse is out of scope).
pub fn pdl_module_count(template_name: &str) -> usize {
    if is_avenger_carrier(template_name) {
        2
    } else {
        1
    }
}

/// Residual fire range for a PDL carrier.
pub fn pdl_fire_range(template_name: &str) -> f32 {
    if is_combat_chinook_pdl_carrier(template_name) {
        COMBAT_CHINOOK_PDL_FIRE_RANGE
    } else if is_king_raptor_carrier(template_name) {
        KING_RAPTOR_PDL_FIRE_RANGE
    } else if is_avenger_carrier(template_name) {
        AVENGER_PDL_FIRE_RANGE
    } else {
        PALADIN_PDL_FIRE_RANGE
    }
}

/// Residual scan range (retail PointDefenseLaserUpdate ScanRange).
pub fn pdl_scan_range(template_name: &str) -> f32 {
    if is_combat_chinook_pdl_carrier(template_name) {
        COMBAT_CHINOOK_PDL_SCAN_RANGE
    } else if is_king_raptor_carrier(template_name) {
        KING_RAPTOR_PDL_SCAN_RANGE
    } else if is_avenger_carrier(template_name) {
        AVENGER_PDL_SCAN_RANGE
    } else {
        PALADIN_PDL_SCAN_RANGE
    }
}

/// Residual ScanRate in logic frames (PointDefenseLaserUpdate ScanRate msec).
///
/// 0 msec = every frame residual. Dual-laser carriers collapse to the faster
/// non-zero stream when present (Avenger laser-two **100**ms, King Raptor
/// Raptor laser **10**ms → 0 frames round → every-frame residual).
pub fn pdl_scan_rate_frames(template_name: &str) -> u32 {
    if is_combat_chinook_pdl_carrier(template_name) {
        COMBAT_CHINOOK_PDL_SCAN_RATE_FRAMES
    } else if is_king_raptor_carrier(template_name) {
        KING_RAPTOR_PDL_SCAN_RATE_FRAMES
    } else if is_avenger_carrier(template_name) {
        AVENGER_PDL_SCAN_RATE_FRAMES
    } else {
        PALADIN_PDL_SCAN_RATE_FRAMES
    }
}

/// Residual PredictTargetVelocityFactor (host intercept lead honesty).
///
/// Fail-closed: constant exposed for residual math; full C++ velocity seeker
/// not simulated in host intercept destroy path.
pub fn pdl_velocity_predict(template_name: &str) -> f32 {
    if is_combat_chinook_pdl_carrier(template_name) {
        COMBAT_CHINOOK_PDL_VELOCITY_PREDICT
    } else if is_king_raptor_carrier(template_name) {
        KING_RAPTOR_PDL_VELOCITY_PREDICT
    } else if is_avenger_carrier(template_name) {
        AVENGER_PDL_VELOCITY_PREDICT
    } else {
        PALADIN_PDL_VELOCITY_PREDICT
    }
}

/// Residual damage per intercept shot.
pub fn pdl_damage(template_name: &str) -> f32 {
    if is_combat_chinook_pdl_carrier(template_name) {
        COMBAT_CHINOOK_PDL_DAMAGE
    } else if is_king_raptor_carrier(template_name) {
        KING_RAPTOR_PDL_DAMAGE
    } else if is_avenger_carrier(template_name) {
        AVENGER_PDL_DAMAGE
    } else {
        PALADIN_PDL_DAMAGE
    }
}

/// Residual reload delay in logic frames.
pub fn pdl_delay_frames(template_name: &str) -> u32 {
    if is_combat_chinook_pdl_carrier(template_name) {
        COMBAT_CHINOOK_PDL_DELAY_FRAMES
    } else if is_king_raptor_carrier(template_name) {
        KING_RAPTOR_PDL_DELAY_FRAMES
    } else if is_avenger_carrier(template_name) {
        AVENGER_PDL_DELAY_FRAMES
    } else {
        PALADIN_PDL_DELAY_FRAMES
    }
}

/// Retail Weapon.ini LaserName residual for PDL carrier template.
pub fn pdl_laser_beam_name(template_name: &str) -> &'static str {
    if is_avenger_carrier(template_name) {
        PDL_LASER_BEAM_AVENGER
    } else if is_king_raptor_carrier(template_name) {
        PDL_LASER_BEAM_AIRF
    } else {
        PDL_LASER_BEAM_DEFAULT
    }
}

/// Whether residual carrier uses Paladin-style SecondaryTargetTypes = INFANTRY.
///
/// Retail: Paladin has Secondary INFANTRY; Avenger / King Raptor / Combat Chinook
/// list only PrimaryTargetTypes (no secondary infantry intercept residual).
pub fn pdl_has_secondary_infantry(template_name: &str) -> bool {
    if is_avenger_carrier(template_name)
        || is_king_raptor_carrier(template_name)
        || is_combat_chinook_pdl_carrier(template_name)
    {
        return false;
    }
    // Paladin family residual (default point-defense carrier).
    is_point_defense_carrier(template_name)
}

/// Wave 49 residual honesty: ScanRate / ScanRange / VelocityPredict / weapon pack.
pub fn honesty_point_defense_residual_ok() -> bool {
    PDL_PRIMARY_TARGET_TYPES == ["BALLISTIC_MISSILE", "SMALL_MISSILE"]
        && PDL_SECONDARY_TARGET_TYPES == ["INFANTRY"]
        && (PALADIN_PDL_FIRE_RANGE - 65.0).abs() < 0.01
        && (PALADIN_PDL_SCAN_RANGE - 120.0).abs() < 0.01
        && PALADIN_PDL_SCAN_RATE_MS == 500
        && PALADIN_PDL_SCAN_RATE_FRAMES == pdl_ms_to_frames(PALADIN_PDL_SCAN_RATE_MS)
        && (PALADIN_PDL_VELOCITY_PREDICT - 3.0).abs() < 0.01
        && (PALADIN_PDL_DAMAGE - 100.0).abs() < 0.01
        && PALADIN_PDL_DELAY_MS == 1000
        && PALADIN_PDL_DELAY_FRAMES == pdl_ms_to_frames(PALADIN_PDL_DELAY_MS)
        && (AVENGER_PDL_FIRE_RANGE - 100.0).abs() < 0.01
        && (AVENGER_PDL_SCAN_RANGE - 200.0).abs() < 0.01
        && AVENGER_PDL_SCAN_RATE_LASER_ONE_MS == 0
        && AVENGER_PDL_SCAN_RATE_LASER_TWO_MS == 100
        && AVENGER_PDL_SCAN_RATE_FRAMES == pdl_ms_to_frames(AVENGER_PDL_SCAN_RATE_LASER_TWO_MS)
        && (AVENGER_PDL_VELOCITY_PREDICT - 1.0).abs() < 0.01
        && AVENGER_PDL_DELAY_MS == 500
        && AVENGER_PDL_DELAY_FRAMES == pdl_ms_to_frames(AVENGER_PDL_DELAY_MS)
        && (KING_RAPTOR_PDL_SCAN_RANGE - 200.0).abs() < 0.01
        && KING_RAPTOR_PDL_SCAN_RATE_RAPTOR_MS == 10
        && KING_RAPTOR_PDL_SCAN_RATE_AIRF_MS == 0
        && (KING_RAPTOR_PDL_VELOCITY_PREDICT - 2.0).abs() < 0.01
        && (COMBAT_CHINOOK_PDL_SCAN_RANGE - 250.0).abs() < 0.01
        && COMBAT_CHINOOK_PDL_SCAN_RATE_MS == 33
        && COMBAT_CHINOOK_PDL_SCAN_RATE_FRAMES == pdl_ms_to_frames(COMBAT_CHINOOK_PDL_SCAN_RATE_MS)
        && (COMBAT_CHINOOK_PDL_VELOCITY_PREDICT - 1.0).abs() < 0.01
}
/// Combined residual honesty pack (Wave 71).
pub fn honesty_point_defense_residual_pack_ok() -> bool {
    honesty_point_defense_residual_ok()
}

/// Whether residual target is a primary intercept candidate (missile / projectile).
///
/// Retail PrimaryTargetTypes = BALLISTIC_MISSILE SMALL_MISSILE.
/// Fail-closed: KindOf::Projectile + name residual (missile/rocket/scud/tomahawk).
pub fn is_primary_intercept_target(
    is_projectile_kind: bool,
    is_alive: bool,
    same_team: bool,
    template_name: &str,
) -> bool {
    if !is_alive || same_team {
        return false;
    }
    if is_projectile_kind {
        return true;
    }
    is_missile_name_residual(template_name)
}

/// Name residual for ballistic / small missiles without KindOf::Projectile.
pub fn is_missile_name_residual(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    // Avoid false positives: MissileDefender infantry, Patriot Battery structure,
    // Stinger Site structure. Projectile names like PatriotMissileProjectile stay true.
    if n.contains("missiledefender")
        || n.contains("stingersite")
        || n.contains("stinger_site")
        || (n.contains("patriot") && !n.contains("projectile") && !n.contains("missileprojectile"))
    {
        return false;
    }
    // Pure structure batteries named "Patriot" / "PatriotBattery".
    if n == "usa_patriot" || n.ends_with("patriotbattery") || n.ends_with("patriotmissilesystem") {
        return false;
    }
    n.contains("missile")
        || n.contains("rocket")
        || n.contains("scud")
        || n.contains("tomahawk")
        || n.contains("cruise")
        || n == "testmissile"
        || n == "testprojectile"
        || (n.ends_with("shell")
            && (n.contains("nuke") || n.contains("scud") || n.contains("artillery")))
}

/// Secondary residual target (Paladin SecondaryTargetTypes = INFANTRY).
/// Lower priority than missiles; residual damages infantry in fire range.
/// Leftover `scan_closest_target` / C++ `scanClosestTarget` skip STEALTHED &&
/// !DETECTED && !DISGUISED (cloaked Ninja/Burton/Kell never lock).
pub fn is_secondary_intercept_target(
    is_infantry: bool,
    is_alive: bool,
    same_team: bool,
    under_construction: bool,
    stealthed: bool,
    detected: bool,
    disguised: bool,
) -> bool {
    if stealthed && !detected && !disguised {
        return false;
    }
    is_infantry && is_alive && !same_team && !under_construction
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_pdl_range_2d(carrier_pos: (f32, f32), target_pos: (f32, f32), range: f32) -> bool {
    let dx = carrier_pos.0 - target_pos.0;
    let dz = carrier_pos.1 - target_pos.1;
    dx * dx + dz * dz <= range * range
}

/// Priority score: lower is better. Primary missiles = 0, secondary infantry = 1.
pub fn intercept_priority(is_primary: bool, is_secondary: bool) -> Option<u8> {
    if is_primary {
        Some(0)
    } else if is_secondary {
        Some(1)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdl_carrier_name_matrix() {
        assert!(is_point_defense_carrier("USA_Paladin"));
        assert!(is_point_defense_carrier("AmericaTankPaladin"));
        assert!(is_point_defense_carrier("Lazr_AmericaTankPaladin"));
        assert!(is_point_defense_carrier("TestPaladin"));
        assert!(is_point_defense_carrier("USA_Avenger"));
        assert!(is_point_defense_carrier("AmericaVehicleAvenger"));
        assert!(is_point_defense_carrier("TestAvenger"));
        // King Raptor residual (Air Force General only).
        assert!(is_point_defense_carrier("AirF_AmericaJetRaptor"));
        assert!(is_point_defense_carrier("TestKingRaptor"));
        assert!(is_king_raptor_carrier("AirF_AmericaJetRaptor"));
        assert!(is_king_raptor_carrier("TestKingRaptor"));
        // Combat Chinook residual (Air Force General only).
        assert!(is_point_defense_carrier("AirF_AmericaVehicleChinook"));
        assert!(is_point_defense_carrier("TestCombatChinook"));
        assert!(is_combat_chinook_pdl_carrier("AirF_AmericaVehicleChinook"));
        // Regular Raptor / vanilla Chinook have no PDL modules — fail-closed.
        assert!(!is_point_defense_carrier("AmericaJetRaptor"));
        assert!(!is_king_raptor_carrier("AmericaJetRaptor"));
        assert!(!is_point_defense_carrier("AmericaVehicleChinook"));
        assert!(!is_combat_chinook_pdl_carrier("AmericaVehicleChinook"));
        assert!(!is_point_defense_carrier("USA_Ranger"));
        assert!(!is_point_defense_carrier("USA_Patriot"));
        assert!(!is_point_defense_carrier("ChinaTankBattleMaster"));
        assert!(!is_point_defense_carrier("TestTank"));
        assert!(!is_king_raptor_carrier("RaptorJetMissile"));
        assert!(!is_king_raptor_carrier("AirF_RaptorPointDefenseLaserBeam"));
    }

    #[test]
    fn avenger_vs_paladin_vs_king_raptor_stats() {
        assert!((pdl_fire_range("USA_Paladin") - PALADIN_PDL_FIRE_RANGE).abs() < 0.01);
        assert!((pdl_fire_range("USA_Avenger") - AVENGER_PDL_FIRE_RANGE).abs() < 0.01);
        assert!(
            (pdl_fire_range("AirF_AmericaJetRaptor") - KING_RAPTOR_PDL_FIRE_RANGE).abs() < 0.01
        );
        assert_eq!(pdl_delay_frames("USA_Paladin"), PALADIN_PDL_DELAY_FRAMES);
        assert_eq!(pdl_delay_frames("USA_Avenger"), AVENGER_PDL_DELAY_FRAMES);
        assert_eq!(
            pdl_delay_frames("AirF_AmericaJetRaptor"),
            KING_RAPTOR_PDL_DELAY_FRAMES
        );
        assert_eq!(
            pdl_scan_range("AirF_AmericaJetRaptor") as i32,
            KING_RAPTOR_PDL_SCAN_RANGE as i32
        );
        assert!(is_avenger_carrier("AmericaVehicleAvenger"));
        assert!(!is_avenger_carrier("AmericaTankPaladin"));
        assert!(!is_avenger_carrier("AirF_AmericaJetRaptor"));
        assert!(is_king_raptor_carrier("TestKingRaptor"));
    }

    #[test]
    fn missile_name_residual_matrix() {
        assert!(is_missile_name_residual("ScudMissile"));
        assert!(is_missile_name_residual("TomahawkMissile"));
        assert!(is_missile_name_residual("TestMissile"));
        assert!(is_missile_name_residual("TestProjectile"));
        assert!(is_missile_name_residual("PatriotMissileProjectile"));
        assert!(!is_missile_name_residual("AmericaInfantryMissileDefender"));
        assert!(!is_missile_name_residual("USA_Patriot"));
        assert!(!is_missile_name_residual("GLA_StingerSite"));
        assert!(!is_missile_name_residual("USA_Ranger"));
    }

    #[test]
    fn primary_secondary_intercept_matrix() {
        assert!(is_primary_intercept_target(true, true, false, "Anything"));
        assert!(is_primary_intercept_target(
            false,
            true,
            false,
            "ScudMissile"
        ));
        assert!(!is_primary_intercept_target(
            false,
            true,
            true,
            "ScudMissile"
        ));
        assert!(!is_primary_intercept_target(
            false,
            false,
            false,
            "ScudMissile"
        ));
        assert!(is_secondary_intercept_target(
            true, true, false, false, false, false, false
        ));
        assert!(!is_secondary_intercept_target(
            true, true, true, false, false, false, false
        ));
        assert!(!is_secondary_intercept_target(
            false, true, false, false, false, false, false
        ));
        // Leftover scan_closest_target: STEALTHED && !DETECTED && !DISGUISED skip.
        assert!(!is_secondary_intercept_target(
            true, true, false, false, true, false, false
        ));
        assert!(is_secondary_intercept_target(
            true, true, false, false, true, true, false
        ));
        assert!(is_secondary_intercept_target(
            true, true, false, false, true, false, true
        ));
        assert_eq!(pdl_module_count("USA_Avenger"), 2);
        assert_eq!(pdl_module_count("USA_Paladin"), 1);
        assert_eq!(pdl_module_count("AirF_AmericaJetRaptor"), 1);
    }

    #[test]
    fn range_and_priority() {
        assert!(in_pdl_range_2d((0.0, 0.0), (50.0, 0.0), 65.0));
        assert!(!in_pdl_range_2d((0.0, 0.0), (80.0, 0.0), 65.0));
        assert_eq!(intercept_priority(true, true), Some(0));
        assert_eq!(intercept_priority(false, true), Some(1));
        assert_eq!(intercept_priority(false, false), None);
    }

    /// Wave 49: ScanRate / ScanRange / VelocityPredict / target-type residual honesty.
    #[test]
    fn point_defense_scan_predict_residual_honesty() {
        assert!(honesty_point_defense_residual_ok());
        assert_eq!(pdl_ms_to_frames(500), 15);
        assert_eq!(pdl_ms_to_frames(100), 3);
        assert_eq!(pdl_ms_to_frames(33), 1);
        assert_eq!(pdl_ms_to_frames(10), 0);
        assert_eq!(pdl_ms_to_frames(0), 0);

        // Avenger ScanRange residual = retail **200** (not fire*1.2).
        assert!((pdl_scan_range("USA_Avenger") - AVENGER_PDL_SCAN_RANGE).abs() < 0.01);
        assert!((pdl_scan_range("USA_Paladin") - PALADIN_PDL_SCAN_RANGE).abs() < 0.01);
        assert!(
            (pdl_scan_range("AirF_AmericaJetRaptor") - KING_RAPTOR_PDL_SCAN_RANGE).abs() < 0.01
        );
        assert!(
            (pdl_scan_range("AirF_AmericaVehicleChinook") - COMBAT_CHINOOK_PDL_SCAN_RANGE).abs()
                < 0.01
        );

        assert_eq!(pdl_scan_rate_frames("USA_Paladin"), 15);
        assert_eq!(pdl_scan_rate_frames("USA_Avenger"), 3);
        assert_eq!(pdl_scan_rate_frames("AirF_AmericaJetRaptor"), 0);
        assert_eq!(pdl_scan_rate_frames("AirF_AmericaVehicleChinook"), 1);

        assert!((pdl_velocity_predict("USA_Paladin") - 3.0).abs() < 0.01);
        assert!((pdl_velocity_predict("USA_Avenger") - 1.0).abs() < 0.01);
        assert!((pdl_velocity_predict("AirF_AmericaJetRaptor") - 2.0).abs() < 0.01);
        assert!((pdl_velocity_predict("AirF_AmericaVehicleChinook") - 1.0).abs() < 0.01);

        assert!(pdl_has_secondary_infantry("USA_Paladin"));
        assert!(!pdl_has_secondary_infantry("USA_Avenger"));
        assert!(!pdl_has_secondary_infantry("AirF_AmericaJetRaptor"));
        assert!(!pdl_has_secondary_infantry("AirF_AmericaVehicleChinook"));

        assert_eq!(PDL_PRIMARY_TARGET_TYPES.len(), 2);
        assert_eq!(PDL_SECONDARY_TARGET_TYPES.len(), 1);
        assert!((pdl_damage("USA_Paladin") - 100.0).abs() < 0.01);
        assert_eq!(pdl_delay_frames("USA_Paladin"), 30);
        assert_eq!(pdl_delay_frames("USA_Avenger"), 15);
    }
    /// Wave 71 residual pack honesty gate.
    #[test]
    fn point_defense_residual_pack_honesty_wave71() {
        assert!(honesty_point_defense_residual_pack_ok());
        assert_eq!(PALADIN_PDL_SCAN_RATE_FRAMES, 15);
        assert_eq!(AVENGER_PDL_DELAY_FRAMES, 15);
        assert!((PALADIN_PDL_DAMAGE - 100.0).abs() < 0.01);
    }
}
