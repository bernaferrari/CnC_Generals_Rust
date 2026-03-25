use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/CreditsMenu.cpp",
    "crate::gui::callbacks::menus::credits_menu",
    "Credits Menu",
    "Credits screen callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "CreditsMenu",
    "Credits",
    "Scrolling credits and acknowledgements.",
    "Shell",
);

pub const PARENT_WINDOW_ID: &str = "CreditsMenu.wnd:ParentCreditsWindow";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreditsMenuState {
    Inactive,
    Active,
    ShuttingDown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreditsMenuAction {
    Pop,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreditsMenuPort {
    pub state: CreditsMenuState,
    pub shell_map_visible: bool,
    pub credits_music_playing: bool,
    pub layout_visible: bool,
    pub credits_initialized: bool,
    pub lines: Vec<String>,
    pub highlighted_line: usize,
    pub scroll_offset: u32,
}

impl Default for CreditsMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl CreditsMenuPort {
    pub fn sample() -> Self {
        Self {
            state: CreditsMenuState::Inactive,
            shell_map_visible: true,
            credits_music_playing: false,
            layout_visible: false,
            credits_initialized: false,
            lines: vec![
                "Engineering".to_string(),
                "Design".to_string(),
                "Audio".to_string(),
                "Quality Assurance".to_string(),
                "Community".to_string(),
            ],
            highlighted_line: 0,
            scroll_offset: 128,
        }
    }

    pub fn init(&mut self) -> CreditsMenuAction {
        self.shell_map_visible = false;
        self.credits_initialized = true;
        self.layout_visible = true;
        self.credits_music_playing = true;
        self.state = CreditsMenuState::Active;
        CreditsMenuAction::None
    }

    pub fn update(&mut self, credits_finished: bool) -> CreditsMenuAction {
        if self.state != CreditsMenuState::Active {
            return CreditsMenuAction::None;
        }
        if !self.credits_initialized || credits_finished {
            return CreditsMenuAction::Pop;
        }
        CreditsMenuAction::None
    }

    pub fn is_finished(&self) -> bool {
        self.state != CreditsMenuState::Active
    }

    pub fn shutdown(&mut self) -> CreditsMenuAction {
        self.credits_initialized = false;
        self.shell_map_visible = true;
        self.layout_visible = false;
        self.credits_music_playing = false;
        self.state = CreditsMenuState::Inactive;
        CreditsMenuAction::None
    }

    pub fn handle_key(&mut self, key: u8, key_state_up: bool) -> CreditsMenuAction {
        if self.state != CreditsMenuState::Active {
            return CreditsMenuAction::None;
        }
        if key == 0x1B && key_state_up {
            return CreditsMenuAction::Pop;
        }
        CreditsMenuAction::None
    }

    pub fn wants_focus(&self) -> bool {
        self.state == CreditsMenuState::Active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_hides_shell_map_and_enables_credits() {
        let mut menu = CreditsMenuPort::sample();
        let action = menu.init();
        assert_eq!(action, CreditsMenuAction::None);
        assert!(!menu.shell_map_visible);
        assert!(menu.credits_initialized);
        assert!(menu.layout_visible);
        assert!(menu.credits_music_playing);
        assert_eq!(menu.state, CreditsMenuState::Active);
    }

    #[test]
    fn update_pops_when_credits_finished() {
        let mut menu = CreditsMenuPort::sample();
        menu.init();
        let action = menu.update(true);
        assert_eq!(action, CreditsMenuAction::Pop);
    }

    #[test]
    fn update_does_nothing_while_credits_scroll() {
        let mut menu = CreditsMenuPort::sample();
        menu.init();
        let action = menu.update(false);
        assert_eq!(action, CreditsMenuAction::None);
    }

    #[test]
    fn update_pops_when_credits_not_initialized() {
        let mut menu = CreditsMenuPort::sample();
        menu.state = CreditsMenuState::Active;
        menu.credits_initialized = false;
        let action = menu.update(false);
        assert_eq!(action, CreditsMenuAction::Pop);
    }

    #[test]
    fn update_noop_when_inactive() {
        let mut menu = CreditsMenuPort::sample();
        let action = menu.update(false);
        assert_eq!(action, CreditsMenuAction::None);
    }

    #[test]
    fn shutdown_restores_shell_map() {
        let mut menu = CreditsMenuPort::sample();
        menu.init();
        menu.shutdown();
        assert!(menu.shell_map_visible);
        assert!(!menu.layout_visible);
        assert!(!menu.credits_music_playing);
        assert!(!menu.credits_initialized);
        assert_eq!(menu.state, CreditsMenuState::Inactive);
    }

    #[test]
    fn esc_pops_menu() {
        let mut menu = CreditsMenuPort::sample();
        menu.init();
        let action = menu.handle_key(0x1B, true);
        assert_eq!(action, CreditsMenuAction::Pop);
    }

    #[test]
    fn esc_ignored_on_key_down() {
        let mut menu = CreditsMenuPort::sample();
        menu.init();
        let action = menu.handle_key(0x1B, false);
        assert_eq!(action, CreditsMenuAction::None);
    }

    #[test]
    fn other_keys_ignored() {
        let mut menu = CreditsMenuPort::sample();
        menu.init();
        let action = menu.handle_key(0x41, true);
        assert_eq!(action, CreditsMenuAction::None);
    }

    #[test]
    fn handle_key_noop_when_inactive() {
        let mut menu = CreditsMenuPort::sample();
        let action = menu.handle_key(0x1B, true);
        assert_eq!(action, CreditsMenuAction::None);
    }

    #[test]
    fn is_finished_false_when_active() {
        let mut menu = CreditsMenuPort::sample();
        menu.init();
        assert!(!menu.is_finished());
    }

    #[test]
    fn is_finished_true_when_inactive() {
        let menu = CreditsMenuPort::sample();
        assert!(menu.is_finished());
    }

    #[test]
    fn wants_focus_true_when_active() {
        let mut menu = CreditsMenuPort::sample();
        menu.init();
        assert!(menu.wants_focus());
    }

    #[test]
    fn wants_focus_false_when_inactive() {
        let menu = CreditsMenuPort::sample();
        assert!(!menu.wants_focus());
    }

    #[test]
    fn parent_window_id_matches_cpp() {
        assert_eq!(PARENT_WINDOW_ID, "CreditsMenu.wnd:ParentCreditsWindow");
    }
}
