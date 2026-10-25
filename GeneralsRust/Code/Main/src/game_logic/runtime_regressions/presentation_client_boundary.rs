//! Executable command and lifecycle regressions.

fn presentation_client_boundary() -> bool {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, GameLogic, GameMode, LONE_EAGLE_MAP_WAVE169,
        resolve_retail_map_path,
    };
    use crate::presentation_frame::PresentationFrame;

    // Live: empty host ObjectId store before map.
    let mut logic = GameLogic::new();
    if !logic.host_objects().registered_is_empty() {
        return false;
    }

    // Prefer Lone Eagle; fall back to Defcon6; soft-ok if neither (CI without MapsZH).
    let map_name = if resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169).is_some() {
        LONE_EAGLE_MAP_WAVE169
    } else if resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169).is_some() {
        DEFAULT_SKIRMISH_MAP_WAVE169
    } else {
        return true;
    };

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
    if logic.host_objects().registered_is_empty() {
        return false;
    }

    logic.update();
    let pres = PresentationFrame::build_from_logic(&mut logic, 0);
    // Presentation boundary: non-empty seed from host ObjectId store (no live GameLogic in execute).
    if pres.objects.is_empty() {
        return false;
    }

    true
}

#[test]
fn presentation_client_boundary_runs() {
    assert!(presentation_client_boundary());
}
