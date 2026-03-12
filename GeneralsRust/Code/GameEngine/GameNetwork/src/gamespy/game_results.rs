//! GameSpy Game Results
//! Tracks and reports game outcomes and statistics

use crate::error::NetworkResult;
use crate::gamespy::GameResults;
use tokio::sync::RwLock;
use tracing::info;

pub struct GameResultsSystem {
    results: RwLock<Vec<GameResults>>,
}

impl GameResultsSystem {
    pub async fn new() -> NetworkResult<Self> {
        Ok(Self {
            results: RwLock::new(Vec::new()),
        })
    }

    pub async fn report_results(&self, results: GameResults) -> NetworkResult<()> {
        info!("Reported game results for: {}", results.game_id);
        let mut results_store = self.results.write().await;
        results_store.push(results);
        Ok(())
    }
}
