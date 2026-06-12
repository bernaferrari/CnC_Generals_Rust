//! DrawGroupInfo defaults for group number rendering.

use std::sync::{OnceLock, RwLock};

use crate::color::{game_make_color, Color};
use game_engine::common::ini::ini_draw_group_info as common_draw_group_info;

#[derive(Debug, Clone)]
pub struct DrawGroupInfo {
    pub font_name: String,
    pub font_size: i32,
    pub font_is_bold: bool,
    pub use_player_color: bool,
    pub color_for_text: Color,
    pub color_for_text_drop_shadow: Color,
    pub drop_shadow_offset_x: i32,
    pub drop_shadow_offset_y: i32,
    pub pixel_offset_x: i32,
    pub percent_offset_x: f32,
    pub using_pixel_offset_x: bool,
    pub pixel_offset_y: i32,
    pub percent_offset_y: f32,
    pub using_pixel_offset_y: bool,
}

impl Default for DrawGroupInfo {
    fn default() -> Self {
        Self {
            font_name: "Arial".to_string(),
            font_size: 10,
            font_is_bold: false,
            use_player_color: true,
            color_for_text: game_make_color(255, 255, 255, 255),
            color_for_text_drop_shadow: game_make_color(0, 0, 0, 255),
            drop_shadow_offset_x: -1,
            drop_shadow_offset_y: -1,
            pixel_offset_x: 0,
            percent_offset_x: -0.05,
            using_pixel_offset_x: false,
            pixel_offset_y: -10,
            percent_offset_y: 0.0,
            using_pixel_offset_y: true,
        }
    }
}

impl DrawGroupInfo {
    pub fn apply_common(&mut self, info: &common_draw_group_info::DrawGroupInfo) {
        self.font_name = info.font.name.clone();
        self.font_size = info.font.size;
        self.font_is_bold = info.font.is_bold;
        self.use_player_color = info.use_player_color;
        self.color_for_text = common_color_to_game_color(info.color_for_text);
        self.color_for_text_drop_shadow =
            common_color_to_game_color(info.color_for_text_drop_shadow);
        self.drop_shadow_offset_x = info.drop_shadow_offset_x;
        self.drop_shadow_offset_y = info.drop_shadow_offset_y;
        self.using_pixel_offset_x = info.offset_x.using_pixel;
        if info.offset_x.using_pixel {
            self.pixel_offset_x = info.offset_x.value as i32;
        } else {
            self.percent_offset_x = info.offset_x.value;
        }
        self.using_pixel_offset_y = info.offset_y.using_pixel;
        if info.offset_y.using_pixel {
            self.pixel_offset_y = info.offset_y.value as i32;
        } else {
            self.percent_offset_y = info.offset_y.value;
        }
    }
}

static DRAW_GROUP_INFO: OnceLock<RwLock<DrawGroupInfo>> = OnceLock::new();

pub fn get_draw_group_info() -> &'static RwLock<DrawGroupInfo> {
    DRAW_GROUP_INFO.get_or_init(|| RwLock::new(DrawGroupInfo::default()))
}

pub fn sync_from_common_draw_group_info(info: &common_draw_group_info::DrawGroupInfo) {
    let mut runtime = get_draw_group_info()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    runtime.apply_common(info);
}

fn common_color_to_game_color(color: common_draw_group_info::Color) -> Color {
    game_make_color(color.r, color.g, color.b, color.a)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_common_preserves_ini_percent_and_pixel_offsets() {
        let mut common = common_draw_group_info::DrawGroupInfo::new();
        common.use_player_color = false;
        common.color_for_text = common_draw_group_info::Color::new(1, 2, 3, 4);
        common.color_for_text_drop_shadow = common_draw_group_info::Color::new(5, 6, 7, 8);
        common.drop_shadow_offset_x = 9;
        common.drop_shadow_offset_y = 10;
        common.set_draw_position_x_percent(-0.2);
        common.set_draw_position_y_pixel(-10);

        let mut runtime = DrawGroupInfo::default();
        runtime.apply_common(&common);

        assert!(!runtime.use_player_color);
        assert_eq!(runtime.color_for_text, game_make_color(1, 2, 3, 4));
        assert_eq!(
            runtime.color_for_text_drop_shadow,
            game_make_color(5, 6, 7, 8)
        );
        assert_eq!(runtime.drop_shadow_offset_x, 9);
        assert_eq!(runtime.drop_shadow_offset_y, 10);
        assert!(!runtime.using_pixel_offset_x);
        assert_eq!(runtime.percent_offset_x, -0.2);
        assert!(runtime.using_pixel_offset_y);
        assert_eq!(runtime.pixel_offset_y, -10);
    }
}
