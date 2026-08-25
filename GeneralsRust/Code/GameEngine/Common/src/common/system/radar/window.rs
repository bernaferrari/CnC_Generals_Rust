//! C++ `Radar::newMap` LeftHUD bind + `screenPixelToWorld` / `localPixelToRadar`.

use super::{
    Coord3D, ICoord2D, RADAR_CELL_HEIGHT, RADAR_CELL_WIDTH, RadarSystem, radar_draw_positions,
};
use crate::common::name_key_generator::NameKeyGenerator;
use std::sync::{Arc, OnceLock};

/// Screen rectangle of `ControlBar.wnd:LeftHUD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarWindowGeom {
    pub screen_x: i32,
    pub screen_y: i32,
    pub width: i32,
    pub height: i32,
}

/// Client window lookup used by `Radar::newMap`.
pub trait RadarWindowSource: Send + Sync {
    fn find_left_hud(&self) -> Option<RadarWindowGeom>;
}

static WINDOW_SOURCE: OnceLock<Arc<dyn RadarWindowSource>> = OnceLock::new();

pub fn register_radar_window_source(source: Arc<dyn RadarWindowSource>) -> bool {
    WINDOW_SOURCE.set(source).is_ok()
}

pub fn left_hud_name_key() -> i32 {
    NameKeyGenerator::name_to_key("ControlBar.wnd:LeftHUD") as i32
}

impl RadarSystem {
    /// C++ `NAMEKEY("ControlBar.wnd:LeftHUD")` + `winGetWindowFromId`.
    pub fn bind_left_hud_window(&mut self) {
        let _ = left_hud_name_key();
        if let Some(source) = WINDOW_SOURCE.get() {
            self.radar_window = source.find_left_hud();
        }
    }

    pub fn set_radar_window(&mut self, geom: Option<RadarWindowGeom>) {
        self.radar_window = geom;
    }

    #[must_use]
    pub fn radar_window(&self) -> Option<RadarWindowGeom> {
        self.radar_window
    }

    /// C++ `Radar::localPixelToRadar`.
    #[must_use]
    pub fn local_pixel_to_radar(&self, pixel: &ICoord2D) -> Option<ICoord2D> {
        let window = self.radar_window?;
        if window.width <= 0 || window.height <= 0 {
            return None;
        }
        let (ul, lr) = radar_draw_positions(0, 0, window.width, window.height, self.map_extent);
        if pixel.x < ul.x || pixel.x > lr.x || pixel.y < ul.y || pixel.y > lr.y {
            return None;
        }
        let scaled_width = (lr.x - ul.x).max(1);
        let scaled_height = (lr.y - ul.y).max(1);
        if scaled_width >= scaled_height {
            let x = (pixel.x - ul.x) * RADAR_CELL_WIDTH as i32 / scaled_width;
            let mut y =
                (((pixel.y - ul.y) as f32 / scaled_height as f32) * window.height as f32) as i32;
            y = (window.height - y) * RADAR_CELL_HEIGHT as i32 / window.height;
            Some(ICoord2D { x, y })
        } else {
            let mut x =
                (((pixel.x - ul.x) as f32 / scaled_width as f32) * window.width as f32) as i32;
            x = x * RADAR_CELL_WIDTH as i32 / window.width;
            let y = (window.height - pixel.y) * RADAR_CELL_HEIGHT as i32 / window.height;
            Some(ICoord2D { x, y })
        }
    }

    /// C++ `Radar::screenPixelToWorld` — false when `m_radarWindow == NULL`.
    #[must_use]
    pub fn screen_pixel_to_world(&self, pixel: &ICoord2D) -> Option<Coord3D> {
        let window = self.radar_window?;
        let local = ICoord2D {
            x: pixel.x - window.screen_x,
            y: pixel.y - window.screen_y,
        };
        let radar = self.local_pixel_to_radar(&local)?;
        self.radar_to_world(&radar)
    }
}
