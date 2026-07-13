//! InGameUI floating-text residual: pack presentation floating cash captions into a
//! CPU layout buffer ready for dual-tick UI / eventual WGPU text draw.
//!
//! Host residual closed here (fail-closed vs full retail DisplayString GPU draw):
//! - Move-up offset from `PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED` (C++ default 1.0)
//! - Timeout / vanish residual from retail DEFAULT_FLOATING_TEXT_TIMEOUT (10 frames)
//! - GameText `GUI:AddCash` caption residual (`+$N` format parity with host text)
//! - DisplayString monospaced measure residual (8×8 glyph extents for caption)
//! - Honesty counters for texts / active / bytes packed
//! - Deterministic pack order for dual-tick presentation consumers
//!
//! Still residual:
//! - Full DisplayString GPU font atlas raster / WW3D StretchRect submit
//! - Full multi-locale CSF/STR Unicode GameText table load at boot
//! - Full vanish-rate alpha blend on live Display surface

use crate::graphics::game_text_residual::{
    honesty_display_string_measure, measure_display_string_residual,
};
use crate::presentation_frame::{
    PresentationFloatingText, PresentationFrame, PresentationWorldAnim,
    PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED, PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES,
    PRESENTATION_FLOATING_TEXT_VANISH_RATE,
};

/// Retail GameText key for cash gain floating captions.
pub const GUI_ADD_CASH_KEY: &str = "GUI:AddCash";

/// Residual GameText resolution for `GUI:AddCash` captions.
///
/// C++: `moneyString.format(TheGameText->fetch("GUI:AddCash"), amount)`.
/// English retail resolves to a `+$N` style caption. Host residual formats
/// `+$amount` (ASCII) when the key is `GUI:AddCash` and falls back to the
/// already-frozen presentation `text` otherwise.
///
/// Fail-closed vs full CSF/STR Unicode localization table load.
pub fn resolve_add_cash_caption(text_key: &str, amount: u32, frozen_text: &str) -> String {
    if text_key == GUI_ADD_CASH_KEY {
        format!("+${amount}")
    } else if !frozen_text.is_empty() {
        frozen_text.to_string()
    } else {
        format!("+${amount}")
    }
}

/// Honesty: key is retail `GUI:AddCash` and caption matches residual format.
pub fn honesty_add_cash_caption(text_key: &str, amount: u32, caption: &str) -> bool {
    text_key == GUI_ADD_CASH_KEY && caption == format!("+${amount}")
}

/// Floats per packed layout entry:
/// pos.xyz + lift_y + color.rgba + alpha + amount + age_frames + timeout_frames = 12 × f32.
pub const FLOATING_TEXT_LAYOUT_FLOATS: usize = 12;
/// Bytes per packed layout entry.
pub const FLOATING_TEXT_LAYOUT_BYTES: usize =
    FLOATING_TEXT_LAYOUT_FLOATS * std::mem::size_of::<f32>();

/// One CPU-side residual floating text layout sample.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingTextLayoutEntry {
    /// World position at spawn (presentation freeze).
    pub position: [f32; 3],
    /// C++ draw residual: `pos.y -= frameCount * moveUpSpeed` (host Y-up → +lift).
    pub lift_y: f32,
    pub color_rgba: [f32; 4],
    /// Alpha after vanish residual (1.0 while active, decays after timeout).
    pub alpha: f32,
    pub amount: f32,
    pub age_frames: f32,
    pub timeout_frames: f32,
    /// Residual GameText caption (`+$N` for GUI:AddCash).
    pub caption: String,
    /// Retail GameText key residual (`GUI:AddCash`).
    pub text_key: String,
    /// DisplayString monospaced measure residual (width px).
    pub measure_width: u32,
    /// DisplayString monospaced measure residual (height px).
    pub measure_height: u32,
}

impl FloatingTextLayoutEntry {
    pub fn to_floats(self) -> [f32; FLOATING_TEXT_LAYOUT_FLOATS] {
        [
            self.position[0],
            self.position[1],
            self.position[2],
            self.lift_y,
            self.color_rgba[0],
            self.color_rgba[1],
            self.color_rgba[2],
            self.color_rgba[3],
            self.alpha,
            self.amount,
            self.age_frames,
            self.timeout_frames,
        ]
    }
}

/// Honesty bookkeeping for the residual floating text layout path.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FloatingTextLayoutHonesty {
    pub texts_packed: u32,
    pub active_packed: u32,
    pub world_anims_observed: u32,
    pub bytes_packed: u32,
    /// True when pack completed without panic (empty is honest success).
    pub cpu_pack_ok: bool,
    /// True when at least one active text was packed.
    pub has_geometry: bool,
    /// True after `mark_gpu_upload_ready` (still not a live font draw).
    pub gpu_upload_ready: bool,
    pub move_up_speed: f32,
    pub vanish_rate: f32,
    pub timeout_frames: u32,
    /// True when all packed entries resolve GUI:AddCash caption residual.
    pub game_text_caption_ok: bool,
    /// True when all packed entries have honest DisplayString measure residual.
    pub display_string_measure_ok: bool,
}

impl FloatingTextLayoutHonesty {
    pub fn honesty_cpu_pack_ok(&self) -> bool {
        self.cpu_pack_ok
    }

    pub fn honesty_geometry_ok(&self) -> bool {
        self.cpu_pack_ok && self.has_geometry && self.active_packed > 0
    }

    pub fn honesty_upload_ready_ok(&self) -> bool {
        self.gpu_upload_ready && self.cpu_pack_ok
    }

    pub fn honesty_retail_params_ok(&self) -> bool {
        (self.move_up_speed - PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED).abs() < 0.001
            && (self.vanish_rate - PRESENTATION_FLOATING_TEXT_VANISH_RATE).abs() < 0.001
            && self.timeout_frames == PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES
    }

    pub fn honesty_game_text_caption_ok(&self) -> bool {
        self.game_text_caption_ok
    }

    pub fn honesty_display_string_measure_ok(&self) -> bool {
        self.display_string_measure_ok
    }
}

/// Packed floating text layout payload ready for dual-tick UI consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingTextLayout {
    pub entries: Vec<FloatingTextLayoutEntry>,
    /// Interleaved f32 layout bytes (see `FloatingTextLayoutEntry`).
    pub layout_bytes: Vec<u8>,
    pub honesty: FloatingTextLayoutHonesty,
}

impl FloatingTextLayout {
    /// Empty pack — honest residual when no floating texts are active.
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            layout_bytes: Vec::new(),
            honesty: FloatingTextLayoutHonesty {
                cpu_pack_ok: true,
                game_text_caption_ok: true,
                display_string_measure_ok: true,
                move_up_speed: PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED,
                vanish_rate: PRESENTATION_FLOATING_TEXT_VANISH_RATE,
                timeout_frames: PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES,
                ..Default::default()
            },
        }
    }

    pub fn mark_gpu_upload_ready(&mut self) {
        self.honesty.gpu_upload_ready = self.honesty.cpu_pack_ok;
    }

    /// Pack presentation floating texts at `logic_frame` into layout samples.
    pub fn pack_from_presentation(frame: &PresentationFrame) -> Self {
        Self::pack_texts_at(&frame.floating_texts, frame.frame.0, &frame.world_anims)
    }

    pub fn pack_texts_at(
        texts: &[PresentationFloatingText],
        logic_frame: u32,
        world_anims: &[PresentationWorldAnim],
    ) -> Self {
        if texts.is_empty() {
            let mut empty = Self::empty();
            empty.honesty.world_anims_observed = world_anims.len() as u32;
            return empty;
        }

        let mut entries = Vec::with_capacity(texts.len());
        let mut active = 0u32;
        let mut caption_ok = true;
        let mut measure_ok = true;
        for t in texts {
            let age = logic_frame.saturating_sub(t.spawn_frame);
            let timeout = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES;
            let lift = age as f32 * PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED;
            // C++ residual: while before timeout alpha stays full; after timeout
            // vanish rate pulls alpha toward 0 until erased.
            let alpha = if age < timeout {
                1.0
            } else {
                let past = (age - timeout) as f32;
                (1.0 - past * PRESENTATION_FLOATING_TEXT_VANISH_RATE).clamp(0.0, 1.0)
            };
            // Pack only non-vanished (alpha > 0) entries — erase residual.
            if alpha <= 0.0 {
                continue;
            }
            if age < timeout {
                active = active.saturating_add(1);
            }
            let caption = resolve_add_cash_caption(&t.text_key, t.amount, &t.text);
            if !honesty_add_cash_caption(&t.text_key, t.amount, &caption)
                && t.text_key == GUI_ADD_CASH_KEY
            {
                caption_ok = false;
            }
            if t.text_key != GUI_ADD_CASH_KEY {
                // Non-AddCash keys still pack; mark caption residual incomplete.
                caption_ok = caption_ok && !t.text_key.is_empty();
            }
            let (measure_width, measure_height) = measure_display_string_residual(&caption);
            if !honesty_display_string_measure(&caption, measure_width, measure_height) {
                measure_ok = false;
            }
            let c = t.color_rgba;
            entries.push(FloatingTextLayoutEntry {
                position: [t.position.x, t.position.y, t.position.z],
                lift_y: lift,
                color_rgba: [
                    c.0 as f32 / 255.0,
                    c.1 as f32 / 255.0,
                    c.2 as f32 / 255.0,
                    c.3 as f32 / 255.0,
                ],
                alpha,
                amount: t.amount as f32,
                age_frames: age as f32,
                timeout_frames: timeout as f32,
                caption,
                text_key: t.text_key.clone(),
                measure_width,
                measure_height,
            });
        }

        let mut floats = Vec::with_capacity(entries.len() * FLOATING_TEXT_LAYOUT_FLOATS);
        for e in &entries {
            floats.extend_from_slice(&e.clone().to_floats());
        }
        let layout_bytes = f32_slice_to_bytes(&floats);
        let texts_packed = entries.len() as u32;
        // Empty of non-AddCash failures: when packing GUI:AddCash entries, require
        // residual caption format; empty list is honest success.
        let game_text_caption_ok = if texts_packed == 0 {
            true
        } else {
            caption_ok
                && entries
                    .iter()
                    .all(|e| honesty_add_cash_caption(&e.text_key, e.amount as u32, &e.caption))
        };
        let display_string_measure_ok = if texts_packed == 0 {
            true
        } else {
            measure_ok
                && entries.iter().all(|e| {
                    honesty_display_string_measure(&e.caption, e.measure_width, e.measure_height)
                        && e.measure_width > 0
                })
        };
        Self {
            honesty: FloatingTextLayoutHonesty {
                texts_packed,
                active_packed: active,
                world_anims_observed: world_anims.len() as u32,
                bytes_packed: layout_bytes.len() as u32,
                cpu_pack_ok: true,
                has_geometry: active > 0,
                gpu_upload_ready: false,
                move_up_speed: PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED,
                vanish_rate: PRESENTATION_FLOATING_TEXT_VANISH_RATE,
                timeout_frames: PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES,
                game_text_caption_ok,
                display_string_measure_ok,
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
pub fn pack_floating_text_and_mark_ready(frame: &PresentationFrame) -> FloatingTextLayout {
    let mut pack = FloatingTextLayout::pack_from_presentation(frame);
    pack.mark_gpu_upload_ready();
    pack
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation_frame::{
        PresentationFloatingText, PresentationWorldAnim, PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES,
    };

    #[test]
    fn empty_pack_is_honest_cpu_success() {
        let pack = FloatingTextLayout::empty();
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(!pack.honesty.honesty_geometry_ok());
        assert!(pack.layout_bytes.is_empty());
        assert!(pack.honesty.honesty_retail_params_ok());
    }

    #[test]
    fn packs_synthetic_cash_with_move_up_and_timeout() {
        let ft = PresentationFloatingText::synthetic_cash(150, 0);
        let pack = FloatingTextLayout::pack_texts_at(
            &[ft],
            3,
            &[PresentationWorldAnim::synthetic_money_pickup(0)],
        );
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(pack.honesty.honesty_geometry_ok());
        assert!(pack.honesty.honesty_game_text_caption_ok());
        assert_eq!(pack.honesty.texts_packed, 1);
        assert_eq!(pack.honesty.active_packed, 1);
        assert_eq!(pack.honesty.world_anims_observed, 1);
        assert_eq!(pack.entries[0].lift_y, 3.0 * PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED);
        assert!((pack.entries[0].alpha - 1.0).abs() < 0.001);
        assert_eq!(pack.entries[0].caption, "+$150");
        assert_eq!(pack.entries[0].text_key, GUI_ADD_CASH_KEY);
        assert!(pack.honesty.honesty_display_string_measure_ok());
        // monospaced 8×8 residual: "+$150" = 5 glyphs → 40 px wide
        assert_eq!(pack.entries[0].measure_width, 5 * 8);
        assert_eq!(pack.entries[0].measure_height, 8);
        assert_eq!(
            pack.layout_bytes.len(),
            FLOATING_TEXT_LAYOUT_BYTES
        );
        let mut marked = pack;
        marked.mark_gpu_upload_ready();
        assert!(marked.honesty.honesty_upload_ready_ok());
    }

    #[test]
    fn resolve_add_cash_caption_residual() {
        assert_eq!(resolve_add_cash_caption(GUI_ADD_CASH_KEY, 200, ""), "+$200");
        assert!(honesty_add_cash_caption(GUI_ADD_CASH_KEY, 200, "+$200"));
    }

    #[test]
    fn vanish_phase_after_timeout_decays_alpha() {
        let ft = PresentationFloatingText::synthetic_cash(50, 0);
        let age = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES + 5;
        let pack = FloatingTextLayout::pack_texts_at(&[ft], age, &[]);
        assert_eq!(pack.honesty.active_packed, 0);
        assert_eq!(pack.honesty.texts_packed, 1);
        let expected = (1.0 - 5.0 * PRESENTATION_FLOATING_TEXT_VANISH_RATE).clamp(0.0, 1.0);
        assert!((pack.entries[0].alpha - expected).abs() < 0.001);
    }
}
