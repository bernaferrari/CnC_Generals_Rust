use crate::gui::source_catalog::GuiPortRecord;

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GameWindowTransitionsStyles.cpp",
    "crate::gui::game_window_transitions_styles",
    "Game Window Transition Styles",
    "Defines named transition style metadata used by shell and popup animations.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransitionStylePort {
    pub name: String,
    pub frame_length: i32,
    pub sound_event: Option<String>,
}

pub fn default_styles() -> Vec<TransitionStylePort> {
    vec![
        TransitionStylePort {
            name: "FlashTransition".to_string(),
            frame_length: 12,
            sound_event: Some("GUIBoarderFadeIn".to_string()),
        },
        TransitionStylePort {
            name: "SlideFromTop".to_string(),
            frame_length: 18,
            sound_event: None,
        },
    ]
}
