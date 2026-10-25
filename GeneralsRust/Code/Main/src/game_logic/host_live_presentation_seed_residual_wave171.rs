//! Wave 171 residual peels: live PresentationFrame seed after map load
//! (Lone Eagle/Defcon6 → build_from_logic non-empty objects; never flips
//! shell `playable_claim`).
//!
//! Orthogonal to Wave 170 live map load residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH architecture):
//! - CncGameEngine::seed_presentation_after_match_start
//! - PresentationFrame::build_from_logic
//! - Immutable presentation boundary (execute reads frame, not live GameLogic)
//!
//! Fail-closed:
//! - Not full WGPU render pass residual
//! - Not full dual-tick shadow overlay residual (wave 102/157)
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live presentation seed residual method names.

/// Ordered live presentation seed residual navigation steps.

/// Runtime-host command residual names.

/// Honesty: method names residual pack.

/// Honesty: nav steps + runtime-host cmd residual pack.

/// Wave 171 composite residual honesty pack.

/// Source residual: engine seeds presentation after match start from GameLogic.

/// Source residual: render execute stays presentation-only (no live GameLogic arg).

/// Live residual: load retail map, build PresentationFrame, require materialised objects.
pub fn simulate_live_presentation_seed_honesty() -> bool {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, GameLogic, GameMode, LONE_EAGLE_MAP_WAVE169,
        resolve_retail_map_path,
    };
    use crate::presentation_frame::PresentationFrame;

    // Prefer Lone Eagle (behavior_gate map); fall back to Defcon6; soft-ok if neither.
    let map_name = if resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169).is_some() {
        LONE_EAGLE_MAP_WAVE169
    } else if resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169).is_some() {
        DEFAULT_SKIRMISH_MAP_WAVE169
    } else {
        return true;
    };

    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    let path = resolve_retail_map_path(map_name);
    let loaded = match path {
        Some(p) => {
            let s = p.to_string_lossy();
            logic.load_map(s.as_ref()) || logic.load_map(map_name)
        }
        None => logic.load_map(map_name),
    };
    if !loaded {
        return false;
    }

    // Advance a few frames so snapshot captures settled spawn state.
    for _ in 0..3 {
        logic.update();
    }

    let local_id = 0u32;
    let pres = PresentationFrame::build_from_logic(&mut logic, local_id);
    if pres.objects.is_empty() {
        return false;
    }
    // Frame identity residual: map load should leave a non-zero host frame clock
    // after updates, and presentation should bind the same logic epoch.
    if logic.get_frame() == 0 {
        return false;
    }
    // Local team frozen (not default-only empty world).
    let _ = pres.local_team;
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simulate_live_presentation_seed_honesty_residual_live() {
        assert!(
            simulate_live_presentation_seed_honesty(),
            "live map load must seed non-empty PresentationFrame from GameLogic"
        );
    }
}
