//! GUIEdit `.wnd` layout save/load.
//!
//! C++ oracle: `Tools/GUIEdit/Source/Save.cpp` (`saveType`, `savePosition`,
//! `saveName`, `FILE_VERSION`, `STARTLAYOUTBLOCK`, nested `WINDOW`/`END`).

/// C++ `MAX_DRAW_DATA` (`WinInstanceData.h`).
pub const MAX_DRAW_DATA: usize = 9;

/// One `WinDrawData` slot (image + color + border).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawDataSlot {
    pub image: String,
    pub color: [u8; 4],
    pub border: [u8; 4],
}

impl Default for DrawDataSlot {
    fn default() -> Self {
        Self {
            image: "NoImage".to_string(),
            color: [255, 255, 255, 255],
            border: [0, 0, 0, 0],
        }
    }
}

/// One GameWindow written to a `.wnd` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WndWindow {
    pub window_type: String,
    pub name: String,
    pub ul_x: i32,
    pub ul_y: i32,
    pub br_x: i32,
    pub br_y: i32,
    pub creation_w: i32,
    pub creation_h: i32,
    pub status: String,
    pub style: String,
    pub system_callback: String,
    pub input_callback: String,
    pub tooltip_callback: String,
    pub draw_callback: String,
    pub font_name: String,
    pub font_size: i32,
    pub font_bold: i32,
    pub header_template: String,
    pub tooltip_text: String,
    pub tooltip_delay: i32,
    pub text: String,
    pub text_color_enabled: [u8; 4],
    pub text_border_enabled: [u8; 4],
    pub text_color_disabled: [u8; 4],
    pub text_border_disabled: [u8; 4],
    pub text_color_hilite: [u8; 4],
    pub text_border_hilite: [u8; 4],
    pub enabled_draw: [DrawDataSlot; MAX_DRAW_DATA],
    pub disabled_draw: [DrawDataSlot; MAX_DRAW_DATA],
    pub hilite_draw: [DrawDataSlot; MAX_DRAW_DATA],
    pub gadget: Option<GadgetData>,
    pub children: Vec<WndWindow>,
}

/// C++ `NUM_TAB_PANES` (`Gadget.h`).
pub const NUM_TAB_PANES: usize = 8;

/// Gadget-specific `.wnd` block after DRAWDATA (`saveGadgetData`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GadgetData {
    ListBox(ListBoxDataEdit),
    ComboBox(ComboBoxDataEdit),
    RadioButton { group: i32 },
    Slider(SliderDataEdit),
    StaticText { centered: i32 },
    TextEntry(TextEntryDataEdit),
    TabControl(TabControlDataEdit),
}

/// C++ `ListboxData` + optional named child DRAWDATA tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListBoxDataEdit {
    pub length: i32,
    pub auto_scroll: i32,
    pub scroll_if_at_end: i32,
    pub auto_purge: i32,
    pub scroll_bar: i32,
    pub multi_select: i32,
    pub columns: i32,
    pub column_width_pct: Vec<i32>,
    pub force_select: i32,
    pub extra_draw: Vec<(String, [DrawDataSlot; MAX_DRAW_DATA])>,
}

/// C++ `ComboBoxData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComboBoxDataEdit {
    pub is_editable: i32,
    pub max_chars: i32,
    pub max_display: i32,
    pub ascii_only: i32,
    pub letters_and_numbers: i32,
    /// C++ `saveDrawData` after COMBOBOXDATA (dropdown/edit/list + nested listbox).
    pub extra_draw: Vec<(String, [DrawDataSlot; MAX_DRAW_DATA])>,
}

/// C++ `SliderData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliderDataEdit {
    pub min_val: i32,
    pub max_val: i32,
    /// C++ `saveSliderData` thumb DRAWDATA after SLIDERDATA.
    pub extra_draw: Vec<(String, [DrawDataSlot; MAX_DRAW_DATA])>,
}

/// C++ `EntryData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEntryDataEdit {
    pub max_len: i32,
    pub secret_text: i32,
    pub numerical_only: i32,
    pub alphanumerical_only: i32,
    pub ascii_only: i32,
}

/// C++ `TabControlData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabControlDataEdit {
    pub tab_orientation: i32,
    pub tab_edge: i32,
    pub tab_width: i32,
    pub tab_height: i32,
    pub tab_count: i32,
    pub pane_border: i32,
    pub pane_disabled: [i32; NUM_TAB_PANES],
}

impl WndWindow {
    pub fn user(name: &str, ul_x: i32, ul_y: i32, br_x: i32, br_y: i32) -> Self {
        Self {
            window_type: "USER".to_string(),
            name: name.to_string(),
            ul_x,
            ul_y,
            br_x,
            br_y,
            creation_w: 800,
            creation_h: 600,
            status: "ENABLED".to_string(),
            style: "USER".to_string(),
            system_callback: "[None]".to_string(),
            input_callback: "[None]".to_string(),
            tooltip_callback: "[None]".to_string(),
            draw_callback: "[None]".to_string(),
            font_name: "Times New Roman".to_string(),
            font_size: 14,
            font_bold: 0,
            header_template: "[None]".to_string(),
            tooltip_text: String::new(),
            tooltip_delay: -1,
            text: String::new(),
            text_color_enabled: [255, 255, 255, 255],
            text_border_enabled: [0, 0, 0, 255],
            text_color_disabled: [128, 128, 128, 255],
            text_border_disabled: [0, 0, 0, 255],
            text_color_hilite: [255, 255, 0, 255],
            text_border_hilite: [0, 0, 0, 255],
            enabled_draw: std::array::from_fn(|_| DrawDataSlot::default()),
            disabled_draw: std::array::from_fn(|_| DrawDataSlot::default()),
            hilite_draw: std::array::from_fn(|_| DrawDataSlot::default()),
            gadget: None,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WndLayout {
    pub file_version: u32,
    pub layout_init: String,
    pub layout_update: String,
    pub layout_shutdown: String,
    pub windows: Vec<WndWindow>,
}

impl Default for WndLayout {
    fn default() -> Self {
        Self {
            file_version: 2,
            layout_init: "[None]".to_string(),
            layout_update: "[None]".to_string(),
            layout_shutdown: "[None]".to_string(),
            windows: Vec::new(),
        }
    }
}

/// Save a layout as C++ GUIEdit `.wnd` text.
pub fn save_layout(filename: &str, layout: &WndLayout) -> String {
    let mut out = String::new();
    out.push_str(&format!("FILE_VERSION = {};\n", layout.file_version));
    out.push_str("STARTLAYOUTBLOCK\n");
    out.push_str(&format!("  LAYOUTINIT = {};\n", layout.layout_init));
    out.push_str(&format!("  LAYOUTUPDATE = {};\n", layout.layout_update));
    out.push_str(&format!("  LAYOUTSHUTDOWN = {};\n", layout.layout_shutdown));
    out.push_str("ENDLAYOUTBLOCK\n");
    for window in &layout.windows {
        write_window(&mut out, filename, window, 0);
    }
    out
}

fn write_window(out: &mut String, filename: &str, window: &WndWindow, indent: usize) {
    let pad = "  ".repeat(indent);
    let data = "  ".repeat(indent + 1);
    out.push_str(&format!("{pad}WINDOW\n"));
    out.push_str(&format!("{data}WINDOWTYPE = {};\n", window.window_type));
    out.push_str(&format!(
        "{data}SCREENRECT = UPPERLEFT: {} {},\n",
        window.ul_x, window.ul_y
    ));
    out.push_str(&format!(
        "{data}             BOTTOMRIGHT: {} {},\n",
        window.br_x, window.br_y
    ));
    out.push_str(&format!(
        "{data}             CREATIONRESOLUTION: {} {};\n",
        window.creation_w, window.creation_h
    ));
    let decorated = if window.name.contains(':') {
        window.name.clone()
    } else {
        format!("{filename}:{}", window.name)
    };
    out.push_str(&format!("{data}NAME = \"{decorated}\";\n"));
    out.push_str(&format!("{data}STATUS = {};\n", window.status));
    out.push_str(&format!("{data}STYLE = {};\n", window.style));
    out.push_str(&format!(
        "{data}SYSTEMCALLBACK = \"{}\";\n",
        window.system_callback
    ));
    out.push_str(&format!(
        "{data}INPUTCALLBACK = \"{}\";\n",
        window.input_callback
    ));
    out.push_str(&format!(
        "{data}TOOLTIPCALLBACK = \"{}\";\n",
        window.tooltip_callback
    ));
    out.push_str(&format!(
        "{data}DRAWCALLBACK = \"{}\";\n",
        window.draw_callback
    ));
    out.push_str(&format!(
        "{data}FONT = NAME: \"{}\", SIZE: {}, BOLD: {};\n",
        window.font_name, window.font_size, window.font_bold
    ));
    out.push_str(&format!(
        "{data}HEADERTEMPLATE = \"{}\";\n",
        window.header_template
    ));
    if !window.tooltip_text.is_empty() {
        out.push_str(&format!(
            "{data}TOOLTIPTEXT = \"{}\";\n",
            window.tooltip_text
        ));
    }
    out.push_str(&format!(
        "{data}TOOLTIPDELAY = {};\n",
        window.tooltip_delay
    ));
    if !window.text.is_empty() {
        out.push_str(&format!("{data}TEXT = \"{}\";\n", window.text));
    }
    write_text_color(out, &data, window);
    write_draw_data(out, &data, "ENABLEDDRAWDATA", &window.enabled_draw);
    write_draw_data(out, &data, "DISABLEDDRAWDATA", &window.disabled_draw);
    write_draw_data(out, &data, "HILITEDRAWDATA", &window.hilite_draw);
    write_gadget_data(out, &data, window.gadget.as_ref());
    if !window.children.is_empty() {
        out.push_str(&format!("{data}CHILD\n"));
        for child in &window.children {
            write_window(out, filename, child, indent + 1);
        }
        out.push_str(&format!("{data}ENDALLCHILDREN\n"));
    }
    out.push_str(&format!("{pad}END\n"));
}

/// Parse a subset of GUIEdit `.wnd` text written by [`save_layout`].
pub fn parse_layout(text: &str) -> Result<WndLayout, SaveError> {
    let mut layout = WndLayout::default();
    let mut lines = text.lines().peekable();
    while let Some(raw) = lines.next() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("FILE_VERSION") {
            layout.file_version = parse_eq_u32(rest)?;
        } else if line.starts_with("LAYOUTINIT") {
            layout.layout_init = parse_eq_token(line)?;
        } else if line.starts_with("LAYOUTUPDATE") {
            layout.layout_update = parse_eq_token(line)?;
        } else if line.starts_with("LAYOUTSHUTDOWN") {
            layout.layout_shutdown = parse_eq_token(line)?;
        } else if line == "WINDOW" {
            layout.windows.push(parse_window(&mut lines)?);
        }
    }
    Ok(layout)
}

fn write_text_color(out: &mut String, data: &str, window: &WndWindow) {
    let e = window.text_color_enabled;
    let eb = window.text_border_enabled;
    let d = window.text_color_disabled;
    let db = window.text_border_disabled;
    let h = window.text_color_hilite;
    let hb = window.text_border_hilite;
    out.push_str(&format!(
        "{data}TEXTCOLOR = ENABLED:  {} {} {} {}, ENABLEDBORDER:  {} {} {} {},\n",
        e[0], e[1], e[2], e[3], eb[0], eb[1], eb[2], eb[3]
    ));
    out.push_str(&format!(
        "{data}            DISABLED: {} {} {} {}, DISABLEDBORDER: {} {} {} {},\n",
        d[0], d[1], d[2], d[3], db[0], db[1], db[2], db[3]
    ));
    out.push_str(&format!(
        "{data}            HILITE:   {} {} {} {}, HILITEBORDER:   {} {} {} {};\n",
        h[0], h[1], h[2], h[3], hb[0], hb[1], hb[2], hb[3]
    ));
}

fn write_draw_data(
    out: &mut String,
    data: &str,
    token: &str,
    slots: &[DrawDataSlot; MAX_DRAW_DATA],
) {
    let pad: String = " ".repeat(token.len());
    for (i, slot) in slots.iter().enumerate() {
        let color = format!(
            "{} {} {} {}",
            slot.color[0], slot.color[1], slot.color[2], slot.color[3]
        );
        let border = format!(
            "{} {} {} {}",
            slot.border[0], slot.border[1], slot.border[2], slot.border[3]
        );
        if i == 0 {
            out.push_str(&format!(
                "{data}{token} = IMAGE: {}, COLOR: {color}, BORDERCOLOR: {border},\n",
                slot.image
            ));
        } else if i + 1 == MAX_DRAW_DATA {
            out.push_str(&format!(
                "{data}{pad}   IMAGE: {}, COLOR: {color}, BORDERCOLOR: {border};\n",
                slot.image
            ));
        } else {
            out.push_str(&format!(
                "{data}{pad}   IMAGE: {}, COLOR: {color}, BORDERCOLOR: {border},\n",
                slot.image
            ));
        }
    }
}

fn write_gadget_data(out: &mut String, data: &str, gadget: Option<&GadgetData>) {
    let Some(gadget) = gadget else {
        return;
    };
    match gadget {
        GadgetData::ListBox(lb) => {
            out.push_str(&format!("{data}LISTBOXDATA = LENGTH: {},\n", lb.length));
            out.push_str(&format!("{data}              AUTOSCROLL: {},\n", lb.auto_scroll));
            out.push_str(&format!(
                "{data}              SCROLLIFATEND: {},\n",
                lb.scroll_if_at_end
            ));
            out.push_str(&format!("{data}              AUTOPURGE: {},\n", lb.auto_purge));
            out.push_str(&format!("{data}              SCROLLBAR: {},\n", lb.scroll_bar));
            out.push_str(&format!("{data}              MULTISELECT: {},\n", lb.multi_select));
            out.push_str(&format!("{data}              COLUMNS: {},\n", lb.columns));
            if lb.columns > 1 {
                for pct in &lb.column_width_pct {
                    out.push_str(&format!("{data}              COLUMNSWIDTH%: {},\n", pct));
                }
            }
            out.push_str(&format!("{data}              FORCESELECT: {};\n", lb.force_select));
            for (token, slots) in &lb.extra_draw {
                write_draw_data(out, data, token, slots);
            }
        }
        GadgetData::ComboBox(cb) => {
            out.push_str(&format!("{data}COMBOBOXDATA = ISEDITABLE: {},\n", cb.is_editable));
            out.push_str(&format!("{data}              MAXCHARS: {},\n", cb.max_chars));
            out.push_str(&format!("{data}              MAXDISPLAY: {},\n", cb.max_display));
            out.push_str(&format!("{data}              ASCIIONLY: {},\n", cb.ascii_only));
            out.push_str(&format!(
                "{data}              LETTERSANDNUMBERS: {};\n",
                cb.letters_and_numbers
            ));
            for (token, slots) in &cb.extra_draw {
                write_draw_data(out, data, token, slots);
            }
        }
        GadgetData::RadioButton { group } => {
            out.push_str(&format!("{data}RADIOBUTTONDATA = GROUP: {};\n", group));
        }
        GadgetData::Slider(sl) => {
            out.push_str(&format!("{data}SLIDERDATA = MINVALUE: {},\n", sl.min_val));
            out.push_str(&format!("{data}             MAXVALUE: {};\n", sl.max_val));
            for (token, slots) in &sl.extra_draw {
                write_draw_data(out, data, token, slots);
            }
        }
        GadgetData::StaticText { centered } => {
            out.push_str(&format!("{data}STATICTEXTDATA = CENTERED: {};\n", centered));
        }
        GadgetData::TextEntry(te) => {
            out.push_str(&format!("{data}TEXTENTRYDATA = MAXLEN: {},\n", te.max_len));
            out.push_str(&format!("{data}                SECRETTEXT: {},\n", te.secret_text));
            out.push_str(&format!(
                "{data}                NUMERICALONLY: {},\n",
                te.numerical_only
            ));
            out.push_str(&format!(
                "{data}                ALPHANUMERICALONLY: {},\n",
                te.alphanumerical_only
            ));
            out.push_str(&format!("{data}                ASCIIONLY: {};\n", te.ascii_only));
        }
        GadgetData::TabControl(tc) => {
            out.push_str(&format!(
                "{data}TABCONTROLDATA = TABORIENTATION: {},\n",
                tc.tab_orientation
            ));
            out.push_str(&format!("{data}                 TABEDGE: {},\n", tc.tab_edge));
            out.push_str(&format!("{data}                 TABWIDTH: {},\n", tc.tab_width));
            out.push_str(&format!("{data}                 TABHEIGHT: {},\n", tc.tab_height));
            out.push_str(&format!("{data}                 TABCOUNT: {},\n", tc.tab_count));
            out.push_str(&format!("{data}                 PANEBORDER: {},\n", tc.pane_border));
            out.push_str(&format!(
                "{data}                 PANEDISABLED: {},",
                NUM_TAB_PANES
            ));
            for (i, v) in tc.pane_disabled.iter().enumerate() {
                if i + 1 == NUM_TAB_PANES {
                    out.push_str(&format!("{v};"));
                } else {
                    out.push_str(&format!("{v},"));
                }
            }
            out.push('\n');
        }
    }
}

fn parse_draw_slot(line: &str) -> Result<DrawDataSlot, SaveError> {
    let image = line
        .split_once("IMAGE:")
        .and_then(|(_, rest)| rest.split_once("COLOR:"))
        .map(|(img, _)| img.trim().trim_end_matches(',').trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or(SaveError::InvalidInput)?;
    let color = parse_four_u8(line, "COLOR:")?;
    let border = parse_four_u8(line, "BORDERCOLOR:")?;
    Ok(DrawDataSlot {
        image,
        color,
        border,
    })
}

fn parse_rgba_after_label(line: &str, label: &str) -> Result<[u8; 4], SaveError> {
    let rest = line
        .split_once(label)
        .map(|(_, r)| r)
        .ok_or(SaveError::InvalidInput)?;
    let cleaned = rest.replace([',', ';'], " ");
    let mut nums = cleaned.split_whitespace().filter_map(|s| s.parse::<u8>().ok());
    Ok([
        nums.next().ok_or(SaveError::InvalidInput)?,
        nums.next().ok_or(SaveError::InvalidInput)?,
        nums.next().ok_or(SaveError::InvalidInput)?,
        nums.next().ok_or(SaveError::InvalidInput)?,
    ])
}

fn parse_text_color_block<'a, I>(
    window: &mut WndWindow,
    first: &str,
    lines: &mut std::iter::Peekable<I>,
) -> Result<(), SaveError>
where
    I: Iterator<Item = &'a str>,
{
    window.text_color_enabled = parse_rgba_after_label(first, "ENABLED:")?;
    window.text_border_enabled = parse_rgba_after_label(first, "ENABLEDBORDER:")?;
    let disabled = lines.next().ok_or(SaveError::InvalidInput)?;
    window.text_color_disabled = parse_rgba_after_label(disabled, "DISABLED:")?;
    window.text_border_disabled = parse_rgba_after_label(disabled, "DISABLEDBORDER:")?;
    let hilite = lines.next().ok_or(SaveError::InvalidInput)?;
    window.text_color_hilite = parse_rgba_after_label(hilite, "HILITE:")?;
    window.text_border_hilite = parse_rgba_after_label(hilite, "HILITEBORDER:")?;
    Ok(())
}

fn parse_four_u8(line: &str, key: &str) -> Result<[u8; 4], SaveError> {
    let rest = line
        .split_once(key)
        .map(|(_, r)| r)
        .ok_or(SaveError::InvalidInput)?;
    let cleaned = rest.replace([',', ';'], " ");
    let mut nums = cleaned.split_whitespace().filter_map(|s| s.parse::<u8>().ok());
    Ok([
        nums.next().ok_or(SaveError::InvalidInput)?,
        nums.next().ok_or(SaveError::InvalidInput)?,
        nums.next().ok_or(SaveError::InvalidInput)?,
        nums.next().ok_or(SaveError::InvalidInput)?,
    ])
}

fn parse_window<'a, I>(lines: &mut std::iter::Peekable<I>) -> Result<WndWindow, SaveError>
where
    I: Iterator<Item = &'a str>,
{
    let mut window = WndWindow::user("", 0, 0, 0, 0);
    window.status.clear();
    window.style.clear();
    let mut in_child = false;
    let mut draw_fill: Option<char> = None; // 'e' 'd' 'h'
    let mut draw_i = 0usize;
    let mut extra_draw_name: Option<String> = None;
    let mut extra_draw_slots: Option<[DrawDataSlot; MAX_DRAW_DATA]> = None;
    let mut extra_draw_i = 0usize;
    while let Some(raw) = lines.next() {
        let line = raw.trim();
        if line == "END" {
            finish_extra_draw(&mut window, extra_draw_name.take(), extra_draw_slots.take());
            return Ok(window);
        }
        if line == "ENDALLCHILDREN" {
            in_child = false;
            continue;
        }
        if line == "CHILD" {
            in_child = true;
            continue;
        }
        if line == "WINDOW" {
            if in_child {
                window.children.push(parse_window(lines)?);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("WINDOWTYPE") {
            window.window_type = parse_eq_token(rest)?;
        } else if line.contains("UPPERLEFT:") {
            let nums = parse_two_ints(line, "UPPERLEFT:")?;
            window.ul_x = nums.0;
            window.ul_y = nums.1;
        } else if line.contains("BOTTOMRIGHT:") {
            let nums = parse_two_ints(line, "BOTTOMRIGHT:")?;
            window.br_x = nums.0;
            window.br_y = nums.1;
        } else if line.contains("CREATIONRESOLUTION:") {
            let nums = parse_two_ints(line, "CREATIONRESOLUTION:")?;
            window.creation_w = nums.0;
            window.creation_h = nums.1;
        } else if let Some(rest) = line.strip_prefix("NAME") {
            let token = parse_eq_token(rest)?;
            window.name = token.trim_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("STATUS") {
            window.status = parse_eq_token(rest)?;
        } else if let Some(rest) = line.strip_prefix("STYLE") {
            window.style = parse_eq_token(rest)?;
        } else if let Some(rest) = line.strip_prefix("SYSTEMCALLBACK") {
            window.system_callback = parse_eq_token(rest)?.trim_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("INPUTCALLBACK") {
            window.input_callback = parse_eq_token(rest)?.trim_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("TOOLTIPCALLBACK") {
            window.tooltip_callback = parse_eq_token(rest)?.trim_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("DRAWCALLBACK") {
            window.draw_callback = parse_eq_token(rest)?.trim_matches('"').to_string();
        } else if let Some(rest) = line.strip_prefix("FONT") {
            let (name, size, bold) = parse_font_token(rest)?;
            window.font_name = name;
            window.font_size = size;
            window.font_bold = bold;
        } else if let Some(rest) = line.strip_prefix("HEADERTEMPLATE") {
            window.header_template = parse_eq_token(rest)?.trim_matches('"').to_string();
        } else if line.starts_with("TOOLTIPTEXT") {
            window.tooltip_text = parse_eq_token(line)?.trim_matches('"').to_string();
        } else if line.starts_with("TOOLTIPDELAY") {
            window.tooltip_delay = parse_eq_token(line)?
                .trim_end_matches(';')
                .parse()
                .unwrap_or(-1);
        } else if line.starts_with("TEXTCOLOR") {
            parse_text_color_block(&mut window, line, lines)?;
        } else if line.starts_with("TEXT =") || line.starts_with("TEXT=") {
            window.text = parse_eq_token(line)?.trim_matches('"').to_string();
        } else if line.starts_with("LISTBOXDATA") {
            window.gadget = Some(GadgetData::ListBox(parse_listbox_data(line, lines)?));
        } else if line.starts_with("COMBOBOXDATA") {
            window.gadget = Some(GadgetData::ComboBox(parse_combobox_data(line, lines)?));
        } else if line.starts_with("RADIOBUTTONDATA") {
            let group = parse_named_i32(line, "GROUP:").unwrap_or(0);
            window.gadget = Some(GadgetData::RadioButton { group });
        } else if line.starts_with("SLIDERDATA") {
            let min_val = parse_named_i32(line, "MINVALUE:").unwrap_or(0);
            let mut max_val = 0;
            if let Some(raw2) = lines.next() {
                max_val = parse_named_i32(raw2, "MAXVALUE:").unwrap_or(0);
            }
            window.gadget = Some(GadgetData::Slider(SliderDataEdit {
                min_val,
                max_val,
                extra_draw: Vec::new(),
            }));
        } else if line.starts_with("STATICTEXTDATA") {
            let centered = parse_named_i32(line, "CENTERED:").unwrap_or(0);
            window.gadget = Some(GadgetData::StaticText { centered });
        } else if line.starts_with("TEXTENTRYDATA") {
            window.gadget = Some(GadgetData::TextEntry(parse_text_entry_data(line, lines)?));
        } else if line.starts_with("TABCONTROLDATA") {
            window.gadget = Some(GadgetData::TabControl(parse_tab_control_data(line, lines)?));
        } else if line.contains("IMAGE:") {
            let slot = parse_draw_slot(line)?;
            let named = gadget_draw_token(line);
            if let Some(token) = named {
                if extra_draw_name.as_deref() != Some(token) {
                    finish_extra_draw(
                        &mut window,
                        extra_draw_name.take(),
                        extra_draw_slots.take(),
                    );
                    extra_draw_name = Some(token.to_string());
                    extra_draw_slots = Some(std::array::from_fn(|_| DrawDataSlot::default()));
                    extra_draw_i = 0;
                    draw_fill = None;
                }
                if let Some(slots) = extra_draw_slots.as_mut() {
                    if extra_draw_i < MAX_DRAW_DATA {
                        slots[extra_draw_i] = slot;
                        extra_draw_i += 1;
                    }
                }
            } else {
                finish_extra_draw(&mut window, extra_draw_name.take(), extra_draw_slots.take());
                if line.contains("DISABLEDDRAWDATA") {
                    draw_fill = Some('d');
                    draw_i = 0;
                } else if line.contains("HILITEDRAWDATA") {
                    draw_fill = Some('h');
                    draw_i = 0;
                } else if line.contains("ENABLEDDRAWDATA") {
                    draw_fill = Some('e');
                    draw_i = 0;
                }
                if let Some(kind) = draw_fill {
                    if draw_i < MAX_DRAW_DATA {
                        match kind {
                            'd' => window.disabled_draw[draw_i] = slot,
                            'h' => window.hilite_draw[draw_i] = slot,
                            _ => window.enabled_draw[draw_i] = slot,
                        }
                        draw_i += 1;
                    }
                }
            }
        }
    }
    Err(SaveError::ProcessingFailed)
}

fn gadget_draw_token(line: &str) -> Option<&str> {
    for token in [
        "LISTBOXENABLEDUPBUTTONDRAWDATA",
        "LISTBOXDISABLEDUPBUTTONDRAWDATA",
        "LISTBOXHILITEUPBUTTONDRAWDATA",
        "LISTBOXENABLEDDOWNBUTTONDRAWDATA",
        "LISTBOXDISABLEDDOWNBUTTONDRAWDATA",
        "LISTBOXHILITEDOWNBUTTONDRAWDATA",
        "LISTBOXENABLEDSLIDERDRAWDATA",
        "LISTBOXDISABLEDSLIDERDRAWDATA",
        "LISTBOXHILITESLIDERDRAWDATA",
        "SLIDERTHUMBENABLEDDRAWDATA",
        "SLIDERTHUMBDISABLEDDRAWDATA",
        "SLIDERTHUMBHILITEDRAWDATA",
        "COMBOBOXDROPDOWNBUTTONENABLEDDRAWDATA",
        "COMBOBOXDROPDOWNBUTTONDISABLEDDRAWDATA",
        "COMBOBOXDROPDOWNBUTTONHILITEDRAWDATA",
        "COMBOBOXEDITBOXENABLEDDRAWDATA",
        "COMBOBOXEDITBOXDISABLEDDRAWDATA",
        "COMBOBOXEDITBOXHILITEDRAWDATA",
        "COMBOBOXLISTBOXENABLEDDRAWDATA",
        "COMBOBOXLISTBOXDISABLEDDRAWDATA",
        "COMBOBOXLISTBOXHILITEDRAWDATA",
    ] {
        if line.contains(token) {
            return Some(token);
        }
    }
    None
}

fn finish_extra_draw(
    window: &mut WndWindow,
    name: Option<String>,
    slots: Option<[DrawDataSlot; MAX_DRAW_DATA]>,
) {
    let (Some(name), Some(slots)) = (name, slots) else {
        return;
    };
    if let Some(GadgetData::ListBox(lb)) = window.gadget.as_mut() {
        lb.extra_draw.push((name, slots));
    } else if let Some(GadgetData::ComboBox(cb)) = window.gadget.as_mut() {
        cb.extra_draw.push((name, slots));
    } else if let Some(GadgetData::Slider(sl)) = window.gadget.as_mut() {
        sl.extra_draw.push((name, slots));
    }
}

fn parse_named_i32(line: &str, key: &str) -> Option<i32> {
    let rest = line.split_once(key)?.1;
    rest.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
        .find_map(|t| t.parse::<i32>().ok())
}

fn parse_listbox_data<'a, I>(first: &str, lines: &mut std::iter::Peekable<I>) -> Result<ListBoxDataEdit, SaveError>
where
    I: Iterator<Item = &'a str>,
{
    let mut lb = ListBoxDataEdit {
        length: parse_named_i32(first, "LENGTH:").unwrap_or(0),
        auto_scroll: 0,
        scroll_if_at_end: 0,
        auto_purge: 0,
        scroll_bar: 0,
        multi_select: 0,
        columns: 1,
        column_width_pct: Vec::new(),
        force_select: 0,
        extra_draw: Vec::new(),
    };
    while let Some(raw) = lines.peek() {
        let line = raw.trim();
        if line.starts_with("AUTOSCROLL:") {
            lb.auto_scroll = parse_named_i32(line, "AUTOSCROLL:").unwrap_or(0);
        } else if line.starts_with("SCROLLIFATEND:") {
            lb.scroll_if_at_end = parse_named_i32(line, "SCROLLIFATEND:").unwrap_or(0);
        } else if line.starts_with("AUTOPURGE:") {
            lb.auto_purge = parse_named_i32(line, "AUTOPURGE:").unwrap_or(0);
        } else if line.starts_with("SCROLLBAR:") {
            lb.scroll_bar = parse_named_i32(line, "SCROLLBAR:").unwrap_or(0);
        } else if line.starts_with("MULTISELECT:") {
            lb.multi_select = parse_named_i32(line, "MULTISELECT:").unwrap_or(0);
        } else if line.starts_with("COLUMNS:") {
            lb.columns = parse_named_i32(line, "COLUMNS:").unwrap_or(1);
        } else if line.contains("COLUMNSWIDTH%") {
            if let Some(v) = parse_named_i32(line, "COLUMNSWIDTH%:") {
                lb.column_width_pct.push(v);
            }
        } else if line.starts_with("FORCESELECT:") {
            lb.force_select = parse_named_i32(line, "FORCESELECT:").unwrap_or(0);
            lines.next();
            break;
        } else {
            break;
        }
        lines.next();
    }
    Ok(lb)
}

fn parse_combobox_data<'a, I>(first: &str, lines: &mut std::iter::Peekable<I>) -> Result<ComboBoxDataEdit, SaveError>
where
    I: Iterator<Item = &'a str>,
{
    let mut cb = ComboBoxDataEdit {
        is_editable: parse_named_i32(first, "ISEDITABLE:").unwrap_or(0),
        max_chars: 0,
        max_display: 0,
        ascii_only: 0,
        letters_and_numbers: 0,
        extra_draw: Vec::new(),
    };
    while let Some(raw) = lines.peek() {
        let line = raw.trim();
        if line.starts_with("MAXCHARS:") {
            cb.max_chars = parse_named_i32(line, "MAXCHARS:").unwrap_or(0);
        } else if line.starts_with("MAXDISPLAY:") {
            cb.max_display = parse_named_i32(line, "MAXDISPLAY:").unwrap_or(0);
        } else if line.starts_with("ASCIIONLY:") {
            cb.ascii_only = parse_named_i32(line, "ASCIIONLY:").unwrap_or(0);
        } else if line.starts_with("LETTERSANDNUMBERS:") {
            cb.letters_and_numbers = parse_named_i32(line, "LETTERSANDNUMBERS:").unwrap_or(0);
            lines.next();
            break;
        } else {
            break;
        }
        lines.next();
    }
    Ok(cb)
}

fn parse_text_entry_data<'a, I>(first: &str, lines: &mut std::iter::Peekable<I>) -> Result<TextEntryDataEdit, SaveError>
where
    I: Iterator<Item = &'a str>,
{
    let mut te = TextEntryDataEdit {
        max_len: parse_named_i32(first, "MAXLEN:").unwrap_or(0),
        secret_text: 0,
        numerical_only: 0,
        alphanumerical_only: 0,
        ascii_only: 0,
    };
    while let Some(raw) = lines.peek() {
        let line = raw.trim();
        if line.starts_with("SECRETTEXT:") {
            te.secret_text = parse_named_i32(line, "SECRETTEXT:").unwrap_or(0);
        } else if line.starts_with("NUMERICALONLY:") {
            te.numerical_only = parse_named_i32(line, "NUMERICALONLY:").unwrap_or(0);
        } else if line.starts_with("ALPHANUMERICALONLY:") {
            te.alphanumerical_only = parse_named_i32(line, "ALPHANUMERICALONLY:").unwrap_or(0);
        } else if line.starts_with("ASCIIONLY:") {
            te.ascii_only = parse_named_i32(line, "ASCIIONLY:").unwrap_or(0);
            lines.next();
            break;
        } else {
            break;
        }
        lines.next();
    }
    Ok(te)
}

fn parse_tab_control_data<'a, I>(first: &str, lines: &mut std::iter::Peekable<I>) -> Result<TabControlDataEdit, SaveError>
where
    I: Iterator<Item = &'a str>,
{
    let mut tc = TabControlDataEdit {
        tab_orientation: parse_named_i32(first, "TABORIENTATION:").unwrap_or(0),
        tab_edge: 0,
        tab_width: 0,
        tab_height: 0,
        tab_count: 0,
        pane_border: 0,
        pane_disabled: [0; NUM_TAB_PANES],
    };
    while let Some(raw) = lines.peek() {
        let line = raw.trim();
        if line.starts_with("TABEDGE:") {
            tc.tab_edge = parse_named_i32(line, "TABEDGE:").unwrap_or(0);
        } else if line.starts_with("TABWIDTH:") {
            tc.tab_width = parse_named_i32(line, "TABWIDTH:").unwrap_or(0);
        } else if line.starts_with("TABHEIGHT:") {
            tc.tab_height = parse_named_i32(line, "TABHEIGHT:").unwrap_or(0);
        } else if line.starts_with("TABCOUNT:") {
            tc.tab_count = parse_named_i32(line, "TABCOUNT:").unwrap_or(0);
        } else if line.starts_with("PANEBORDER:") {
            tc.pane_border = parse_named_i32(line, "PANEBORDER:").unwrap_or(0);
        } else if line.contains("PANEDISABLED:") {
            let rest = line.split_once("PANEDISABLED:").map(|(_, r)| r).unwrap_or("");
            let nums: Vec<i32> = rest
                .split(|c: char| c == ',' || c == ';')
                .filter_map(|t| t.trim().parse::<i32>().ok())
                .collect();
            // First number is NUM_TAB_PANES count, remaining are flags.
            let flags = if nums.len() > NUM_TAB_PANES {
                &nums[1..]
            } else {
                &nums
            };
            for (i, v) in flags.iter().take(NUM_TAB_PANES).enumerate() {
                tc.pane_disabled[i] = *v;
            }
            lines.next();
            break;
        } else {
            break;
        }
        lines.next();
    }
    Ok(tc)
}

fn parse_eq_token(line: &str) -> Result<String, SaveError> {
    let (_, rhs) = line.split_once('=').ok_or(SaveError::InvalidInput)?;
    Ok(rhs.trim().trim_end_matches(';').trim().to_string())
}

fn parse_eq_u32(line: &str) -> Result<u32, SaveError> {
    parse_eq_token(line)?
        .parse()
        .map_err(|_| SaveError::InvalidInput)
}

/// C++ `FONT = NAME: "Times New Roman", SIZE: 14, BOLD: 0;`
fn parse_font_token(line: &str) -> Result<(String, i32, i32), SaveError> {
    let rhs = parse_eq_token(line)?;
    let name = rhs
        .split_once("NAME:")
        .and_then(|(_, rest)| rest.split_once("SIZE:"))
        .map(|(n, _)| n.trim().trim_matches(|c| c == '"' || c == ',').trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or(SaveError::InvalidInput)?;
    let size = rhs
        .split_once("SIZE:")
        .and_then(|(_, rest)| {
            rest.split(|c: char| c == ',' || c.is_whitespace())
                .find_map(|t| t.parse::<i32>().ok())
        })
        .ok_or(SaveError::InvalidInput)?;
    let bold = rhs
        .split_once("BOLD:")
        .and_then(|(_, rest)| {
            rest.split(|c: char| c == ',' || c == ';' || c.is_whitespace())
                .find_map(|t| t.parse::<i32>().ok())
        })
        .unwrap_or(0);
    Ok((name, size, bold))
}

fn parse_two_ints(line: &str, key: &str) -> Result<(i32, i32), SaveError> {
    let rest = line
        .split_once(key)
        .map(|(_, r)| r)
        .ok_or(SaveError::InvalidInput)?;
    let cleaned = rest.replace([',', ';'], " ");
    let mut nums = cleaned.split_whitespace().filter_map(|s| s.parse::<i32>().ok());
    let a = nums.next().ok_or(SaveError::InvalidInput)?;
    let b = nums.next().ok_or(SaveError::InvalidInput)?;
    Ok((a, b))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveError {
    NotActive,
    ProcessingFailed,
    InvalidInput,
}

impl std::fmt::Display for SaveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SaveError::NotActive => write!(f, "Not active"),
            SaveError::ProcessingFailed => write!(f, "Processing failed"),
            SaveError::InvalidInput => write!(f, "Invalid input"),
        }
    }
}

impl std::error::Error for SaveError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wnd_layout_roundtrip_matches_cpp_tokens() {
        let mut root = WndWindow::user("Candidate", 56, 56, 208, 224);
        root.status = "DRAGABLE+ENABLED".to_string();
        root.children.push({
            let mut child = WndWindow::user("Ok", 64, 80, 120, 100);
            child.window_type = "PUSHBUTTON".to_string();
            child.style = "PUSHBUTTON".to_string();
            child
        });
        let layout = WndLayout {
            windows: vec![root],
            ..WndLayout::default()
        };
        let text = save_layout("IMECandidateWindow.wnd", &layout);
        assert!(text.contains("FILE_VERSION = 2;"));
        assert!(text.contains("STARTLAYOUTBLOCK"));
        assert!(text.contains("WINDOWTYPE = USER;"));
        assert!(text.contains("SCREENRECT = UPPERLEFT: 56 56,"));
        assert!(text.contains("BOTTOMRIGHT: 208 224,"));
        assert!(text.contains("CREATIONRESOLUTION: 800 600;"));
        assert!(text.contains("NAME = \"IMECandidateWindow.wnd:Candidate\";"));
        assert!(text.contains("SYSTEMCALLBACK = \"[None]\";"));
        assert!(text.contains("INPUTCALLBACK = \"[None]\";"));
        assert!(text.contains("FONT = NAME: \"Times New Roman\", SIZE: 14, BOLD: 0;"));
        assert!(text.contains("WINDOWTYPE = PUSHBUTTON;"));
        assert!(text.contains("ENDALLCHILDREN"));
        let parsed = parse_layout(&text).expect("parse");
        assert_eq!(parsed.file_version, 2);
        assert_eq!(parsed.windows.len(), 1);
        assert_eq!(parsed.windows[0].window_type, "USER");
        assert_eq!(parsed.windows[0].ul_x, 56);
        assert_eq!(parsed.windows[0].br_y, 224);
        assert_eq!(parsed.windows[0].font_name, "Times New Roman");
        assert_eq!(parsed.windows[0].font_size, 14);
        assert_eq!(parsed.windows[0].font_bold, 0);
        assert_eq!(parsed.windows[0].children.len(), 1);
        assert_eq!(parsed.windows[0].children[0].window_type, "PUSHBUTTON");
        assert_eq!(parsed.windows[0].children[0].ul_x, 64);
        assert_eq!(parsed.windows[0].children[0].br_x, 120);
        assert!(text.contains("ENABLEDDRAWDATA = IMAGE: NoImage, COLOR: 255 255 255 255, BORDERCOLOR: 0 0 0 0,"));
        assert!(text.contains("HILITEDRAWDATA"));
        assert_eq!(parsed.windows[0].enabled_draw.len(), MAX_DRAW_DATA);
        assert_eq!(parsed.windows[0].enabled_draw[0].image, "NoImage");
        assert_eq!(parsed.windows[0].enabled_draw[8].image, "NoImage");
    }

    #[test]
    fn drawdata_roundtrips_nine_slots_like_cpp_savedrawdata() {
        let mut root = WndWindow::user("Button", 0, 0, 40, 20);
        root.enabled_draw[0] = DrawDataSlot {
            image: "ButtonEnabled".into(),
            color: [1, 2, 3, 4],
            border: [5, 6, 7, 8],
        };
        root.enabled_draw[8] = DrawDataSlot {
            image: "ButtonEnabled8".into(),
            color: [9, 10, 11, 12],
            border: [13, 14, 15, 16],
        };
        let text = save_layout("B.wnd", &WndLayout {
            windows: vec![root],
            ..WndLayout::default()
        });
        assert!(text.contains("ENABLEDDRAWDATA = IMAGE: ButtonEnabled, COLOR: 1 2 3 4, BORDERCOLOR: 5 6 7 8,"));
        assert!(text.contains("IMAGE: ButtonEnabled8, COLOR: 9 10 11 12, BORDERCOLOR: 13 14 15 16;"));
        let parsed = parse_layout(&text).expect("parse");
        assert_eq!(parsed.windows[0].enabled_draw[0].image, "ButtonEnabled");
        assert_eq!(parsed.windows[0].enabled_draw[0].color, [1, 2, 3, 4]);
        assert_eq!(parsed.windows[0].enabled_draw[8].image, "ButtonEnabled8");
        assert_eq!(parsed.windows[0].enabled_draw[8].border, [13, 14, 15, 16]);
    }

    #[test]
    fn font_bold_roundtrips_like_cpp_savefont() {
        let mut root = WndWindow::user("Title", 0, 0, 100, 20);
        root.font_name = "Arial".to_string();
        root.font_size = 12;
        root.font_bold = 1;
        let text = save_layout("Title.wnd", &WndLayout {
            windows: vec![root],
            ..WndLayout::default()
        });
        assert!(text.contains("FONT = NAME: \"Arial\", SIZE: 12, BOLD: 1;"));
        let parsed = parse_layout(&text).expect("parse");
        assert_eq!(parsed.windows[0].font_name, "Arial");
        assert_eq!(parsed.windows[0].font_size, 12);
        assert_eq!(parsed.windows[0].font_bold, 1);
    }

    #[test]
    fn gadget_blocks_roundtrip_like_cpp_savegadgetdata() {
        let mut list = WndWindow::user("Chat", 0, 0, 200, 100);
        list.window_type = "LISTBOX".into();
        list.style = "SCROLL_LISTBOX".into();
        list.gadget = Some(GadgetData::ListBox(ListBoxDataEdit {
            length: 8,
            auto_scroll: 1,
            scroll_if_at_end: 0,
            auto_purge: 1,
            scroll_bar: 1,
            multi_select: 0,
            columns: 2,
            column_width_pct: vec![40, 60],
            force_select: 1,
            extra_draw: vec![(
                "LISTBOXENABLEDUPBUTTONDRAWDATA".into(),
                std::array::from_fn(|i| {
                    if i == 0 {
                        DrawDataSlot {
                            image: "UpArrow".into(),
                            color: [1, 2, 3, 4],
                            border: [0, 0, 0, 0],
                        }
                    } else {
                        DrawDataSlot::default()
                    }
                }),
            )],
        }));
        let mut combo = WndWindow::user("Pick", 0, 120, 200, 140);
        combo.window_type = "COMBOBOX".into();
        combo.style = "COMBOBOX".into();
        combo.gadget = Some(GadgetData::ComboBox(ComboBoxDataEdit {
            is_editable: 1,
            max_chars: 32,
            max_display: 6,
            ascii_only: 1,
            letters_and_numbers: 0,
            extra_draw: vec![(
                "COMBOBOXDROPDOWNBUTTONENABLEDDRAWDATA".into(),
                std::array::from_fn(|i| {
                    if i == 0 {
                        DrawDataSlot {
                            image: "ComboDrop".into(),
                            color: [9, 8, 7, 6],
                            border: [0, 0, 0, 255],
                        }
                    } else {
                        DrawDataSlot::default()
                    }
                }),
            )],
        }));
        let mut radio = WndWindow::user("Opt", 0, 150, 40, 170);
        radio.gadget = Some(GadgetData::RadioButton { group: 3 });
        let mut slider = WndWindow::user("Vol", 0, 180, 100, 200);
        slider.gadget = Some(GadgetData::Slider(SliderDataEdit {
            min_val: 0,
            max_val: 100,
            extra_draw: vec![(
                "SLIDERTHUMBENABLEDDRAWDATA".into(),
                std::array::from_fn(|i| {
                    if i == 0 {
                        DrawDataSlot {
                            image: "Thumb".into(),
                            color: [2, 3, 4, 5],
                            border: [0, 0, 0, 0],
                        }
                    } else {
                        DrawDataSlot::default()
                    }
                }),
            )],
        }));
        let mut entry = WndWindow::user("Name", 0, 210, 120, 230);
        entry.gadget = Some(GadgetData::TextEntry(TextEntryDataEdit {
            max_len: 16,
            secret_text: 0,
            numerical_only: 0,
            alphanumerical_only: 1,
            ascii_only: 1,
        }));
        let mut tab = WndWindow::user("Tabs", 0, 240, 200, 300);
        let mut pane = [0; NUM_TAB_PANES];
        pane[0] = 1;
        pane[7] = 1;
        tab.gadget = Some(GadgetData::TabControl(TabControlDataEdit {
            tab_orientation: 0,
            tab_edge: 1,
            tab_width: 80,
            tab_height: 20,
            tab_count: 3,
            pane_border: 2,
            pane_disabled: pane,
        }));

        let text = save_layout(
            "Gadgets.wnd",
            &WndLayout {
                windows: vec![list, combo, radio, slider, entry, tab],
                ..WndLayout::default()
            },
        );
        assert!(text.contains("LISTBOXDATA = LENGTH: 8,"));
        assert!(text.contains("COLUMNS: 2,"));
        assert!(text.contains("COLUMNSWIDTH%: 40,"));
        assert!(text.contains("COLUMNSWIDTH%: 60,"));
        assert!(text.contains("FORCESELECT: 1;"));
        assert!(text.contains("LISTBOXENABLEDUPBUTTONDRAWDATA = IMAGE: UpArrow"));
        assert!(text.contains("COMBOBOXDATA = ISEDITABLE: 1,"));
        assert!(text.contains("MAXCHARS: 32,"));
        assert!(text.contains("LETTERSANDNUMBERS: 0;"));
        assert!(text.contains("COMBOBOXDROPDOWNBUTTONENABLEDDRAWDATA = IMAGE: ComboDrop"));
        assert!(text.contains("RADIOBUTTONDATA = GROUP: 3;"));
        assert!(text.contains("SLIDERDATA = MINVALUE: 0,"));
        assert!(text.contains("MAXVALUE: 100;"));
        assert!(text.contains("SLIDERTHUMBENABLEDDRAWDATA = IMAGE: Thumb"));
        assert!(text.contains("TEXTENTRYDATA = MAXLEN: 16,"));
        assert!(text.contains("ALPHANUMERICALONLY: 1,"));
        assert!(text.contains("TABCONTROLDATA = TABORIENTATION: 0,"));
        assert!(text.contains("PANEDISABLED: 8,1,0,0,0,0,0,0,1;"));

        let parsed = parse_layout(&text).expect("parse gadgets");
        assert_eq!(parsed.windows.len(), 6);
        match &parsed.windows[0].gadget {
            Some(GadgetData::ListBox(lb)) => {
                assert_eq!(lb.length, 8);
                assert_eq!(lb.auto_scroll, 1);
                assert_eq!(lb.columns, 2);
                assert_eq!(lb.column_width_pct, vec![40, 60]);
                assert_eq!(lb.force_select, 1);
                assert_eq!(lb.extra_draw.len(), 1);
                assert_eq!(lb.extra_draw[0].0, "LISTBOXENABLEDUPBUTTONDRAWDATA");
                assert_eq!(lb.extra_draw[0].1[0].image, "UpArrow");
            }
            other => panic!("listbox gadget missing: {other:?}"),
        }
        match &parsed.windows[1].gadget {
            Some(GadgetData::ComboBox(cb)) => {
                assert_eq!(cb.is_editable, 1);
                assert_eq!(cb.max_chars, 32);
                assert_eq!(cb.max_display, 6);
                assert_eq!(cb.ascii_only, 1);
                assert_eq!(cb.extra_draw.len(), 1);
                assert_eq!(cb.extra_draw[0].0, "COMBOBOXDROPDOWNBUTTONENABLEDDRAWDATA");
                assert_eq!(cb.extra_draw[0].1[0].image, "ComboDrop");
                assert_eq!(cb.extra_draw[0].1[0].color, [9, 8, 7, 6]);
            }
            other => panic!("combo gadget missing: {other:?}"),
        }
        assert_eq!(
            parsed.windows[2].gadget,
            Some(GadgetData::RadioButton { group: 3 })
        );
        match &parsed.windows[3].gadget {
            Some(GadgetData::Slider(sl)) => {
                assert_eq!(sl.min_val, 0);
                assert_eq!(sl.max_val, 100);
                assert_eq!(sl.extra_draw.len(), 1);
                assert_eq!(sl.extra_draw[0].0, "SLIDERTHUMBENABLEDDRAWDATA");
                assert_eq!(sl.extra_draw[0].1[0].image, "Thumb");
                assert_eq!(sl.extra_draw[0].1[0].color, [2, 3, 4, 5]);
            }
            other => panic!("slider missing: {other:?}"),
        }
        match &parsed.windows[4].gadget {
            Some(GadgetData::TextEntry(te)) => {
                assert_eq!(te.max_len, 16);
                assert_eq!(te.alphanumerical_only, 1);
                assert_eq!(te.ascii_only, 1);
            }
            other => panic!("textentry missing: {other:?}"),
        }
        match &parsed.windows[5].gadget {
            Some(GadgetData::TabControl(tc)) => {
                assert_eq!(tc.tab_width, 80);
                assert_eq!(tc.tab_count, 3);
                assert_eq!(tc.pane_disabled[0], 1);
                assert_eq!(tc.pane_disabled[7], 1);
            }
            other => panic!("tab missing: {other:?}"),
        }
    }

    #[test]
    fn textcolor_tooltipdelay_text_roundtrip_like_cpp_savewindow() {
        let mut root = WndWindow::user("OkButton", 10, 10, 80, 30);
        root.tooltip_text = "GUI:OkTip".into();
        root.tooltip_delay = 12;
        root.text = "GUI:Ok".into();
        root.text_color_enabled = [1, 2, 3, 4];
        root.text_border_enabled = [5, 6, 7, 8];
        root.text_color_disabled = [9, 10, 11, 12];
        root.text_border_disabled = [13, 14, 15, 16];
        root.text_color_hilite = [17, 18, 19, 20];
        root.text_border_hilite = [21, 22, 23, 24];
        let text = save_layout(
            "Ok.wnd",
            &WndLayout {
                windows: vec![root],
                ..WndLayout::default()
            },
        );
        let header = text.find("HEADERTEMPLATE").expect("header");
        let tooltip = text.find("TOOLTIPTEXT").expect("tooltip");
        let delay = text.find("TOOLTIPDELAY").expect("delay");
        let label = text.find("\n  TEXT = ").expect("text");
        let color = text.find("TEXTCOLOR").expect("color");
        let draw = text.find("ENABLEDDRAWDATA").expect("draw");
        assert!(
            header < tooltip && tooltip < delay && delay < label && label < color && color < draw,
            "order header={header} tip={tooltip} delay={delay} text={label} color={color} draw={draw}\n{text}"
        );
        assert!(text.contains("TOOLTIPTEXT = \"GUI:OkTip\";"));
        assert!(text.contains("TOOLTIPDELAY = 12;"));
        assert!(text.contains("TEXT = \"GUI:Ok\";"));
        assert!(text.contains(
            "TEXTCOLOR = ENABLED:  1 2 3 4, ENABLEDBORDER:  5 6 7 8,"
        ));
        assert!(text.contains("DISABLED: 9 10 11 12, DISABLEDBORDER: 13 14 15 16,"));
        assert!(text.contains("HILITE:   17 18 19 20, HILITEBORDER:   21 22 23 24;"));
        let parsed = parse_layout(&text).expect("parse text fields");
        let w = &parsed.windows[0];
        assert_eq!(w.tooltip_text, "GUI:OkTip");
        assert_eq!(w.tooltip_delay, 12);
        assert_eq!(w.text, "GUI:Ok");
        assert_eq!(w.text_color_enabled, [1, 2, 3, 4]);
        assert_eq!(w.text_border_enabled, [5, 6, 7, 8]);
        assert_eq!(w.text_color_disabled, [9, 10, 11, 12]);
        assert_eq!(w.text_border_disabled, [13, 14, 15, 16]);
        assert_eq!(w.text_color_hilite, [17, 18, 19, 20]);
        assert_eq!(w.text_border_hilite, [21, 22, 23, 24]);
        assert!(
            save_layout("Ok.wnd", &WndLayout {
                windows: vec![WndWindow::user("Bare", 0, 0, 10, 10)],
                ..WndLayout::default()
            })
            .contains("TOOLTIPDELAY = -1;"),
            "C++ always writes TOOLTIPDELAY (default -1)"
        );
    }
}
