use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use game_network::{
    commands::CommandPayload, get_network, ExecutedFrame, FrameListener, NetCommandType,
};
use log::{debug, warn};
use once_cell::sync::Lazy;

use crate::gui::callbacks::ingame_callbacks::push_network_chat_message;
use crate::message_stream::command_list::get_command_list;
use crate::message_stream::game_message::GameMessage;

use super::command_conversion::{decode_game_command, log_unsupported_command};

#[derive(Debug, Clone)]
pub struct NetworkProgressState {
    pub progress_type: game_network::commands::ProgressType,
    pub percentage: u8,
    pub last_command: NetCommandType,
}

static NETWORK_PROGRESS: Lazy<Mutex<HashMap<u8, NetworkProgressState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub fn get_network_progress(player_id: u8) -> Option<NetworkProgressState> {
    NETWORK_PROGRESS
        .lock()
        .ok()
        .and_then(|map| map.get(&player_id).cloned())
}

/// Handle returned when the bridge is installed.  Dropping this handle will
/// automatically unregister the frame listener from the networking layer.
pub struct NetworkBridgeHandle {
    network: Arc<game_network::NetworkInterface>,
    listener_id: game_network::FrameListenerId,
}

impl NetworkBridgeHandle {
    /// Attempt to install the bridge. Returns `None` when the networking layer
    /// has not been initialised (e.g. single-player or early startup).
    pub fn install() -> Option<Self> {
        let network = get_network()?;

        let listener: FrameListener = Arc::new(|frame: &ExecutedFrame| {
            handle_executed_frame(frame);
        });

        let listener_id = network.register_frame_listener(listener);
        debug!("Registered network frame listener with id {}", listener_id);

        Some(Self {
            network,
            listener_id,
        })
    }
}

impl Drop for NetworkBridgeHandle {
    fn drop(&mut self) {
        if self.network.unregister_frame_listener(self.listener_id) {
            debug!(
                "Unregistered network frame listener with id {}",
                self.listener_id
            );
        } else {
            warn!(
                "Failed to unregister network frame listener with id {}",
                self.listener_id
            );
        }
    }
}

fn handle_executed_frame(frame: &ExecutedFrame) {
    for command in &frame.commands {
        match command.command_type {
            NetCommandType::GameCommand => {
                if let CommandPayload::GameCommand(data) = &command.payload {
                    match decode_game_command(data, command.player_id) {
                        Some(message) => append_to_command_list(message),
                        None => log_unsupported_command(data),
                    }
                }
            }
            NetCommandType::Chat | NetCommandType::DisconnectChat => {
                if let CommandPayload::Chat(chat) = &command.payload {
                    push_network_chat_message(
                        command.player_id,
                        chat.message.clone(),
                        chat.target_mask,
                        command.command_type == NetCommandType::DisconnectChat,
                    );
                }
            }
            NetCommandType::Progress
            | NetCommandType::LoadComplete
            | NetCommandType::TimeoutStart => {
                if let CommandPayload::Progress(progress) = &command.payload {
                    if let Ok(mut map) = NETWORK_PROGRESS.lock() {
                        map.insert(
                            command.player_id,
                            NetworkProgressState {
                                progress_type: progress.progress_type,
                                percentage: progress.percentage,
                                last_command: command.command_type,
                            },
                        );
                    }
                } else {
                    warn!(
                        "Unexpected payload for {:?}: {:?}",
                        command.command_type, command.payload
                    );
                }
            }
            _ => {
                // For now ignore other command types (keep-alives, acks, etc.).
            }
        }
    }
}

fn append_to_command_list(message: GameMessage) {
    match get_command_list().write() {
        Ok(mut list) => list.append_message(message),
        Err(err) => warn!("Failed to lock command list for network command: {err}"),
    }
}
