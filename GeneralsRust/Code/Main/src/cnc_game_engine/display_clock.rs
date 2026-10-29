//! Instance-owned W3DDisplay::draw admission, independent of wall-clock pacing.
//! Source: W3DDisplay.cpp:1730-1855 and W3DView.cpp:1344-1368.

const FRAME_LENGTH_MS: u32 = 33;

#[derive(Debug, Clone, Copy)]
pub(super) struct DisplayInputs {
    pub client_frame: u32,
    pub frozen: bool,
    pub time_fast: bool,
    pub multiplier: i32,
    /// Source cooperative-device test for CPU view/particle work.
    pub device_ready: bool,
    /// Effective TiVO replay mode, already excluding a paused game.
    pub tivo_replay: bool,
    pub replay_frame: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(super) struct DisplayAdmission {
    pub sync_time_ms: u32,
    pub sync: bool,
    pub frame_time_ms: u32,
    pub update_views: bool,
    pub update_particles: bool,
    pub draw_drawables: bool,
    /// GPU attempt eligibility; Begin_Render/device acquisition remains fallible.
    pub draw_gpu: bool,
    pub fast_camera_only: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct VisualClockContinuation {
    sync_ms: u32,
    previous_sync_ms: u32,
    last_client_frame: u32,
    time_multiplier_counter: i32,
}

pub(super) struct VisualClock {
    sync_ms: u32,
    previous_sync_ms: u32,
    last_client_frame: u32,
    time_multiplier_counter: i32,
}

impl VisualClock {
    pub(super) const fn new() -> Self {
        Self {
            sync_ms: 0,
            previous_sync_ms: 0,
            last_client_frame: u32::MAX,
            time_multiplier_counter: 1,
        }
    }

    pub(super) fn capture_continuation(&self) -> VisualClockContinuation {
        VisualClockContinuation {
            sync_ms: self.sync_ms,
            previous_sync_ms: self.previous_sync_ms,
            last_client_frame: self.last_client_frame,
            time_multiplier_counter: self.time_multiplier_counter,
        }
    }

    pub(super) fn validate_continuation(state: &VisualClockContinuation) -> Result<(), String> {
        if state.time_multiplier_counter < 1 {
            return Err("invalid camera display multiplier counter".into());
        }
        Ok(())
    }

    pub(super) fn admit_continuation(
        &mut self,
        state: VisualClockContinuation,
        saved_client_frame: u32,
        restored_client_frame: u32,
    ) {
        self.sync_ms = state.sync_ms;
        self.previous_sync_ms = state.previous_sync_ms;
        self.last_client_frame = restored_client_frame
            .wrapping_add(state.last_client_frame.wrapping_sub(saved_client_frame));
        self.time_multiplier_counter = state.time_multiplier_counter;
    }

    pub(super) fn admit(&mut self, input: DisplayInputs) -> DisplayAdmission {
        let frozen = input.frozen || self.last_client_frame == input.client_frame;
        self.last_client_frame = input.client_frame;
        if !frozen {
            self.sync_ms = self.sync_ms.wrapping_add(FRAME_LENGTH_MS);
        }
        let mut admission = DisplayAdmission {
            sync_time_ms: self.sync_ms,
            sync: false,
            frame_time_ms: 0,
            update_views: false,
            update_particles: false,
            draw_drawables: false,
            draw_gpu: false,
            fast_camera_only: false,
        };
        // Fast time returns before WW3D::Sync and before multiplier admission.
        if !frozen && input.time_fast {
            admission.fast_camera_only = true;
            return admission;
        }
        admission.sync = true;
        admission.frame_time_ms = self.sync_ms.wrapping_sub(self.previous_sync_ms);
        self.previous_sync_ms = self.sync_ms;
        if input.multiplier > 1 {
            self.time_multiplier_counter -= 1;
            if self.time_multiplier_counter > 1 {
                return admission;
            }
            self.time_multiplier_counter = input.multiplier;
        }
        admission.update_views = input.device_ready;
        admission.update_particles = input.device_ready;
        // W3DView still updates camera before rejecting fast-time Drawable work.
        admission.draw_drawables =
            admission.update_views && !input.time_fast && admission.frame_time_ms != 0;
        // TiVO is checked after updateViews/particles. Cooperative readiness is
        // a separate earlier test, not the source Begin_Render result.
        admission.draw_gpu = !input.tivo_replay || input.replay_frame % 30 == 1;
        admission
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(frame: u32) -> DisplayInputs {
        DisplayInputs {
            client_frame: frame,
            frozen: false,
            time_fast: false,
            multiplier: 1,
            device_ready: true,
            tivo_replay: false,
            replay_frame: frame,
        }
    }

    #[test]
    fn continuation_preserves_repeat_fast_and_multiplier_history_after_frame_rebase() {
        let mut original = VisualClock::new();
        let mut first = input(u32::MAX - 1);
        first.multiplier = 4;
        original.admit(first);
        let state = original.capture_continuation();
        VisualClock::validate_continuation(&state).unwrap();
        let mut restored = VisualClock::new();
        restored.admit_continuation(state, first.client_frame, 100);
        for (offset, fast, ready) in [
            (0, false, true),
            (1, true, true),
            (2, false, false),
            (3, false, true),
            (4, false, true),
        ] {
            let mut a = input(first.client_frame.wrapping_add(offset));
            a.multiplier = 4;
            a.time_fast = fast;
            a.device_ready = ready;
            let mut b = a;
            b.client_frame = 100_u32.wrapping_add(offset);
            assert_eq!(original.admit(a), restored.admit(b));
        }
    }

    #[test]
    fn ordinary_repeat_and_frozen_offers_keep_source_clock_order() {
        let mut clock = VisualClock::new();
        let first = clock.admit(input(1));
        assert_eq!((first.sync_time_ms, first.frame_time_ms), (33, 33));
        assert!(first.sync && first.draw_drawables);
        let repeated = clock.admit(input(1));
        assert_eq!(repeated.frame_time_ms, 0);
        assert!(!repeated.draw_drawables);
        assert!(repeated.update_views && repeated.update_particles && repeated.draw_gpu);
        let frozen = clock.admit(DisplayInputs {
            frozen: true,
            ..input(2)
        });
        assert_eq!(frozen.sync_time_ms, 33);
        assert!(!frozen.draw_drawables);
        assert!(
            !clock.admit(input(2)).draw_drawables,
            "frozen offer still records last frame"
        );
        assert!(clock.admit(input(3)).draw_drawables);
    }

    #[test]
    fn fast_time_defers_sync_and_frozen_exit_consumes_accumulated_delta() {
        let mut clock = VisualClock::new();
        for frame in 1..=2 {
            let offered = clock.admit(DisplayInputs {
                time_fast: true,
                multiplier: 4,
                ..input(frame)
            });
            assert!(offered.fast_camera_only);
            assert!(
                !offered.sync
                    && !offered.draw_drawables
                    && !offered.update_particles
                    && !offered.draw_gpu
            );
            assert_eq!(offered.sync_time_ms, frame * 33);
        }
        let exit = clock.admit(DisplayInputs {
            frozen: true,
            multiplier: 4,
            ..input(3)
        });
        assert!(exit.sync && exit.draw_drawables);
        assert_eq!(exit.frame_time_ms, 66);
        assert_eq!(
            clock.time_multiplier_counter, 4,
            "fast early returns never change counter"
        );
        let next = clock.admit(DisplayInputs {
            multiplier: 4,
            ..input(4)
        });
        assert_eq!(next.frame_time_ms, 33);
        assert!(!next.update_views);
    }

    #[test]
    fn repeated_exit_from_fast_can_draw_without_new_client_frame() {
        let mut clock = VisualClock::new();
        clock.admit(DisplayInputs {
            time_fast: true,
            ..input(5)
        });
        let exit = clock.admit(input(5));
        assert_eq!(exit.frame_time_ms, 33);
        assert!(exit.draw_drawables);
        assert!(!clock.admit(input(5)).draw_drawables);
    }

    #[test]
    fn frozen_fast_time_syncs_and_updates_views_but_never_drawables() {
        let mut clock = VisualClock::new();
        clock.admit(DisplayInputs {
            time_fast: true,
            ..input(1)
        });
        let frozen = clock.admit(DisplayInputs {
            frozen: true,
            time_fast: true,
            ..input(2)
        });
        assert!(frozen.sync && frozen.update_views && frozen.update_particles);
        assert_eq!(frozen.frame_time_ms, 33);
        assert!(!frozen.fast_camera_only && !frozen.draw_drawables);
        assert!(frozen.draw_gpu);
    }

    #[test]
    fn multiplier_two_three_four_match_original_counter() {
        for (multiplier, expected) in [
            (2, vec![true, true, true, true, true, true, true]),
            (3, vec![true, false, true, false, true, false, true]),
            (4, vec![true, false, false, true, false, false, true]),
        ] {
            let mut clock = VisualClock::new();
            let actual: Vec<_> = (1..=7)
                .map(|frame| {
                    let offered = clock.admit(DisplayInputs {
                        multiplier,
                        ..input(frame)
                    });
                    assert!(offered.sync);
                    assert_eq!(offered.frame_time_ms, 33);
                    offered.draw_drawables
                })
                .collect();
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn multiplier_counter_survives_normal_speed_and_throttled_sync() {
        let mut clock = VisualClock::new();
        assert!(
            clock
                .admit(DisplayInputs {
                    multiplier: 4,
                    ..input(1)
                })
                .draw_drawables
        );
        assert!(
            !clock
                .admit(DisplayInputs {
                    multiplier: 4,
                    ..input(2)
                })
                .draw_drawables
        );
        assert!(clock.admit(input(3)).draw_drawables);
        assert!(
            !clock
                .admit(DisplayInputs {
                    multiplier: 4,
                    ..input(4)
                })
                .draw_drawables
        );
        let frozen = clock.admit(DisplayInputs {
            frozen: true,
            multiplier: 4,
            ..input(5)
        });
        assert!(frozen.update_views);
        assert_eq!(
            frozen.frame_time_ms, 0,
            "throttled offers still perform Sync"
        );
        assert!(!frozen.draw_drawables);
    }

    #[test]
    fn zero_and_negative_view_multipliers_keep_raw_value_without_throttling() {
        for multiplier in [0, -2] {
            let mut clock = VisualClock::new();
            for frame in 1..=3 {
                let mut offered = input(frame);
                offered.multiplier = multiplier;
                let admission = clock.admit(offered);
                assert!(admission.update_views);
                assert!(admission.draw_drawables);
                assert_eq!(clock.time_multiplier_counter, 1);
            }
        }
    }

    #[test]
    fn switching_below_one_resets_the_deadline_but_retains_cpp_counter() {
        let mut clock = VisualClock::new();
        assert!(
            clock
                .admit(DisplayInputs {
                    multiplier: 4,
                    ..input(1)
                })
                .draw_drawables
        );
        assert!(
            !clock
                .admit(DisplayInputs {
                    multiplier: 4,
                    ..input(2)
                })
                .draw_drawables
        );
        assert!(
            clock
                .admit(DisplayInputs {
                    multiplier: 0,
                    ..input(3)
                })
                .draw_drawables
        );
        assert!(
            clock
                .admit(DisplayInputs {
                    multiplier: -2,
                    ..input(4)
                })
                .draw_drawables
        );
        // C++ W3DDisplay.cpp:1788-1800 skips the static throttle counter in
        // the <=1 branch, then resumes its previous counter when speed rises.
        let continuation = clock.capture_continuation();
        VisualClock::validate_continuation(&continuation).unwrap();
        let mut restored = VisualClock::new();
        restored.admit_continuation(continuation, 4, 100);
        for (frame, restored_frame, multiplier) in [(5, 101, 4), (6, 102, 4), (7, 103, 4)] {
            let source = clock.admit(DisplayInputs {
                multiplier,
                ..input(frame)
            });
            let resumed = restored.admit(DisplayInputs {
                multiplier,
                ..input(restored_frame)
            });
            assert_eq!(source, resumed);
        }
    }

    #[test]
    fn tivo_skips_only_gpu_and_device_readiness_is_a_separate_phase() {
        let mut clock = VisualClock::new();
        let tivo = clock.admit(DisplayInputs {
            tivo_replay: true,
            replay_frame: 2,
            ..input(1)
        });
        assert!(tivo.update_views && tivo.update_particles && tivo.draw_drawables);
        assert!(!tivo.draw_gpu);
        let device = clock.admit(DisplayInputs {
            device_ready: false,
            ..input(2)
        });
        assert!(device.sync);
        assert!(!device.update_views && !device.update_particles && !device.draw_drawables);
        assert!(
            device.draw_gpu,
            "cooperative test does not replace Begin_Render eligibility"
        );
        assert!(
            clock
                .admit(DisplayInputs {
                    tivo_replay: true,
                    replay_frame: 31,
                    ..input(3)
                })
                .draw_gpu
        );
    }

    #[test]
    fn virtual_sync_wraps_and_independent_instances_do_not_interfere() {
        let mut first = VisualClock::new();
        let mut peer = VisualClock::new();
        first.sync_ms = u32::MAX - 10;
        first.previous_sync_ms = first.sync_ms;
        let wrapped = first.admit(input(1));
        assert_eq!((wrapped.sync_time_ms, wrapped.frame_time_ms), (22, 33));
        peer.admit(DisplayInputs {
            time_fast: true,
            ..input(1)
        });
        peer.admit(DisplayInputs {
            multiplier: 4,
            ..input(2)
        });
        let next = first.admit(input(2));
        assert_eq!((next.sync_time_ms, next.frame_time_ms), (55, 33));
        assert!(next.draw_drawables);
        let initial_max = VisualClock::new().admit(input(u32::MAX));
        assert_eq!(
            initial_max.frame_time_ms, 0,
            "source initializes lastFrame to unsigned MAX"
        );
    }
}
