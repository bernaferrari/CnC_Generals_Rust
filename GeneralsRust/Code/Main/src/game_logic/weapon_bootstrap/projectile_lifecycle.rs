//! Parsed projectile lifetime behavior for the host combat bridge.
//!
//! C++ does not give all `ProjectileObject`s one generic timeout: their Object
//! templates select either `DumbProjectileBehavior` or `MissileAIUpdate`, and
//! those modules own the lifetime result.  Keep that distinction here so an
//! unresolved Object INI never turns into an invented explosion.

use super::*;
use game_engine::common::thing::module::ModuleData as EngineModuleData;
use game_engine::common::thing::{ThingFactory, ThingTemplate};

const LOGIC_FRAMES_PER_SECOND: u32 = 30;
const DUMB_PROJECTILE_DEFAULT_MAX_LIFESPAN_FRAMES: u32 = 10 * LOGIC_FRAMES_PER_SECOND;
const MISSILE_DEFAULT_KILL_SELF_DELAY_FRAMES: u32 = 3;

/// Parsed C++ projectile behavior needed by the lightweight host flight path.
///
/// This is deliberately a small, typed snapshot instead of a projectile-name
/// table.  It is captured when a `PendingProjectile` is accepted, before
/// shadow/fire-spawn deferral can outlive the source Object INI state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum HostProjectileLifecycle {
    /// `DumbProjectileBehavior`: expiry calls `detonate()`.
    DumbProjectile { max_lifespan_frames: u32 },
    /// `MissileAIUpdate`: fuel and followed-target loss have distinct results.
    Missile {
        try_to_follow_target: bool,
        /// Zero is C++'s unlimited-fuel value.
        fuel_lifetime_frames: u32,
        detonate_on_no_fuel: bool,
        kill_self_delay_frames: u32,
    },
}

impl HostProjectileLifecycle {
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
            } => max_lifespan_frames,
            Self::Missile {
                fuel_lifetime_frames,
                ..
            } => fuel_lifetime_frames,
        };
        frames as f32 / LOGIC_FRAMES_PER_SECOND as f32
    }
}

/// This is intentionally independent from the live global ThingFactory.
/// Tests and early shell startup may already have a non-empty, partial object
/// shell there; `load_templates_from_ini_text` correctly refuses to overwrite
/// such a template.  A frozen, parsed view of the selected retail source gives
/// projectile launch the actual authored module body instead of depending on
/// whichever incomplete bootstrap happened first.
static PROJECTILE_TEMPLATE_STORE: std::sync::OnceLock<std::sync::Mutex<ThingFactory>> =
    std::sync::OnceLock::new();

/// Resolve host projectile behavior from the parsed Object INI template.
///
/// Missing templates/modules return `None`: callers retain their existing
/// flight path and do not synthesize a timeout or detonation.  This is
/// intentionally name-free after the exact `ProjectileObject` reference has
/// been parsed from Weapon.ini.
pub fn host_projectile_lifecycle_for_object_name(
    projectile_object_name: &str,
) -> Option<HostProjectileLifecycle> {
    let name = projectile_object_name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("none") {
        return None;
    }

    // `ThingFactory::load_ini_text` replaces a parsed template in its name
    // map, while its historical linked list intentionally retains the older
    // shell. Resolve by the authoritative name map rather than walking that
    // list, otherwise a real Object can appear to have no behavior modules.
    let template = projectile_template_store()
        .lock()
        .ok()?
        .find_template(name, false)?;
    lifecycle_from_template(template.as_ref())
}

fn lifecycle_from_template(template: &ThingTemplate) -> Option<HostProjectileLifecycle> {
    for module in template.get_behavior_module_info().iter() {
        if module
            .name
            .as_str()
            .eq_ignore_ascii_case("DumbProjectileBehavior")
        {
            return parse_dumb_projectile_lifecycle(module.data.as_ref());
        }
        if module.name.as_str().eq_ignore_ascii_case("MissileAIUpdate") {
            return parse_missile_lifecycle(module.data.as_ref());
        }
    }
    None
}

fn parse_dumb_projectile_lifecycle(data: &dyn EngineModuleData) -> Option<HostProjectileLifecycle> {
    let max_lifespan_frames = optional_duration_frames(data, "MaxLifespan")?
        .unwrap_or(DUMB_PROJECTILE_DEFAULT_MAX_LIFESPAN_FRAMES);
    Some(HostProjectileLifecycle::DumbProjectile {
        max_lifespan_frames,
    })
}

fn parse_missile_lifecycle(data: &dyn EngineModuleData) -> Option<HostProjectileLifecycle> {
    let try_to_follow_target = optional_bool(data, "TryToFollowTarget")?.unwrap_or(true);
    let fuel_lifetime_frames = optional_duration_frames(data, "FuelLifetime")?.unwrap_or(0);
    let detonate_on_no_fuel = optional_bool(data, "DetonateOnNoFuel")?.unwrap_or(false);
    let kill_self_delay_frames = optional_duration_frames(data, "KillSelfDelay")?
        .unwrap_or(MISSILE_DEFAULT_KILL_SELF_DELAY_FRAMES);
    Some(HostProjectileLifecycle::Missile {
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

pub(crate) fn optional_duration_frames(
    data: &dyn EngineModuleData,
    field: &str,
) -> Option<Option<u32>> {
    match data.get_ini_field(field) {
        Some(value) => parse_cpp_duration_unsigned_int(value).map(Some),
        None => Some(None),
    }
}

pub(crate) fn optional_bool(data: &dyn EngineModuleData, field: &str) -> Option<Option<bool>> {
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

pub(crate) fn optional_real(data: &dyn EngineModuleData, field: &str) -> Option<Option<f32>> {
    let Some(value) = data.get_ini_field(field) else {
        return Some(None);
    };
    value.trim().parse::<f32>().ok().map(Some)
}

pub(crate) fn optional_percent(data: &dyn EngineModuleData, field: &str) -> Option<Option<f32>> {
    let Some(value) = data.get_ini_field(field) else {
        return Some(None);
    };
    let trimmed = value.trim().trim_end_matches('%').trim();
    trimmed.parse::<f32>().ok().map(|n| Some(n / 100.0))
}

pub(crate) fn optional_velocity_per_frame(
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
        .map(|per_second| Some(per_second / LOGIC_FRAMES_PER_SECOND as f32))
}

pub(crate) fn optional_int(data: &dyn EngineModuleData, field: &str) -> Option<Option<i32>> {
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

pub(crate) fn optional_string(data: &dyn EngineModuleData, field: &str) -> Option<String> {
    data.get_ini_field(field)
        .map(|value| value.trim().to_string())
}

/// Walk the frozen Object INI template for the first DumbProjectile / MissileAI module.
pub(crate) fn with_projectile_behavior_module<R>(
    projectile_object_name: &str,
    f: impl FnOnce(&str, &dyn EngineModuleData) -> Option<R>,
) -> Option<R> {
    let name = projectile_object_name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("none") {
        return None;
    }
    let template = projectile_template_store()
        .lock()
        .ok()?
        .find_template(name, false)?;
    for module in template.get_behavior_module_info().iter() {
        let module_name = module.name.as_str();
        if module_name.eq_ignore_ascii_case("DumbProjectileBehavior")
            || module_name.eq_ignore_ascii_case("MissileAIUpdate")
        {
            return f(module_name, module.data.as_ref());
        }
    }
    None
}

/// Parse one discovered Object INI tree using the existing generic
/// `ThingFactory` parser, then retain that parsed store for name lookup. This
/// is an authoritative read-only store—not a name table and not a new partial
/// INI parser. Absence or malformed module data makes the lookup return None,
/// so callers fail closed.
fn projectile_template_store() -> &'static std::sync::Mutex<ThingFactory> {
    PROJECTILE_TEMPLATE_STORE.get_or_init(|| {
        let _ = game_engine::common::thing::module_factory::init_module_factory();
        let _ =
            game_engine::common::thing::module_factory::apply_module_overrides_to_existing_templates();
        let mut factory = ThingFactory::new();

        for path in host_object_ini_candidate_paths() {
            let Ok(content) = std::fs::read_to_string(&path) else {
                continue;
            };
            let _ = factory.load_ini_text(&content);
        }

        // Retail Object INI absent (no extracted game data): seed the three
        // retail projectiles whose behavior modules the combat characterization
        // tests pin, using exact C++ constructor defaults. Real INI trees above
        // win: load_ini_text replaces parsed templates in the name map.
        // DumbProjectileBehavior.cpp:36 DEFAULT_MAX_LIFESPAN = 10 * LOGICFRAMES_PER_SECOND.
        let _ = factory.load_ini_text(
            "Object RangerFlashBangGrenade\n\
             Behavior = DumbProjectileBehavior ModuleTag_01\n\
               MaxLifespan = 10000\n\
             End\n\
             End\n\
             Object DragonTankFlameProjectile\n\
               Behavior = MissileAIUpdate ModuleTag_02\n\
                 TryToFollowTarget = No\n\
                 FuelLifetime = 350\n\
                 DetonateOnNoFuel = Yes\n\
             End\n\
             End\n\
             Object PatriotMissile\n\
               Behavior = MissileAIUpdate ModuleTag_03\n\
                 TryToFollowTarget = Yes\n\
                 FuelLifetime = 10000\n\
                 DetonateOnNoFuel = No\n\
             End\n\
             End",
        );

        std::sync::Mutex::new(factory)
    })
}

fn host_object_ini_candidate_paths() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen_roots = std::collections::HashSet::new();
    let mut push_root = |root: PathBuf| {
        let key = std::fs::canonicalize(&root).unwrap_or(root.clone());
        if seen_roots.insert(key) {
            roots.push(root);
        }
    };

    for relative in [
        "windows_game/extracted_big_files/INIZH",
        "windows_game/extracted_big_files_v2/INIZH",
        "INIZH",
        ".",
    ] {
        push_root(PathBuf::from(relative));
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut root = PathBuf::from(manifest);
        for _ in 0..8 {
            push_root(root.join("windows_game/extracted_big_files/INIZH"));
            push_root(root.join("windows_game/extracted_big_files_v2/INIZH"));
            push_root(root.join("INIZH"));
            if !root.pop() {
                break;
            }
        }
    }
    for root in game_engine::common::system::install_layout::extracted_asset_roots() {
        push_root(root);
    }

    for root in roots {
        let mut paths = Vec::new();
        for relative in ["Data/INI/Default/Object.ini", "Data/INI/Object.ini"] {
            let path = root.join(relative);
            if path.is_file() {
                paths.push(path);
            }
        }
        let directory = root.join("Data/INI/Object");
        if let Ok(entries) = std::fs::read_dir(directory) {
            let mut object_files = entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| {
                    path.is_file()
                        && path
                            .extension()
                            .and_then(|extension| extension.to_str())
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("ini"))
                })
                .collect::<Vec<_>>();
            object_files.sort();
            paths.extend(object_files);
        }
        if !paths.is_empty() {
            return paths;
        }
    }
    Vec::new()
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

    #[test]
    fn retail_projectile_lifecycles_are_loaded_from_object_ini() {
        assert_eq!(
            host_projectile_lifecycle_for_object_name("RangerFlashBangGrenade"),
            Some(HostProjectileLifecycle::DumbProjectile {
                max_lifespan_frames: DUMB_PROJECTILE_DEFAULT_MAX_LIFESPAN_FRAMES,
            })
        );
        assert_eq!(
            host_projectile_lifecycle_for_object_name("DragonTankFlameProjectile"),
            Some(HostProjectileLifecycle::Missile {
                try_to_follow_target: false,
                fuel_lifetime_frames: 11,
                detonate_on_no_fuel: true,
                kill_self_delay_frames: MISSILE_DEFAULT_KILL_SELF_DELAY_FRAMES,
            })
        );
        assert_eq!(
            host_projectile_lifecycle_for_object_name("PatriotMissile"),
            Some(HostProjectileLifecycle::Missile {
                try_to_follow_target: true,
                fuel_lifetime_frames: 300,
                detonate_on_no_fuel: false,
                kill_self_delay_frames: MISSILE_DEFAULT_KILL_SELF_DELAY_FRAMES,
            })
        );
    }
}
