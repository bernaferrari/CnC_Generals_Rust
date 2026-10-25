//! UI tooltip drawing with an explicit receiving renderer and mouse.
//! Device drawing belongs to WindowDrawContext; no process renderer or TLS
//! deferred UI work queue exists.
use super::ui_renderer::UIRenderer;

fn color_u8_to_f32(c: [u8; 4]) -> [f32; 4] {
    [
        c[0] as f32 / 255.0,
        c[1] as f32 / 255.0,
        c[2] as f32 / 255.0,
        c[3] as f32 / 255.0,
    ]
}

/// Advance Mouse tooltip still-time / delay (C++ `Mouse::update`).
pub fn tick_cursor_tooltip() {
    crate::input::mouse::with_mouse(|mouse| mouse.update());
}

pub fn cursor_tooltip_already_submitted() -> bool {
    crate::input::mouse::with_mouse(|mouse| mouse.tooltip_draw_submitted())
}

/// C++ `Mouse::drawTooltip` (Mouse.cpp:963-1023): fill, border, wrap, highlight.
pub fn submit_cursor_tooltip(renderer: &mut UIRenderer) -> bool {
    crate::input::mouse::with_mouse(|mouse| submit_cursor_tooltip_from_mouse(renderer, mouse))
}

pub fn submit_cursor_tooltip_from_mouse(
    renderer: &mut UIRenderer,
    mouse: &mut crate::input::mouse::Mouse,
) -> bool {
    use glam::Vec2;

    let (sw, sh) = renderer.screen_size();
    let screen_w = if sw == 0 { 1024.0 } else { sw as f32 };
    let screen_h = if sh == 0 { 768.0 } else { sh as f32 };

    if !mouse.get_visibility() || mouse.tooltip_draw_submitted() {
        return false;
    }
    let Some(info) = mouse.compute_tooltip_draw_info(screen_w, screen_h) else {
        return false;
    };

    let back = color_u8_to_f32(info.back_color);
    let border = color_u8_to_f32(info.border_color);
    let text = color_u8_to_f32(info.text_color);
    let shadow = color_u8_to_f32(info.shadow_color);
    let highlight = color_u8_to_f32(info.highlight_color);

    let box_w = (info.box_width + 2.0).max(2.0);
    let box_h = info.height + 2.0;
    let rect = super::ui_renderer::UIRect::new(info.x, info.y, box_w, box_h);
    renderer.draw_rect(rect, back, 0.0);
    renderer.draw_rect_outline(rect, 1.0, border, 0.0);

    let clip = super::ui_renderer::UIRect::new(
        info.x + 2.0,
        info.y + 1.0,
        info.box_width.max(0.0),
        info.height,
    );
    let font = info.font_size;
    let line_h = info.line_height;
    for (i, line) in info.lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        let y = info.y + 1.0 + i as f32 * line_h;
        let x = info.x + 2.0;
        let _ = renderer.draw_text_simple_with_scissor(
            line,
            Vec2::new(x + 1.0, y + 1.0),
            font,
            shadow,
            clip,
        );
        let _ = renderer.draw_text_simple_with_scissor(line, Vec2::new(x, y), font, text, clip);
        let hl_lo = (info.highlight_pos as f32 - 15.0).max(0.0);
        let hl_hi = info.highlight_pos as f32;
        if hl_hi > hl_lo {
            let hl_clip = super::ui_renderer::UIRect::new(
                info.x + 2.0 + hl_lo,
                info.y + 1.0,
                (hl_hi - hl_lo).max(1.0),
                info.height,
            );
            let _ = renderer.draw_text_simple_with_scissor(
                line,
                Vec2::new(x, y),
                font,
                highlight,
                hl_clip,
            );
        }
    }

    mouse.mark_tooltip_draw_submitted();
    true
}

pub fn tick_cursor_tooltip_from_mouse(mouse: &mut crate::input::mouse::Mouse) {
    mouse.update();
}
pub fn cursor_tooltip_already_submitted_from_mouse(mouse: &crate::input::mouse::Mouse) -> bool {
    mouse.tooltip_draw_submitted()
}
