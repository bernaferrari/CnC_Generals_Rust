//! InGameUI world-anim residual: pack presentation MoneyPickUp Anim2D samples into a
//! CPU layout buffer ready for dual-tick UI / eventual Anim2DCollection draw.
//!
//! Host residual closed here (fail-closed vs full retail Anim2D GPU draw):
//! - Z-rise offset from retail `zRisePerSecond` × age (host Y-up → +lift)
//! - Display-time / fade residual from MoneyPickUp template parameters
//! - Honesty counters for anims / active / bytes packed
//! - Deterministic pack order for dual-tick presentation consumers
//!
//! Still residual:
//! - Full Anim2DCollection GPU frame advance / texture atlas sample
//! - WORLD_ANIM_FADE_ON_EXPIRE live Display surface blend

use crate::presentation_frame::{PresentationFrame, PresentationWorldAnim};

/// Floats per packed world-anim layout entry:
/// pos.xyz + lift_y + display_time + z_rise + age_frames + alpha + fades = 9 × f32.
pub const WORLD_ANIM_LAYOUT_FLOATS: usize = 9;
/// Bytes per packed layout entry.
pub const WORLD_ANIM_LAYOUT_BYTES: usize = WORLD_ANIM_LAYOUT_FLOATS * std::mem::size_of::<f32>();

/// Logic frames per second residual for age → seconds conversion.
pub const WORLD_ANIM_LOGIC_FPS: f32 = 30.0;

/// One CPU-side residual world-anim layout sample.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldAnimLayoutEntry {
    pub position: [f32; 3],
    /// C++ residual: Z rise over age (`zRisePerSecond * age_seconds`); host Y-up → +lift.
    pub lift_y: f32,
    pub display_time_seconds: f32,
    pub z_rise_per_second: f32,
    pub age_frames: f32,
    /// Alpha after fade residual (1.0 while active; decays after display time when fades).
    pub alpha: f32,
    pub fades: f32,
    pub template_hash: u32,
}

impl WorldAnimLayoutEntry {
    pub fn to_floats(self) -> [f32; WORLD_ANIM_LAYOUT_FLOATS] {
        [
            self.position[0],
            self.position[1],
            self.position[2],
            self.lift_y,
            self.display_time_seconds,
            self.z_rise_per_second,
            self.age_frames,
            self.alpha,
            self.fades,
        ]
    }
}

/// Honesty bookkeeping for the residual world-anim layout path.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WorldAnimLayoutHonesty {
    pub anims_packed: u32,
    pub active_packed: u32,
    pub bytes_packed: u32,
    pub cpu_pack_ok: bool,
    pub has_geometry: bool,
    pub gpu_upload_ready: bool,
    pub money_pickup_templates_ok: bool,
}

impl WorldAnimLayoutHonesty {
    pub fn honesty_cpu_pack_ok(&self) -> bool {
        self.cpu_pack_ok
    }

    pub fn honesty_geometry_ok(&self) -> bool {
        self.cpu_pack_ok && self.has_geometry && self.active_packed > 0
    }

    pub fn honesty_upload_ready_ok(&self) -> bool {
        self.gpu_upload_ready && self.cpu_pack_ok
    }

    pub fn honesty_template_ok(&self) -> bool {
        self.money_pickup_templates_ok
    }
}

/// Packed world-anim layout payload ready for dual-tick UI consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct WorldAnimLayout {
    pub entries: Vec<WorldAnimLayoutEntry>,
    pub layout_bytes: Vec<u8>,
    pub honesty: WorldAnimLayoutHonesty,
}

fn template_hash(name: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in name.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

impl WorldAnimLayout {
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            layout_bytes: Vec::new(),
            honesty: WorldAnimLayoutHonesty {
                cpu_pack_ok: true,
                money_pickup_templates_ok: true,
                ..Default::default()
            },
        }
    }

    pub fn mark_gpu_upload_ready(&mut self) {
        self.honesty.gpu_upload_ready = self.honesty.cpu_pack_ok;
    }

    pub fn pack_from_presentation(frame: &PresentationFrame) -> Self {
        Self::pack_anims_at(&frame.world_anims, frame.frame.0)
    }

    pub fn pack_anims_at(anims: &[PresentationWorldAnim], logic_frame: u32) -> Self {
        if anims.is_empty() {
            return Self::empty();
        }

        let mut entries = Vec::with_capacity(anims.len());
        let mut active = 0u32;
        let mut templates_ok = true;
        for a in anims {
            if a.template.is_empty() {
                templates_ok = false;
            }
            let age = logic_frame.saturating_sub(a.spawn_frame);
            let age_sec = age as f32 / WORLD_ANIM_LOGIC_FPS;
            let lift = a.z_rise_per_second * age_sec;
            let display = a.display_time_seconds.max(0.0);
            let alpha = if age_sec < display {
                active = active.saturating_add(1);
                1.0
            } else if a.fades {
                // Residual fade window: 1 second after display time.
                let past = age_sec - display;
                (1.0 - past).clamp(0.0, 1.0)
            } else {
                0.0
            };
            if alpha <= 0.0 {
                continue;
            }
            entries.push(WorldAnimLayoutEntry {
                position: [a.position.x, a.position.y, a.position.z],
                lift_y: lift,
                display_time_seconds: display,
                z_rise_per_second: a.z_rise_per_second,
                age_frames: age as f32,
                alpha,
                fades: if a.fades { 1.0 } else { 0.0 },
                template_hash: template_hash(&a.template),
            });
        }

        let mut floats = Vec::with_capacity(entries.len() * WORLD_ANIM_LAYOUT_FLOATS);
        for e in &entries {
            floats.extend_from_slice(&e.clone().to_floats());
        }
        let layout_bytes = f32_slice_to_bytes(&floats);
        let anims_packed = entries.len() as u32;
        Self {
            honesty: WorldAnimLayoutHonesty {
                anims_packed,
                active_packed: active,
                bytes_packed: layout_bytes.len() as u32,
                cpu_pack_ok: true,
                has_geometry: active > 0,
                gpu_upload_ready: false,
                money_pickup_templates_ok: templates_ok,
            },
            entries,
            layout_bytes,
        }
    }
}

fn f32_slice_to_bytes(floats: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(floats.len() * 4);
    for f in floats {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

/// Host-testable residual: pack + mark upload-ready without a live GPU device.
pub fn pack_world_anim_and_mark_ready(frame: &PresentationFrame) -> WorldAnimLayout {
    let mut pack = WorldAnimLayout::pack_from_presentation(frame);
    pack.mark_gpu_upload_ready();
    pack
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation_frame::PresentationWorldAnim;

    #[test]
    fn empty_pack_is_honest_cpu_success() {
        let pack = WorldAnimLayout::empty();
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(!pack.honesty.honesty_geometry_ok());
        assert!(pack.honesty.honesty_template_ok());
    }

    #[test]
    fn packs_synthetic_money_pickup_with_z_rise() {
        let anim = PresentationWorldAnim::synthetic_money_pickup(0);
        let pack = WorldAnimLayout::pack_anims_at(&[anim.clone()], 15);
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(pack.honesty.honesty_geometry_ok());
        assert_eq!(pack.honesty.anims_packed, 1);
        assert_eq!(pack.honesty.active_packed, 1);
        let age_sec = 15.0 / WORLD_ANIM_LOGIC_FPS;
        let expected_lift = anim.z_rise_per_second * age_sec;
        assert!((pack.entries[0].lift_y - expected_lift).abs() < 0.001);
        assert!((pack.entries[0].alpha - 1.0).abs() < 0.001);
        let mut marked = pack;
        marked.mark_gpu_upload_ready();
        assert!(marked.honesty.honesty_upload_ready_ok());
    }
}
