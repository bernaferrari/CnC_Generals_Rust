use crate::game_logic::victory::VictorySummary;
use anyhow::{Result, anyhow};
use std::collections::VecDeque;

/// FIFO of offline end-of-game summaries owned by one engine subsystem.
#[derive(Debug, Default)]
pub struct GameResultsQueue {
    initialized: bool,
    queue: VecDeque<VictorySummary>,
}

impl GameResultsQueue {
    pub fn new() -> Self {
        Self {
            initialized: false,
            queue: VecDeque::new(),
        }
    }

    pub fn init(&mut self) {
        self.initialized = true;
        self.queue.clear();
    }

    pub fn reset(&mut self) {
        // C++ parity: subsystem reset is a no-op for the results queue.
    }

    pub fn shutdown(&mut self) {
        self.queue.clear();
        self.initialized = false;
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn peek(&self) -> Result<Option<&VictorySummary>> {
        self.ensure_initialized()?;
        Ok(self.queue.front())
    }

    pub fn enqueue(&mut self, summary: VictorySummary) -> Result<()> {
        self.ensure_initialized()?;
        self.queue.push_back(summary);
        Ok(())
    }

    pub fn dequeue(&mut self) -> Result<Option<VictorySummary>> {
        self.ensure_initialized()?;
        Ok(self.queue.pop_front())
    }

    pub fn drain(&mut self) -> Result<Vec<VictorySummary>> {
        self.ensure_initialized()?;
        Ok(self.queue.drain(..).collect())
    }

    fn ensure_initialized(&self) -> Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(anyhow!("GameResultsQueue has not been initialized"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn summary(name: &str) -> VictorySummary {
        let mut value = VictorySummary::new();
        value.mission_name = Some(name.into());
        value
    }
    #[test]
    fn game_results_queue_preserves_fifo_order() {
        let mut queue = GameResultsQueue::new();
        queue.init();
        queue.enqueue(summary("First")).unwrap();
        queue.enqueue(summary("Second")).unwrap();
        assert_eq!(
            queue.dequeue().unwrap().unwrap().mission_name.as_deref(),
            Some("First")
        );
        assert_eq!(
            queue.dequeue().unwrap().unwrap().mission_name.as_deref(),
            Some("Second")
        );
        assert!(queue.dequeue().unwrap().is_none());
    }
    #[test]
    fn game_results_queue_reset_preserves_pending_entries() {
        let mut queue = GameResultsQueue::new();
        queue.init();
        queue.enqueue(summary("Reset Me")).unwrap();
        queue.reset();
        assert_eq!(queue.len(), 1);
        assert!(!queue.is_empty());
        assert_eq!(
            queue.peek().unwrap().unwrap().mission_name.as_deref(),
            Some("Reset Me")
        );
    }
    #[test]
    fn game_results_queue_shutdown_disables_enqueue_until_reinit() {
        let mut queue = GameResultsQueue::new();
        assert!(queue.enqueue(summary("Before Init")).is_err());
        queue.init();
        queue.enqueue(summary("Before Shutdown")).unwrap();
        queue.shutdown();
        assert!(!queue.is_initialized());
        assert!(queue.enqueue(summary("After Shutdown")).is_err());
        assert!(queue.dequeue().is_err());
        queue.init();
        assert!(queue.is_empty());
        queue.enqueue(summary("After Reinit")).unwrap();
        assert_eq!(
            queue.dequeue().unwrap().unwrap().mission_name.as_deref(),
            Some("After Reinit")
        );
    }
    #[test]
    fn peer_lifecycle_never_changes_pending_results() {
        let mut first = GameResultsQueue::new();
        first.init();
        first.enqueue(summary("First")).unwrap();
        let mut peer = GameResultsQueue::new();
        peer.init();
        peer.enqueue(summary("Peer")).unwrap();
        peer.reset();
        peer.shutdown();
        peer.init();
        drop(peer);
        assert_eq!(
            first.dequeue().unwrap().unwrap().mission_name.as_deref(),
            Some("First")
        );
    }
}
