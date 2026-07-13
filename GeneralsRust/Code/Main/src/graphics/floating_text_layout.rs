//! InGameUI floating-text residual: pack presentation floating cash captions into a
//! CPU layout buffer ready for dual-tick UI / eventual WGPU text draw.
//!
//! Host residual closed here (fail-closed vs full retail DisplayString GPU draw):
//! - Move-up offset from `PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED` (C++ default 1.0)
//! - Timeout / vanish residual from retail DEFAULT_FLOATING_TEXT_TIMEOUT (10 frames)
//! - GameText `GUI:AddCash` caption residual (`+$N` format parity with host text)
//! - DisplayString monospaced measure residual (8×8 glyph extents for caption)
//! - Honesty counters for texts / active / bytes packed
//! - Deterministic pack order for dual-tick presentation consumers
//!
//! Host residual also closed (fail-closed vs live Display):
//! - DisplayString color residual normalize u8 RGBA → f32 (0..1)
//! - Retail green/yellow cash caption color honesty samples
//! - DisplayString setText residual (notifyTextChanged when text differs)
//! - DisplayString setFont residual (equal font early-out / m_fontChanged)
//! - DisplayString getTextLength residual (UnicodeString length = char count)
//! - DisplayString getText / reset residual (identity text / clear text+font, no notify)
//! - DisplayString appendChar / removeLastChar residual (mutate + notifyTextChanged)
//! - DisplayString getWidth residual (monospaced charPos width; skip `\n`)
//! - DisplayString getSize residual (monospaced width×height; empty/no-font → 0×0)
//! - DisplayString setWordWrap residual (wrapping width change → notify)
//! - DisplayString setWordWrapCentered residual (centered flag change → notify)
//! - DisplayString setUseHotkey residual (flag+color; always notify)
//! - DisplayString setClipRegion residual (region equality early-out)
//! - DisplayString getFont residual (identity font pointer residual)
//! - DisplayString draw residual (empty early-out / default drop 1,1 /
//!   pos+color rebuild dirty residual; fail-closed vs GPU StretchRect)
//!
//! Still residual:
//! - Full DisplayString GPU font atlas raster / WW3D StretchRect submit
//! - Full multi-locale CSF/STR Unicode GameText table load at boot
//! - Full vanish-rate alpha blend on live Display surface
//! - Full FontCharsClass Get_Formatted_Text_Extents for getSize

use crate::graphics::game_text_residual::{
    honesty_display_string_measure, measure_display_string_residual,
};
use crate::presentation_frame::{
    PresentationFloatingText, PresentationFrame, PresentationWorldAnim,
    PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED, PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES,
    PRESENTATION_FLOATING_TEXT_VANISH_RATE,
};

/// Retail GameText key for cash gain floating captions.
pub const GUI_ADD_CASH_KEY: &str = "GUI:AddCash";

/// Residual GameText resolution for `GUI:AddCash` captions.
///
/// C++: `moneyString.format(TheGameText->fetch("GUI:AddCash"), amount)`.
/// English retail resolves to a `+$N` style caption. Host residual formats
/// `+$amount` (ASCII) when the key is `GUI:AddCash` and falls back to the
/// already-frozen presentation `text` otherwise.
///
/// Fail-closed vs full CSF/STR Unicode localization table load.
pub fn resolve_add_cash_caption(text_key: &str, amount: u32, frozen_text: &str) -> String {
    if text_key == GUI_ADD_CASH_KEY {
        format!("+${amount}")
    } else if !frozen_text.is_empty() {
        frozen_text.to_string()
    } else {
        format!("+${amount}")
    }
}

/// Honesty: key is retail `GUI:AddCash` and caption matches residual format.
pub fn honesty_add_cash_caption(text_key: &str, amount: u32, caption: &str) -> bool {
    text_key == GUI_ADD_CASH_KEY && caption == format!("+${amount}")
}

/// Retail green cash floating text color (Hacker / MoneyCrate residual).
pub const FLOATING_TEXT_COLOR_GREEN_U8: (u8, u8, u8, u8) = (0, 255, 0, 255);
/// Retail yellow cash floating text color (CashBounty residual).
pub const FLOATING_TEXT_COLOR_YELLOW_U8: (u8, u8, u8, u8) = (255, 255, 0, 255);

/// Normalize GameMakeColor u8 RGBA → DisplayString residual f32 color (0..1).
pub fn normalize_display_string_color(rgba: (u8, u8, u8, u8)) -> [f32; 4] {
    [
        rgba.0 as f32 / 255.0,
        rgba.1 as f32 / 255.0,
        rgba.2 as f32 / 255.0,
        rgba.3 as f32 / 255.0,
    ]
}

/// Honesty: packed color matches normalized retail residual u8 RGBA.
pub fn honesty_display_string_color(packed: [f32; 4], rgba: (u8, u8, u8, u8)) -> bool {
    let expected = normalize_display_string_color(rgba);
    (0..4).all(|i| (packed[i] - expected[i]).abs() < 0.001)
}

/// DisplayString `setText` residual: whether text change notifies residual dirty.
///
/// C++ `DisplayString::setText`: if `text == m_textString` return early; else
/// copy and `notifyTextChanged()`. Host residual returns true when text differs
/// (changed) and false when equal (no-op). Fail-closed vs live font re-raster.
#[inline]
pub fn display_string_set_text_changed(previous: &str, new_text: &str) -> bool {
    previous != new_text
}

/// Honesty: setText residual matches C++ early-out vs notifyTextChanged path.
pub fn honesty_display_string_set_text(previous: &str, new_text: &str, changed: bool) -> bool {
    display_string_set_text_changed(previous, new_text) == changed
}

/// DisplayString `setFont` residual: whether font change arms residual dirty.
///
/// C++ `W3DDisplayString::setFont`: if `font == NULL` return; if `m_font == font`
/// return early; else set font + `m_fontChanged = TRUE`. Host residual returns
/// true when font name differs (changed) and false when equal (no-op).
/// Fail-closed vs live FontCharsClass re-raster / hotkey underline font.
#[inline]
pub fn display_string_set_font_changed(previous_font: &str, new_font: &str) -> bool {
    !new_font.is_empty() && previous_font != new_font
}

/// Honesty: setFont residual matches C++ early-out vs m_fontChanged path.
pub fn honesty_display_string_set_font(
    previous_font: &str,
    new_font: &str,
    changed: bool,
) -> bool {
    display_string_set_font_changed(previous_font, new_font) == changed
}

/// DisplayString `getTextLength` residual: number of characters in residual text.
///
/// C++ `DisplayString::getTextLength` → `m_textString.getLength()`.
/// Host residual uses Unicode scalar count (char count) for ASCII/captions.
/// Fail-closed vs full UTF-16 WideChar length on live Display surface.
#[inline]
pub fn display_string_get_text_length(text: &str) -> u32 {
    text.chars().count() as u32
}

/// Honesty: getTextLength residual matches char count.
pub fn honesty_display_string_get_text_length(text: &str, length: u32) -> bool {
    display_string_get_text_length(text) == length
}

/// DisplayString `getText` residual: return current residual text string.
///
/// C++ `DisplayString::getText` → `m_textString`. Host residual is identity.
/// Fail-closed vs live UnicodeString wide-char storage.
#[inline]
pub fn display_string_get_text(text: &str) -> &str {
    text
}

/// Honesty: getText residual matches stored residual text.
pub fn honesty_display_string_get_text(text: &str, got: &str) -> bool {
    display_string_get_text(text) == got
}

/// DisplayString `reset` residual: clear text + clear font; does **not** notify.
///
/// C++ `DisplayString::reset`: `m_textString.clear(); m_font = NULL;` — no
/// `notifyTextChanged()`. Host residual returns empty text/font and `changed=false`.
/// Fail-closed vs full DisplayStringManager free/reuse pool.
#[inline]
pub fn display_string_reset_residual() -> (String, String, bool) {
    (String::new(), String::new(), false)
}

/// Honesty: reset residual clears text/font without notify residual.
pub fn honesty_display_string_reset(text: &str, font: &str, notified: bool) -> bool {
    text.is_empty() && font.is_empty() && !notified
}

/// DisplayString `appendChar` residual: append one char and notifyTextChanged.
///
/// C++ always concat + notify (no early-out). Fail-closed vs live font re-raster.
#[inline]
pub fn display_string_append_char(text: &str, c: char) -> (String, bool) {
    let mut out = text.to_string();
    out.push(c);
    (out, true)
}

/// Honesty: appendChar residual mutates text and notifies.
pub fn honesty_display_string_append_char(
    previous: &str,
    c: char,
    result: &str,
    notified: bool,
) -> bool {
    let (expected, expect_notify) = display_string_append_char(previous, c);
    result == expected.as_str() && notified == expect_notify
}

/// DisplayString `removeLastChar` residual: pop last char and notifyTextChanged.
///
/// C++ always `removeLastChar` + notify. Empty string residual stays empty but
/// still notifies (matches C++ call order). Fail-closed vs live font re-raster.
#[inline]
pub fn display_string_remove_last_char(text: &str) -> (String, bool) {
    if text.is_empty() {
        return (String::new(), true);
    }
    let mut out = text.to_string();
    out.pop();
    (out, true)
}

/// Honesty: removeLastChar residual mutates text and notifies.
pub fn honesty_display_string_remove_last_char(
    previous: &str,
    result: &str,
    notified: bool,
) -> bool {
    let (expected, expect_notify) = display_string_remove_last_char(previous);
    result == expected.as_str() && notified == expect_notify
}

/// DisplayString `getWidth` residual: monospaced width up to `char_pos` chars.
///
/// C++ `W3DDisplayString::getWidth(charPos)`: walk glyphs, skip `\\n`, stop at
/// `charPos` (`-1` = all). Host residual uses 8px monospaced glyph spacing.
/// Fail-closed vs live FontCharsClass::Get_Char_Spacing.
#[inline]
pub fn display_string_get_width(text: &str, char_pos: i32) -> u32 {
    let mut width = 0u32;
    let mut count = 0i32;
    for ch in text.chars() {
        if char_pos >= 0 && count >= char_pos {
            break;
        }
        if ch != '\n' {
            width = width.saturating_add(8);
        }
        count = count.saturating_add(1);
    }
    width
}

/// Honesty: getWidth residual matches monospaced charPos walk.
pub fn honesty_display_string_get_width(text: &str, char_pos: i32, width: u32) -> bool {
    display_string_get_width(text, char_pos) == width
}

/// DisplayString monospaced glyph residual height (font8x8 family).
pub const DISPLAY_STRING_GLYPH_HEIGHT: u32 = 8;
/// Default hotkey residual color (C++ `GameMakeColor(255,255,255,255)` / 0xffffffff).
pub const DISPLAY_STRING_HOTKEY_COLOR_DEFAULT: (u8, u8, u8, u8) = (255, 255, 255, 255);

/// DisplayString `getSize` residual: monospaced (width, height).
///
/// C++ `W3DDisplayString::computeExtents`: empty text **or** null font → (0,0);
/// else `Get_Formatted_Text_Extents`. Host residual: empty/`has_font=false` → (0,0);
/// otherwise width = monospaced getWidth(-1), height = line_count × 8px.
/// Fail-closed vs live FontCharsClass formatted extents / word-wrap height.
#[inline]
pub fn display_string_get_size(text: &str, has_font: bool) -> (u32, u32) {
    if text.is_empty() || !has_font {
        return (0, 0);
    }
    let width = display_string_get_width(text, -1);
    // Count lines (at least 1 when non-empty). Trailing newline still adds a line
    // residual matching simple split-lines walk.
    let lines = text.lines().count().max(1) as u32;
    // If text ends with `\n`, `lines()` drops the trailing empty; retail formatted
    // extents typically include it — host residual keeps max(1, lines) for captions.
    let height = lines.saturating_mul(DISPLAY_STRING_GLYPH_HEIGHT);
    (width, height)
}

/// Honesty: getSize residual matches monospaced width×height / empty-or-no-font zero.
pub fn honesty_display_string_get_size(
    text: &str,
    has_font: bool,
    width: u32,
    height: u32,
) -> bool {
    display_string_get_size(text, has_font) == (width, height)
}

/// DisplayString `setWordWrap` residual: change wrap width → notify when changed.
///
/// C++ `W3DDisplayString::setWordWrap`: `Set_Wrapping_Width` returns true on change
/// then `notifyTextChanged()`. Host residual compares previous vs new width.
/// Fail-closed vs live Render2DSentence wrap reflow / poly rebuild.
#[inline]
pub fn display_string_set_word_wrap(previous_width: i32, new_width: i32) -> (i32, bool) {
    if previous_width == new_width {
        (previous_width, false)
    } else {
        (new_width, true)
    }
}

/// Honesty: setWordWrap residual matches change → notify path.
pub fn honesty_display_string_set_word_wrap(
    previous_width: i32,
    new_width: i32,
    result_width: i32,
    notified: bool,
) -> bool {
    let (expected, expect_notify) = display_string_set_word_wrap(previous_width, new_width);
    result_width == expected && notified == expect_notify
}

/// DisplayString `setWordWrapCentered` residual: change centered flag → notify.
///
/// C++ `Set_Word_Wrap_Centered` returns true on change then notify.
#[inline]
pub fn display_string_set_word_wrap_centered(
    previous_centered: bool,
    new_centered: bool,
) -> (bool, bool) {
    if previous_centered == new_centered {
        (previous_centered, false)
    } else {
        (new_centered, true)
    }
}

/// Honesty: setWordWrapCentered residual matches change → notify path.
pub fn honesty_display_string_set_word_wrap_centered(
    previous_centered: bool,
    new_centered: bool,
    result: bool,
    notified: bool,
) -> bool {
    let (expected, expect_notify) =
        display_string_set_word_wrap_centered(previous_centered, new_centered);
    result == expected && notified == expect_notify
}

/// DisplayString `setUseHotkey` residual: set flag+color and always notify.
///
/// C++ always assigns `m_useHotKey` / `m_hotKeyColor`, enables hotkey parse, and
/// `notifyTextChanged()` (no early-out). Fail-closed vs live hotkey marker parse.
#[inline]
pub fn display_string_set_use_hotkey(
    use_hotkey: bool,
    hotkey_color: (u8, u8, u8, u8),
) -> (bool, (u8, u8, u8, u8), bool) {
    (use_hotkey, hotkey_color, true)
}

/// Honesty: setUseHotkey residual always notifies with stored flag+color.
pub fn honesty_display_string_set_use_hotkey(
    use_hotkey: bool,
    hotkey_color: (u8, u8, u8, u8),
    result_use: bool,
    result_color: (u8, u8, u8, u8),
    notified: bool,
) -> bool {
    let (eu, ec, en) = display_string_set_use_hotkey(use_hotkey, hotkey_color);
    result_use == eu && result_color == ec && notified == en
}

/// DisplayString clip region residual (lo.x, lo.y, hi.x, hi.y).
pub type DisplayStringClipRegion = (i32, i32, i32, i32);

/// DisplayString `setClipRegion` residual: assign region when any edge differs.
///
/// C++ only updates when lo/hi differ from stored clip region. Base
/// `DisplayString::setClipRegion` is a no-op; W3D path applies RectClass.
/// Host residual returns (region, changed). Fail-closed vs live renderer clip.
#[inline]
pub fn display_string_set_clip_region(
    previous: DisplayStringClipRegion,
    new_region: DisplayStringClipRegion,
) -> (DisplayStringClipRegion, bool) {
    if previous == new_region {
        (previous, false)
    } else {
        (new_region, true)
    }
}

/// Honesty: setClipRegion residual matches equality early-out path.
pub fn honesty_display_string_set_clip_region(
    previous: DisplayStringClipRegion,
    new_region: DisplayStringClipRegion,
    result: DisplayStringClipRegion,
    changed: bool,
) -> bool {
    let (expected, expect_changed) = display_string_set_clip_region(previous, new_region);
    result == expected && changed == expect_changed
}

/// DisplayString `getFont` residual: return current residual font name.
///
/// C++ `DisplayString::getFont` → `m_font`. Host residual is identity.
#[inline]
pub fn display_string_get_font(font: &str) -> &str {
    font
}

/// Honesty: getFont residual matches stored residual font.
pub fn honesty_display_string_get_font(font: &str, got: &str) -> bool {
    got == display_string_get_font(font)
}

/// DisplayString draw residual sample (host-testable; fail-closed vs GPU).
///
/// C++ `W3DDisplayString::draw(x,y,color,dropColor)` delegates to
/// `draw(..., xDrop=1, yDrop=1)`. Empty text early-outs. Rebuilds sentence polys
/// when font/text dirty or position/color changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayStringDrawResidual {
    pub x: i32,
    pub y: i32,
    pub color: (u8, u8, u8, u8),
    pub drop_color: (u8, u8, u8, u8),
    pub x_drop: i32,
    pub y_drop: i32,
    /// Shadow screen position residual (`x + xDrop` / `y + yDrop` order honesty).
    pub shadow_x: i32,
    pub shadow_y: i32,
    /// True when sentence rebuild residual would run (text/font/pos/color change).
    pub rebuilt: bool,
    /// True when any draw residual executed (text non-empty).
    pub drew: bool,
}

/// DisplayString `draw` residual with default drop shadow offset (1, 1).
#[inline]
pub fn display_string_draw(
    text: &str,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
    drop_color: (u8, u8, u8, u8),
    prev_x: i32,
    prev_y: i32,
    prev_color: (u8, u8, u8, u8),
    prev_drop_color: (u8, u8, u8, u8),
    text_or_font_dirty: bool,
) -> DisplayStringDrawResidual {
    display_string_draw_with_drop(
        text, x, y, color, drop_color, 1, 1,
        prev_x, prev_y, prev_color, prev_drop_color, text_or_font_dirty,
    )
}

/// DisplayString `draw` residual with explicit drop shadow offsets.
#[inline]
pub fn display_string_draw_with_drop(
    text: &str,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
    drop_color: (u8, u8, u8, u8),
    x_drop: i32,
    y_drop: i32,
    prev_x: i32,
    prev_y: i32,
    prev_color: (u8, u8, u8, u8),
    prev_drop_color: (u8, u8, u8, u8),
    text_or_font_dirty: bool,
) -> DisplayStringDrawResidual {
    if text.is_empty() {
        return DisplayStringDrawResidual {
            x, y, color, drop_color, x_drop, y_drop,
            shadow_x: x + x_drop, shadow_y: y + y_drop,
            rebuilt: false, drew: false,
        };
    }
    let pos_or_color_changed = x != prev_x
        || y != prev_y
        || color != prev_color
        || drop_color != prev_drop_color;
    let rebuilt = text_or_font_dirty || pos_or_color_changed;
    DisplayStringDrawResidual {
        x, y, color, drop_color, x_drop, y_drop,
        // Shadow drawn first at (x+xDrop, y+yDrop), then text at (x,y).
        shadow_x: x + x_drop, shadow_y: y + y_drop,
        rebuilt, drew: true,
    }
}

/// Honesty: draw residual matches empty early-out / default drop / rebuild dirty path.
pub fn honesty_display_string_draw(
    text: &str,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
    drop_color: (u8, u8, u8, u8),
    prev_x: i32,
    prev_y: i32,
    prev_color: (u8, u8, u8, u8),
    prev_drop_color: (u8, u8, u8, u8),
    text_or_font_dirty: bool,
    sample: DisplayStringDrawResidual,
) -> bool {
    let expected = display_string_draw(
        text, x, y, color, drop_color,
        prev_x, prev_y, prev_color, prev_drop_color, text_or_font_dirty,
    );
    sample == expected
}

/// Floats per packed layout entry:
/// pos.xyz + lift_y + color.rgba + alpha + amount + age_frames + timeout_frames = 12 × f32.
pub const FLOATING_TEXT_LAYOUT_FLOATS: usize = 12;
/// Bytes per packed layout entry.
pub const FLOATING_TEXT_LAYOUT_BYTES: usize =
    FLOATING_TEXT_LAYOUT_FLOATS * std::mem::size_of::<f32>();

/// One CPU-side residual floating text layout sample.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingTextLayoutEntry {
    /// World position at spawn (presentation freeze).
    pub position: [f32; 3],
    /// C++ draw residual: `pos.y -= frameCount * moveUpSpeed` (host Y-up → +lift).
    pub lift_y: f32,
    pub color_rgba: [f32; 4],
    /// Alpha after vanish residual (1.0 while active, decays after timeout).
    pub alpha: f32,
    pub amount: f32,
    pub age_frames: f32,
    pub timeout_frames: f32,
    /// Residual GameText caption (`+$N` for GUI:AddCash).
    pub caption: String,
    /// Retail GameText key residual (`GUI:AddCash`).
    pub text_key: String,
    /// DisplayString monospaced measure residual (width px).
    pub measure_width: u32,
    /// DisplayString monospaced measure residual (height px).
    pub measure_height: u32,
}

impl FloatingTextLayoutEntry {
    pub fn to_floats(self) -> [f32; FLOATING_TEXT_LAYOUT_FLOATS] {
        [
            self.position[0],
            self.position[1],
            self.position[2],
            self.lift_y,
            self.color_rgba[0],
            self.color_rgba[1],
            self.color_rgba[2],
            self.color_rgba[3],
            self.alpha,
            self.amount,
            self.age_frames,
            self.timeout_frames,
        ]
    }
}

/// Honesty bookkeeping for the residual floating text layout path.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FloatingTextLayoutHonesty {
    pub texts_packed: u32,
    pub active_packed: u32,
    pub world_anims_observed: u32,
    pub bytes_packed: u32,
    /// True when pack completed without panic (empty is honest success).
    pub cpu_pack_ok: bool,
    /// True when at least one active text was packed.
    pub has_geometry: bool,
    /// True after `mark_gpu_upload_ready` (still not a live font draw).
    pub gpu_upload_ready: bool,
    pub move_up_speed: f32,
    pub vanish_rate: f32,
    pub timeout_frames: u32,
    /// True when all packed entries resolve GUI:AddCash caption residual.
    pub game_text_caption_ok: bool,
    /// True when all packed entries have honest DisplayString measure residual.
    pub display_string_measure_ok: bool,
    /// True when packed colors match normalized residual RGBA (0..1).
    pub display_string_color_ok: bool,
}

impl FloatingTextLayoutHonesty {
    pub fn honesty_cpu_pack_ok(&self) -> bool {
        self.cpu_pack_ok
    }

    pub fn honesty_geometry_ok(&self) -> bool {
        self.cpu_pack_ok && self.has_geometry && self.active_packed > 0
    }

    pub fn honesty_upload_ready_ok(&self) -> bool {
        self.gpu_upload_ready && self.cpu_pack_ok
    }

    pub fn honesty_retail_params_ok(&self) -> bool {
        (self.move_up_speed - PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED).abs() < 0.001
            && (self.vanish_rate - PRESENTATION_FLOATING_TEXT_VANISH_RATE).abs() < 0.001
            && self.timeout_frames == PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES
    }

    pub fn honesty_game_text_caption_ok(&self) -> bool {
        self.game_text_caption_ok
    }

    pub fn honesty_display_string_measure_ok(&self) -> bool {
        self.display_string_measure_ok
    }

    pub fn honesty_display_string_color_ok(&self) -> bool {
        self.display_string_color_ok
    }
}

/// Packed floating text layout payload ready for dual-tick UI consumers.
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingTextLayout {
    pub entries: Vec<FloatingTextLayoutEntry>,
    /// Interleaved f32 layout bytes (see `FloatingTextLayoutEntry`).
    pub layout_bytes: Vec<u8>,
    pub honesty: FloatingTextLayoutHonesty,
}

impl FloatingTextLayout {
    /// Empty pack — honest residual when no floating texts are active.
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            layout_bytes: Vec::new(),
            honesty: FloatingTextLayoutHonesty {
                cpu_pack_ok: true,
                game_text_caption_ok: true,
                display_string_measure_ok: true,
                display_string_color_ok: true,
                move_up_speed: PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED,
                vanish_rate: PRESENTATION_FLOATING_TEXT_VANISH_RATE,
                timeout_frames: PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES,
                ..Default::default()
            },
        }
    }

    pub fn mark_gpu_upload_ready(&mut self) {
        self.honesty.gpu_upload_ready = self.honesty.cpu_pack_ok;
    }

    /// Pack presentation floating texts at `logic_frame` into layout samples.
    pub fn pack_from_presentation(frame: &PresentationFrame) -> Self {
        Self::pack_texts_at(&frame.floating_texts, frame.frame.0, &frame.world_anims)
    }

    pub fn pack_texts_at(
        texts: &[PresentationFloatingText],
        logic_frame: u32,
        world_anims: &[PresentationWorldAnim],
    ) -> Self {
        if texts.is_empty() {
            let mut empty = Self::empty();
            empty.honesty.world_anims_observed = world_anims.len() as u32;
            return empty;
        }

        let mut entries = Vec::with_capacity(texts.len());
        let mut active = 0u32;
        let mut caption_ok = true;
        let mut measure_ok = true;
        for t in texts {
            let age = logic_frame.saturating_sub(t.spawn_frame);
            let timeout = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES;
            let lift = age as f32 * PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED;
            // C++ residual: while before timeout alpha stays full; after timeout
            // vanish rate pulls alpha toward 0 until erased.
            let alpha = if age < timeout {
                1.0
            } else {
                let past = (age - timeout) as f32;
                (1.0 - past * PRESENTATION_FLOATING_TEXT_VANISH_RATE).clamp(0.0, 1.0)
            };
            // Pack only non-vanished (alpha > 0) entries — erase residual.
            if alpha <= 0.0 {
                continue;
            }
            if age < timeout {
                active = active.saturating_add(1);
            }
            let caption = resolve_add_cash_caption(&t.text_key, t.amount, &t.text);
            if !honesty_add_cash_caption(&t.text_key, t.amount, &caption)
                && t.text_key == GUI_ADD_CASH_KEY
            {
                caption_ok = false;
            }
            if t.text_key != GUI_ADD_CASH_KEY {
                // Non-AddCash keys still pack; mark caption residual incomplete.
                caption_ok = caption_ok && !t.text_key.is_empty();
            }
            let (measure_width, measure_height) = measure_display_string_residual(&caption);
            if !honesty_display_string_measure(&caption, measure_width, measure_height) {
                measure_ok = false;
            }
            let c = t.color_rgba;
            entries.push(FloatingTextLayoutEntry {
                position: [t.position.x, t.position.y, t.position.z],
                lift_y: lift,
                color_rgba: normalize_display_string_color(c),
                alpha,
                amount: t.amount as f32,
                age_frames: age as f32,
                timeout_frames: timeout as f32,
                caption,
                text_key: t.text_key.clone(),
                measure_width,
                measure_height,
            });
        }

        let mut floats = Vec::with_capacity(entries.len() * FLOATING_TEXT_LAYOUT_FLOATS);
        for e in &entries {
            floats.extend_from_slice(&e.clone().to_floats());
        }
        let layout_bytes = f32_slice_to_bytes(&floats);
        let texts_packed = entries.len() as u32;
        // Empty of non-AddCash failures: when packing GUI:AddCash entries, require
        // residual caption format; empty list is honest success.
        let game_text_caption_ok = if texts_packed == 0 {
            true
        } else {
            caption_ok
                && entries
                    .iter()
                    .all(|e| honesty_add_cash_caption(&e.text_key, e.amount as u32, &e.caption))
        };
        let display_string_measure_ok = if texts_packed == 0 {
            true
        } else {
            measure_ok
                && entries.iter().all(|e| {
                    honesty_display_string_measure(&e.caption, e.measure_width, e.measure_height)
                        && e.measure_width > 0
                })
        };
        let display_string_color_ok = if texts_packed == 0 {
            true
        } else {
            entries.iter().all(|e| {
                e.color_rgba.iter().all(|c| *c >= 0.0 && *c <= 1.0) && e.color_rgba[3] > 0.0
            })
        };
        Self {
            honesty: FloatingTextLayoutHonesty {
                texts_packed,
                active_packed: active,
                world_anims_observed: world_anims.len() as u32,
                bytes_packed: layout_bytes.len() as u32,
                cpu_pack_ok: true,
                has_geometry: active > 0,
                gpu_upload_ready: false,
                move_up_speed: PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED,
                vanish_rate: PRESENTATION_FLOATING_TEXT_VANISH_RATE,
                timeout_frames: PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES,
                game_text_caption_ok,
                display_string_measure_ok,
                display_string_color_ok,
            },
            entries,
            layout_bytes,
        }
    }
}

fn f32_slice_to_bytes(floats: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(floats.len() * 4);
    for f in floats {
        out.extend_from_slice(&f.to_le_bytes());
    }
    out
}

/// Host-testable residual: pack + mark upload-ready without a live GPU device.
pub fn pack_floating_text_and_mark_ready(frame: &PresentationFrame) -> FloatingTextLayout {
    let mut pack = FloatingTextLayout::pack_from_presentation(frame);
    pack.mark_gpu_upload_ready();
    pack
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation_frame::{
        PresentationFloatingText, PresentationWorldAnim, PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES,
    };

    #[test]
    fn empty_pack_is_honest_cpu_success() {
        let pack = FloatingTextLayout::empty();
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(!pack.honesty.honesty_geometry_ok());
        assert!(pack.layout_bytes.is_empty());
        assert!(pack.honesty.honesty_retail_params_ok());
    }

    #[test]
    fn packs_synthetic_cash_with_move_up_and_timeout() {
        let ft = PresentationFloatingText::synthetic_cash(150, 0);
        let pack = FloatingTextLayout::pack_texts_at(
            &[ft],
            3,
            &[PresentationWorldAnim::synthetic_money_pickup(0)],
        );
        assert!(pack.honesty.honesty_cpu_pack_ok());
        assert!(pack.honesty.honesty_geometry_ok());
        assert!(pack.honesty.honesty_game_text_caption_ok());
        assert_eq!(pack.honesty.texts_packed, 1);
        assert_eq!(pack.honesty.active_packed, 1);
        assert_eq!(pack.honesty.world_anims_observed, 1);
        assert_eq!(pack.entries[0].lift_y, 3.0 * PRESENTATION_FLOATING_TEXT_MOVE_UP_SPEED);
        assert!((pack.entries[0].alpha - 1.0).abs() < 0.001);
        assert_eq!(pack.entries[0].caption, "+$150");
        assert_eq!(pack.entries[0].text_key, GUI_ADD_CASH_KEY);
        assert!(pack.honesty.honesty_display_string_measure_ok());
        // monospaced 8×8 residual: "+$150" = 5 glyphs → 40 px wide
        assert_eq!(pack.entries[0].measure_width, 5 * 8);
        assert_eq!(pack.entries[0].measure_height, 8);
        assert_eq!(
            pack.layout_bytes.len(),
            FLOATING_TEXT_LAYOUT_BYTES
        );
        let mut marked = pack;
        marked.mark_gpu_upload_ready();
        assert!(marked.honesty.honesty_upload_ready_ok());
    }

    #[test]
    fn resolve_add_cash_caption_residual() {
        assert_eq!(resolve_add_cash_caption(GUI_ADD_CASH_KEY, 200, ""), "+$200");
        assert!(honesty_add_cash_caption(GUI_ADD_CASH_KEY, 200, "+$200"));
    }

    #[test]
    fn vanish_phase_after_timeout_decays_alpha() {
        let ft = PresentationFloatingText::synthetic_cash(50, 0);
        let age = PRESENTATION_FLOATING_TEXT_TIMEOUT_FRAMES + 5;
        let pack = FloatingTextLayout::pack_texts_at(&[ft], age, &[]);
        assert_eq!(pack.honesty.active_packed, 0);
        assert_eq!(pack.honesty.texts_packed, 1);
        let expected = (1.0 - 5.0 * PRESENTATION_FLOATING_TEXT_VANISH_RATE).clamp(0.0, 1.0);
        assert!((pack.entries[0].alpha - expected).abs() < 0.001);
    }

    #[test]
    fn display_string_color_residual_normalize() {
        let green = normalize_display_string_color(FLOATING_TEXT_COLOR_GREEN_U8);
        assert!((green[1] - 1.0).abs() < 0.001);
        assert!((green[0] - 0.0).abs() < 0.001);
        assert!(honesty_display_string_color(green, FLOATING_TEXT_COLOR_GREEN_U8));
        let yellow = normalize_display_string_color(FLOATING_TEXT_COLOR_YELLOW_U8);
        assert!((yellow[0] - 1.0).abs() < 0.001 && (yellow[1] - 1.0).abs() < 0.001);
        assert!(honesty_display_string_color(yellow, FLOATING_TEXT_COLOR_YELLOW_U8));
        assert_eq!(FLOATING_TEXT_COLOR_GREEN_U8, (0, 255, 0, 255));
        assert_eq!(FLOATING_TEXT_COLOR_YELLOW_U8, (255, 255, 0, 255));
    }

    #[test]
    fn display_string_set_text_residual_honesty() {
        // Equal text → no change residual (C++ early return).
        assert!(!display_string_set_text_changed("+$150", "+$150"));
        assert!(honesty_display_string_set_text("+$150", "+$150", false));
        // Different text → notifyTextChanged residual.
        assert!(display_string_set_text_changed("+$150", "+$200"));
        assert!(honesty_display_string_set_text("+$150", "+$200", true));
        assert!(display_string_set_text_changed("", "+$0"));
        assert!(!display_string_set_text_changed("", ""));
    }

    #[test]
    fn display_string_set_font_residual_honesty() {
        // Equal font → no change residual (C++ early return).
        assert!(!display_string_set_font_changed("Arial", "Arial"));
        assert!(honesty_display_string_set_font("Arial", "Arial", false));
        // Different font → m_fontChanged residual.
        assert!(display_string_set_font_changed("Arial", "Times"));
        assert!(honesty_display_string_set_font("Arial", "Times", true));
        // Empty new font → fail-closed (C++ NULL font early return).
        assert!(!display_string_set_font_changed("Arial", ""));
        assert!(honesty_display_string_set_font("Arial", "", false));
    }

    #[test]
    fn display_string_get_text_length_residual_honesty() {
        assert_eq!(display_string_get_text_length("+$150"), 5);
        assert!(honesty_display_string_get_text_length("+$150", 5));
        assert_eq!(display_string_get_text_length(""), 0);
        assert!(honesty_display_string_get_text_length("", 0));
        assert_eq!(display_string_get_text_length("$200"), 4);
        assert!(honesty_display_string_get_text_length("$200", 4));
        // Measure residual width tracks glyph width × length.
        let (w, _) = crate::graphics::game_text_residual::measure_display_string_residual("+$150");
        assert_eq!(w, display_string_get_text_length("+$150") * 8);
    }

    #[test]
    fn display_string_get_text_and_reset_residual_honesty() {
        assert_eq!(display_string_get_text("+$150"), "+$150");
        assert!(honesty_display_string_get_text("+$150", "+$150"));
        assert_eq!(display_string_get_text(""), "");
        assert!(honesty_display_string_get_text("", ""));

        let (text, font, notified) = display_string_reset_residual();
        assert!(honesty_display_string_reset(&text, &font, notified));
        assert!(text.is_empty() && font.is_empty() && !notified);
    }

    #[test]
    fn display_string_append_remove_char_residual_honesty() {
        let (appended, notified) = display_string_append_char("+$15", '0');
        assert_eq!(appended, "+$150");
        assert!(notified);
        assert!(honesty_display_string_append_char("+$15", '0', "+$150", true));
        assert!(!honesty_display_string_append_char("+$15", '0', "+$15", true));

        let (removed, notified) = display_string_remove_last_char("+$150");
        assert_eq!(removed, "+$15");
        assert!(notified);
        assert!(honesty_display_string_remove_last_char("+$150", "+$15", true));

        // Empty remove still notifies (C++ call order residual).
        let (empty, notified) = display_string_remove_last_char("");
        assert!(empty.is_empty() && notified);
        assert!(honesty_display_string_remove_last_char("", "", true));
    }

    #[test]
    fn display_string_get_width_residual_honesty() {
        // Full string monospaced residual.
        assert_eq!(display_string_get_width("+$150", -1), 5 * 8);
        assert!(honesty_display_string_get_width("+$150", -1, 40));
        // Partial charPos residual.
        assert_eq!(display_string_get_width("+$150", 2), 2 * 8);
        assert!(honesty_display_string_get_width("+$150", 2, 16));
        // Newline does not contribute width residual.
        assert_eq!(display_string_get_width("A\nB", -1), 2 * 8);
        assert!(honesty_display_string_get_width("A\nB", -1, 16));
        assert_eq!(display_string_get_width("", -1), 0);
        assert!(honesty_display_string_get_width("", 0, 0));
        // Full width matches measure residual for ASCII captions (no newlines).
        let (mw, _) = crate::graphics::game_text_residual::measure_display_string_residual("+$150");
        assert_eq!(mw, display_string_get_width("+$150", -1));
    }

    #[test]
    fn display_string_get_size_residual_honesty() {
        // Empty text or no font → (0,0) residual (C++ computeExtents early-out).
        assert_eq!(display_string_get_size("", true), (0, 0));
        assert_eq!(display_string_get_size("+$150", false), (0, 0));
        assert!(honesty_display_string_get_size("", true, 0, 0));
        assert!(honesty_display_string_get_size("+$150", false, 0, 0));
        // Monospaced single-line residual.
        assert_eq!(display_string_get_size("+$150", true), (40, 8));
        assert!(honesty_display_string_get_size("+$150", true, 40, 8));
        // Multi-line residual height.
        assert_eq!(display_string_get_size("A\nB", true), (16, 16));
        assert!(honesty_display_string_get_size("A\nB", true, 16, 16));
        assert_eq!(DISPLAY_STRING_GLYPH_HEIGHT, 8);
    }

    #[test]
    fn display_string_set_word_wrap_residual_honesty() {
        let (w, notified) = display_string_set_word_wrap(0, 200);
        assert_eq!(w, 200);
        assert!(notified);
        assert!(honesty_display_string_set_word_wrap(0, 200, 200, true));
        // Equal width → no notify residual.
        let (w, notified) = display_string_set_word_wrap(200, 200);
        assert_eq!(w, 200);
        assert!(!notified);
        assert!(honesty_display_string_set_word_wrap(200, 200, 200, false));

        let (c, notified) = display_string_set_word_wrap_centered(false, true);
        assert!(c && notified);
        assert!(honesty_display_string_set_word_wrap_centered(false, true, true, true));
        let (c, notified) = display_string_set_word_wrap_centered(true, true);
        assert!(c && !notified);
        assert!(honesty_display_string_set_word_wrap_centered(true, true, true, false));
    }

    #[test]
    fn display_string_set_use_hotkey_and_clip_residual_honesty() {
        let (use_hk, color, notified) =
            display_string_set_use_hotkey(true, DISPLAY_STRING_HOTKEY_COLOR_DEFAULT);
        assert!(use_hk && notified);
        assert_eq!(color, (255, 255, 255, 255));
        assert!(honesty_display_string_set_use_hotkey(
            true,
            DISPLAY_STRING_HOTKEY_COLOR_DEFAULT,
            true,
            (255, 255, 255, 255),
            true
        ));
        // Always notifies even when flag already true residual.
        let (use_hk, _, notified) =
            display_string_set_use_hotkey(false, (255, 0, 0, 255));
        assert!(!use_hk && notified);

        let prev = (0, 0, 0, 0);
        let next = (10, 20, 100, 80);
        let (region, changed) = display_string_set_clip_region(prev, next);
        assert_eq!(region, next);
        assert!(changed);
        assert!(honesty_display_string_set_clip_region(prev, next, next, true));
        // Equal region → early-out residual.
        let (region, changed) = display_string_set_clip_region(next, next);
        assert_eq!(region, next);
        assert!(!changed);
        assert!(honesty_display_string_set_clip_region(next, next, next, false));
    }

    #[test]
    fn display_string_get_font_residual_honesty() {
        assert_eq!(display_string_get_font("Arial"), "Arial");
        assert_eq!(display_string_get_font(""), "");
        assert!(honesty_display_string_get_font("FixedSys", "FixedSys"));
        assert!(!honesty_display_string_get_font("Arial", "Other"));
    }

    #[test]
    fn display_string_draw_residual_honesty() {
        let empty = display_string_draw(
            "", 10, 20, (255, 255, 255, 255), (0, 0, 0, 255),
            0, 0, (0, 0, 0, 0), (0, 0, 0, 0), true,
        );
        assert!(!empty.drew);
        assert!(!empty.rebuilt);
        assert_eq!(empty.x_drop, 1);
        assert_eq!(empty.y_drop, 1);
        assert!(honesty_display_string_draw(
            "", 10, 20, (255, 255, 255, 255), (0, 0, 0, 255),
            0, 0, (0, 0, 0, 0), (0, 0, 0, 0), true, empty,
        ));

        let same = display_string_draw(
            "+$100", 10, 20, (0, 255, 0, 255), (0, 0, 0, 255),
            10, 20, (0, 255, 0, 255), (0, 0, 0, 255), false,
        );
        assert!(same.drew);
        assert!(!same.rebuilt);

        let moved = display_string_draw(
            "+$100", 12, 20, (0, 255, 0, 255), (0, 0, 0, 255),
            10, 20, (0, 255, 0, 255), (0, 0, 0, 255), false,
        );
        assert!(moved.drew && moved.rebuilt);

        let dirty = display_string_draw(
            "+$100", 10, 20, (0, 255, 0, 255), (0, 0, 0, 255),
            10, 20, (0, 255, 0, 255), (0, 0, 0, 255), true,
        );
        assert!(dirty.drew && dirty.rebuilt);

        let drop = display_string_draw_with_drop(
            "X", 0, 0, (255, 255, 0, 255), (0, 0, 0, 128),
            2, 3, 0, 0, (255, 255, 0, 255), (0, 0, 0, 128), false,
        );
        assert!(drop.drew);
        assert!(!drop.rebuilt);
        assert_eq!(drop.x_drop, 2);
        assert_eq!(drop.y_drop, 3);
        // Shadow-then-text order residual: shadow at (x+xDrop, y+yDrop).
        assert_eq!(drop.shadow_x, 2);
        assert_eq!(drop.shadow_y, 3);
        assert_eq!(same.shadow_x, 11); // 10+1 default drop
        assert_eq!(same.shadow_y, 21);
    }

}
