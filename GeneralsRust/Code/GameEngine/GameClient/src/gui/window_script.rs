//! Window script parser for `.wnd` files.
//!
//! This module parses the legacy Generals GUI window scripts into in-memory
//! window definitions for creation by the WindowManager.

use super::game_window::{
    GameFont, Image, WindowDrawData, WindowStatus, WindowTextColors, GWS_ANIMATED, GWS_CHECK_BOX,
    GWS_COMBO_BOX, GWS_ENTRY_FIELD, GWS_HORZ_SLIDER, GWS_MOUSE_TRACK, GWS_PROGRESS_BAR,
    GWS_PUSH_BUTTON, GWS_RADIO_BUTTON, GWS_SCROLL_LISTBOX, GWS_STATIC_TEXT, GWS_TAB_CONTROL,
    GWS_TAB_PANE, GWS_TAB_STOP, GWS_USER_WINDOW, GWS_VERT_SLIDER,
};
use super::MAX_DRAW_DATA;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Default)]
pub struct WindowLayoutDefinition {
    pub version: u32,
    pub init_callback: String,
    pub update_callback: String,
    pub shutdown_callback: String,
    pub default_text_color: Option<u32>,
    pub default_font: Option<GameFont>,
    pub windows: Vec<WindowDefinition>,
    pub listbox_enabled_up_button_draw_data: Vec<WindowDrawData>,
    pub listbox_disabled_up_button_draw_data: Vec<WindowDrawData>,
    pub listbox_hilite_up_button_draw_data: Vec<WindowDrawData>,
    pub listbox_enabled_down_button_draw_data: Vec<WindowDrawData>,
    pub listbox_disabled_down_button_draw_data: Vec<WindowDrawData>,
    pub listbox_hilite_down_button_draw_data: Vec<WindowDrawData>,
    pub listbox_enabled_slider_draw_data: Vec<WindowDrawData>,
    pub listbox_disabled_slider_draw_data: Vec<WindowDrawData>,
    pub listbox_hilite_slider_draw_data: Vec<WindowDrawData>,
    pub slider_thumb_enabled_draw_data: Vec<WindowDrawData>,
    pub slider_thumb_disabled_draw_data: Vec<WindowDrawData>,
    pub slider_thumb_hilite_draw_data: Vec<WindowDrawData>,
    pub combo_dropdown_enabled_draw_data: Vec<WindowDrawData>,
    pub combo_dropdown_disabled_draw_data: Vec<WindowDrawData>,
    pub combo_dropdown_hilite_draw_data: Vec<WindowDrawData>,
    pub combo_edit_enabled_draw_data: Vec<WindowDrawData>,
    pub combo_edit_disabled_draw_data: Vec<WindowDrawData>,
    pub combo_edit_hilite_draw_data: Vec<WindowDrawData>,
    pub combo_list_enabled_draw_data: Vec<WindowDrawData>,
    pub combo_list_disabled_draw_data: Vec<WindowDrawData>,
    pub combo_list_hilite_draw_data: Vec<WindowDrawData>,
}

#[derive(Debug, Default)]
pub struct WindowDefinition {
    pub name: String,
    pub window_type: String,
    pub status: WindowStatus,
    pub style: u32,
    pub position: (i32, i32),
    pub size: (i32, i32),
    pub raw_screen_rect: Option<(i32, i32, i32, i32)>,
    pub creation_resolution: Option<(i32, i32)>,
    pub listbox_data: Option<ListBoxData>,
    pub text_entry_data: Option<TextEntryData>,
    pub combo_box_data: Option<ComboBoxData>,
    pub tab_control_data: Option<TabControlData>,
    pub slider_data: Option<SliderData>,
    pub radio_button_data: Option<RadioButtonData>,
    pub static_text_data: Option<StaticTextData>,
    pub image_offset: (i32, i32),
    pub system_callback: String,
    pub input_callback: String,
    pub tooltip_callback: String,
    pub draw_callback: String,
    pub font: Option<GameFont>,
    pub header_template: String,
    pub tooltip_delay: i32,
    pub text: String,
    pub text_label: String,
    pub tooltip: String,
    pub enabled_text: WindowTextColors,
    pub disabled_text: WindowTextColors,
    pub hilite_text: WindowTextColors,
    pub enabled_draw_data: Vec<WindowDrawData>,
    pub disabled_draw_data: Vec<WindowDrawData>,
    pub hilite_draw_data: Vec<WindowDrawData>,
    pub children: Vec<WindowDefinition>,
}

#[derive(Debug, Default, Clone)]
pub struct ListBoxData {
    pub length: usize,
    pub autoscroll: bool,
    pub scroll_if_at_end: bool,
    pub autopurge: bool,
    pub scrollbar: bool,
    pub multiselect: bool,
    pub columns: u32,
    pub column_widths: Vec<u32>,
    pub force_select: bool,
}

#[derive(Debug, Default, Clone)]
pub struct TextEntryData {
    pub max_len: usize,
    pub secret_text: bool,
    pub numerical_only: bool,
    pub alphanumerical_only: bool,
    pub ascii_only: bool,
}

#[derive(Debug, Default, Clone)]
pub struct ComboBoxData {
    pub is_editable: bool,
    pub max_chars: usize,
    pub max_display: usize,
    pub ascii_only: bool,
    pub letters_and_numbers: bool,
}

#[derive(Debug, Default, Clone)]
pub struct TabControlData {
    pub tab_orientation: i32,
    pub tab_edge: i32,
    pub tab_width: i32,
    pub tab_height: i32,
    pub tab_count: i32,
    pub pane_border: i32,
    pub sub_pane_disabled: [bool; 8],
}

#[derive(Debug, Default, Clone)]
pub struct SliderData {
    pub min_value: i32,
    pub max_value: i32,
    pub num_ticks: f32,
    pub position: i32,
}

#[derive(Debug, Default, Clone)]
pub struct RadioButtonData {
    pub group: u32,
}

#[derive(Debug, Default, Clone)]
pub struct StaticTextData {
    pub centered: bool,
    pub centered_vertically: bool,
    pub left_margin: u32,
    pub top_margin: u32,
}

#[derive(Debug)]
pub enum WindowScriptError {
    Io(std::io::Error),
    Parse(String),
}

impl From<std::io::Error> for WindowScriptError {
    fn from(err: std::io::Error) -> Self {
        WindowScriptError::Io(err)
    }
}

pub fn parse_window_script(path: &Path) -> Result<WindowLayoutDefinition, WindowScriptError> {
    let content = fs::read_to_string(path)?;
    let statements = split_statements(&content);
    let mut layout = WindowLayoutDefinition::default();
    let mut window_stack: Vec<WindowDefinition> = Vec::new();
    let mut in_layout_block = false;

    for statement in statements {
        let stmt = statement.trim();
        if stmt.is_empty() {
            continue;
        }
        match stmt {
            "STARTLAYOUTBLOCK" => {
                in_layout_block = true;
                continue;
            }
            "ENDLAYOUTBLOCK" => {
                in_layout_block = false;
                continue;
            }
            "WINDOW" => {
                window_stack.push(WindowDefinition::default());
                continue;
            }
            "CHILD" => {
                continue;
            }
            "ENDALLCHILDREN" => {
                continue;
            }
            "END" => {
                let window = window_stack
                    .pop()
                    .ok_or_else(|| WindowScriptError::Parse("Unexpected END".to_string()))?;
                if let Some(parent) = window_stack.last_mut() {
                    parent.children.push(window);
                } else {
                    layout.windows.push(window);
                }
                continue;
            }
            _ => {}
        }

        if stmt.starts_with("FILE_VERSION") {
            parse_layout_statement(stmt, &mut layout)?;
        } else if in_layout_block {
            parse_layout_statement(stmt, &mut layout)?;
        } else if let Some(current) = window_stack.last_mut() {
            parse_window_statement(stmt, current)?;
        } else {
            parse_layout_statement(stmt, &mut layout)?;
        }
    }

    Ok(layout)
}

fn split_statements(content: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut buffer = String::new();

    for raw_line in content.lines() {
        let line = raw_line.split("//").next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if matches!(
            line,
            "WINDOW" | "END" | "CHILD" | "ENDALLCHILDREN" | "STARTLAYOUTBLOCK" | "ENDLAYOUTBLOCK"
        ) {
            if !buffer.trim().is_empty() {
                statements.push(buffer.trim().to_string());
                buffer.clear();
            }
            statements.push(line.to_string());
            continue;
        }
        if !buffer.is_empty() {
            buffer.push(' ');
        }
        buffer.push_str(line);
        if line.ends_with(';') {
            statements.push(buffer.trim().to_string());
            buffer.clear();
        }
    }

    if !buffer.trim().is_empty() {
        statements.push(buffer.trim().to_string());
    }

    statements
}

fn parse_layout_statement(
    stmt: &str,
    layout: &mut WindowLayoutDefinition,
) -> Result<(), WindowScriptError> {
    let (key, value) = split_key_value(stmt)?;
    let val = strip_wrapped_value(value);
    match key {
        "FILE_VERSION" => {
            layout.version = val.parse::<u32>().unwrap_or(0);
        }
        "LAYOUTINIT" => layout.init_callback = normalize_callback_name(&val),
        "LAYOUTUPDATE" => layout.update_callback = normalize_callback_name(&val),
        "LAYOUTSHUTDOWN" => layout.shutdown_callback = normalize_callback_name(&val),
        "TEXTCOLOR" => {
            if let Some(color) = parse_default_color(&val) {
                layout.default_text_color = Some(color);
            }
        }
        "FONT" => layout.default_font = parse_font(&val),
        "LISTBOXENABLEDUPBUTTONDRAWDATA" => {
            layout.listbox_enabled_up_button_draw_data = parse_draw_data(&val);
        }
        "LISTBOXDISABLEDUPBUTTONDRAWDATA" => {
            layout.listbox_disabled_up_button_draw_data = parse_draw_data(&val);
        }
        "LISTBOXHILITEUPBUTTONDRAWDATA" => {
            layout.listbox_hilite_up_button_draw_data = parse_draw_data(&val);
        }
        "LISTBOXENABLEDDOWNBUTTONDRAWDATA" => {
            layout.listbox_enabled_down_button_draw_data = parse_draw_data(&val);
        }
        "LISTBOXDISABLEDDOWNBUTTONDRAWDATA" => {
            layout.listbox_disabled_down_button_draw_data = parse_draw_data(&val);
        }
        "LISTBOXHILITEDOWNBUTTONDRAWDATA" => {
            layout.listbox_hilite_down_button_draw_data = parse_draw_data(&val);
        }
        "LISTBOXENABLEDSLIDERDRAWDATA" => {
            layout.listbox_enabled_slider_draw_data = parse_draw_data(&val);
        }
        "LISTBOXDISABLEDSLIDERDRAWDATA" => {
            layout.listbox_disabled_slider_draw_data = parse_draw_data(&val);
        }
        "LISTBOXHILITESLIDERDRAWDATA" => {
            layout.listbox_hilite_slider_draw_data = parse_draw_data(&val);
        }
        "SLIDERTHUMBENABLEDDRAWDATA" => {
            layout.slider_thumb_enabled_draw_data = parse_draw_data(&val);
        }
        "SLIDERTHUMBDISABLEDDRAWDATA" => {
            layout.slider_thumb_disabled_draw_data = parse_draw_data(&val);
        }
        "SLIDERTHUMBHILITEDRAWDATA" => {
            layout.slider_thumb_hilite_draw_data = parse_draw_data(&val);
        }
        "COMBOBOXDROPDOWNBUTTONENABLEDDRAWDATA" => {
            layout.combo_dropdown_enabled_draw_data = parse_draw_data(&val);
        }
        "COMBOBOXDROPDOWNBUTTONDISABLEDDRAWDATA" => {
            layout.combo_dropdown_disabled_draw_data = parse_draw_data(&val);
        }
        "COMBOBOXDROPDOWNBUTTONHILITEDRAWDATA" => {
            layout.combo_dropdown_hilite_draw_data = parse_draw_data(&val);
        }
        "COMBOBOXEDITBOXENABLEDDRAWDATA" => {
            layout.combo_edit_enabled_draw_data = parse_draw_data(&val);
        }
        "COMBOBOXEDITBOXDISABLEDDRAWDATA" => {
            layout.combo_edit_disabled_draw_data = parse_draw_data(&val);
        }
        "COMBOBOXEDITBOXHILITEDRAWDATA" => {
            layout.combo_edit_hilite_draw_data = parse_draw_data(&val);
        }
        "COMBOBOXLISTBOXENABLEDDRAWDATA" => {
            layout.combo_list_enabled_draw_data = parse_draw_data(&val);
        }
        "COMBOBOXLISTBOXDISABLEDDRAWDATA" => {
            layout.combo_list_disabled_draw_data = parse_draw_data(&val);
        }
        "COMBOBOXLISTBOXHILITEDRAWDATA" => {
            layout.combo_list_hilite_draw_data = parse_draw_data(&val);
        }
        _ => {}
    }
    Ok(())
}

fn parse_window_statement(
    stmt: &str,
    window: &mut WindowDefinition,
) -> Result<(), WindowScriptError> {
    let (key, value) = split_key_value(stmt)?;
    let val = value.trim();
    match key {
        "WINDOWTYPE" => window.window_type = strip_wrapped_value(val),
        "SCREENRECT" => parse_screen_rect(val, window),
        "NAME" => window.name = strip_wrapped_value(val),
        "STATUS" => window.status = parse_window_status(val),
        "STYLE" => window.style = parse_window_style(val),
        "SYSTEMCALLBACK" => window.system_callback = normalize_callback_name(val),
        "INPUTCALLBACK" => window.input_callback = normalize_callback_name(val),
        "TOOLTIPCALLBACK" => window.tooltip_callback = normalize_callback_name(val),
        "DRAWCALLBACK" => window.draw_callback = normalize_callback_name(val),
        "FONT" => window.font = parse_font(val),
        "HEADERTEMPLATE" => window.header_template = strip_wrapped_value(val),
        "TOOLTIPDELAY" => {
            window.tooltip_delay = val.trim_end_matches(';').trim().parse().unwrap_or(0)
        }
        "TEXT" => window.text = strip_wrapped_value(val),
        "TEXTLABEL" => window.text_label = strip_wrapped_value(val),
        "TOOLTIP" | "TOOLTIPTEXT" => window.tooltip = strip_wrapped_value(val),
        "TEXTCOLOR" => parse_text_colors(val, window),
        "ENABLEDDRAWDATA" => window.enabled_draw_data = parse_draw_data(val),
        "DISABLEDDRAWDATA" => window.disabled_draw_data = parse_draw_data(val),
        "HILITEDRAWDATA" => window.hilite_draw_data = parse_draw_data(val),
        "LISTBOXDATA" => window.listbox_data = Some(parse_listbox_data(val)),
        "TEXTENTRYDATA" => window.text_entry_data = Some(parse_text_entry_data(val)),
        "COMBOBOXDATA" => window.combo_box_data = Some(parse_combo_box_data(val)),
        "TABCONTROLDATA" => window.tab_control_data = Some(parse_tab_control_data(val)),
        "SLIDERDATA" => window.slider_data = Some(parse_slider_data(val)),
        "RADIOBUTTONDATA" => window.radio_button_data = Some(parse_radio_button_data(val)),
        "STATICTEXTDATA" => window.static_text_data = Some(parse_static_text_data(val)),
        "IMAGEOFFSET" => {
            let nums = extract_numbers(val);
            if nums.len() >= 2 {
                window.image_offset = (nums[0], nums[1]);
            }
        }
        _ => {}
    }
    Ok(())
}

fn split_key_value(stmt: &str) -> Result<(&str, &str), WindowScriptError> {
    let mut parts = stmt.splitn(2, '=');
    let key = parts.next().unwrap_or("").trim();
    let value = parts
        .next()
        .ok_or_else(|| WindowScriptError::Parse(format!("Invalid statement: {}", stmt)))?
        .trim();
    Ok((key, value.trim_end_matches(';').trim()))
}

fn strip_wrapped_value(value: &str) -> String {
    let mut val = value.trim().trim_end_matches(';').trim().to_string();
    if val.starts_with('[') && val.ends_with(']') {
        val = val
            .trim_start_matches('[')
            .trim_end_matches(']')
            .trim()
            .to_string();
    }
    if val.starts_with('"') && val.ends_with('"') {
        val = val.trim_matches('"').to_string();
    }
    val
}

fn normalize_callback_name(value: &str) -> String {
    let name = strip_wrapped_value(value);
    if name.eq_ignore_ascii_case("none") || name.eq_ignore_ascii_case("[none]") {
        String::new()
    } else {
        name
    }
}

fn parse_screen_rect(value: &str, window: &mut WindowDefinition) {
    let numbers = extract_numbers(value);
    if numbers.len() >= 4 {
        let upper_left = (numbers[0], numbers[1]);
        let bottom_right = (numbers[2], numbers[3]);
        window.raw_screen_rect = Some((upper_left.0, upper_left.1, bottom_right.0, bottom_right.1));
        window.position = upper_left;
        window.size = (bottom_right.0 - upper_left.0, bottom_right.1 - upper_left.1);
    }
    if numbers.len() >= 6 {
        window.creation_resolution = Some((numbers[4], numbers[5]));
    }
}

fn parse_window_status(value: &str) -> WindowStatus {
    let val = strip_wrapped_value(value);
    let mut status = WindowStatus::NONE;
    for token in val.split('+') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        status |= match trimmed {
            "ACTIVE" => WindowStatus::ACTIVE,
            "TOGGLE" => WindowStatus::TOGGLE,
            "DRAGABLE" => WindowStatus::DRAGABLE,
            "ENABLED" => WindowStatus::ENABLED,
            "HIDDEN" => WindowStatus::HIDDEN,
            "ABOVE" => WindowStatus::ABOVE,
            "BELOW" => WindowStatus::BELOW,
            "IMAGE" => WindowStatus::IMAGE,
            "TABSTOP" => WindowStatus::TAB_STOP,
            "NOINPUT" => WindowStatus::NO_INPUT,
            "NOFOCUS" => WindowStatus::NO_FOCUS,
            "DESTROYED" => WindowStatus::DESTROYED,
            "BORDER" => WindowStatus::BORDER,
            "SMOOTH_TEXT" => WindowStatus::SMOOTH_TEXT,
            "ONE_LINE" => WindowStatus::ONE_LINE,
            "NO_FLUSH" => WindowStatus::NO_FLUSH,
            "SEE_THRU" => WindowStatus::SEE_THRU,
            "RIGHT_CLICK" => WindowStatus::RIGHT_CLICK,
            "WRAP_CENTERED" => WindowStatus::WRAP_CENTERED,
            "CHECK_LIKE" => WindowStatus::CHECK_LIKE,
            "HOTKEY_TEXT" => WindowStatus::HOTKEY_TEXT,
            "USE_OVERLAY_STATES" => WindowStatus::USE_OVERLAY_STATES,
            "NOT_READY" => WindowStatus::NOT_READY,
            "FLASHING" => WindowStatus::FLASHING,
            "ALWAYS_COLOR" => WindowStatus::ALWAYS_COLOR,
            "ON_MOUSE_DOWN" => WindowStatus::ON_MOUSE_DOWN,
            "SHORTCUT_BUTTON" => WindowStatus::SHORTCUT_BUTTON,
            _ => WindowStatus::NONE,
        };
    }
    status
}

fn parse_window_style(value: &str) -> u32 {
    let val = strip_wrapped_value(value);
    let mut style: u32 = 0;
    for token in val.split('+') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        style |= match trimmed.to_ascii_uppercase().as_str() {
            "PUSHBUTTON" => GWS_PUSH_BUTTON,
            "RADIOBUTTON" => GWS_RADIO_BUTTON,
            "CHECKBOX" => GWS_CHECK_BOX,
            "VERTSLIDER" => GWS_VERT_SLIDER,
            "HORZSLIDER" => GWS_HORZ_SLIDER,
            "SCROLLLISTBOX" => GWS_SCROLL_LISTBOX,
            "ENTRYFIELD" => GWS_ENTRY_FIELD,
            "STATICTEXT" => GWS_STATIC_TEXT,
            "PROGRESSBAR" => GWS_PROGRESS_BAR,
            "USER" => GWS_USER_WINDOW,
            "MOUSETRACK" => GWS_MOUSE_TRACK,
            "ANIMATED" => GWS_ANIMATED,
            "TABSTOP" => GWS_TAB_STOP,
            "TABCONTROL" => GWS_TAB_CONTROL,
            "TABPANE" => GWS_TAB_PANE,
            "COMBOBOX" => GWS_COMBO_BOX,
            _ => 0,
        };
    }
    style
}

fn parse_font(value: &str) -> Option<GameFont> {
    let mut name: Option<String> = None;
    let mut size: Option<i32> = None;
    let mut bold: Option<bool> = None;

    for segment in value.split(',') {
        let segment = segment.trim();
        if segment.starts_with("NAME:") {
            let raw = segment.trim_start_matches("NAME:").trim();
            let raw = strip_wrapped_value(raw);
            if !raw.is_empty() {
                name = Some(raw);
            }
        } else if segment.starts_with("SIZE:") {
            let raw = segment.trim_start_matches("SIZE:").trim();
            size = raw.parse::<i32>().ok();
        } else if segment.starts_with("BOLD:") {
            let raw = segment.trim_start_matches("BOLD:").trim();
            bold = Some(raw == "1");
        }
    }

    match (name, size) {
        (Some(name), Some(size)) => Some(GameFont {
            name,
            size,
            bold: bold.unwrap_or(false),
        }),
        _ => None,
    }
}

fn parse_default_color(value: &str) -> Option<u32> {
    let cleaned = strip_wrapped_value(value);
    let cleaned = cleaned.trim_end_matches(';').trim();
    if cleaned.is_empty() {
        return None;
    }
    let mut components = Vec::new();
    for part in cleaned.split(|c: char| c.is_whitespace() || c == ',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Ok(value) = part.parse::<u8>() {
            components.push(value);
        }
    }
    match components.as_slice() {
        [r, g, b, a] => Some(pack_color([*r, *g, *b, *a])),
        [r, g, b] => Some(pack_color([*r, *g, *b, 255])),
        _ => None,
    }
}

fn parse_text_colors(value: &str, window: &mut WindowDefinition) {
    let mut colors = HashMap::new();
    for segment in value.split(',') {
        let segment = segment.trim();
        if let Some((label, rgba)) = parse_color_segment(segment) {
            colors.insert(label, rgba);
        }
    }

    if let Some(rgba) = colors.get("ENABLED") {
        window.enabled_text.color = pack_color(*rgba);
    }
    if let Some(rgba) = colors.get("ENABLEDBORDER") {
        window.enabled_text.border_color = pack_color(*rgba);
    }
    if let Some(rgba) = colors.get("DISABLED") {
        window.disabled_text.color = pack_color(*rgba);
    }
    if let Some(rgba) = colors.get("DISABLEDBORDER") {
        window.disabled_text.border_color = pack_color(*rgba);
    }
    if let Some(rgba) = colors.get("HILITE") {
        window.hilite_text.color = pack_color(*rgba);
    }
    if let Some(rgba) = colors.get("HILITEBORDER") {
        window.hilite_text.border_color = pack_color(*rgba);
    }
}

fn parse_draw_data(value: &str) -> Vec<WindowDrawData> {
    let mut data = Vec::with_capacity(MAX_DRAW_DATA);
    let mut current = WindowDrawData::default();

    for segment in value.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        if segment.starts_with("IMAGE:") {
            let name = strip_wrapped_value(segment.trim_start_matches("IMAGE:").trim());
            current.image = if name.is_empty() || name.eq_ignore_ascii_case("NoImage") {
                None
            } else {
                Some(Image {
                    name,
                    width: 0,
                    height: 0,
                })
            };
        } else if segment.starts_with("COLOR:") {
            if let Some((_, rgba)) = parse_color_segment(segment) {
                current.color = pack_color(rgba);
            }
        } else if segment.starts_with("BORDERCOLOR:") {
            if let Some((_, rgba)) = parse_color_segment(segment) {
                current.border_color = pack_color(rgba);
            }
            if data.len() < MAX_DRAW_DATA {
                data.push(current.clone());
            }
            current = WindowDrawData::default();
        }
    }

    if data.len() < MAX_DRAW_DATA {
        data.resize_with(MAX_DRAW_DATA, WindowDrawData::default);
    }

    data
}

fn parse_listbox_data(value: &str) -> ListBoxData {
    let mut data = ListBoxData::default();
    let tokens: Vec<String> = value
        .split(|ch: char| ch.is_whitespace() || ch == ':' || ch == ',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    let mut idx = 0;
    while idx + 1 <= tokens.len() {
        let key = tokens[idx].to_ascii_uppercase();
        idx += 1;
        if idx >= tokens.len() {
            break;
        }
        let value = &tokens[idx];
        idx += 1;
        match key.as_str() {
            "LENGTH" => data.length = parse_usize(value).unwrap_or(0),
            "AUTOSCROLL" => data.autoscroll = parse_bool(value).unwrap_or(false),
            "SCROLLIFATEND" => data.scroll_if_at_end = parse_bool(value).unwrap_or(false),
            "AUTOPURGE" => data.autopurge = parse_bool(value).unwrap_or(false),
            "SCROLLBAR" => data.scrollbar = parse_bool(value).unwrap_or(false),
            "MULTISELECT" => data.multiselect = parse_bool(value).unwrap_or(false),
            "COLUMNS" => data.columns = parse_u32(value).unwrap_or(1),
            "COLUMNSWIDTH" | "COLUMNSWIDTH%" => {
                if let Some(width) = parse_u32(value) {
                    data.column_widths.push(width);
                }
            }
            "FORCESELECT" => data.force_select = parse_bool(value).unwrap_or(false),
            _ => {}
        }
    }

    if data.columns == 0 {
        data.columns = 1;
    }

    data
}

fn parse_text_entry_data(value: &str) -> TextEntryData {
    let map = parse_kv_map(value);
    TextEntryData {
        max_len: map.get("MAXLEN").and_then(|v| parse_usize(v)).unwrap_or(0),
        secret_text: map
            .get("SECRETTEXT")
            .and_then(|v| parse_bool(v))
            .unwrap_or(false),
        numerical_only: map
            .get("NUMERICALONLY")
            .and_then(|v| parse_bool(v))
            .unwrap_or(false),
        alphanumerical_only: map
            .get("ALPHANUMERICALONLY")
            .and_then(|v| parse_bool(v))
            .unwrap_or(false),
        ascii_only: map
            .get("ASCIIONLY")
            .and_then(|v| parse_bool(v))
            .unwrap_or(false),
    }
}

fn parse_combo_box_data(value: &str) -> ComboBoxData {
    let map = parse_kv_map(value);
    ComboBoxData {
        is_editable: map
            .get("ISEDITABLE")
            .and_then(|v| parse_bool(v))
            .unwrap_or(false),
        max_chars: map
            .get("MAXCHARS")
            .and_then(|v| parse_usize(v))
            .unwrap_or(0),
        max_display: map
            .get("MAXDISPLAY")
            .and_then(|v| parse_usize(v))
            .unwrap_or(0),
        ascii_only: map
            .get("ASCIIONLY")
            .and_then(|v| parse_bool(v))
            .unwrap_or(false),
        letters_and_numbers: map
            .get("LETTERSANDNUMBERS")
            .and_then(|v| parse_bool(v))
            .unwrap_or(false),
    }
}

fn parse_tab_control_data(value: &str) -> TabControlData {
    let mut data = TabControlData::default();
    let tokens = tokenize_tab_control_data(value);
    let mut idx = 0;
    while idx < tokens.len() {
        let key = tokens[idx].to_ascii_uppercase();
        idx += 1;
        match key.as_str() {
            "TABORIENTATION" => {
                if let Some(v) = tokens.get(idx).and_then(|v| parse_i32(v)) {
                    data.tab_orientation = v;
                }
                idx += 1;
            }
            "TABEDGE" => {
                if let Some(v) = tokens.get(idx).and_then(|v| parse_i32(v)) {
                    data.tab_edge = v;
                }
                idx += 1;
            }
            "TABWIDTH" => {
                if let Some(v) = tokens.get(idx).and_then(|v| parse_i32(v)) {
                    data.tab_width = v;
                }
                idx += 1;
            }
            "TABHEIGHT" => {
                if let Some(v) = tokens.get(idx).and_then(|v| parse_i32(v)) {
                    data.tab_height = v;
                }
                idx += 1;
            }
            "TABCOUNT" => {
                if let Some(v) = tokens.get(idx).and_then(|v| parse_i32(v)) {
                    data.tab_count = v;
                }
                idx += 1;
            }
            "PANEBORDER" => {
                if let Some(v) = tokens.get(idx).and_then(|v| parse_i32(v)) {
                    data.pane_border = v;
                }
                idx += 1;
            }
            "PANEDISABLED" => {
                let count = tokens.get(idx).and_then(|v| parse_i32(v)).unwrap_or(0);
                idx += 1;
                for pane_index in 0..count.max(0) as usize {
                    if pane_index >= data.sub_pane_disabled.len() {
                        break;
                    }
                    if let Some(flag) = tokens.get(idx).and_then(|v| parse_bool(v)) {
                        data.sub_pane_disabled[pane_index] = flag;
                    }
                    idx += 1;
                }
            }
            _ => {}
        }
    }
    data
}

fn parse_slider_data(value: &str) -> SliderData {
    let map = parse_kv_map(value);
    SliderData {
        min_value: map.get("MINVALUE").and_then(|v| parse_i32(v)).unwrap_or(0),
        max_value: map.get("MAXVALUE").and_then(|v| parse_i32(v)).unwrap_or(0),
        num_ticks: map
            .get("NUMTICKS")
            .and_then(|v| parse_f32(v))
            .unwrap_or(0.0),
        position: map.get("POSITION").and_then(|v| parse_i32(v)).unwrap_or(0),
    }
}

fn parse_radio_button_data(value: &str) -> RadioButtonData {
    let map = parse_kv_map(value);
    RadioButtonData {
        group: map.get("GROUP").and_then(|v| parse_u32(v)).unwrap_or(0),
    }
}

fn parse_static_text_data(value: &str) -> StaticTextData {
    let map = parse_kv_map(value);
    StaticTextData {
        centered: map
            .get("CENTERED")
            .and_then(|v| parse_bool(v))
            .unwrap_or(false),
        centered_vertically: true,
        left_margin: 7,
        top_margin: 7,
    }
}

fn parse_kv_map(value: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for segment in value.split(',') {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let mut parts = segment.splitn(2, ':');
        let key = parts.next().unwrap_or("").trim().to_ascii_uppercase();
        let val = parts.next().unwrap_or("").trim();
        if !key.is_empty() {
            map.insert(key, val.to_string());
        }
    }
    map
}

fn tokenize_tab_control_data(value: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_whitespace() || ch == ':' || ch == ',' {
            if !current.is_empty() {
                tokens.push(current.clone());
                current.clear();
            }
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn parse_bool(value: &str) -> Option<bool> {
    value.trim().parse::<i32>().ok().map(|v| v != 0)
}

fn parse_usize(value: &str) -> Option<usize> {
    value.trim().parse::<usize>().ok()
}

fn parse_u32(value: &str) -> Option<u32> {
    value.trim().parse::<u32>().ok()
}

fn parse_i32(value: &str) -> Option<i32> {
    value.trim().parse::<i32>().ok()
}

fn parse_f32(value: &str) -> Option<f32> {
    value.trim().parse::<f32>().ok()
}

fn parse_color_segment(segment: &str) -> Option<(String, [u8; 4])> {
    let mut parts = segment.splitn(2, ':');
    let label = parts.next()?.trim().to_uppercase();
    let values = parts.next()?.trim();
    let numbers = extract_numbers(values);
    if numbers.len() >= 4 {
        return Some((
            label,
            [
                numbers[0] as u8,
                numbers[1] as u8,
                numbers[2] as u8,
                numbers[3] as u8,
            ],
        ));
    }
    None
}

fn extract_numbers(value: &str) -> Vec<i32> {
    let mut numbers = Vec::new();
    let mut current = String::new();
    for ch in value.chars() {
        if ch.is_ascii_digit() || ch == '-' {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(num) = current.parse::<i32>() {
                numbers.push(num);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(num) = current.parse::<i32>() {
            numbers.push(num);
        }
    }
    numbers
}

fn pack_color(rgba: [u8; 4]) -> u32 {
    ((rgba[3] as u32) << 24) | ((rgba[0] as u32) << 16) | ((rgba[1] as u32) << 8) | (rgba[2] as u32)
}

#[cfg(test)]
mod tests {
    use super::parse_static_text_data;

    #[test]
    fn static_text_data_uses_cpp_defaults() {
        let data = parse_static_text_data("CENTERED: 0;");

        assert!(!data.centered);
        assert!(data.centered_vertically);
        assert_eq!(data.left_margin, 7);
        assert_eq!(data.top_margin, 7);
    }
}
