//! Wave 348 residual peels: ScriptEngine dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), named-cache/
//! team helpers use host adapters without dual-world factory walks. Sequential
//! scripts still walk the list and clean up missing actors as C++ requires.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 347 ActionManager dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/scripting/engine/mod.rs` and its reachable fragments.
//! - `GameLogic/src/scripting/engine/attack_priorities.rs` ungated table Xfer.
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - `xfer` intentionally ungated so save/load validation stays intact

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// ScriptEngine dual-world empty-gate residual method names.
pub const LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE348: &[&str] = &[
    "dual_world_registry_unavailable",
    "create_named_cache",
    "evaluate_and_progress_all_sequential_scripts",
    "team_ai_status",
    "transfer_object_name",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE348: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SCRIPT_ENGINE_EMPTY_GATES",
    "LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE348: &[&str] = &[
    "click_live_script_engine_dual_world_empty_gate_ok_prepare",
    "click_live_script_engine_dual_world_empty_gate_ok_live",
    "click_live_script_engine_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_script_engine_dual_world_empty_gate_method_names_residual_wave348() -> bool {
    LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE348.len() == 6
        && residual_name_index(
            LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE348,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE348,
            "transfer_object_name",
        ) == Some(4)
        && residual_name_index(
            LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE348,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_script_engine_dual_world_empty_gate_nav_commands_residual_wave348() -> bool {
    LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE348.len() == 4
        && residual_name_index(
            LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE348,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE348,
            "LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SCRIPT_ENGINE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE348.len() == 3
}

/// Wave 348 composite residual honesty pack.
pub fn honesty_live_script_engine_dual_world_empty_gate_residual_pack_wave348() -> bool {
    honesty_live_script_engine_dual_world_empty_gate_method_names_residual_wave348()
        && honesty_live_script_engine_dual_world_empty_gate_nav_commands_residual_wave348()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(name) {
        let i = search_from + rel;
        let Some(b) = src[i..].find('{') else {
            search_from = i + name.len();
            continue;
        };
        let brace = i + b;
        let mut depth = 0usize;
        for (off, ch) in src[brace..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let body = &src[i..brace + off + 1];
                        return Some(body);
                    }
                }
                _ => {}
            }
        }
        search_from = i + name.len();
    }
    None
}

/// Source residual: ScriptEngine empty dual-world short-circuits.
pub fn honesty_script_engine_dual_world_empty_gate_source() -> bool {
    script_engine_source_contract(gamelogic::scripting::engine::SCRIPT_ENGINE_SRC)
}

fn script_engine_source_contract(g: &str) -> bool {
    if !(g.contains("Wave 348")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    // Inspect the real Engine and extracted table, independent of other Xfer impls.
    let Some((_, engine)) = g.split_once("impl XferSnapshot for ScriptEngine {") else {
        return false;
    };
    let Some(engine_xfer) = fn_body(engine, "fn xfer(") else {
        return false;
    };
    let Some((_, table)) = g.split_once("impl AttackPriorityTable {") else {
        return false;
    };
    let Some(table_xfer) = fn_body(table, "fn xfer(") else {
        return false;
    };
    if engine_xfer.contains("dual_world_registry_unavailable")
        || table_xfer.contains("dual_world_registry_unavailable")
        || !engine_xfer.contains("inner.attack_priorities.xfer(xfer)?")
        || !table_xfer.contains("self.rows[i].xfer(xfer)?")
    {
        return false;
    }
    let Some(cache) = fn_body(g, "fn create_named_cache(") else {
        return false;
    };
    let Some(eval) = fn_body(g, "fn evaluate_and_progress_all_sequential_scripts(") else {
        return false;
    };
    let Some(team) = fn_body(g, "fn team_ai_status(") else {
        return false;
    };
    let Some(xfer_name) = fn_body(g, "fn transfer_object_name(") else {
        return false;
    };
    // C++ ScriptEngine.cpp:7860 walks the list even without Objects. Preserve
    // missing-actor cleanup and the host-team adapter rather than skipping it.
    let Some(loop_start) = eval.find("while i < self.sequential_script_count() {") else {
        return false;
    };
    helper_ok
        && cache.contains("dual_world_registry_unavailable()")
        && cache.contains("return;")
        && !eval[..loop_start].contains("dual_world_registry_unavailable()")
        && eval.contains("cleanup_sequential_script_by_token")
        && team.contains("dual_world_registry_unavailable()")
        && team.contains("return crate::scripting::host_team_sequential_status(&name)")
        && team.contains("return (false, true)")
        && xfer_name.contains("dual_world_registry_unavailable()")
        && xfer_name.contains("return Ok(())")
        && g.contains("fn set_objects_should_receive_difficulty_bonus")
        && g.contains("fn add_object_to_cache")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_script_engine_dual_world_empty_gate_honesty() -> bool {
    honesty_live_script_engine_dual_world_empty_gate_residual_pack_wave348()
        && honesty_script_engine_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_script_engine_dual_world_empty_gate_method_names_residual_wave348());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_script_engine_dual_world_empty_gate_nav_commands_residual_wave348());
    }

    #[test]
    fn wave348_composite_pack() {
        assert!(honesty_live_script_engine_dual_world_empty_gate_residual_pack_wave348());
    }

    #[test]
    fn script_engine_dual_world_empty_gate_sources() {
        assert!(honesty_script_engine_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_script_engine_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_script_engine_dual_world_empty_gate_honesty(),
            "script engine dual-world empty gate residual must latch"
        );
    }

    #[test]
    fn production_sequential_walk_satisfies_cpp_contract() {
        assert!(script_engine_source_contract(
            gamelogic::scripting::engine::SCRIPT_ENGINE_SRC
        ));
    }

    #[test]
    fn source_contract_rejects_sequential_empty_registry_skip() {
        let source = gamelogic::scripting::engine::SCRIPT_ENGINE_SRC;
        let marker = "while i < self.sequential_script_count() {";
        assert_eq!(source.matches(marker).count(), 1);
        let skipped = source.replacen(marker,
            "if dual_world_registry_unavailable() { return Ok(()); }\n        while i < self.sequential_script_count() {", 1);
        assert!(!script_engine_source_contract(&skipped));
    }

    #[test]
    fn source_extraction_finds_ungated_engine_xfer() {
        let source = gamelogic::scripting::engine::SCRIPT_ENGINE_SRC;
        let (_, engine) = source
            .split_once("impl XferSnapshot for ScriptEngine {")
            .unwrap();
        let body = fn_body(engine, "fn xfer(")
            .expect("actual engine xfer must be inspected even without a registry gate");
        assert!(!body.contains("dual_world_registry_unavailable"));
    }

    #[test]
    fn source_extraction_finds_ungated_priority_table_xfer() {
        let source = gamelogic::scripting::engine::SCRIPT_ENGINE_SRC;
        let (_, table) = source.split_once("impl AttackPriorityTable {").unwrap();
        let body = fn_body(table, "fn xfer(")
            .expect("actual table xfer must be inspected even without a registry gate");
        assert!(body.contains("self.rows[i].xfer(xfer)?"));
        assert!(!body.contains("dual_world_registry_unavailable"));
    }

    #[test]
    fn source_contract_rejects_gated_engine_xfer() {
        let source = gamelogic::scripting::engine::SCRIPT_ENGINE_SRC;
        let (prefix, engine) = source
            .split_once("impl XferSnapshot for ScriptEngine {")
            .unwrap();
        let marker = "let inner = self.inner.get_mut();";
        let engine = engine.replacen(marker, "if dual_world_registry_unavailable() { return Ok(()); }\n        let inner = self.inner.get_mut();", 1);
        let gated = format!("{prefix}impl XferSnapshot for ScriptEngine {{{engine}");
        assert!(!script_engine_source_contract(&gated));
    }

    #[test]
    fn source_contract_rejects_gated_priority_table_xfer() {
        let source = gamelogic::scripting::engine::SCRIPT_ENGINE_SRC;
        let marker = "let mut attack_priority_size: u16 =";
        assert_eq!(source.matches(marker).count(), 1);
        let gated = source.replacen(marker,
            "if dual_world_registry_unavailable() { return Ok(()); }\n        let mut attack_priority_size: u16 =", 1);
        assert!(!script_engine_source_contract(&gated));
    }
}
