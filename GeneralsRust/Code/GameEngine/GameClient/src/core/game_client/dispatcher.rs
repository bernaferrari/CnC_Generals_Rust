// GameClient message dispatcher and stream translator adapters.
// Split from `core/game_client.rs` dump. Included by `game_client/mod.rs`
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
    fn requires_receiving_window_context(&self) -> bool {
        true
    }
    fn translate_game_message(&mut self, _msg: &GameMessage) -> GameMessageDisposition {
        log::error!("GUI command adapter requires receiving window context");
        GameMessageDisposition::KeepMessage
    }
    fn translate_game_message_with_context(
        &mut self,
        msg: &GameMessage,
        client: game_engine::common::message_stream::TranslatorClientObservation,
        dispatch: &mut game_engine::common::message_stream::WindowInputDispatchFn<'_>,
        emitted: &mut Vec<GameMessage>,
        pick_view: Option<&game_engine::common::message_stream::TranslatorPickObjectView>,
        client_ui: &mut dyn std::any::Any,
        drawable_owner: Option<&mut dyn std::any::Any>,
    ) -> GameMessageDisposition {
        self.translator
            .write()
            .expect("receiving command translator poisoned")
            .translate_game_message_with_context(
                msg,
                client,
                dispatch,
                emitted,
                pick_view,
                client_ui,
                drawable_owner,
            )
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
