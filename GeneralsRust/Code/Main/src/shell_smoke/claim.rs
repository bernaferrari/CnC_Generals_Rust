//! Headless host-path claim helpers. `playable_claim` stays false.

/// Never claim full retail playability from headless smoke (no W3D/window/GPU).
///
/// Honesty contract — keep `false` until ALL of:
/// - trigger ENTER iterates every `PolygonTrigger` (C++ Object.cpp:2615)
/// - weapon defaults match C++ Weapon.cpp:271-273
/// - dual-world gate does not skip production team events
/// - pathfinding can finish golden skirmish without `set_position` residual
/// - `create_render_obj` returns a real object for a stock unit
/// - map.ini is parsed
/// - `selectObject` selects
#[allow(dead_code)]
pub(super) const fn playable_claim() -> bool {
    false
}

pub(super) fn map_requirement_ok(map_resolved: bool, map_loaded: bool) -> bool {
    if map_resolved { map_loaded } else { true }
}

pub(super) fn host_path_ok(
    host_constructed: bool,
    skirmish_config_ok: bool,
    menu_config_ok: bool,
    frames_ok: bool,
    presentation_ok: bool,
    hud_selection_ok: bool,
    selection_consumers_ok: bool,
    dual_tick_presentation_ok: bool,
    screen_skirmish_ok: bool,
    control_bar_layout_ok: bool,
    map_requirement_ok: bool,
) -> bool {
    host_constructed
        && skirmish_config_ok
        && menu_config_ok
        && frames_ok
        && presentation_ok
        && hud_selection_ok
        && selection_consumers_ok
        && dual_tick_presentation_ok
        && screen_skirmish_ok
        && control_bar_layout_ok
        && map_requirement_ok
}

pub(super) fn status_from_host_path(host_path_ok: bool) -> String {
    if host_path_ok {
        "success".into()
    } else {
        "partial".into()
    }
}
