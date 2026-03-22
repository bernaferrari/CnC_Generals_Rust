use crate::game_logic::victory::VictorySummary;
use anyhow::{anyhow, Result};
use log::warn;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

static GAME_RESULTS_QUEUE: OnceLock<Arc<Mutex<GameResultsQueue>>> = OnceLock::new();

/// Thread-safe FIFO queue for end-of-game summaries.
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

pub fn init_game_results_queue() -> Result<Arc<Mutex<GameResultsQueue>>> {
    let queue = GAME_RESULTS_QUEUE
        .get_or_init(|| Arc::new(Mutex::new(GameResultsQueue::new())))
        .clone();
    {
        let mut guard = queue
            .lock()
            .map_err(|err| anyhow!("Failed to lock GameResultsQueue during init: {err}"))?;
        guard.init();
    }
    Ok(queue)
}

pub fn get_game_results_queue() -> Option<Arc<Mutex<GameResultsQueue>>> {
    GAME_RESULTS_QUEUE.get().cloned()
}

pub fn queue_victory_summary(summary: VictorySummary) -> Result<()> {
    let Some(queue) = get_game_results_queue() else {
        return Err(anyhow!("GameResultsQueue is not initialized"));
    };
    let mut guard = queue
        .lock()
        .map_err(|err| anyhow!("Failed to lock GameResultsQueue: {err}"))?;
    guard.enqueue(summary)
}

pub fn dequeue_victory_summary() -> Result<Option<VictorySummary>> {
    let Some(queue) = get_game_results_queue() else {
        return Err(anyhow!("GameResultsQueue is not initialized"));
    };
    let mut guard = queue
        .lock()
        .map_err(|err| anyhow!("Failed to lock GameResultsQueue: {err}"))?;
    guard.dequeue()
}

pub fn reset_game_results_queue() -> Result<()> {
    let Some(queue) = get_game_results_queue() else {
        return Ok(());
    };
    let _guard = queue
        .lock()
        .map_err(|err| anyhow!("Failed to lock GameResultsQueue: {err}"))?;
    Ok(())
}

pub fn shutdown_game_results_queue() -> Result<()> {
    let Some(queue) = get_game_results_queue() else {
        return Ok(());
    };
    let mut guard = queue
        .lock()
        .map_err(|err| anyhow!("Failed to lock GameResultsQueue: {err}"))?;
    guard.shutdown();
    Ok(())
}

pub fn queue_len() -> Option<usize> {
    let queue = get_game_results_queue()?;
    let guard = queue.lock().ok()?;
    Some(guard.len())
}

pub fn queue_is_empty() -> Option<bool> {
    let queue = get_game_results_queue()?;
    let guard = queue.lock().ok()?;
    Some(guard.is_empty())
}

pub fn log_queue_unavailable(context: &str) {
    warn!("GameResultsQueue unavailable while {context}");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_summary(name: &str) -> VictorySummary {
        let mut summary = VictorySummary::new();
        summary.mission_name = Some(name.to_string());
        summary
    }

    fn reset_singleton() {
        if let Some(queue) = get_game_results_queue() {
            if let Ok(mut guard) = queue.lock() {
                guard.shutdown();
            }
        }
        let _ = reset_game_results_queue();
    }

    #[test]
    fn game_results_queue_preserves_fifo_order() {
        reset_singleton();
        let queue = init_game_results_queue().unwrap();

        {
            let mut guard = queue.lock().unwrap();
            guard.enqueue(sample_summary("First")).unwrap();
            guard.enqueue(sample_summary("Second")).unwrap();
        }

        let first = dequeue_victory_summary().unwrap().unwrap();
        let second = dequeue_victory_summary().unwrap().unwrap();

        assert_eq!(first.mission_name.as_deref(), Some("First"));
        assert_eq!(second.mission_name.as_deref(), Some("Second"));
    }

    #[test]
    fn game_results_queue_reset_preserves_pending_entries() {
        reset_singleton();
        let queue = init_game_results_queue().unwrap();

        {
            let mut guard = queue.lock().unwrap();
            guard.enqueue(sample_summary("Reset Me")).unwrap();
            assert_eq!(guard.len(), 1);
        }

        reset_game_results_queue().unwrap();

        {
            let guard = queue.lock().unwrap();
            assert_eq!(guard.len(), 1);
            assert!(!guard.is_empty());
            assert_eq!(
                guard.peek().unwrap().unwrap().mission_name.as_deref(),
                Some("Reset Me")
            );
        }
    }

    #[test]
    fn game_results_queue_shutdown_disables_enqueue_until_reinit() {
        reset_singleton();
        let queue = init_game_results_queue().unwrap();

        {
            let mut guard = queue.lock().unwrap();
            guard.enqueue(sample_summary("Before Shutdown")).unwrap();
        }

        shutdown_game_results_queue().unwrap();

        {
            let mut guard = queue.lock().unwrap();
            assert!(!guard.is_initialized());
            assert!(guard.enqueue(sample_summary("After Shutdown")).is_err());
        }

        init_game_results_queue().unwrap();

        {
            let mut guard = queue.lock().unwrap();
            guard.enqueue(sample_summary("After Reinit")).unwrap();
            assert_eq!(
                guard.dequeue().unwrap().unwrap().mission_name.as_deref(),
                Some("After Reinit")
            );
        }
    }
}
