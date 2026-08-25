//! Host GameLogic tests — `unit_residuals`.
#![allow(unused_imports, non_snake_case, unused_variables, dead_code)]
use super::super::*;
use super::helpers::*;
#[path = "../hq_nnuu_live_harness.rs"]
mod hq_nnuu_live_harness;
use hq_nnuu_live_harness::*;

// -----------------------------------------------------------------------
// China Troop Crawler residual (transport 8 + Redguard payload + assault + detector)
// Fail-closed: not multi-exit-path / HealthRegen / wounded retrieve matrix.
// -----------------------------------------------------------------------

fn strip_troop_crawler_payload(game_logic: &mut GameLogic, crawler_id: ObjectId) {
    let old_occ = game_logic
        .host_object(crawler_id)
        .map(|c| c.contained_units())
        .unwrap_or_default();
    for oid in old_occ {
        if let Some(c) = game_logic.host_object_mut(crawler_id) {
            c.remove_occupant(oid);
        }
        if let Some(u) = game_logic.host_object_mut(oid) {
            u.set_contained_by(None);
        }
        game_logic.destroy_object(oid);
    }
}

// Behavior-named suites keep each test file below the 4k LOC ceiling.
mod infantry_and_transport;
mod projectiles_and_vehicle_residuals;
