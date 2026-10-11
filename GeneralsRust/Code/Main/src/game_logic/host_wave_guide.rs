//! Host WaveGuideUpdate residual (dam flood wave after DamDie).
//!
//! C++: WaveGuide objects start DISABLED_DEFAULT; DamDie clears it. After
//! WaveDelay the wave initializes along `WaveGuide1`, damages objects in
//! DamageRadius (DAMAGE_WATER / DEATH_FLOODED, OBJECT_STATUS_WET once-gate),
//! skips towers / z>PreferredHeight, swaps hit bridges to WaterWaveBridge,
//! then destroys itself at the last waypoint.

use serde::{Deserialize, Serialize};

pub const WAVE_GUIDE_LOGIC_FPS: f32 = 30.0;
pub const WAVE_DELAY_MS: u32 = 750;
pub const WAVE_SPEED_PER_SEC: f32 = 120.0;
pub const WAVE_DAMAGE_RADIUS: f32 = 25.0;
pub const WAVE_DAMAGE_AMOUNT: f32 = 99999.0;
pub const WAVE_TOPPLE_FORCE: f32 = 0.25;
/// C++ PATH_EXTRA_DISTANCE = 10 * PATHFIND_CELL_SIZE_F.
pub const PATH_EXTRA_DISTANCE: f32 = 100.0;
/// Residual PreferredHeight when WaveGuide INI is not loaded on the host.
/// Aircraft / high objects above this are skipped (C++ z > preferredHeight).
pub const WAVE_PREFERRED_HEIGHT: f32 = 40.0;
/// C++ MODELCONDITION_FLOODED residual bit index.
pub const MC_BIT_FLOODED: u32 = 69;
/// Residual `WaterVelocity` (INI dist/sec via leftover `parseVelocityReal`).
pub const WAVE_WATER_VELOCITY: f32 = 2.0 / WAVE_GUIDE_LOGIC_FPS;
/// Residual `YSize` / `LinearWaveSpacing` for C++ `computeWaveShapePoints`.
pub const WAVE_Y_SIZE: f32 = 400.0;
pub const WAVE_LINEAR_SPACING: f32 = 20.0;
pub const WAVE_BEND_MAGNITUDE: f32 = 0.0;
/// Retail `Object WaveGuide` `LoopingSound` / `RandomSplashSound`.
pub const WAVE_LOOPING_SOUND: &str = "DamBreakWaveLoop";
pub const WAVE_RANDOM_SPLASH_SOUND: &str = "WaveRandomSplash";

/// Immutable data of the driving object's authored WaveGuideUpdate module.
/// C++ constructor defaults are empty event names and frequency zero.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WaveGuideAudioMetadata {
    pub looping_sound: String,
    pub random_splash_sound: String,
    pub random_splash_sound_frequency: i32,
}

const MAX_WAVEGUIDE_SHAPE_POINTS: usize = 64;

/// C++ `computeWaveShapePoints` + object yaw transform; returns C++ world XY.
pub fn wave_shape_world_points(pos_x: f32, pos_z: f32, facing: f32) -> Vec<(f32, f32)> {
    let step = WAVE_LINEAR_SPACING as i32;
    if step == 0 {
        return vec![(pos_x, pos_z)];
    }
    let half_y = (WAVE_Y_SIZE * 0.5) as i32;
    let cos = facing.cos();
    let sin = facing.sin();
    let mut points = Vec::new();
    let mut y = -half_y;
    while y < half_y && points.len() < MAX_WAVEGUIDE_SHAPE_POINTS {
        let y_f = y as f32;
        let x = if WAVE_BEND_MAGNITUDE != 0.0 {
            -(y_f * y_f) / WAVE_BEND_MAGNITUDE
        } else {
            0.0
        };
        let wx = pos_x + x * cos - y_f * sin;
        let wy = pos_z + x * sin + y_f * cos;
        points.push((wx, wy));
        y += step;
    }
    if points.is_empty() {
        points.push((pos_x, pos_z));
    }
    points
}

#[inline]
pub fn ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * WAVE_GUIDE_LOGIC_FPS / 1000.0).round() as u32
}

pub fn wave_speed_per_frame() -> f32 {
    WAVE_SPEED_PER_SEC / WAVE_GUIDE_LOGIC_FPS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostWaveGuideData {
    pub active_frame: u32,
    pub initialized: bool,
    pub done: bool,
    /// Facing residual (radians, yaw about Y).
    pub facing: f32,
    pub damage_applications: u32,
    pub topple_requests: u32,
    /// C++ m_finalDestination XY (host XZ).
    #[serde(default)]
    pub final_destination: Option<(f32, f32)>,
    /// C++ WaveGuideUpdateModuleData::m_preferredHeight.
    #[serde(default = "default_preferred_height")]
    pub preferred_height: f32,
    /// C++ `WaveGuideUpdate::m_splashSoundFrame`.
    #[serde(default)]
    pub splash_sound_frame: u32,
    /// C++ `startMoving` TheAudio add happens once.
    #[serde(default)]
    pub looping_started: bool,
}

fn default_preferred_height() -> f32 {
    WAVE_PREFERRED_HEIGHT
}

impl Default for HostWaveGuideData {
    fn default() -> Self {
        Self {
            active_frame: 0,
            initialized: false,
            done: false,
            facing: 0.0,
            damage_applications: 0,
            topple_requests: 0,
            final_destination: None,
            preferred_height: WAVE_PREFERRED_HEIGHT,
            splash_sound_frame: 0,
            looping_started: false,
        }
    }
}

impl HostWaveGuideData {
    pub fn ensure_active(&mut self, current_frame: u32) {
        if self.active_frame == 0 {
            self.active_frame = current_frame.max(1);
        }
    }

    pub fn is_moving(&self, current_frame: u32) -> bool {
        if self.done || self.active_frame == 0 {
            return false;
        }
        current_frame.saturating_sub(self.active_frame) >= ms_to_frames(WAVE_DELAY_MS)
    }

    pub fn mark_done(&mut self) {
        self.done = true;
    }

    /// C++ update:795-822 — close enough to the last WaveGuide1 waypoint.
    pub fn reached_destination(&self, host_x: f32, host_z: f32) -> bool {
        let Some((dx, dy)) = self.final_destination else {
            return false;
        };
        let vx = dx - host_x;
        let vy = dy - host_z;
        vx * vx + vy * vy <= PATH_EXTRA_DISTANCE * PATH_EXTRA_DISTANCE
    }

    /// Returns displacement (dx, dz) for this frame when moving.
    pub fn motion_delta(&mut self, current_frame: u32) -> Option<(f32, f32)> {
        if !self.is_moving(current_frame) {
            return None;
        }
        if !self.initialized {
            self.initialized = true;
        }
        let speed = wave_speed_per_frame();
        let dx = self.facing.cos() * speed;
        let dz = self.facing.sin() * speed;
        Some((dx, dz))
    }
}

/// True if template is a waveguide flood object.
pub fn is_wave_guide_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("waveguide") || n.contains("waterwave") || n.contains("floodwave")
}

/// C++ damage residual: objects in radius take water flood damage.
pub fn wave_damage_at_distance(dist: f32) -> f32 {
    if dist <= WAVE_DAMAGE_RADIUS {
        WAVE_DAMAGE_AMOUNT
    } else {
        0.0
    }
}

/// Return semantic audio requests for the driving world's presentation queue.
/// No definition discovery or playback publication occurs in this operation.
pub fn wave_guide_audio_tick(
    data: &mut HostWaveGuideData,
    audio: &WaveGuideAudioMetadata,
    random: &game_engine::common::random_value::SessionRandom,
    now: u32,
) -> (Option<String>, Option<String>) {
    let playable = |name: &str| !name.is_empty() && !name.eq_ignore_ascii_case("NoSound");
    let looping = if !data.looping_started {
        data.looping_started = true;
        playable(&audio.looping_sound).then(|| audio.looping_sound.clone())
    } else {
        None
    };
    // CPP unsigned elapsed arithmetic and one draw at each due opportunity,
    // even when the authored threshold is 0 or 100.
    let due = now.wrapping_sub(data.splash_sound_frame) > 15;
    let splash = if due {
        data.splash_sound_frame = now;
        let roll = (random.next_u32() % 100) as i32 + 1;
        if roll > audio.random_splash_sound_frequency {
            playable(&audio.random_splash_sound).then(|| audio.random_splash_sound.clone())
        } else {
            None
        }
    } else {
        None
    };
    (looping, splash)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_delay_then_moves() {
        let mut w = HostWaveGuideData::default();
        w.facing = 0.0; // +X
        w.ensure_active(10);
        assert!(w.motion_delta(10).is_none());
        let t = 10 + ms_to_frames(WAVE_DELAY_MS);
        let d = w.motion_delta(t).expect("moving");
        assert!(d.0 > 0.0);
        assert!((d.1).abs() < 0.01);
    }

    #[test]
    fn damage_inside_radius() {
        assert!(wave_damage_at_distance(10.0) >= 99999.0);
        assert_eq!(wave_damage_at_distance(40.0), 0.0);
    }

    #[test]
    fn dest_reached_sets_stop() {
        let mut w = HostWaveGuideData::default();
        w.final_destination = Some((100.0, 0.0));
        assert!(w.reached_destination(50.0, 0.0));
        assert!(!w.reached_destination(250.0, 0.0));
        w.mark_done();
        w.ensure_active(1);
        assert!(w.motion_delta(1000).is_none());
    }

    #[test]
    fn authored_audio_tick_preserves_strict_due_frames_and_extreme_frequencies() {
        for frequency in [0, 100] {
            let audio = WaveGuideAudioMetadata {
                looping_sound: WAVE_LOOPING_SOUND.into(),
                random_splash_sound: WAVE_RANDOM_SPLASH_SOUND.into(),
                random_splash_sound_frequency: frequency,
            };
            let mut w = HostWaveGuideData::default();
            let random = game_engine::common::random_value::SessionRandom::default();
            for now in [15, 16, 31, 32] {
                let (looping, splash) = wave_guide_audio_tick(&mut w, &audio, &random, now);
                assert_eq!(
                    looping.as_deref(),
                    (now == 15).then_some(WAVE_LOOPING_SOUND)
                );
                assert_eq!(
                    splash.as_deref(),
                    (frequency == 0 && (now == 16 || now == 32))
                        .then_some(WAVE_RANDOM_SPLASH_SOUND)
                );
                assert_eq!(
                    w.splash_sound_frame,
                    match now {
                        15 => 0,
                        16 | 31 => 16,
                        _ => 32,
                    }
                );
            }
            assert!(w.looping_started);
        }
    }
}
