//! Live GameClient present-path HUD for military subtitles and named timers.
//!
//! Crate `InGameUI` already typewrites and formats these. The live
//! `InGameUISubsystem` / `impl_draw` snapshot used to dump the full string
//! and never called `add_named_timer`. This module is the shared live state.

use super::NamedTimerData;
use crate::game_text::GameText;
use crate::gui::{
    callbacks::diplomacy::update_diplomacy_briefing_text,
    display_string::DisplayString,
    font::{FontDesc, GameFont},
    ui_renderer::{UIRect, UIRenderer},
};
use std::{collections::HashMap, sync::Arc};

/// C++ `MAX_SUBTITLE_LINES` (InGameUI.h:235).
const MAX_SUBTITLE_LINES: usize = 4;
/// Retail InGameUI.ini MilitaryCaptionPosition after constructor (10,380).
const INI_CAPTION_POSITION: (f32, f32) = (10.0, 340.0);
/// Retail InGameUI.ini MilitaryCaptionColor white AARRGGBB.
const INI_CAPTION_COLOR: u32 = 0xFFFF_FFFF;
const DEFAULT_NAMED_TIMER_NORMAL: u32 = 0xFFFF_FF00; // yellow
const DEFAULT_NAMED_TIMER_READY: u32 = 0xFFFF_00FF; // magenta
const DEFAULT_NAMED_TIMER_FLASH: u32 = 0xFF00_FFFF; // cyan
/// C++ drawName/drawTime color 0 → default white.
const DEFAULT_SUPERWEAPON_NORMAL: u32 = 0xFFFF_FFFF;
/// Live READY flash stand-in when INI flash color is also white (retail yellow strip).
const DEFAULT_SUPERWEAPON_FLASH: u32 = 0xFFFF_FF33;
/// C++ `parseDurationReal` 0.5s at 30fps — visible READY blink.
const DEFAULT_SUPERWEAPON_FLASH_FRAMES: u32 = 15;

struct LiveMilitarySubtitle {
    text: String,
    next_byte: usize,
    position: (f32, f32),
    lifetime_frame: u32,
    block_drawn: bool,
    block_begin_frame: u32,
    increment_on_frame: u32,
    color: u32,
    display_lines: Vec<DisplayString>,
}

pub struct LiveHud {
    counter_values: HashMap<String, i32>,
    /// Typed caption characters whose C++ `MilitarySubtitlesTyping` sound
    /// the owning UI has not played yet.
    pending_typing_sounds: u32,
    subtitle: Option<LiveMilitarySubtitle>,
    caption_speed: i32,
    caption_fonts: [FontDesc; 2],
    caption_resolution_adjustment: f32,
    caption_position: (f32, f32),
    caption_color: u32,
    named_timers: Vec<NamedTimerData>,
    show_named_timers: bool,
    named_timer_used_flash_color: bool,
    named_timer_last_flash_frame: i32,
    named_timer_flash_duration: i32,
    named_timer_normal_color: u32,
    named_timer_flash_color: u32,
    superweapon_used_flash_color: bool,
    superweapon_last_flash_frame: u32,
    superweapon_flash_duration: u32,
    superweapon_flash_color: u32,
    last_step_frame: u32,
}

impl LiveHud {
    pub fn new() -> Self {
        Self {
            counter_values: HashMap::new(),
            subtitle: None,
            caption_speed: 1,
            caption_fonts: [
                FontDesc::new("Courier New", 12, true),
                FontDesc::new("Courier New", 12, false),
            ],
            caption_resolution_adjustment: 0.7,
            caption_position: INI_CAPTION_POSITION,
            caption_color: INI_CAPTION_COLOR,
            named_timers: Vec::new(),
            show_named_timers: true,
            named_timer_used_flash_color: true,
            named_timer_last_flash_frame: 0,
            named_timer_flash_duration: 1,
            named_timer_normal_color: DEFAULT_NAMED_TIMER_NORMAL,
            named_timer_flash_color: DEFAULT_NAMED_TIMER_FLASH,
            superweapon_used_flash_color: true,
            superweapon_last_flash_frame: 0,
            superweapon_flash_duration: DEFAULT_SUPERWEAPON_FLASH_FRAMES,
            superweapon_flash_color: DEFAULT_SUPERWEAPON_FLASH,
            last_step_frame: 0,
            pending_typing_sounds: 0,
        }
    }
}

impl Default for LiveHud {
    fn default() -> Self {
        Self::new()
    }
}

/// Admit a completed counter observation from this HUD's receiving script world.
pub fn observe_named_timer_counters(hud: &mut LiveHud, counters: HashMap<String, i32>) {
    hud.counter_values = counters;
}

/// Reset match runtime while retaining this client's admitted INI style.
pub fn reset_live_hud(hud: &mut LiveHud) {
    hud.subtitle = None;
    hud.named_timers.clear();
    hud.counter_values.clear();
    hud.show_named_timers = true;
    hud.named_timer_used_flash_color = true;
    hud.named_timer_last_flash_frame = 0;
    hud.superweapon_used_flash_color = true;
    hud.superweapon_last_flash_frame = 0;
    hud.last_step_frame = 0;
}

fn military_caption_text(label: &str) -> String {
    GameText::fetch(label)
}

/// Typing sounds the caption asked for since the last call; the owning UI
/// plays them in its client's engine.
pub fn take_live_hud_typing_sounds(hud: &mut LiveHud) -> u32 {
    std::mem::take(&mut hud.pending_typing_sounds)
}

/// C++ InGameUI::militarySubtitle — start typewriter from empty displayStrings[0].
pub fn start_military_subtitle(
    hud: &mut LiveHud,
    label: &str,
    duration_ms: i32,
    frame: u32,
    manager: &mut crate::gui::window_manager::WindowManager,
) {
    (|hud: &mut LiveHud| {
        hud.subtitle = None;

        let title = military_caption_text(label);
        if title.is_empty() || duration_ms <= 0 {
            return;
        }
        let lifetime_frame = frame + (30 * duration_ms.max(0) as u32) / 1000;
        let delay = super::InGameUI::military_caption_delay_frames();
        let pos = hud.caption_position;
        hud.subtitle = Some(LiveMilitarySubtitle {
            text: title,
            next_byte: 0,
            position: pos,
            lifetime_frame,
            block_drawn: true,
            block_begin_frame: frame,
            increment_on_frame: frame + delay,
            color: hud.caption_color,
            display_lines: vec![DisplayString::new()],
        });
    })(hud);
}

/// Admit the caption's authored origin, color, title font, and body font.
pub fn apply_military_caption_style(
    hud: &mut LiveHud,
    position: (f32, f32),
    color: u32,
    fonts: [FontDesc; 2],
    resolution_adjustment: f32,
) {
    hud.caption_position = position;
    hud.caption_color = color;
    hud.caption_fonts = fonts;
    hud.caption_resolution_adjustment = resolution_adjustment;
}

pub fn add_named_timer(hud: &mut LiveHud, name: &str, text: &str, is_countdown: bool) {
    (|hud: &mut LiveHud| {
        hud.named_timers.retain(|t| t.name != name);
        let remaining = script_counter_value(hud, name).unwrap_or(0);
        let color = hud.named_timer_normal_color;
        hud.named_timers.push(NamedTimerData {
            name: name.to_string(),
            text: text.to_string(),
            is_countdown,
            timestamp: -1,
            color,
            display_text: String::new(),
            use_ready_font: false,
            remaining_frames: remaining,
            last_tick_frame: 0,
            draw_x: 0.0,
            draw_y: 0.0,
            draw_color: color,
        });
    })(hud);
}

pub fn remove_named_timer(hud: &mut LiveHud, name: &str) {
    (|hud: &mut LiveHud| hud.named_timers.retain(|t| t.name != name))(hud);
}

pub fn show_named_timer_display(hud: &mut LiveHud, show: bool) {
    (|hud: &mut LiveHud| hud.show_named_timers = show)(hud);
}

fn script_counter_value(hud: &LiveHud, name: &str) -> Option<i32> {
    hud.counter_values.get(name).copied()
}

fn step_subtitle(hud: &mut LiveHud, frame: u32) {
    let Some(subtitle) = hud.subtitle.as_mut() else {
        return;
    };
    if subtitle.lifetime_frame < frame {
        let alpha = (subtitle.color >> 24) as i32;
        let fade_amount = ((frame - subtitle.lifetime_frame) as f32 * 0.1) as i32;
        if alpha - fade_amount < 0 {
            hud.subtitle = None;
        } else {
            let new_alpha = (alpha - fade_amount) as u32;
            subtitle.color = (subtitle.color & 0x00FF_FFFF) | (new_alpha << 24);
        }
        return;
    }
    if subtitle.block_begin_frame + 9 < frame {
        subtitle.block_begin_frame = frame;
        subtitle.block_drawn = !subtitle.block_drawn;
    }
    if subtitle.increment_on_frame >= frame {
        return;
    }
    let Some(ch) = subtitle.text[subtitle.next_byte..].chars().next() else {
        subtitle.increment_on_frame = subtitle.lifetime_frame + 1;
        return;
    };
    subtitle.next_byte += ch.len_utf8();
    if ch == '\n' {
        if subtitle.display_lines.len() >= MAX_SUBTITLE_LINES {
            subtitle.next_byte = subtitle.text.len();
        } else {
            subtitle.display_lines.push(DisplayString::new());
            subtitle.block_drawn = true;
            subtitle.increment_on_frame = frame + super::InGameUI::military_caption_delay_frames();
        }
    } else {
        subtitle.display_lines.last_mut().unwrap().append_char(ch);
        subtitle.increment_on_frame = frame + hud.caption_speed.max(1) as u32;
        hud.pending_typing_sounds += 1;
    }
    if subtitle.next_byte == subtitle.text.len() {
        subtitle.increment_on_frame = subtitle.lifetime_frame + 1;
    }
}

fn step_named_timers(hud: &mut LiveHud, frame: u32) {
    if !hud.show_named_timers {
        return;
    }
    let mut used_flash = hud.named_timer_used_flash_color;
    let mut last_flash = hud.named_timer_last_flash_frame;
    let flash_duration = hud.named_timer_flash_duration;
    for timer in &mut hud.named_timers {
        let script_frames = hud.counter_values.get(&timer.name).copied();
        let frames_left = if let Some(frames) = script_frames {
            frames
        } else if frame != timer.last_tick_frame
            && timer.is_countdown
            && timer.remaining_frames >= 0
        {
            timer.remaining_frames.saturating_sub(1)
        } else {
            timer.remaining_frames
        };
        timer.remaining_frames = frames_left;
        timer.last_tick_frame = frame;
        let ready_secs = if frames_left > 0 {
            (frames_left as f32 * (1.0 / 30.0)) as i32
        } else {
            0
        };
        timer.use_ready_font = timer.is_countdown && ready_secs == 0;
        timer.display_text = if timer.is_countdown {
            let min = ready_secs / 60;
            let sec = ready_secs - min * 60;
            if sec >= 10 {
                format!("{} {min}:{sec}", timer.text)
            } else {
                format!("{} {min}:0{sec}", timer.text)
            }
        } else {
            format!("{} {frames_left}", timer.text)
        };
        if timer.is_countdown && ready_secs == 0 && flash_duration != 0 {
            if frame as i32 >= last_flash + flash_duration {
                used_flash = !used_flash;
                last_flash = frame as i32;
            }
            timer.draw_color = if used_flash {
                timer.color
            } else {
                hud.named_timer_flash_color
            };
        } else {
            timer.draw_color = if timer.use_ready_font {
                DEFAULT_NAMED_TIMER_READY
            } else {
                timer.color
            };
        }
    }
    hud.named_timer_used_flash_color = used_flash;
    hud.named_timer_last_flash_frame = last_flash;
}

fn step_to_frame(hud: &mut LiveHud, frame: u32) {
    if hud.last_step_frame == frame && frame != 0 {
        return;
    }
    hud.last_step_frame = frame;
    step_subtitle(hud, frame);
    step_named_timers(hud, frame);
}

pub(crate) fn military_subtitle_is_active(hud: &LiveHud) -> bool {
    hud.subtitle.is_some()
}

/// Draw the caption and its cursor using the same display strings and font metrics.
pub(crate) fn paint_live_military_subtitle(
    hud: &mut LiveHud,
    renderer: &mut UIRenderer,
    viewport: (f32, f32),
) {
    let Some(subtitle) = hud.subtitle.as_mut() else {
        return;
    };
    let (width, height) = viewport;
    let x = (subtitle.position.0 * (width / 800.0)) as i32;
    let mut y = (subtitle.position.1 * (height / 600.0)) as i32;
    let font_scale =
        (1.0 + (width / 800.0 - 1.0) * hud.caption_resolution_adjustment).clamp(1.0, 2.0);
    let drop_color = subtitle.color & 0xff00_0000;
    let final_line = subtitle.display_lines.len() - 1;
    for (index, line) in subtitle.display_lines.iter_mut().enumerate() {
        let face = &hud.caption_fonts[usize::from(index > 0)];
        let point_size = ((face.size as f32 * font_scale) as i32).clamp(1, 100);
        if line.get_font().is_none_or(|font| {
            font.desc.size != point_size || font.desc.bold != face.bold || font.desc.name != face.name
        }) {
            let desc = FontDesc::new(&face.name, point_size, face.bold);
            let font = renderer
                .registered_font(&desc)
                .or_else(|| GameFont::new(desc).ok().map(Arc::new));
            if let Some(font) = font {
                line.set_font(font);
            }
        }
        line.draw_with_renderer(renderer, x, y, subtitle.color, drop_color, 1, 1);
        let (advance, line_height) = line.get_size();
        if index == final_line && subtitle.block_drawn {
            let cell = line.get_font().map_or(line_height, |font| font.height);
            // Leave one pixel after typed text for the raster's right bearing.
            let cursor_x = x + advance + i32::from(advance > 0);
            renderer.draw_rect(
                UIRect::new(
                    cursor_x as f32,
                    y as f32,
                    (cell as f32 * 0.8).trunc(),
                    cell as f32,
                ),
                argb_to_rgba(subtitle.color),
                0.0,
            );
        }
        y += line_height;
    }
}

#[cfg(test)]
pub(super) fn live_military_subtitle_draw(
    hud: &mut LiveHud,
    frame: u32,
) -> Option<(String, bool, u32, (f32, f32))> {
    step_to_frame(hud, frame);
    hud.subtitle.as_ref().map(|s| {
        let text = s
            .display_lines
            .iter()
            .map(DisplayString::get_text)
            .collect::<Vec<_>>()
            .join("\n");
        (text, s.block_drawn, s.color, s.position)
    })
}

/// Formatted named-timer lines (text, x-fraction, color, ready-font).
pub fn live_named_timer_draw(hud: &mut LiveHud, frame: u32) -> Vec<(String, u32, bool)> {
    (|hud: &mut LiveHud| {
        step_to_frame(hud, frame);
        if !hud.show_named_timers {
            return Vec::new();
        }
        hud.named_timers
            .iter()
            .filter(|t| !t.display_text.is_empty())
            .map(|t| (t.display_text.clone(), t.draw_color, t.use_ready_font))
            .collect()
    })(hud)
}

fn argb_to_rgba(color: u32) -> [f32; 4] {
    let a = ((color >> 24) & 0xFF) as f32 / 255.0;
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}

/// C++ InGameUI.cpp:3654-3677 — READY strip blinks flash color vs default.
pub fn live_superweapon_draw_style(hud: &mut LiveHud, frame: u32, ready: bool) -> ([f32; 4], f32) {
    (|hud: &mut LiveHud| {
        let mut state = super::SuperweaponFlashState {
            used_flash_color: hud.superweapon_used_flash_color,
            last_flash_frame: hud.superweapon_last_flash_frame,
        };
        let style = super::superweapon_ready_draw_style(
            frame,
            ready,
            hud.superweapon_flash_duration,
            argb_to_rgba(hud.superweapon_flash_color),
            &mut state,
        );
        hud.superweapon_used_flash_color = state.used_flash_color;
        hud.superweapon_last_flash_frame = state.last_flash_frame;
        style
    })(hud)
}

pub fn step_live_hud(hud: &mut LiveHud, frame: u32) {
    (|hud: &mut LiveHud| step_to_frame(hud, frame))(hud);
}
