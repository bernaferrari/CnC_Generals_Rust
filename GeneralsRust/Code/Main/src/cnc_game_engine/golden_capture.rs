//! Deterministic golden-frame capture (renderer plan R0, "Golden-R").
//!
//! Enabled by `GENERALS_GOLDEN_CAPTURE=<out.png>`. The engine runs normally
//! until the logic frame reaches `GENERALS_GOLDEN_LOGIC_FRAME` (default 90).
//! From then on logic and client updates stop, the WW3D clock is pinned to
//! that logic frame's time, and after `GENERALS_GOLDEN_SETTLE_FRAMES` frozen
//! presents (default 4) the next present is captured to `<out.png>`. The last
//! frame's counters go to `<out>.stats.txt`, then the engine exits.
//! `GENERALS_GOLDEN_STATE=menu` freezes the shell map instead of a match.
//!
//! Logic steps are capped so the freeze lands exactly on the requested frame;
//! with no input and a fixed seed the captured world state is reproducible.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// C++ `LOGICFRAMES_PER_SECOND` = 30.
const LOGIC_FRAME: Duration = Duration::from_nanos(33_333_333);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Logic advances toward the freeze frame.
    Running,
    /// Logic stopped; counting frozen presents before the capture.
    Frozen { presents: u32 },
    /// Screenshot requested; waiting for the PNG to land.
    Capturing { presents: u32 },
    /// Capture written; the engine is exiting.
    Done,
}

/// Golden capture request and progress for one engine instance.
#[derive(Debug, Clone)]
pub(crate) struct GoldenCapture {
    out: PathBuf,
    /// Freeze only in the shell (`menu`, ShellMapMD) or in a match (`ingame`).
    in_shell: bool,
    freeze_logic_frame: u32,
    settle_presents: u32,
    phase: Phase,
    /// Shell logic steps run in Menu state (the shell map has no match frame).
    shell_steps: u32,
}

impl GoldenCapture {
    /// Read the request from the process environment (`None` when disabled).
    pub(crate) fn from_env() -> Option<Self> {
        let out = PathBuf::from(std::env::var_os("GENERALS_GOLDEN_CAPTURE")?);
        let number = |key: &str, default: u32| {
            std::env::var(key)
                .ok()
                .and_then(|value| value.trim().parse().ok())
                .unwrap_or(default)
        };
        let in_shell = std::env::var("GENERALS_GOLDEN_STATE")
            .is_ok_and(|state| state.trim().eq_ignore_ascii_case("menu"));
        Some(Self {
            out,
            in_shell,
            freeze_logic_frame: number("GENERALS_GOLDEN_LOGIC_FRAME", 90),
            settle_presents: number("GENERALS_GOLDEN_SETTLE_FRAMES", 4),
            phase: Phase::Running,
            shell_steps: 0,
        })
    }

    /// True while the world must not advance. After the capture the engine
    /// updates again so its pending `Exiting` transition is processed.
    pub(crate) fn logic_frozen(&self) -> bool {
        matches!(self.phase, Phase::Frozen { .. } | Phase::Capturing { .. })
    }

    /// Logic steps still allowed before the freeze frame (`None` once the
    /// capture is done and the engine runs on to exit).
    pub(crate) fn remaining_logic_steps(&self, logic_frame: u32) -> Option<usize> {
        match self.phase {
            Phase::Running => Some(
                usize::try_from(self.freeze_logic_frame.saturating_sub(logic_frame))
                    .unwrap_or(usize::MAX),
            ),
            Phase::Frozen { .. } | Phase::Capturing { .. } => Some(0),
            Phase::Done => None,
        }
    }

    /// Fixed WW3D clock for the frozen frame: the logic frame's game time.
    pub(crate) fn frozen_clock(&self) -> ww3d_engine::FixedClock {
        ww3d_engine::FixedClock {
            total_time: LOGIC_FRAME * self.freeze_logic_frame,
            delta_time: LOGIC_FRAME,
        }
    }

    /// Freeze when the logic frame has been reached. Returns true on the
    /// transition so the caller can pin the render clock once.
    pub(crate) fn observe_logic_frame(&mut self, logic_frame: u32) -> bool {
        if self.phase == Phase::Running && logic_frame >= self.freeze_logic_frame {
            log::info!(
                "golden capture: logic frozen at frame {logic_frame} (requested {})",
                self.freeze_logic_frame
            );
            self.phase = Phase::Frozen { presents: 0 };
            return true;
        }
        false
    }

    /// Advance after one present. Returns what the engine must do next.
    pub(crate) fn after_present(&mut self) -> GoldenAction {
        match self.phase {
            Phase::Running | Phase::Done => GoldenAction::None,
            Phase::Frozen { presents } => {
                let presents = presents + 1;
                if presents >= self.settle_presents {
                    let _ = std::fs::remove_file(&self.out);
                    self.phase = Phase::Capturing { presents: 0 };
                    GoldenAction::RequestCapture(self.out.clone())
                } else {
                    self.phase = Phase::Frozen { presents };
                    GoldenAction::None
                }
            }
            Phase::Capturing { presents } => {
                if self.out.is_file() || presents > 40 {
                    self.phase = Phase::Done;
                    GoldenAction::Finish(stats_path(&self.out))
                } else {
                    self.phase = Phase::Capturing {
                        presents: presents + 1,
                    };
                    GoldenAction::None
                }
            }
        }
    }
}

impl super::CnCGameEngine {
    /// Freeze the world once the golden logic frame is reached. Returns true
    /// while frozen: the caller must skip every logic/client update.
    pub(super) fn golden_logic_frozen(&mut self) -> bool {
        let match_frame = self.presentation_or_boot_logic_frame();
        let state = self.current_state;
        let Some(golden) = self.golden_capture.as_mut() else {
            return false;
        };
        let logic_frame = if golden.in_shell {
            golden.shell_steps
        } else {
            match_frame
        };
        let wanted = if golden.in_shell {
            super::GameState::Menu
        } else {
            super::GameState::InGame
        };
        if state == wanted && golden.observe_logic_frame(logic_frame) {
            self.ww3d.lock().set_fixed_clock(Some(golden.frozen_clock()));
        }
        golden.logic_frozen()
    }

    /// Logic steps the golden capture still allows (`None` = unlimited).
    pub(super) fn golden_remaining_logic_steps(&self) -> Option<usize> {
        let golden = self.golden_capture.as_ref()?;
        let wanted = if golden.in_shell {
            super::GameState::Menu
        } else {
            super::GameState::InGame
        };
        if self.current_state != wanted {
            return None;
        }
        let logic_frame = if golden.in_shell {
            golden.shell_steps
        } else {
            self.presentation_or_boot_logic_frame()
        };
        golden.remaining_logic_steps(logic_frame)
    }

    /// Count one shell (Menu) logic step toward a shell-map freeze.
    pub(super) fn golden_note_shell_step(&mut self) {
        if self.current_state != super::GameState::Menu {
            return;
        }
        if let Some(golden) = self.golden_capture.as_mut() {
            golden.shell_steps = golden.shell_steps.saturating_add(1);
        }
    }

    /// Drive the capture after a successful present.
    pub(super) fn golden_after_present(&mut self) {
        let Some(golden) = self.golden_capture.as_mut() else {
            return;
        };
        match golden.after_present() {
            GoldenAction::None => {}
            GoldenAction::RequestCapture(path) => {
                log::info!("golden capture: requesting {}", path.display());
                self.ww3d.lock().make_screenshot(&path);
            }
            GoldenAction::Finish(stats) => {
                let line = self.ww3d.lock().last_frame_stats().to_string();
                if let Err(err) = std::fs::write(&stats, format!("{line}\n")) {
                    log::error!("golden capture: cannot write {}: {err}", stats.display());
                }
                log::info!("golden capture: done ({line})");
                self.request_state_change(super::GameState::Exiting);
            }
        }
    }
}

/// Engine follow-up for one golden capture step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GoldenAction {
    None,
    /// Queue a screenshot of the next present into this path.
    RequestCapture(PathBuf),
    /// Write frame counters to this path and exit.
    Finish(PathBuf),
}

fn stats_path(out: &Path) -> PathBuf {
    let mut name = out.file_stem().unwrap_or_default().to_os_string();
    name.push(".stats.txt");
    out.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture(freeze: u32, settle: u32) -> GoldenCapture {
        GoldenCapture {
            out: std::env::temp_dir().join("golden_capture_unit_never_written.png"),
            in_shell: false,
            freeze_logic_frame: freeze,
            settle_presents: settle,
            phase: Phase::Running,
            shell_steps: 0,
        }
    }

    #[test]
    fn logic_steps_are_capped_at_the_freeze_frame() {
        let mut golden = capture(90, 2);
        assert_eq!(golden.remaining_logic_steps(86), Some(4));
        assert_eq!(golden.remaining_logic_steps(90), Some(0));
        assert!(!golden.observe_logic_frame(89));
        assert!(golden.observe_logic_frame(90));
        assert!(golden.logic_frozen());
        assert_eq!(golden.remaining_logic_steps(0), Some(0));
        assert_eq!(golden.frozen_clock().total_time, LOGIC_FRAME * 90);
    }

    #[test]
    fn capture_is_requested_after_settle_presents_then_finishes() {
        let mut golden = capture(1, 2);
        assert_eq!(golden.after_present(), GoldenAction::None);
        golden.observe_logic_frame(1);
        assert_eq!(golden.after_present(), GoldenAction::None);
        assert!(matches!(golden.after_present(), GoldenAction::RequestCapture(_)));
        // The PNG never appears here; the capture gives up after 40 presents.
        let finish = (0..60)
            .map(|_| golden.after_present())
            .find(|action| matches!(action, GoldenAction::Finish(_)));
        assert!(matches!(finish, Some(GoldenAction::Finish(path))
            if path.to_string_lossy().ends_with("golden_capture_unit_never_written.stats.txt")));
    }
}
