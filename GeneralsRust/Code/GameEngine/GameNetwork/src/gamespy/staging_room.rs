#![allow(dead_code, unused_imports, unused_variables)]
//! GameSpy Staging Room
//! Handles game setup and player management before starting a game

use crate::error::{NetworkError, NetworkResult};
use crate::gamespy::{GameInvite, GameSettings};
use std::collections::HashMap;
use tracing::info;

/// Staging room table owned by the GameSpy interface. Only the owning task's
/// methods touch it, so it is a plain field with no shared lock.
pub struct StagingRoom {
    rooms: HashMap<String, GameRoom>,
}

pub struct GameRoom {
    pub id: String,
    pub host: String,
    pub players: Vec<String>,
    pub settings: GameSettings,
    pub invites: Vec<GameInvite>,
}

impl StagingRoom {
    pub async fn new() -> NetworkResult<Self> {
        Ok(Self {
            rooms: HashMap::new(),
        })
    }

    pub async fn create_game(&self, settings: GameSettings) -> NetworkResult<String> {
        let room_id = uuid::Uuid::new_v4().to_string();
        info!("Created game room: {}", room_id);
        Ok(room_id)
    }

    pub async fn join_game(&self, game_id: String) -> NetworkResult<()> {
        info!("Joined game room: {}", game_id);
        Ok(())
    }

    pub async fn send_invite(
        &self,
        player_id: String,
        settings: GameSettings,
    ) -> NetworkResult<()> {
        info!("Sent game invite to: {}", player_id);
        Ok(())
    }

    pub async fn accept_invite(&self, invite_id: String) -> NetworkResult<()> {
        info!("Accepted game invite: {}", invite_id);
        Ok(())
    }
}
