//! C++ `TurretAI` / `TurretAIData` spawn residual.
//!
//! `TurretAI` is constructed only when `AIUpdateModuleData` authors a
//! `Turret` / `AltTurret` block. Natural pose defaults to **0/0**; Strategy
//! Center is the special case (`NaturalTurretAngle -90`, `NaturalTurretPitch
//! 45`, `InitiallyDisabled = Yes`). Idle-scan angles stay **0/0** unless
//! authored (do not invent a hunt on tanks / Humvee / Patriot).

use crate::assets::BehaviorModuleDefinition;

/// C++ `LOGICFRAMES_PER_SECOND`.
const TURRET_LOGIC_FPS: f32 = 30.0;
/// C++ `TurretAIData` default Min/MaxIdleScanInterval.
const DEFAULT_IDLE_SCAN_INTERVAL: u32 = 9_999_999;
/// Retail Gattling / QuadCannon `TurretFireAngleSweep` residual (deg).
const GATTLING_FIRE_ANGLE_SWEEP_DEG: f32 = 3.0;

/// Resolved TurretAI spawn pose + INI rates for one object.
#[derive(Debug, Clone)]
pub struct TurretSpawnSpec {
    pub has_turret: bool,
    pub enabled: bool,
    pub angle_deg: f32,
    pub pitch_deg: f32,
    pub natural_angle_deg: f32,
    pub natural_pitch_deg: f32,
    pub turn_rate_rad: f32,
    pub pitch_rate_rad: f32,
    pub recenter_frames: u32,
    pub allows_pitch: bool,
    pub fire_pitch_rad: f32,
    pub min_pitch_rad: f32,
    pub ground_unit_pitch_rad: f32,
    pub min_idle_scan_angle_rad: f32,
    pub max_idle_scan_angle_rad: f32,
    pub min_idle_scan_interval: u32,
    pub max_idle_scan_interval: u32,
    pub fire_angle_sweep: [f32; 3],
    pub sweep_speed_mod: [f32; 3],
    pub fires_while_turning: bool,
}

impl TurretSpawnSpec {
    /// No TurretAI: infantry, dozers, props. Pose stays 0/0 and disabled.
    pub fn absent() -> Self {
        Self {
            has_turret: false,
            enabled: false,
            angle_deg: 0.0,
            pitch_deg: 0.0,
            natural_angle_deg: 0.0,
            natural_pitch_deg: 0.0,
            turn_rate_rad: super::default_turret_turn_rate(),
            pitch_rate_rad: super::default_turret_turn_rate(),
            recenter_frames: super::default_turret_recenter_frames(),
            allows_pitch: false,
            fire_pitch_rad: 0.0,
            min_pitch_rad: 0.0,
            ground_unit_pitch_rad: 0.0,
            min_idle_scan_angle_rad: 0.0,
            max_idle_scan_angle_rad: 0.0,
            min_idle_scan_interval: DEFAULT_IDLE_SCAN_INTERVAL,
            max_idle_scan_interval: DEFAULT_IDLE_SCAN_INTERVAL,
            fire_angle_sweep: [0.0; 3],
            sweep_speed_mod: [1.0; 3],
            fires_while_turning: false,
        }
    }

    fn enabled_with_turn_rate(turn_deg_s: f32) -> Self {
        let rate = turret_deg_per_sec_to_rad_per_frame(turn_deg_s);
        let mut spec = Self::absent();
        spec.has_turret = true;
        spec.enabled = true;
        spec.turn_rate_rad = rate;
        spec.pitch_rate_rad = rate;
        spec
    }
}

/// C++ `INI::parseAngularVelocityReal` — deg/s → rad/logic-frame.
pub fn turret_deg_per_sec_to_rad_per_frame(deg_per_sec: f32) -> f32 {
    deg_per_sec.to_radians() / TURRET_LOGIC_FPS
}

/// C++ `INI::parseDurationUnsignedInt` — msec → frames (ceil).
pub fn turret_ms_to_frames(ms: u32) -> u32 {
    let frames = (ms as u64).saturating_mul(30).saturating_add(999) / 1_000;
    u32::try_from(frames).unwrap_or(u32::MAX)
}

/// Resolve TurretAI spawn for a template. Prefer authored Object INI; fall
/// back to host honesty constants for known turreted units.
pub fn turret_spawn_for_template(template_name: &str) -> TurretSpawnSpec {
    if let Some(from_ini) = turret_spawn_from_object_definition(template_name) {
        return from_ini;
    }
    turret_spawn_from_name_honesty(template_name)
}

fn turret_spawn_from_object_definition(template_name: &str) -> Option<TurretSpawnSpec> {
    let manager = crate::assets::get_asset_manager()?;
    let guard = manager.lock().ok()?;
    let definition = guard
        .get_object_definition(template_name)
        .or_else(|| guard.resolve_object_definition(template_name, None))?;
    let module = definition
        .behavior_modules
        .iter()
        .find(|module| module_has_turret_block(module))?;
    Some(parse_turret_module(module))
}

fn module_has_turret_block(module: &BehaviorModuleDefinition) -> bool {
    module.attribute("TurretTurnRate").is_some()
        || module.attribute("TurretPitchRate").is_some()
        || module.attribute("NaturalTurretAngle").is_some()
        || module.attribute("NaturalTurretPitch").is_some()
        || module.attribute("ControlledWeaponSlots").is_some()
        || module.attribute("TurretFireAngleSweep").is_some()
}

fn parse_turret_module(module: &BehaviorModuleDefinition) -> TurretSpawnSpec {
    let mut spec = TurretSpawnSpec::absent();
    spec.has_turret = true;

    let turn_deg = parse_f32_attr(module, "TurretTurnRate");
    let pitch_deg = parse_f32_attr(module, "TurretPitchRate").or(turn_deg);
    if let Some(deg) = turn_deg {
        spec.turn_rate_rad = turret_deg_per_sec_to_rad_per_frame(deg);
    }
    if let Some(deg) = pitch_deg {
        spec.pitch_rate_rad = turret_deg_per_sec_to_rad_per_frame(deg);
    }

    let nat_a = parse_f32_attr(module, "NaturalTurretAngle").unwrap_or(0.0);
    let nat_p = parse_f32_attr(module, "NaturalTurretPitch").unwrap_or(0.0);
    spec.natural_angle_deg = nat_a;
    spec.natural_pitch_deg = nat_p;
    spec.angle_deg = nat_a;
    spec.pitch_deg = nat_p;

    spec.allows_pitch = parse_bool_attr(module, "AllowsPitch").unwrap_or(false);
    if let Some(deg) = parse_f32_attr(module, "FirePitch") {
        spec.fire_pitch_rad = deg.to_radians();
        spec.allows_pitch = true;
    }
    if let Some(deg) = parse_f32_attr(module, "MinPhysicalPitch") {
        spec.min_pitch_rad = deg.to_radians();
    }
    if let Some(deg) = parse_f32_attr(module, "GroundUnitPitch") {
        spec.ground_unit_pitch_rad = deg.to_radians();
    }

    spec.min_idle_scan_angle_rad = parse_f32_attr(module, "MinIdleScanAngle")
        .unwrap_or(0.0)
        .to_radians();
    spec.max_idle_scan_angle_rad = parse_f32_attr(module, "MaxIdleScanAngle")
        .unwrap_or(0.0)
        .to_radians();
    if let Some(ms) = parse_u32_attr(module, "MinIdleScanInterval") {
        spec.min_idle_scan_interval = turret_ms_to_frames(ms);
    }
    if let Some(ms) = parse_u32_attr(module, "MaxIdleScanInterval") {
        spec.max_idle_scan_interval = turret_ms_to_frames(ms).max(spec.min_idle_scan_interval);
    }
    if let Some(ms) = parse_u32_attr(module, "RecenterTime") {
        spec.recenter_frames = turret_ms_to_frames(ms).max(1);
    }
    spec.fires_while_turning = parse_bool_attr(module, "FiresWhileTurning").unwrap_or(false);
    let initially_disabled = parse_bool_attr(module, "InitiallyDisabled").unwrap_or(false);
    spec.enabled = !initially_disabled;

    if let Some(sweep) = parse_sweep_attr(module, "TurretFireAngleSweep") {
        spec.fire_angle_sweep = sweep;
    }
    if let Some(modif) = parse_sweep_speed_attr(module, "TurretSweepSpeedModifier") {
        spec.sweep_speed_mod = modif;
    }
    spec
}

fn turret_spawn_from_name_honesty(template_name: &str) -> TurretSpawnSpec {
    use crate::game_logic::host_base_defense::{
        is_gattling_cannon_structure, is_patriot_battery_structure,
    };
    use crate::game_logic::host_battlemaster::BATTLE_MASTER_TURRET_TURN_RATE;
    use crate::game_logic::host_gattling_tank::is_gattling_tank_template;
    use crate::game_logic::host_humvee::{
        HUMVEE_TURRET_RECENTER_FRAMES, HUMVEE_TURRET_TURN_RATE, is_humvee_template,
    };
    use crate::game_logic::host_neutron_shell::is_nuke_cannon_template;
    use crate::game_logic::host_nuke_cannon::NUKE_CANNON_TURRET_TURN_RATE;
    use crate::game_logic::host_strategy_center::{
        STRATEGY_CENTER_FIRE_PITCH_DEG, STRATEGY_CENTER_MAX_IDLE_SCAN_ANGLE_DEG,
        STRATEGY_CENTER_MAX_IDLE_SCAN_INTERVAL_FRAMES, STRATEGY_CENTER_MIN_IDLE_SCAN_ANGLE_DEG,
        STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES, STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG,
        STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG, STRATEGY_CENTER_RECENTER_TIME_FRAMES,
        STRATEGY_CENTER_TURRET_PITCH_RATE_DEG_PER_SEC,
        STRATEGY_CENTER_TURRET_TURN_RATE_DEG_PER_SEC, is_strategy_center_template,
    };
    use crate::game_logic::host_technical::{TECHNICAL_TURRET_TURN_RATE, is_technical_template};
    use crate::game_logic::host_tomahawk::{
        TOMAHAWK_FIRE_PITCH, TOMAHAWK_TURRET_PITCH_RATE, TOMAHAWK_TURRET_TURN_RATE,
        is_tomahawk_template,
    };
    use crate::game_logic::host_tunnel_network::TUNNEL_NETWORK_TURRET_TURN_RATE;
    use crate::game_logic::host_usa_tanks::USA_TANK_TURRET_TURN_RATE;

    if is_strategy_center_template(template_name) {
        let mut spec =
            TurretSpawnSpec::enabled_with_turn_rate(STRATEGY_CENTER_TURRET_TURN_RATE_DEG_PER_SEC);
        spec.pitch_rate_rad =
            turret_deg_per_sec_to_rad_per_frame(STRATEGY_CENTER_TURRET_PITCH_RATE_DEG_PER_SEC);
        spec.natural_angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        spec.natural_pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        spec.angle_deg = STRATEGY_CENTER_NATURAL_TURRET_ANGLE_DEG;
        spec.pitch_deg = STRATEGY_CENTER_NATURAL_TURRET_PITCH_DEG;
        spec.enabled = false;
        spec.allows_pitch = true;
        spec.fire_pitch_rad = STRATEGY_CENTER_FIRE_PITCH_DEG.to_radians();
        spec.min_idle_scan_angle_rad = STRATEGY_CENTER_MIN_IDLE_SCAN_ANGLE_DEG.to_radians();
        spec.max_idle_scan_angle_rad = STRATEGY_CENTER_MAX_IDLE_SCAN_ANGLE_DEG.to_radians();
        spec.min_idle_scan_interval = STRATEGY_CENTER_MIN_IDLE_SCAN_INTERVAL_FRAMES;
        spec.max_idle_scan_interval = STRATEGY_CENTER_MAX_IDLE_SCAN_INTERVAL_FRAMES;
        spec.recenter_frames = STRATEGY_CENTER_RECENTER_TIME_FRAMES;
        return spec;
    }

    if is_tomahawk_template(template_name) {
        let mut spec = TurretSpawnSpec::enabled_with_turn_rate(TOMAHAWK_TURRET_TURN_RATE);
        spec.pitch_rate_rad = turret_deg_per_sec_to_rad_per_frame(TOMAHAWK_TURRET_PITCH_RATE);
        spec.allows_pitch = true;
        spec.fire_pitch_rad = TOMAHAWK_FIRE_PITCH.to_radians();
        spec.min_idle_scan_angle_rad = 0.0;
        spec.max_idle_scan_angle_rad = 45.0_f32.to_radians();
        spec.min_idle_scan_interval = 15;
        spec.max_idle_scan_interval = 30;
        return spec;
    }

    if is_humvee_template(template_name) {
        let mut spec = TurretSpawnSpec::enabled_with_turn_rate(HUMVEE_TURRET_TURN_RATE);
        spec.recenter_frames = HUMVEE_TURRET_RECENTER_FRAMES;
        return spec;
    }

    if is_technical_template(template_name) {
        return TurretSpawnSpec::enabled_with_turn_rate(TECHNICAL_TURRET_TURN_RATE);
    }

    if is_nuke_cannon_template(template_name) {
        let mut spec = TurretSpawnSpec::enabled_with_turn_rate(NUKE_CANNON_TURRET_TURN_RATE);
        spec.allows_pitch = true;
        return spec;
    }

    if is_usa_tank_name(template_name) {
        return TurretSpawnSpec::enabled_with_turn_rate(USA_TANK_TURRET_TURN_RATE);
    }

    if is_battlemaster_name(template_name) {
        return TurretSpawnSpec::enabled_with_turn_rate(BATTLE_MASTER_TURRET_TURN_RATE);
    }

    if is_tunnel_network_name(template_name) {
        return TurretSpawnSpec::enabled_with_turn_rate(TUNNEL_NETWORK_TURRET_TURN_RATE);
    }

    if is_patriot_battery_structure(template_name) {
        return TurretSpawnSpec::enabled_with_turn_rate(USA_TANK_TURRET_TURN_RATE);
    }

    if is_gattling_cannon_structure(template_name) || is_gattling_tank_template(template_name) {
        let mut spec = TurretSpawnSpec::enabled_with_turn_rate(USA_TANK_TURRET_TURN_RATE);
        let sweep = GATTLING_FIRE_ANGLE_SWEEP_DEG.to_radians();
        spec.fire_angle_sweep = [sweep, sweep, 0.0];
        spec.fires_while_turning = true;
        return spec;
    }

    if is_quad_cannon_name(template_name) {
        let mut spec = TurretSpawnSpec::enabled_with_turn_rate(USA_TANK_TURRET_TURN_RATE);
        let sweep = GATTLING_FIRE_ANGLE_SWEEP_DEG.to_radians();
        spec.fire_angle_sweep = [sweep, sweep, 0.0];
        spec.fires_while_turning = true;
        return spec;
    }

    if is_overlord_name(template_name)
        || is_scorpion_name(template_name)
        || is_inferno_name(template_name)
    {
        return TurretSpawnSpec::enabled_with_turn_rate(USA_TANK_TURRET_TURN_RATE);
    }

    TurretSpawnSpec::absent()
}

fn is_usa_tank_name(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.is_empty()
        || n.contains("weapon")
        || n.contains("shell")
        || n.contains("projectile")
        || n.contains("upgrade")
        || n.contains("science")
    {
        return false;
    }
    n.contains("crusader")
        || n.contains("paladin")
        || n == "testtank"
        || (n.contains("tank")
            && (n.contains("america") || n.contains("usa"))
            && !n.contains("humvee"))
}

fn is_battlemaster_name(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    n.contains("battlemaster") && !n.contains("weapon") && !n.contains("shell")
}

fn is_tunnel_network_name(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    if n.contains("weapon") || n.contains("gun") || n.contains("hole") {
        return false;
    }
    n.contains("tunnelnetwork")
}

fn is_quad_cannon_name(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    (n.contains("quadcannon") || n.contains("quadcannon"))
        && !n.contains("weapon")
        && !n.contains("shell")
}

fn is_overlord_name(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    n.contains("overlord")
        && !n.contains("weapon")
        && !n.contains("addon")
        && !n.contains("gattling")
        && !n.contains("helix")
}

fn is_scorpion_name(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    n.contains("scorpion") && !n.contains("weapon") && !n.contains("shell") && !n.contains("rocket")
}

fn is_inferno_name(template_name: &str) -> bool {
    let n = alnum_lower(template_name);
    n.contains("inferno") && (n.contains("tank") || n.contains("cannon") || n.contains("vehicle"))
}

fn alnum_lower(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

fn parse_f32_attr(module: &BehaviorModuleDefinition, name: &str) -> Option<f32> {
    let raw = module.attribute(name)?;
    let token = raw.split_whitespace().next()?;
    let v: f32 = token.parse().ok()?;
    v.is_finite().then_some(v)
}

fn parse_u32_attr(module: &BehaviorModuleDefinition, name: &str) -> Option<u32> {
    let raw = module.attribute(name)?;
    let token = raw.split_whitespace().next()?;
    token.parse().ok()
}

fn parse_bool_attr(module: &BehaviorModuleDefinition, name: &str) -> Option<bool> {
    let raw = module.attribute(name)?;
    match raw.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => Some(true),
        "no" | "false" | "0" => Some(false),
        _ => None,
    }
}

fn parse_sweep_attr(module: &BehaviorModuleDefinition, name: &str) -> Option<[f32; 3]> {
    let raw = module.attribute(name)?;
    let mut sweep = [0.0; 3];
    let mut tokens = raw.split_whitespace();
    let first = tokens.next()?;
    if let Ok(deg) = first.parse::<f32>() {
        sweep[0] = deg.to_radians();
        return Some(sweep);
    }
    let slot = weapon_slot_index(first)?;
    let deg: f32 = tokens.next()?.parse().ok()?;
    sweep[slot] = deg.to_radians();
    Some(sweep)
}

fn parse_sweep_speed_attr(module: &BehaviorModuleDefinition, name: &str) -> Option<[f32; 3]> {
    let raw = module.attribute(name)?;
    let mut mods = [1.0; 3];
    let mut tokens = raw.split_whitespace();
    let first = tokens.next()?;
    if let Ok(v) = first.parse::<f32>() {
        mods[0] = v;
        return Some(mods);
    }
    let slot = weapon_slot_index(first)?;
    let v: f32 = tokens.next()?.parse().ok()?;
    mods[slot] = v;
    Some(mods)
}

fn weapon_slot_index(token: &str) -> Option<usize> {
    match token.to_ascii_uppercase().as_str() {
        "PRIMARY" => Some(0),
        "SECONDARY" => Some(1),
        "TERTIARY" => Some(2),
        _ => None,
    }
}
