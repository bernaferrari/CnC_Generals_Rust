//! Map-window supply/tech overlay + MapSelectorTooltip.
//!
//! C++ `SkirmishGameOptionsMenu.cpp`: `positionAdditionalImages`,
//! `MapSelectorTooltip`, `windowMap->winSetTooltipFunc`.

use crate::display::image::get_mapped_image_collection;
use crate::game_text::GameText;
use crate::gui::{GameWindow, WindowInstanceData};
use crate::input::mouse::with_mouse;
use crate::map_util::{find_draw_positions, get_supply_and_tech_image_locations};
use crate::message_stream::game_message::ICoord2D;
use game_engine::common::ini::ini_map_cache::MapMetaData;
use std::cell::RefCell;
use std::rc::Rc;

/// C++ `SUPPLY_TECH_SIZE`.
pub const SUPPLY_TECH_SIZE: i32 = 15;

fn mapped_image_exists(name: &str) -> bool {
    get_mapped_image_collection()
        .try_read()
        .map(|collection| collection.find_image_by_name(name).is_some())
        .unwrap_or(false)
}

fn project_marker_in_window(
    pos: &game_engine::common::ini::ini_map_cache::Coord3D,
    meta: &MapMetaData,
    ul_x: i32,
    ul_y: i32,
    small_width: f32,
    small_height: f32,
) -> ICoord2D {
    let extent_width = (meta.extent.hi.x - meta.extent.lo.x).max(1.0);
    let extent_height = (meta.extent.hi.y - meta.extent.lo.y).max(1.0);
    let ratio_x = (pos.x - meta.extent.lo.x) / extent_width;
    let ratio_y = (pos.y - meta.extent.lo.y) / extent_height;
    ICoord2D {
        x: (ratio_x * small_width) as i32 - SUPPLY_TECH_SIZE / 2 + ul_x,
        y: ((1.0 - ratio_y) * small_height) as i32 - SUPPLY_TECH_SIZE / 2 + ul_y,
    }
}

/// C++ `positionAdditionalImages`. Fills `TheSupplyAndTechImageLocations`
/// from `MapMetaData::m_supplyPositions` / `m_techPositions` in map-window space.
pub fn position_additional_images(
    meta: Option<&MapMetaData>,
    map_window: Option<&Rc<RefCell<GameWindow>>>,
    _force: bool,
) {
    let locations = get_supply_and_tech_image_locations();
    let mut overlay = locations.lock().unwrap_or_else(|e| e.into_inner());
    overlay.supply_positions.clear();
    overlay.tech_positions.clear();

    let Some(meta) = meta else {
        return;
    };
    let Some(map_window) = map_window else {
        return;
    };
    let map_guard = map_window.borrow();
    if map_guard.is_hidden() {
        return;
    }
    let (map_w, map_h) = map_guard.get_size();
    if map_w <= 0 || map_h <= 0 {
        return;
    }
    let (ul, lr) = find_draw_positions(0, 0, map_w, map_h, meta.extent);
    let small_width = (lr.x - ul.x) as f32;
    let small_height = (lr.y - ul.y) as f32;

    // C++ `push_front` while walking begin→end: last map pos is first in the list.
    for pos in meta.supply_positions.iter().rev() {
        overlay.supply_positions.push(project_marker_in_window(
            pos,
            meta,
            ul.x,
            ul.y,
            small_width,
            small_height,
        ));
    }
    for pos in meta.tech_positions.iter().rev() {
        overlay.tech_positions.push(project_marker_in_window(
            pos,
            meta,
            ul.x,
            ul.y,
            small_width,
            small_height,
        ));
    }
}

/// C++ `MapSelectorTooltip` — hit-tests supply/tech icon rects on the map preview.
pub fn map_selector_tooltip(window: &GameWindow, _inst: &WindowInstanceData, mouse: u32) {
    let x = (mouse & 0xFFFF) as i16 as i32;
    let y = (mouse >> 16) as i16 as i32;
    let (pixel_x, pixel_y) = window.get_screen_position();

    let locations = get_supply_and_tech_image_locations();
    let overlay = locations.lock().unwrap_or_else(|e| e.into_inner());

    if mapped_image_exists("TecBuilding") {
        for pos in &overlay.tech_positions {
            if x > pixel_x + pos.x
                && x < pixel_x + pos.x + SUPPLY_TECH_SIZE
                && y > pixel_y + pos.y
                && y < pixel_y + pos.y + SUPPLY_TECH_SIZE
            {
                let text = GameText::fetch("TOOLTIP:TechBuilding");
                with_mouse(|mouse| mouse.set_cursor_tooltip(text, Some(-1), None, None));
                return;
            }
        }
    }

    if mapped_image_exists("Cash") {
        for pos in &overlay.supply_positions {
            if x > pixel_x + pos.x
                && x < pixel_x + pos.x + SUPPLY_TECH_SIZE
                && y > pixel_y + pos.y
                && y < pixel_y + pos.y + SUPPLY_TECH_SIZE
            {
                let text = GameText::fetch("TOOLTIP:SupplyDock");
                with_mouse(|mouse| mouse.set_cursor_tooltip(text, Some(-1), None, None));
                return;
            }
        }
    }
}

/// C++ `windowMap->winSetTooltipFunc(MapSelectorTooltip)`.
pub fn bind_map_selector_tooltip(map_window: &Option<Rc<RefCell<GameWindow>>>) {
    let Some(window) = map_window else {
        return;
    };
    window
        .borrow_mut()
        .set_tooltip_callback(map_selector_tooltip);
}
