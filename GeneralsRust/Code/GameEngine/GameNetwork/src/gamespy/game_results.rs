//! GameSpy Game Results
//! Tracks and reports game outcomes and statistics

use crate::error::NetworkResult;
use crate::gamespy::GameResults;
use tracing::info;

/// Game result log owned by the GameSpy interface. Only the owning task's
/// methods touch it, so it is a plain field with no shared lock.
pub struct GameResultsSystem {
    results: Vec<GameResults>,
}

impl GameResultsSystem {
    pub async fn new() -> NetworkResult<Self> {
        Ok(Self {
            results: Vec::new(),
        })
    }

    pub async fn report_results(&mut self, results: GameResults) -> NetworkResult<()> {
        info!("Reported game results for: {}", results.game_id);
        self.results.push(results);
        Ok(())
    }
}
