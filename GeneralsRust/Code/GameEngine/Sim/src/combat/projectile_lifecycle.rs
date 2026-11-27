//! DumbProjectile and MissileAI lifetime rules from the admitted Object INI.

use crate::time::LOGIC_FRAMES_PER_SECOND;
use game_engine::common::thing::module::ModuleData as EngineModuleData;

const DUMB_PROJECTILE_DEFAULT_MAX_LIFESPAN_FRAMES: u32 = 10 * crate::time::LOGIC_HZ;
const MISSILE_DEFAULT_KILL_SELF_DELAY_FRAMES: u32 = 3;

/// Authored lifetime and detonation behavior frozen when a shot is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename = "HostProjectileLifecycle")]
pub enum ProjectileLifecycle {
    /// `DumbProjectileBehavior`: expiry calls `detonate()`.
    DumbProjectile {
        max_lifespan_frames: u32,
        detonate_calls_kill: bool,
    },
    /// `MissileAIUpdate`: fuel and followed-target loss have distinct results.
    Missile {
        try_to_follow_target: bool,
        /// Zero is C++'s unlimited-fuel value.
        fuel_lifetime_frames: u32,
        detonate_on_no_fuel: bool,
        kill_self_delay_frames: u32,
    },
}

impl ProjectileLifecycle {
    pub fn detonate_calls_kill(self) -> bool {
        matches!(
            self,
            Self::DumbProjectile {
                detonate_calls_kill: true,
                ..
            }
        )
    }

    /// Whether this authored behavior tracks an object target in flight.
    pub fn follows_target(self) -> bool {
        matches!(
            self,
            Self::Missile {
                try_to_follow_target: true,
                ..
            }
        )
    }

    /// Presentation/debug duration in seconds.  Zero means no finite generic
    /// lifetime (unresolved behavior or a missile with unlimited fuel).
    pub fn lifetime_seconds(self) -> f32 {
        let frames = match self {
            Self::DumbProjectile {
                max_lifespan_frames,
                ..
            } => max_lifespan_frames,
            Self::Missile {
                fuel_lifetime_frames,
                ..
            } => fuel_lifetime_frames,
        };
        frames as f32 / LOGIC_FRAMES_PER_SECOND
    }
}

pub fn parse_dumb_projectile_lifecycle(data: &dyn EngineModuleData) -> Option<ProjectileLifecycle> {
    let max_lifespan_frames = optional_duration_frames(data, "MaxLifespan")?
        .unwrap_or(DUMB_PROJECTILE_DEFAULT_MAX_LIFESPAN_FRAMES);
    let detonate_calls_kill = optional_bool(data, "DetonateCallsKill")?.unwrap_or(false);
    Some(ProjectileLifecycle::DumbProjectile {
        max_lifespan_frames,
        detonate_calls_kill,
    })
}

pub fn parse_missile_lifecycle(data: &dyn EngineModuleData) -> Option<ProjectileLifecycle> {
    let try_to_follow_target = optional_bool(data, "TryToFollowTarget")?.unwrap_or(true);
    let fuel_lifetime_frames = optional_duration_frames(data, "FuelLifetime")?.unwrap_or(0);
    let detonate_on_no_fuel = optional_bool(data, "DetonateOnNoFuel")?.unwrap_or(false);
    let kill_self_delay_frames = optional_duration_frames(data, "KillSelfDelay")?
        .unwrap_or(MISSILE_DEFAULT_KILL_SELF_DELAY_FRAMES);
    Some(ProjectileLifecycle::Missile {
        try_to_follow_target,
        fuel_lifetime_frames,
        detonate_on_no_fuel,
        kill_self_delay_frames,
    })
}

/// C++ `INI::parseDurationUnsignedInt`: an unsigned millisecond value, turned
/// into logic frames with an upward round. `scanUnsignedInt` uses `sscanf("%u")`,
/// which accepts a decimal numeric prefix and ignores a trailing suffix.  Thus
/// retail/mod values such as `350ms` still mean 350 milliseconds; conversely,
/// this must not adopt the generic Rust duration parser's unit semantics (for
/// example, C++ reads `1s` as the numeric prefix 1, not one second).
fn parse_cpp_duration_unsigned_int(value: &str) -> Option<u32> {
    let value = value.trim_start();
    let digits = value
        .bytes()
        .take_while(u8::is_ascii_digit)
        .map(char::from)
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    let milliseconds = digits.parse::<u64>().ok()?;
    if milliseconds > u32::MAX as u64 {
        // C++ scanf overflow is implementation-defined. Do not invent a
        // wrapped duration for malformed authored data.
        return None;
    }
    let frames = (milliseconds as f64 * LOGIC_FRAMES_PER_SECOND as f64 / 1000.0).ceil();
    (frames <= u32::MAX as f64).then_some(frames as u32)
}

pub fn optional_duration_frames(data: &dyn EngineModuleData, field: &str) -> Option<Option<u32>> {
    match data.get_ini_field(field) {
        Some(value) => parse_cpp_duration_unsigned_int(value).map(Some),
        None => Some(None),
    }
}

pub fn optional_bool(data: &dyn EngineModuleData, field: &str) -> Option<Option<bool>> {
    let Some(value) = data.get_ini_field(field) else {
        return Some(None);
    };
    let parsed = match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "true" | "1" => true,
        "no" | "false" | "0" => false,
        _ => return None,
    };
    Some(Some(parsed))
}

pub fn optional_real(data: &dyn EngineModuleData, field: &str) -> Option<Option<f32>> {
    let Some(value) = data.get_ini_field(field) else {
        return Some(None);
    };
    value.trim().parse::<f32>().ok().map(Some)
}

pub fn optional_percent(data: &dyn EngineModuleData, field: &str) -> Option<Option<f32>> {
    let Some(value) = data.get_ini_field(field) else {
        return Some(None);
    };
    let trimmed = value.trim().trim_end_matches('%').trim();
    trimmed.parse::<f32>().ok().map(|n| Some(n / 100.0))
}

pub fn optional_velocity_per_frame(
    data: &dyn EngineModuleData,
    field: &str,
) -> Option<Option<f32>> {
    let Some(value) = data.get_ini_field(field) else {
        return Some(None);
    };
    value
        .trim()
        .parse::<f32>()
        .ok()
        .map(|per_second| Some(per_second / LOGIC_FRAMES_PER_SECOND))
}

pub fn optional_int(data: &dyn EngineModuleData, field: &str) -> Option<Option<i32>> {
    let Some(value) = data.get_ini_field(field) else {
        return Some(None);
    };
    let digits: String = value
        .trim()
        .bytes()
        .take_while(|b| b.is_ascii_digit() || *b == b'-')
        .map(char::from)
        .collect();
    if digits.is_empty() || digits == "-" {
        return None;
    }
    digits.parse::<i32>().ok().map(Some)
}

pub fn optional_string(data: &dyn EngineModuleData, field: &str) -> Option<String> {
    data.get_ini_field(field)
        .map(|value| value.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpp_duration_parser_rounds_up_to_logic_frames() {
        assert_eq!(parse_cpp_duration_unsigned_int("1"), Some(1));
        assert_eq!(parse_cpp_duration_unsigned_int("34"), Some(2));
        assert_eq!(parse_cpp_duration_unsigned_int("350"), Some(11));
        assert_eq!(parse_cpp_duration_unsigned_int("350ms"), Some(11));
        assert_eq!(parse_cpp_duration_unsigned_int("1000"), Some(30));
        assert_eq!(parse_cpp_duration_unsigned_int("1s"), Some(1));
        assert_eq!(parse_cpp_duration_unsigned_int("milliseconds"), None);
    }
}
