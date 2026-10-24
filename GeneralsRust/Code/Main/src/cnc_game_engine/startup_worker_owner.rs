//! Cancellation belongs to one asynchronous startup request, not the process.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct StartupWorkerOwner(Arc<AtomicBool>);
pub(super) struct StartupWorkerSignal(Arc<AtomicBool>);

pub(super) fn worker_pair() -> (StartupWorkerOwner, StartupWorkerSignal) {
    let cancelled = Arc::new(AtomicBool::new(false));
    (
        StartupWorkerOwner(Arc::clone(&cancelled)),
        StartupWorkerSignal(cancelled),
    )
}
impl Drop for StartupWorkerOwner {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}
impl StartupWorkerSignal {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.0.load(Ordering::Acquire) {
            Err("startup worker abandoned before admitted definition work".into())
        } else {
            Ok(())
        }
    }
}

/// Original initial-file .map payload: single player, normal difficulty, rank 0.
/// It joins only this candidate's Recorder messages, in original append order.
pub(super) fn initial_file_message() -> game_engine::common::message_stream::GameMessage {
    use game_engine::common::message_stream::{GameMessage, GameMessageType};
    let mut message = GameMessage::new(GameMessageType::NewGame);
    for argument in [0, 1, 0] {
        message.append_integer_argument(argument);
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc, time::Duration};

    #[test]
    fn dropping_failed_peer_startup_never_cancels_retained_request() {
        let (owner, signal) = worker_pair();
        {
            let (peer, peer_signal) = worker_pair();
            assert!(signal.validate().is_ok());
            assert!(peer_signal.validate().is_ok());
            drop(peer);
            assert!(peer_signal.validate().is_err());
            assert!(signal.validate().is_ok());
        }
        let (replacement, replacement_signal) = worker_pair();
        assert!(replacement_signal.validate().is_ok());
        drop(replacement);
        assert!(replacement_signal.validate().is_err());
        assert!(signal.validate().is_ok());
        drop(owner);
        assert!(signal.validate().is_err());
        let (fresh_owner, fresh_signal) = worker_pair();
        assert!(fresh_signal.validate().is_ok());
        drop(fresh_owner);
        assert!(fresh_signal.validate().is_err());
    }

    #[test]
    fn actual_startup_state_retains_its_signal_until_its_own_replacement() {
        use super::super::StartupLoadState;
        use std::time::Instant;
        let state = |owner| {
            let (_, receiver) = mpsc::channel();
            StartupLoadState::InProgress {
                _worker_owner: owner,
                receiver,
                started_at: Instant::now(),
                last_worker_progress: 0.0,
                last_worker_phase: None,
                last_worker_logged_bucket: 0,
            }
        };
        let (a, a_signal) = worker_pair();
        let a = state(a);
        let (b, b_signal) = worker_pair();
        let b = state(b);
        drop(b);
        assert!(b_signal.validate().is_err());
        assert!(a_signal.validate().is_ok());
        let mut a = a;
        assert!(matches!(a, StartupLoadState::InProgress { .. }));
        a = StartupLoadState::Complete;
        assert!(matches!(a, StartupLoadState::Complete));
        assert!(a_signal.validate().is_err());
    }

    #[test]
    fn actual_worker_observes_only_its_own_driving_owner_drop() {
        let (owner, signal) = worker_pair();
        let (peer, peer_signal) = worker_pair();
        let (proceed, next) = mpsc::channel();
        let (finished, result) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            for _ in 0..2 {
                next.recv_timeout(Duration::from_secs(2)).unwrap();
                finished.send(signal.validate().is_ok()).unwrap();
            }
        });
        drop(peer);
        assert!(peer_signal.validate().is_err());
        proceed.send(()).unwrap();
        assert!(result.recv_timeout(Duration::from_secs(2)).unwrap());
        drop(owner);
        proceed.send(()).unwrap();
        assert!(!result.recv_timeout(Duration::from_secs(2)).unwrap());
        worker.join().unwrap();
    }

    #[test]
    fn initial_file_and_replay_batches_keep_cpp_order_without_peer_consumption() {
        use super::super::CnCGameEngine;
        use game_engine::common::message_stream::{GameMessage, GameMessageType};
        let mut own = vec![initial_file_message()];
        let peer = vec![initial_file_message()];
        let initial = CnCGameEngine::startup_new_game_dispatch_from_messages(&own).unwrap();
        assert_eq!(
            (
                initial.game_mode_code,
                initial.difficulty_code,
                initial.rank_points
            ),
            (0, 1, 0)
        );
        assert_eq!(initial.max_fps, None);
        let mut replay = GameMessage::new(GameMessageType::NewGame);
        for argument in [3, 2, 17, 60] {
            replay.append_integer_argument(argument);
        }
        own.push(replay);
        let replay = CnCGameEngine::startup_new_game_dispatch_from_messages(&own).unwrap();
        assert_eq!(
            (
                replay.game_mode_code,
                replay.difficulty_code,
                replay.rank_points
            ),
            (3, 2, 17)
        );
        assert_eq!(replay.max_fps, Some(60));
        own.clear();
        let unchanged = CnCGameEngine::startup_new_game_dispatch_from_messages(&peer).unwrap();
        assert_eq!(
            (
                unchanged.game_mode_code,
                unchanged.difficulty_code,
                unchanged.rank_points
            ),
            (0, 1, 0)
        );
        assert_eq!(peer.len(), 1);
    }
}
