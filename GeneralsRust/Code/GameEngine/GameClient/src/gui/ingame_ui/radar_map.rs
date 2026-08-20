// C++ Radar::localPixelToRadar / screenPixelToWorld (Radar.cpp:692-788).
// Included by ingame_ui/mod.rs.

/// Convert a pixel inside the radar window to radar-cell coords.
/// Letterbox bars return None. Y is inverted to match `radar_to_pixel`.
pub fn local_pixel_to_radar_cell(
    local_x: i32,
    local_y: i32,
    width: i32,
    height: i32,
    extent_w: f32,
    extent_h: f32,
) -> Option<game_engine::common::system::radar::ICoord2D> {
    use game_engine::common::system::radar::{ICoord2D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH};

    if width <= 0 || height <= 0 || extent_w <= 0.0 || extent_h <= 0.0 {
        return None;
    }
    let ratio_width = extent_w / width as f32;
    let ratio_height = extent_h / height as f32;
    let (ul_x, ul_y, lr_x, lr_y) = if ratio_width >= ratio_height {
        let radar_x = extent_w / ratio_width;
        let radar_y = extent_h / ratio_width;
        let ul_y = ((height as f32 - radar_y) / 2.0) as i32;
        (0, ul_y, radar_x as i32, height - ul_y)
    } else {
        let radar_x = extent_w / ratio_height;
        let radar_y = extent_h / ratio_height;
        let ul_x = ((width as f32 - radar_x) / 2.0) as i32;
        (ul_x, 0, width - ul_x, radar_y as i32)
    };
    if local_x < ul_x || local_x > lr_x || local_y < ul_y || local_y > lr_y {
        return None;
    }
    let scaled_width = (lr_x - ul_x).max(1);
    let scaled_height = (lr_y - ul_y).max(1);

    let (radar_x, radar_y) = if scaled_width >= scaled_height {
        let radar_x = (local_x - ul_x) * RADAR_CELL_WIDTH as i32 / scaled_width;
        let mut radar_y =
            (((local_y - ul_y) as f32 / scaled_height as f32) * height as f32) as i32;
        radar_y = (height - radar_y) * RADAR_CELL_HEIGHT as i32 / height;
        (radar_x, radar_y)
    } else {
        let mut radar_x = (((local_x - ul_x) as f32 / scaled_width as f32) * width as f32) as i32;
        radar_x = radar_x * RADAR_CELL_WIDTH as i32 / width;
        let radar_y = (height - local_y) * RADAR_CELL_HEIGHT as i32 / height;
        (radar_x, radar_y)
    };

    Some(ICoord2D::new(radar_x, radar_y))
}

/// C++ `Radar::screenPixelToWorld`.
pub fn radar_screen_pixel_to_world(mx: i32, my: i32) -> Option<Coord3D> {
    use game_engine::common::system::radar::get_radar_system;

    let radar = get_radar_system();
    let radar = radar.read().ok()?;
    let local_has_radar = player_list()
        .read()
        .ok()
        .and_then(|list| list.get_local_player().cloned())
        .and_then(|player| player.read().ok().map(|g| g.has_radar()))
        .unwrap_or(false);
    let radar_on = radar.is_radar_forced() || (!radar.is_radar_hidden() && local_has_radar);
    if !radar_on {
        return None;
    }
    let key = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
        "ControlBar.wnd:LeftHUD",
    );
    with_window_manager_ref(|manager| {
        let window = manager.get_window_by_id(key as i32)?;
        let (wx, wy) = window.borrow().get_screen_position();
        let (ww, wh) = window.borrow().get_size();
        let lx = mx - wx;
        let ly = my - wy;
        let extent = radar.map_extent();
        let cell = local_pixel_to_radar_cell(
            lx,
            ly,
            ww,
            wh,
            extent.hi.x - extent.lo.x,
            extent.hi.y - extent.lo.y,
        )?;
        radar
            .radar_to_world(&cell)
            .map(|p| Coord3D::new(p.x, p.y, p.z))
    })
}
