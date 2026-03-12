//! W3DGameWindowManager wrapper (device implementation).
//!
//! C++ reference: W3DDevice/GameClient/W3DGameWindowManager.h

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};
use game_client_rust::gui::WindowManager;

use crate::W3DDevice::GameClient::GUI::Gadget::{
    wthree_d_check_box::{w3d_gadget_check_box_draw, w3d_gadget_check_box_image_draw},
    wthree_d_combo_box::{w3d_gadget_combo_box_draw, w3d_gadget_combo_box_image_draw},
    wthree_d_horizontal_slider::{
        w3d_gadget_horizontal_slider_draw, w3d_gadget_horizontal_slider_image_draw,
    },
    wthree_d_list_box::{w3d_gadget_list_box_draw, w3d_gadget_list_box_image_draw},
    wthree_d_progress_bar::{w3d_gadget_progress_bar_draw, w3d_gadget_progress_bar_image_draw},
    wthree_d_push_button::{w3d_gadget_push_button_draw, w3d_gadget_push_button_image_draw},
    wthree_d_radio_button::{w3d_gadget_radio_button_draw, w3d_gadget_radio_button_image_draw},
    wthree_d_static_text::{w3d_gadget_static_text_draw, w3d_gadget_static_text_image_draw},
    wthree_d_tab_control::{w3d_gadget_tab_control_draw, w3d_gadget_tab_control_image_draw},
    wthree_d_text_entry::{w3d_gadget_text_entry_draw, w3d_gadget_text_entry_image_draw},
    wthree_d_vertical_slider::{w3d_gadget_vertical_slider_draw, w3d_gadget_vertical_slider_image_draw},
};

use crate::W3DDevice::GameClient::wthree_d_game_window::WthreeDGameWindow;

pub type GameWinDrawFunc = fn(&GameWindow, &WindowInstanceData);

fn w3d_default_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}

/// W3D implementation of the game window manager.
pub struct WthreeDGameWindowManager {
    inner: WindowManager,
}

impl WthreeDGameWindowManager {
    pub fn new() -> Self {
        Self {
            inner: WindowManager::new(),
        }
    }

    pub fn init(&mut self) {
        self.inner.init();
    }

    pub fn reset(&mut self) {
        self.inner.reset();
    }

    pub fn inner(&self) -> &WindowManager {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut WindowManager {
        &mut self.inner
    }

    pub fn allocate_new_window(&self) -> WthreeDGameWindow {
        WthreeDGameWindow::new()
    }

    pub fn get_default_draw(&self) -> GameWinDrawFunc {
        w3d_default_draw
    }

    pub fn get_push_button_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_push_button_image_draw
    }

    pub fn get_push_button_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_push_button_draw
    }

    pub fn get_check_box_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_check_box_image_draw
    }

    pub fn get_check_box_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_check_box_draw
    }

    pub fn get_radio_button_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_radio_button_image_draw
    }

    pub fn get_radio_button_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_radio_button_draw
    }

    pub fn get_tab_control_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_tab_control_image_draw
    }

    pub fn get_tab_control_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_tab_control_draw
    }

    pub fn get_list_box_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_list_box_image_draw
    }

    pub fn get_list_box_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_list_box_draw
    }

    pub fn get_combo_box_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_combo_box_image_draw
    }

    pub fn get_combo_box_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_combo_box_draw
    }

    pub fn get_horizontal_slider_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_horizontal_slider_image_draw
    }

    pub fn get_horizontal_slider_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_horizontal_slider_draw
    }

    pub fn get_vertical_slider_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_vertical_slider_image_draw
    }

    pub fn get_vertical_slider_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_vertical_slider_draw
    }

    pub fn get_progress_bar_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_progress_bar_image_draw
    }

    pub fn get_progress_bar_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_progress_bar_draw
    }

    pub fn get_static_text_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_static_text_image_draw
    }

    pub fn get_static_text_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_static_text_draw
    }

    pub fn get_text_entry_image_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_text_entry_image_draw
    }

    pub fn get_text_entry_draw_func(&self) -> GameWinDrawFunc {
        w3d_gadget_text_entry_draw
    }
}

impl Default for WthreeDGameWindowManager {
    fn default() -> Self {
        Self::new()
    }
}
