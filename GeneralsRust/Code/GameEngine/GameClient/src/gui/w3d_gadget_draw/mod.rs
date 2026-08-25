//! W3D gadget draw callbacks (push button) for device-style rendering.

// Restricted re-exports so impl submodules can `use super::*;`
// without dumping the parent crate surface through `pub use`.
pub(in crate::gui::w3d_gadget_draw) use crate::display::image::{
    ensure_client_mapped_image, get_mapped_image_collection,
};
pub(in crate::gui::w3d_gadget_draw) use crate::display::view::{IPoint2, with_tactical_view_ref};
pub(in crate::gui::w3d_gadget_draw) use crate::gui::callbacks::get_menu_manager;
pub(in crate::gui::w3d_gadget_draw) use crate::gui::display_string::DisplayString;
pub(in crate::gui::w3d_gadget_draw) use crate::gui::font::{FontDesc, get_font_library};
pub(in crate::gui::w3d_gadget_draw) use crate::gui::gadgets::tabcontrol::{
    TP_BOTTOM_SIDE, TP_BOTTOMRIGHT, TP_CENTER, TP_LEFT_SIDE, TP_RIGHT_SIDE, TP_TOP_SIDE,
};
pub(in crate::gui::w3d_gadget_draw) use crate::gui::gadgets::{
    ClockMode, PushButton, TabControl, TextAlignment, TextEntry, VerticalAlignment,
};
pub(in crate::gui::w3d_gadget_draw) use crate::gui::game_window::{
    GWS_COMBO_BOX, WIN_COLOR_UNDEFINED, WindowId, WindowState, WindowStatus, read_video_frame,
    resolve_window_text,
};
pub(in crate::gui::w3d_gadget_draw) use crate::gui::shell::get_shell;
pub(in crate::gui::w3d_gadget_draw) use crate::gui::ui_globals::with_ui_renderer_mut;
pub(in crate::gui::w3d_gadget_draw) use crate::gui::ui_renderer::UIRect;
pub(in crate::gui::w3d_gadget_draw) use crate::gui::window_manager::{
    with_window_manager, with_window_manager_ref,
};
pub(in crate::gui::w3d_gadget_draw) use crate::gui::{GameWindow, WindowInstanceData};
pub(in crate::gui::w3d_gadget_draw) use crate::helpers::TheControlBar;
pub(in crate::gui::w3d_gadget_draw) use crate::map_util::{
    find_draw_positions, get_supply_and_tech_image_locations,
};
pub(in crate::gui::w3d_gadget_draw) use crate::message_stream::game_message::IRegion2D;
pub(in crate::gui::w3d_gadget_draw) use chrono::Local;
pub(in crate::gui::w3d_gadget_draw) use game_engine::common::ini::ICoord2D;
pub(in crate::gui::w3d_gadget_draw) use game_engine::common::ini::SchemeDrawFunc;
pub(in crate::gui::w3d_gadget_draw) use game_engine::common::ini::get_control_bar_scheme_manager;
pub(in crate::gui::w3d_gadget_draw) use game_engine::common::ini::get_global_data;
pub(in crate::gui::w3d_gadget_draw) use game_engine::common::ini::ini_map_cache::MapMetaData;
pub(in crate::gui::w3d_gadget_draw) use game_engine::common::ini::set_scheme_draw_func;
pub(in crate::gui::w3d_gadget_draw) use game_engine::common::system::radar::{
    Coord3D, RGBAColorInt, RadarEventMarkerKind, RadarEventType, Region3D, get_radar_system,
    radar_draw_positions, radar_event_marker, should_refresh_w3d_object_overlay,
};
pub(in crate::gui::w3d_gadget_draw) use gamelogic::player::{RankProgressInfo, ThePlayerList};
pub(in crate::gui::w3d_gadget_draw) use std::sync::atomic::{AtomicU8, Ordering};
pub(in crate::gui::w3d_gadget_draw) use std::sync::{Arc, Mutex, OnceLock};
pub(in crate::gui::w3d_gadget_draw) use std::time::Instant;

mod common;
pub use common::*;
mod main_menu;
pub use main_menu::*;
mod hud;
pub use hud::*;
mod power;
pub use power::*;
mod command_bar;
pub use command_bar::*;
mod push_button;
pub use push_button::*;
mod static_text;
pub use static_text::*;
mod progress;
pub use progress::*;
mod check_radio;
pub use check_radio::*;
mod slider;
pub use slider::*;
mod text_entry;
pub use text_entry::*;
mod list_box;
pub use list_box::*;
mod tab_combo;
pub use tab_combo::*;
mod map_preview;
pub use map_preview::*;

#[cfg(test)]
mod tests;

/// Concatenated live sources for residual `include_str!` scans.
pub const W3D_GADGET_DRAW_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("common.rs"),
    include_str!("main_menu.rs"),
    include_str!("hud.rs"),
    include_str!("power.rs"),
    include_str!("command_bar.rs"),
    include_str!("push_button.rs"),
    include_str!("static_text.rs"),
    include_str!("progress.rs"),
    include_str!("check_radio.rs"),
    include_str!("slider.rs"),
    include_str!("text_entry.rs"),
    include_str!("list_box.rs"),
    include_str!("tab_combo.rs"),
    include_str!("map_preview.rs"),
);
