//! C++ `Drawable::flashAsSelected` + `saturateRGB`.

use super::*;
use game_engine::common::ini::get_global_data;

impl BasicDrawable {
    /// C++ `Drawable::saturateRGB`.
    pub fn saturate_rgb(color: &mut Vector3, factor: f32) {
        color.x *= factor;
        color.y *= factor;
        color.z *= factor;
        let half = factor * 0.5;
        color.x -= half;
        color.y -= half;
        color.z -= half;
    }

    /// C++ `Drawable::flashAsSelected`.
    ///
    /// Plays `m_selectionFlashEnvelope` with house color when
    /// `TheGlobalData->m_selectionFlashHouseColor`, else white, then
    /// `saturateRGB` by `m_selectionFlashSaturationFactor` (default 0.5)
    /// and `play(color, 0, 4)`.
    pub fn flash_as_selected(&mut self, color: Option<Vector3>) {
        if self.selection_flash_envelope.is_none() {
            self.selection_flash_envelope = Some(TintEnvelope::new());
        }
        let peak = match color {
            Some(color) => color,
            None => {
                let (use_house, saturation) = get_global_data()
                    .map(|data| {
                        let data = data.read();
                        (
                            data.selection_flash_house_color,
                            data.selection_flash_saturation_factor,
                        )
                    })
                    .unwrap_or((false, 0.5));
                let mut temp = if use_house {
                    self.indicator_color
                        .or(self.presentation_indicator_color)
                        .map(|(r, g, b)| {
                            Vector3::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
                        })
                        .unwrap_or(Vector3::one())
                } else {
                    Vector3::one()
                };
                Self::saturate_rgb(&mut temp, saturation);
                temp
            }
        };
        if let Some(envelope) = self.selection_flash_envelope.as_mut() {
            envelope.play(peak, 0, 4, DEF_SUSTAIN_FRAMES);
        }
    }
}
