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
    /// Dynamic point light for searchlight (C++: m_light).
    /// Created lazily via createDynamicLight().
    light: Option<W3DDynamicLight>,
    /// Current animation frame for color cycling (random initial offset).
    /// C++: m_curFrame = GameClientRandomValueReal(0, 10)
    cur_frame: f32,
    /// Total number of animation frames (from render object Peek_Animation).
    /// Used to wrap cur_frame. C++: anim->Get_Num_Frames()
    num_frames: f32,
    /// Whether a render object exists (C++: getRenderObject() != NULL).
    has_render_object: bool,
    /// Drawable position for light placement (C++: getDrawable()->getPosition()).
    drawable_pos: Point3<f32>,
    hidden: bool,
    fully_obscured_by_shroud: bool,
    shadow_enabled: bool,
}

impl W3DPoliceCarDraw {
    pub fn new() -> Self {
        Self {
            light: None,
            cur_frame: fastrand::f32() * 10.0,
            num_frames: 15.0,
            has_render_object: false,
            drawable_pos: Point3::new(0.0, 0.0, 0.0),
            hidden: false,
            fully_obscured_by_shroud: false,
            shadow_enabled: true,
        }
    }

    /// Main per-frame draw with light color cycling.
    ///
    /// C++ parity (W3DPoliceCarDraw::doDrawModule):
    /// 1. Get render object; return if NULL.
    /// 2. Peek animation; advance m_curFrame by 0.25, wrap to 0 if >= numFrames-1.
    /// 3. Compute light color from cur_frame.
    /// 4. Create dynamic light if NULL.
    /// 5. Set light: diffuse=(r,g,b), ambient=(r/2,g/2,b/2), far atten=(3,20),
    ///    position=(pos.x, pos.y, pos.z+8.0).
    /// 6. Call W3DTruckDraw::doDrawModule(transformMtx).
    pub fn do_draw_module(&mut self, _transform_mtx: &Matrix4<f32>) {
        // C++: RenderObjClass* policeCarRenderObj = getRenderObject();
        // C++: if (policeCarRenderObj == NULL) return;
        if !self.has_render_object {
            return;
        }

        // C++: HAnimClass *anim = policeCarRenderObj->Peek_Animation();
        // C++: if (anim) {
        // C++:   Real frames = anim->Get_Num_Frames();
        // C++:   m_curFrame += animAmt;  // animAmt = 0.25
        // C++:   if (m_curFrame > frames-1) { m_curFrame = 0; }
        // C++:   policeCarRenderObj->Set_Animation(anim, m_curFrame);
        // C++: }
        self.cur_frame += ANIM_INCREMENT;
        if self.num_frames > 1.0 && self.cur_frame > self.num_frames - 1.0 {
            self.cur_frame = 0.0;
        }
        // PARITY_NOTE: Set_Animation on render object via scene manager

        let (red, green, blue) = self.compute_light_color();

        // C++: if (m_light == NULL) m_light = createDynamicLight();
        if self.light.is_none() {
            self.light = Some(self.create_dynamic_light());
        }

        // C++: if (m_light) {
        // C++:   Coord3D pos = *getDrawable()->getPosition();
        // C++:   m_light->Set_Diffuse(Vector3(red, green, blue));
        // C++:   m_light->Set_Ambient(Vector3(red/2, green/2, blue/2));
        // C++:   m_light->Set_Far_Attenuation_Range(3, 20);
        // C++:   m_light->Set_Position(Vector3(pos.x, pos.y, pos.z+floatAmt));
        // C++: }
        if let Some(light) = &mut self.light {
            light.set_diffuse(Vector3::new(red, green, blue));
            light.set_ambient(Vector3::new(red / 2.0, green / 2.0, blue / 2.0));
            light.set_range(3.0, 20.0);
            light.set_position(Vector3::new(
                self.drawable_pos.x,
                self.drawable_pos.y,
                self.drawable_pos.z + LIGHT_HEIGHT,
            ));
        }

        // PARITY_NOTE: W3DTruckDraw::doDrawModule(transformMtx) called last in C++.
        // Parent class handles turret, treads, etc.
    }

    fn create_dynamic_light(&self) -> W3DDynamicLight {
        // C++: W3DDynamicLight *light = W3DDisplay::m_3DScene->getADynamicLight();
        // C++: light->setEnabled(TRUE);
        // C++: light->Set_Ambient(Vector3(0,0,0));
        // C++: light->Set_Diffuse(Vector3(0,0,0));  // No diffuse for searchlight
        // C++: light->Set_Position(Vector3(0,0,0));
        // C++: light->Set_Far_Attenuation_Range(5, 15);
        let mut light = W3DDynamicLight::point();
        light.set_enabled(true);
        light.set_ambient(Vector3::new(0.0, 0.0, 0.0));
        light.set_diffuse(Vector3::new(0.0, 0.0, 0.0));
        light.set_range(5.0, 15.0);
        light
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

    /// C++ parity: Inherited via `W3DTruckDraw -> W3DModelDraw::releaseShadows()` — releases
    /// shadow via `m_shadow->release()` and sets `m_shadow = NULL`.
    // PARITY_NOTE: Would call W3DModelDraw::releaseShadows() in C++ (removes shadow from scene).
    // This struct lacks shadow_id; when full W3DModelDraw state is composed in, delegate to parent.
    pub fn release_shadows(&mut self) {}

    /// C++ parity: Inherited via `W3DTruckDraw -> W3DModelDraw::allocateShadows()` — creates
    /// shadow from ThingTemplate info if no shadow exists, render object exists, and shadow type != SHADOW_NONE.
    // PARITY_NOTE: Would call W3DModelDraw::allocateShadows() in C++.
    // This struct lacks shadow_id; when full W3DModelDraw state is composed in, delegate to parent.
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

    /// C++ parity: Inherited via `W3DTruckDraw::reactToGeometryChange() { }` — empty override
    /// in W3DTruckDraw.h. Police car geometry bounds are implicitly updated via render object transforms.
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
    /// C++ parity: `W3DPoliceCarDraw::loadPostProcess()` — calls
    /// `W3DTruckDraw::loadPostProcess()` which calls `W3DModelDraw::loadPostProcess()`
    /// then `tossEmitters()`. Releases dust/dirt/powerslide particle systems.
    pub fn load_post_process(&mut self) {
        // PARITY_NOTE: C++ chain: W3DPoliceCarDraw -> W3DTruckDraw::loadPostProcess()
        //   -> W3DModelDraw::loadPostProcess()
        //   -> tossEmitters() (releases m_dustEffect, m_dirtEffect, m_powerslideEffect)
        // Emitters are re-created lazily when enableEmitters(true) is called during doDrawModule.
        // Requires particle system infrastructure to be wired.
    }

    pub fn set_has_render_object(&mut self, has: bool) {
        self.has_render_object = has;
    }

    pub fn set_num_frames(&mut self, frames: f32) {
        self.num_frames = frames;
    }

    pub fn set_drawable_pos(&mut self, pos: Point3<f32>) {
        self.drawable_pos = pos;
    }

    pub fn get_cur_frame(&self) -> f32 {
        self.cur_frame
    }

    fn on_delete(&mut self) {
        // C++: if (m_light) {
        // C++:   m_light->setFrameFade(0, 5);  // fade out over 5 frames
        // C++:   m_light->setDecayRange();
        // C++:   m_light->setDecayColor();
        // C++:   m_light = NULL;
        // C++: }
        if let Some(light) = &mut self.light {
            light.set_frame_fade(0, 5);
            light.set_decay_range(true);
            light.set_decay_color(true);
        }
        self.light = None;
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

    #[test]
    fn test_wthree_d_police_car_draw_no_render_object() {
        let mut draw = W3DPoliceCarDraw::new();
        let initial_frame = draw.cur_frame;
        draw.do_draw_module(&Matrix4::identity());
        assert_eq!(draw.cur_frame, initial_frame);
        assert!(draw.light.is_none());
    }

    #[test]
    fn test_wthree_d_police_car_draw_frame_advancement() {
        let mut draw = W3DPoliceCarDraw::new();
        draw.set_has_render_object(true);
        draw.set_num_frames(15.0);
        let initial_frame = draw.cur_frame;
        draw.do_draw_module(&Matrix4::identity());
        assert!((draw.cur_frame - (initial_frame + ANIM_INCREMENT)).abs() < 0.001);
    }

    #[test]
    fn test_wthree_d_police_car_draw_frame_wrapping() {
        let mut draw = W3DPoliceCarDraw {
            cur_frame: 14.5,
            num_frames: 15.0,
            has_render_object: true,
            ..Default::default()
        };
        draw.do_draw_module(&Matrix4::identity());
        assert_eq!(draw.cur_frame, 0.0);
    }

    #[test]
    fn test_wthree_d_police_car_draw_light_creation() {
        let mut draw = W3DPoliceCarDraw {
            has_render_object: true,
            drawable_pos: Point3::new(10.0, 20.0, 5.0),
            ..Default::default()
        };
        draw.do_draw_module(&Matrix4::identity());
        assert!(draw.light.is_some());
        let light = draw.light.as_ref().unwrap();
        assert!(light.enabled);
        assert_eq!(light.position.z, 5.0 + LIGHT_HEIGHT);
    }

    #[test]
    fn test_wthree_d_police_car_draw_light_color_update() {
        let mut draw = W3DPoliceCarDraw {
            has_render_object: true,
            cur_frame: 10.0,
            drawable_pos: Point3::new(0.0, 0.0, 0.0),
            ..Default::default()
        };
        draw.do_draw_module(&Matrix4::identity());
        let light = draw.light.as_ref().unwrap();
        assert!((light.diffuse.z - 1.0).abs() < 0.01);
        assert!((light.ambient.z - 0.5).abs() < 0.01);
        assert!((light.diffuse.x).abs() < 0.01);
    }

    #[test]
    fn test_wthree_d_police_car_draw_light_attenuation() {
        let mut draw = W3DPoliceCarDraw {
            has_render_object: true,
            drawable_pos: Point3::new(0.0, 0.0, 0.0),
            ..Default::default()
        };
        draw.do_draw_module(&Matrix4::identity());
        let light = draw.light.as_ref().unwrap();
        assert_eq!(light.far_atten_start, 3.0);
        assert_eq!(light.far_atten_end, 20.0);
    }

    #[test]
    fn test_wthree_d_police_car_draw_delete_fades_light() {
        let mut draw = W3DPoliceCarDraw {
            has_render_object: true,
            ..Default::default()
        };
        draw.do_draw_module(&Matrix4::identity());
        assert!(draw.light.is_some());
        draw.on_delete();
        assert!(draw.light.is_none());
    }
}
