//! Freeze ControlBar's beacon-limit decision from the receiving world.

use crate::game_logic::{GameLogic, ObjectId};

/// ControlBar.cpp:1486-1503 counts the local Player's exact beacon template
/// through its team membership, including dead objects and excluding objects
/// under construction (Player.h's default argument).
pub(super) fn freeze(logic: &GameLogic, player_id: u32) -> Option<bool> {
    logic.get_player(player_id)?;
    let store = logic.player_template_definitions.as_ref()?;
    let player_template = logic
        .player_template_identity(player_id)?
        .resolve_in(store)?;
    let beacon_name = player_template.get_beacon_template();
    let admitted = logic
        .receiving_module_content
        .as_ref()
        .and_then(|content| content.admitted_template(beacon_name));
    let host_template = logic.templates.get(beacon_name);
    let teams = logic
        .team_factory
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .owned_team_instances_for_controller(player_id);
    let mut count = 0i32;
    for team in teams {
        let team = team.read().unwrap_or_else(|error| error.into_inner());
        for raw_id in team.get_members() {
            let Some(object) = logic.host_object(ObjectId(*raw_id)) else {
                continue;
            };
            if object.status.under_construction {
                continue;
            }
            let object_template = object.get_template();
            let equivalent = match (&object_template.receiving_client_template, &admitted) {
                (Some(source), Some(target)) => source.is_equivalent_to_admitted(target),
                _ => host_template.is_some_and(|target| object_template.is_equivalent_to(target)),
            };
            if equivalent {
                count = count.saturating_add(1);
            }
        }
    }
    Some(count < logic.max_beacons_per_player)
}
