use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/KeyboardOptionsMenu.cpp",
    "crate::gui::callbacks::menus::keyboard_options_menu",
    "Keyboard Options Menu",
    "Keyboard configuration callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "KeyboardOptionsMenu",
    "Keyboard Options",
    "Key binding and keyboard settings.",
    "Shell",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardCategoryPort {
    Control,
    Unit,
    Interface,
    Camera,
}

impl KeyboardCategoryPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Control => "Control",
            Self::Unit => "Unit",
            Self::Interface => "Interface",
            Self::Camera => "Camera",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardCommandPort {
    pub category: KeyboardCategoryPort,
    pub command_name: String,
    pub description: String,
    pub current_hotkey: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardOptionsMenuPort {
    pub selected_category: KeyboardCategoryPort,
    pub commands: Vec<KeyboardCommandPort>,
    pub selected_command: Option<usize>,
    pub assign_text: String,
    pub shift_down: bool,
    pub ctrl_down: bool,
    pub alt_down: bool,
    pub absolute_assignment_ready: bool,
}

impl Default for KeyboardOptionsMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl KeyboardOptionsMenuPort {
    pub fn select_category(&mut self, category: KeyboardCategoryPort) {
        self.selected_category = category;
        self.selected_command = self
            .commands
            .iter()
            .position(|command| command.category == category);
    }

    pub fn select_command(&mut self, index: usize) -> bool {
        if index >= self.commands.len() {
            return false;
        }
        self.selected_command = Some(index);
        true
    }

    pub fn press_modifier(&mut self, modifier: &str) {
        match modifier {
            "Shift" => self.shift_down = true,
            "Ctrl" => self.ctrl_down = true,
            "Alt" => self.alt_down = true,
            _ => {}
        }
        self.assign_text = self.modifier_prefix();
        self.absolute_assignment_ready = false;
    }

    pub fn release_modifier(&mut self, modifier: &str) {
        match modifier {
            "Shift" => self.shift_down = false,
            "Ctrl" => self.ctrl_down = false,
            "Alt" => self.alt_down = false,
            _ => {}
        }
        self.assign_text = self.modifier_prefix();
    }

    pub fn assign_key(&mut self, key: &str) {
        let prefix = self.modifier_prefix();
        self.assign_text = format!("{prefix}{key}");
        self.absolute_assignment_ready = true;
    }

    pub fn reset_all(&mut self) {
        for command in &mut self.commands {
            command.current_hotkey = "Default".to_string();
        }
        self.assign_text.clear();
        self.absolute_assignment_ready = false;
    }

    pub fn selected_command(&self) -> Option<&KeyboardCommandPort> {
        self.selected_command
            .and_then(|index| self.commands.get(index))
    }

    pub fn sample() -> Self {
        Self {
            selected_category: KeyboardCategoryPort::Control,
            commands: vec![
                KeyboardCommandPort {
                    category: KeyboardCategoryPort::Control,
                    command_name: "Attack Move".to_string(),
                    description: "Orders selected units to move and engage enemies on the path."
                        .to_string(),
                    current_hotkey: "A".to_string(),
                },
                KeyboardCommandPort {
                    category: KeyboardCategoryPort::Control,
                    command_name: "Force Fire".to_string(),
                    description: "Forces units to fire on the targeted ground or object."
                        .to_string(),
                    current_hotkey: "Ctrl+F".to_string(),
                },
                KeyboardCommandPort {
                    category: KeyboardCategoryPort::Interface,
                    command_name: "Toggle Radar".to_string(),
                    description: "Shows or hides the radar display.".to_string(),
                    current_hotkey: "R".to_string(),
                },
            ],
            selected_command: Some(0),
            assign_text: String::new(),
            shift_down: false,
            ctrl_down: false,
            alt_down: false,
            absolute_assignment_ready: false,
        }
    }

    fn modifier_prefix(&self) -> String {
        let mut parts = Vec::new();
        if self.shift_down {
            parts.push("Shift");
        }
        if self.ctrl_down {
            parts.push("Ctrl");
        }
        if self.alt_down {
            parts.push("Alt");
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("{}+", parts.join("+"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modifier_key_builds_assignment_prefix() {
        let mut menu = KeyboardOptionsMenuPort::sample();
        menu.press_modifier("Ctrl");
        menu.press_modifier("Shift");
        menu.assign_key("K");

        assert_eq!(menu.assign_text, "Shift+Ctrl+K");
        assert!(menu.absolute_assignment_ready);
    }

    #[test]
    fn reset_all_restores_default_marker() {
        let mut menu = KeyboardOptionsMenuPort::sample();
        menu.commands[0].current_hotkey = "Q".to_string();
        menu.reset_all();

        assert_eq!(menu.commands[0].current_hotkey, "Default");
    }
}
