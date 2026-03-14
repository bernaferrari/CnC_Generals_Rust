#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GuiPortRecord {
    pub cpp_relative_path: &'static str,
    pub rust_module_path: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
}

impl GuiPortRecord {
    pub const fn new(
        cpp_relative_path: &'static str,
        rust_module_path: &'static str,
        title: &'static str,
        summary: &'static str,
    ) -> Self {
        Self {
            cpp_relative_path,
            rust_module_path,
            title,
            summary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GadgetKind {
    PushButton,
    CheckBox,
    RadioButton,
    HorizontalSlider,
    VerticalSlider,
    ListBox,
    ComboBox,
    ProgressBar,
    StaticText,
    TextEntry,
    TabControl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GadgetPort {
    pub record: &'static GuiPortRecord,
    pub label: &'static str,
    pub summary: &'static str,
    pub interaction: &'static str,
    pub kind: GadgetKind,
}

impl GadgetPort {
    pub const fn new(
        record: &'static GuiPortRecord,
        label: &'static str,
        summary: &'static str,
        interaction: &'static str,
        kind: GadgetKind,
    ) -> Self {
        Self {
            record,
            label,
            summary,
            interaction,
            kind,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlBarPort {
    pub record: &'static GuiPortRecord,
    pub label: &'static str,
    pub summary: &'static str,
}

impl ControlBarPort {
    pub const fn new(
        record: &'static GuiPortRecord,
        label: &'static str,
        summary: &'static str,
    ) -> Self {
        Self {
            record,
            label,
            summary,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MenuScreenPort {
    pub record: &'static GuiPortRecord,
    pub key: &'static str,
    pub title: &'static str,
    pub summary: &'static str,
    pub group: &'static str,
}

impl MenuScreenPort {
    pub const fn new(
        record: &'static GuiPortRecord,
        key: &'static str,
        title: &'static str,
        summary: &'static str,
        group: &'static str,
    ) -> Self {
        Self {
            record,
            key,
            title,
            summary,
            group,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallbackPort {
    pub record: &'static GuiPortRecord,
    pub label: &'static str,
    pub summary: &'static str,
}

impl CallbackPort {
    pub const fn new(
        record: &'static GuiPortRecord,
        label: &'static str,
        summary: &'static str,
    ) -> Self {
        Self {
            record,
            label,
            summary,
        }
    }
}
