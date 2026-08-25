// W3D device game client modules (C++: W3DDevice/GameClient)
//
// Shipped merge of the excluded Code/GameEngineDevice wrappers: gadget draw,
// display light pulses, view shake, scorch, and ray FX all resolve to
// GameClient wgpu rather than a parallel D3D tree.

pub mod w3_d_bib_buffer;
pub mod w3_d_bridge_buffer;
pub mod w3_d_custom_edging;
pub mod w3_d_debug_icons;
pub mod w3_d_display;
pub mod w3_d_dynamic_light;
pub mod w3_d_file_system;
pub mod w3_d_font_chars;
pub mod w3_d_gadget_shims;
pub mod w3_d_game_client_fx;
pub mod w3_d_poly;
pub mod w3_d_prop_buffer;
pub mod w3_d_status_circle;
pub mod w3_d_view;
pub mod w3_d_waypoint_buffer;

pub use w3_d_display::{
    DisplayDynamicLight, DisplayLightPulse, create_light_pulse, do_the_dynamic_light,
    do_the_dynamic_light_from_scene, do_the_dynamic_light_wgpu, far_atten_factor,
};
pub use w3_d_gadget_shims::*;
pub use w3_d_game_client_fx::{add_scorch, create_ray_effect_by_template};
pub use w3_d_view::{CameraShakeType, shake};
