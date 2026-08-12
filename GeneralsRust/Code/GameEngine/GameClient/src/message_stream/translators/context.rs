use super::*;

pub(super) fn pick_context_target_for_click(
    region: &IRegion2D,
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    force_attack_mode: bool,
) -> Option<ObjectID> {
    pub(super) const PICK_RADIUS_WORLD: f32 = 10.0;

    let profile = context_pick_profile(force_attack_mode, selection);
    let (mut mine, mut other) =
        collect_selectable_objects(region, true, PICK_RADIUS_WORLD, local_player, profile);
    let mine_pick = pick_closest(&mut mine);
    let other_pick = pick_closest(&mut other);

    match (mine_pick, other_pick) {
        (Some(mine_id), Some(other_id)) => {
            let mine_dist = mine
                .iter()
                .find(|(id, _)| *id == mine_id)
                .map(|(_, d)| *d)
                .unwrap_or(f32::MAX);
            let other_dist = other
                .iter()
                .find(|(id, _)| *id == other_id)
                .map(|(_, d)| *d)
                .unwrap_or(f32::MAX);
            if mine_dist <= other_dist {
                Some(mine_id)
            } else {
                Some(other_id)
            }
        }
        (Some(id), None) | (None, Some(id)) => Some(id),
        (None, None) => None,
    }
}

pub(super) fn is_locally_controlled_mine_target(object_id: ObjectID) -> bool {
    // Wave 973/1048: host empty dual-world → presentation catalog residual.
    if dual_world_registry_unavailable() {
        // Wave 1074: mine dual fail-closed on FOW/stealth non-local residuals.
        return translator_catalog_entry(object_id)
            .map(|e| {
                dual_target_status_ok(&e)
                    && translator_entry_is_local(&e)
                    && !(e.shroud_status >= 2)
                    && !e.effectively_stealthed
                    && translator_catalog_has_kind(object_id, "Mine")
            })
            .unwrap_or(false);
    }
    OBJECT_REGISTRY
        .get_object(object_id)
        .and_then(|obj| {
            obj.read()
                .ok()
                .map(|guard| guard.is_kind_of(KindOf::Mine) && guard.is_locally_controlled())
        })
        .unwrap_or(false)
}

pub(super) fn is_pending_gui_non_context_command(pending: &PendingCommand) -> bool {
    if (pending.options & CMD_CONTEXTMODE_COMMAND) != 0 {
        return false;
    }

    matches!(
        pending.command_type,
        CommandType::DoAttackMoveTo
            | CommandType::DoGuardPosition
            | CommandType::DoGuardObject
            | CommandType::SetRallyPoint
            | CommandType::PlaceBeacon
            | CommandType::RemoveBeacon
            | CommandType::DoAttackObject
            | CommandType::DoWeaponAtObject
            | CommandType::DoWeaponAtLocation
            | CommandType::Evacuate
    )
}
