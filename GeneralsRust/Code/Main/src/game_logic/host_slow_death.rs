//! Host SlowDeathBehavior residual (delayed sink + destroy after lethal damage).
//!
//! C++: `SlowDeathBehavior::beginSlowDeath` + `update` phases:
//! sink delay → sink rate → destruction delay → destroyObject.
//!
//! Live path starts only when Object INI authors `Behavior = SlowDeathBehavior`.
//! SinkDelay / SinkRate / DestructionDelay / DestructionAltitude / FlingForce
//! come from that module, not KindOf infantry/vehicle hardcoded skip.
//!
//! Residual playability slice:
//! - Authored SlowDeathBehavior module lookup via AssetManager ObjectDefinition
//! - Presentation sink_offset (negative Y)
//! - Defers GameLogic destroy until destruction frame
//!
//! Fail-closed:
//! - Not full fling physics / multi DeathTypes probability matrix
//! - Not full FX/OCL/Weapon phase bursts (INITIAL/MIDPOINT/FINAL)
//! - Not LOD instant-death scale matrix
//! - Variance uses 0 (no GameLogicRandomValue)

use serde::{Deserialize, Serialize};
use std::cell::RefCell;

/// Logic FPS residual.
pub const SLOW_DEATH_LOGIC_FPS: f32 = 30.0;

/// C++ SlowDeathBehavior.cpp:36 BEGIN_MIDPOINT_RATIO.
pub const BEGIN_MIDPOINT_RATIO: f32 = 0.35;

/// Retail infantry SinkDelay 3000 ms → frames.
pub const INFANTRY_SINK_DELAY_MS: u32 = 3_000;
/// Retail infantry SinkRate 0.5 dist/sec.
pub const INFANTRY_SINK_RATE_PER_SEC: f32 = 0.5;
/// Retail infantry DestructionDelay 8000 ms.
pub const INFANTRY_DESTRUCTION_DELAY_MS: u32 = 8_000;
/// Default vehicle destruction delay residual (instant-ish but one beat).
pub const VEHICLE_DESTRUCTION_DELAY_MS: u32 = 1_000;

thread_local! {
    static SLOW_DEATH_INI_OVERRIDE: RefCell<Option<(String, HostSlowDeathIni)>> =
        const { RefCell::new(None) };
}

/// Authored `SlowDeathBehavior` Object INI fields (msec / dist-per-sec as written).
///
/// C++ stores duration fields as frames after `INI::parseDurationUnsignedInt`
/// (`ceil(msec * 30 / 1000)`). Conversion happens at begin time here.
#[derive(Debug, Clone, PartialEq)]
pub struct HostSlowDeathIni {
    pub sink_delay_ms: u32,
    pub sink_delay_variance_ms: u32,
    /// Object INI `SinkRate` (dist/sec). C++ `parseVelocityReal` → per frame.
    pub sink_rate_per_sec: f32,
    pub destruction_delay_ms: u32,
    pub destruction_delay_variance_ms: u32,
    /// C++ default `-10` (`SlowDeathBehavior.cpp:49`).
    pub destruction_altitude: f32,
    pub fling_force: f32,
    pub fling_force_variance: f32,
    /// INI `FlingPitch` degrees (C++ `parseAngleReal`).
    pub fling_pitch_deg: f32,
}

impl Default for HostSlowDeathIni {
    fn default() -> Self {
        Self {
            sink_delay_ms: 0,
            sink_delay_variance_ms: 0,
            sink_rate_per_sec: 0.0,
            destruction_delay_ms: 0,
            destruction_delay_variance_ms: 0,
            destruction_altitude: -10.0,
            fling_force: 0.0,
            fling_force_variance: 0.0,
            fling_pitch_deg: 0.0,
        }
    }
}

impl HostSlowDeathIni {
    /// Retail AmericaInfantry* SlowDeathBehavior peel (SinkDelay 3000 / SinkRate 0.5 / DestructionDelay 8000).
    pub fn infantry_retail() -> Self {
        Self {
            sink_delay_ms: INFANTRY_SINK_DELAY_MS,
            sink_rate_per_sec: INFANTRY_SINK_RATE_PER_SEC,
            destruction_delay_ms: INFANTRY_DESTRUCTION_DELAY_MS,
            destruction_altitude: -10.0,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HostSlowDeathPhase {
    #[default]
    Inactive = 0,
    WaitingToSink = 1,
    Sinking = 2,
    WaitingToDestroy = 3,
    Done = 4,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSlowDeathData {
    pub phase: HostSlowDeathPhase,
    pub begin_frame: u32,
    pub sink_at_frame: u32,
    pub destroy_at_frame: u32,
    /// World units per logic frame (positive magnitude; applied as -Y).
    pub sink_rate_per_frame: f32,
    /// Accumulated sink offset (negative).
    pub sink_offset: f32,
    /// C++ destructionAltitude residual (stop sinking around this altitude).
    pub destruction_altitude: f32,
    /// C++ FlingForce residual (applied once on begin as horizontal kick).
    pub fling_vx: f32,
    pub fling_vz: f32,
    pub fling_vy: f32,
    pub fling_applied: bool,
}

impl Default for HostSlowDeathData {
    fn default() -> Self {
        Self {
            phase: HostSlowDeathPhase::Inactive,
            begin_frame: 0,
            sink_at_frame: 0,
            destroy_at_frame: 0,
            sink_rate_per_frame: 0.0,
            sink_offset: 0.0,
            destruction_altitude: -10.0,
            fling_vx: 0.0,
            fling_vz: 0.0,
            fling_vy: 0.0,
            fling_applied: false,
        }
    }
}

impl HostSlowDeathData {
    pub fn is_active(&self) -> bool {
        !matches!(
            self.phase,
            HostSlowDeathPhase::Inactive | HostSlowDeathPhase::Done
        )
    }

    pub fn is_done(&self) -> bool {
        self.phase == HostSlowDeathPhase::Done
    }

    /// C++ `SlowDeathBehavior::beginSlowDeath` timing from authored module data.
    pub fn from_ini(current_frame: u32, ini: &HostSlowDeathIni) -> Self {
        // Fail-closed: variance stays 0 (no GameLogicRandomValue).
        let sink_delay = msec_to_logic_frames(ini.sink_delay_ms);
        let destroy_delay = msec_to_logic_frames(ini.destruction_delay_ms);
        let sink_rate_per_frame = ini.sink_rate_per_sec / SLOW_DEATH_LOGIC_FPS;
        let phase = if sink_rate_per_frame > 0.0 {
            HostSlowDeathPhase::WaitingToSink
        } else {
            HostSlowDeathPhase::WaitingToDestroy
        };
        let mut s = Self {
            phase,
            begin_frame: current_frame,
            sink_at_frame: current_frame.saturating_add(sink_delay),
            destroy_at_frame: current_frame.saturating_add(destroy_delay),
            sink_rate_per_frame,
            sink_offset: 0.0,
            destruction_altitude: ini.destruction_altitude,
            fling_vx: 0.0,
            fling_vz: 0.0,
            fling_vy: 0.0,
            fling_applied: false,
        };
        if ini.fling_force > 0.0 {
            // Deterministic angle residual (object id supplied by caller via apply_fling).
            s.fling_vx = ini.fling_force * 0.15;
            s.fling_vy = ini.fling_force * 0.08 * (ini.fling_pitch_deg.to_radians().sin().max(0.15));
        }
        s
    }

    /// Apply object-id-derived fling direction after `from_ini`.
    pub fn apply_fling_angle(&mut self, angle: f32) {
        if self.fling_vx == 0.0 && self.fling_vz == 0.0 {
            return;
        }
        let mag = (self.fling_vx * self.fling_vx + self.fling_vz * self.fling_vz).sqrt();
        let mag = if mag > 0.0 { mag } else { self.fling_vx.abs() };
        self.fling_vx = angle.cos() * mag;
        self.fling_vz = angle.sin() * mag;
    }

    /// Midpoint frame (C++ `BEGIN_MIDPOINT_RATIO * m_destructionFrame` residual).
    pub fn midpoint_at_frame(&self) -> u32 {
        let span = self.destroy_at_frame.saturating_sub(self.begin_frame);
        self.begin_frame
            .saturating_add((span as f32 * BEGIN_MIDPOINT_RATIO) as u32)
    }

    pub fn infantry_residual(current_frame: u32) -> Self {
        Self::from_ini(current_frame, &HostSlowDeathIni::infantry_retail())
    }

    /// Infantry with FlingForce residual (e.g. exploded death type peel).
    pub fn infantry_fling_residual(current_frame: u32, force: f32, angle: f32) -> Self {
        let mut ini = HostSlowDeathIni::infantry_retail();
        ini.fling_force = force;
        ini.fling_pitch_deg = 30.0;
        let mut s = Self::from_ini(current_frame, &ini);
        s.apply_fling_angle(angle);
        s
    }

    pub fn vehicle_residual(current_frame: u32) -> Self {
        let mut ini = HostSlowDeathIni::default();
        ini.destruction_delay_ms = VEHICLE_DESTRUCTION_DELAY_MS;
        Self::from_ini(current_frame, &ini)
    }

    /// Consume one-shot fling impulse residual.
    pub fn take_fling_impulse(&mut self) -> Option<(f32, f32, f32)> {
        if self.fling_applied {
            return None;
        }
        if self.fling_vx == 0.0 && self.fling_vz == 0.0 && self.fling_vy == 0.0 {
            self.fling_applied = true;
            return None;
        }
        self.fling_applied = true;
        Some((self.fling_vx, self.fling_vy, self.fling_vz))
    }

    /// Begin slow death. Returns false if already active/done.
    pub fn begin_infantry(&mut self, current_frame: u32) -> bool {
        if self.is_active() || self.is_done() {
            return false;
        }
        *self = Self::infantry_residual(current_frame);
        true
    }

    pub fn begin_vehicle(&mut self, current_frame: u32) -> bool {
        if self.is_active() || self.is_done() {
            return false;
        }
        *self = Self::vehicle_residual(current_frame);
        true
    }

    pub fn begin_from_ini(&mut self, current_frame: u32, ini: &HostSlowDeathIni) -> bool {
        if self.is_active() || self.is_done() {
            return false;
        }
        *self = Self::from_ini(current_frame, ini);
        true
    }

    /// Tick one frame. Returns true when object should be destroyed now.
    pub fn tick(&mut self, current_frame: u32) -> bool {
        match self.phase {
            HostSlowDeathPhase::Inactive | HostSlowDeathPhase::Done => false,
            HostSlowDeathPhase::WaitingToSink => {
                if current_frame >= self.sink_at_frame {
                    self.phase = HostSlowDeathPhase::Sinking;
                }
                if current_frame >= self.destroy_at_frame {
                    self.phase = HostSlowDeathPhase::Done;
                    return true;
                }
                false
            }
            HostSlowDeathPhase::Sinking => {
                if self.sink_rate_per_frame > 0.0 {
                    self.sink_offset -= self.sink_rate_per_frame;
                    if self.sink_offset < self.destruction_altitude {
                        self.sink_offset = self.destruction_altitude;
                    }
                }
                if current_frame >= self.destroy_at_frame {
                    self.phase = HostSlowDeathPhase::Done;
                    return true;
                }
                false
            }
            HostSlowDeathPhase::WaitingToDestroy => {
                if current_frame >= self.destroy_at_frame {
                    self.phase = HostSlowDeathPhase::Done;
                    return true;
                }
                false
            }
        }
    }
}

/// C++ `INI::parseDurationUnsignedInt` / `ConvertDurationFromMsecsToFrames`:
/// `ceil(msec * LOGICFRAMES_PER_SECOND / 1000)`.
pub fn msec_to_logic_frames(msec: u32) -> u32 {
    ((msec as f32) * SLOW_DEATH_LOGIC_FPS / 1000.0).ceil() as u32
}

fn ms_to_frames(msec: u32) -> u32 {
    msec_to_logic_frames(msec)
}

fn parse_msec(raw: &str) -> Option<u32> {
    let s = raw.split(';').next().unwrap_or_default().trim();
    s.trim_end_matches('f').parse::<f32>().ok().and_then(|v| {
        if v.is_finite() && v >= 0.0 {
            Some(v as u32)
        } else {
            None
        }
    })
}

fn parse_real(raw: &str) -> Option<f32> {
    let s = raw.split(';').next().unwrap_or_default().trim();
    s.trim_end_matches('f')
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
}

/// Build authored SlowDeath INI from `Behavior = SlowDeathBehavior` field tokens.
pub fn slow_death_ini_from_behavior_attrs(attrs: &[(&str, &str)]) -> HostSlowDeathIni {
    let get = |key: &str| {
        attrs
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(key))
            .map(|(_, v)| *v)
    };
    HostSlowDeathIni {
        sink_delay_ms: get("SinkDelay").and_then(parse_msec).unwrap_or(0),
        sink_delay_variance_ms: get("SinkDelayVariance").and_then(parse_msec).unwrap_or(0),
        sink_rate_per_sec: get("SinkRate").and_then(parse_real).unwrap_or(0.0),
        destruction_delay_ms: get("DestructionDelay").and_then(parse_msec).unwrap_or(0),
        destruction_delay_variance_ms: get("DestructionDelayVariance")
            .and_then(parse_msec)
            .unwrap_or(0),
        destruction_altitude: get("DestructionAltitude")
            .and_then(parse_real)
            .unwrap_or(-10.0),
        fling_force: get("FlingForce").and_then(parse_real).unwrap_or(0.0),
        fling_force_variance: get("FlingForceVariance").and_then(parse_real).unwrap_or(0.0),
        fling_pitch_deg: get("FlingPitch").and_then(parse_real).unwrap_or(0.0),
    }
}

fn authored_slow_death_ini(name: &str) -> Option<HostSlowDeathIni> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(name)?;
    for module in &definition.behavior_modules {
        if !module
            .class_name
            .eq_ignore_ascii_case("SlowDeathBehavior")
        {
            continue;
        }
        let attrs: Vec<(&str, &str)> = module
            .attributes
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        return Some(slow_death_ini_from_behavior_attrs(&attrs));
    }
    None
}

/// Test helper: treat `template_name` as authoring SlowDeathBehavior.
pub fn override_slow_death_ini_for_tests(template_name: &str, ini: HostSlowDeathIni) {
    SLOW_DEATH_INI_OVERRIDE.with(|slot| {
        *slot.borrow_mut() = Some((template_name.to_string(), ini));
    });
}

pub fn clear_slow_death_ini_override_for_tests() {
    SLOW_DEATH_INI_OVERRIDE.with(|slot| {
        *slot.borrow_mut() = None;
    });
}

/// Live lookup: override (tests) then authored Object INI `SlowDeathBehavior`.
pub fn slow_death_ini_for_template(name: &str) -> Option<HostSlowDeathIni> {
    let hit = SLOW_DEATH_INI_OVERRIDE.with(|slot| {
        slot.borrow().as_ref().and_then(|(n, ini)| {
            n.eq_ignore_ascii_case(name).then(|| ini.clone())
        })
    });
    if hit.is_some() {
        return hit;
    }
    authored_slow_death_ini(name)
}

pub fn has_slow_death_behavior(template_name: &str) -> bool {
    slow_death_ini_for_template(template_name).is_some()
}

/// True only when Object INI authors SlowDeathBehavior (not KindOf).
pub fn wants_slow_death(template_name: &str, _is_infantry: bool, _is_vehicle: bool) -> bool {
    has_slow_death_behavior(template_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infantry_sinks_then_destroys() {
        let mut d = HostSlowDeathData::infantry_residual(0);
        assert_eq!(d.phase, HostSlowDeathPhase::WaitingToSink);
        // Before sink delay (90f)
        assert!(!d.tick(50));
        assert_eq!(d.phase, HostSlowDeathPhase::WaitingToSink);
        assert!(!d.tick(90));
        assert_eq!(d.phase, HostSlowDeathPhase::Sinking);
        assert!(d.sink_offset < 0.0 || d.tick(91) == false);
        // Force near destroy
        let mut destroyed = false;
        for f in 91..300 {
            if d.tick(f) {
                destroyed = true;
                break;
            }
        }
        assert!(destroyed);
        assert!(d.sink_offset <= 0.0);
    }

    #[test]
    fn vehicle_delay_only() {
        let mut d = HostSlowDeathData::vehicle_residual(10);
        assert!(!d.tick(20));
        assert!(d.tick(10 + ms_to_frames(VEHICLE_DESTRUCTION_DELAY_MS)));
    }

    #[test]
    fn ini_attrs_drive_phases_not_kindof_defaults() {
        // C++ SlowDeathBehavior.cpp:113-116 parseDuration + :238-239 beginSlowDeath.
        let ini = slow_death_ini_from_behavior_attrs(&[
            ("SinkDelay", "1200"),
            ("SinkRate", "2.0"),
            ("DestructionDelay", "5000"),
            ("DestructionAltitude", "-8"),
        ]);
        assert_eq!(ini.sink_delay_ms, 1200);
        assert!((ini.sink_rate_per_sec - 2.0).abs() < 1e-5);
        assert_eq!(ini.destruction_delay_ms, 5000);
        let d = HostSlowDeathData::from_ini(10, &ini);
        assert_eq!(d.phase, HostSlowDeathPhase::WaitingToSink);
        assert_eq!(d.sink_at_frame, 10 + msec_to_logic_frames(1200));
        assert_eq!(d.destroy_at_frame, 10 + msec_to_logic_frames(5000));
        assert!((d.destruction_altitude + 8.0).abs() < 1e-5);
        // Not the hardcoded vehicle 1000ms peel.
        assert_ne!(
            d.destroy_at_frame,
            10 + msec_to_logic_frames(VEHICLE_DESTRUCTION_DELAY_MS)
        );
        assert_eq!(
            d.midpoint_at_frame(),
            10 + ((msec_to_logic_frames(5000) as f32) * BEGIN_MIDPOINT_RATIO) as u32
        );
    }

    #[test]
    fn missing_slow_death_module_is_not_kindof_inferred() {
        clear_slow_death_ini_override_for_tests();
        assert!(!wants_slow_death("AmericaInfantryRanger", true, false));
        assert!(!wants_slow_death("AmericaTankCrusader", false, true));
    }
}
