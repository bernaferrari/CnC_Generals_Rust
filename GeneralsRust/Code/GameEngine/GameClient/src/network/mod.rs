//! Networking integration layer for the game client.
//!
//! This module bridges the modern Rust networking stack (`game_network`) with
//! the legacy message/command pipeline that the gameplay code expects.  It
//! provides:
//!   * Conversion helpers between `GameMessage` structures and the generic
//!     `GameCommandData` payloads that travel across the network.
//!   * A `NetworkBridgeHandle` that subscribes to executed network frames and
//!     appends the reconstructed commands onto the client command list.

#[cfg(feature = "network")]
mod bridge;
#[cfg(feature = "network")]
mod command_conversion;

#[cfg(feature = "network")]
pub use bridge::NetworkBridgeHandle;
#[cfg(feature = "network")]
pub use command_conversion::{
    decode_game_command, encode_game_message, is_network_command_message,
};

#[cfg(not(feature = "network"))]
#[derive(Clone, Default)]
pub struct NetworkBridgeHandle;

#[cfg(not(feature = "network"))]
impl NetworkBridgeHandle {
    pub fn install() -> Option<Self> {
        None
    }

    pub fn new() -> Option<Self> {
        None
    }

    pub fn has_peer_subscriptions(&self) -> bool {
        false
    }

    pub fn poll(&self) {}
}

#[cfg(not(feature = "network"))]
pub fn is_network_command_message(
    ty: crate::message_stream::game_message::GameMessageType,
) -> bool {
    game_engine::common::message_stream::is_network_command_message(&ty)
}

#[cfg(all(test, not(feature = "network")))]
mod tests {
    use super::is_network_command_message;
    use crate::message_stream::game_message::{Coord3D, GameMessageType};

    #[test]
    fn network_command_classifier_matches_command_messages_in_default_build() {
        assert!(is_network_command_message(GameMessageType::DoMoveTo(
            Coord3D::default()
        )));
        assert!(is_network_command_message(GameMessageType::LogicCRC(
            0xDEADBEEF
        )));
        assert!(is_network_command_message(
            GameMessageType::CaptureBuilding(1, 2)
        ));
        assert!(!is_network_command_message(GameMessageType::NewGame));
        assert!(!is_network_command_message(GameMessageType::FrameTick(30)));
    }
}
