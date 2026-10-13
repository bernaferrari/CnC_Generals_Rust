//! Structural audit for the sole construction owner. Runtime regressions live
//! beside host construction and shadow disagreement tests; these checks only
//! prevent reintroducing the retired competing writer/selector.
pub(super) fn host_owns_construction() -> bool {
    let host = include_str!("../world_tick/production.rs");
    let shadow = include_str!("../../gameworld_shadow/construction_observation.rs");
    let boundary = include_str!("../../gameworld_shadow/host_observation_boundary.rs");
    host.contains("obj.construction_percent = projected")
        && host.contains("if projected >= 1.0")
        && host.contains("obj.set_status_under_construction(false)")
        && host.contains("notify_structure_construction_complete")
        && shadow.contains("pub fn compare_construction_to_host(&self, logic: &GameLogic)")
        && shadow.contains("pub fn compare_rebuild_producer_to_host(&self, logic: &GameLogic)")
        && !shadow.contains("pub fn writeback_construction_to_host")
        && !shadow.contains("pub fn writeback_rebuild_producer_to_host")
        && !shadow.contains("pub fn tick_construction_progress")
        && boundary.contains("shadow.sync_from_host(logic)")
}
