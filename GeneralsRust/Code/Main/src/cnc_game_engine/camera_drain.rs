#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

const REPLAY_FAST_FORWARD_LOGIC_STEPS: usize = 4;
#[inline]
fn replay_logic_step_count(replay_fast_forward: bool) -> usize {
    if replay_fast_forward {
        REPLAY_FAST_FORWARD_LOGIC_STEPS
    } else {
        1
    }
}

/// C++ SuperweaponInfo name drawn in the owning player's color
/// (InGameUI.cpp:245 / 3505). Presentation already stamps enemy/ally + `#RRGGBB`
/// on non-local rows; fill color from the frozen roster when missing.
fn superweapon_timer_strip_name(
    pres: &crate::presentation_frame::PresentationFrame,
    timer: &crate::presentation_frame::PresentationSuperweaponTimer,
) -> String {
    if timer.name.contains(" (Enemy")
        || timer.name.contains(" (Ally")
        || timer.name.contains(" (Neutral")
        || timer.name.contains('#')
    {
        return timer.name.clone();
    }
    let owner_id = match timer.power_key.rsplit_once('#') {
        Some((key, id)) if !key.is_empty() => id.parse::<u32>().unwrap_or(pres.local_player_id),
        _ => pres.local_player_id,
    };
    if owner_id == pres.local_player_id {
        return timer.name.clone();
    }
    let owner = pres.players.iter().find(|p| p.id == owner_id);
    let color = owner.map(|p| p.color_rgb).unwrap_or((200, 200, 200));
    let local_alliance = pres
        .players
        .iter()
        .find(|lp| lp.id == pres.local_player_id)
        .map(|lp| lp.alliance_team)
        .unwrap_or(-1);
    let rel = owner
        .map(|p| {
            if p.alliance_team >= 0 && local_alliance >= 0 && local_alliance == p.alliance_team {
                "Ally"
            } else if p.alliance_team >= 0 && local_alliance >= 0 {
                "Enemy"
            } else {
                "Neutral"
            }
        })
        .unwrap_or("Neutral");
    let (r, g, b) = color;
    format!("{} ({rel} #{r:02X}{g:02X}{b:02X})", timer.name)
}

/// C++ InGameUI.cpp:3644-3650 `time.format(L"%d:%2.2d", min, sec)` even when ready (0:00).
fn superweapon_countdown_text(remaining: f32) -> String {
    let secs = remaining.max(0.0) as u32;
    format!("{}:{:02}", secs / 60, secs % 60)
}

mod client_command_pump;
mod client_residual_ticks;
mod presentation_or_boot_residuals;
mod presentation_sync_and_broadcast;
mod render_ui_presentation;

fn snow_anim2d_client_only_dt(last: &mut Option<Instant>, now: Instant) -> f32 {
    let elapsed = last
        .map(|t| now.saturating_duration_since(t).as_secs_f32())
        .unwrap_or(game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL);
    if elapsed + f32::EPSILON < game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL {
        return 0.0;
    }
    *last = Some(now);
    game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL
}

#[cfg(any(test, feature = "internal"))]
mod replay_fast_forward_probe;

#[cfg(all(feature = "internal", feature = "game_client", not(target_arch = "wasm32")))]
mod native_rank_science_probe;

#[cfg(all(feature = "internal", feature = "game_client", not(target_arch = "wasm32")))]
pub(super) fn run_native_rank_science_probe() -> anyhow::Result<()> {
    native_rank_science_probe::run()
}

#[cfg(test)]
mod superweapon_countdown_tests {
    use super::superweapon_countdown_text;

    #[test]
    fn ready_and_recharging_use_mm_ss_not_ready_word() {
        assert_eq!(superweapon_countdown_text(0.0), "0:00");
        assert_eq!(superweapon_countdown_text(-1.0), "0:00");
        assert_eq!(superweapon_countdown_text(90.0), "1:30");
        assert_eq!(superweapon_countdown_text(5.9), "0:05");
        assert_ne!(superweapon_countdown_text(0.0), "READY");
    }
}

#[cfg(feature = "internal")]
pub(super) fn run_replay_fast_forward_engine_probe() -> anyhow::Result<()> {
    replay_fast_forward_probe::run_replay_fast_forward_engine_probe()
}

#[cfg(all(feature = "internal", not(target_arch = "wasm32")))]
#[path = "snow_anim2d_owner_probe.rs"]
mod snow_anim2d_owner_probe;

#[cfg(all(feature = "internal", not(target_arch = "wasm32")))]
pub(super) fn run_snow_anim2d_owner_probe() -> anyhow::Result<()> {
    snow_anim2d_owner_probe::run()
}

#[cfg(all(test, feature = "game_client"))]
#[path = "receiving_draw_module_name_tests.rs"]
mod receiving_draw_module_name_tests;
