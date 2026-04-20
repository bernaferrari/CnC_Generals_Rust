//! Window transition system (C++-faithful port of GameWindowTransitions).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use glam::Vec2;

use super::game_window::{resolve_window_text, Color, GameWindow, WindowDrawData, WindowStatus};
use super::ui_globals::with_ui_renderer;
use super::ui_renderer::{UIRect, UIRenderer};
use crate::display::image::get_mapped_image_collection;
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::TheAudio;

const TRANSITION_FLASH: i32 = 0;
const BUTTON_TRANSITION_FLASH: i32 = 1;
const WIN_FADE_TRANSITION: i32 = 2;
const WIN_SCALE_UP_TRANSITION: i32 = 3;
const MAINMENU_SCALE_UP_TRANSITION: i32 = 4;
const TEXT_TYPE_TRANSITION: i32 = 5;
const SCREEN_FADE_TRANSITION: i32 = 6;
const COUNT_UP_TRANSITION: i32 = 7;
const FULL_FADE_TRANSITION: i32 = 8;
const TEXT_ON_FRAME_TRANSITION: i32 = 9;
const MAINMENU_MEDIUM_SCALE_UP_TRANSITION: i32 = 10;
const MAINMENU_SMALL_SCALE_DOWN_TRANSITION: i32 = 11;
const CONTROL_BAR_ARROW_TRANSITION: i32 = 12;
const SCORE_SCALE_UP_TRANSITION: i32 = 13;
const REVERSE_SOUND_TRANSITION: i32 = 14;

fn transition_style_from_name(token: &str) -> Option<i32> {
    match token.trim().to_ascii_uppercase().as_str() {
        "FLASH" => Some(TRANSITION_FLASH),
        "BUTTONFLASH" => Some(BUTTON_TRANSITION_FLASH),
        "WINFADE" => Some(WIN_FADE_TRANSITION),
        "WINSCALEUP" => Some(WIN_SCALE_UP_TRANSITION),
        "MAINMENUSCALEUP" => Some(MAINMENU_SCALE_UP_TRANSITION),
        "TYPETEXT" => Some(TEXT_TYPE_TRANSITION),
        "SCREENFADE" => Some(SCREEN_FADE_TRANSITION),
        "COUNTUP" => Some(COUNT_UP_TRANSITION),
        "FULLFADE" => Some(FULL_FADE_TRANSITION),
        "TEXTONFRAME" => Some(TEXT_ON_FRAME_TRANSITION),
        "MAINMENUMEDIUMSCALEUP" => Some(MAINMENU_MEDIUM_SCALE_UP_TRANSITION),
        "MAINMENUSMALLSCALEDOWN" => Some(MAINMENU_SMALL_SCALE_DOWN_TRANSITION),
        "CONTROLBARARROW" => Some(CONTROL_BAR_ARROW_TRANSITION),
        "SCORESCALEUP" => Some(SCORE_SCALE_UP_TRANSITION),
        "REVERSESOUND" => Some(REVERSE_SOUND_TRANSITION),
        _ => None,
    }
}

fn parse_bool_token(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "yes" | "true" | "1"
    )
}

fn lookup_window(
    window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    name: &str,
) -> Option<Rc<RefCell<GameWindow>>> {
    let id = NameKeyGenerator::name_to_key(name) as i32;
    window_lookup.get(&id).and_then(|w| w.upgrade())
}

fn rgba_from_color(color: Color, alpha_override: Option<u8>) -> [f32; 4] {
    let a = alpha_override.unwrap_or(((color >> 24) & 0xFF) as u8);
    let r = ((color >> 16) & 0xFF) as u8;
    let g = ((color >> 8) & 0xFF) as u8;
    let b = (color & 0xFF) as u8;
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

fn rgba_from_components(r: u8, g: u8, b: u8, a: u8) -> [f32; 4] {
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

trait UIRendererHandleExt {
    fn draw_rect(&self, rect: UIRect, color: [f32; 4], z_order: f32);
    fn draw_rect_outline(&self, rect: UIRect, thickness: f32, color: [f32; 4], z_order: f32);
    fn draw_text_simple(&self, text: &str, position: Vec2, font_size: f32, color: [f32; 4]);
    fn screen_size(&self) -> (u32, u32);
}

impl UIRendererHandleExt for Arc<std::sync::RwLock<UIRenderer>> {
    fn draw_rect(&self, rect: UIRect, color: [f32; 4], z_order: f32) {
        if let Ok(mut renderer) = self.write() {
            renderer.draw_rect(rect, color, z_order);
        }
    }

    fn draw_rect_outline(&self, rect: UIRect, thickness: f32, color: [f32; 4], z_order: f32) {
        if let Ok(mut renderer) = self.write() {
            renderer.draw_rect_outline(rect, thickness, color, z_order);
        }
    }

    fn draw_text_simple(&self, text: &str, position: Vec2, font_size: f32, color: [f32; 4]) {
        if let Ok(mut renderer) = self.write() {
            let _ = renderer.draw_text_simple(text, position, font_size, color);
        }
    }

    fn screen_size(&self) -> (u32, u32) {
        self.read()
            .map(|renderer| renderer.screen_size())
            .unwrap_or((0, 0))
    }
}

fn draw_window_image(window: &GameWindow, rect: UIRect, alpha: u8) -> bool {
    let Some(draw_data) = window.get_enabled_draw_data(0) else {
        return false;
    };
    let Some(image) = draw_data.image else {
        return false;
    };
    let mut drawn = false;
    let _ = with_ui_renderer(|renderer| {
        let mut renderer = renderer.write().unwrap_or_else(|e| e.into_inner());
        let collection = get_mapped_image_collection();
        let mut collection = collection.write();
        if let Some(mapped) = collection.find_image_by_name_mut(&image.name) {
            if mapped.get_gpu_texture().is_none() {
                let _ = mapped.create_gpu_texture(renderer.device(), renderer.queue());
            }
            if let Some(gpu) = mapped.get_gpu_texture() {
                let color = if draw_data.color != 0 {
                    rgba_from_color(draw_data.color, Some(alpha))
                } else {
                    rgba_from_components(255, 255, 255, alpha)
                };
                let uv = mapped.get_uv();
                let texture = Arc::new(gpu.view().clone());
                let tex_rect = UIRect::new(uv.min.x, uv.min.y, uv.width(), uv.height());
                renderer.draw_textured_rect(rect, texture, color, Some(tex_rect), 0.0);
                drawn = true;
            }
        }
    });
    drawn
}

fn play_sound(event_name: &str) {
    if let Some(audio) = TheAudio::get() {
        let event = AudioEventRts::new(event_name);
        audio.add_audio_event(&event);
    }
}

fn with_game_window_ref<R>(
    win_rc: &Rc<RefCell<GameWindow>>,
    f: impl FnOnce(&GameWindow) -> R,
) -> R {
    if let Ok(window) = win_rc.try_borrow() {
        f(&window)
    } else {
        let ptr = win_rc.as_ptr();
        // SAFETY: transition callbacks run on the legacy single-threaded UI path and need
        // the same re-entrant access fallback used by the rest of the window system.
        let window = unsafe { &*ptr };
        f(window)
    }
}

fn with_game_window_mut<R>(
    win_rc: &Rc<RefCell<GameWindow>>,
    f: impl FnOnce(&mut GameWindow) -> R,
) -> R {
    if let Ok(mut window) = win_rc.try_borrow_mut() {
        f(&mut window)
    } else {
        let ptr = win_rc.as_ptr();
        // SAFETY: transition callbacks run on the legacy single-threaded UI path and need
        // the same re-entrant access fallback used by the rest of the window system.
        let window = unsafe { &mut *ptr };
        f(window)
    }
}

trait Transition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    );
    fn update(&mut self, frame: i32);
    fn reverse(&mut self);
    fn draw(&mut self);
    fn skip(&mut self);
    fn is_finished(&self) -> bool;
    fn frame_length(&self) -> i32;
}

#[derive(Clone, Copy, Default)]
struct ICoord2D {
    x: i32,
    y: i32,
}

struct FlashTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    win: Option<Weak<RefCell<GameWindow>>>,
}

impl FlashTransition {
    fn new() -> Self {
        Self {
            frame_length: 7,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for FlashTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            let ((w, h), (x, y)) = with_game_window_ref(&win_rc, |win_ref| {
                (win_ref.get_size(), win_ref.get_screen_position())
            });
            self.size = ICoord2D { x: w, y: h };
            self.pos = ICoord2D { x, y };
        }
        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = -1;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.is_finished = true;
                }
            }
            1 => {
                if self.is_forward {
                    play_sound("GUIBoarderFadeIn");
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.draw_state = frame;
                }
            }
            2 | 3 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.draw_state = frame;
                }
            }
            4 | 5 | 6 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(false));
                    self.draw_state = frame;
                }
            }
            7 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(false));
                    self.is_finished = true;
                }
            }
            _ => {}
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        let rect = UIRect::new(
            (self.pos.x + 1) as f32,
            (self.pos.y + 1) as f32,
            (self.size.x - 2) as f32,
            self.size.y as f32,
        );
        let (outline, fill) = match self.draw_state {
            1 => (100, 33),
            2 => (150, 66),
            3 => (200, 99),
            4 => (250, 75),
            5 => (250, 50),
            6 => (250, 25),
            _ => return,
        };
        with_ui_renderer(|renderer| {
            renderer.draw_rect_outline(
                rect,
                1.0,
                rgba_from_components(255, 255, 255, outline),
                0.0,
            );
            renderer.draw_rect(rect, rgba_from_components(255, 255, 255, fill), 0.0);
        });
    }

    fn skip(&mut self) {
        self.update(7);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct ButtonFlashTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    win: Option<Weak<RefCell<GameWindow>>>,
}

impl ButtonFlashTransition {
    fn new() -> Self {
        Self {
            frame_length: 15,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }

    fn draw_button_background(&self, alpha: u8) {
        let Some(win_rc) = self.with_window() else {
            return;
        };
        let draw = with_game_window_ref(&win_rc, |win_ref| {
            win_ref
                .get_enabled_draw_data(0)
                .unwrap_or(WindowDrawData::default())
        });
        let rect = UIRect::new(
            self.pos.x as f32,
            self.pos.y as f32,
            self.size.x as f32,
            self.size.y as f32,
        );
        let color = rgba_from_color(draw.color, Some(alpha));
        with_ui_renderer(|renderer| {
            renderer.draw_rect(rect, color, 0.0);
        });
    }
}

impl Transition for ButtonFlashTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            let ((w, h), (x, y)) = with_game_window_ref(&win_rc, |win_ref| {
                (win_ref.get_size(), win_ref.get_screen_position())
            });
            self.size = ICoord2D { x: w, y: h };
            self.pos = ICoord2D { x, y };
        }
        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = -1;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.is_finished = true;
                }
            }
            1 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    if self.is_forward {
                        play_sound("GUIButtonsFadeIn");
                        self.draw_state = frame;
                    } else {
                        self.draw_state = 7;
                    }
                }
            }
            2 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.draw_state = if self.is_forward { frame } else { 6 };
                }
            }
            3 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.draw_state = if self.is_forward { frame } else { 5 };
                }
            }
            4 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.draw_state = 4;
                }
            }
            5 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.draw_state = if self.is_forward { frame } else { 3 };
                }
            }
            6 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.draw_state = if self.is_forward { frame } else { 2 };
                }
            }
            7 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                    self.draw_state = if self.is_forward { frame } else { 1 };
                }
            }
            11 => {
                if let Some(win_rc) = self.with_window() {
                    if self.is_forward {
                        let _ = with_game_window_mut(&win_rc, |window| window.hide(false));
                        self.draw_state = frame;
                    } else {
                        let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                        self.draw_state = 14;
                    }
                }
            }
            12 => {
                if let Some(win_rc) = self.with_window() {
                    if self.is_forward {
                        let _ = with_game_window_mut(&win_rc, |window| window.hide(false));
                        self.draw_state = frame;
                    } else {
                        let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                        self.draw_state = 13;
                    }
                }
            }
            13 => {
                if let Some(win_rc) = self.with_window() {
                    if self.is_forward {
                        let _ = with_game_window_mut(&win_rc, |window| window.hide(false));
                        self.draw_state = frame;
                    } else {
                        let _ = with_game_window_mut(&win_rc, |window| window.hide(true));
                        self.draw_state = 12;
                    }
                }
            }
            14 => {
                if let Some(win_rc) = self.with_window() {
                    if self.is_forward {
                        let _ = with_game_window_mut(&win_rc, |window| window.hide(false));
                        self.draw_state = frame;
                    } else {
                        let _ = with_game_window_mut(&win_rc, |window| window.hide(false));
                        self.draw_state = 11;
                    }
                }
            }
            15 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = with_game_window_mut(&win_rc, |window| window.hide(false));
                    self.is_finished = true;
                }
            }
            _ => {}
        }
        if frame > 7 && frame < 11 {
            self.draw_state = 16;
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        match self.draw_state {
            1 => {
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect_outline(
                        rect,
                        1.0,
                        rgba_from_components(255, 255, 255, 100),
                        0.0,
                    );
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 75), 0.0);
                });
            }
            2 => {
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect_outline(
                        rect,
                        1.0,
                        rgba_from_components(255, 255, 255, 150),
                        0.0,
                    );
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 150), 0.0);
                });
            }
            3 => {
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect_outline(
                        rect,
                        1.0,
                        rgba_from_components(255, 255, 255, 200),
                        0.0,
                    );
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 200), 0.0);
                });
            }
            4 => {
                self.draw_button_background(255);
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect_outline(
                        rect,
                        1.0,
                        rgba_from_components(255, 255, 255, 250),
                        0.0,
                    );
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 150), 0.0);
                });
            }
            5 => {
                self.draw_button_background(255);
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect_outline(
                        rect,
                        1.0,
                        rgba_from_components(255, 255, 255, 250),
                        0.0,
                    );
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 100), 0.0);
                });
            }
            6 => {
                self.draw_button_background(255);
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect_outline(
                        rect,
                        1.0,
                        rgba_from_components(255, 255, 255, 250),
                        0.0,
                    );
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 50), 0.0);
                });
            }
            7 => {
                self.draw_button_background(255);
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect_outline(
                        rect,
                        1.0,
                        rgba_from_components(255, 255, 255, 250),
                        0.0,
                    );
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 15), 0.0);
                });
            }
            11 => {
                if self.is_forward {
                    self.draw_button_background(255);
                }
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 100), 0.0);
                });
            }
            12 => {
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 200), 0.0);
                });
            }
            13 => {
                if !self.is_forward {
                    self.draw_button_background(255);
                }
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 150), 0.0);
                });
            }
            14 => {
                if !self.is_forward {
                    self.draw_button_background(255);
                }
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 100), 0.0);
                });
            }
            15 => {
                if !self.is_forward {
                    self.draw_button_background(255);
                }
                let rect = UIRect::new(
                    self.pos.x as f32,
                    self.pos.y as f32,
                    self.size.x as f32,
                    self.size.y as f32,
                );
                with_ui_renderer(|renderer| {
                    renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 17), 0.0);
                });
            }
            16 => {
                self.draw_button_background(255);
            }
            _ => {}
        }
    }

    fn skip(&mut self) {
        self.update(15);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct FadeTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    win: Option<Weak<RefCell<GameWindow>>>,
}

impl FadeTransition {
    fn new() -> Self {
        Self {
            frame_length: 9,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for FadeTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            let win_ref = win_rc.borrow();
            let (w, h) = win_ref.get_size();
            let (x, y) = win_ref.get_screen_position();
            self.size = ICoord2D { x: w, y: h };
            self.pos = ICoord2D { x, y };
        }
        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = -1;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                    self.is_finished = true;
                }
            }
            1..=8 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                    self.draw_state = frame;
                }
            }
            9 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(false);
                    self.is_finished = true;
                }
            }
            _ => {}
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        let alpha = match self.draw_state {
            1 => 25,
            2 => 50,
            3 => 75,
            4 => 100,
            5 => 125,
            6 => 150,
            7 => 175,
            8 => 200,
            9 => 225,
            _ => return,
        };
        let rect = UIRect::new(
            self.pos.x as f32,
            self.pos.y as f32,
            self.size.x as f32,
            self.size.y as f32,
        );
        if let Some(win_rc) = self.with_window() {
            let win_ref = win_rc.borrow();
            if draw_window_image(&win_ref, rect, alpha) {
                return;
            }
        }
        with_ui_renderer(|renderer| {
            renderer.draw_rect(rect, rgba_from_components(255, 255, 255, alpha), 0.0);
        });
    }

    fn skip(&mut self) {
        self.update(9);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct ScaleUpTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    center_pos: ICoord2D,
    increment_size: ICoord2D,
    win: Option<Weak<RefCell<GameWindow>>>,
}

impl ScaleUpTransition {
    fn new() -> Self {
        Self {
            frame_length: 6,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            center_pos: ICoord2D::default(),
            increment_size: ICoord2D::default(),
            win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for ScaleUpTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            let win_ref = win_rc.borrow();
            let (w, h) = win_ref.get_size();
            let (x, y) = win_ref.get_screen_position();
            self.size = ICoord2D { x: w, y: h };
            self.pos = ICoord2D { x, y };
        }
        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;

        self.center_pos = ICoord2D {
            x: self.pos.x + self.size.x / 2,
            y: self.pos.y + self.size.y / 2,
        };
        self.increment_size = ICoord2D {
            x: self.size.x / self.frame_length,
            y: self.size.y / self.frame_length,
        };
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = -1;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                    self.is_finished = true;
                }
            }
            1 => {
                if self.is_forward {
                    play_sound("GUILogoMouseOver");
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                self.draw_state = frame;
            }
            2 | 3 | 4 | 5 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                self.draw_state = frame;
            }
            6 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(false);
                    self.is_finished = true;
                }
            }
            _ => {}
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        if self.draw_state <= 0 || self.draw_state >= self.frame_length {
            return;
        }
        let x = self.center_pos.x - ((self.increment_size.x * self.draw_state) / 2);
        let y = self.center_pos.y - ((self.increment_size.y * self.draw_state) / 2);
        let x1 = x + self.increment_size.x * self.draw_state;
        let y1 = y + self.increment_size.y * self.draw_state;
        let rect = UIRect::new(x as f32, y as f32, (x1 - x) as f32, (y1 - y) as f32);
        if let Some(win_rc) = self.with_window() {
            let win_ref = win_rc.borrow();
            if draw_window_image(&win_ref, rect, 255) {
                return;
            }
        }
    }

    fn skip(&mut self) {
        self.update(self.frame_length);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct ScoreScaleUpTransition {
    inner: ScaleUpTransition,
}

impl ScoreScaleUpTransition {
    fn new() -> Self {
        Self {
            inner: ScaleUpTransition::new(),
        }
    }
}

impl Transition for ScoreScaleUpTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.inner.init(win, window_lookup);
    }

    fn update(&mut self, frame: i32) {
        self.inner.draw_state = -1;
        match frame {
            0 => {
                if self.inner.is_forward {
                    return;
                }
                if let Some(win_rc) = self.inner.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                    self.inner.is_finished = true;
                }
            }
            1 => {
                if self.inner.is_forward {
                    play_sound("GUIScoreScreenPictures");
                }
                if let Some(win_rc) = self.inner.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                self.inner.draw_state = frame;
            }
            2 | 3 | 4 | 5 => {
                if let Some(win_rc) = self.inner.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                self.inner.draw_state = frame;
            }
            6 => {
                if !self.inner.is_forward {
                    return;
                }
                if let Some(win_rc) = self.inner.with_window() {
                    let _ = win_rc.borrow_mut().hide(false);
                    self.inner.is_finished = true;
                }
            }
            _ => {}
        }
    }

    fn reverse(&mut self) {
        self.inner.reverse();
    }

    fn draw(&mut self) {
        self.inner.draw();
    }

    fn skip(&mut self) {
        self.inner.skip();
    }

    fn is_finished(&self) -> bool {
        self.inner.is_finished()
    }

    fn frame_length(&self) -> i32 {
        self.inner.frame_length()
    }
}

struct MainMenuScaleUpTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    grow_pos: ICoord2D,
    grow_size: ICoord2D,
    increment_pos: ICoord2D,
    increment_size: ICoord2D,
    win: Option<Weak<RefCell<GameWindow>>>,
    grow_win: Option<Weak<RefCell<GameWindow>>>,
}

impl MainMenuScaleUpTransition {
    fn new() -> Self {
        Self {
            frame_length: 5,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            grow_pos: ICoord2D::default(),
            grow_size: ICoord2D::default(),
            increment_pos: ICoord2D::default(),
            increment_size: ICoord2D::default(),
            win: None,
            grow_win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }

    fn with_grow_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.grow_win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for MainMenuScaleUpTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            let win_ref = win_rc.borrow();
            let (w, h) = win_ref.get_size();
            let (x, y) = win_ref.get_screen_position();
            self.size = ICoord2D { x: w, y: h };
            self.pos = ICoord2D { x, y };
        }
        self.grow_win = lookup_window(window_lookup, "MainMenu.wnd:WinGrowMarker")
            .map(|window| Rc::downgrade(&window));
        let Some(grow_rc) = self.with_grow_window() else {
            self.is_finished = true;
            return;
        };
        {
            let grow_ref = grow_rc.borrow();
            let (gw, gh) = grow_ref.get_size();
            let (gx, gy) = grow_ref.get_screen_position();
            self.grow_size = ICoord2D { x: gw, y: gh };
            self.grow_pos = ICoord2D { x: gx, y: gy };
        }
        if let (Some(win_rc), Some(grow_rc)) = (self.with_window(), self.with_grow_window()) {
            let image = {
                let win_ref = win_rc.borrow();
                win_ref
                    .get_disabled_draw_data(0)
                    .and_then(|data| data.image)
            };
            if let Some(image) = image {
                let _ = grow_rc.borrow_mut().set_enabled_image(0, image);
            }
        }

        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;

        self.increment_pos = ICoord2D {
            x: (self.grow_pos.x - self.pos.x) / self.frame_length,
            y: (self.grow_pos.y - self.pos.y) / self.frame_length,
        };
        self.increment_size = ICoord2D {
            x: (self.grow_size.x - self.size.x) / self.frame_length,
            y: (self.grow_size.y - self.size.y) / self.frame_length,
        };
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = -1;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(grow_rc) = self.with_grow_window() {
                    let _ = grow_rc.borrow_mut().hide(true);
                    self.is_finished = true;
                }
            }
            5 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                if let Some(grow_rc) = self.with_grow_window() {
                    let _ = grow_rc.borrow_mut().hide(false);
                }
                self.is_finished = true;
            }
            1..=4 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                if let Some(grow_rc) = self.with_grow_window() {
                    let _ = grow_rc.borrow_mut().hide(true);
                }
                self.draw_state = frame;
            }
            _ => {}
        }
        if frame == 1 {
            play_sound("GUILogoSelect");
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        if self.draw_state <= 0 || self.draw_state >= self.frame_length {
            return;
        }
        let x = self.pos.x + self.increment_pos.x * self.draw_state;
        let y = self.pos.y + self.increment_pos.y * self.draw_state;
        let x1 = x + self.size.x + (self.increment_size.x * self.draw_state);
        let y1 = y + self.size.y + (self.increment_size.y * self.draw_state);
        let rect = UIRect::new(x as f32, y as f32, (x1 - x) as f32, (y1 - y) as f32);
        if let Some(grow_rc) = self.with_grow_window() {
            let grow_ref = grow_rc.borrow();
            if draw_window_image(&grow_ref, rect, 255) {
                return;
            }
        }
    }

    fn skip(&mut self) {
        self.update(self.frame_length);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct MainMenuMediumScaleUpTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    increment_size: ICoord2D,
    win: Option<Weak<RefCell<GameWindow>>>,
    grow_win: Option<Weak<RefCell<GameWindow>>>,
}

impl MainMenuMediumScaleUpTransition {
    fn new() -> Self {
        Self {
            frame_length: 3,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            increment_size: ICoord2D::default(),
            win: None,
            grow_win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }

    fn with_grow_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.grow_win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for MainMenuMediumScaleUpTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            let win_ref = win_rc.borrow();
            let (w, h) = win_ref.get_size();
            let (x, y) = win_ref.get_screen_position();
            self.size = ICoord2D { x: w, y: h };
            self.pos = ICoord2D { x, y };
            let mut grow_name = win_ref.instance_data().decorated_name.clone();
            grow_name.push_str("Medium");
            self.grow_win =
                lookup_window(window_lookup, &grow_name).map(|window| Rc::downgrade(&window));
        }
        let Some(grow_rc) = self.with_grow_window() else {
            self.is_finished = true;
            return;
        };
        {
            let grow_ref = grow_rc.borrow();
            let (gw, gh) = grow_ref.get_size();
            self.increment_size = ICoord2D {
                x: (gw - self.size.x) / self.frame_length,
                y: (gh - self.size.y) / self.frame_length,
            };
        }
        if let (Some(win_rc), Some(grow_rc)) = (self.with_window(), self.with_grow_window()) {
            let image = {
                let win_ref = win_rc.borrow();
                win_ref.get_enabled_draw_data(0).and_then(|data| data.image)
            };
            if let Some(image) = image {
                let _ = grow_rc.borrow_mut().set_enabled_image(0, image);
            }
        }

        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = -1;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(false);
                }
                if let Some(grow_rc) = self.with_grow_window() {
                    let _ = grow_rc.borrow_mut().hide(true);
                }
                self.is_finished = true;
            }
            3 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                if let Some(grow_rc) = self.with_grow_window() {
                    let _ = grow_rc.borrow_mut().hide(false);
                }
                self.is_finished = true;
            }
            1 | 2 => {
                if frame == 1 && self.is_forward {
                    play_sound("GUILogoMouseOver");
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                if let Some(grow_rc) = self.with_grow_window() {
                    let _ = grow_rc.borrow_mut().hide(true);
                }
                self.draw_state = frame;
            }
            _ => {}
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
        if let Some(win_rc) = self.with_window() {
            let _ = win_rc.borrow_mut().hide(true);
        }
        if let Some(grow_rc) = self.with_grow_window() {
            let _ = grow_rc.borrow_mut().hide(true);
        }
    }

    fn draw(&mut self) {
        if self.draw_state <= 0 || self.draw_state >= self.frame_length {
            return;
        }
        let x = self.pos.x - ((self.increment_size.x * self.draw_state) / 2);
        let y = self.pos.y - ((self.increment_size.y * self.draw_state) / 2);
        let x1 = self.pos.x + self.size.x + ((self.increment_size.x * self.draw_state) / 2);
        let y1 = self.pos.y + self.size.y + ((self.increment_size.y * self.draw_state) / 2);
        let rect = UIRect::new(x as f32, y as f32, (x1 - x) as f32, (y1 - y) as f32);
        if let Some(win_rc) = self.with_window() {
            let win_ref = win_rc.borrow();
            if draw_window_image(&win_ref, rect, 255) {
                return;
            }
        }
    }

    fn skip(&mut self) {
        self.update(self.frame_length);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct MainMenuSmallScaleDownTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    increment_size: ICoord2D,
    win: Option<Weak<RefCell<GameWindow>>>,
    grow_win: Option<Weak<RefCell<GameWindow>>>,
}

impl MainMenuSmallScaleDownTransition {
    fn new() -> Self {
        Self {
            frame_length: 5,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            increment_size: ICoord2D::default(),
            win: None,
            grow_win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }

    fn with_grow_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.grow_win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for MainMenuSmallScaleDownTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            let win_ref = win_rc.borrow();
            let (w, h) = win_ref.get_size();
            let (x, y) = win_ref.get_screen_position();
            self.size = ICoord2D { x: w, y: h };
            self.pos = ICoord2D { x, y };
            let mut grow_name = win_ref.instance_data().decorated_name.clone();
            grow_name.push_str("Small");
            self.grow_win =
                lookup_window(window_lookup, &grow_name).map(|window| Rc::downgrade(&window));
        }
        let Some(grow_rc) = self.with_grow_window() else {
            self.is_finished = true;
            return;
        };
        let grow_ref = grow_rc.borrow();
        let (gw, gh) = grow_ref.get_size();
        self.increment_size = ICoord2D {
            x: (gw - self.size.x) / self.frame_length,
            y: (gh - self.size.y) / self.frame_length,
        };

        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = -1;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(false);
                }
                if let Some(grow_rc) = self.with_grow_window() {
                    let _ = grow_rc.borrow_mut().hide(true);
                }
                self.is_finished = true;
            }
            5 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                if let Some(grow_rc) = self.with_grow_window() {
                    let _ = grow_rc.borrow_mut().hide(false);
                }
                self.is_finished = true;
            }
            1..=4 => {
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                }
                if let Some(grow_rc) = self.with_grow_window() {
                    let _ = grow_rc.borrow_mut().hide(true);
                }
                self.draw_state = frame;
            }
            _ => {}
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        if self.draw_state <= 0 || self.draw_state >= self.frame_length {
            return;
        }
        let x = self.pos.x - ((self.increment_size.x * self.draw_state) / 2);
        let y = self.pos.y - ((self.increment_size.y * self.draw_state) / 2);
        let x1 = self.pos.x + self.size.x + ((self.increment_size.x * self.draw_state) / 2);
        let y1 = self.pos.y + self.size.y + ((self.increment_size.y * self.draw_state) / 2);
        let rect = UIRect::new(x as f32, y as f32, (x1 - x) as f32, (y1 - y) as f32);
        if let Some(win_rc) = self.with_window() {
            let win_ref = win_rc.borrow();
            if draw_window_image(&win_ref, rect, 255) {
                return;
            }
        }
    }

    fn skip(&mut self) {
        self.update(self.frame_length);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct TextTypeTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    full_text: String,
    partial_text: String,
    win: Option<Weak<RefCell<GameWindow>>>,
}

impl TextTypeTransition {
    fn new() -> Self {
        Self {
            frame_length: 30,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            full_text: String::new(),
            partial_text: String::new(),
            win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for TextTypeTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            let win_ref = win_rc.borrow();
            let (w, h) = win_ref.get_size();
            let (x, y) = win_ref.get_screen_position();
            self.size = ICoord2D { x: w, y: h };
            self.pos = ICoord2D { x, y };
            self.full_text = resolve_window_text(win_ref.get_text());
        }
        let length = self.full_text.chars().count() as i32;
        if length > 0 {
            self.frame_length = length.min(30);
        }
        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;
        self.partial_text.clear();
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = -1;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                    self.is_finished = true;
                }
            }
            30 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(false);
                    self.is_finished = true;
                }
            }
            _ => {}
        }
        if frame >= self.frame_length {
            if let Some(win_rc) = self.with_window() {
                let _ = win_rc.borrow_mut().hide(false);
            }
        }
        if frame > 0 && frame < self.frame_length {
            if let Some(win_rc) = self.with_window() {
                let _ = win_rc.borrow_mut().hide(true);
            }
            self.draw_state = frame;
            play_sound("GUITypeText");
            if self.is_forward {
                let idx = frame as usize - 1;
                if let Some(ch) = self.full_text.chars().nth(idx) {
                    self.partial_text.push(ch);
                }
            } else {
                self.partial_text.pop();
            }
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
        self.partial_text = self.full_text.clone();
    }

    fn draw(&mut self) {
        if self.draw_state <= 0 || self.draw_state >= self.frame_length {
            return;
        }
        let Some(win_rc) = self.with_window() else {
            return;
        };
        let win_ref = win_rc.borrow();
        let color = rgba_from_color(win_ref.get_enabled_text_color(), None);
        let font_size = win_ref.get_font().map(|f| f.size).unwrap_or(14) as f32;
        let text = self.partial_text.clone();
        let (x, y) = if win_ref.get_status().contains(WindowStatus::WRAP_CENTERED) {
            let width_estimate = font_size * text.chars().count() as f32 * 0.6;
            (
                self.pos.x as f32 + (self.size.x as f32 / 2.0) - (width_estimate / 2.0),
                self.pos.y as f32 + (self.size.y as f32 / 2.0) - (font_size / 2.0),
            )
        } else {
            (
                self.pos.x as f32 + 7.0,
                self.pos.y as f32 + (self.size.y as f32 / 2.0) - (font_size / 2.0),
            )
        };
        with_ui_renderer(|renderer| {
            let _ = renderer.draw_text_simple(&text, Vec2::new(x, y), font_size, color);
        });
    }

    fn skip(&mut self) {
        self.update(30);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct CountUpTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    full_text: String,
    int_value: i32,
    current_value: i32,
    count_state: i32,
    win: Option<Weak<RefCell<GameWindow>>>,
}

impl CountUpTransition {
    fn new() -> Self {
        Self {
            frame_length: 30,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            full_text: String::new(),
            int_value: 0,
            current_value: 0,
            count_state: 1,
            win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for CountUpTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            if win_rc.borrow().is_hidden() {
                self.is_forward = true;
                self.is_finished = true;
                self.frame_length = 0;
                return;
            }
            self.full_text = resolve_window_text(win_rc.borrow().get_text());
        }
        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;

        self.int_value = self.full_text.trim().parse::<i32>().unwrap_or(0);
        if self.int_value < 30 {
            self.count_state = 1;
            self.frame_length = self.int_value.min(30);
        } else if self.int_value / 100 < 30 {
            self.count_state = 100;
            self.frame_length = (self.int_value / 100).min(30);
        } else {
            self.count_state = 1000;
            self.frame_length = (self.int_value / 1000).min(30);
        }
        self.current_value = 0;
        if let Some(win_rc) = self.with_window() {
            let _ = win_rc
                .borrow_mut()
                .set_text(&self.current_value.to_string());
        }
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = -1;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                self.current_value = 0;
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().set_text("0");
                    let _ = win_rc.borrow_mut().hide(true);
                }
                self.is_finished = true;
            }
            30 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(false);
                }
                self.is_finished = true;
            }
            _ => {}
        }
        if frame >= self.frame_length {
            if let Some(win_rc) = self.with_window() {
                let _ = win_rc.borrow_mut().hide(false);
            }
        }
        if frame > 0 && frame < self.frame_length {
            if let Some(win_rc) = self.with_window() {
                let _ = win_rc.borrow_mut().hide(false);
            }
            self.draw_state = frame;
            play_sound("GUIScoreScreenTick");
            self.current_value += self.count_state;
            if self.current_value > self.int_value {
                self.current_value = self.int_value;
            }
            if let Some(win_rc) = self.with_window() {
                let _ = win_rc
                    .borrow_mut()
                    .set_text(&self.current_value.to_string());
            }
        }
        if frame == self.frame_length {
            if let Some(win_rc) = self.with_window() {
                let _ = win_rc.borrow_mut().set_text(&self.full_text);
            }
            self.is_finished = true;
        }
    }

    fn reverse(&mut self) {
        if let Some(win_rc) = self.with_window() {
            if win_rc.borrow().is_hidden() {
                self.is_forward = false;
                self.is_finished = true;
                self.frame_length = 0;
                return;
            }
        }
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        // C++ parity: CountUpTransition has no custom draw logic.
        // The count is rendered by the window text (set via set_text() in update()).
    }

    fn skip(&mut self) {
        if !self.is_finished {
            self.update(30);
        }
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct ScreenFadeTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    percent: f32,
}

impl ScreenFadeTransition {
    fn new() -> Self {
        Self {
            frame_length: 30,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            percent: 0.0,
        }
    }
}

impl Transition for ScreenFadeTransition {
    fn init(
        &mut self,
        _win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;

        self.percent = 1.0 / (self.frame_length as f32 - 1.0);
        self.pos = ICoord2D { x: 0, y: 0 };
        with_ui_renderer(|renderer| {
            let (w, h) = renderer.screen_size();
            self.size = ICoord2D {
                x: w as i32,
                y: h as i32,
            };
        });
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = frame;
        self.is_finished = true;
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        if self.draw_state < 0 {
            return;
        }
        let mut alpha = (self.percent * 255.0 * self.draw_state as f32) as i32;
        if alpha > 255 {
            alpha = 255;
        }
        let rect = UIRect::new(
            self.pos.x as f32,
            self.pos.y as f32,
            self.size.x as f32,
            self.size.y as f32,
        );
        with_ui_renderer(|renderer| {
            renderer.draw_rect(rect, rgba_from_components(0, 0, 0, alpha as u8), 0.0);
        });
    }

    fn skip(&mut self) {
        self.update(self.frame_length);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct ControlBarArrowTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    increment_pos: ICoord2D,
    percent: f32,
    fade_percent: f32,
}

impl ControlBarArrowTransition {
    fn new() -> Self {
        Self {
            frame_length: 22,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            increment_pos: ICoord2D::default(),
            percent: 0.0,
            fade_percent: 0.0,
        }
    }
}

impl Transition for ControlBarArrowTransition {
    fn init(
        &mut self,
        _win: Option<Rc<RefCell<GameWindow>>>,
        window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;

        self.percent = 1.0 / 16.0;
        self.fade_percent = 1.0 / (self.frame_length as f32 - 16.0);

        let Some(button) = lookup_window(window_lookup, "ControlBar.wnd:ButtonGeneral") else {
            self.is_finished = true;
            return;
        };
        let (x, y) = button.borrow().get_screen_position();
        let (w, h) = button.borrow().get_size();
        self.increment_pos = ICoord2D {
            x: 0,
            y: (y as f32 * self.percent) as i32,
        };
        self.pos = ICoord2D {
            x: x + w / 2 - 8,
            y: 0 - 16 + 20,
        };
        self.size = ICoord2D { x: 16, y: 16 };
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = frame;
        self.is_finished = true;
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        if self.draw_state < 0 {
            return;
        }
        if self.draw_state < 16 {
            let y_pos = self.pos.y + self.increment_pos.y * self.draw_state;
            let rect = UIRect::new(
                self.pos.x as f32,
                y_pos as f32,
                self.size.x as f32,
                self.size.y as f32,
            );
            with_ui_renderer(|renderer| {
                renderer.draw_rect(rect, rgba_from_components(255, 255, 255, 255), 0.0);
            });
        } else {
            let mut alpha = (1.0 - (self.fade_percent * (self.draw_state - 16) as f32)) * 255.0;
            if alpha > 255.0 {
                alpha = 255.0;
            }
            let y_pos = self.pos.y + self.increment_pos.y * (16 - 1);
            let rect = UIRect::new(
                self.pos.x as f32,
                y_pos as f32,
                self.size.x as f32,
                self.size.y as f32,
            );
            with_ui_renderer(|renderer| {
                renderer.draw_rect(rect, rgba_from_components(255, 255, 255, alpha as u8), 0.0);
            });
        }
    }

    fn skip(&mut self) {
        self.update(self.frame_length);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct FullFadeTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    draw_state: i32,
    pos: ICoord2D,
    size: ICoord2D,
    percent: f32,
    win: Option<Weak<RefCell<GameWindow>>>,
}

impl FullFadeTransition {
    fn new() -> Self {
        Self {
            frame_length: 10,
            is_finished: false,
            is_forward: true,
            draw_state: -1,
            pos: ICoord2D::default(),
            size: ICoord2D::default(),
            percent: 0.0,
            win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for FullFadeTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = win {
            let win_ref = win_rc.borrow();
            let (w, h) = win_ref.get_size();
            let (x, y) = win_ref.get_screen_position();
            self.size = ICoord2D { x: w, y: h };
            self.pos = ICoord2D { x, y };
        }

        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;

        self.percent = 1.0 / (self.frame_length as f32 / 2.0);
    }

    fn update(&mut self, frame: i32) {
        self.draw_state = frame;
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                    self.is_finished = true;
                }
            }
            10 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(false);
                    self.is_finished = true;
                }
            }
            _ => {}
        }
        if frame == self.frame_length / 2 {
            if let Some(win_rc) = self.with_window() {
                let _ = if self.is_forward {
                    win_rc.borrow_mut().hide(false)
                } else {
                    win_rc.borrow_mut().hide(true)
                };
            }
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        let half = self.frame_length / 2;
        let mut alpha = if self.draw_state > half {
            self.percent * 255.0 * (self.frame_length - self.draw_state) as f32
        } else {
            self.percent * 255.0 * self.draw_state as f32
        };
        if alpha > 255.0 {
            alpha = 255.0;
        }
        let rect = UIRect::new(
            self.pos.x as f32,
            self.pos.y as f32,
            self.size.x as f32,
            self.size.y as f32,
        );
        with_ui_renderer(|renderer| {
            renderer.draw_rect(rect, rgba_from_components(0, 0, 0, alpha as u8), 0.0);
            renderer.draw_rect_outline(
                rect,
                1.0,
                rgba_from_components(60, 60, 180, alpha as u8),
                0.0,
            );
        });
    }

    fn skip(&mut self) {
        self.update(self.frame_length);
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct TextOnFrameTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
    win: Option<Weak<RefCell<GameWindow>>>,
}

impl TextOnFrameTransition {
    fn new() -> Self {
        Self {
            frame_length: 1,
            is_finished: false,
            is_forward: true,
            win: None,
        }
    }

    fn with_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.win.as_ref().and_then(|w| w.upgrade())
    }
}

impl Transition for TextOnFrameTransition {
    fn init(
        &mut self,
        win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.win = win.as_ref().map(Rc::downgrade);
        if let Some(win_rc) = self.with_window() {
            if win_rc.borrow().is_hidden() {
                self.is_finished = true;
                self.is_forward = true;
                self.frame_length = 0;
                return;
            }
        }
        self.is_forward = false;
        self.update(0);
        self.is_finished = false;
        self.is_forward = true;
    }

    fn update(&mut self, frame: i32) {
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(true);
                    self.is_finished = true;
                }
            }
            1 => {
                if !self.is_forward {
                    return;
                }
                if let Some(win_rc) = self.with_window() {
                    let _ = win_rc.borrow_mut().hide(false);
                    self.is_finished = true;
                }
            }
            _ => {}
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
        if let Some(win_rc) = self.with_window() {
            if win_rc.borrow().is_hidden() {
                self.is_finished = true;
                self.frame_length = 0;
            }
        }
    }

    fn draw(&mut self) {
        // C++ parity: TextOnFrameTransition has no custom draw logic.
        // The window is shown/hidden in update(); the window system renders content.
    }

    fn skip(&mut self) {
        if !self.is_finished {
            self.update(1);
        }
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

struct ReverseSoundTransition {
    frame_length: i32,
    is_finished: bool,
    is_forward: bool,
}

impl ReverseSoundTransition {
    fn new() -> Self {
        Self {
            frame_length: 2,
            is_finished: false,
            is_forward: true,
        }
    }
}

impl Transition for ReverseSoundTransition {
    fn init(
        &mut self,
        _win: Option<Rc<RefCell<GameWindow>>>,
        _window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        self.is_finished = true;
        self.is_forward = true;
    }

    fn update(&mut self, frame: i32) {
        match frame {
            0 => {
                if self.is_forward {
                    return;
                }
                self.is_finished = true;
            }
            1 => {
                play_sound("GUITransitionFade");
            }
            2 => {
                if !self.is_forward {
                    return;
                }
                self.is_finished = true;
            }
            _ => {}
        }
    }

    fn reverse(&mut self) {
        self.is_finished = false;
        self.is_forward = false;
    }

    fn draw(&mut self) {
        // C++ parity: ReverseSoundTransition has no custom draw logic.
        // Only plays a sound event in update(); nothing to render.
    }

    fn skip(&mut self) {
        if !self.is_finished {
            self.update(2);
        }
    }

    fn is_finished(&self) -> bool {
        self.is_finished
    }

    fn frame_length(&self) -> i32 {
        self.frame_length
    }
}

fn get_transition_for_style(style: i32) -> Option<Box<dyn Transition>> {
    match style {
        TRANSITION_FLASH => Some(Box::new(FlashTransition::new())),
        BUTTON_TRANSITION_FLASH => Some(Box::new(ButtonFlashTransition::new())),
        WIN_FADE_TRANSITION => Some(Box::new(FadeTransition::new())),
        WIN_SCALE_UP_TRANSITION => Some(Box::new(ScaleUpTransition::new())),
        MAINMENU_SCALE_UP_TRANSITION => Some(Box::new(MainMenuScaleUpTransition::new())),
        TEXT_TYPE_TRANSITION => Some(Box::new(TextTypeTransition::new())),
        SCREEN_FADE_TRANSITION => Some(Box::new(ScreenFadeTransition::new())),
        COUNT_UP_TRANSITION => Some(Box::new(CountUpTransition::new())),
        FULL_FADE_TRANSITION => Some(Box::new(FullFadeTransition::new())),
        TEXT_ON_FRAME_TRANSITION => Some(Box::new(TextOnFrameTransition::new())),
        MAINMENU_MEDIUM_SCALE_UP_TRANSITION => {
            Some(Box::new(MainMenuMediumScaleUpTransition::new()))
        }
        MAINMENU_SMALL_SCALE_DOWN_TRANSITION => {
            Some(Box::new(MainMenuSmallScaleDownTransition::new()))
        }
        CONTROL_BAR_ARROW_TRANSITION => Some(Box::new(ControlBarArrowTransition::new())),
        SCORE_SCALE_UP_TRANSITION => Some(Box::new(ScoreScaleUpTransition::new())),
        REVERSE_SOUND_TRANSITION => Some(Box::new(ReverseSoundTransition::new())),
        _ => None,
    }
}

pub struct TransitionWindow {
    pub win_name: String,
    pub frame_delay: i32,
    pub style: i32,
    win_id: i32,
    win: Option<Weak<RefCell<GameWindow>>>,
    transition: Option<Box<dyn Transition>>,
    current_frame_delay: i32,
}

impl TransitionWindow {
    fn new() -> Self {
        Self {
            win_name: String::new(),
            frame_delay: 0,
            style: 0,
            win_id: -1,
            win: None,
            transition: None,
            current_frame_delay: 0,
        }
    }

    fn init(&mut self, window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>) {
        self.win_id = NameKeyGenerator::name_to_key(&self.win_name) as i32;
        self.win = window_lookup.get(&self.win_id).cloned();
        self.current_frame_delay = self.frame_delay;
        self.transition = get_transition_for_style(self.style);
        if let Some(transition) = &mut self.transition {
            let win_rc = self.win.as_ref().and_then(|w| w.upgrade());
            transition.init(win_rc, window_lookup);
        }
    }

    fn update(&mut self, frame: i32) {
        let Some(transition) = &mut self.transition else {
            return;
        };
        if frame < self.current_frame_delay
            || frame > (self.current_frame_delay + transition.frame_length())
        {
            return;
        }
        transition.update(frame - self.current_frame_delay);
    }

    fn is_finished(&self) -> bool {
        self.transition
            .as_ref()
            .map(|t| t.is_finished())
            .unwrap_or(true)
    }

    fn reverse(&mut self) {
        if let Some(transition) = &mut self.transition {
            transition.reverse();
        }
    }

    fn skip(&mut self) {
        if let Some(transition) = &mut self.transition {
            transition.skip();
        }
    }

    fn draw(&mut self) {
        if let Some(transition) = &mut self.transition {
            transition.draw();
        }
    }

    fn get_total_frames(&self) -> i32 {
        if let Some(transition) = &self.transition {
            self.frame_delay + transition.frame_length()
        } else {
            self.frame_delay
        }
    }
}

pub struct TransitionGroup {
    name: String,
    fire_once: bool,
    transition_windows: Vec<TransitionWindow>,
    direction_multiplier: i32,
    current_frame: i32,
}

impl TransitionGroup {
    fn new() -> Self {
        Self {
            name: String::new(),
            fire_once: false,
            transition_windows: Vec::new(),
            direction_multiplier: 1,
            current_frame: 0,
        }
    }

    fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn init(&mut self, window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>) {
        self.current_frame = 0;
        self.direction_multiplier = 1;
        for win in &mut self.transition_windows {
            win.init(window_lookup);
        }
    }

    fn update(&mut self) {
        self.current_frame += self.direction_multiplier;
        for win in &mut self.transition_windows {
            win.update(self.current_frame);
        }
    }

    fn is_finished(&self) -> bool {
        self.transition_windows.iter().all(|w| w.is_finished())
    }

    fn reverse(&mut self) {
        let mut total_frames = 0;
        self.direction_multiplier = -1;
        for win in &self.transition_windows {
            let frames = win.get_total_frames();
            if frames > total_frames {
                total_frames = frames;
            }
        }
        for win in &mut self.transition_windows {
            win.reverse();
        }
        self.current_frame = total_frames;
    }

    fn is_reversed(&self) -> bool {
        self.direction_multiplier < 0
    }

    fn skip(&mut self) {
        for win in &mut self.transition_windows {
            win.skip();
        }
    }

    fn draw(&mut self) {
        for win in &mut self.transition_windows {
            win.draw();
        }
    }

    fn add_window(&mut self, win: TransitionWindow) {
        self.transition_windows.push(win);
    }
}

pub struct GameWindowTransitionsHandler {
    groups: Vec<TransitionGroup>,
    current_group: Option<usize>,
    pending_group: Option<usize>,
    draw_group: Option<usize>,
    secondary_draw_group: Option<usize>,
}

impl GameWindowTransitionsHandler {
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            current_group: None,
            pending_group: None,
            draw_group: None,
            secondary_draw_group: None,
        }
    }

    pub fn init(&mut self) {
        self.current_group = None;
        self.pending_group = None;
        self.draw_group = None;
        self.secondary_draw_group = None;
    }

    pub fn load(&mut self, path: &str) {
        if let Ok(contents) = std::fs::read_to_string(path) {
            self.parse_window_transitions(&contents);
        }
    }

    pub fn reset(&mut self) {
        self.current_group = None;
        self.pending_group = None;
        self.draw_group = None;
        self.secondary_draw_group = None;
    }

    pub fn update(&mut self) {
        if self.draw_group != self.current_group {
            self.secondary_draw_group = self.draw_group;
        } else {
            self.secondary_draw_group = None;
        }
        self.draw_group = self.current_group;

        if let Some(idx) = self.current_group {
            let group = &mut self.groups[idx];
            if !group.is_finished() {
                group.update();
            }
        }

        if let Some(idx) = self.current_group {
            if self.groups[idx].is_finished() && self.groups[idx].fire_once {
                self.current_group = None;
            }
        }

        if let (Some(current), Some(pending)) = (self.current_group, self.pending_group) {
            if self.groups[current].is_finished() {
                self.current_group = Some(pending);
                self.pending_group = None;
            }
        }

        if self.current_group.is_none() {
            if let Some(pending) = self.pending_group {
                self.current_group = Some(pending);
                self.pending_group = None;
            }
        }

        if let Some(idx) = self.current_group {
            if self.groups[idx].is_finished() && self.groups[idx].is_reversed() {
                self.current_group = None;
            }
        }
    }

    pub fn draw(&mut self) {
        if let Some(idx) = self.draw_group {
            self.groups[idx].draw();
        }
        if let Some(idx) = self.secondary_draw_group {
            self.groups[idx].draw();
        }
    }

    pub fn set_group(
        &mut self,
        group_name: &str,
        immediate: bool,
        window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        if group_name.is_empty() && immediate {
            self.current_group = None;
        }
        if immediate {
            if let Some(current) = self.current_group {
                self.groups[current].skip();
            }
            self.current_group = self.find_group_index(group_name);
            if let Some(idx) = self.current_group {
                self.groups[idx].init(window_lookup);
            }
            return;
        }

        if let Some(current) = self.current_group {
            if !self.groups[current].fire_once && !self.groups[current].is_reversed() {
                self.groups[current].reverse();
            }
            self.pending_group = self.find_group_index(group_name);
            if let Some(idx) = self.pending_group {
                self.groups[idx].init(window_lookup);
            }
            return;
        }

        self.current_group = self.find_group_index(group_name);
        if let Some(idx) = self.current_group {
            self.groups[idx].init(window_lookup);
        }
    }

    pub fn reverse(
        &mut self,
        group_name: &str,
        window_lookup: &HashMap<i32, Weak<RefCell<GameWindow>>>,
    ) {
        let group = self.find_group_index(group_name);
        if let Some(group_idx) = group {
            if self.current_group == Some(group_idx) {
                self.groups[group_idx].reverse();
                return;
            }
            if self.pending_group == Some(group_idx) {
                self.pending_group = None;
                return;
            }
            if let Some(current) = self.current_group {
                self.groups[current].skip();
            }
            if let Some(pending) = self.pending_group {
                self.groups[pending].skip();
            }
            self.current_group = Some(group_idx);
            self.groups[group_idx].init(window_lookup);
            self.groups[group_idx].skip();
            self.groups[group_idx].reverse();
            self.pending_group = None;
        }
    }

    pub fn remove(&mut self, group_name: &str, skip_pending: bool) {
        let group = self.find_group_index(group_name);
        if let Some(idx) = group {
            if self.pending_group == Some(idx) {
                if skip_pending {
                    self.groups[idx].skip();
                }
                self.pending_group = None;
            }
            if self.current_group == Some(idx) {
                self.groups[idx].skip();
                self.current_group = None;
                if let Some(pending) = self.pending_group {
                    self.current_group = Some(pending);
                }
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        self.current_group
            .map(|idx| self.groups[idx].is_finished())
            .unwrap_or(true)
    }

    fn find_group_index(&self, group_name: &str) -> Option<usize> {
        let target = group_name.to_ascii_lowercase();
        self.groups
            .iter()
            .enumerate()
            .find(|(_, g)| g.get_name().eq_ignore_ascii_case(&target))
            .map(|(idx, _)| idx)
    }

    fn parse_window_transitions(&mut self, contents: &str) {
        let mut current_group: Option<TransitionGroup> = None;
        let mut current_window: Option<TransitionWindow> = None;

        for raw_line in contents.lines() {
            let mut line = raw_line.split(';').next().unwrap_or("").trim().to_string();
            if line.is_empty() {
                continue;
            }
            if line.starts_with("WindowTransition") {
                if let Some(group) = current_group.take() {
                    self.groups.push(group);
                }
                let name = line["WindowTransition".len()..].trim();
                let mut group = TransitionGroup::new();
                group.set_name(name);
                current_group = Some(group);
                continue;
            }
            if line.eq_ignore_ascii_case("Window") {
                current_window = Some(TransitionWindow::new());
                continue;
            }
            if line.eq_ignore_ascii_case("END") {
                if let Some(win) = current_window.take() {
                    if let Some(group) = current_group.as_mut() {
                        group.add_window(win);
                    }
                } else if let Some(group) = current_group.take() {
                    self.groups.push(group);
                }
                continue;
            }
            if let Some(window) = current_window.as_mut() {
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    match key.to_ascii_lowercase().as_str() {
                        "winname" => window.win_name = value.to_string(),
                        "style" => {
                            if let Some(style) = transition_style_from_name(value) {
                                window.style = style;
                            }
                        }
                        "framedelay" => window.frame_delay = value.parse::<i32>().unwrap_or(0),
                        _ => {}
                    }
                }
                continue;
            }
            if let Some(group) = current_group.as_mut() {
                if let Some((key, value)) = line.split_once('=') {
                    if key.trim().eq_ignore_ascii_case("FireOnce") {
                        group.fire_once = parse_bool_token(value);
                    }
                }
            }
        }
        if let Some(win) = current_window {
            if let Some(group) = current_group.as_mut() {
                group.add_window(win);
            }
        }
        if let Some(group) = current_group {
            self.groups.push(group);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_text::GameText;

    #[test]
    fn fade_transition_finishes_on_last_forward_frame() {
        let win = Rc::new(RefCell::new(GameWindow::new()));
        let mut transition = FadeTransition::new();
        transition.init(Some(win.clone()), &HashMap::new());

        transition.update(9);

        assert!(transition.is_finished());
    }

    #[test]
    fn fade_transition_is_not_finished_before_last_forward_frame() {
        let win = Rc::new(RefCell::new(GameWindow::new()));
        let mut transition = FadeTransition::new();
        transition.init(Some(win.clone()), &HashMap::new());

        transition.update(8);

        assert!(!transition.is_finished());
    }

    #[test]
    fn fade_transition_reverse_finishes_on_zero_frame_only() {
        let win = Rc::new(RefCell::new(GameWindow::new()));
        let mut transition = FadeTransition::new();
        transition.init(Some(win.clone()), &HashMap::new());

        transition.reverse();
        transition.update(9);
        assert!(!transition.is_finished());

        transition.update(0);
        assert!(transition.is_finished());
    }

    #[test]
    fn text_type_transition_localizes_window_text_on_init() {
        let _ = GameText::init_runtime_strings();
        let win = Rc::new(RefCell::new(GameWindow::new()));
        win.borrow_mut().set_text("GUI:Back").unwrap();
        let mut transition = TextTypeTransition::new();

        transition.init(Some(win), &HashMap::new());

        assert_eq!(transition.full_text, "BACK");
    }

    #[test]
    fn count_up_transition_keeps_numeric_text_after_localization() {
        let win = Rc::new(RefCell::new(GameWindow::new()));
        win.borrow_mut().set_text("1234").unwrap();
        let mut transition = CountUpTransition::new();

        transition.init(Some(win), &HashMap::new());

        assert_eq!(transition.full_text, "1234");
        assert_eq!(transition.int_value, 1234);
    }
}
