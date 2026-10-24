//! Structural audit for the sole construction owner. Runtime regressions live
//! beside host construction and shadow disagreement tests; these checks only
//! prevent reintroducing the retired competing writer/selector.
pub(super) fn host_owns_construction() -> bool {
    let host = include_str!("../world_tick/production.rs");
    let status_owner = include_str!("../world_scripts/stealth_mines/construction_clearance.rs");
    let completion_owner = include_str!("../world_scripts/production_eva.rs");
    let shadow = include_str!("../../gameworld_shadow/construction_observation.rs");
    let boundary = include_str!("../../gameworld_shadow/host_observation_boundary.rs");
    // Construction completion was split into synchronous status/effect and
    // notification owners. Follow those live calls instead of requiring the
    // pre-split inline mutations to remain in production.rs.
    host.contains("obj.construction_percent = projected")
        && host.contains("if projected >= 1.0")
        && host.contains("self.set_object_under_construction_with_effects(id, false, ui);")
        && observes_call_result(
            host,
            completion_owner,
            "notify_structure_construction_complete_with_effects",
            "self.notify_structure_construction_complete_with_effects(completed_id, None, ui)",
        )
        && status_owner.contains("object.set_status_under_construction(value);")
        && observes_call_result(
            completion_owner,
            completion_owner,
            "finish_structure_construction",
            "self.finish_structure_construction(structure_id, interpreter, |_, _| {}, ui)",
        )
        && completion_owner
            .contains("self.set_object_under_construction_with_effects(structure_id, false, ui);")
        && shadow.contains("pub fn compare_construction_to_host(&self, logic: &GameLogic)")
        && shadow.contains("pub fn compare_rebuild_producer_to_host(&self, logic: &GameLogic)")
        && !shadow.contains("pub fn writeback_construction_to_host")
        && !shadow.contains("pub fn writeback_rebuild_producer_to_host")
        && !shadow.contains("pub fn tick_construction_progress")
        && boundary.contains("shadow.sync_from_host(logic)")
}

// Check the actual callee's error channel. A void status operation has no
// Result to propagate; a fallible completion must retain its `?` at the call.
fn observes_call_result(caller: &str, callee_source: &str, callee: &str, call: &str) -> bool {
    let declaration = format!("fn {callee}(");
    let Some((_, definition)) = callee_source.split_once(&declaration) else {
        return false;
    };
    let Some((header, _)) = definition.split_once('{') else {
        return false;
    };
    let suffix = if header.contains("GameLogicResult<") || header.contains("Result<") {
        "?;"
    } else {
        ";"
    };
    caller.contains(&format!("{call}{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn completion_checks_follow_the_declared_error_channel() {
        let checked = "fn complete(id: u32) -> GameLogicResult<()> { Ok(()) }";
        assert!(observes_call_result(
            "self.complete(id)?;",
            checked,
            "complete",
            "self.complete(id)"
        ));
        assert!(!observes_call_result(
            "self.complete(id);",
            checked,
            "complete",
            "self.complete(id)"
        ));
        assert!(observes_call_result(
            "self.complete(id);",
            "fn complete(id: u32) {}",
            "complete",
            "self.complete(id)"
        ));
        assert!(!observes_call_result(
            "self.complete(id)?;",
            "",
            "complete",
            "self.complete(id)"
        ));
    }
}
