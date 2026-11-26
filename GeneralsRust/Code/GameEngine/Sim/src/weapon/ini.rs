//! Ordered C++ Weapon.ini property application.

use std::collections::HashMap;

use generals_game_domain::ini::{parse_bit_string_32_with_initial, parse_decimal_integer_prefix};
use log::warn;

use crate::damage::{DamageType, DeathType};

use super::{
    Coord2D, WeaponAffectsMask, WeaponBonus, WeaponBonusSet, WeaponCollideMask, WeaponDamageStatus,
    WeaponDefinition, WeaponFxName, WeaponPrefireType, WeaponReloadType, WeaponSound,
};

/// Slots whose new INI assignment supersedes any resolved legacy handle.
/// A named reference and an explicit `None` both replace the cached handle.
#[derive(Default)]
pub struct WeaponEffectOverrides {
    pub clears_historic_bonus_weapon: bool,
    pub fire_ocl: [bool; 4],
    pub projectile_detonation_ocl: [bool; 4],
    pub projectile_exhaust: [bool; 4],
}

/// Apply one ordered Weapon INI block to the effective definition.
pub fn apply_weapon_ini_fields(
    template: &mut WeaponDefinition,
    ordered: &[(String, String)],
) -> WeaponEffectOverrides {
    let name = &template.name;
    let properties = ordered.iter().cloned().collect::<HashMap<_, _>>();
    if let Some(val) = properties.get("DamageType") {
        template.damage_type = parse_damage_type(val);
    }
    // C++ ObjectStatusMaskType::parseSingleBitFromINI → m_damageStatusType.
    // Default remains OBJECT_STATUS_NONE (Weapon.cpp:303).
    if let Some(val) = properties.get("DamageStatusType") {
        template.damage_status_type = parse_damage_status_type(val);
    }

    if let Some(val) = properties.get("WeaponSpeed") {
        if let Ok(speed) = val.parse::<f32>() {
            template.weapon_speed = secs_to_frames_velocity(speed);
        }
    }

    if let Some(val) = properties.get("MinWeaponSpeed") {
        if let Ok(speed) = val.parse::<f32>() {
            template.min_weapon_speed = secs_to_frames_velocity(speed);
        }
    }

    if let Some(val) = properties.get("AttackRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.attack_range = range;
        }
    }

    if let Some(val) = properties.get("MinimumAttackRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.minimum_attack_range = range;
        }
    }
    if let Some(val) = properties.get("MinRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.minimum_attack_range = range;
        }
    }

    if let Some(val) = properties.get("PrimaryDamage") {
        if let Ok(damage) = val.parse::<f32>() {
            template.primary_damage = damage;
        }
    }

    if let Some(val) = properties.get("SecondaryDamage") {
        if let Ok(damage) = val.parse::<f32>() {
            template.secondary_damage = damage;
        }
    }

    if let Some(val) = properties.get("PrimaryDamageRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.primary_damage_radius = radius;
        }
    }

    if let Some(val) = properties.get("SecondaryDamageRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.secondary_damage_radius = radius;
        }
    }

    if let Some(val) = properties.get("ShockWaveAmount") {
        if let Ok(amount) = val.parse::<f32>() {
            template.shock_wave_amount = amount;
        }
    }

    if let Some(val) = properties.get("ShockWaveRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.shock_wave_radius = radius;
        }
    }

    if let Some(val) = properties.get("ShockWaveTaperOff") {
        if let Ok(val) = val.parse::<f32>() {
            template.shock_wave_taper_off = val;
        }
    }

    // C++ only authors DelayBetweenShots (parseShotDelay). Always overwrite
    // both min/max, including the labeled `Min:N Max:M` retail form.
    if let Some(val) = properties.get("DelayBetweenShots") {
        if let Some((min_msec, max_msec)) = parse_shot_delay_msec(val) {
            template.min_delay_between_shots = msec_to_logic_frames(min_msec);
            template.max_delay_between_shots = msec_to_logic_frames(max_msec);
        }
    }

    if let Some(val) = properties.get("ClipSize") {
        if let Ok(size) = val.parse::<i32>() {
            template.clip_size = size;
        }
    }

    // C++ WeaponTemplateFieldParseTable owns this independently of clip
    // cadence.  It drives the per-Weapon barrel cursor, so do not silently
    // collapse a multi-shot barrel into one shot while importing Weapon.ini.
    if let Some(val) = properties.get("ShotsPerBarrel") {
        if let Ok(shots) = val.parse::<i32>() {
            template.shots_per_barrel = shots;
        }
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames (30 FPS).
    if let Some(val) = properties.get("ClipReloadTime") {
        let tokens: Vec<&str> = val.split_whitespace().collect();
        if let Some(first) = tokens.first() {
            if let Ok(msec) = first.parse::<i32>() {
                template.clip_reload_time = msec_to_logic_frames(msec);
            }
        }
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames (30 FPS).
    if let Some(val) = properties.get("PreAttackDelay") {
        let tokens: Vec<&str> = val.split_whitespace().collect();
        if let Some(first) = tokens.first() {
            if let Ok(msec) = first.parse::<i32>() {
                template.pre_attack_delay = msec_to_logic_frames(msec);
            }
        }
    }

    if let Some(val) = properties.get("ProjectileTemplate") {
        template.projectile_name = val.clone();
    }
    if let Some(val) = properties.get("ProjectileObject") {
        template.projectile_name = val.clone();
    }

    if let Some(val) = properties.get("ProjectileStreamName") {
        template.projectile_stream_name = val.clone();
    }

    if let Some(val) = properties.get("FireSound") {
        template.fire_sound = WeaponSound::new(val.clone());
    }

    if let Some(val) = properties.get("ScatterRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.scatter_radius = radius;
        }
    }

    if let Some(val) = properties.get("AcceptableAimDelta") {
        if let Some(delta) = parse_angle_real(val) {
            template.aim_delta = delta;
        }
    }

    if let Some(val) = properties.get("RequestAssistRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.request_assist_range = range;
        }
    }

    if let Some(val) = properties.get("WeaponRecoil") {
        if let Some(recoil) = parse_angle_real(val) {
            template.weapon_recoil = recoil;
        }
    }

    if let Some(val) = properties.get("MinTargetPitch") {
        if let Some(pitch) = parse_angle_real(val) {
            template.min_target_pitch = pitch;
        }
    }
    if let Some(val) = properties.get("MaxTargetPitch") {
        if let Some(pitch) = parse_angle_real(val) {
            template.max_target_pitch = pitch;
        }
    }
    if let Some(val) = properties.get("RadiusDamageAngle") {
        if let Some(angle) = parse_angle_real(val) {
            template.radius_damage_angle = angle;
        }
    }

    if let Some(val) = properties.get("ScatterTargetScalar") {
        if let Ok(scalar) = val.parse::<f32>() {
            template.scatter_target_scalar = scalar;
        }
    }
    if let Some(val) = properties.get("ScatterRadiusVsInfantry") {
        if let Ok(dist) = val.parse::<f32>() {
            template.infantry_inaccuracy_dist = dist;
        }
    }

    // C++ `INI::parseBitInInt32` obeys both Yes and No.  In particular,
    // WeaponTemplate starts with AntiGround, so treating `AntiGround = No`
    // as a no-op quietly lets dedicated AA/anti-missile weapons hit ground.
    // Keep the exact per-bit result rather than inferring broad air/ground
    // capability from a weapon name or projectile type.
    let mut parse_anti_mask = |field: &str, bit: u32| {
        let Some(value) = properties.get(field) else {
            return;
        };
        match parse_ini_bool(value) {
            Some(true) => template.anti_mask.insert(bit),
            Some(false) => template.anti_mask.remove(bit),
            None => log::warn!("Invalid boolean for Weapon `{name}` field `{field}`: {value}"),
        }
    };
    parse_anti_mask(
        "AntiAirborneVehicle",
        super::WeaponAntiMask::AIRBORNE_VEHICLE,
    );
    parse_anti_mask("AntiGround", super::WeaponAntiMask::GROUND);
    parse_anti_mask("AntiProjectile", super::WeaponAntiMask::PROJECTILE);
    parse_anti_mask("AntiSmallMissile", super::WeaponAntiMask::SMALL_MISSILE);
    parse_anti_mask("AntiMine", super::WeaponAntiMask::MINE);
    parse_anti_mask("AntiParachute", super::WeaponAntiMask::PARACHUTE);
    parse_anti_mask(
        "AntiAirborneInfantry",
        super::WeaponAntiMask::AIRBORNE_INFANTRY,
    );
    parse_anti_mask(
        "AntiBallisticMissile",
        super::WeaponAntiMask::BALLISTIC_MISSILE,
    );
    // Release the closure's exclusive template borrow before parsing the
    // remainder of the Weapon.ini fields.
    drop(parse_anti_mask);

    if let Some(val) = properties.get("ScaleWeaponSpeed") {
        if let Some(b) = parse_ini_bool(val) {
            template.is_scale_weapon_speed = b;
        }
    }

    // C++ WeaponTemplateFieldParseTable: FireFX from an undetected stealth
    // source is suppressed unless this exact authored flag is true. Keep it
    // in the parsed template rather than reconstructing it from a weapon name.
    if let Some(val) = properties.get("PlayFXWhenStealthed") {
        if let Some(b) = parse_ini_bool(val) {
            template.play_fx_when_stealthed = b;
        }
    }

    if let Some(val) = properties.get("DeathType") {
        template.death_type = parse_death_type(val);
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames.
    if let Some(val) = properties.get("AutoReloadWhenIdle") {
        if let Ok(msec) = val.parse::<i32>() {
            template.auto_reload_when_idle_frames = msec_to_logic_frames(msec) as u32;
        }
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames.
    if let Some(val) = properties.get("SuspendFXDelay") {
        if let Ok(msec) = val.parse::<i32>() {
            template.suspend_fx_delay = msec_to_logic_frames(msec) as u32;
        }
    }

    if let Some(val) = properties.get("ContinueAttackRange") {
        if let Ok(range) = val.parse::<f32>() {
            template.continue_attack_range = range;
        }
    }

    // C++: INI::parseDurationUnsignedInt — msec → logic frames.
    if let Some(val) = properties.get("HistoricBonusTime") {
        if let Ok(msec) = val.parse::<i32>() {
            template.historic_bonus_time = msec_to_logic_frames(msec) as u32;
        }
    }

    if let Some(val) = properties.get("HistoricBonusRadius") {
        if let Ok(radius) = val.parse::<f32>() {
            template.historic_bonus_radius = radius;
        }
    }

    if let Some(val) = properties.get("HistoricBonusCount") {
        if let Ok(count) = val.parse::<i32>() {
            template.historic_bonus_count = count;
        }
    }

    let clears_historic_bonus_weapon = properties.get("HistoricBonusWeapon").is_some_and(|val| {
        template.historic_bonus_weapon_name = parse_weapon_reference_name(val).unwrap_or_default();
        template.historic_bonus_weapon_name.is_empty()
    });

    if let Some(val) = properties.get("LaserName") {
        template.laser_name = val.trim().to_string();
    }
    if let Some(val) = properties.get("LaserBoneName") {
        template.laser_bone_name = val.trim().to_string();
    }

    if let Some(val) = properties.get("ContinuousFireOne") {
        if let Ok(shots) = val.parse::<i32>() {
            template.continuous_fire_one_shots_needed = shots;
        }
    }
    if let Some(val) = properties.get("ContinuousFireTwo") {
        if let Ok(shots) = val.parse::<i32>() {
            template.continuous_fire_two_shots_needed = shots;
        }
    }
    if let Some(val) = properties.get("ContinuousFireCoast") {
        if let Ok(msec) = val.parse::<i32>() {
            template.continuous_fire_coast_frames = msec_to_logic_frames(msec) as u32;
        }
    }

    if let Some(val) = properties.get("FireSoundLoopTime") {
        if let Ok(msec) = val.parse::<i32>() {
            template.fire_sound_loop_time = msec_to_logic_frames(msec) as u32;
        }
    }

    if let Some(val) = properties.get("AutoReloadsClip") {
        if let Some(reload) = WeaponReloadType::from_ini(val.trim()) {
            template.reload_type = reload;
        }
    }
    if let Some(val) = properties.get("PreAttackType") {
        if let Some(prefire) = WeaponPrefireType::from_ini(val.trim()) {
            template.prefire_type = prefire;
        }
    }

    if let Some(val) = properties.get("RadiusDamageAffects") {
        if let Some(bits) = parse_bit_string_32(
            val,
            &[
                "SELF",
                "ALLIES",
                "ENEMIES",
                "NEUTRALS",
                "SUICIDE",
                "NOT_SIMILAR",
                "NOT_AIRBORNE",
            ],
        ) {
            template.affects_mask = WeaponAffectsMask::new(bits);
        }
    }
    if let Some(val) = properties.get("ProjectileCollidesWith") {
        if let Some(bits) = parse_bit_string_32(
            val,
            &[
                "ALLIES",
                "ENEMIES",
                "STRUCTURES",
                "SHRUBBERY",
                "PROJECTILES",
                "WALLS",
                "SMALL_MISSILES",
                "BALLISTIC_MISSILES",
                "CONTROLLED_STRUCTURES",
            ],
        ) {
            template.collide_mask = WeaponCollideMask::new(bits);
        }
    }

    if let Some(val) = properties.get("DamageDealtAtSelfPosition") {
        if let Some(b) = parse_ini_bool(val) {
            template.damage_dealt_at_self_position = b;
        }
    }
    if let Some(val) = properties.get("LeechRangeWeapon") {
        if let Some(b) = parse_ini_bool(val) {
            template.leech_range_weapon = b;
        }
    }
    if let Some(val) = properties.get("CapableOfFollowingWaypoints") {
        if let Some(b) = parse_ini_bool(val) {
            template.capable_of_following_waypoint = b;
        }
    }
    if let Some(val) = properties.get("ShowsAmmoPips") {
        if let Some(b) = parse_ini_bool(val) {
            template.is_shows_ammo_pips = b;
        }
    }
    if let Some(val) = properties.get("AllowAttackGarrisonedBldgs") {
        if let Some(b) = parse_ini_bool(val) {
            template.allow_attack_garrisoned_bldgs = b;
        }
    }
    if let Some(val) = properties.get("MissileCallsOnDie") {
        if let Some(b) = parse_ini_bool(val) {
            template.die_on_detonate = b;
        }
    }

    // `Weapon.cpp` applies these fields in file order. Keep this separate from
    // scalar-property parsing above because a HashMap has already discarded
    // repeated `Veterancy…` / `WeaponBonus` / `ScatterTarget` assignments.
    WeaponEffectOverrides {
        clears_historic_bonus_weapon,
        ..apply_weapon_effect_references(template, ordered)
    }
}

/// C++ ConvertDurationFromMsecsToFrames (logic clock at 30 FPS).
fn msec_to_logic_frames(msec: i32) -> i32 {
    if msec <= 0 {
        return 0;
    }
    // ceil(msec * FPS / 1000)
    ((msec as i64 * 30 + 999) / 1000) as i32
}

/// Parse C++ INI booleans: Yes/No, True/False, 1/0.
fn parse_ini_bool(val: &str) -> Option<bool> {
    match val.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => val.parse::<bool>().ok(),
    }
}

/// C++ `ConvertVelocityInSecsToFrames` (`GameCommon.h`): dist/sec → dist/frame.
fn secs_to_frames_velocity(speed: f32) -> f32 {
    speed * (1.0_f32 / 30.0_f32)
}

/// C++ `INI::parseAngleReal`: authored degrees → radians.
fn parse_angle_real(token: &str) -> Option<f32> {
    token
        .trim()
        .parse::<f32>()
        .ok()
        .map(|degrees| degrees * std::f32::consts::PI / 180.0)
}

/// C++ `WeaponTemplate::parseShotDelay`: one msec value, or `Min:N Max:M`.
/// Always overwrites both min and max, then converts msec → frames with ceil.
fn parse_shot_delay_msec(value: &str) -> Option<(i32, i32)> {
    let tokens: Vec<&str> = value
        .split(|c: char| c.is_whitespace() || c == ':')
        .filter(|token| !token.is_empty())
        .collect();
    let first = *tokens.first()?;
    if first.eq_ignore_ascii_case("Min") {
        let min = parse_decimal_integer_prefix(*tokens.get(1)?)?;
        let max = if tokens
            .get(2)
            .is_some_and(|token| token.eq_ignore_ascii_case("Max"))
        {
            parse_decimal_integer_prefix(*tokens.get(3)?)?
        } else {
            min
        };
        Some((min, max))
    } else {
        let msec = parse_decimal_integer_prefix(first)?;
        Some((msec, msec))
    }
}

/// C++ `INI::parseBitString32`: `NONE`, bare tokens, or `+/-` (not mixed).
fn parse_bit_string_32(value: &str, names: &[&str]) -> Option<u32> {
    parse_bit_string_32_with_initial(value, names, 0)
}

fn parse_percent_to_real(token: &str) -> Option<f32> {
    let trimmed = token.trim();
    if let Some(stripped) = trimmed.strip_suffix('%') {
        stripped.parse::<f32>().ok().map(|value| value / 100.0)
    } else {
        trimmed.parse::<f32>().ok()
    }
}

fn parse_weapon_bonus_condition(token: &str) -> Option<super::WeaponBonusConditionType> {
    use super::WeaponBonusConditionType::*;
    match token.trim().to_ascii_uppercase().as_str() {
        "GARRISONED" => Some(Garrisoned),
        "HORDE" => Some(Horde),
        "CONTINUOUS_FIRE_MEAN" => Some(ContinuousFireMean),
        "CONTINUOUS_FIRE_FAST" => Some(ContinuousFireFast),
        "NATIONALISM" => Some(Nationalism),
        "PLAYER_UPGRADE" => Some(PlayerUpgrade),
        "DRONE_SPOTTING" => Some(DroneSpotting),
        "DEMORALIZED" => Some(Demoralized),
        "ENTHUSIASTIC" => Some(Enthusiastic),
        "VETERAN" => Some(Veteran),
        "ELITE" => Some(Elite),
        "HERO" => Some(Hero),
        "BATTLEPLAN_BOMBARDMENT" => Some(BattleplanBombardment),
        "BATTLEPLAN_HOLDTHELINE" => Some(BattleplanHoldtheLine),
        "BATTLEPLAN_SEARCHANDDESTROY" => Some(BattleplanSearchAndDestroy),
        "SUBLIMINAL" => Some(Subliminal),
        "SOLO_HUMAN_EASY" => Some(SoloHumanEasy),
        "SOLO_HUMAN_NORMAL" => Some(SoloHumanNormal),
        "SOLO_HUMAN_HARD" => Some(SoloHumanHard),
        "SOLO_AI_EASY" => Some(SoloAiEasy),
        "SOLO_AI_NORMAL" => Some(SoloAiNormal),
        "SOLO_AI_HARD" => Some(SoloAiHard),
        "TARGET_FAERIE_FIRE" => Some(TargetFaerieFire),
        "FANATICISM" => Some(Fanaticism),
        "FRENZY_ONE" => Some(FrenzyOne),
        "FRENZY_TWO" => Some(FrenzyTwo),
        "FRENZY_THREE" => Some(FrenzyThree),
        _ => None,
    }
}

fn parse_weapon_bonus_field(token: &str) -> Option<super::WeaponBonusField> {
    use super::WeaponBonusField::*;
    match token.trim().to_ascii_uppercase().as_str() {
        "DAMAGE" => Some(Damage),
        "RADIUS" => Some(Radius),
        "RANGE" => Some(Range),
        "RATE_OF_FIRE" => Some(RateOfFire),
        "PRE_ATTACK" => Some(PreAttack),
        _ => None,
    }
}

fn apply_weapon_bonus_line(template: &mut WeaponDefinition, value: &str) {
    let tokens: Vec<&str> = value.split_whitespace().collect();
    if tokens.len() < 3 {
        return;
    }
    let Some(condition) = parse_weapon_bonus_condition(tokens[0]) else {
        return;
    };
    let Some(field) = parse_weapon_bonus_field(tokens[1]) else {
        return;
    };
    let Some(percent) = parse_percent_to_real(tokens[2]) else {
        return;
    };
    let set = template.extra_bonus.get_or_insert_with(WeaponBonusSet::new);
    let mut bonus = set
        .get_bonus(condition)
        .cloned()
        .unwrap_or_else(WeaponBonus::new);
    bonus.set_field(field, percent);
    set.set_bonus(condition, bonus);
}

fn apply_scatter_target_line(template: &mut WeaponDefinition, value: &str) {
    let tokens: Vec<&str> = value
        .split(|c: char| c.is_whitespace() || c == ':')
        .filter(|token| !token.is_empty())
        .filter(|token| !token.eq_ignore_ascii_case("X") && !token.eq_ignore_ascii_case("Y"))
        .collect();
    if tokens.len() < 2 {
        return;
    }
    if let (Ok(x), Ok(y)) = (tokens[0].parse::<f32>(), tokens[1].parse::<f32>()) {
        template.scatter_targets.push(Coord2D::new(x, y));
    }
}

/// Parse the one identifier C++ `INI::getNextToken` consumes for a named
/// Weapon.ini reference. `None` is a null pointer in the original engine, not
/// a valid FX/OCL/particle template name.
fn parse_weapon_reference_name(value: &str) -> Option<String> {
    let name = value.split_whitespace().next()?.trim();
    (!name.is_empty() && !name.eq_ignore_ascii_case("none")).then(|| name.to_string())
}

/// C++ `TheVeterancyNames` is exactly
/// `REGULAR`, `VETERAN`, `ELITE`, `HEROIC` in that order. Do not silently map
/// unknown names to Regular: that would turn malformed data into a visible
/// wrong-level effect.
fn parse_cpp_veterancy_index(value: &str) -> Option<usize> {
    if value.eq_ignore_ascii_case("regular") {
        Some(0)
    } else if value.eq_ignore_ascii_case("veteran") {
        Some(1)
    } else if value.eq_ignore_ascii_case("elite") {
        Some(2)
    } else if value.eq_ignore_ascii_case("heroic") {
        Some(3)
    } else {
        None
    }
}

/// Parse `Veterancy<Field> = LEVEL TemplateName` without losing the second
/// token. `Some((level, None))` is an intentional `None` override; `None`
/// means the property was malformed and is ignored like an unresolved C++ INI
/// field rather than being guessed.
fn parse_veterancy_weapon_reference(value: &str) -> Option<(usize, Option<String>)> {
    let mut tokens = value.split_whitespace();
    let level = parse_cpp_veterancy_index(tokens.next()?)?;
    let reference = tokens.next()?;
    let reference = (!reference.eq_ignore_ascii_case("none")).then(|| reference.to_string());
    Some((level, reference))
}

fn all_veterancy<T: Clone>(value: Option<T>) -> [Option<T>; 4] {
    std::array::from_fn(|_| value.clone())
}

/// Apply C++ Weapon.cpp lines 173–182 exactly enough for the active Rust
/// store:
///
/// - base fields populate Regular/Veteran/Elite/Heroic;
/// - every repeated `Veterancy…` field subsequently overwrites one named slot;
/// - `None` remains absent instead of becoming a fabricated template;
/// - OCL and particle names are retained for deferred, real-store resolution.
///
fn apply_weapon_effect_references(
    template: &mut WeaponDefinition,
    ordered: &[(String, String)],
) -> WeaponEffectOverrides {
    let mut overrides = WeaponEffectOverrides::default();
    for (key, value) in ordered {
        if key.eq_ignore_ascii_case("FireFX") {
            let fx = parse_weapon_reference_name(value).map(|name| WeaponFxName::new(&name));
            template.fire_fx = all_veterancy(fx);
            continue;
        }

        if key.eq_ignore_ascii_case("ProjectileDetonationFX") {
            let fx = parse_weapon_reference_name(value).map(|name| WeaponFxName::new(&name));
            template.projectile_detonate_fx = all_veterancy(fx);
            continue;
        }

        if key.eq_ignore_ascii_case("FireOCL") {
            template.fire_ocl_names = all_veterancy(parse_weapon_reference_name(value));
            overrides.fire_ocl = [true; 4];
            continue;
        }

        if key.eq_ignore_ascii_case("ProjectileDetonationOCL") {
            template.projectile_detonation_ocl_names =
                all_veterancy(parse_weapon_reference_name(value));
            overrides.projectile_detonation_ocl = [true; 4];
            continue;
        }

        if key.eq_ignore_ascii_case("ProjectileExhaust") {
            template.projectile_exhaust_names = all_veterancy(parse_weapon_reference_name(value));
            overrides.projectile_exhaust = [true; 4];
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyFireFX") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.fire_fx[level] = name.map(|name| WeaponFxName::new(&name));
            } else {
                warn!(
                    "Ignoring malformed VeterancyFireFX for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyProjectileDetonationFX") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.projectile_detonate_fx[level] = name.map(|name| WeaponFxName::new(&name));
            } else {
                warn!(
                    "Ignoring malformed VeterancyProjectileDetonationFX for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyFireOCL") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.fire_ocl_names[level] = name;
                overrides.fire_ocl[level] = true;
            } else {
                warn!(
                    "Ignoring malformed VeterancyFireOCL for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyProjectileDetonationOCL") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.projectile_detonation_ocl_names[level] = name;
                overrides.projectile_detonation_ocl[level] = true;
            } else {
                warn!(
                    "Ignoring malformed VeterancyProjectileDetonationOCL for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("VeterancyProjectileExhaust") {
            if let Some((level, name)) = parse_veterancy_weapon_reference(value) {
                template.projectile_exhaust_names[level] = name;
                overrides.projectile_exhaust[level] = true;
            } else {
                warn!(
                    "Ignoring malformed VeterancyProjectileExhaust for '{}': {}",
                    template.name, value
                );
            }
            continue;
        }

        if key.eq_ignore_ascii_case("WeaponBonus") {
            apply_weapon_bonus_line(template, value);
            continue;
        }

        if key.eq_ignore_ascii_case("ScatterTarget") {
            apply_scatter_target_line(template, value);
        }
    }
    overrides
}

/// Parse a damage type string into the GameLogic DamageType.
fn parse_damage_type(s: &str) -> DamageType {
    match s.to_lowercase().as_str() {
        "explosion" | "explosive" => DamageType::Explosion,
        "small_arms" | "smallarms" | "bullet" => DamageType::SmallArms,
        "flame" | "fire" | "burn" => DamageType::Flame,
        "crush" => DamageType::Crush,
        "armor_piercing" | "piercing" | "ap" => DamageType::ArmorPiercing,
        "hazard" | "chemical" | "hazard_cleanup" => DamageType::HazardCleanup,
        "heal" | "repair" | "healing" => DamageType::Healing,
        "disarm" => DamageType::Disarm,
        "sabotage" => DamageType::Hack,
        "snipe" | "sniper" => DamageType::Sniper,
        "laser" => DamageType::Laser,
        "radiation" | "rad" => DamageType::Radiation,
        "microwave" | "electric" | "electricity" | "emp" => DamageType::Microwave,
        "subdual" => DamageType::SubdualUnresistable,
        "status" => DamageType::Status,
        "combat" => DamageType::SmallArms,
        "particle" | "particlebeam" | "particle_beam" => DamageType::ParticleBeam,
        "poison" | "toxin" | "anthrax" => DamageType::Poison,
        "leadership" | "leadership_bonus" => DamageType::Unresistable,
        "demoralizing" | "demoralizing_shock" => DamageType::Penalty,
        "unresistable" | "none" => DamageType::Unresistable,
        "gattling" => DamageType::Gattling,
        "water" => DamageType::Water,
        "deploy" => DamageType::Deploy,
        "surrender" => DamageType::Surrender,
        "hack" => DamageType::Hack,
        "kill_pilot" => DamageType::KillPilot,
        "penalty" => DamageType::Penalty,
        "falling" => DamageType::Falling,
        "melee" => DamageType::Melee,
        "toppling" => DamageType::Toppling,
        "infantry_missile" => DamageType::InfantryMissile,
        "aurora_bomb" => DamageType::AuroraBomb,
        "land_mine" => DamageType::LandMine,
        "jet_missiles" => DamageType::JetMissiles,
        "stealthjet_missiles" => DamageType::StealthJetMissiles,
        "molotov_cocktail" => DamageType::MolotovCocktail,
        "comanche_vulcan" => DamageType::ComancheVulcan,
        "subdual_missile" => DamageType::SubdualMissile,
        "subdual_vehicle" => DamageType::SubdualVehicle,
        "subdual_building" => DamageType::SubdualBuilding,
        "subdual_unresistable" => DamageType::SubdualUnresistable,
        "kill_garrisoned" => DamageType::KillGarrisoned,
        _ => DamageType::Explosion, // Default fallback
    }
}

/// C++ `ObjectStatusMaskType::parseSingleBitFromINI` — one bit-name index.
///
/// Names match `ObjectStatusMaskType::s_bitNameList`. Unknown tokens stay
/// `OBJECT_STATUS_NONE` (ctor default) rather than inventing a bit.
fn parse_damage_status_type(s: &str) -> WeaponDamageStatus {
    use crate::object::ObjectStatusTypes as CommonStatus;
    let status = match s.trim().to_ascii_uppercase().as_str() {
        "NONE" => CommonStatus::None,
        "DESTROYED" => CommonStatus::Destroyed,
        "CAN_ATTACK" => CommonStatus::CanAttack,
        "UNDER_CONSTRUCTION" => CommonStatus::UnderConstruction,
        "UNSELECTABLE" => CommonStatus::Unselectable,
        "NO_COLLISIONS" => CommonStatus::NoCollisions,
        "NO_ATTACK" => CommonStatus::NoAttack,
        "AIRBORNE_TARGET" => CommonStatus::AirborneTarget,
        "PARACHUTING" => CommonStatus::Parachuting,
        "REPULSOR" => CommonStatus::Repulsor,
        "HIJACKED" => CommonStatus::Hijacked,
        "AFLAME" => CommonStatus::Aflame,
        "BURNED" => CommonStatus::Burned,
        "WET" => CommonStatus::Wet,
        "IS_FIRING_WEAPON" => CommonStatus::IsFiringWeapon,
        "IS_BRAKING" => CommonStatus::Braking,
        "STEALTHED" => CommonStatus::Stealthed,
        "DETECTED" => CommonStatus::Detected,
        "CAN_STEALTH" => CommonStatus::CanStealth,
        "SOLD" => CommonStatus::Sold,
        "UNDERGOING_REPAIR" => CommonStatus::UndergoingRepair,
        "RECONSTRUCTING" => CommonStatus::Reconstructing,
        "MASKED" => CommonStatus::Masked,
        "IS_ATTACKING" => CommonStatus::IsAttacking,
        "USING_ABILITY" => CommonStatus::IsUsingAbility,
        "IS_AIMING_WEAPON" => CommonStatus::IsAimingWeapon,
        "NO_ATTACK_FROM_AI" => CommonStatus::NoAttackFromAi,
        "IGNORING_STEALTH" => CommonStatus::IgnoringStealth,
        "IS_CARBOMB" => CommonStatus::IsCarBomb,
        "DECK_HEIGHT_OFFSET" => CommonStatus::DeckHeightOffset,
        "STATUS_RIDER1" => CommonStatus::Rider1,
        "STATUS_RIDER2" => CommonStatus::Rider2,
        "STATUS_RIDER3" => CommonStatus::Rider3,
        "STATUS_RIDER4" => CommonStatus::Rider4,
        "STATUS_RIDER5" => CommonStatus::Rider5,
        "STATUS_RIDER6" => CommonStatus::Rider6,
        "STATUS_RIDER7" => CommonStatus::Rider7,
        "STATUS_RIDER8" => CommonStatus::Rider8,
        "FAERIE_FIRE" => CommonStatus::FaerieFire,
        "KILLING_SELF" => CommonStatus::MissileKillingSelf,
        "REASSIGN_PARKING" => CommonStatus::ReassignParking,
        "BOOBY_TRAPPED" => CommonStatus::BoobyTrapped,
        "IMMOBILE" => CommonStatus::Immobile,
        "DISGUISED" => CommonStatus::Disguised,
        "DEPLOYED" => CommonStatus::Deployed,
        _ => CommonStatus::None,
    };
    WeaponDamageStatus::new(status as u32)
}

/// Parse a death type string into the GameLogic DeathType.
fn parse_death_type(s: &str) -> DeathType {
    match s.to_lowercase().as_str() {
        "normal" => DeathType::Normal,
        "burned" | "fire" | "flame" => DeathType::Burned,
        "crushed" => DeathType::Crushed,
        "exploded" | "explosion" => DeathType::Exploded,
        "flooded" => DeathType::Flooded,
        "poisoned" | "poison" => DeathType::Poisoned,
        "poisoned_beta" | "poisonedbeta" => DeathType::PoisonedBeta,
        "poisoned_gamma" | "poisonedgamma" => DeathType::PoisonedGamma,
        "toppled" => DeathType::Toppled,
        "suicided" => DeathType::Suicided,
        "lasered" => DeathType::Lasered,
        // CPP Damage.h TheDeathNames[] entries previously collapsed to NORMAL.
        "none" => DeathType::None,
        "detonated" => DeathType::Detonated,
        "splatted" => DeathType::Splatted,
        "extra_2" => DeathType::Extra2,
        "extra_3" => DeathType::Extra3,
        "extra_4" => DeathType::Extra4,
        "extra_5" => DeathType::Extra5,
        "extra_6" => DeathType::Extra6,
        "extra_7" => DeathType::Extra7,
        "extra_8" => DeathType::Extra8,
        _ => DeathType::Normal, // Default fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_msec_to_logic_frames_matches_cpp_duration() {
        // C++ ConvertDurationFromMsecsToFrames: ceil(msec * 30 / 1000)
        assert_eq!(msec_to_logic_frames(0), 0);
        assert_eq!(msec_to_logic_frames(-1), 0);
        assert_eq!(msec_to_logic_frames(1000), 30); // 1 second
        assert_eq!(msec_to_logic_frames(100), 3); // ceil(3.0)
        assert_eq!(msec_to_logic_frames(33), 1); // ceil(0.99) = 1
        assert_eq!(msec_to_logic_frames(2000), 60);
    }

    #[test]
    fn test_parse_damage_type() {
        assert_eq!(parse_damage_type("Crush"), DamageType::Crush);
        assert_eq!(parse_damage_type("Small_Arms"), DamageType::SmallArms);
        assert_eq!(parse_damage_type("Flame"), DamageType::Flame);
        assert_eq!(parse_damage_type("Laser"), DamageType::Laser);
    }

    #[test]
    fn parse_shot_delay_accepts_min_max_colon_form() {
        assert_eq!(parse_shot_delay_msec("500"), Some((500, 500)));
        assert_eq!(parse_shot_delay_msec("250.0"), Some((250, 250)));
        assert_eq!(
            parse_shot_delay_msec("Min:34.99 Max:250.9"),
            Some((34, 250))
        );
        assert_eq!(parse_shot_delay_msec("Min:200 Max:800"), Some((200, 800)));
        assert_eq!(parse_shot_delay_msec("2147483648"), None);
        assert_eq!(msec_to_logic_frames(500), 15);
        assert_eq!(
            secs_to_frames_velocity(600.0),
            600.0_f32 * (1.0_f32 / 30.0_f32)
        );
        let delta = parse_angle_real("30").unwrap();
        assert!((delta - std::f32::consts::PI / 6.0).abs() < 1e-5);
    }
}
