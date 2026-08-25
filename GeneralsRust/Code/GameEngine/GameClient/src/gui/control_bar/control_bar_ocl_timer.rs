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

/// C++ ControlBarOCLTimer.cpp:71-104 sell / rally / hide choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OclTimerButtonKind {
    Sell,
    RallyPoint,
    Hidden,
}

/// C++ `populateOCLTimer`: !TECH_BUILDING → Sell; TECH+AUTO_RALLYPOINT → Rally; else hide.
pub fn ocl_timer_button_kind(
    is_tech_building: bool,
    is_auto_rallypoint: bool,
) -> OclTimerButtonKind {
    if !is_tech_building {
        OclTimerButtonKind::Sell
    } else if is_auto_rallypoint {
        OclTimerButtonKind::RallyPoint
    } else {
        OclTimerButtonKind::Hidden
    }
}

pub fn ocl_timer_kind_for_object(obj_id: u32) -> OclTimerButtonKind {
    if let Some(obj_arc) = gamelogic::object::registry::OBJECT_REGISTRY.get_object(obj_id) {
        if let Ok(obj) = obj_arc.read() {
            return ocl_timer_button_kind(
                obj.is_kind_of(gamelogic::common::types::KindOf::TechBuilding),
                obj.is_kind_of(gamelogic::common::types::KindOf::AutoRallypoint),
            );
        }
    }
    if let Some(entry) = crate::presentation_translator_residual::translator_catalog_entry(obj_id) {
        let is_tech = crate::presentation_translator_residual::translator_entry_has_kind(
            &entry,
            "TECH_BUILDING",
        );
        let is_rally = crate::presentation_translator_residual::translator_entry_has_kind(
            &entry,
            "AUTO_RALLYPOINT",
        );
        return ocl_timer_button_kind(is_tech, is_rally);
    }
    // Fail-closed: unknown creator is not assumed Sell.
    OclTimerButtonKind::Hidden
}

fn ocl_timer_command_button(kind: OclTimerButtonKind) -> Option<super::CommandButton> {
    match kind {
        OclTimerButtonKind::Sell => Some(super::CommandButton {
            command_name: "Command_Sell".to_string(),
            command_type: gamelogic::commands::command::CommandType::Sell,
            ..Default::default()
        }),
        OclTimerButtonKind::RallyPoint => Some(super::CommandButton {
            command_name: "Command_SetRallyPoint".to_string(),
            command_type: gamelogic::commands::command::CommandType::SetRallyPoint,
            ..Default::default()
        }),
        OclTimerButtonKind::Hidden => None,
    }
}

/// Populate OCL timer command buttons into the control bar context.
///
/// C++ ControlBarOCLTimer.cpp:55 `populateOCLTimer`: adds a sell button
/// (`Command_Sell`) for non-tech buildings, a rally-point button
/// (`Command_SetRallyPoint`) for tech buildings with `AUTO_RALLYPOINT`,
/// or hides the button. The timer display is updated via `update_context_ocl_timer`.
pub fn populate_ocl_timer_commands(
    context: &mut ControlBarContext,
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(&obj_id) = context.selected_objects.first() else {
        return Ok(());
    };

    context.available_commands.retain(|command| {
        command.command_name != "Command_Sell" && command.command_name != "Command_SetRallyPoint"
    });

    if let Some(button) = ocl_timer_command_button(ocl_timer_kind_for_object(obj_id)) {
        context.available_commands.push(button);
    }

    Ok(())
}

/// C++ ControlBarOCLTimer.cpp:23-49 updateOCLTimerTextDisplay + reveal CP_OCL_TIMER.
pub fn apply_ocl_timer_windows(text: &str, progress_percent: f32, button_kind: OclTimerButtonKind) {
    crate::gui::with_window_manager(|wm| {
        if let Some(win) = wm.find_window_by_name("ControlBar.wnd:OCLTimerWindow") {
            let _ = win.borrow_mut().hide(false);
        }
        if let Some(win) = wm.find_window_by_name("ControlBar.wnd:OCLTimerStaticText") {
            let _ = win.borrow_mut().set_text(text);
        }
        if let Some(win) = wm.find_window_by_name("ControlBar.wnd:OCLTimerProgressBar") {
            if let Some(bar) = win.borrow_mut().progress_bar_mut() {
                bar.set_progress(progress_percent.clamp(0.0, 100.0));
            }
        }
        if let Some(win) = wm.find_window_by_name("ControlBar.wnd:OCLTimerSellButton") {
            match button_kind {
                OclTimerButtonKind::Hidden => {
                    let _ = win.borrow_mut().hide(true);
                }
                OclTimerButtonKind::Sell => {
                    win.borrow_mut().set_user_data("Command_Sell".to_string());
                    let _ = win.borrow_mut().hide(false);
                    let _ = win.borrow_mut().enable(true);
                }
                OclTimerButtonKind::RallyPoint => {
                    win.borrow_mut()
                        .set_user_data("Command_SetRallyPoint".to_string());
                    let _ = win.borrow_mut().hide(false);
                    let _ = win.borrow_mut().enable(true);
                }
            }
        }
    });
}

/// Residual: last OCL timer action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualOclTimerAction {
    None = 0,
    Format = 1,
    FramesToDisplay = 2,
    ShouldUpdate = 3,
    Prepare = 4,
}

static RESIDUAL_OCL_TIMER_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_OCL_TIMER_SECONDS: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);
static RESIDUAL_OCL_TIMER_PROGRESS_MILLI: std::sync::atomic::AtomicU32 =
    std::sync::atomic::AtomicU32::new(0);

fn residual_ocl_timer_action_store(action: ResidualOclTimerAction) {
    RESIDUAL_OCL_TIMER_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last OCL timer residual action.
pub fn residual_ocl_timer_last_action() -> ResidualOclTimerAction {
    match RESIDUAL_OCL_TIMER_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualOclTimerAction::Format,
        2 => ResidualOclTimerAction::FramesToDisplay,
        3 => ResidualOclTimerAction::ShouldUpdate,
        4 => ResidualOclTimerAction::Prepare,
        _ => ResidualOclTimerAction::None,
    }
}

/// Residual: last displayed seconds latch.
pub fn residual_ocl_timer_seconds() -> u32 {
    RESIDUAL_OCL_TIMER_SECONDS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: last progress percent * 1000 latch.
pub fn residual_ocl_timer_progress_milli() -> u32 {
    RESIDUAL_OCL_TIMER_PROGRESS_MILLI.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: format OCL timer display without control-bar context.
pub fn simulate_ocl_timer_format(total_seconds: u32, percent: f32) -> (String, f32) {
    let (text, progress) = format_ocl_timer_display(total_seconds, percent);
    RESIDUAL_OCL_TIMER_SECONDS.store(total_seconds, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_OCL_TIMER_PROGRESS_MILLI.store(
        (progress * 10.0) as u32,
        std::sync::atomic::Ordering::Relaxed,
    );
    residual_ocl_timer_action_store(ResidualOclTimerAction::Format);
    (text, progress)
}

/// Residual: convert frames to display seconds/percent without selection.
pub fn simulate_ocl_timer_frames_to_display(
    remaining_frames: u32,
    total_frames: u32,
) -> (u32, f32) {
    let (seconds, percent) = ocl_frames_to_display(remaining_frames, total_frames);
    RESIDUAL_OCL_TIMER_SECONDS.store(seconds, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_OCL_TIMER_PROGRESS_MILLI.store(
        (percent * 1000.0) as u32,
        std::sync::atomic::Ordering::Relaxed,
    );
    residual_ocl_timer_action_store(ResidualOclTimerAction::FramesToDisplay);
    (seconds, percent)
}

/// Residual: should-update check residual.
pub fn simulate_ocl_timer_should_update(displayed_seconds: u32, current_seconds: u32) -> bool {
    let state = OCLTimerDisplayState { displayed_seconds };
    let should = should_update_timer_text(&state, current_seconds);
    residual_ocl_timer_action_store(ResidualOclTimerAction::ShouldUpdate);
    should
}

/// Residual: frames -> format composite (common control-bar path).
pub fn simulate_ocl_timer_prepare_display(
    remaining_frames: u32,
    total_frames: u32,
) -> Option<(String, f32, u32)> {
    let (seconds, percent) = simulate_ocl_timer_frames_to_display(remaining_frames, total_frames);
    let (text, progress) = simulate_ocl_timer_format(seconds, percent);
    residual_ocl_timer_action_store(ResidualOclTimerAction::Prepare);
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

    #[test]
    fn test_ocl_timer_button_kind_matches_cpp() {
        assert_eq!(
            ocl_timer_button_kind(false, false),
            OclTimerButtonKind::Sell
        );
        assert_eq!(ocl_timer_button_kind(false, true), OclTimerButtonKind::Sell);
        assert_eq!(
            ocl_timer_button_kind(true, true),
            OclTimerButtonKind::RallyPoint
        );
        assert_eq!(
            ocl_timer_button_kind(true, false),
            OclTimerButtonKind::Hidden
        );
    }
}
