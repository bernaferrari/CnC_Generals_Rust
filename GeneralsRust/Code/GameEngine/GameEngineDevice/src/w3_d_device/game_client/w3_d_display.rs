//! W3DDisplay shim → GameClient wgpu FXList/display path.
//!
//! C++ `W3DDisplay::createLightPulse` is invoked by FXList LightPulse nuggets.

pub use game_client::fx_list::{
    DisplayDynamicLight, DisplayLightPulse, create_display_light_pulse, do_the_dynamic_light,
    do_the_dynamic_light_from_scene, drain_display_light_pulses, far_atten_factor,
    light_pulse_too_small, scene_dynamic_lights,
};

/// C++ `W3DShaderManager::startRenderToTexture` / `endRenderToTexture` /
/// `filterPostRender` — live GameClient wgpu analog.
pub use game_client::display::shader_filter::{
    MOTION_BLUR_MAX_COUNT, end_render_to_texture, filter_post_render, start_render_to_texture,
};

/// C++ `TheDisplay->createLightPulse`.
pub fn create_light_pulse(pulse: DisplayLightPulse) -> bool {
    create_display_light_pulse(pulse)
}

/// C++ `HeightMapRenderObjClass::doTheDynamicLight` on the GameClient wgpu light list.
pub fn do_the_dynamic_light_wgpu(
    vertex_xyz: [f32; 3],
    vertex_normal: [f32; 3],
    vertex_diffuse: u32,
) -> u32 {
    do_the_dynamic_light_from_scene(vertex_xyz, vertex_normal, vertex_diffuse)
}

/// C++ RTS2DScene `W3DStatusCircle::Render` after the 3D scene.
pub fn draw_status_circle_overlay()
-> Option<crate::w3_d_device::game_client::w3_d_status_circle::CameraFadeOverlay> {
    crate::w3_d_device::game_client::w3_d_status_circle::render_camera_fade()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_light_pulse_forwards_to_gameclient_display() {
        let _ = drain_display_light_pulses();
        assert!(create_light_pulse(DisplayLightPulse {
            pos: [10.0, 20.0, 5.0],
            color: [1.0, 0.0, 0.0],
            inner_radius: 1.0,
            outer_radius: 80.0,
            increase_frames: 2,
            decay_frames: 8,
        }));
        let pulses = drain_display_light_pulses();
        assert_eq!(pulses.len(), 1);
        assert_eq!(pulses[0].inner_radius, 1.0);
        assert_eq!(pulses[0].outer_radius, 80.0);
        assert!(!light_pulse_too_small(1.0, 80.0));
    }
}
