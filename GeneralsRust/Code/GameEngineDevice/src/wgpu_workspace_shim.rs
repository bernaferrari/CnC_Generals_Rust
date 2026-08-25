//! Leftover `Code/GameEngineDevice` default target.
//!
//! The 55k-LOC archive stays on disk (optional `legacy-full`). Default build
//! is a compiling workspace member that re-exports workspace
//! `GameEngine/GameEngineDevice` → GameClient wgpu shims. Not a D3D revival.

pub use game_engine_device::w3_d_device::game_client::{
    CameraShakeType, DisplayDynamicLight, DisplayLightPulse, add_scorch, create_light_pulse,
    create_ray_effect_by_template, do_the_dynamic_light, do_the_dynamic_light_from_scene,
    do_the_dynamic_light_wgpu, far_atten_factor, shake, w3d_gadget_check_box_draw,
    w3d_gadget_check_box_image_draw, w3d_gadget_combo_box_draw, w3d_gadget_combo_box_image_draw,
    w3d_gadget_horizontal_slider_draw, w3d_gadget_horizontal_slider_image_draw,
    w3d_gadget_horizontal_slider_image_draw_a, w3d_gadget_horizontal_slider_image_draw_b,
    w3d_gadget_list_box_draw, w3d_gadget_list_box_image_draw, w3d_gadget_progress_bar_draw,
    w3d_gadget_progress_bar_image_draw, w3d_gadget_progress_bar_image_draw_a,
    w3d_gadget_push_button_draw, w3d_gadget_push_button_image_draw, w3d_gadget_radio_button_draw,
    w3d_gadget_radio_button_image_draw, w3d_gadget_static_text_draw,
    w3d_gadget_static_text_image_draw, w3d_gadget_tab_control_draw,
    w3d_gadget_tab_control_image_draw, w3d_gadget_text_entry_draw,
    w3d_gadget_text_entry_image_draw, w3d_gadget_vertical_slider_draw,
    w3d_gadget_vertical_slider_image_draw,
};

/// `legacy-full` leftover wrappers (still wgpu — not D3D).
#[cfg(feature = "legacy-full")]
pub mod leftover_legacy_full;

#[cfg(feature = "legacy-full")]
pub use leftover_legacy_full::leftover_legacy_full_enabled;

#[cfg(not(feature = "legacy-full"))]
pub const fn leftover_legacy_full_enabled() -> bool {
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn leftover_device_reexports_workspace_gameclient_wgpu_gadgets() {
        use game_client::gui::w3d_gadget_draw;
        assert_eq!(
            super::w3d_gadget_push_button_draw as *const (),
            w3d_gadget_draw::w3d_gadget_push_button_draw as *const ()
        );
        assert_eq!(
            super::w3d_gadget_list_box_draw as *const (),
            w3d_gadget_draw::w3d_gadget_list_box_draw as *const ()
        );
        assert_eq!(
            super::w3d_gadget_check_box_draw as *const (),
            w3d_gadget_draw::w3d_gadget_check_box_draw as *const ()
        );
        assert_eq!(
            super::w3d_gadget_radio_button_draw as *const (),
            w3d_gadget_draw::w3d_gadget_radio_button_draw as *const ()
        );
        assert_eq!(
            super::w3d_gadget_text_entry_draw as *const (),
            w3d_gadget_draw::w3d_gadget_text_entry_draw as *const ()
        );
        assert_eq!(
            super::w3d_gadget_tab_control_draw as *const (),
            w3d_gadget_draw::w3d_gadget_tab_control_draw as *const ()
        );
        assert_eq!(
            super::w3d_gadget_horizontal_slider_draw as *const (),
            w3d_gadget_draw::w3d_gadget_horizontal_slider_draw as *const ()
        );
        assert_eq!(
            super::w3d_gadget_vertical_slider_draw as *const (),
            w3d_gadget_draw::w3d_gadget_vertical_slider_draw as *const ()
        );
    }

    #[test]
    fn leftover_device_fx_wrappers_hit_gameclient_wgpu() {
        use game_client::fx_list::drain_display_light_pulses;
        use game_client::terrain::scorch_mesh::{clear_terrain_scorches, terrain_scorch_count};

        clear_terrain_scorches();
        assert!(super::add_scorch([8.0, 9.0, 0.0], 12.0, 2));
        assert_eq!(terrain_scorch_count(), 1);
        clear_terrain_scorches();

        let _ = drain_display_light_pulses();
        game_client::fx_list::clear_scene_dynamic_lights();
        assert!(super::create_light_pulse(super::DisplayLightPulse {
            pos: [0.0, 0.0, 0.0],
            color: [1.0, 0.0, 0.0],
            inner_radius: 10.0,
            outer_radius: 30.0,
            increase_frames: 0,
            decay_frames: 0,
        }));
        assert_eq!(drain_display_light_pulses().len(), 1);
        let factor = super::far_atten_factor(20.0, 10.0, 40.0).expect("mid-range");
        let expected_ambient = 0xFF00_0000 | (((factor * 255.0) as u32) << 16);
        assert_eq!(
            super::do_the_dynamic_light_wgpu([0.0, 0.0, 20.0], [0.0, 0.0, -1.0], 0xFF00_0000),
            0xFFFF_0000
        );
        assert_eq!(
            super::do_the_dynamic_light_from_scene([0.0, 0.0, 20.0], [0.0, 0.0, 1.0], 0xFF00_0000),
            expected_ambient
        );
        assert_eq!(
            super::do_the_dynamic_light as *const (),
            game_client::fx_list::do_the_dynamic_light as *const ()
        );
        game_client::fx_list::clear_scene_dynamic_lights();
    }

    #[test]
    fn leftover_legacy_full_feature_is_documented() {
        // Default build is wgpu shim; `--features legacy-full` enables extra wrappers.
        let enabled = super::leftover_legacy_full_enabled();
        #[cfg(feature = "legacy-full")]
        assert!(enabled, "legacy-full must compile leftover wgpu wrappers");
        #[cfg(not(feature = "legacy-full"))]
        assert!(!enabled, "default leftover crate is wgpu shim only");
    }

    #[test]
    fn leftover_device_archive_is_dead_by_default_matching_ghcr16() {
        // C++ GameEngineDevice/Source/W3DDevice is the live W3DDisplay path.
        // Rust leftover archive (this crate) is dead-by-default: `legacy-full` off.
        // Live draws are GameClient wgpu re-exports (hq-ghcr.16 / hq-rdde).
        assert!(
            !super::leftover_legacy_full_enabled(),
            "default leftover crate must keep legacy-full off"
        );
        let cargo = include_str!("../Cargo.toml");
        assert!(
            cargo.contains("default = []") && cargo.contains("legacy-full = []"),
            "leftover archive must not enable legacy-full by default"
        );
        let stub = include_str!("W3DDevice/GameClient/wthree_d_water.rs");
        assert!(
            stub.contains("pub const DEFAULT_VALUE: u32 = 0"),
            "archive DEFAULT_VALUE stubs stay on disk and are not the live path"
        );
    }

    #[cfg(feature = "legacy-full")]
    #[test]
    fn leftover_legacy_full_routes_fx_and_tree_to_gameclient_wgpu() {
        use game_client::effects::tracer_fx;
        use game_client::fx_list;
        assert_eq!(
            super::leftover_legacy_full::create_tracer_fx as *const (),
            tracer_fx::create_tracer_fx as *const ()
        );
        assert_eq!(
            super::leftover_legacy_full::do_the_dynamic_light as *const (),
            fx_list::do_the_dynamic_light as *const ()
        );
        assert_eq!(
            super::leftover_legacy_full::add_scorch as *const (),
            super::add_scorch as *const ()
        );
        assert_eq!(
            super::leftover_legacy_full::create_light_pulse as *const (),
            super::create_light_pulse as *const ()
        );
        assert_eq!(
            super::leftover_legacy_full::leftover_create_ray_effect as *const (),
            super::leftover_legacy_full::create_ray_effect_by_template as *const ()
        );
        assert_eq!(
            super::leftover_legacy_full::leftover_create_ray_effect as *const (),
            game_client::effects::ray_effect_system::create_ray_effect_by_template as *const ()
        );
        assert_eq!(
            super::leftover_legacy_full::tracer_line3d_local_endpoints as *const (),
            tracer_fx::tracer_line3d_local_endpoints as *const ()
        );
        assert_eq!(
            super::leftover_legacy_full::leftover_spawn_fxlist_tracer as *const (),
            tracer_fx::spawn_tracer_drawable_like_cpp as *const ()
        );
        assert!(
            super::leftover_legacy_full::leftover_ensure_generic_tracer_ini(),
            "leftover-full must register C++ System.ini GenericTracer"
        );
        assert_eq!(
            super::leftover_legacy_full::leftover_ensure_generic_tracer_ini as *const (),
            game_client::effects::ensure_generic_tracer_ini as *const ()
        );
        let _guard = tracer_fx::lock_tracer_fx_tests();
        tracer_fx::clear_tracer_fx();
        let spawned = super::leftover_legacy_full::leftover_spawn_fxlist_tracer(
            "GenericTracer",
            [0.0, 0.0, 0.0],
            [40.0, 0.0, 0.0],
            8.0,
            4.0,
            1.0,
            [1.0, 1.0, 1.0],
            1.0,
            0,
        )
        .expect("leftover FXList tracer spawn");
        assert!(
            spawned.used_thing_factory,
            "leftover spawn must hit ThingFactory GenericTracer"
        );
        assert_eq!(spawned.fx.tracer_name, "GenericTracer");
        tracer_fx::clear_tracer_fx();
        assert_eq!(
            super::leftover_legacy_full::tracer_line3d_local_endpoints(12.0),
            ([0.0, 0.0, 0.0], [12.0, 0.0, 0.0])
        );
        let hi = vec![8u8; 4 * 4 * 4];
        let lo = super::leftover_legacy_full::do_tree_atlas_mip(&hi, 4);
        assert_eq!(lo.len(), 2 * 2 * 4);
    }
}
