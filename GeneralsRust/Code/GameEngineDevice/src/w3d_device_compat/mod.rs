//! Auto-generated compatibility wrappers for W3DDevice
#[cfg(feature = "legacy-full")]
pub mod w3_d_asset_manager;
#[cfg(feature = "legacy-full")]
pub mod w3_d_asset_manager_exposed;
#[cfg(feature = "legacy-full")]
pub mod w3_d_bib_buffer;
#[cfg(feature = "legacy-full")]
pub mod w3_d_bridge_buffer;
#[cfg(feature = "legacy-full")]
pub mod w3_d_buffer_manager;
pub mod w3_d_check_box;
#[cfg(feature = "legacy-full")]
pub mod w3_d_combo_box;
#[cfg(feature = "legacy-full")]
pub mod w3_d_control_bar;
#[cfg(feature = "legacy-full")]
pub mod w3_d_convert;
#[cfg(feature = "legacy-full")]
pub mod w3_d_debris_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_debug_display;
#[cfg(feature = "legacy-full")]
pub mod w3_d_default_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_dependency_model_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_display;
#[cfg(feature = "legacy-full")]
pub mod w3_d_display_string;
#[cfg(feature = "legacy-full")]
pub mod w3_d_display_string_manager;
#[cfg(feature = "legacy-full")]
pub mod w3_d_dynamic_light;
#[cfg(feature = "legacy-full")]
pub mod w3_d_file_system;
#[cfg(feature = "legacy-full")]
pub mod w3_d_function_lexicon;
#[cfg(feature = "legacy-full")]
pub mod w3_d_gadget;
#[cfg(feature = "legacy-full")]
pub mod w3_d_game_font;
#[cfg(feature = "legacy-full")]
pub mod w3_d_game_logic;
#[cfg(feature = "legacy-full")]
pub mod w3_d_game_window;
#[cfg(feature = "legacy-full")]
pub mod w3_d_game_window_manager;
#[cfg(feature = "legacy-full")]
pub mod w3_d_ghost_object;
#[cfg(feature = "legacy-full")]
pub mod w3_d_granny;
#[cfg(feature = "legacy-full")]
pub mod w3_d_gui_callbacks;
#[cfg(feature = "legacy-full")]
pub mod w3_d_horizontal_slider;
#[cfg(feature = "legacy-full")]
pub mod w3_d_in_game_ui;
#[cfg(feature = "legacy-full")]
pub mod w3_d_laser_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_list_box;
#[cfg(feature = "legacy-full")]
pub mod w3_d_main_menu;
#[cfg(feature = "legacy-full")]
pub mod w3_d_model_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_module_factory;
#[cfg(feature = "legacy-full")]
pub mod w3_d_motd;
#[cfg(feature = "legacy-full")]
pub mod w3_d_mouse;
#[cfg(feature = "legacy-full")]
pub mod w3_d_overlord_aircraft_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_overlord_tank_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_overlord_truck_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_particle_sys;
#[cfg(feature = "legacy-full")]
pub mod w3_d_police_car_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_progress_bar;
#[cfg(feature = "legacy-full")]
pub mod w3_d_projected_shadow;
#[cfg(feature = "legacy-full")]
pub mod w3_d_projectile_stream_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_prop_buffer;
#[cfg(feature = "legacy-full")]
pub mod w3_d_prop_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_push_button;
#[cfg(feature = "legacy-full")]
pub mod w3_d_radar;
pub mod w3_d_radio_button;
#[cfg(feature = "legacy-full")]
pub mod w3_d_road_buffer;
#[cfg(feature = "legacy-full")]
pub mod w3_d_rope_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_scene;
#[cfg(feature = "legacy-full")]
pub mod w3_d_science_model_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_shader_manager;
#[cfg(feature = "legacy-full")]
pub mod w3_d_shadow;
#[cfg(feature = "legacy-full")]
pub mod w3_d_shroud;
#[cfg(feature = "legacy-full")]
pub mod w3_d_smudge;
#[cfg(feature = "legacy-full")]
pub mod w3_d_snow;
#[cfg(feature = "legacy-full")]
pub mod w3_d_static_text;
#[cfg(feature = "legacy-full")]
pub mod w3_d_supply_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_tab_control;
#[cfg(feature = "legacy-full")]
pub mod w3_d_tank_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_tank_truck_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_terrain_background;
#[cfg(feature = "legacy-full")]
pub mod w3_d_terrain_logic;
#[cfg(feature = "legacy-full")]
pub mod w3_d_terrain_tracks;
#[cfg(feature = "legacy-full")]
pub mod w3_d_terrain_visual;
#[cfg(feature = "legacy-full")]
pub mod w3_d_text_entry;
#[cfg(feature = "legacy-full")]
pub mod w3_d_thing_factory;
#[cfg(feature = "legacy-full")]
pub mod w3_d_tracer_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_tree_buffer;
#[cfg(feature = "legacy-full")]
pub mod w3_d_tree_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_truck_draw;
#[cfg(feature = "legacy-full")]
pub mod w3_d_vertical_slider;
#[cfg(feature = "legacy-full")]
pub mod w3_d_video_buffer;
#[cfg(feature = "legacy-full")]
pub mod w3_d_videobuffer;
#[cfg(feature = "legacy-full")]
pub mod w3_d_view;
#[cfg(feature = "legacy-full")]
pub mod w3_d_volumetric_shadow;
#[cfg(feature = "legacy-full")]
pub mod w3_d_water;
#[cfg(feature = "legacy-full")]
pub mod w3_d_water_tracks;
#[cfg(feature = "legacy-full")]
pub mod w3_d_waypoint_buffer;
#[cfg(feature = "legacy-full")]
pub mod w3_dgui_callbacks;
#[cfg(feature = "legacy-full")]
pub mod w3_dmotd;

#[cfg(test)]
mod tests {
    use super::{w3_d_check_box, w3_d_radio_button};
    use game_client_rust::gui::w3d_gadget_draw::{
        w3d_gadget_check_box_draw, w3d_gadget_check_box_image_draw, w3d_gadget_radio_button_draw,
        w3d_gadget_radio_button_image_draw,
    };

    #[test]
    fn checkbox_and_radio_wrappers_reexport_gameclient_draw_callbacks() {
        assert_eq!(
            w3_d_check_box::w3d_gadget_check_box_draw as *const (),
            w3d_gadget_check_box_draw as *const ()
        );
        assert_eq!(
            w3_d_check_box::w3d_gadget_check_box_image_draw as *const (),
            w3d_gadget_check_box_image_draw as *const ()
        );
        assert_eq!(
            w3_d_radio_button::w3d_gadget_radio_button_draw as *const (),
            w3d_gadget_radio_button_draw as *const ()
        );
        assert_eq!(
            w3_d_radio_button::w3d_gadget_radio_button_image_draw as *const (),
            w3d_gadget_radio_button_image_draw as *const ()
        );
    }
}
