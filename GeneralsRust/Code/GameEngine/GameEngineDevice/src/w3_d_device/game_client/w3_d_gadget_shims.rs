//! W3D gadget draw shims → shipped GameClient wgpu gadget path.
//!
//! C++ `W3DDevice/GameClient/GUI/Gadget/W3D*`. The Device crate stays out of
//! the workspace until wrappers land here; these re-exports are that merge.

pub use game_client::gui::w3d_gadget_draw::{
    w3d_gadget_check_box_draw, w3d_gadget_check_box_image_draw, w3d_gadget_combo_box_draw,
    w3d_gadget_combo_box_image_draw, w3d_gadget_horizontal_slider_draw,
    w3d_gadget_horizontal_slider_image_draw, w3d_gadget_horizontal_slider_image_draw_a,
    w3d_gadget_horizontal_slider_image_draw_b, w3d_gadget_list_box_draw,
    w3d_gadget_list_box_image_draw, w3d_gadget_progress_bar_draw,
    w3d_gadget_progress_bar_image_draw, w3d_gadget_progress_bar_image_draw_a,
    w3d_gadget_push_button_draw, w3d_gadget_push_button_image_draw, w3d_gadget_radio_button_draw,
    w3d_gadget_radio_button_image_draw, w3d_gadget_static_text_draw,
    w3d_gadget_static_text_image_draw, w3d_gadget_tab_control_draw,
    w3d_gadget_tab_control_image_draw, w3d_gadget_text_entry_draw,
    w3d_gadget_text_entry_image_draw, w3d_gadget_vertical_slider_draw,
    w3d_gadget_vertical_slider_image_draw,
};

#[cfg(test)]
mod tests {
    use super::*;
    use game_client::gui::w3d_gadget_draw;

    #[test]
    fn gadget_shims_are_gameclient_wgpu_callbacks() {
        assert_eq!(
            w3d_gadget_push_button_draw as *const (),
            w3d_gadget_draw::w3d_gadget_push_button_draw as *const ()
        );
        assert_eq!(
            w3d_gadget_list_box_draw as *const (),
            w3d_gadget_draw::w3d_gadget_list_box_draw as *const ()
        );
        assert_eq!(
            w3d_gadget_combo_box_image_draw as *const (),
            w3d_gadget_draw::w3d_gadget_combo_box_image_draw as *const ()
        );
        assert_eq!(
            w3d_gadget_progress_bar_image_draw_a as *const (),
            w3d_gadget_draw::w3d_gadget_progress_bar_image_draw_a as *const ()
        );
        assert_eq!(
            w3d_gadget_tab_control_draw as *const (),
            w3d_gadget_draw::w3d_gadget_tab_control_draw as *const ()
        );
    }
}
