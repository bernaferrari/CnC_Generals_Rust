use glam::{Vec2, Vec3};
use std::collections::HashMap;

pub const MAX_2D_CURSOR_ANIM_FRAMES: usize = 32;
pub const EDGE_SCROLL_BORDER: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseCursor {
    None,
    Arrow,
    Move,
    Attack,
    Build,
    Select,
    ScrollNorth,
    ScrollSouth,
    ScrollEast,
    ScrollWest,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorRedrawMode {
    Polygon,
    Dx8,
    W3d,
}

#[derive(Debug, Clone)]
pub struct CursorAssetSet {
    pub texture_frames: Vec<String>,
    pub image_name: Option<String>,
    pub w3d_model_name: Option<String>,
    pub w3d_anim_name: Option<String>,
    pub frames_per_second: f32,
    pub hot_spot: Vec2,
    pub looped: bool,
}

impl Default for CursorAssetSet {
    fn default() -> Self {
        Self {
            texture_frames: Vec::new(),
            image_name: None,
            w3d_model_name: None,
            w3d_anim_name: None,
            frames_per_second: 10.0,
            hot_spot: Vec2::ZERO,
            looped: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraScroll {
    pub delta: Vec2,
}

#[derive(Debug, Clone)]
pub struct MouseDrawState {
    pub cursor: MouseCursor,
    pub redraw_mode: CursorRedrawMode,
    pub screen_position: Vec2,
    pub hot_spot: Vec2,
    pub texture_frame: Option<String>,
    pub world_position: Option<Vec3>,
    pub camera_scroll: CameraScroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorContext {
    Normal,
    Move,
    Attack,
    Build,
    Selection,
}

#[derive(Debug)]
pub struct W3DMouse {
    current_cursor: MouseCursor,
    current_anim_frame: usize,
    redraw_mode: CursorRedrawMode,
    screen_size: Vec2,
    current_position: Vec2,
    asset_sets: HashMap<MouseCursor, CursorAssetSet>,
    current_time_accumulator: f32,
}

impl Default for W3DMouse {
    fn default() -> Self {
        let mut asset_sets = HashMap::new();
        asset_sets.insert(
            MouseCursor::Arrow,
            CursorAssetSet {
                texture_frames: vec!["cursor_arrow.tga".to_string()],
                image_name: Some("SCCursorArrow".to_string()),
                ..Default::default()
            },
        );
        asset_sets.insert(
            MouseCursor::Move,
            CursorAssetSet {
                texture_frames: vec![
                    "cursor_move0000.tga".to_string(),
                    "cursor_move0001.tga".to_string(),
                ],
                w3d_model_name: Some("MoveCursor".to_string()),
                w3d_anim_name: Some("MoveCursorLoop".to_string()),
                frames_per_second: 12.0,
                ..Default::default()
            },
        );
        asset_sets.insert(
            MouseCursor::Attack,
            CursorAssetSet {
                texture_frames: vec!["cursor_attack.tga".to_string()],
                ..Default::default()
            },
        );
        asset_sets.insert(
            MouseCursor::Build,
            CursorAssetSet {
                texture_frames: vec!["cursor_build.tga".to_string()],
                ..Default::default()
            },
        );
        asset_sets.insert(
            MouseCursor::Select,
            CursorAssetSet {
                texture_frames: vec!["cursor_select.tga".to_string()],
                ..Default::default()
            },
        );

        Self {
            current_cursor: MouseCursor::Arrow,
            current_anim_frame: 0,
            redraw_mode: CursorRedrawMode::Dx8,
            screen_size: Vec2::new(1024.0, 768.0),
            current_position: Vec2::ZERO,
            asset_sets,
            current_time_accumulator: 0.0,
        }
    }
}

impl W3DMouse {
    pub fn new(screen_width: f32, screen_height: f32) -> Self {
        Self {
            screen_size: Vec2::new(screen_width, screen_height),
            ..Default::default()
        }
    }

    pub fn set_redraw_mode(&mut self, mode: CursorRedrawMode) {
        self.redraw_mode = mode;
    }

    pub fn set_cursor(&mut self, cursor: MouseCursor) {
        if self.current_cursor != cursor {
            self.current_cursor = cursor;
            self.current_anim_frame = 0;
            self.current_time_accumulator = 0.0;
        }
    }

    pub fn set_cursor_from_context(&mut self, context: CursorContext) {
        self.set_cursor(match context {
            CursorContext::Normal => MouseCursor::Arrow,
            CursorContext::Move => MouseCursor::Move,
            CursorContext::Attack => MouseCursor::Attack,
            CursorContext::Build => MouseCursor::Build,
            CursorContext::Selection => MouseCursor::Select,
        });
    }

    pub fn update(&mut self, mouse_position: Vec2, delta_seconds: f32) {
        self.current_position = mouse_position;
        if let Some(assets) = self.asset_sets.get(&self.current_cursor) {
            if assets.texture_frames.len() > 1 && assets.frames_per_second > 0.0 {
                self.current_time_accumulator += delta_seconds.max(0.0);
                let frame_time = 1.0 / assets.frames_per_second;
                while self.current_time_accumulator >= frame_time {
                    self.current_time_accumulator -= frame_time;
                    self.current_anim_frame = (self.current_anim_frame + 1)
                        % assets.texture_frames.len().min(MAX_2D_CURSOR_ANIM_FRAMES);
                }
            }
        }
    }

    pub fn set_cursor_direction(&mut self, cursor: MouseCursor) {
        self.set_cursor(cursor);
    }

    pub fn compute_camera_scroll(&self) -> CameraScroll {
        let mut delta = Vec2::ZERO;
        if self.current_position.x <= EDGE_SCROLL_BORDER {
            delta.x -= 1.0;
        }
        if self.current_position.x >= self.screen_size.x - EDGE_SCROLL_BORDER {
            delta.x += 1.0;
        }
        if self.current_position.y <= EDGE_SCROLL_BORDER {
            delta.y += 1.0;
        }
        if self.current_position.y >= self.screen_size.y - EDGE_SCROLL_BORDER {
            delta.y -= 1.0;
        }
        CameraScroll { delta }
    }

    pub fn draw(&self, world_position: Option<Vec3>) -> MouseDrawState {
        let assets = self.asset_sets.get(&self.current_cursor);
        let hot_spot = assets.map(|asset| asset.hot_spot).unwrap_or(Vec2::ZERO);
        let texture_frame =
            assets.and_then(|asset| asset.texture_frames.get(self.current_anim_frame).cloned());
        MouseDrawState {
            cursor: self.current_cursor,
            redraw_mode: self.redraw_mode,
            screen_position: self.current_position,
            hot_spot,
            texture_frame,
            world_position,
            camera_scroll: self.compute_camera_scroll(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_switches_cursor() {
        let mut mouse = W3DMouse::default();
        mouse.set_cursor_from_context(CursorContext::Attack);
        assert_eq!(mouse.draw(None).cursor, MouseCursor::Attack);
    }

    #[test]
    fn edge_scroll_detected() {
        let mut mouse = W3DMouse::new(100.0, 100.0);
        mouse.update(Vec2::new(1.0, 99.0), 0.016);
        let scroll = mouse.compute_camera_scroll();
        assert_eq!(scroll.delta, Vec2::new(-1.0, -1.0));
    }
}
