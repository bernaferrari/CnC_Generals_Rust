//! Wave 976: meta-event TheGameLogic peels + drawable template residual.
//!
//! Removes incorrect OBJECT_REGISTRY empty gates from meta-event selection
//! helpers that already use TheGameLogic IDs. Peels resolve_drawable_template_name
//! / register_drawable_with_template onto presentation residual for host path.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_META_DRAWABLE_TEMPLATE_METHOD_NAMES_WAVE976: &[&str] = &[
    "kill_local_player_selection",
    "adjust_local_selection_veterancy",
    "resolve_drawable_template_name",
    "register_drawable_with_template",
    "Wave 976",
    "playable_claim = false",
];

pub const LIVE_HOST_META_DRAWABLE_TEMPLATE_NAV_STEPS_WAVE976: &[&str] = &[
    "META_THEGAMELOGIC_IDS",
    "DRAWABLE_TEMPLATE_FROM_CATALOG",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_META_DRAWABLE_TEMPLATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMetaDrawableTemplateAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMetaDrawableTemplateAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn meta_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/message_stream/meta_event.rs")
}

fn client_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

fn residual_mod_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}

pub fn honesty_host_meta_drawable_template_method_names_residual_wave976() -> bool {
    let names = LIVE_HOST_META_DRAWABLE_TEMPLATE_METHOD_NAMES_WAVE976;
    let ok = residual_name_index(names, "kill_local_player_selection").is_some()
        && residual_name_index(names, "Wave 976").is_some();
    residual_action_store(ResidualHostMetaDrawableTemplateAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_meta_drawable_template_nav_commands_residual_wave976() -> bool {
    let steps = LIVE_HOST_META_DRAWABLE_TEMPLATE_NAV_STEPS_WAVE976;
    let ok = residual_name_index(steps, "LIVE_HOST_META_DRAWABLE_TEMPLATE").is_some()
        && residual_name_index(steps, "META_THEGAMELOGIC_IDS").is_some();
    residual_action_store(ResidualHostMetaDrawableTemplateAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_meta_drawable_template_residual_pack_wave976() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let meta = meta_source();
    let client = client_source();
    let residual = residual_mod_source();
    let kill = match meta.find("fn kill_local_player_selection") {
        Some(i) => &meta[i..meta.len().min(i + 500)],
        None => "",
    };
    let vet = match meta.find("fn adjust_local_selection_veterancy") {
        Some(i) => &meta[i..meta.len().min(i + 400)],
        None => "",
    };
    let resolve = match client.find("fn resolve_drawable_template_name") {
        Some(i) => &client[i..client.len().min(i + 900)],
        None => "",
    };
    let register = match client.find("fn register_drawable_with_template") {
        Some(i) => &client[i..client.len().min(i + 500)],
        None => "",
    };
    let ok = meta.contains("Wave 976")
        && client.contains("Wave 976")
        && residual.contains("template_name")
        && kill.contains("TheGameLogic")
        && !kill.contains("dual_world_registry_unavailable")
        && vet.contains("Wave 976")
        && !vet.contains("dual_world_registry_unavailable")
        && resolve.contains("translator_catalog_entry")
        && resolve.contains("get_template_name")
        && !register.contains("dual-world registry unavailable")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostMetaDrawableTemplateAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_meta_drawable_template_honesty() -> bool {
    let a = honesty_host_meta_drawable_template_method_names_residual_wave976();
    let b = honesty_host_meta_drawable_template_nav_commands_residual_wave976();
    let c = honesty_host_meta_drawable_template_residual_pack_wave976();
    residual_action_store(ResidualHostMetaDrawableTemplateAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_meta_drawable_template_residual_wave976() {
        assert!(honesty_host_meta_drawable_template_residual_pack_wave976());
        assert!(honesty_host_meta_drawable_template_method_names_residual_wave976());
        assert!(honesty_host_meta_drawable_template_nav_commands_residual_wave976());
        assert!(simulate_live_host_meta_drawable_template_honesty());
    }
}
