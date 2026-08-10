// GameClient message dispatcher and stream translator adapters.
// Split from `core/game_client.rs` dump. Included by `game_client_impl/mod.rs`
// so this stays one logical `game_client` module (public API identical).

/// Message dispatcher for filtering and routing game messages
pub struct GameClientMessageDispatcher {
    message_filters: Vec<Box<dyn MessageFilter + Send + Sync>>,
}

struct DispatcherTranslator {
    dispatcher: Arc<GameClientMessageDispatcher>,
}

impl DispatcherTranslator {
    fn new(dispatcher: Arc<GameClientMessageDispatcher>) -> Self {
        Self { dispatcher }
    }
}

impl GameMessageTranslator for DispatcherTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        self.dispatcher.translate_game_message(msg)
    }
}

struct CommandTranslatorMessageAdapter {
    translator: Arc<RwLock<CommandTranslatorImpl>>,
}

impl CommandTranslatorMessageAdapter {
    fn new(translator: Arc<RwLock<CommandTranslatorImpl>>) -> Self {
        Self { translator }
    }
}

impl GameMessageTranslator for CommandTranslatorMessageAdapter {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        match self.translator.write() {
            Ok(mut translator) => translator.translate_game_message(msg),
            Err(err) => {
                log::warn!("Command translator lock poisoned: {}", err);
                GameMessageDisposition::KeepMessage
            }
        }
    }
}

// Message dispatcher implementation

impl GameClientMessageDispatcher {
    pub fn new() -> Self {
        Self {
            message_filters: Vec::new(),
        }
    }

    pub fn translate_game_message(&self, msg: &GameMessage) -> GameMessageDisposition {
        let msg_type = msg.get_type().clone();
        // Keep network messages (placeholder until network layer implemented)
        if self.is_network_message(&msg_type) {
            return GameMessageDisposition::KeepMessage;
        }

        // Keep game control messages
        match msg_type {
            GameMessageType::NewGame
            | GameMessageType::ClearGameData
            | GameMessageType::FrameTick(_) => GameMessageDisposition::KeepMessage,
            _ => GameMessageDisposition::DestroyMessage,
        }
    }

    fn is_network_message(&self, msg_type: &GameMessageType) -> bool {
        is_network_command_message(msg_type)
    }

    pub fn add_filter(&mut self, filter: Box<dyn MessageFilter + Send + Sync>) {
        self.message_filters.push(filter);
    }
}

impl Default for GameClientMessageDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
