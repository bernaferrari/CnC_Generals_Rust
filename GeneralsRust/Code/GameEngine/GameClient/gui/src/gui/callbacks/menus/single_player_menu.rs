use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/SinglePlayerMenu.cpp",
    "crate::gui::callbacks::menus::single_player_menu",
    "Single Player Menu",
    "Single-player mode selection shell.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "SinglePlayerMenu",
    "Single Player",
    "Campaign and challenge entry points.",
    "Shell",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SinglePlayerControlPort {
    Parent,
    ButtonNew,
    ButtonLoad,
    ButtonBack,
}

impl SinglePlayerControlPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Parent => "SinglePlayerMenuParent",
            Self::ButtonNew => "ButtonNew",
            Self::ButtonLoad => "ButtonLoad",
            Self::ButtonBack => "ButtonBack",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ShellAnimationPort {
    SlideLeft,
    SlideRight,
}

impl ShellAnimationPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::SlideLeft => "SlideLeft",
            Self::SlideRight => "SlideRight",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnimationRegistrationPort {
    pub control: SinglePlayerControlPort,
    pub animation: ShellAnimationPort,
    pub start_delay_ms: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinglePlayerMenuPort {
    pub shell_map_visible: bool,
    pub visible: bool,
    pub button_pushed: bool,
    pub is_shutting_down: bool,
    pub focused_control: SinglePlayerControlPort,
    pub animations: Vec<AnimationRegistrationPort>,
    pub pending_shell_push: Option<String>,
    pub pop_requested: bool,
    pub shutdown_completed: bool,
}

impl Default for SinglePlayerMenuPort {
    fn default() -> Self {
        Self::init()
    }
}

impl SinglePlayerMenuPort {
    pub fn init() -> Self {
        Self {
            shell_map_visible: true,
            visible: true,
            button_pushed: false,
            is_shutting_down: false,
            focused_control: SinglePlayerControlPort::Parent,
            animations: vec![
                AnimationRegistrationPort {
                    control: SinglePlayerControlPort::ButtonNew,
                    animation: ShellAnimationPort::SlideLeft,
                    start_delay_ms: 1,
                },
                AnimationRegistrationPort {
                    control: SinglePlayerControlPort::ButtonLoad,
                    animation: ShellAnimationPort::SlideLeft,
                    start_delay_ms: 200,
                },
                AnimationRegistrationPort {
                    control: SinglePlayerControlPort::ButtonBack,
                    animation: ShellAnimationPort::SlideRight,
                    start_delay_ms: 1,
                },
            ],
            pending_shell_push: None,
            pop_requested: false,
            shutdown_completed: false,
        }
    }

    pub fn shutdown(&mut self, pop_immediate: bool) -> bool {
        self.is_shutting_down = true;
        if pop_immediate {
            self.complete_shutdown();
            return true;
        }
        false
    }

    pub fn update(&mut self, shell_anim_finished: bool) -> bool {
        if self.is_shutting_down && shell_anim_finished {
            self.complete_shutdown();
            return true;
        }
        false
    }

    pub fn handle_escape(&mut self, key_up: bool) -> bool {
        if self.button_pushed || !key_up {
            return false;
        }
        self.select(SinglePlayerControlPort::ButtonBack)
    }

    pub fn take_input_focus(&self, offered_focus: bool) -> bool {
        offered_focus
    }

    pub fn select(&mut self, control: SinglePlayerControlPort) -> bool {
        if self.button_pushed {
            return false;
        }

        match control {
            SinglePlayerControlPort::ButtonNew => {
                self.pending_shell_push = Some("Menus/MapSelectMenu.wnd".to_string());
                self.button_pushed = true;
                true
            }
            SinglePlayerControlPort::ButtonLoad => true,
            SinglePlayerControlPort::ButtonBack => {
                self.pop_requested = true;
                self.button_pushed = true;
                true
            }
            SinglePlayerControlPort::Parent => false,
        }
    }

    pub fn sample() -> Self {
        Self::init()
    }

    fn complete_shutdown(&mut self) {
        self.is_shutting_down = false;
        self.visible = false;
        self.shutdown_completed = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_registers_expected_shell_animations() {
        let menu = SinglePlayerMenuPort::init();

        assert_eq!(menu.focused_control, SinglePlayerControlPort::Parent);
        assert_eq!(menu.animations.len(), 3);
        assert_eq!(
            menu.animations[1].control,
            SinglePlayerControlPort::ButtonLoad
        );
        assert_eq!(menu.animations[1].start_delay_ms, 200);
    }

    #[test]
    fn escape_routes_to_back_button_behavior() {
        let mut menu = SinglePlayerMenuPort::init();

        assert!(menu.handle_escape(true));
        assert!(menu.pop_requested);
        assert!(menu.button_pushed);
    }

    #[test]
    fn shutdown_completes_after_shell_animation_finishes() {
        let mut menu = SinglePlayerMenuPort::init();

        assert!(!menu.shutdown(false));
        assert!(menu.update(true));
        assert!(!menu.visible);
        assert!(menu.shutdown_completed);
    }
}
