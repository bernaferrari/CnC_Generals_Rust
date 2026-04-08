pub use crate::gui::callbacks::ingame_callbacks::{
    hide_in_game_chat, is_in_game_chat_active, reset_in_game_chat, set_in_game_chat_type,
    show_in_game_chat, toggle_in_game_chat,
};
pub use crate::gui::callbacks::ingame_callbacks::{InGameChatCallbacks, InGameChatType};

// PARITY_NOTE: the concrete chat state machine lives in `callbacks::ingame_callbacks`; this module
// keeps the original compilation unit entry points available at the legacy path.
