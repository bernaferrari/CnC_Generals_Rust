use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "WinInstanceData.cpp",
    "crate::gui::win_instance_data",
    "Win Instance Data",
    "Ports per-window visual state, owner routing, and gadget-specific data attachments.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DrawDataPort {
    pub image: Option<String>,
    pub color: Option<u32>,
    pub border_color: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct WinInstanceDataPort {
    pub id: i32,
    pub state: u32,
    pub style: u32,
    pub status: u32,
    pub text_label: String,
    pub tooltip_label: String,
    pub tooltip_delay: i32,
    pub decorated_name: String,
    pub image_offset: (i32, i32),
    pub enabled_draw_data: Vec<DrawDataPort>,
    pub disabled_draw_data: Vec<DrawDataPort>,
    pub hilite_draw_data: Vec<DrawDataPort>,
}

impl Default for WinInstanceDataPort {
    fn default() -> Self {
        let default_draw = || DrawDataPort {
            image: None,
            color: None,
            border_color: None,
        };

        Self {
            id: 0,
            state: 0,
            style: 0,
            status: 0,
            text_label: String::new(),
            tooltip_label: String::new(),
            tooltip_delay: -1,
            decorated_name: String::new(),
            image_offset: (0, 0),
            enabled_draw_data: vec![default_draw(); 4],
            disabled_draw_data: vec![default_draw(); 4],
            hilite_draw_data: vec![default_draw(); 4],
        }
    }
}
