//! Live GameClient present-path HUD for military subtitles and named timers.
//!
//! Crate `InGameUI` already typewrites and formats these. The live
//! `InGameUISubsystem` / `impl_draw` snapshot used to dump the full string
//! and never called `add_named_timer`. This module is the shared live state.

use super::{MilitarySubtitle, NamedTimerData};
use crate::game_text::GameText;
use crate::gui::callbacks::diplomacy::update_diplomacy_briefing_text;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::{TheAudio, TheGameLogic};
use std::sync::Mutex;

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

struct LiveHud {
    subtitle: Option<MilitarySubtitle>,
    caption_speed: i32,
    caption_point_size: i32,
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
    fn new() -> Self {
        Self {
            subtitle: None,
            caption_speed: 1,
            caption_point_size: 12,
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
        }
    }
}

fn live_hud() -> &'static Mutex<LiveHud> {
    static HUD: Mutex<LiveHud> = Mutex::new(LiveHud {
        subtitle: None,
        caption_speed: 1,
        caption_point_size: 12,
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
    });
    &HUD
}

fn military_caption_text(label: &str) -> String {
    GameText::fetch(label)
}

fn play_typing_sound() {
    if let Some(audio) = TheAudio::get() {
        let event = AudioEventRts::new("MilitarySubtitlesTyping");
        let _ = audio.add_audio_event(&event);
    }
}

/// C++ InGameUI::militarySubtitle — start typewriter from empty displayStrings[0].
pub fn start_military_subtitle(label: &str, duration_ms: i32) {
    let mut hud = live_hud().lock().unwrap_or_else(|e| e.into_inner());
    hud.subtitle = None;
    update_diplomacy_briefing_text(label, false);
    let title = military_caption_text(label);
    if title.is_empty() || duration_ms <= 0 {
        return;
    }
    let frame = TheGameLogic::get_frame();
    let lifetime_frame = frame + (30 * duration_ms.max(0) as u32) / 1000;
    let delay = super::InGameUI::military_caption_delay_frames();
    let pos = hud.caption_position;
    hud.subtitle = Some(MilitarySubtitle {
        text: title,
        index: 0,
        position: pos,
        lifetime_frame,
        block_drawn: true,
        block_begin_frame: frame,
        block_pos: pos,
        increment_on_frame: frame + delay,
        color: hud.caption_color,
        display_lines: vec![String::new()],
        current_display_string: 0,
    });
}

/// Apply InGameUI.ini MilitaryCaptionPosition / MilitaryCaptionColor to the live path.
pub fn apply_military_caption_style(position: (f32, f32), color: u32, point_size: i32) {
    let mut hud = live_hud().lock().unwrap_or_else(|e| e.into_inner());
    hud.caption_position = position;
    hud.caption_color = color;
    if point_size > 0 {
        hud.caption_point_size = point_size;
    }
}

pub fn add_named_timer(name: &str, text: &str, is_countdown: bool) {
    let mut hud = live_hud().lock().unwrap_or_else(|e| e.into_inner());
    hud.named_timers.retain(|t| t.name != name);
    let remaining = script_counter_value(name).unwrap_or(0);
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
}

pub fn remove_named_timer(name: &str) {
    let mut hud = live_hud().lock().unwrap_or_else(|e| e.into_inner());
    hud.named_timers.retain(|t| t.name != name);
}

pub fn show_named_timer_display(show: bool) {
    let mut hud = live_hud().lock().unwrap_or_else(|e| e.into_inner());
    hud.show_named_timers = show;
}

fn script_counter_value(name: &str) -> Option<i32> {
    gamelogic::scripting::engine::get_script_engine()
        .read()
        .ok()
        .and_then(|guard| {
            guard
                .as_ref()
                .and_then(|engine| engine.get_counter(name).map(|c| c.value))
        })
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
    let Some(ch) = subtitle.text.chars().nth(subtitle.index) else {
        subtitle.increment_on_frame = subtitle.lifetime_frame + 1;
        return;
    };
    if ch == '\n' {
        subtitle.block_pos.1 += hud.caption_point_size.max(1) as f32;
        subtitle.current_display_string = subtitle.current_display_string.saturating_add(1);
        if subtitle.current_display_string >= MAX_SUBTITLE_LINES {
            subtitle.index = subtitle.text.chars().count();
        } else {
            subtitle.block_pos.0 = subtitle.position.0;
            if subtitle.display_lines.len() <= subtitle.current_display_string {
                subtitle.display_lines.push(String::new());
            }
            subtitle.block_drawn = true;
            subtitle.increment_on_frame = frame + super::InGameUI::military_caption_delay_frames();
        }
    } else {
        if subtitle.display_lines.is_empty() {
            subtitle.display_lines.push(String::new());
            subtitle.current_display_string = 0;
        }
        let line = subtitle
            .current_display_string
            .min(subtitle.display_lines.len().saturating_sub(1));
        subtitle.display_lines[line].push(ch);
        let printed = subtitle.display_lines[line].chars().count();
        subtitle.block_pos.0 =
            subtitle.position.0 + printed as f32 * hud.caption_point_size.max(1) as f32 * 0.6;
        subtitle.increment_on_frame = frame + hud.caption_speed.max(1) as u32;
        play_typing_sound();
    }
    subtitle.index += 1;
    if subtitle.index >= subtitle.text.chars().count() {
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
        let script_frames = script_counter_value(&timer.name);
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

/// Typed caption + blinking block for the live postDraw path.
pub fn live_military_subtitle_draw(
    frame: u32,
) -> Option<(String, bool, u32, (f32, f32), (f32, f32))> {
    let mut hud = live_hud().lock().unwrap_or_else(|e| e.into_inner());
    step_to_frame(&mut hud, frame);
    hud.subtitle.as_ref().map(|s| {
        (
            s.visible_text(),
            s.block_drawn,
            s.color,
            s.position,
            s.block_pos,
        )
    })
}

/// Formatted named-timer lines (text, x-fraction, color, ready-font).
pub fn live_named_timer_draw(frame: u32) -> Vec<(String, u32, bool)> {
    let mut hud = live_hud().lock().unwrap_or_else(|e| e.into_inner());
    step_to_frame(&mut hud, frame);
    if !hud.show_named_timers {
        return Vec::new();
    }
    hud.named_timers
        .iter()
        .filter(|t| !t.display_text.is_empty())
        .map(|t| (t.display_text.clone(), t.draw_color, t.use_ready_font))
        .collect()
}

fn argb_to_rgba(color: u32) -> [f32; 4] {
    let a = ((color >> 24) & 0xFF) as f32 / 255.0;
    let r = ((color >> 16) & 0xFF) as f32 / 255.0;
    let g = ((color >> 8) & 0xFF) as f32 / 255.0;
    let b = (color & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}

/// C++ InGameUI.cpp:3654-3677 — READY strip blinks flash color vs default.
pub fn live_superweapon_draw_style(frame: u32, ready: bool) -> ([f32; 4], f32) {
    let mut hud = live_hud().lock().unwrap_or_else(|e| e.into_inner());
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
}

pub fn step_live_hud(frame: u32) {
    let mut hud = live_hud().lock().unwrap_or_else(|e| e.into_inner());
    step_to_frame(&mut hud, frame);
}
