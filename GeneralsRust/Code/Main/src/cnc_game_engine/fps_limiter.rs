//! Per-engine counterpart of C++ GameEngine::m_useFpsLimit / m_maxFPS.
/// Configured application defaults are captured once after startup config
/// admission. Runtime mutations never write back to Common GlobalData.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct FpsLimiterState {
    configured_rate: i32,
    enabled: bool,
    rate: i32,
}

pub(super) struct StartupFpsLimitRequest {
    pub(super) requested: i32,
    pub(super) reply: std::sync::mpsc::SyncSender<i32>,
}

impl FpsLimiterState {
    pub(crate) fn new(configured_rate: i32, configured_enabled: bool) -> Self {
        Self {
            configured_rate,
            enabled: configured_enabled,
            rate: configured_rate,
        }
    }

    pub(crate) fn enabled(self) -> bool {
        self.enabled
    }
    pub(crate) fn rate(self) -> i32 {
        self.rate
    }
    pub(crate) fn configured_rate(self) -> i32 {
        self.configured_rate
    }

    /// ScriptActions.cpp: SET_FPS_LIMIT uses configured rate only for zero.
    pub(crate) fn apply_script(&mut self, requested: i32) {
        self.rate = if requested == 0 {
            self.configured_rate
        } else {
            requested
        };
        self.enabled = true;
    }

    /// GameLogicDispatch.cpp: invalid NewGame rate falls back to configured FPS.
    pub(crate) fn apply_new_game(&mut self, requested: i32) -> i32 {
        self.rate = if (1..=1000).contains(&requested) {
            requested
        } else {
            self.configured_rate
        };
        self.enabled = true;
        self.rate
    }

    /// ScriptEngine::reset restores the configured rate but leaves use enabled.
    pub(crate) fn reset_game(&mut self) {
        self.rate = self.configured_rate;
    }

    pub(crate) fn restore_continuation(&mut self, enabled: bool, rate: i32) {
        self.enabled = enabled;
        self.rate = rate;
    }

    pub(crate) fn rollback_startup(&mut self, previous: Option<Self>) -> bool {
        if let Some(previous) = previous {
            *self = previous;
            true
        } else {
            false
        }
    }

    pub(crate) fn restore_legacy_camera_rate(
        &mut self,
        legacy_rate: Option<u32>,
    ) -> Result<(), String> {
        match legacy_rate {
            Some(rate) => {
                let rate = i32::try_from(rate)
                    .map_err(|_| "legacy camera FPS rate exceeds signed engine range")?;
                self.restore_continuation(true, rate);
            }
            None => self.rate = self.configured_rate,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FpsLimiterState, StartupFpsLimitRequest};

    #[test]
    fn engines_isolate_script_reset_and_new_game_rates() {
        let mut first = FpsLimiterState::new(37, false);
        let mut peer = FpsLimiterState::new(59, true);
        first.apply_script(0);
        assert_eq!((first.enabled(), first.rate()), (true, 37));
        assert_eq!((peer.enabled(), peer.rate()), (true, 59));
        first.apply_script(-4);
        assert_eq!(first.rate(), -4);
        first.reset_game();
        assert_eq!((first.enabled(), first.rate()), (true, 37));
        assert_eq!(peer.rate(), 59);
        assert_eq!(peer.apply_new_game(1001), 59);
        assert_eq!(peer.apply_new_game(72), 72);
        assert_eq!(first.configured_rate(), 37);
        first.restore_continuation(false, -12);
        assert_eq!((first.enabled(), first.rate()), (false, -12));
        first.reset_game();
        assert_eq!((first.enabled(), first.rate()), (false, 37));
        first
            .restore_legacy_camera_rate(Some(24))
            .expect("positive legacy rate fits engine range");
        assert_eq!((first.enabled(), first.rate()), (true, 24));
        first.reset_game();
        assert_eq!((first.enabled(), first.rate()), (true, 37));
        first.apply_script(24);
        first
            .restore_legacy_camera_rate(None)
            .expect("legacy none clears old override");
        assert_eq!((first.enabled(), first.rate()), (true, 37));
        first.restore_continuation(false, 19);
        first.restore_legacy_camera_rate(None).unwrap();
        assert_eq!((first.enabled(), first.rate()), (false, 37));
        let before = (first.enabled(), first.rate());
        assert!(first.restore_legacy_camera_rate(Some(u32::MAX)).is_err());
        assert_eq!((first.enabled(), first.rate()), before);
    }

    #[test]
    fn startup_replies_are_bound_to_the_receiving_owner_channel() {
        use std::sync::mpsc;
        use std::time::Duration;

        let mut first = FpsLimiterState::new(37, false);
        let mut peer = FpsLimiterState::new(59, true);
        let (first_messages, first_receiver) = mpsc::channel();
        let (peer_messages, peer_receiver) = mpsc::channel();
        let first_worker = std::thread::spawn(move || {
            let (reply, response) = mpsc::sync_channel(0);
            first_messages
                .send(StartupFpsLimitRequest {
                    requested: 1001,
                    reply,
                })
                .unwrap();
            response.recv_timeout(Duration::from_secs(1)).unwrap()
        });
        let peer_worker = std::thread::spawn(move || {
            let (reply, response) = mpsc::sync_channel(0);
            peer_messages
                .send(StartupFpsLimitRequest {
                    requested: 72,
                    reply,
                })
                .unwrap();
            response.recv_timeout(Duration::from_secs(1)).unwrap()
        });
        let peer_request = peer_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        peer_request
            .reply
            .send(peer.apply_new_game(peer_request.requested))
            .unwrap();
        let first_request = first_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        first_request
            .reply
            .send(first.apply_new_game(first_request.requested))
            .unwrap();
        assert_eq!(first_worker.join().unwrap(), 37);
        assert_eq!(peer_worker.join().unwrap(), 72);
        assert_eq!((first.rate(), peer.rate()), (37, 72));
    }

    #[test]
    fn dropped_startup_reply_unblocks_worker_without_fallback() {
        use std::sync::mpsc;
        use std::time::Duration;

        let (messages, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let (reply, response) = mpsc::sync_channel(0);
            messages
                .send(StartupFpsLimitRequest {
                    requested: 11,
                    reply,
                })
                .unwrap();
            response.recv_timeout(Duration::from_millis(250))
        });
        let request = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(request);
        assert!(worker.join().unwrap().is_err());
    }

    #[test]
    fn abandoning_startup_restores_only_its_pending_limiter_mutation() {
        let mut receiver = FpsLimiterState::new(41, false);
        let previous = receiver;
        receiver.apply_new_game(90);
        assert_eq!((receiver.enabled(), receiver.rate()), (true, 90));
        assert!(receiver.rollback_startup(Some(previous)));
        assert_eq!((receiver.enabled(), receiver.rate()), (false, 41));
        assert!(!receiver.rollback_startup(None));
    }
}
