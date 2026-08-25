use super::*;

pub(super) fn pending_command_accepts_object(options: u32) -> bool {
    options
        & (CMD_NEED_TARGET_ENEMY_OBJECT
            | CMD_NEED_TARGET_NEUTRAL_OBJECT
            | CMD_NEED_TARGET_ALLY_OBJECT
            | CMD_NEED_TARGET_PRISONER)
        != 0
}

pub(super) fn pending_command_accepts_position(options: u32) -> bool {
    options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION) != 0
}

pub(super) fn relationship_to_target(
    local_player_id: i32,
    target_id: ObjectID,
) -> Option<Relationship> {
    if local_player_id < 0 {
        return None;
    }
    // Wave 973/1043: host empty dual-world → team residual relationship.
    // Disguised units present apparent team to non-allied viewers (C++ InGameUI).
    if dual_world_registry_unavailable() {
        let entry = translator_catalog_entry(target_id)?;
        let local = translator_local_team_name();
        if local.is_empty() {
            return None;
        }
        // Wave 1066: FOW fogged/black non-local relationship residual fail-closed.
        if entry.shroud_status >= 2 && !translator_entry_is_local(&entry) {
            return None;
        }
        // Wave 1073: destroyed/sold/masked/unselectable relationship residual fail-closed.
        if entry.destroyed || entry.sold || entry.masked || entry.unselectable {
            return None;
        }
        // Wave 1073: non-local effectively-stealthed relationship residual fail-closed.
        if entry.effectively_stealthed && !translator_entry_is_local(&entry) {
            return None;
        }
        let apparent = translator_entry_apparent_team(&entry);
        return Some(if apparent == local {
            Relationship::Allies
        } else if apparent == "Neutral" || local == "Neutral" {
            Relationship::Neutral
        } else {
            Relationship::Enemies
        });
    }

    let target = OBJECT_REGISTRY.get_object(target_id)?;
    let target_guard = target.read().ok()?;
    let owner = target_guard.get_controlling_player_id()?;

    let list = player_list().read().ok()?;
    let me = list.get_player(local_player_id)?;
    let them = list.get_player(owner as i32)?;
    let (Ok(me_guard), Ok(them_guard)) = (me.read(), them.read()) else {
        return None;
    };

    Some(me_guard.get_relationship(&them_guard))
}

pub(super) fn is_prisoner_target(target_id: ObjectID) -> bool {
    // Wave 973/1049: host empty dual-world → presentation catalog residual.
    if dual_world_registry_unavailable() {
        let Some(e) = translator_catalog_entry(target_id) else {
            return false;
        };
        if !dual_target_status_ok(&e) {
            return false;
        }
        // Wave 1082: disabled prisoner residual fail-closed.
        if e.disabled {
            return false;
        }
        // Wave 1074: FOW/stealth non-local prisoner residual fail-closed.
        if e.shroud_status >= 2 && !translator_entry_is_local(&e) {
            return false;
        }
        if e.effectively_stealthed && !translator_entry_is_local(&e) {
            return false;
        }
        return translator_entry_has_kind(&e, "CanSurrender")
            || translator_entry_has_kind(&e, "Prison")
            || translator_entry_has_kind(&e, "PowTruck");
    }
    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };
    target_guard.is_kind_of(KindOf::CanSurrender)
        || target_guard.is_kind_of(KindOf::Prison)
        || target_guard.is_kind_of(KindOf::PowTruck)
}

pub(super) fn pending_command_target_allowed(
    options: u32,
    local_player_id: i32,
    target_id: ObjectID,
) -> bool {
    let needs_enemy = options & CMD_NEED_TARGET_ENEMY_OBJECT != 0;
    let needs_neutral = options & CMD_NEED_TARGET_NEUTRAL_OBJECT != 0;
    let needs_ally = options & CMD_NEED_TARGET_ALLY_OBJECT != 0;
    let needs_prisoner = options & CMD_NEED_TARGET_PRISONER != 0;

    if !(needs_enemy || needs_neutral || needs_ally || needs_prisoner) {
        return true;
    }

    if needs_prisoner && is_prisoner_target(target_id) {
        return true;
    }

    let Some(relationship) = relationship_to_target(local_player_id, target_id) else {
        return false;
    };

    if needs_enemy && matches!(relationship, Relationship::Enemies) {
        return true;
    }
    if needs_neutral && matches!(relationship, Relationship::Neutral) {
        return true;
    }
    if needs_ally && matches!(relationship, Relationship::Allies) {
        return true;
    }

    false
}

pub(super) fn weapon_slot_from_u32(value: u32) -> WeaponSlotType {
    match value {
        1 => WeaponSlotType::Secondary,
        2 => WeaponSlotType::Tertiary,
        _ => WeaponSlotType::Primary,
    }
}

pub(super) fn pending_weapon_slot(pending: &PendingCommand) -> WeaponSlotType {
    weapon_slot_from_u32(pending.source_object_id)
}

pub(super) fn pending_special_power_payload()
-> Option<(crate::helpers::PendingSpecialPower, SpecialPowerTemplate)> {
    let power = TheInGameUI::get_pending_special_power()?;
    let store = get_special_power_store()?;
    let template = store
        .find_special_power_template_by_id(power.power_id)?
        .clone();
    Some((power, template))
}

pub(super) fn pending_fire_weapon_can_target_object(
    pending: &PendingCommand,
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };
    let slot = pending_weapon_slot(pending);
    let mut saw_owned_source = false;

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
        saw_owned_source = true;

        if ActionManager::can_fire_weapon_at_object(
            &sel_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
            slot,
        ) {
            return true;
        }
    }

    !saw_owned_source
}

pub(super) fn pending_fire_weapon_can_target_position(
    pending: &PendingCommand,
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    position: &Coord3D,
    object_in_way: Option<ObjectID>,
) -> bool {
    let slot = pending_weapon_slot(pending);
    let logic_pos = LogicCoord3D::new(position.x, position.y, position.z);
    let object_in_way_obj = object_in_way.and_then(|id| OBJECT_REGISTRY.get_object(id));
    let mut saw_owned_source = false;

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
        saw_owned_source = true;
        let object_in_way_guard = object_in_way_obj.as_ref().and_then(|obj| obj.read().ok());

        if ActionManager::can_fire_weapon_at_location(
            &sel_guard,
            &logic_pos,
            CommandSourceType::FromPlayer,
            slot,
            object_in_way_guard.as_deref(),
        ) {
            return true;
        }
    }

    !saw_owned_source
}

pub(super) fn pending_special_power_can_target_object(
    pending: &PendingCommand,
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };
    let Some((power, template)) = pending_special_power_payload() else {
        // Keep legacy permissive behavior when special-power metadata isn't available yet.
        return true;
    };
    let mut saw_owned_source = false;

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

        if power.source_object_id != gamelogic::common::INVALID_ID
            && sel_guard.get_id() != power.source_object_id
        {
            continue;
        }
        saw_owned_source = true;

        if ActionManager::can_do_special_power_at_object(
            &sel_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
            &template,
            power.options,
            true,
        ) {
            return true;
        }
    }

    !saw_owned_source
}

pub(super) fn pending_special_power_can_target_position(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    position: &Coord3D,
    object_in_way: Option<ObjectID>,
) -> bool {
    let Some((power, template)) = pending_special_power_payload() else {
        // Keep legacy permissive behavior when special-power metadata isn't available yet.
        return true;
    };
    let logic_pos = LogicCoord3D::new(position.x, position.y, position.z);
    let object_in_way_obj = object_in_way.and_then(|id| OBJECT_REGISTRY.get_object(id));
    let mut saw_owned_source = false;

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

        if power.source_object_id != gamelogic::common::INVALID_ID
            && sel_guard.get_id() != power.source_object_id
        {
            continue;
        }
        saw_owned_source = true;
        let object_in_way_guard = object_in_way_obj.as_ref().and_then(|obj| obj.read().ok());

        if ActionManager::can_do_special_power_at_location(
            &sel_guard,
            &logic_pos,
            CommandSourceType::FromPlayer,
            &template,
            object_in_way_guard.as_deref(),
            power.options,
            true,
        ) {
            return true;
        }
    }

    !saw_owned_source
}

pub(super) fn pending_command_for_object(
    pending: &PendingCommand,
    target: ObjectID,
) -> Option<GameMessageType> {
    match pending.command_type {
        CommandType::CombatDropAtObject => Some(GameMessageType::CombatDropAtObject(target)),
        CommandType::DoWeaponAtObject | CommandType::DoAttackObject => {
            if pending.options & CMD_ATTACK_OBJECTS_POSITION != 0 {
                None
            } else {
                Some(GameMessageType::DoWeaponAtObject(
                    pending.source_object_id,
                    target,
                ))
            }
        }
        CommandType::DoSpecialPowerAtObject | CommandType::DoSpecialPower => {
            if !pending_command_accepts_object(pending.options) {
                return None;
            }
            TheInGameUI::get_pending_special_power().map(|power| {
                GameMessageType::DoSpecialPowerAtObject(
                    power.power_id,
                    target,
                    power.options,
                    power.source_object_id,
                )
            })
        }
        CommandType::ConvertToCarbomb => Some(GameMessageType::ConvertToCarbomb(
            pending.source_object_id,
            target,
        )),
        CommandType::CaptureBuilding => Some(GameMessageType::CaptureBuilding(
            pending.source_object_id,
            target,
        )),
        CommandType::DisableVehicleHack => Some(GameMessageType::DisableVehicleHack(
            pending.source_object_id,
            target,
        )),
        CommandType::StealCashHack => Some(GameMessageType::StealCashHack(
            pending.source_object_id,
            target,
        )),
        CommandType::DisableBuildingHack => Some(GameMessageType::DisableBuildingHack(
            pending.source_object_id,
            target,
        )),
        CommandType::SnipeVehicle => Some(GameMessageType::SnipeVehicle(
            pending.source_object_id,
            target,
        )),
        CommandType::DoGuardObject => Some(GameMessageType::DoGuardObject(target, 0)),
        CommandType::Enter => Some(GameMessageType::Enter(0, target)),
        CommandType::DoRepair => Some(GameMessageType::DoRepair(target)),
        CommandType::GetRepaired => Some(GameMessageType::GetRepaired(target)),
        CommandType::GetHealed => Some(GameMessageType::GetHealed(target)),
        CommandType::ResumeConstruction => Some(GameMessageType::ResumeConstruction(target)),
        CommandType::Dock => Some(GameMessageType::Dock(target)),
        _ => None,
    }
}

pub(super) fn pending_command_hint_for_object(
    pending: &PendingCommand,
    _local_player: i32,
    local_player_u32: Option<u32>,
    selection: &HashSet<ObjectID>,
    target: ObjectID,
) -> Option<GameMessageType> {
    match pending.command_type {
        CommandType::ConvertToCarbomb => Some(GameMessageType::ConvertToCarbombHint(target)),
        CommandType::CaptureBuilding => Some(GameMessageType::CaptureBuildingHint(target)),
        CommandType::DisableVehicleHack
        | CommandType::StealCashHack
        | CommandType::DisableBuildingHack => Some(GameMessageType::HackHint(target)),
        CommandType::Enter => {
            if selection_can_hijack_target(local_player_u32, selection, target) {
                Some(GameMessageType::HijackHint(target))
            } else if selection_can_sabotage_target(local_player_u32, selection, target) {
                Some(GameMessageType::SabotageHint(target))
            } else {
                Some(GameMessageType::EnterHint(target))
            }
        }
        CommandType::DoRepair => Some(GameMessageType::DoRepairHint(target)),
        CommandType::GetRepaired => Some(GameMessageType::GetRepairedHint(target)),
        CommandType::GetHealed => Some(GameMessageType::GetHealedHint(target)),
        CommandType::ResumeConstruction => Some(GameMessageType::ResumeConstructionHint(target)),
        CommandType::Dock => Some(GameMessageType::DockHint(target)),
        CommandType::DoAttackMoveTo => None,
        CommandType::DoGuardPosition | CommandType::DoGuardObject => None,
        _ => {
            if selection_can_capture_building_target(local_player_u32, selection, target) {
                Some(GameMessageType::CaptureBuildingHint(target))
            } else if selection_can_disable_vehicle_hack_target(local_player_u32, selection, target)
                || selection_can_steal_cash_hack_target(local_player_u32, selection, target)
                || selection_can_disable_building_hack_target(local_player_u32, selection, target)
            {
                Some(GameMessageType::HackHint(target))
            } else {
                None
            }
        }
    }
}

pub(super) fn pending_command_hint_for_position(
    pending: &PendingCommand,
    position: Coord3D,
) -> Option<GameMessageType> {
    match pending.command_type {
        CommandType::DoAttackMoveTo => Some(GameMessageType::DoAttackMoveToHint(position)),
        CommandType::SetRallyPoint => Some(GameMessageType::SetRallyPointHint(position)),
        CommandType::DoSpecialPowerAtLocation
        | CommandType::DoWeaponAtLocation
        | CommandType::CombatDropAtLocation => None,
        CommandType::DoGuardPosition => None,
        CommandType::DoGuardObject => None,
        CommandType::PlaceBeacon | CommandType::RemoveBeacon => None,
        _ if pending_command_accepts_position(pending.options) => {
            Some(GameMessageType::DoMoveToHint(position))
        }
        _ => None,
    }
}

pub(super) fn pending_command_for_position(
    pending: &PendingCommand,
    position: Coord3D,
    object_in_way: Option<ObjectID>,
) -> Option<GameMessageType> {
    match pending.command_type {
        CommandType::CombatDropAtLocation => Some(GameMessageType::CombatDropAtLocation(position)),
        CommandType::DoWeaponAtLocation | CommandType::DoAttackObject => {
            if !(pending_command_accepts_position(pending.options)
                || pending.options & CMD_ATTACK_OBJECTS_POSITION != 0)
            {
                return None;
            }
            Some(GameMessageType::DoWeaponAtLocation(
                pending.source_object_id,
                position,
            ))
        }
        CommandType::DoSpecialPowerAtLocation | CommandType::DoSpecialPower => {
            if !pending_command_accepts_position(pending.options) {
                return None;
            }
            TheInGameUI::get_pending_special_power().map(|power| {
                GameMessageType::DoSpecialPowerAtLocation(
                    power.power_id,
                    position,
                    -1.0,
                    object_in_way.unwrap_or(gamelogic::common::INVALID_ID),
                    power.options,
                    power.source_object_id,
                )
            })
        }
        CommandType::DoAttackMoveTo => Some(GameMessageType::DoAttackMoveTo(position)),
        CommandType::DoGuardPosition => Some(GameMessageType::DoGuardPosition(position, 0)),
        CommandType::Evacuate => {
            if pending_command_accepts_position(pending.options) {
                Some(GameMessageType::EvacuateAtLocation(position))
            } else {
                Some(GameMessageType::Evacuate)
            }
        }
        CommandType::PlaceBeacon => Some(GameMessageType::PlaceBeacon(position.clone())),
        CommandType::RemoveBeacon => Some(GameMessageType::RemoveBeacon(position.clone())),
        CommandType::SetRallyPoint => Some(GameMessageType::SetRallyPoint(
            pending.source_object_id,
            position,
        )),
        _ => None,
    }
}

pub(super) fn pending_command_messages_for_position(
    pending: &PendingCommand,
    position: Coord3D,
    selection: &HashSet<ObjectID>,
    object_in_way: Option<ObjectID>,
) -> Vec<GameMessageType> {
    if pending.command_type == CommandType::SetRallyPoint {
        let mut ids: Vec<ObjectID> = selection.iter().copied().collect();
        ids.sort_unstable();
        return ids
            .into_iter()
            .map(|id| GameMessageType::SetRallyPoint(id, position.clone()))
            .collect();
    }

    pending_command_for_position(pending, position, object_in_way)
        .into_iter()
        .collect()
}

pub(super) fn selection_source_object_id(
    selection: &HashSet<ObjectID>,
    local_player_u32: Option<u32>,
) -> ObjectID {
    // Wave 973: host empty dual-world → prefer local-team catalog residual.
    if selection.is_empty() {
        return 0;
    }
    if dual_world_registry_unavailable() {
        // Wave 1049: prefer living local source residual.
        // Wave 1111: also fail-closed on masked/unselectable local sources
        // (parity with selection_attack_result / dual_target_status_ok peels).
        let usable_local = |e: &crate::presentation_translator_residual::TranslatorCatalogEntry| {
            translator_entry_is_local(e)
                && !e.destroyed
                && !e.sold
                && !e.disabled
                && !e.under_construction
                && !e.masked
                && !e.unselectable
        };
        for &id in selection {
            if let Some(e) = translator_catalog_entry(id) {
                // Wave 1074: under-construction local source residual fail-closed.
                // Wave 1111: masked/unselectable local source residual fail-closed.
                if usable_local(&e) {
                    return id;
                }
            }
        }
        // Wave 1075/1111: no unusable local fallback residual.
        for &id in selection {
            if let Some(e) = translator_catalog_entry(id) {
                if usable_local(&e) {
                    return id;
                }
            }
        }
        return 0;
    }
    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let is_mine = local_player_u32
            .and_then(|pid| {
                sel_guard
                    .get_controlling_player_id()
                    .map(|owner| owner == pid)
            })
            .unwrap_or(false);
        if is_mine {
            return id;
        }
    }

    gamelogic::common::INVALID_ID
}
