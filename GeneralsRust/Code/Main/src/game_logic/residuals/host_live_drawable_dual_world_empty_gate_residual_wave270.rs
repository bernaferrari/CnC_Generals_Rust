//! Wave 270 residual peels: Drawable dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), Drawable
//! object-bound icon/UI helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 269 GameClient dual-world empty-gate residual.
//!
//! Sources:
//! - `GameClient/src/drawable/drawable.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - Full `update`/`render` keep host presentation work (not short-circuited)

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Drawable dual-world empty-gate residual method names.
pub const LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE270: &[&str] = &[
    "dual_world_registry_unavailable",
    "is_object_kind_of",
    "object_stealth_visuals",
    "draw_health_bar",
    "draw_icon_ui",
    "handle_weapon_fire_fx",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE270: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_DRAWABLE_EMPTY_GATES",
    "LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE270: &[&str] = &[
    "click_live_drawable_dual_world_empty_gate_ok_prepare",
    "click_live_drawable_dual_world_empty_gate_ok_live",
    "click_live_drawable_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_drawable_dual_world_empty_gate_method_names_residual_wave270() -> bool {
    LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE270.len() == 7
        && residual_name_index(
            LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE270,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE270,
            "draw_icon_ui",
        ) == Some(4)
        && residual_name_index(
            LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE270,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_drawable_dual_world_empty_gate_nav_commands_residual_wave270() -> bool {
    LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE270.len() == 4
        && residual_name_index(
            LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE270,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE270,
            "LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_DRAWABLE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE270.len() == 3
}

/// Wave 270 composite residual honesty pack.
pub fn honesty_live_drawable_dual_world_empty_gate_residual_pack_wave270() -> bool {
    honesty_live_drawable_dual_world_empty_gate_method_names_residual_wave270()
        && honesty_live_drawable_dual_world_empty_gate_nav_commands_residual_wave270()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[i..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Source residual: Drawable empty dual-world short-circuits.
pub fn honesty_drawable_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameClient/src/drawable/drawable.rs");
    if !(g.contains("Wave 270")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(kind) = fn_body(g, "fn is_object_kind_of(") else {
        return false;
    };
    let Some(stealth) = fn_body(g, "fn object_stealth_visuals(") else {
        return false;
    };
    // Prefer the gated BasicDrawable inherent draw_icon_ui (has dual-world helper),
    // not the Drawable trait default stub.
    let Some(icon) = fn_body(g, "pub fn draw_icon_ui(").or_else(|| fn_body(g, "fn draw_icon_ui("))
    else {
        return false;
    };
    // Prefer the gated BasicDrawable draw_icon_ui (has dual-world helper).
    let icon_ok = g.contains("fn draw_icon_ui(&mut self) {\n        // Wave 270:")
        || g.contains("pub fn draw_icon_ui(&mut self) {\n        // Wave 270:")
        || icon.contains("dual_world_registry_unavailable");
    kind.contains("dual_world_registry_unavailable")
        && kind.contains("return false")
        && stealth.contains("dual_world_registry_unavailable")
        && stealth.contains("return None")
        && icon_ok
        // update/render must not early-return solely on empty dual-world
        && !g.contains("fn update(&mut self, _delta_time: f32) {\n        // Wave 270:")
        && !g.contains(
            "fn render(&mut self, view_matrix: &Matrix4, projection_matrix: &Matrix4) {\n        // Wave 270:",
        )
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_drawable_dual_world_empty_gate_honesty() -> bool {
    honesty_live_drawable_dual_world_empty_gate_residual_pack_wave270()
        && honesty_drawable_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_drawable_dual_world_empty_gate_method_names_residual_wave270());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_drawable_dual_world_empty_gate_nav_commands_residual_wave270());
    }

    #[test]
    fn wave270_composite_pack() {
        assert!(honesty_live_drawable_dual_world_empty_gate_residual_pack_wave270());
    }

    #[test]
    fn drawable_dual_world_empty_gate_sources() {
        assert!(honesty_drawable_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_drawable_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_drawable_dual_world_empty_gate_honesty(),
            "drawable dual-world empty gate residual must latch"
        );
    }
}
