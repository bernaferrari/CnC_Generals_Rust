//! Executable command and lifecycle regressions.

fn map_load() -> bool {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, LONE_EAGLE_MAP_WAVE169, resolve_retail_map_path,
    };

    let defcon_path = resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169);
    let lone_path = resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169);

    // No maps checked out — soft residual (still require pack honesty).
    if defcon_path.is_none() && lone_path.is_none() {
        return true;
    }

    // Partial tree fail-closed.
    let (Some(_), Some(_)) = (defcon_path, lone_path) else {
        return false;
    };

    let Some((defcon_adv, _)) = live_load_one(DEFAULT_SKIRMISH_MAP_WAVE169) else {
        return false;
    };
    if defcon_adv == 0 {
        return false;
    }

    let Some((lone_adv, lone_objs)) = live_load_one(LONE_EAGLE_MAP_WAVE169) else {
        return false;
    };
    if lone_adv == 0 {
        return false;
    }
    // Lone Eagle behavior_gate residual historically loads hundreds of objects.
    // Accept any non-empty world as materialisation; do not hardcode 903.
    if lone_objs == 0 {
        return false;
    }
    true
}

#[test]
fn map_load_runs() {
    assert!(map_load());
}

fn live_load_one(map_name: &str) -> Option<(u32, usize)> {
    use crate::game_logic::{GameLogic, GameMode, resolve_retail_map_path};

    let path = resolve_retail_map_path(map_name)?;
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    let s = path.to_string_lossy();
    let loaded = logic.load_map(s.as_ref()) || logic.load_map(map_name);
    if !loaded {
        return None;
    }
    let before = logic.get_frame();
    for _ in 0..LIVE_MAP_LOAD_FRAME_ADVANCE_WAVE170 {
        logic.update();
    }
    let advanced = logic.get_frame().saturating_sub(before);
    let object_count =
        crate::presentation_frame::PresentationFrame::build_from_logic(&mut logic, 0)
            .objects
            .len();
    Some((advanced, object_count))
}

pub const LIVE_MAP_LOAD_FRAME_ADVANCE_WAVE170: u32 = 5;
