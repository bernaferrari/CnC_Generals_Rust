//! `legacy-full` leftover Device wrappers.
//!
//! Compiles extra archive-facing names as **GameClient wgpu** re-exports.
//! The 55k-LOC W3D/Miles/Win32 tree stays on disk and is not a D3D backend.

pub use game_client::effects::ensure_generic_tracer_ini as leftover_ensure_generic_tracer_ini;
pub use game_client::effects::ray_effect_system::{
    bake_ray_effect_gpu_mesh, create_ray_effect_by_template as leftover_create_ray_effect,
};
pub use game_client::effects::tracer_fx::{
    bake_tracer_gpu_mesh, create_tracer_fx, live_tracer_drawables,
    spawn_tracer_drawable_like_cpp as leftover_spawn_fxlist_tracer, tracer_line3d_local_endpoints,
    tracer_opacity_after_frames, tracer_world_endpoints,
};
pub use game_client::fx_list::{do_the_dynamic_light, far_atten_factor};
pub use game_client::terrain::tree_buffer::{
    blit_tree_tile_into_atlas, do_tree_atlas_mip, generate_box_mip_chain,
};
pub use game_engine_device::w3_d_device::game_client::{
    CameraShakeType, DisplayDynamicLight, DisplayLightPulse, add_scorch, create_light_pulse,
    create_ray_effect_by_template, do_the_dynamic_light_from_scene, do_the_dynamic_light_wgpu,
    shake,
};

/// True only when leftover crate is built with `--features legacy-full`.
#[inline]
pub const fn leftover_legacy_full_enabled() -> bool {
    true
}
