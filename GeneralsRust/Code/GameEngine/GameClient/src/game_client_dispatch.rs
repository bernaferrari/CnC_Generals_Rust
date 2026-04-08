pub use crate::core::GameClientMessageDispatcher;
pub use crate::message_stream::game_message::GameMessage;
pub use game_engine::common::message_stream::GameMessageDisposition;

pub fn translate_game_message(
    dispatcher: &GameClientMessageDispatcher,
    msg: &GameMessage,
) -> GameMessageDisposition {
    dispatcher.translate_game_message(msg)
}
