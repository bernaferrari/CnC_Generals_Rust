use super::*;

/// Wave 249: host/presentation path has no dual-world factory objects.
#[inline]
pub(super) fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

/// Wave 1047: dual catalog target legality for context enter/repair/resume.
#[inline]
pub(super) fn dual_target_status_ok(
    target: &crate::presentation_translator_residual::TranslatorCatalogEntry,
) -> bool {
    !target.destroyed && !target.sold && !target.unselectable && !target.masked
}

/// Wave 1047: dual catalog apparent-ally residual (local team == apparent team).
#[inline]
pub(super) fn dual_target_is_apparent_ally(
    target: &crate::presentation_translator_residual::TranslatorCatalogEntry,
) -> bool {
    let local = translator_local_team_name();
    if local.is_empty() {
        return false;
    }
    translator_entry_apparent_team(target) == local
}

pub(super) fn selection_any_local_object_can_target<F>(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
    mut can_do: F,
) -> bool
where
    F: FnMut(&gamelogic::object::Object, &gamelogic::object::Object) -> bool,
{
    if selection.is_empty() {
        return false;
    }
    // Wave 975/1048: host empty dual-world → presentation catalog residual.
    // ActionManager dual-world can_do is unavailable; residual answers "local selection
    // has a unit and target is known" so command-hint / context paths can proceed.
    // Authoritative command issuance remains Main presentation-command peels.
    // Wave 1048: fail-closed on destroyed/sold/unselectable/masked targets and
    // non-local effectively-stealthed residuals.
    if dual_world_registry_unavailable() {
        let Some(target) = translator_catalog_entry(target_id) else {
            return false;
        };
        if !dual_target_status_ok(&target) {
            return false;
        }
        if target.effectively_stealthed && !translator_entry_is_local(&target) {
            return false;
        }
        // Wave 1064: FOW fogged/black non-local targets fail-closed.
        if target.shroud_status >= 2 && !translator_entry_is_local(&target) {
            return false;
        }
        for &id in selection {
            if let Some(sel) = translator_catalog_entry(id) {
                // Wave 1067: under-construction local source residual fail-closed.
                if translator_entry_is_local(&sel)
                    && !sel.destroyed
                    && !sel.sold
                    && !sel.disabled
                    && !sel.under_construction
                    && !sel.masked
                    && !sel.unselectable
                {
                    return true;
                }
            }
        }
        return false;
    }
    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let is_mine = local_player
            .and_then(|pid| {
                sel_guard
                    .get_controlling_player_id()
                    .map(|owner| owner == pid)
            })
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if can_do(&sel_guard, &target_guard) {
            return true;
        }
    }

    false
}

pub(super) fn selection_can_enter_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    // Wave 975/1047: host empty dual-world → presentation catalog residual.
    if dual_world_registry_unavailable() {
        let Some(target) = translator_catalog_entry(target_id) else {
            return false;
        };
        if !dual_target_status_ok(&target) {
            return false;
        }
        // Wave 1064: FOW fogged/black non-local targets fail-closed.
        if target.shroud_status >= 2 && !translator_entry_is_local(&target) {
            return false;
        }
        // Enter residual: apparent ally container only (C++ relationship gate).
        if !dual_target_is_apparent_ally(&target) {
            return false;
        }
        // Transport/garrison residual: selectable structure/container kinds.
        let transportish = translator_entry_has_kind(&target, "Structure")
            || translator_entry_has_kind(&target, "Transport")
            || translator_entry_has_kind(&target, "Vehicle")
            || target.selectable;
        if !transportish {
            return false;
        }
        // Wave 1068: full garrison residual fail-closed (C++ contain capacity).
        if target.max_garrison > 0 && target.occupant_count >= target.max_garrison {
            return false;
        }
        // Wave 1068: under-construction container residual fail-closed.
        if target.under_construction {
            return false;
        }
        // Wave 1072: disabled container residual fail-closed.
        if target.disabled {
            return false;
        }
        for &id in selection {
            if let Some(sel) = translator_catalog_entry(id) {
                // Wave 1068: unusable local source residual fail-closed.
                if translator_entry_is_local(&sel)
                    && !sel.destroyed
                    && !sel.sold
                    && !sel.disabled
                    && !sel.under_construction
                    && !sel.masked
                    && !sel.unselectable
                {
                    return true;
                }
            }
        }
        return false;
    }

    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let is_mine = local_player
            .and_then(|pid| {
                sel_guard
                    .get_controlling_player_id()
                    .map(|owner| owner == pid)
            })
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if ActionManager::can_enter_object(
            &sel_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
            CanEnterType::CheckCapacity,
        ) {
            return true;
        }
    }

    false
}

pub(super) fn selection_can_repair_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    // Wave 975/1047: host empty dual-world → presentation catalog residual.
    if dual_world_registry_unavailable() {
        let Some(target) = translator_catalog_entry(target_id) else {
            return false;
        };
        if !dual_target_status_ok(&target) || !dual_target_is_apparent_ally(&target) {
            return false;
        }
        // Wave 1069: FOW fogged/black non-local repair target residual fail-closed.
        if target.shroud_status >= 2 && !translator_entry_is_local(&target) {
            return false;
        }
        // Wave 1069: under-construction is resume, not repair.
        if target.under_construction {
            return false;
        }
        // Wave 1069: undamaged residual fail-closed (health_current >= health_maximum).
        if target.health_maximum > 0.0 && target.health_current >= target.health_maximum {
            return false;
        }
        // Dozer repair residual: local selection vs damaged structure/vehicle residual.
        let repairable = translator_entry_has_kind(&target, "Structure")
            || translator_entry_has_kind(&target, "Vehicle")
            || translator_entry_has_kind(&target, "Dozer");
        if !repairable {
            return false;
        }
        for &id in selection {
            if let Some(sel) = translator_catalog_entry(id) {
                // Wave 1069: unusable local source residual fail-closed.
                if translator_entry_is_local(&sel)
                    && !sel.destroyed
                    && !sel.sold
                    && !sel.disabled
                    && !sel.under_construction
                    && !sel.masked
                    && !sel.unselectable
                    && (translator_entry_has_kind(&sel, "Dozer")
                        || translator_entry_has_kind(&sel, "Vehicle")
                        || sel.selectable)
                {
                    return true;
                }
            }
        }
        return false;
    }

    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };
    let current_repairer = target_guard.get_sole_healing_benefactor();

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let is_mine = local_player
            .and_then(|pid| {
                sel_guard
                    .get_controlling_player_id()
                    .map(|owner| owner == pid)
            })
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if ActionManager::can_repair_object(
            &sel_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
        ) && (current_repairer == gamelogic::common::INVALID_ID || current_repairer == id)
        {
            return true;
        }
    }

    false
}

pub(super) fn selection_can_get_repaired_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        ActionManager::can_get_repaired_at(selected, target, CommandSourceType::FromPlayer)
    })
}

pub(super) fn selection_can_get_healed_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        if !ActionManager::can_get_healed_at(selected, target, CommandSourceType::FromPlayer) {
            return false;
        }

        if let Some(contain) = target.get_contain() {
            if let Ok(contain_guard) = contain.lock() {
                if contain_guard.is_heal_contain() {
                    return false;
                }
            }
        }

        true
    })
}

pub(super) fn selection_can_hijack_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        ActionManager::can_hijack_vehicle(selected, target, CommandSourceType::FromPlayer)
    })
}

pub(super) fn selection_can_sabotage_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        ActionManager::can_sabotage_building(selected, target, CommandSourceType::FromPlayer)
    })
}

pub(super) fn selection_can_capture_building_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        ActionManager::can_capture_building(selected, target, CommandSourceType::FromPlayer)
    })
}

pub(super) fn selection_can_disable_vehicle_hack_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        ActionManager::can_disable_vehicle_via_hacking(
            selected,
            target,
            CommandSourceType::FromPlayer,
            true,
        )
    })
}

pub(super) fn selection_can_steal_cash_hack_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        ActionManager::can_steal_cash_via_hacking(selected, target, CommandSourceType::FromPlayer)
    })
}

pub(super) fn selection_can_disable_building_hack_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        ActionManager::can_disable_building_via_hacking(
            selected,
            target,
            CommandSourceType::FromPlayer,
        )
    })
}

pub(super) fn selection_can_pickup_crate_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> Option<Coord3D> {
    // Wave 975/1047: host empty dual-world → presentation catalog residual.
    if dual_world_registry_unavailable() {
        let Some(target) = translator_catalog_entry(target_id) else {
            return None;
        };
        // Wave 1072: crate dual fail-closed on status/FOW and unusable local sources.
        if target.destroyed
            || target.sold
            || target.unselectable
            || target.masked
            || target.disabled
        {
            return None;
        }
        if target.shroud_status >= 2 && !translator_entry_is_local(&target) {
            return None;
        }
        if !translator_entry_has_kind(&target, "Crate") {
            return None;
        }
        for &id in selection {
            if let Some(sel) = translator_catalog_entry(id) {
                if translator_entry_is_local(&sel)
                    && !sel.destroyed
                    && !sel.sold
                    && !sel.disabled
                    && !sel.under_construction
                    && !sel.masked
                    && !sel.unselectable
                {
                    return Some(Coord3D::new(
                        target.position[0],
                        target.position[1],
                        target.position[2],
                    ));
                }
            }
        }
        return None;
    }

    let target = OBJECT_REGISTRY.get_object(target_id)?;
    let target_guard = target.read().ok()?;
    if !target_guard.is_kind_of(KindOf::Crate)
        || target_guard.is_salvage_crate()
        || target_guard.is_effectively_dead()
    {
        return None;
    }

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let is_mine = local_player
            .and_then(|pid| {
                sel_guard
                    .get_controlling_player_id()
                    .map(|owner| owner == pid)
            })
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if sel_guard.is_mobile() {
            let pos = target_guard.get_position();
            return Some(Coord3D::new(pos.x, pos.y, pos.z));
        }
    }

    None
}

pub(super) fn selection_can_salvage_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> Option<Coord3D> {
    let target = OBJECT_REGISTRY.get_object(target_id)?;
    let target_guard = target.read().ok()?;
    if !target_guard.is_salvage_crate() {
        return None;
    }

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let is_mine = local_player
            .and_then(|pid| {
                sel_guard
                    .get_controlling_player_id()
                    .map(|owner| owner == pid)
            })
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if sel_guard.is_kind_of(KindOf::Salvager) {
            let pos = target_guard.get_position();
            return Some(Coord3D::new(pos.x, pos.y, pos.z));
        }
    }

    None
}

pub(super) fn selection_can_resume_construction_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    // Wave 975/1047: host empty dual-world → presentation catalog residual.
    if dual_world_registry_unavailable() {
        let Some(target) = translator_catalog_entry(target_id) else {
            return false;
        };
        if !dual_target_status_ok(&target) || !dual_target_is_apparent_ally(&target) {
            return false;
        }
        // Wave 1069: FOW fogged/black non-local resume target residual fail-closed.
        if target.shroud_status >= 2 && !translator_entry_is_local(&target) {
            return false;
        }
        if !target.under_construction {
            return false;
        }
        if !(translator_entry_has_kind(&target, "Structure") || target.selectable) {
            return false;
        }
        for &id in selection {
            if let Some(sel) = translator_catalog_entry(id) {
                // Wave 1069: unusable local source residual fail-closed.
                if translator_entry_is_local(&sel)
                    && !sel.destroyed
                    && !sel.sold
                    && !sel.disabled
                    && !sel.under_construction
                    && !sel.masked
                    && !sel.unselectable
                    && (translator_entry_has_kind(&sel, "Dozer") || sel.selectable)
                {
                    return true;
                }
            }
        }
        return false;
    }

    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let is_mine = local_player
            .and_then(|pid| {
                sel_guard
                    .get_controlling_player_id()
                    .map(|owner| owner == pid)
            })
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if ActionManager::can_resume_construction_of(
            &sel_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
        ) {
            return true;
        }
    }

    false
}

pub(super) fn selection_can_dock_at_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        ActionManager::can_dock_at(selected, target, CommandSourceType::FromPlayer)
    })
}

pub(super) fn selection_can_convert_to_carbomb_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_any_local_object_can_target(local_player, selection, target_id, |selected, target| {
        ActionManager::can_convert_object_to_car_bomb(
            selected,
            target,
            CommandSourceType::FromPlayer,
        )
    })
}

pub(super) fn selection_can_attack_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    matches!(
        selection_attack_result(local_player, selection, target_id),
        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
    )
}
