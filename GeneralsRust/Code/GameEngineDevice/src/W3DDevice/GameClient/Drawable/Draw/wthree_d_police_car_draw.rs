//! W3DPoliceCarDraw Module
//!
//! Corresponds to C++ file: GameEngineDevice/Source/W3DDevice/GameClient/Drawable/Draw/W3DPoliceCarDraw.cpp
//!
//! Extends W3DTruckDraw with a dynamic colored searchlight that cycles through
//! red/blue colors. Has a flashing light bar effect via HAnim animation and
//! a W3DDynamicLight.
//!
//! C++ author: Colin Day, May 2001

use crate::W3DDevice::GameClient::wthree_d_dynamic_light::W3DDynamicLight;
use crate::W3DDevice::GameClient::wthree_d_scene::RenderObjectId;
use cgmath::{Matrix4, Point3, Vector3};

/// Light height above car (C++: floatAmt)
const LIGHT_HEIGHT: f32 = 8.0;
/// Frame increment per draw (C++: animAmt)
const ANIM_INCREMENT: f32 = 0.25;

/// W3DPoliceCarDraw implementation
///
/// Extends W3DTruckDraw with light color cycling. The light cycles through
/// red -> bright red -> red fade -> transition -> bright blue -> blue fade.
/// The animation advances by 0.25 frames per draw call.
#[derive(Debug)]
pub struct W3DPoliceCarDraw {
    /// Dynamic point light for searchlight
    light_id: Option<RenderObjectId>,
    /// Current animation frame for color cycling (random initial offset)
    cur_frame: f32,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
}

impl W3DPoliceCarDraw {
    pub fn new() -> Self {
        Self {
            light_id: None,
            // C++: GameClientRandomValueReal(0, 10)
            cur_frame: fastrand::f32() * 10.0,
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
        }
    }

    /// Main per-frame draw with light color cycling.
    ///
    /// Color calculation (frame-based red/blue cycling):
    /// - Frame 0-2.99: red=1, green=0.5
    /// - Frame 3-5.99: red=1 (bright red)
    /// - Frame 6-6.99: red=1, green=0.5 (fade)
    /// - Frame 7-8.99: red transition, blue transition
    /// - Frame 9-11.99: blue=1 (bright blue)
    /// - Frame 12-14: green/blue/red fade
    ///
    /// Light positioned at drawable pos + (0, 0, 8.0).
    /// Diffuse set to (red, green, blue), ambient to half that.
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // PARITY_NOTE: W3DTruckDraw::doDrawModule(transformMtx)

        // PARITY_NOTE: Animation update:
        // Peek HAnimClass from render object
        // Increment m_curFrame by 0.25, wrap to 0 if >= num_frames - 1
        // Set animation: renderObj->Set_Animation(anim, m_curFrame)

        let (red, green, blue) = self.compute_light_color();

        // PARITY_NOTE: Create light if NULL via W3DDisplay::m_3DScene->getADynamicLight()
        // Set diffuse, ambient, far attenuation (3, 20), position

        let _ = (red, green, blue);
    }

    fn compute_light_color(&self) -> (f32, f32, f32) {
        let f = self.cur_frame;
        let (red, green, blue) = if f < 3.0 {
            (1.0, 0.5, 0.0)
        } else if f < 6.0 {
            (1.0, 0.0, 0.0)
        } else if f < 7.0 {
            (1.0, 0.5, 0.0)
        } else if f < 9.0 {
            (0.5 + (9.0 - f) / 4.0, 0.0, (f - 5.0) / 6.0)
        } else if f < 12.0 {
            (0.0, 0.0, 1.0)
        } else {
            ((f - 11.0) / 3.0, (f - 11.0) / 3.0, (14.0 - f) / 2.0)
        };
        (red, green, blue)
    }

    pub fn set_hidden(&mut self, hidden: bool) {
        self.hidden = hidden;
    }
    pub fn set_shadows_enabled(&mut self, enable: bool) {
        self.shadow_enabled = enable;
    }
    pub fn release_shadows(&mut self) {}
    pub fn allocate_shadows(&mut self) {}
    pub fn set_fully_obscured_by_shroud(&mut self, fully_obscured: bool) {
        self.fully_obscured_by_shroud = fully_obscured;
    }
    pub fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix4<f32>,
        _old_pos: &Point3<f32>,
        _old_angle: f32,
    ) {
    }
    pub fn react_to_geometry_change(&mut self) {}
    pub fn is_visible(&self) -> bool {
        !self.hidden && !self.fully_obscured_by_shroud
    }
    pub fn crc(&self) -> u32 {
        0
    }
    pub fn xfer(&self) -> u32 {
        1
    }
    pub fn load_post_process(&mut self) {}

    fn on_delete(&mut self) {
        // PARITY_NOTE: If light exists, set frame fade out, enable decay
        self.light_id = None;
    }
}

impl Default for W3DPoliceCarDraw {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for W3DPoliceCarDraw {
    fn drop(&mut self) {
        self.on_delete();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_wthree_d_police_car_draw_basic() {
        let draw = W3DPoliceCarDraw::new();
        assert!(draw.is_visible());
    }
    #[test]
    fn test_wthree_d_police_car_light_color_red_phase() {
        let draw = W3DPoliceCarDraw {
            cur_frame: 1.0,
            ..Default::default()
        };
        let (r, g, b) = draw.compute_light_color();
        assert!((r - 1.0).abs() < 0.01);
        assert!((g - 0.5).abs() < 0.01);
        assert!((b - 0.0).abs() < 0.01);
    }
    #[test]
    fn test_wthree_d_police_car_light_color_blue_phase() {
        let draw = W3DPoliceCarDraw {
            cur_frame: 10.0,
            ..Default::default()
        };
        let (r, g, b) = draw.compute_light_color();
        assert!((b - 1.0).abs() < 0.01);
    }
    #[test]
    fn test_wthree_d_police_car_light_color_transition() {
        let draw = W3DPoliceCarDraw {
            cur_frame: 8.0,
            ..Default::default()
        };
        let (r, _g, b) = draw.compute_light_color();
        assert!(r > 0.0 && r < 1.0);
        assert!(b > 0.0 && b < 1.0);
    }
}
