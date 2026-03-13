//! Control-bar OCL timer helpers.
//!
//! Ported from `ControlBarOCLTimer.cpp` (Author: Colin Day, March 2002).
//!
//! Provides UI display and update logic for the OCL (Object Creation List) timer context.
//! When a selected object has an active OCL countdown, the control bar shows remaining time
//! and a progress bar.

use super::ControlBarContext;
use game_engine::common::game_common::LOGICFRAMES_PER_SECOND;

/// State tracked between frames to avoid redundant UI redraws.
#[derive(Debug, Clone, Default)]
pub struct OCLTimerDisplayState {
    /// The last number of seconds shown to the user.
    pub displayed_seconds: u32,
}

/// Format the OCL timer text and progress for display.
///
/// Returns `(formatted_text, progress_percent)` where:
/// - `formatted_text` is a "M:SS" string suitable for UI display.
/// - `progress_percent` is 0.0–100.0.
pub fn format_ocl_timer_display(total_seconds: u32, percent: f32) -> (String, f32) {
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    let text = if seconds < 10 {
        format!("{}:0{}", minutes, seconds)
    } else {
        format!("{}:{}", minutes, seconds)
    };
    (text, (percent * 100.0).clamp(0.0, 100.0))
}

/// Compute remaining seconds and countdown percentage from raw frame counts.
///
/// `remaining_frames` comes from `OCLUpdate::get_remaining_frames()`.
/// `total_frames` is `next_creation_frame - timer_started_frame`.
pub fn ocl_frames_to_display(remaining_frames: u32, total_frames: u32) -> (u32, f32) {
    let seconds = remaining_frames / LOGICFRAMES_PER_SECOND;
    let percent = if total_frames == 0 {
        0.0
    } else {
        1.0 - (remaining_frames as f32 / total_frames as f32)
    };
    (seconds, percent.clamp(0.0, 1.0))
}

/// Returns `true` when the OCL timer text needs to be refreshed.
///
/// Mirrors the C++ guard `m_displayedOCLTimerSeconds != seconds`.
pub fn should_update_timer_text(state: &OCLTimerDisplayState, current_seconds: u32) -> bool {
    state.displayed_seconds != current_seconds
}

/// Populate OCL-timer command availability into the context.
///
/// The original C++ `populateOCLTimer` set up a sell or rally-point button depending on the
/// creator object's kind-of flags, updated the timer display, and set the portrait.
/// The Rust control bar handles command population generically; this function provides the
/// timer-specific bookkeeping so the main `ControlBar` can delegate here.
///
/// Returns the timer display tuple `(text, progress_percent)` if the timer is active.
pub fn populate_ocl_timer(
    context: &mut ControlBarContext,
    remaining_frames: u32,
    total_frames: u32,
) -> Option<(String, f32)> {
    if context.selected_objects.is_empty() {
        return None;
    }

    let (seconds, percent) = ocl_frames_to_display(remaining_frames, total_frames);
    let (text, progress) = format_ocl_timer_display(seconds, percent);

    context.construction_queue.clear();
    context.construction_queue.push(super::ProductionItem {
        template_name: "OCLTimer".to_string(),
        production_type: super::ProductionType::SpecialPower,
        progress: percent,
        cost: Default::default(),
        build_time: total_frames as f32 / LOGICFRAMES_PER_SECOND as f32,
    });

    Some((text, progress))
}

/// Per-frame update for the OCL timer context.
///
/// Returns updated `(text, progress_percent, current_seconds)` when the display should change,
/// or `None` when no refresh is needed.
pub fn update_context_ocl_timer(
    state: &mut OCLTimerDisplayState,
    remaining_frames: u32,
    total_frames: u32,
) -> Option<(String, f32, u32)> {
    let (seconds, percent) = ocl_frames_to_display(remaining_frames, total_frames);

    if !should_update_timer_text(state, seconds) {
        return None;
    }

    state.displayed_seconds = seconds;
    let (text, progress) = format_ocl_timer_display(seconds, percent);
    Some((text, progress, seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ocl_timer_display() {
        let (text, progress) = format_ocl_timer_display(65, 0.5);
        assert_eq!(text, "1:05");
        assert!((progress - 50.0).abs() < 0.01);

        let (text, progress) = format_ocl_timer_display(120, 1.0);
        assert_eq!(text, "2:00");
        assert!((progress - 100.0).abs() < 0.01);

        let (text, progress) = format_ocl_timer_display(5, 0.0);
        assert_eq!(text, "0:05");
        assert!((progress - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_ocl_frames_to_display() {
        // 900 frames = 30 seconds at 30 fps
        let (secs, pct) = ocl_frames_to_display(900, 900);
        assert_eq!(secs, 30);
        assert!((pct - 0.0).abs() < 0.01);

        // Halfway through
        let (secs, pct) = ocl_frames_to_display(450, 900);
        assert_eq!(secs, 15);
        assert!((pct - 0.5).abs() < 0.01);

        // Done
        let (secs, pct) = ocl_frames_to_display(0, 900);
        assert_eq!(secs, 0);
        assert!((pct - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_should_update_timer_text() {
        let state = OCLTimerDisplayState {
            displayed_seconds: 10,
        };
        assert!(should_update_timer_text(&state, 9));
        assert!(!should_update_timer_text(&state, 10));
        assert!(should_update_timer_text(&state, 11));
    }

    #[test]
    fn test_update_context_ocl_timer() {
        let mut state = OCLTimerDisplayState::default();

        // First call should always return
        let result = update_context_ocl_timer(&mut state, 900, 900);
        assert!(result.is_some());
        let (text, progress, secs) = result.unwrap();
        assert_eq!(text, "0:30");
        assert_eq!(secs, 30);

        // Same seconds should return None
        let result = update_context_ocl_timer(&mut state, 900, 900);
        assert!(result.is_none());

        // Different seconds should return Some
        let result = update_context_ocl_timer(&mut state, 870, 900);
        assert!(result.is_some());
        let (text, _progress, secs) = result.unwrap();
        assert_eq!(text, "0:29");
        assert_eq!(secs, 29);
    }
}
