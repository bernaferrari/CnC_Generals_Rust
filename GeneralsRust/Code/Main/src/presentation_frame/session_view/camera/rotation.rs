//! One source rcInfo timing definition owned by the receiving SessionCamera.
//! Historical raw seconds are admitted only by DTO adapters, without quantizing
//! their accepted f32 bits. Authored requests retain exact source milliseconds.
use super::motion::parabolic_ease;

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
enum RotationDuration {
    Milliseconds(i32),
    Seconds(f32),
}
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
struct RotationCurve {
    duration: RotationDuration,
    ease_in: f32,
    ease_out: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct RotationTiming {
    pub(crate) turn_frames: u32,
    pub(crate) time: RotationTime,
    curve: RotationCurve,
}
impl Default for RotationTiming {
    fn default() -> Self {
        Self::historical(0., 0., 0., RotationTime::default())
    }
}
impl RotationTiming {
    pub(crate) fn authored(seconds: f32, ease_in: f32, ease_out: f32, time: RotationTime) -> Self {
        let ms = ((seconds * 1000.) as i32).max(1);
        Self::milliseconds(ms, ease_in.max(0.), ease_out.max(0.), time)
    }
    pub(crate) fn milliseconds(ms: i32, ease_in: f32, ease_out: f32, time: RotationTime) -> Self {
        Self {
            turn_frames: (ms.max(1) / 33).max(1) as u32,
            time,
            curve: RotationCurve {
                duration: RotationDuration::Milliseconds(ms),
                ease_in,
                ease_out,
            },
        }
    }
    pub(crate) fn historical(
        seconds: f32,
        ease_in: f32,
        ease_out: f32,
        time: RotationTime,
    ) -> Self {
        Self {
            turn_frames: (((seconds * 1000.) as i32).max(1) / 33).max(1) as u32,
            time,
            curve: RotationCurve {
                duration: RotationDuration::Seconds(seconds),
                ease_in,
                ease_out,
            },
        }
    }
    pub(crate) fn update_count(&mut self, seconds: f32) {
        self.turn_frames = ((((seconds * 1000.) as i32).max(1) / 33).max(1)) as u32;
    }
    pub(crate) fn duration_seconds(self) -> f32 {
        match self.curve.duration {
            RotationDuration::Milliseconds(ms) => ms as f32 / 1000.,
            RotationDuration::Seconds(s) => s,
        }
    }
    pub(crate) fn duration_ms(self) -> i32 {
        match self.curve.duration {
            RotationDuration::Milliseconds(ms) => ms,
            RotationDuration::Seconds(s) => (s * 1000.) as i32,
        }
    }
    pub(crate) fn ease_in(self) -> f32 {
        self.curve.ease_in
    }
    pub(crate) fn ease_out(self) -> f32 {
        self.curve.ease_out
    }
    pub(crate) fn factor(self, frame: u32) -> f32 {
        let ms = self.duration_ms().max(1) as f32;
        parabolic_ease(
            frame as f32 / self.turn_frames as f32,
            self.curve.ease_in * 1000. / ms,
            self.curve.ease_out * 1000. / ms,
        )
    }
    pub(crate) fn same_observation(self, other: Self) -> bool {
        let duration = match (self.curve.duration, other.curve.duration) {
            (RotationDuration::Milliseconds(a), RotationDuration::Milliseconds(b)) => a == b,
            (RotationDuration::Seconds(a), RotationDuration::Seconds(b)) => {
                a.to_bits() == b.to_bits()
            }
            _ => false,
        };
        duration
            && self.turn_frames == other.turn_frames
            && self.time == other.time
            && self.curve.ease_in.to_bits() == other.curve.ease_in.to_bits()
            && self.curve.ease_out.to_bits() == other.curve.ease_out.to_bits()
    }
    pub(crate) fn validate(self) -> Result<(), String> {
        let valid_duration = match self.curve.duration {
            RotationDuration::Milliseconds(ms) => ms >= 1,
            RotationDuration::Seconds(s) => s.is_finite(),
        };
        if !valid_duration
            || self.turn_frames == 0
            || self.turn_frames > i32::MAX as u32 / 33
            || !self.curve.ease_in.is_finite()
            || !self.curve.ease_out.is_finite()
        {
            return Err("invalid camera rotation timing".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct RotationTime {
    pub(crate) start: i32,
    pub(crate) end: i32,
}
impl Default for RotationTime {
    fn default() -> Self {
        Self::constant(1)
    }
}
impl RotationTime {
    pub(crate) fn constant(value: i32) -> Self {
        Self {
            start: value,
            end: value,
        }
    }
    pub(crate) fn sample(self, factor: f32) -> f32 {
        // Source performs the integer subtraction, multiplies in Real, then
        // promotes for the double literal 0.5 before floor. Explicit wrapping
        // keeps Rust release/debug arithmetic identical outside valid C++ range.
        let delta = self.end.wrapping_sub(self.start) as f32 * factor;
        let value = (0.5 + f64::from(delta)) as f32;
        // Original fast_float_floor, with the nearest source FP mode. Safe
        // Rust integer conversion saturates outside C++'s defined input range.
        let adjusted = if value.is_sign_negative() {
            value - f32::from_bits(0x3f7fffff)
        } else {
            value
        };
        self.start.wrapping_add(adjusted as i32) as f32
    }
}
