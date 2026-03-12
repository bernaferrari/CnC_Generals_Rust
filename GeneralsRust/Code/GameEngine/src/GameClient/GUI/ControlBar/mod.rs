// FILE: mod.rs
// Control Bar Module - Context-sensitive command interface
//
// This module contains the complete ControlBar/CommandBar UI system ported from C++.
//
// The Control Bar is the primary command interface in C&C Generals, providing context-sensitive
// buttons and displays based on the currently selected units or buildings.
//
// Original C++ source: GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ControlBar/
//
// Major components:
// - CommandButton: Individual command buttons that can be assigned to the UI
// - CommandSet: Collections of command buttons for different unit types
// - ControlBar: Main control bar that manages the entire UI system
// - ControlBarScheme: Visual scheme system for different factions
//
// Key features:
// - Command button grid (18 buttons max)
// - Production queue display (9 queue slots)
// - General's powers/abilities bar (11 shortcuts max)
// - Science/upgrade purchase interface
// - Context switching (command, inventory, beacon, construction, observer, etc.)
// - Multi-selection support
// - Visual theming per faction

pub mod types;
pub mod command_button;
pub mod command_set;
pub mod scheme;
pub mod control_bar;

// Re-export commonly used types
pub use types::*;
pub use command_button::CommandButton;
pub use command_set::CommandSet;
pub use scheme::{
    ControlBarScheme,
    ControlBarSchemeManager,
    ControlBarSchemeImage,
    ControlBarSchemeAnimation,
    AnimationType,
};
pub use control_bar::{
    ControlBar,
    SideSelectWindowData,
    ContainEntry,
    QueueEntry,
};

/// Initialize the Control Bar system
pub fn initialize() {
    ControlBar::initialize_instance();
}

/// Get the global ControlBar instance
pub fn get_control_bar() -> Option<std::sync::Arc<std::sync::Mutex<ControlBar>>> {
    ControlBar::get_instance()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_button_creation() {
        let button = CommandButton::new();
        assert_eq!(button.get_command_type(), GUICommandType::None);
        assert_eq!(button.get_options(), COMMAND_OPTION_NONE);
    }

    #[test]
    fn test_command_set_creation() {
        let set = CommandSet::new("TestSet".to_string());
        assert_eq!(set.get_name(), "TestSet");
        assert!(set.get_command_button(0).is_none());
    }

    #[test]
    fn test_gui_command_type_conversion() {
        let cmd = GUICommandType::from_name("ATTACK_MOVE");
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap(), GUICommandType::AttackMove);

        let name = GUICommandType::AttackMove.to_name();
        assert_eq!(name, "ATTACK_MOVE");
    }

    #[test]
    fn test_command_options() {
        let need_target = NEED_TARGET_ENEMY_OBJECT | NEED_TARGET_POS;
        assert_ne!(need_target & NEED_TARGET_ENEMY_OBJECT, 0);
        assert_ne!(need_target & NEED_TARGET_POS, 0);
        assert_eq!(need_target & NEED_UPGRADE, 0);
    }

    #[test]
    fn test_control_bar_contexts() {
        assert_eq!(ControlBarContext::None as u32, 0);
        assert_eq!(ControlBarContext::Command as u32, 1);
    }

    #[test]
    fn test_scheme_animation_types() {
        let anim = AnimationType::from_name("SLIDE_RIGHT");
        assert!(anim.is_some());
        assert_eq!(anim.unwrap(), AnimationType::SlideRight);
    }
}
