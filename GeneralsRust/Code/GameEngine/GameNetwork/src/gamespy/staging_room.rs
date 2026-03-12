#![allow(dead_code, unused_imports, unused_variables)]
//! GameSpy Staging Room
//! Handles game setup and player management before starting a game

use crate::error::{NetworkError, NetworkResult};
use crate::gamespy::{GameInvite, GameSettings};
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::info;

pub struct StagingRoom {
    rooms: RwLock<HashMap<String, GameRoom>>,
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
            rooms: RwLock::new(HashMap::new()),
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
