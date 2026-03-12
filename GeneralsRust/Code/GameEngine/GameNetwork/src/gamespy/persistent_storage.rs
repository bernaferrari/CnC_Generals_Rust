//! GameSpy Persistent Storage
//! Persists ladder rankings, buddy lists, and related metadata to disk while
//! exposing async helpers for the rest of the networking stack.

use crate::error::{NetworkError, NetworkResult};
use crate::gamespy::{PlayerRanking, RankTier};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

#[derive(Debug, Serialize, Deserialize, Default)]
struct BuddyListSnapshot {
    buddies: HashSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PlayerStatsSnapshot {
    stats: PlayerRanking,
}

/// Persistent storage facade mirroring the legacy GameSpy storage service.
/// Data is cached in-memory for fast access and synchronised to disk in a
/// simple JSON format for resilience.
pub struct PersistentStorage {
    root: PathBuf,
    buddies_dir: PathBuf,
    stats_dir: PathBuf,
    cached_buddies: RwLock<HashMap<String, HashSet<String>>>,
    cached_stats: RwLock<HashMap<String, PlayerRanking>>,
}

impl PersistentStorage {
    /// Create a new persistent storage instance rooted at `storage_root`.
    pub async fn new<P: AsRef<Path>>(storage_root: P) -> NetworkResult<Self> {
        let root = storage_root.as_ref().to_path_buf();
        let buddies_dir = root.join("buddies");
        let stats_dir = root.join("stats");

        fs::create_dir_all(&buddies_dir).await.map_err(|err| {
            NetworkError::generic(format!(
                "Failed to create buddies directory {}: {}",
                buddies_dir.display(),
                err
            ))
        })?;
        fs::create_dir_all(&stats_dir).await.map_err(|err| {
            NetworkError::generic(format!(
                "Failed to create stats directory {}: {}",
                stats_dir.display(),
                err
            ))
        })?;

        Ok(Self {
            root,
            buddies_dir,
            stats_dir,
            cached_buddies: RwLock::new(HashMap::new()),
            cached_stats: RwLock::new(HashMap::new()),
        })
    }

    fn buddy_file(&self, player_id: &str) -> PathBuf {
        self.buddies_dir.join(format!("{}.json", player_id))
    }

    fn stats_file(&self, player_id: &str) -> PathBuf {
        self.stats_dir.join(format!("{}.json", player_id))
    }

    /// Persist the provided buddy list to disk.
    pub async fn save_buddy_list(
        &self,
        player_id: &str,
        buddies: HashSet<String>,
    ) -> NetworkResult<()> {
        {
            let mut cache = self.cached_buddies.write().await;
            cache.insert(player_id.to_string(), buddies.clone());
        }

        let snapshot = BuddyListSnapshot { buddies };
        let payload = serde_json::to_vec_pretty(&snapshot).map_err(|err| {
            NetworkError::generic(format!("Failed to serialise buddy list: {}", err))
        })?;

        self.write_file(self.buddy_file(player_id), payload).await
    }

    /// Load the buddy list for the player from cache or disk.
    pub async fn load_buddy_list(&self, player_id: &str) -> NetworkResult<HashSet<String>> {
        if let Some(cached) = self.cached_buddies.read().await.get(player_id).cloned() {
            return Ok(cached);
        }

        let path = self.buddy_file(player_id);
        match fs::read(&path).await {
            Ok(bytes) => {
                let snapshot: BuddyListSnapshot =
                    serde_json::from_slice(&bytes).map_err(|err| {
                        NetworkError::generic(format!(
                            "Failed to parse buddy list {}: {}",
                            path.display(),
                            err
                        ))
                    })?;
                {
                    let mut cache = self.cached_buddies.write().await;
                    cache.insert(player_id.to_string(), snapshot.buddies.clone());
                }
                Ok(snapshot.buddies)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(HashSet::new()),
            Err(err) => Err(NetworkError::generic(format!(
                "Failed to load buddy list {}: {}",
                path.display(),
                err
            ))),
        }
    }

    /// Persist per-player stats to disk.
    pub async fn save_player_stats(
        &self,
        player_id: &str,
        stats: PlayerRanking,
    ) -> NetworkResult<()> {
        {
            let mut cache = self.cached_stats.write().await;
            cache.insert(player_id.to_string(), stats.clone());
        }

        let payload = serde_json::to_vec_pretty(&PlayerStatsSnapshot { stats }).map_err(|err| {
            NetworkError::generic(format!("Failed to serialise player stats: {}", err))
        })?;

        self.write_file(self.stats_file(player_id), payload).await
    }

    /// Load player stats, returning a default bronze ranking if none exist yet.
    pub async fn load_player_stats(&self, player_id: &str) -> NetworkResult<PlayerRanking> {
        if let Some(cached) = self.cached_stats.read().await.get(player_id).cloned() {
            return Ok(cached);
        }

        let path = self.stats_file(player_id);
        match fs::read(&path).await {
            Ok(bytes) => {
                let snapshot: PlayerStatsSnapshot =
                    serde_json::from_slice(&bytes).map_err(|err| {
                        NetworkError::generic(format!(
                            "Failed to parse player stats {}: {}",
                            path.display(),
                            err
                        ))
                    })?;
                {
                    let mut cache = self.cached_stats.write().await;
                    cache.insert(player_id.to_string(), snapshot.stats.clone());
                }
                Ok(snapshot.stats)
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let default = PlayerRanking {
                    player_id: player_id.to_string(),
                    points: 1000,
                    rank: RankTier::Bronze,
                    games_played: 0,
                    games_won: 0,
                    win_streak: 0,
                    best_win_streak: 0,
                    last_activity: chrono::Utc::now(),
                };
                Ok(default)
            }
            Err(err) => Err(NetworkError::generic(format!(
                "Failed to load player stats {}: {}",
                path.display(),
                err
            ))),
        }
    }

    /// Load all player stats currently stored on disk.
    pub async fn load_all_player_stats(&self) -> NetworkResult<Vec<PlayerRanking>> {
        let mut results = Vec::new();
        let mut read_dir = fs::read_dir(&self.stats_dir).await.map_err(|err| {
            NetworkError::generic(format!(
                "Failed to read stats directory {}: {}",
                self.stats_dir.display(),
                err
            ))
        })?;

        while let Some(entry) = read_dir.next_entry().await.map_err(|err| {
            NetworkError::generic(format!(
                "Failed to iterate stats directory {}: {}",
                self.stats_dir.display(),
                err
            ))
        })? {
            if entry
                .file_type()
                .await
                .map(|ft| ft.is_file())
                .unwrap_or(false)
            {
                match fs::read(entry.path()).await {
                    Ok(bytes) => match serde_json::from_slice::<PlayerStatsSnapshot>(&bytes) {
                        Ok(snapshot) => {
                            let mut cache = self.cached_stats.write().await;
                            cache.insert(snapshot.stats.player_id.clone(), snapshot.stats.clone());
                            results.push(snapshot.stats);
                        }
                        Err(err) => warn!(
                            "Failed to parse player stats file {}: {}",
                            entry.path().display(),
                            err
                        ),
                    },
                    Err(err) => warn!(
                        "Failed to read player stats file {}: {}",
                        entry.path().display(),
                        err
                    ),
                }
            }
        }

        Ok(results)
    }

    async fn write_file(&self, path: PathBuf, payload: Vec<u8>) -> NetworkResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|err| {
                NetworkError::generic(format!(
                    "Failed to ensure directory {}: {}",
                    parent.display(),
                    err
                ))
            })?;
        }

        let mut file = fs::File::create(&path).await.map_err(|err| {
            NetworkError::generic(format!("Failed to create {}: {}", path.display(), err))
        })?;
        file.write_all(&payload).await.map_err(|err| {
            NetworkError::generic(format!("Failed to write {}: {}", path.display(), err))
        })?;
        file.flush().await.map_err(|err| {
            NetworkError::generic(format!("Failed to flush {}: {}", path.display(), err))
        })?;
        debug!("Persisted {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_buddy_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let storage = PersistentStorage::new(tmp.path()).await.unwrap();

        let mut buddies = HashSet::new();
        buddies.insert("Alice".to_string());
        buddies.insert("Bob".to_string());

        storage
            .save_buddy_list("player1", buddies.clone())
            .await
            .unwrap();

        let loaded = storage.load_buddy_list("player1").await.unwrap();
        assert_eq!(loaded, buddies);
    }

    #[tokio::test]
    async fn test_stats_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let storage = PersistentStorage::new(tmp.path()).await.unwrap();

        let stats = PlayerRanking {
            player_id: "player1".to_string(),
            points: 1800,
            rank: RankTier::Gold,
            games_played: 10,
            games_won: 8,
            win_streak: 4,
            best_win_streak: 5,
            last_activity: chrono::Utc::now(),
        };

        storage
            .save_player_stats("player1", stats.clone())
            .await
            .unwrap();

        let loaded = storage.load_player_stats("player1").await.unwrap();
        assert_eq!(loaded.points, stats.points);
        assert_eq!(loaded.rank, stats.rank);

        let all = storage.load_all_player_stats().await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].player_id, "player1");
    }
}
