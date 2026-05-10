#![allow(missing_docs)]

//! Message Stream Translators
//!
//! This module contains implementations of various message translators that
//! convert raw input events into tactical commands and game actions.
//!
//! Translators are the heart of the message stream system, processing messages
//! in priority order and deciding whether to keep, modify, or destroy them.

use super::command_list::get_command_list;
use super::game_message::*;
use super::hot_key::HotKeyTranslator;
use super::look_at_xlat::LookAtTranslator;
use super::message_stream::{GameMessageDisposition, GameMessageTranslator};
use super::meta_event::MetaEventTranslator;
use super::place_event_translator::PlaceEventTranslator;
use super::player_state::get_local_player_id;
use super::selection_xlat::SelectionTranslator as SelectionTranslatorXlat;
use super::window_xlat::WindowTranslator;
use crate::core::game_client::CommandEvaluateType as ClientCommandEvaluateType;
use crate::display::view::{with_tactical_view, with_tactical_view_ref, IPoint2, Point3};
use crate::drawable::Drawable;
use crate::gui::{toggle_control_bar, toggle_diplomacy, toggle_quit_menu};
use crate::helpers::{PendingCommand, TheInGameUI};
use crate::input::KeyModifiers;
use crate::system::beacon_display;
use crate::system::GameMessageResult;
use game_engine::common::ini::ini_game_data::get_global_data;
use gamelogic::action_manager::ActionManager;
use gamelogic::attack::{AbleToAttackType, CanAttackResult};
use gamelogic::commands::command::CommandType;
use gamelogic::commands::get_selection_manager;
use gamelogic::common::Coord3D as LogicCoord3D;
use gamelogic::common::{
    CommandSourceType, KindOf, ObjectStatusMaskType as LogicObjectStatusMaskType, Relationship,
};
use gamelogic::damage::DamageType;
use gamelogic::helpers::{TheGameLogic, TheTerrainLogic};
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::object::special_power_template::{get_special_power_store, SpecialPowerTemplate};
use gamelogic::path::SURFACE_CLIFF;
use gamelogic::player::player_list;
use gamelogic::system::shroud_manager::{get_shroud_manager, ShroudState};
use gamelogic::weapon::WeaponSlotType;
use log::{debug, info, warn};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

fn logic_to_message_coord(pos: &LogicCoord3D) -> Coord3D {
    Coord3D::new(pos.x, pos.y, pos.z)
}

fn screen_to_terrain(pos: &ICoord2D) -> Option<Coord3D> {
    let screen = IPoint2::new(pos.x, pos.y);
    with_tactical_view_ref(|view| {
        view.screen_to_terrain(&screen)
            .ok()
            .map(|point| Coord3D::new(point.x, point.y, point.z))
    })
}

fn is_alternate_mouse_enabled() -> bool {
    get_global_data()
        .map(|data| data.read().use_alternate_mouse)
        .unwrap_or(false)
}

fn is_double_click_attack_move_enabled() -> bool {
    get_global_data()
        .map(|data| data.read().double_click_attack_move)
        .unwrap_or(false)
}

fn point_click_is_actionable(
    right_click: bool,
    alternate_mouse: bool,
    pending_command_active: bool,
) -> bool {
    if right_click {
        // C++ only processes right-click point commands in alternate mouse mode,
        // except when a pending GUI command is active and the click is used to cancel it.
        alternate_mouse || pending_command_active
    } else {
        // C++ only processes left-click point commands in alternate mouse mode when
        // a GUI command is actively firing.
        !alternate_mouse || pending_command_active
    }
}

const CMD_NEED_TARGET_ENEMY_OBJECT: u32 = 0x0000_0001;
const CMD_NEED_TARGET_NEUTRAL_OBJECT: u32 = 0x0000_0002;
const CMD_NEED_TARGET_ALLY_OBJECT: u32 = 0x0000_0004;
const CMD_NEED_TARGET_PRISONER: u32 = 0x0000_0008;
const CMD_ALLOW_SHRUBBERY_TARGET: u32 = 0x0000_0010;
const CMD_NEED_TARGET_POS: u32 = 0x0000_0020;
const CMD_CONTEXTMODE_COMMAND: u32 = 0x0000_0200;
const CMD_ALLOW_MINE_TARGET: u32 = 0x0000_0800;
const CMD_ATTACK_OBJECTS_POSITION: u32 = 0x0000_1000;
const SPECIAL_POWER_INVALID: u32 = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ContextPickProfile {
    include_selectable: bool,
    include_force_attackable: bool,
    include_mines: bool,
    include_shrubbery: bool,
}

impl Default for ContextPickProfile {
    fn default() -> Self {
        Self {
            include_selectable: true,
            include_force_attackable: false,
            include_mines: false,
            include_shrubbery: false,
        }
    }
}

fn selection_has_flame_weapon(selection: &HashSet<ObjectID>) -> bool {
    for &id in selection {
        let Some(obj) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(guard) = obj.read() else {
            continue;
        };
        if guard.is_destroyed() {
            continue;
        }
        if guard
            .weapon_set
            .has_weapon_to_deal_damage_type(DamageType::Flame)
        {
            return true;
        }
    }
    false
}

fn context_pick_profile(
    force_attack_mode: bool,
    selection: &HashSet<ObjectID>,
) -> ContextPickProfile {
    let mut profile = ContextPickProfile::default();
    if force_attack_mode {
        profile.include_force_attackable = true;
    }

    let pending_options = TheInGameUI::get_pending_command()
        .map(|pending| pending.options)
        .or_else(|| TheInGameUI::get_pending_special_power().map(|pending| pending.options));

    if let Some(options) = pending_options {
        if options & CMD_ALLOW_MINE_TARGET != 0 {
            profile.include_mines = true;
        }
        if options & CMD_ALLOW_SHRUBBERY_TARGET != 0 {
            profile.include_shrubbery = true;
        }
    } else if force_attack_mode && selection_has_flame_weapon(selection) {
        // Matches C++ getPickTypesForCurrentSelection(forceAttackMode): flame weapons can target shrubbery.
        profile.include_shrubbery = true;
    }

    profile
}

fn pending_command_accepts_object(options: u32) -> bool {
    options
        & (CMD_NEED_TARGET_ENEMY_OBJECT
            | CMD_NEED_TARGET_NEUTRAL_OBJECT
            | CMD_NEED_TARGET_ALLY_OBJECT
            | CMD_NEED_TARGET_PRISONER)
        != 0
}

fn pending_command_accepts_position(options: u32) -> bool {
    options & (CMD_NEED_TARGET_POS | CMD_ATTACK_OBJECTS_POSITION) != 0
}

fn relationship_to_target(local_player_id: i32, target_id: ObjectID) -> Option<Relationship> {
    if local_player_id < 0 {
        return None;
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

fn is_prisoner_target(target_id: ObjectID) -> bool {
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

fn pending_command_target_allowed(options: u32, local_player_id: i32, target_id: ObjectID) -> bool {
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

fn weapon_slot_from_u32(value: u32) -> WeaponSlotType {
    match value {
        1 => WeaponSlotType::Secondary,
        2 => WeaponSlotType::Tertiary,
        _ => WeaponSlotType::Primary,
    }
}

fn pending_weapon_slot(pending: &PendingCommand) -> WeaponSlotType {
    weapon_slot_from_u32(pending.source_object_id)
}

fn pending_special_power_payload(
) -> Option<(crate::helpers::PendingSpecialPower, SpecialPowerTemplate)> {
    let power = TheInGameUI::get_pending_special_power()?;
    let store = get_special_power_store()?;
    let template = store
        .find_special_power_template_by_id(power.power_id)?
        .clone();
    Some((power, template))
}

fn pending_fire_weapon_can_target_object(
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

fn pending_fire_weapon_can_target_position(
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

fn pending_special_power_can_target_object(
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

fn pending_special_power_can_target_position(
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

fn pending_command_for_object(
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

fn pending_command_hint_for_object(
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

fn pending_command_hint_for_position(
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

fn pending_command_for_position(
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

fn pending_command_messages_for_position(
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

fn selection_source_object_id(
    selection: &HashSet<ObjectID>,
    local_player_u32: Option<u32>,
) -> ObjectID {
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

fn selection_can_override_special_power_destination(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    special_power_type: u32,
) -> bool {
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
        if !is_mine || sel_guard.is_effectively_dead() {
            continue;
        }

        let mut matches_power = special_power_type == SPECIAL_POWER_INVALID;
        if !matches_power {
            for behavior_arc in sel_guard.get_behavior_modules() {
                let Ok(behavior_lock) = behavior_arc.lock() else {
                    continue;
                };
                let Some(sp_module) = behavior_lock.get_special_power_module_interface_const()
                else {
                    continue;
                };
                let Some(template) = sp_module.get_special_power_template_full() else {
                    continue;
                };
                if template.get_special_power_type() as u32 == special_power_type {
                    matches_power = true;
                }
                if matches_power {
                    break;
                }
            }
        }
        if !matches_power {
            continue;
        }

        let mut can_override = false;
        for behavior_arc in sel_guard.get_behavior_modules() {
            let Ok(mut behavior_lock) = behavior_arc.lock() else {
                continue;
            };
            let Some(update) = behavior_lock.get_special_power_update_interface() else {
                continue;
            };
            if update.does_special_power_have_overridable_destination_active()
                || update.does_special_power_have_overridable_destination()
            {
                can_override = true;
            }
            if can_override {
                break;
            }
        }

        if can_override {
            return true;
        }
    }

    false
}

fn selection_attack_result(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> CanAttackResult {
    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return CanAttackResult::NotPossible;
    };
    let Ok(target_guard) = target.read() else {
        return CanAttackResult::NotPossible;
    };

    let mut saw_invalid_shot = false;
    let mut saw_possible_after_moving = false;

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

        if !sel_guard.is_able_to_attack() {
            continue;
        }

        match sel_guard.get_able_to_attack_specific_object(
            AbleToAttackType::NewTarget,
            &target_guard,
            CommandSourceType::FromPlayer,
        ) {
            CanAttackResult::Possible => return CanAttackResult::Possible,
            CanAttackResult::PossibleAfterMoving => saw_possible_after_moving = true,
            CanAttackResult::InvalidShot => saw_invalid_shot = true,
            CanAttackResult::NotPossible => {}
        }
    }

    if saw_possible_after_moving {
        CanAttackResult::PossibleAfterMoving
    } else if saw_invalid_shot {
        CanAttackResult::InvalidShot
    } else {
        CanAttackResult::NotPossible
    }
}

fn selection_force_attack_object_result(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> CanAttackResult {
    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return CanAttackResult::NotPossible;
    };
    let Ok(target_guard) = target.read() else {
        return CanAttackResult::NotPossible;
    };

    let mut saw_invalid_shot = false;
    let mut saw_possible_after_moving = false;

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
        if !is_mine || !sel_guard.is_able_to_attack() {
            continue;
        }

        match force_attack_object_result_for_attacker(&sel_guard, &target_guard) {
            CanAttackResult::Possible => return CanAttackResult::Possible,
            CanAttackResult::PossibleAfterMoving => saw_possible_after_moving = true,
            CanAttackResult::InvalidShot => saw_invalid_shot = true,
            CanAttackResult::NotPossible => {}
        }
    }

    if saw_possible_after_moving {
        CanAttackResult::PossibleAfterMoving
    } else if saw_invalid_shot {
        CanAttackResult::InvalidShot
    } else {
        CanAttackResult::NotPossible
    }
}

fn closest_spawn_slave_id_for_position(
    owner: &gamelogic::object::Object,
    pos: &LogicCoord3D,
) -> Option<ObjectID> {
    for module in owner.behavior_modules() {
        let mut closest: Option<ObjectID> = None;
        module.with_module_downcast::<
            gamelogic::object::behavior::spawn_behavior::SpawnBehaviorModule,
            _,
            _,
        >(|spawn_module| {
            closest = gamelogic::object::behavior::spawn_behavior::SpawnBehaviorInterface::get_closest_slave(
                spawn_module.behavior_mut(),
                pos,
            )
            .and_then(|slave| slave.read().ok().map(|guard| guard.get_id()));
        });
        if closest.is_some() {
            return closest;
        }
    }

    None
}

fn closest_contained_rider_id_for_position(
    owner: &gamelogic::object::Object,
    pos: &LogicCoord3D,
) -> Option<ObjectID> {
    let contain = owner.get_contain()?;
    let contain_guard = contain.lock().ok()?;

    let mut closest = None;
    let mut closest_dist_sq = f32::INFINITY;

    for &rider_id in contain_guard.get_contained_objects() {
        let Some(rider) = OBJECT_REGISTRY.get_object(rider_id) else {
            continue;
        };
        let Ok(rider_guard) = rider.read() else {
            continue;
        };
        if rider_guard.is_effectively_dead() {
            continue;
        }

        let rider_pos = rider_guard.get_position();
        let dx = rider_pos.x - pos.x;
        let dy = rider_pos.y - pos.y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq < closest_dist_sq {
            closest_dist_sq = dist_sq;
            closest = Some(rider_id);
        }
    }

    closest
}

fn force_attack_object_result_for_attacker(
    attacker: &gamelogic::object::Object,
    target: &gamelogic::object::Object,
) -> CanAttackResult {
    let mut result = ActionManager::get_can_attack_object(
        attacker,
        target,
        CommandSourceType::FromPlayer,
        AbleToAttackType::NewTarget,
    );

    if !attacker.is_kind_of(KindOf::SpawnsAreTheWeapons) {
        return result;
    }

    let target_pos = target.get_position();

    if !matches!(
        result,
        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
    ) {
        if let Some(slave_id) = closest_spawn_slave_id_for_position(attacker, target_pos) {
            if let Some(slave) = OBJECT_REGISTRY.get_object(slave_id) {
                if let Ok(slave_guard) = slave.read() {
                    result = slave_guard.get_able_to_attack_specific_object(
                        AbleToAttackType::NewTarget,
                        target,
                        CommandSourceType::FromPlayer,
                    );
                }
            }
        }
    } else if let Some(rider_id) = closest_contained_rider_id_for_position(attacker, target_pos) {
        if let Some(rider) = OBJECT_REGISTRY.get_object(rider_id) {
            if let Ok(rider_guard) = rider.read() {
                let rider_result = rider_guard.get_able_to_attack_specific_object(
                    AbleToAttackType::NewTarget,
                    target,
                    CommandSourceType::FromPlayer,
                );
                if rider_result != CanAttackResult::NotPossible {
                    return rider_result;
                }
            }
        }
    }

    result
}

fn force_attack_position_result_for_attacker(
    attacker: &gamelogic::object::Object,
    pos: &LogicCoord3D,
) -> CanAttackResult {
    let mut test_attacker = attacker.get_id();

    if attacker.is_kind_of(KindOf::Immobile) || attacker.is_kind_of(KindOf::SpawnsAreTheWeapons) {
        if let Some(slave_id) = closest_spawn_slave_id_for_position(attacker, pos) {
            test_attacker = slave_id;
        } else {
            let result = attacker.get_able_to_use_weapon_against_position(
                AbleToAttackType::NewTarget,
                pos,
                CommandSourceType::FromPlayer,
            );
            if result != CanAttackResult::Possible {
                if let Some(rider_id) = closest_contained_rider_id_for_position(attacker, pos) {
                    test_attacker = rider_id;
                }
            }
        }
    }

    if test_attacker == attacker.get_id() {
        return attacker.get_able_to_use_weapon_against_position(
            AbleToAttackType::NewTarget,
            pos,
            CommandSourceType::FromPlayer,
        );
    }

    let Some(test_obj) = OBJECT_REGISTRY.get_object(test_attacker) else {
        return CanAttackResult::NotPossible;
    };
    let Ok(test_guard) = test_obj.read() else {
        return CanAttackResult::NotPossible;
    };

    test_guard.get_able_to_use_weapon_against_position(
        AbleToAttackType::NewTarget,
        pos,
        CommandSourceType::FromPlayer,
    )
}

fn selection_force_attack_position_result(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    world: &Coord3D,
) -> CanAttackResult {
    let logic_pos = LogicCoord3D::new(world.x, world.y, world.z);
    let mut saw_invalid_shot = false;
    let mut saw_possible_after_moving = false;

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
        if !is_mine || !sel_guard.is_able_to_attack() {
            continue;
        }

        match force_attack_position_result_for_attacker(&sel_guard, &logic_pos) {
            CanAttackResult::Possible => return CanAttackResult::Possible,
            CanAttackResult::PossibleAfterMoving => saw_possible_after_moving = true,
            CanAttackResult::InvalidShot => saw_invalid_shot = true,
            CanAttackResult::NotPossible => {}
        }
    }

    if saw_possible_after_moving {
        CanAttackResult::PossibleAfterMoving
    } else if saw_invalid_shot {
        CanAttackResult::InvalidShot
    } else {
        CanAttackResult::NotPossible
    }
}

fn pending_command_selection_valid(
    pending: &PendingCommand,
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    match pending.command_type {
        CommandType::DoAttackObject | CommandType::DoWeaponAtObject => {
            pending_fire_weapon_can_target_object(pending, local_player, selection, target_id)
        }
        CommandType::DoSpecialPower | CommandType::DoSpecialPowerAtObject => {
            pending_special_power_can_target_object(pending, local_player, selection, target_id)
        }
        CommandType::Enter => selection_can_enter_target(local_player, selection, target_id),
        CommandType::DoRepair => selection_can_repair_target(local_player, selection, target_id),
        CommandType::GetRepaired => {
            selection_can_get_repaired_target(local_player, selection, target_id)
        }
        CommandType::GetHealed => {
            selection_can_get_healed_target(local_player, selection, target_id)
        }
        CommandType::ResumeConstruction => {
            selection_can_resume_construction_target(local_player, selection, target_id)
        }
        CommandType::Dock => selection_can_dock_at_target(local_player, selection, target_id),
        _ => true,
    }
}

fn pending_command_position_valid(
    pending: &PendingCommand,
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    position: &Coord3D,
    object_in_way: Option<ObjectID>,
) -> bool {
    match pending.command_type {
        CommandType::DoAttackObject
        | CommandType::DoWeaponAtObject
        | CommandType::DoWeaponAtLocation => pending_fire_weapon_can_target_position(
            pending,
            local_player,
            selection,
            position,
            object_in_way,
        ),
        CommandType::DoSpecialPower | CommandType::DoSpecialPowerAtLocation => {
            pending_special_power_can_target_position(
                local_player,
                selection,
                position,
                object_in_way,
            )
        }
        _ => true,
    }
}

fn current_local_selection(local_player: i32) -> HashSet<ObjectID> {
    let mut selection_ids = HashSet::new();
    if local_player < 0 {
        return selection_ids;
    }

    let selection_manager = get_selection_manager();
    let Ok(manager) = selection_manager.read() else {
        return selection_ids;
    };
    let Some(selection) = manager.get_player_selection_ref(local_player) else {
        return selection_ids;
    };

    selection_ids.extend(selection.get_selected_objects());
    selection_ids
}

fn pick_context_target_for_click(
    region: &IRegion2D,
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    force_attack_mode: bool,
) -> Option<ObjectID> {
    const PICK_RADIUS_WORLD: f32 = 10.0;

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

fn is_locally_controlled_mine_target(object_id: ObjectID) -> bool {
    OBJECT_REGISTRY
        .get_object(object_id)
        .and_then(|obj| {
            obj.read()
                .ok()
                .map(|guard| guard.is_kind_of(KindOf::Mine) && guard.is_locally_controlled())
        })
        .unwrap_or(false)
}

fn is_pending_gui_non_context_command(pending: &PendingCommand) -> bool {
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

/// Command Translator - converts raw input into game commands
pub struct CommandTranslator {
    // State for tracking mouse operations
    mouse_down_position: Option<ICoord2D>,
    drag_threshold: i32,
    mouse_down_modifiers: u32,
    right_click_anchor: Option<ICoord2D>,
    right_click_lift: Option<ICoord2D>,
    right_click_down_time: u32,
    right_click_up_time: u32,

    // State for selection operations
    current_selection: HashSet<ObjectID>,
    selection_anchor: Option<ICoord2D>,

    // Mode flags
    force_attack_mode: bool,
    force_move_mode: bool,
    waypoint_mode: bool,
    path_build_mode: bool,
    prefer_selection_mode: bool,
}

impl CommandTranslator {
    pub fn new() -> Self {
        Self {
            mouse_down_position: None,
            drag_threshold: 5, // pixels
            mouse_down_modifiers: 0,
            right_click_anchor: None,
            right_click_lift: None,
            right_click_down_time: 0,
            right_click_up_time: 0,
            current_selection: HashSet::new(),
            selection_anchor: None,
            force_attack_mode: false,
            force_move_mode: false,
            waypoint_mode: false,
            path_build_mode: false,
            prefer_selection_mode: false,
        }
    }

    /// Evaluate a context-sensitive command against the current selection state.
    ///
    /// This keeps the command translator itself as the source of truth for context evaluation,
    /// matching the C++ `GameClient` hookup where the registered command translator is also the
    /// object consulted by context selection logic.
    pub fn evaluate_context_command(
        &mut self,
        drawable: &dyn Drawable,
        position: &Coord3D,
        cmd_type: ClientCommandEvaluateType,
    ) -> GameMessageResult<GameMessageType> {
        self.sync_selection_from_logic();

        // C++ parity: "null out draw/obj" forces position-based evaluation.
        let mut evaluate_as_position = false;

        if let Some(obj_id) = drawable.get_object_id() {
            if let Some(obj) = OBJECT_REGISTRY.get_object(obj_id) {
                if let Ok(guard) = obj.read() {
                    let is_masked = guard
                        .get_status_bits()
                        .contains(LogicObjectStatusMaskType::MASKED);
                    if is_masked
                        && !guard.is_kind_of(KindOf::Shrubbery)
                        && !guard.is_kind_of(KindOf::ForceAttackable)
                    {
                        evaluate_as_position = true;
                    }

                    if !evaluate_as_position
                        && guard.is_kind_of(KindOf::Mine)
                        && guard.is_locally_controlled()
                    {
                        evaluate_as_position = true;
                    }

                    if !evaluate_as_position
                        && guard.is_locally_controlled()
                        && TheInGameUI::is_in_prefer_selection_mode()
                    {
                        return Ok(GameMessageType::Invalid);
                    }
                }
            }
        } else {
            evaluate_as_position = true;
        }

        if self.force_move_mode || TheInGameUI::is_in_force_move_to_mode() {
            evaluate_as_position = true;
        }

        let result = match cmd_type {
            ClientCommandEvaluateType::Context
            | ClientCommandEvaluateType::Primary
            | ClientCommandEvaluateType::Secondary => {
                if evaluate_as_position || drawable.get_object_id().is_none() {
                    self.handle_mouseover_location_hint(position)
                        .into_iter()
                        .next()
                        .unwrap_or(GameMessageType::Invalid)
                } else {
                    self.handle_mouseover_drawable_hint(drawable.get_id().0)
                        .into_iter()
                        .next()
                        .unwrap_or(GameMessageType::Invalid)
                }
            }
        };

        Ok(result)
    }

    /// Process mouse button down events
    fn handle_mouse_button_down(
        &mut self,
        position: &ICoord2D,
        button: MouseButton,
        modifiers: u32,
        time: u32,
    ) -> Vec<GameMessageType> {
        debug!("Mouse button {:?} down at {:?}", button, position);

        match button {
            MouseButton::Left => {
                self.mouse_down_position = Some(position.clone());
                self.selection_anchor = Some(position.clone());
                self.mouse_down_modifiers = modifiers;
                vec![]
            }
            MouseButton::Right => {
                // Mirrors C++ right-button click bookkeeping used by click/drag gating.
                self.right_click_anchor = Some(position.clone());
                self.right_click_down_time = time;
                vec![]
            }
            MouseButton::Middle => {
                vec![]
            }
        }
    }

    fn sync_selection_from_logic(&mut self) {
        let local_player = get_local_player_id();
        if local_player < 0 {
            return;
        }

        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return;
        };

        let Some(selection) = manager.get_player_selection_ref(local_player) else {
            return;
        };

        self.current_selection.clear();
        self.current_selection
            .extend(selection.get_selected_objects());
    }

    fn clear_targeting_modes(&mut self) {
        TheInGameUI::clear_pending_command();
        TheInGameUI::clear_pending_special_power();
        TheInGameUI::set_force_attack_mode(false);
        TheInGameUI::set_force_move_to_mode(false);
        TheInGameUI::set_prefer_selection_mode(false);
        self.force_attack_mode = false;
        self.force_move_mode = false;
        self.prefer_selection_mode = false;
    }

    fn pick_context_target(
        &self,
        region: &IRegion2D,
        local_player: Option<u32>,
    ) -> Option<ObjectID> {
        const PICK_RADIUS_WORLD: f32 = 10.0;
        let force_attack_active = self.force_attack_mode || TheInGameUI::is_in_force_attack_mode();
        let profile = context_pick_profile(force_attack_active, &self.current_selection);
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

    fn resolve_pending_command_click(
        &mut self,
        local_player: i32,
        local_player_u32: Option<u32>,
        target: Option<ObjectID>,
        world: &Coord3D,
    ) -> Vec<GameMessageType> {
        let Some(pending) = TheInGameUI::get_pending_command() else {
            return Vec::new();
        };

        if let Some(object_id) = target {
            if pending_command_accepts_object(pending.options)
                && pending_command_target_allowed(pending.options, local_player, object_id)
                && pending_command_selection_valid(
                    &pending,
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                )
            {
                if let Some(message) = pending_command_for_object(&pending, object_id) {
                    self.clear_targeting_modes();
                    return vec![message];
                }
            }

            if pending_command_accepts_position(pending.options) {
                if let Some(obj) = OBJECT_REGISTRY.get_object(object_id) {
                    if let Ok(obj_guard) = obj.read() {
                        let position = logic_to_message_coord(obj_guard.get_position());
                        if pending_command_position_valid(
                            &pending,
                            local_player_u32,
                            &self.current_selection,
                            &position,
                            Some(object_id),
                        ) {
                            let messages = pending_command_messages_for_position(
                                &pending,
                                position,
                                &self.current_selection,
                                Some(object_id),
                            );
                            if !messages.is_empty() {
                                self.clear_targeting_modes();
                                return messages;
                            }
                        }
                    }
                }
            }
        } else if pending_command_accepts_position(pending.options) {
            if pending_command_position_valid(
                &pending,
                local_player_u32,
                &self.current_selection,
                world,
                None,
            ) {
                let messages = pending_command_messages_for_position(
                    &pending,
                    world.clone(),
                    &self.current_selection,
                    None,
                );
                if !messages.is_empty() {
                    self.clear_targeting_modes();
                    return messages;
                }
            }
        }

        if pending_command_accepts_object(pending.options)
            || pending_command_accepts_position(pending.options)
        {
            vec![GameMessageType::InvalidGUICommandHint]
        } else {
            Vec::new()
        }
    }

    fn resolve_move_command(&self, world: Coord3D) -> GameMessageType {
        if self.waypoint_mode {
            GameMessageType::AddWaypoint(world)
        } else if TheInGameUI::is_in_attack_move_to_mode() {
            GameMessageType::DoAttackMoveTo(world)
        } else if self.force_move_mode || TheInGameUI::is_in_force_move_to_mode() {
            GameMessageType::DoForceMoveTO(world)
        } else {
            GameMessageType::DoMoveTo(world)
        }
    }

    fn resolve_move_hint(&self, world: Coord3D) -> GameMessageType {
        if !selection_has_quick_path_to(&self.current_selection, &world) {
            return GameMessageType::DoInvalidHint;
        }

        if self.waypoint_mode {
            GameMessageType::AddWaypointHint(world)
        } else if TheInGameUI::is_in_attack_move_to_mode() {
            GameMessageType::DoAttackMoveToHint(world)
        } else {
            GameMessageType::DoMoveToHint(world)
        }
    }

    fn evaluate_force_attack_command(
        &self,
        local_player_u32: Option<u32>,
        target: Option<ObjectID>,
        world: Coord3D,
    ) -> Option<GameMessageType> {
        if let Some(target_id) = target {
            return match selection_force_attack_object_result(
                local_player_u32,
                &self.current_selection,
                target_id,
            ) {
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                    Some(GameMessageType::DoForceAttackObject(target_id))
                }
                // C++ DO_COMMAND force-attack path does not emit invalid hint messages.
                CanAttackResult::InvalidShot | CanAttackResult::NotPossible => None,
            };
        }

        match selection_force_attack_position_result(
            local_player_u32,
            &self.current_selection,
            &world,
        ) {
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                Some(GameMessageType::DoForceAttackGround(world))
            }
            CanAttackResult::InvalidShot | CanAttackResult::NotPossible => None,
        }
    }

    fn evaluate_force_attack_hint(
        &self,
        local_player_u32: Option<u32>,
        target: Option<ObjectID>,
        world: Coord3D,
    ) -> Option<GameMessageType> {
        if let Some(target_id) = target {
            return match selection_force_attack_object_result(
                local_player_u32,
                &self.current_selection,
                target_id,
            ) {
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                    Some(GameMessageType::DoForceAttackObjectHint(target_id))
                }
                CanAttackResult::InvalidShot => Some(GameMessageType::ImpossibleAttackHint),
                CanAttackResult::NotPossible => None,
            };
        }

        match selection_force_attack_position_result(
            local_player_u32,
            &self.current_selection,
            &world,
        ) {
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                Some(GameMessageType::DoForceAttackGroundHint(world))
            }
            CanAttackResult::InvalidShot => Some(GameMessageType::ImpossibleAttackHint),
            CanAttackResult::NotPossible => None,
        }
    }

    fn try_double_click_guard_command(
        &self,
        region: &IRegion2D,
        right_click: bool,
    ) -> Option<GameMessageType> {
        if region.width != 0 || region.height != 0 {
            return None;
        }

        if !is_double_click_attack_move_enabled() {
            return None;
        }

        let alternate_mouse = is_alternate_mouse_enabled();
        let should_issue_guard = if right_click {
            alternate_mouse
        } else {
            !alternate_mouse
        };
        if !should_issue_guard {
            return None;
        }

        let click_pos = ICoord2D::new(region.x, region.y);
        let world = screen_to_terrain(&click_pos).unwrap_or(Coord3D {
            x: click_pos.x as f32,
            y: click_pos.y as f32,
            z: 0.0,
        });

        Some(GameMessageType::DoGuardPosition(world, 0))
    }

    fn evaluate_context_action(
        &self,
        _local_player: i32,
        local_player_u32: Option<u32>,
        target_id: ObjectID,
        world: Coord3D,
    ) -> Option<GameMessageType> {
        if selection_can_override_special_power_destination(
            local_player_u32,
            &self.current_selection,
            SPECIAL_POWER_INVALID,
        ) {
            return Some(GameMessageType::DoSpecialPowerOverrideDestination(
                world,
                SPECIAL_POWER_INVALID,
                gamelogic::common::INVALID_ID,
            ));
        }

        if selection_can_resume_construction_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::ResumeConstruction(target_id));
        }

        if selection_can_dock_at_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::Dock(target_id));
        }

        if selection_can_repair_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::DoRepair(target_id));
        }

        if selection_can_get_repaired_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::GetRepaired(target_id));
        }

        if selection_can_get_healed_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::GetHealed(target_id));
        }

        if selection_can_hijack_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::Enter(0, target_id));
        }

        if selection_can_convert_to_carbomb_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::ConvertToCarbomb(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if selection_can_sabotage_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::Enter(0, target_id));
        }

        if let Some(dest) =
            selection_can_pickup_crate_target(local_player_u32, &self.current_selection, target_id)
        {
            return Some(GameMessageType::DoMoveTo(dest));
        }

        if let Some(dest) =
            selection_can_salvage_target(local_player_u32, &self.current_selection, target_id)
        {
            return Some(GameMessageType::DoSalvage(dest));
        }

        if selection_can_enter_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::Enter(0, target_id));
        }

        let attack_result =
            selection_attack_result(local_player_u32, &self.current_selection, target_id);
        match attack_result {
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving => {
                return Some(GameMessageType::DoAttackObject(target_id));
            }
            // C++ evaluateContextCommand emits MSG_IMPOSSIBLE_ATTACK_HINT for invalid shots.
            CanAttackResult::InvalidShot => return Some(GameMessageType::ImpossibleAttackHint),
            CanAttackResult::NotPossible => {}
        }

        if selection_can_capture_building_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::CaptureBuilding(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if selection_can_disable_vehicle_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::DisableVehicleHack(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if selection_can_steal_cash_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::StealCashHack(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if selection_can_disable_building_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::DisableBuildingHack(
                selection_source_object_id(&self.current_selection, local_player_u32),
                target_id,
            ));
        }

        if let Some(dest) =
            selection_can_pickup_crate_target(local_player_u32, &self.current_selection, target_id)
        {
            return Some(GameMessageType::DoMoveTo(dest));
        }

        None
    }

    fn evaluate_context_hint(
        &self,
        _local_player: i32,
        local_player_u32: Option<u32>,
        target_id: ObjectID,
        world: Coord3D,
    ) -> Option<GameMessageType> {
        if selection_can_override_special_power_destination(
            local_player_u32,
            &self.current_selection,
            SPECIAL_POWER_INVALID,
        ) {
            return Some(GameMessageType::DoSpecialPowerOverrideDestinationHint(
                world,
            ));
        }

        let attack_result =
            selection_attack_result(local_player_u32, &self.current_selection, target_id);

        if selection_can_resume_construction_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::ResumeConstructionHint(target_id));
        }

        if selection_can_dock_at_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::DockHint(target_id));
        }

        if selection_can_repair_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::DoRepairHint(target_id));
        }

        if selection_can_get_repaired_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::GetRepairedHint(target_id));
        }

        if selection_can_get_healed_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::GetHealedHint(target_id));
        }

        if selection_can_hijack_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::HijackHint(target_id));
        }

        if selection_can_convert_to_carbomb_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::ConvertToCarbombHint(target_id));
        }

        if selection_can_sabotage_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::SabotageHint(target_id));
        }

        if selection_can_pickup_crate_target(local_player_u32, &self.current_selection, target_id)
            .is_some()
        {
            return Some(self.resolve_move_hint(world));
        }

        if let Some(dest) =
            selection_can_salvage_target(local_player_u32, &self.current_selection, target_id)
        {
            return Some(GameMessageType::DoSalvageHint(dest));
        }

        if selection_can_enter_target(local_player_u32, &self.current_selection, target_id) {
            return Some(GameMessageType::EnterHint(target_id));
        }

        match attack_result {
            CanAttackResult::Possible => {
                return Some(GameMessageType::DoAttackObjectHint(target_id))
            }
            CanAttackResult::PossibleAfterMoving => {
                return Some(GameMessageType::DoAttackObjectAfterMovingHint(target_id));
            }
            CanAttackResult::InvalidShot | CanAttackResult::NotPossible => {}
        }

        if selection_can_capture_building_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::CaptureBuildingHint(target_id));
        }

        if selection_can_disable_vehicle_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) || selection_can_steal_cash_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) || selection_can_disable_building_hack_target(
            local_player_u32,
            &self.current_selection,
            target_id,
        ) {
            return Some(GameMessageType::HackHint(target_id));
        }

        if attack_result == CanAttackResult::InvalidShot {
            return Some(GameMessageType::ImpossibleAttackHint);
        }

        None
    }

    fn handle_point_click(
        &mut self,
        region: &IRegion2D,
        right_click: bool,
    ) -> Vec<GameMessageType> {
        if region.width != 0 || region.height != 0 {
            return Vec::new();
        }

        if right_click && is_alternate_mouse_enabled() && !self.right_click_is_click_gesture() {
            return Vec::new();
        }

        let click_pos = ICoord2D::new(region.x, region.y);
        let world = screen_to_terrain(&click_pos).unwrap_or(Coord3D {
            x: click_pos.x as f32,
            y: click_pos.y as f32,
            z: 0.0,
        });

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };
        let alternate_mouse = is_alternate_mouse_enabled();
        let pending_command_active = TheInGameUI::get_pending_command().is_some();
        let target = self
            .pick_context_target(region, local_player_u32)
            .filter(|object_id| !is_locally_controlled_mine_target(*object_id));

        // In C++, right-click in alternate mouse mode still evaluates context/pending
        // commands; non-alternate right-click cancels pending targeting.
        if right_click && pending_command_active && !alternate_mouse {
            self.clear_targeting_modes();
            TheInGameUI::clear_attack_move_to_mode();
            return Vec::new();
        }

        if !point_click_is_actionable(right_click, alternate_mouse, pending_command_active) {
            return Vec::new();
        }

        // C++ pending GUI command execution happens on left-click paths; right-click
        // is used to cancel GUI mode by SelectionXlat.
        let should_resolve_pending = pending_command_active && !right_click;
        if should_resolve_pending {
            let messages =
                self.resolve_pending_command_click(local_player, local_player_u32, target, &world);
            if !messages.is_empty() {
                TheInGameUI::clear_attack_move_to_mode();
                return messages;
            }
            // Targeting mode stays active until fulfilled/cancelled.
            return Vec::new();
        }

        if self.current_selection.is_empty() {
            return Vec::new();
        }

        let force_attack_active = self.force_attack_mode || TheInGameUI::is_in_force_attack_mode();
        let command = if force_attack_active {
            self.evaluate_force_attack_command(local_player_u32, target, world.clone())
        } else if let Some(target_id) = target {
            self.evaluate_context_action(local_player, local_player_u32, target_id, world.clone())
        } else if selection_can_override_special_power_destination(
            local_player_u32,
            &self.current_selection,
            SPECIAL_POWER_INVALID,
        ) {
            Some(GameMessageType::DoSpecialPowerOverrideDestination(
                world.clone(),
                SPECIAL_POWER_INVALID,
                gamelogic::common::INVALID_ID,
            ))
        } else {
            Some(self.resolve_move_command(world.clone()))
        };

        TheInGameUI::clear_attack_move_to_mode();

        if command.is_none() {
            if let Some(target_id) = target {
                if !force_attack_active
                    && selection_attack_result(local_player_u32, &self.current_selection, target_id)
                        == CanAttackResult::InvalidShot
                {
                    return vec![GameMessageType::ImpossibleAttackHint];
                }
                return Vec::new();
            }
        }

        command.map(|msg| vec![msg]).unwrap_or_default()
    }

    fn handle_mouseover_location_hint(&self, pos: &Coord3D) -> Vec<GameMessageType> {
        if self.current_selection.is_empty() {
            return Vec::new();
        }

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };

        if let Some(pending) = TheInGameUI::get_pending_command() {
            if pending_command_accepts_position(pending.options) {
                if !pending_command_position_valid(
                    &pending,
                    local_player_u32,
                    &self.current_selection,
                    pos,
                    None,
                ) {
                    return vec![GameMessageType::InvalidGUICommandHint];
                }
                if let Some(hint) = pending_command_hint_for_position(&pending, pos.clone()) {
                    return vec![hint];
                }
                return vec![GameMessageType::ValidGUICommandHint];
            }
            if pending_command_accepts_object(pending.options) {
                return vec![GameMessageType::InvalidGUICommandHint];
            }
        }

        let force_attack_active = self.force_attack_mode || TheInGameUI::is_in_force_attack_mode();
        if force_attack_active {
            return self
                .evaluate_force_attack_hint(local_player_u32, None, pos.clone())
                .map(|hint| vec![hint])
                .unwrap_or_default();
        }

        if selection_can_override_special_power_destination(
            local_player_u32,
            &self.current_selection,
            SPECIAL_POWER_INVALID,
        ) {
            return vec![GameMessageType::DoSpecialPowerOverrideDestinationHint(
                pos.clone(),
            )];
        }

        vec![self.resolve_move_hint(pos.clone())]
    }

    fn handle_mouseover_drawable_hint(&self, drawable: DrawableID) -> Vec<GameMessageType> {
        if self.current_selection.is_empty() {
            return Vec::new();
        }

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };
        let target_id = drawable as ObjectID;
        let world = OBJECT_REGISTRY
            .get_object(target_id)
            .and_then(|obj| {
                obj.read()
                    .ok()
                    .map(|guard| logic_to_message_coord(guard.get_position()))
            })
            .unwrap_or_default();

        // C++ evaluateContextCommand treats locally controlled mines as position
        // interactions instead of object-target interactions.
        if is_locally_controlled_mine_target(target_id) {
            return self.handle_mouseover_location_hint(&world);
        }

        if let Some(pending) = TheInGameUI::get_pending_command() {
            if pending_command_accepts_object(pending.options) {
                if pending_command_target_allowed(pending.options, local_player, target_id)
                    && pending_command_selection_valid(
                        &pending,
                        local_player_u32,
                        &self.current_selection,
                        target_id,
                    )
                {
                    // C++ GUI context-command hover uses generic valid/invalid GUI
                    // command hints rather than per-command hint message variants.
                    return vec![GameMessageType::ValidGUICommandHint];
                }
                return vec![GameMessageType::InvalidGUICommandHint];
            }

            if pending_command_accepts_position(pending.options) {
                if !pending_command_position_valid(
                    &pending,
                    local_player_u32,
                    &self.current_selection,
                    &world,
                    Some(target_id),
                ) {
                    return vec![GameMessageType::InvalidGUICommandHint];
                }
                if let Some(hint) = pending_command_hint_for_position(&pending, world.clone()) {
                    return vec![hint];
                }
                return vec![GameMessageType::ValidGUICommandHint];
            }
        }

        let force_attack_active = self.force_attack_mode || TheInGameUI::is_in_force_attack_mode();
        if force_attack_active {
            return self
                .evaluate_force_attack_hint(local_player_u32, Some(target_id), world)
                .map(|hint| vec![hint])
                .unwrap_or_default();
        }

        if let Some(hint) =
            self.evaluate_context_hint(local_player, local_player_u32, target_id, world.clone())
        {
            return vec![hint];
        }

        vec![self.resolve_move_hint(world)]
    }

    /// Process mouse button up events
    fn handle_mouse_button_up(
        &mut self,
        position: &ICoord2D,
        button: MouseButton,
        modifiers: u32,
        time: u32,
    ) -> Vec<GameMessageType> {
        debug!("Mouse button {:?} up at {:?}", button, position);

        match button {
            MouseButton::Left => {
                let mut messages = Vec::new();

                if let Some(down_pos) = &self.mouse_down_position {
                    let dx = (position.x - down_pos.x) as f32;
                    let dy = (position.y - down_pos.y) as f32;
                    let distance = (dx * dx + dy * dy).sqrt();

                    let key_mods = KeyModifiers::from_bits_truncate(modifiers as u8);
                    if distance < self.drag_threshold as f32 {
                        let region = IRegion2D {
                            x: position.x,
                            y: position.y,
                            width: 0,
                            height: 0,
                        };
                        messages.extend(self.handle_selection_region(&region, key_mods));
                    } else if let Some(anchor) = &self.selection_anchor {
                        let region = build_region(anchor, position);
                        messages.extend(self.handle_selection_region(&region, key_mods));
                    }
                }

                self.mouse_down_position = None;
                self.selection_anchor = None;
                self.mouse_down_modifiers = 0;
                messages
            }
            MouseButton::Right => {
                // C++ raw right-button-up only updates click/drag bookkeeping and does not
                // directly issue command messages; context commands are generated on click events.
                self.right_click_lift = Some(position.clone());
                self.right_click_up_time = time;
                let had_pending_place_source =
                    TheInGameUI::get_pending_place_source_object_id() != 0;
                // C++ parity (CommandXlat.cpp MSG_RAW_MOUSE_RIGHT_BUTTON_UP):
                // right-click click gesture cancels pending build-placement mode.
                if self.right_click_is_click_gesture() {
                    TheInGameUI::place_build_available(None, None);
                    if TheInGameUI::get_pending_command().is_none()
                        && (!is_alternate_mouse_enabled() || had_pending_place_source)
                        && !self.current_selection.is_empty()
                    {
                        self.current_selection.clear();
                        return vec![GameMessageType::CreateSelectedGroup(true, Vec::new())];
                    }
                }
                vec![]
            }
            MouseButton::Middle => {
                vec![]
            }
        }
    }

    fn right_click_is_click_gesture(&self) -> bool {
        let (Some(anchor), Some(lift)) = (&self.right_click_anchor, &self.right_click_lift) else {
            return false;
        };
        let dx = (anchor.x - lift.x).abs();
        let dy = (anchor.y - lift.y).abs();
        let dt = self
            .right_click_up_time
            .wrapping_sub(self.right_click_down_time);

        // C++ Mouse::isClick parity: movement within drag tolerance and short click duration.
        dx <= self.drag_threshold && dy <= self.drag_threshold && dt <= 250
    }

    fn handle_selection_region(
        &mut self,
        region: &IRegion2D,
        modifiers: KeyModifiers,
    ) -> Vec<GameMessageType> {
        const MAX_SELECTION_COUNT: usize = 40;
        const PICK_RADIUS_WORLD: f32 = 10.0;

        let is_point = region.width == 0 && region.height == 0;
        let allow_add = modifiers.contains(KeyModifiers::SHIFT) || self.prefer_selection_mode;
        let allow_toggle = modifiers.contains(KeyModifiers::CTRL);

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };

        let (mut mine, mut other) = collect_selectable_objects(
            region,
            is_point,
            PICK_RADIUS_WORLD,
            local_player_u32,
            ContextPickProfile::default(),
        );

        if is_point {
            let picked_object = pick_closest(&mut mine).or_else(|| pick_closest(&mut other));

            if TheInGameUI::get_pending_command().is_some() {
                let world =
                    screen_to_terrain(&ICoord2D::new(region.x, region.y)).unwrap_or(Coord3D {
                        x: region.x as f32,
                        y: region.y as f32,
                        z: 0.0,
                    });
                let messages = self.resolve_pending_command_click(
                    local_player,
                    local_player_u32,
                    picked_object,
                    &world,
                );
                if !messages.is_empty() {
                    return messages;
                }
                // Targeting mode active: ignore selection changes until command is fulfilled/cancelled.
                return Vec::new();
            }

            let Some(object_id) = picked_object else {
                // C++ SelectionXlat leaves blank point clicks in the stream so CommandXlat can
                // issue the terrain/context command for the current selection.
                return Vec::new();
            };

            let (
                current_count_mine,
                current_count_mine_infantry,
                current_count_mine_buildings,
                current_count_other,
            ) = selection_counts(local_player_u32, &self.current_selection);

            // C++ SelectionInfo.cpp: context sensitive selection never applies in force-attack or
            // force-move modes.
            let allow_context = !self.force_attack_mode
                && !self.force_move_mode
                && current_count_other == 0
                && current_count_mine > 0;

            if allow_context {
                // Enemy click becomes an action (typically attack) rather than selecting the enemy.
                if is_enemy_target(local_player, object_id)
                    && selection_can_attack_target(
                        local_player_u32,
                        &self.current_selection,
                        object_id,
                    )
                {
                    return vec![GameMessageType::DoAttackObject(object_id)];
                }

                // Clicking a garrison/transport-capable container with infantry selected issues Enter.
                if current_count_mine_infantry > 0
                    && selection_can_enter_target(
                        local_player_u32,
                        &self.current_selection,
                        object_id,
                    )
                {
                    return vec![GameMessageType::Enter(0, object_id)];
                }

                // Clicking a damaged friendly object with a dozer selected issues DoRepair.
                if selection_can_repair_target(local_player_u32, &self.current_selection, object_id)
                {
                    return vec![GameMessageType::DoRepair(object_id)];
                }

                if selection_can_resume_construction_target(
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                ) {
                    return vec![GameMessageType::ResumeConstruction(object_id)];
                }

                if selection_can_dock_at_target(
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                ) {
                    return vec![GameMessageType::Dock(object_id)];
                }

                if let Some(dest) = selection_can_pickup_crate_target(
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                ) {
                    return vec![GameMessageType::DoMoveTo(dest)];
                }

                // Salvage (hulks): C++ issues MSG_DO_SALVAGE with the target's position.
                if let Some(dest) = selection_can_salvage_target(
                    local_player_u32,
                    &self.current_selection,
                    object_id,
                ) {
                    return vec![GameMessageType::DoSalvage(dest)];
                }
            }

            // SelectionXlat.cpp: prefer-selection mode appends/removes, but selecting enemies,
            // friends, civilians, or buildings forces a replace selection.
            let mut add_to_group = allow_add;
            if current_count_mine_buildings > 0 || current_count_other > 0 {
                add_to_group = false;
            }

            if allow_toggle {
                if self.current_selection.remove(&object_id) {
                    return vec![GameMessageType::RemoveFromSelectedGroup(vec![object_id])];
                }
                if self.current_selection.len() >= MAX_SELECTION_COUNT {
                    return Vec::new();
                }
                self.current_selection.insert(object_id);
                return vec![GameMessageType::CreateSelectedGroup(false, vec![object_id])];
            }

            if add_to_group {
                if self.current_selection.contains(&object_id) {
                    self.current_selection.remove(&object_id);
                    return vec![GameMessageType::RemoveFromSelectedGroup(vec![object_id])];
                }
                if self.current_selection.len() >= MAX_SELECTION_COUNT {
                    return Vec::new();
                }
                self.current_selection.insert(object_id);
                return vec![GameMessageType::CreateSelectedGroup(false, vec![object_id])];
            }

            self.current_selection.clear();
            self.current_selection.insert(object_id);
            return vec![GameMessageType::CreateSelectedGroup(true, vec![object_id])];
        }

        // Region selection: C++ selection prefers locally controlled units; buildings can be
        // selected when no units are selectable in the region.
        mine.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut selected_ids = Vec::new();
        let mut building_ids = Vec::new();
        for (id, _) in mine.into_iter() {
            if selected_ids.len() >= MAX_SELECTION_COUNT {
                break;
            }

            let Some(obj) = OBJECT_REGISTRY.get_object(id) else {
                continue;
            };
            let Ok(guard) = obj.read() else {
                continue;
            };

            if guard.is_kind_of(KindOf::Structure) || guard.is_kind_of(KindOf::Building) {
                building_ids.push(id);
                continue;
            }

            selected_ids.push(id);
        }

        if selected_ids.is_empty() && building_ids.len() == 1 {
            selected_ids.push(building_ids[0]);
        }

        if selected_ids.is_empty() {
            return Vec::new();
        }

        if allow_add {
            let mut new_ids = Vec::new();
            for id in selected_ids {
                if self.current_selection.len() >= MAX_SELECTION_COUNT {
                    break;
                }
                if self.current_selection.insert(id) {
                    new_ids.push(id);
                }
            }
            if new_ids.is_empty() {
                Vec::new()
            } else {
                vec![GameMessageType::CreateSelectedGroup(false, new_ids)]
            }
        } else {
            self.current_selection.clear();
            self.current_selection.extend(selected_ids.iter().copied());
            vec![GameMessageType::CreateSelectedGroup(true, selected_ids)]
        }
    }

    /// Process keyboard events
    fn handle_keyboard(&mut self, key: u32, down: bool) -> Vec<GameMessageType> {
        debug!("Key {} {}", key, if down { "down" } else { "up" });

        let mut messages = Vec::new();

        match key {
            // Meta commands mapped to keys
            0x53 => {
                // 'S' key - stop
                if down {
                    messages.push(GameMessageType::MetaStop);
                }
            }
            0x41 => {
                // 'A' key - attack move
                if down {
                    messages.push(GameMessageType::MetaToggleAttackMove);
                }
            }
            0x47 => {
                // 'G' key - guard
                if down && !self.current_selection.is_empty() {
                    // Guard current position
                    let first = *self.current_selection.iter().next().unwrap();
                    let pos = OBJECT_REGISTRY
                        .get_object(first)
                        .and_then(|obj| {
                            obj.read()
                                .ok()
                                .map(|guard| logic_to_message_coord(guard.get_position()))
                        })
                        .unwrap_or_default();
                    messages.push(GameMessageType::DoGuardPosition(pos, 0));
                }
            }
            0x48 => {
                // 'H' key - halt/stop
                if down {
                    messages.push(GameMessageType::MetaStop);
                }
            }
            0x20 => {
                // Spacebar - scatter
                if down {
                    messages.push(GameMessageType::MetaScatter);
                }
            }
            // Control key modifiers
            0x11 => {
                // Ctrl key
                if down {
                    self.prefer_selection_mode = true;
                    TheInGameUI::set_prefer_selection_mode(true);
                    messages.push(GameMessageType::MetaBeginPreferSelection);
                } else {
                    self.prefer_selection_mode = false;
                    TheInGameUI::set_prefer_selection_mode(false);
                    messages.push(GameMessageType::MetaEndPreferSelection);
                }
            }
            // Alt key for force attack
            0x12 => {
                // Alt key
                if down {
                    self.force_attack_mode = true;
                    TheInGameUI::set_force_attack_mode(true);
                    messages.push(GameMessageType::MetaBeginForceAttack);
                } else {
                    self.force_attack_mode = false;
                    TheInGameUI::set_force_attack_mode(false);
                    messages.push(GameMessageType::MetaEndForceAttack);
                }
            }
            0x10 => {
                // Shift key
                if down {
                    self.waypoint_mode = true;
                    messages.push(GameMessageType::MetaBeginWaypoints);
                } else {
                    self.waypoint_mode = false;
                    messages.push(GameMessageType::MetaEndWaypoints);
                }
            }
            _ => {}
        }

        messages
    }

    /// Update current selection
    fn update_selection(&mut self, objects: HashSet<ObjectID>) {
        debug!("Updating selection with {} objects", objects.len());
        self.current_selection = objects;
    }
}

impl Default for CommandTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl GameMessageTranslator for CommandTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        let (new_messages, disposition) = match msg.get_type() {
            GameMessageType::FrameTick(_) => {
                // Keep the client-side selection cache in sync with the GameLogic selection manager
                // so commands work after selection changes originating outside this translator
                // (control groups, scripts, multiplayer, etc.).
                self.sync_selection_from_logic();
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::CreateSelectedGroup(create_new, objects) => {
                if *create_new {
                    self.current_selection.clear();
                }
                self.current_selection.extend(objects.iter().copied());
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::CreateSelectedGroupNoSound(create_new, objects) => {
                if *create_new {
                    self.current_selection.clear();
                }
                self.current_selection.extend(objects.iter().copied());
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::RemoveFromSelectedGroup(objects) => {
                for object_id in objects {
                    self.current_selection.remove(object_id);
                }
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::MetaBeginForceAttack => {
                self.force_attack_mode = true;
                TheInGameUI::set_force_attack_mode(true);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndForceAttack => {
                self.force_attack_mode = false;
                TheInGameUI::set_force_attack_mode(false);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginForceMove => {
                self.force_move_mode = true;
                TheInGameUI::set_force_move_to_mode(true);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndForceMove => {
                self.force_move_mode = false;
                TheInGameUI::set_force_move_to_mode(false);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginWaypoints => {
                self.waypoint_mode = true;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndWaypoints => {
                self.waypoint_mode = false;
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaBeginPreferSelection => {
                self.prefer_selection_mode = true;
                TheInGameUI::set_prefer_selection_mode(true);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaEndPreferSelection => {
                self.prefer_selection_mode = false;
                TheInGameUI::set_prefer_selection_mode(false);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaStop => {
                TheInGameUI::issue_stop_command();
                dispatch_translated_message(&GameMessageType::DoStop);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaScatter => {
                dispatch_translated_message(&GameMessageType::DoScatter);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaCreateFormation => {
                dispatch_translated_message(&GameMessageType::CreateFormation(Vec::new()));
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaAllCheer => {
                if TheGameLogic::is_in_multiplayer_game() {
                    dispatch_translated_message(&GameMessageType::DoCheer);
                }
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaDeploy
            | GameMessageType::MetaFollow
            | GameMessageType::MetaChatPlayers
            | GameMessageType::MetaChatAllies
            | GameMessageType::MetaChatEveryone => {
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MouseRightClick(region, _modifiers) => (
                self.handle_point_click(region, true),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::MouseLeftClick(region, _modifiers) => (
                self.handle_point_click(region, false),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::MouseRightDoubleClick(region, _modifiers) => (
                self.try_double_click_guard_command(region, true)
                    .map(|msg| vec![msg])
                    .unwrap_or_else(|| self.handle_point_click(region, true)),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::MouseLeftDoubleClick(region, _modifiers) => (
                self.try_double_click_guard_command(region, false)
                    .map(|msg| vec![msg])
                    .unwrap_or_else(|| self.handle_point_click(region, false)),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::MouseoverLocationHint(pos) => (
                self.handle_mouseover_location_hint(pos),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::MouseoverDrawableHint(drawable) => (
                self.handle_mouseover_drawable_hint(*drawable),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseLeftButtonDown(pos, _modifiers, time) => (
                self.handle_mouse_button_down(pos, MouseButton::Left, *_modifiers, *time),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseRightButtonDown(pos, _modifiers, time) => (
                self.handle_mouse_button_down(pos, MouseButton::Right, *_modifiers, *time),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseMiddleButtonDown(pos, _modifiers, time) => (
                self.handle_mouse_button_down(pos, MouseButton::Middle, *_modifiers, *time),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::RawMouseLeftButtonUp(pos, _modifiers, time) => (
                self.handle_mouse_button_up(pos, MouseButton::Left, *_modifiers, *time),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseRightButtonUp(pos, _modifiers, time) => (
                self.handle_mouse_button_up(pos, MouseButton::Right, *_modifiers, *time),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawMouseMiddleButtonUp(pos, _modifiers, time) => (
                self.handle_mouse_button_up(pos, MouseButton::Middle, *_modifiers, *time),
                GameMessageDisposition::DestroyMessage,
            ),
            GameMessageType::RawKeyDown(key) => (
                self.handle_keyboard(*key, true),
                GameMessageDisposition::KeepMessage,
            ),
            GameMessageType::RawKeyUp(key) => (
                self.handle_keyboard(*key, false),
                GameMessageDisposition::KeepMessage,
            ),
            _ => {
                // Pass through other messages unchanged
                return GameMessageDisposition::KeepMessage;
            }
        };

        // Translated high-level messages are forwarded into the command list, matching the C++
        // message stream flow where raw input messages are consumed and replaced with commands.
        for new_msg in new_messages {
            dispatch_translated_message(&new_msg);
        }

        disposition
    }
}

/// Mouse button enumeration
#[derive(Debug, Clone, PartialEq)]
enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Selection Translator - handles unit selection and group management
pub struct SelectionTranslator {
    selected_objects: HashSet<ObjectID>,
    control_groups: HashMap<u8, Vec<ObjectID>>, // 0-9 control groups
    last_selected_group: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionCycleFilter {
    Unit,
    Worker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectionCycleDirection {
    Forward,
    Backward,
}

impl SelectionTranslator {
    pub fn new() -> Self {
        Self {
            selected_objects: HashSet::new(),
            control_groups: HashMap::new(),
            last_selected_group: None,
        }
    }

    fn handle_area_selection(&mut self, region: &IRegion2D) -> Vec<GameMessageType> {
        debug!("Handling area selection: {:?}", region);
        let upper_left = ICoord2D::new(region.x, region.y);
        let lower_right = ICoord2D::new(region.x + region.width, region.y + region.height);
        TheInGameUI::select_area(upper_left, lower_right);
        Vec::new()
    }

    fn handle_control_group_create(&mut self, group: u8) -> Vec<GameMessageType> {
        debug!("Creating control group {}", group);

        if !self.selected_objects.is_empty() {
            let objects: Vec<_> = self.selected_objects.iter().cloned().collect();
            self.control_groups.insert(group, objects.clone());
            vec![GameMessageType::CreateTeamSlot(group)]
        } else {
            vec![]
        }
    }

    fn handle_control_group_select(&mut self, group: u8) -> Vec<GameMessageType> {
        debug!("Selecting control group {}", group);

        if let Some(objects) = self.control_groups.get(&group).cloned() {
            self.selected_objects.clear();
            self.selected_objects.extend(objects.iter());
            self.last_selected_group = Some(group);
            vec![GameMessageType::SelectTeamSlot(group)]
        } else {
            vec![]
        }
    }

    fn handle_control_group_add(&mut self, group: u8) -> Vec<GameMessageType> {
        debug!("Adding control group {}", group);

        if let Some(objects) = self.control_groups.get(&group).cloned() {
            self.selected_objects.extend(objects.iter());
            self.last_selected_group = Some(group);
            vec![GameMessageType::AddTeamSlot(group)]
        } else {
            vec![]
        }
    }

    fn handle_control_group_view(&self, group: u8) {
        let Some(object_id) = self
            .control_groups
            .get(&group)
            .and_then(|objects| objects.last())
            .copied()
        else {
            return;
        };

        let Some(position) = OBJECT_REGISTRY
            .get_object(object_id)
            .and_then(|object| object.read().ok().map(|guard| *guard.get_position()))
        else {
            return;
        };

        with_tactical_view(|view| {
            view.look_at(&Point3::new(position.x, position.y, position.z));
        });
    }

    fn object_matches_cycle_filter(
        object: &gamelogic::object::Object,
        filter: SelectionCycleFilter,
    ) -> bool {
        if !object.is_locally_controlled() || object.is_contained() {
            return false;
        }

        match filter {
            SelectionCycleFilter::Unit => {
                object.is_mobile() && !object.is_kind_of(KindOf::NoSelect)
            }
            SelectionCycleFilter::Worker => object.is_kind_of(KindOf::Dozer),
        }
    }

    fn cycle_candidates(filter: SelectionCycleFilter) -> Vec<ObjectID> {
        let mut candidates = Vec::new();
        for object in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = object.read() else {
                continue;
            };

            if Self::object_matches_cycle_filter(&guard, filter) {
                candidates.push(guard.get_id());
            }
        }

        candidates.sort_unstable();
        candidates
    }

    fn current_cycle_anchor(&self, candidates: &[ObjectID]) -> Option<usize> {
        self.selected_objects
            .iter()
            .filter_map(|id| candidates.binary_search(id).ok())
            .min()
    }

    fn select_single_object(&mut self, object_id: ObjectID) -> Vec<GameMessageType> {
        self.selected_objects.clear();
        self.selected_objects.insert(object_id);

        Self::look_at_object(object_id);

        vec![GameMessageType::CreateSelectedGroup(true, vec![object_id])]
    }

    fn look_at_object(object_id: ObjectID) {
        if let Some(position) = OBJECT_REGISTRY
            .get_object(object_id)
            .and_then(|object| object.read().ok().map(|guard| *guard.get_position()))
        {
            with_tactical_view(|view| {
                view.look_at(&Point3::new(position.x, position.y, position.z));
            });
        }
    }

    fn handle_cycle_selection(
        &mut self,
        filter: SelectionCycleFilter,
        direction: SelectionCycleDirection,
    ) -> Vec<GameMessageType> {
        let candidates = Self::cycle_candidates(filter);
        if candidates.is_empty() {
            return Vec::new();
        }

        let selected_index = match self.current_cycle_anchor(&candidates) {
            Some(index) => match direction {
                SelectionCycleDirection::Forward => (index + 1) % candidates.len(),
                SelectionCycleDirection::Backward => {
                    if index == 0 {
                        candidates.len() - 1
                    } else {
                        index - 1
                    }
                }
            },
            None => match direction {
                SelectionCycleDirection::Forward => 0,
                SelectionCycleDirection::Backward => candidates.len() - 1,
            },
        };

        self.select_single_object(candidates[selected_index])
    }

    fn handle_select_hero(&mut self) -> Vec<GameMessageType> {
        let mut heroes: Vec<ObjectID> = OBJECT_REGISTRY
            .get_all_objects()
            .into_iter()
            .filter_map(|object| {
                let guard = object.read().ok()?;
                if !guard.is_locally_controlled() || !guard.is_kind_of(KindOf::Hero) {
                    return None;
                }
                Some(guard.get_contained_by().unwrap_or_else(|| guard.get_id()))
            })
            .collect();

        heroes.sort_unstable();
        heroes.dedup();

        if let Some(hero_or_container) = heroes.first().copied() {
            self.select_single_object(hero_or_container)
        } else {
            Vec::new()
        }
    }

    fn handle_view_command_center(&self) {
        let mut command_center: Option<ObjectID> = None;
        let mut fallback: Option<(i32, ObjectID)> = None;

        for object in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = object.read() else {
                continue;
            };

            if !guard.is_locally_controlled() {
                continue;
            }

            if guard.is_kind_of(KindOf::CommandCenter) {
                command_center = Some(match command_center {
                    Some(current) => current.min(guard.get_id()),
                    None => guard.get_id(),
                });
                continue;
            }

            if guard.is_kind_of(KindOf::Structure) {
                let candidate = (guard.get_build_cost(), guard.get_id());
                fallback = Some(match fallback {
                    Some(current) if current.0 > candidate.0 => current,
                    Some(current) if current.0 == candidate.0 => {
                        (current.0, current.1.min(candidate.1))
                    }
                    _ => candidate,
                });
            }
        }

        if let Some(object_id) = command_center.or_else(|| fallback.map(|(_, id)| id)) {
            Self::look_at_object(object_id);
        }
    }

    fn selected_local_template_names(&self) -> Vec<String> {
        let mut templates = Vec::new();
        for &object_id in &self.selected_objects {
            let Some(object) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(guard) = object.read() else {
                continue;
            };
            if !guard.is_locally_controlled() {
                continue;
            }

            let template = guard.get_template_name().to_string();
            if !templates.contains(&template) {
                templates.push(template);
            }
        }
        templates
    }

    fn matching_selection_screen_region() -> Option<IRegion2D> {
        with_tactical_view_ref(|view| {
            let top_left = view.screen_to_world(&IPoint2::new(0, 0)).ok()?;
            let bottom_right = view
                .screen_to_world(&IPoint2::new(view.width(), view.height()))
                .ok()?;
            Some(IRegion2D {
                x: top_left.x.min(bottom_right.x).floor() as i32,
                y: top_left.y.min(bottom_right.y).floor() as i32,
                width: (top_left.x.max(bottom_right.x).ceil()
                    - top_left.x.min(bottom_right.x).floor()) as i32,
                height: (top_left.y.max(bottom_right.y).ceil()
                    - top_left.y.min(bottom_right.y).floor()) as i32,
            })
        })
    }

    fn collect_matching_selection(&self, region: Option<&IRegion2D>) -> Vec<ObjectID> {
        let templates = self.selected_local_template_names();
        if templates.is_empty() {
            return Vec::new();
        }

        let mut matching = Vec::new();
        for object in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = object.read() else {
                continue;
            };

            if !guard.is_selectable() || !guard.is_locally_controlled() {
                continue;
            }

            if let Some(region) = region {
                let position = guard.get_position();
                let min_x = region.x.min(region.x + region.width) as f32;
                let max_x = region.x.max(region.x + region.width) as f32;
                let min_y = region.y.min(region.y + region.height) as f32;
                let max_y = region.y.max(region.y + region.height) as f32;
                if position.x < min_x
                    || position.x > max_x
                    || position.y < min_y
                    || position.y > max_y
                {
                    continue;
                }
            }

            if templates
                .iter()
                .any(|template| template == guard.get_template_name())
            {
                matching.push(guard.get_id());
            }
        }

        matching.sort_unstable();
        matching.dedup();
        matching
    }

    fn handle_select_matching_units(&mut self) -> Vec<GameMessageType> {
        if self.selected_local_template_names().is_empty() {
            return Vec::new();
        }

        let screen_region = Self::matching_selection_screen_region();
        let mut matching = screen_region
            .as_ref()
            .map(|region| self.collect_matching_selection(Some(region)))
            .unwrap_or_default();

        if matching.is_empty() {
            matching = self.collect_matching_selection(None);
        }

        if matching.is_empty() {
            return Vec::new();
        }

        self.selected_objects.extend(matching.iter().copied());
        vec![GameMessageType::CreateSelectedGroupNoSound(false, matching)]
    }

    fn handle_select_all(&mut self, aircraft_only: bool) -> Vec<GameMessageType> {
        let mut selected = Vec::new();
        for object in OBJECT_REGISTRY.get_all_objects() {
            let Ok(guard) = object.read() else {
                continue;
            };

            if !guard.is_mobile()
                || !guard.is_locally_controlled()
                || guard.is_contained()
                || guard.is_effectively_dead()
                || !guard.is_mass_selectable()
                || guard.is_any_kind_of(&[
                    KindOf::Dozer,
                    KindOf::Harvester,
                    KindOf::IgnoresSelectAll,
                ])
            {
                continue;
            }

            if aircraft_only {
                if !guard.is_kind_of(KindOf::Aircraft) {
                    continue;
                }
            } else if guard.is_kind_of(KindOf::Structure) || guard.is_kind_of(KindOf::Building) {
                continue;
            }

            selected.push(guard.get_id());
        }

        selected.sort_unstable();
        self.selected_objects.clear();
        self.selected_objects.extend(selected.iter().copied());
        vec![GameMessageType::CreateSelectedGroup(true, selected)]
    }
}

fn collect_selectable_objects(
    region: &IRegion2D,
    is_point: bool,
    pick_radius_world: f32,
    local_player: Option<u32>,
    profile: ContextPickProfile,
) -> (Vec<(ObjectID, f32)>, Vec<(ObjectID, f32)>) {
    let min_x = region.x.min(region.x + region.width);
    let min_y = region.y.min(region.y + region.height);
    let max_x = region.x.max(region.x + region.width);
    let max_y = region.y.max(region.y + region.height);

    let cx = region.x as f32;
    let cy = region.y as f32;
    let radius_sq = pick_radius_world * pick_radius_world;

    let mut mine = Vec::new();
    let mut other = Vec::new();

    for obj_ref in OBJECT_REGISTRY.get_all_objects() {
        let Ok(obj) = obj_ref.read() else {
            continue;
        };

        if obj.is_destroyed() {
            continue;
        }

        let selectable_kind =
            obj.is_kind_of(KindOf::Selectable) || obj.is_kind_of(KindOf::AlwaysSelectable);
        let mine_kind = obj.is_kind_of(KindOf::Mine);
        let shrubbery_kind = obj.is_kind_of(KindOf::Shrubbery);
        let force_attackable_kind = obj.is_kind_of(KindOf::ForceAttackable);

        let selectable_pick = profile.include_selectable && selectable_kind;
        let mine_pick = profile.include_mines && mine_kind;
        let shrubbery_pick = profile.include_shrubbery && shrubbery_kind;
        let force_attackable_pick = profile.include_force_attackable && force_attackable_kind;
        let special_pick = mine_pick || shrubbery_pick || force_attackable_pick;

        if !(selectable_pick || special_pick) {
            continue;
        }

        let status = obj.get_status_bits();
        if status.contains(LogicObjectStatusMaskType::UNSELECTABLE) && !special_pick {
            continue;
        }
        if status.contains(LogicObjectStatusMaskType::MASKED)
            && !(shrubbery_pick || force_attackable_pick)
        {
            continue;
        }

        let pos = obj.get_position();
        let x = pos.x as i32;
        let y = pos.y as i32;

        let in_region = if is_point {
            let dx = pos.x - cx;
            let dy = pos.y - cy;
            (dx * dx + dy * dy) <= radius_sq
        } else {
            x >= min_x && x <= max_x && y >= min_y && y <= max_y
        };

        if !in_region {
            continue;
        }

        let dx = pos.x - cx;
        let dy = pos.y - cy;
        let dist_sq = dx * dx + dy * dy;

        let is_mine = local_player
            .and_then(|pid| obj.get_controlling_player_id().map(|owner| owner == pid))
            .unwrap_or(false);

        if is_mine {
            mine.push((obj.get_id(), dist_sq));
        } else {
            other.push((obj.get_id(), dist_sq));
        }
    }

    (mine, other)
}

fn pick_closest(candidates: &mut Vec<(ObjectID, f32)>) -> Option<ObjectID> {
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    candidates.first().map(|(id, _)| *id)
}

fn selection_counts(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
) -> (usize, usize, usize, usize) {
    let mut mine = 0usize;
    let mut mine_infantry = 0usize;
    let mut mine_buildings = 0usize;
    let mut other = 0usize;

    for &id in selection {
        let Some(obj) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(guard) = obj.read() else {
            continue;
        };
        if guard.is_destroyed() {
            continue;
        }

        let is_mine = local_player
            .and_then(|pid| guard.get_controlling_player_id().map(|owner| owner == pid))
            .unwrap_or(false);

        if is_mine {
            mine += 1;
            if guard.is_kind_of(KindOf::Infantry) {
                mine_infantry += 1;
            }
            if guard.is_kind_of(KindOf::Structure) || guard.is_kind_of(KindOf::Building) {
                mine_buildings += 1;
            }
        } else {
            other += 1;
        }
    }

    (mine, mine_infantry, mine_buildings, other)
}

fn selection_can_attack_target(
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

        if !sel_guard.is_able_to_attack() {
            continue;
        }

        let result = sel_guard.get_able_to_attack_specific_object(
            AbleToAttackType::NewTarget,
            &target_guard,
            CommandSourceType::FromPlayer,
        );

        if !matches!(
            result,
            CanAttackResult::NotPossible | CanAttackResult::InvalidShot
        ) {
            return true;
        }
    }

    false
}

fn selection_can_hijack_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_can_action(local_player, selection, target_id, |sel, target, source| {
        ActionManager::can_hijack_vehicle(sel, target, source)
    })
}

fn selection_can_convert_to_carbomb_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_can_action(local_player, selection, target_id, |sel, target, source| {
        ActionManager::can_convert_object_to_car_bomb(sel, target, source)
    })
}

fn selection_can_sabotage_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_can_action(local_player, selection, target_id, |sel, target, source| {
        ActionManager::can_sabotage_building(sel, target, source)
    })
}

fn selection_can_capture_building_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_can_action(local_player, selection, target_id, |sel, target, source| {
        ActionManager::can_capture_building(sel, target, source)
    })
}

fn selection_can_disable_vehicle_hack_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_can_action(local_player, selection, target_id, |sel, target, source| {
        ActionManager::can_disable_vehicle_via_hacking(sel, target, source, true)
    })
}

fn selection_can_steal_cash_hack_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_can_action(local_player, selection, target_id, |sel, target, source| {
        ActionManager::can_steal_cash_via_hacking(sel, target, source)
    })
}

fn selection_can_disable_building_hack_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    selection_can_action(local_player, selection, target_id, |sel, target, source| {
        ActionManager::can_disable_building_via_hacking(sel, target, source)
    })
}

fn selection_can_action<F>(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
    mut predicate: F,
) -> bool
where
    F: FnMut(&gamelogic::object::Object, &gamelogic::object::Object, CommandSourceType) -> bool,
{
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

        if predicate(&sel_guard, &target_guard, CommandSourceType::FromPlayer) {
            return true;
        }
    }

    false
}

fn selection_has_quick_path_to(selection: &HashSet<ObjectID>, world: &Coord3D) -> bool {
    let world = LogicCoord3D::new(world.x, world.y, world.z);

    let local_player = get_local_player_id();
    if local_player >= 0 {
        if let Ok(shroud) = get_shroud_manager().lock() {
            if shroud.get_shroud_state(local_player as u32, &world) != ShroudState::Visible {
                // C++ parity: when target point is fogged/shrouded, skip quick-path rejection.
                return true;
            }
        }
    }

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let Some(ai_arc) = sel_guard.get_ai() else {
            continue;
        };
        let Ok(ai_guard) = ai_arc.lock() else {
            continue;
        };

        if ai_guard.is_quick_path_available(&world) {
            return true;
        }

        if ai_guard.has_locomotor_for_surface(SURFACE_CLIFF) {
            if let Some(terrain) = TheTerrainLogic::get() {
                if terrain.is_cliff_cell(world.x, world.y) {
                    return true;
                }
            }
        }
    }

    false
}

fn selection_has_attack_capability(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
) -> bool {
    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };
        if sel_guard.is_destroyed() {
            continue;
        }

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

        if sel_guard.is_able_to_attack() {
            return true;
        }
    }

    false
}

fn selection_can_enter_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let contain = {
        let Ok(target_guard) = target.read() else {
            return false;
        };
        target_guard.get_contain()
    };

    let Some(contain) = contain else {
        return false;
    };
    let Ok(contain_guard) = contain.lock() else {
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
        if !sel_guard.is_kind_of(KindOf::Infantry) {
            continue;
        }

        if contain_guard.can_contain(id) {
            return true;
        }
    }

    false
}

fn selection_can_repair_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    let Some(local_player) = local_player else {
        return false;
    };

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

        let is_mine = sel_guard
            .get_controlling_player_id()
            .map(|owner| owner == local_player)
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if ActionManager::can_repair_object(
            &sel_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
        ) && (current_repairer == gamelogic::common::INVALID_ID
            || current_repairer == sel_guard.get_id())
        {
            return true;
        }
    }

    false
}

fn selection_can_resume_construction_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    let Some(local_player) = local_player else {
        return false;
    };

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

        let is_mine = sel_guard
            .get_controlling_player_id()
            .map(|owner| owner == local_player)
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

fn selection_can_dock_at_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    let Some(local_player) = local_player else {
        return false;
    };

    if selection.is_empty() {
        return false;
    }

    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };
    let mut saw_any = false;

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            return false;
        };
        let Ok(sel_guard) = sel.read() else {
            return false;
        };

        let is_mine = sel_guard
            .get_controlling_player_id()
            .map(|owner| owner == local_player)
            .unwrap_or(false);
        if !is_mine {
            return false;
        }

        if !ActionManager::can_dock_at(&sel_guard, &target_guard, CommandSourceType::FromPlayer) {
            return false;
        }
        saw_any = true;
    }

    saw_any
}

fn selection_can_get_repaired_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    let Some(local_player) = local_player else {
        return false;
    };

    if selection.is_empty() {
        return false;
    }

    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };
    if let Some(contain) = target_guard.get_contain() {
        if let Ok(contain_guard) = contain.lock() {
            if contain_guard.is_heal_contain() {
                return false;
            }
        }
    }

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let is_mine = sel_guard
            .get_controlling_player_id()
            .map(|owner| owner == local_player)
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if ActionManager::can_get_repaired_at(
            &sel_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
        ) {
            return true;
        }
    }

    false
}

fn selection_can_get_healed_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> bool {
    let Some(local_player) = local_player else {
        return false;
    };

    if selection.is_empty() {
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

        let is_mine = sel_guard
            .get_controlling_player_id()
            .map(|owner| owner == local_player)
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if ActionManager::can_get_healed_at(
            &sel_guard,
            &target_guard,
            CommandSourceType::FromPlayer,
        ) {
            return true;
        }
    }

    false
}

fn selection_can_pickup_crate_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> Option<Coord3D> {
    let Some(local_player) = local_player else {
        return None;
    };

    if selection.is_empty() {
        return None;
    }

    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return None;
    };
    let Ok(target_guard) = target.read() else {
        return None;
    };
    if target_guard.is_destroyed() || !target_guard.is_kind_of(KindOf::Crate) {
        return None;
    }

    for &id in selection {
        let Some(sel) = OBJECT_REGISTRY.get_object(id) else {
            continue;
        };
        let Ok(sel_guard) = sel.read() else {
            continue;
        };

        let is_mine = sel_guard
            .get_controlling_player_id()
            .map(|owner| owner == local_player)
            .unwrap_or(false);
        if !is_mine {
            continue;
        }

        if sel_guard.is_kind_of(KindOf::Unit) {
            return Some(logic_to_message_coord(target_guard.get_position()));
        }
    }

    None
}

fn selection_can_salvage_target(
    local_player: Option<u32>,
    selection: &HashSet<ObjectID>,
    target_id: ObjectID,
) -> Option<Coord3D> {
    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return None;
    };
    let Ok(target_guard) = target.read() else {
        return None;
    };
    if target_guard.is_destroyed() {
        return None;
    }

    // In Generals, salvage is typically performed on hulks/wrecks.
    if !target_guard.is_kind_of(KindOf::Hulk) {
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

        if sel_guard.is_kind_of(KindOf::Salvager)
            || sel_guard.is_kind_of(KindOf::WeaponSalvager)
            || sel_guard.is_kind_of(KindOf::ArmorSalvager)
        {
            return Some(logic_to_message_coord(target_guard.get_position()));
        }
    }

    None
}

fn is_enemy_target(local_player_id: i32, target_id: ObjectID) -> bool {
    if local_player_id < 0 {
        return false;
    }

    let Some(target) = OBJECT_REGISTRY.get_object(target_id) else {
        return false;
    };
    let Ok(target_guard) = target.read() else {
        return false;
    };
    let Some(owner) = target_guard.get_controlling_player_id() else {
        return false;
    };
    if owner as i32 == local_player_id {
        return false;
    }

    let Ok(list) = player_list().read() else {
        return false;
    };
    let Some(me) = list.get_player(local_player_id) else {
        return false;
    };
    let Some(them) = list.get_player(owner as i32) else {
        return false;
    };
    let (Ok(me_guard), Ok(them_guard)) = (me.read(), them.read()) else {
        return false;
    };

    matches!(
        me_guard.get_relationship(&them_guard),
        Relationship::Enemies
    )
}

fn dispatch_translated_message(message: &GameMessageType) {
    use GameMessageType::*;

    fn enqueue(message_type: GameMessageType) {
        match get_command_list().write() {
            Ok(mut list) => {
                let player = get_local_player_id();
                list.append_message(GameMessage::with_player(message_type, player));
            }
            Err(err) => {
                warn!("Failed to enqueue translated message: {}", err);
            }
        }
    }

    match message {
        CreateSelectedGroup(_, _)
        | CreateSelectedGroupNoSound(_, _)
        | DestroySelectedGroup(_)
        | RemoveFromSelectedGroup(_)
        | SelectedGroupCommand(_)
        | DoStop
        | DoScatter => {
            enqueue(message.clone());
        }
        AreaSelection(region) => {
            let upper_left = ICoord2D::new(region.x, region.y);
            let lower_right = ICoord2D::new(region.x + region.width, region.y + region.height);
            TheInGameUI::select_area(upper_left, lower_right);
            enqueue(message.clone());
        }
        DoMoveTo(pos) | DoForceMoveTO(pos) | AddWaypoint(pos) => {
            let queue = matches!(message, AddWaypoint(_) | DoForceMoveTO(_));
            let world = Coord3D::new(pos.x, pos.y, pos.z);
            TheInGameUI::issue_move_command(world, queue);
            enqueue(message.clone());
        }
        DoAttackMoveTo(_) => {
            TheInGameUI::clear_attack_move_to_mode();
            enqueue(message.clone());
        }
        DoSalvage(pos) => {
            let world = Coord3D::new(pos.x, pos.y, pos.z);
            TheInGameUI::issue_move_command(world, false);
            enqueue(message.clone());
        }
        DoForceAttackGround(pos) => {
            let world = Coord3D::new(pos.x, pos.y, pos.z);
            TheInGameUI::issue_force_attack_ground(world);
            enqueue(message.clone());
        }
        DoAttackObject(target) => {
            TheInGameUI::issue_attack_command(*target, false);
            enqueue(message.clone());
        }
        DoForceAttackObject(target) => {
            TheInGameUI::issue_attack_command(*target, true);
            enqueue(message.clone());
        }
        MetaToggleAttackMove => {
            TheInGameUI::toggle_attack_move_to_mode();
        }
        Exit(_)
        | Evacuate
        | EvacuateAtLocation(_)
        | ExecuteRailedTransport
        | DoAttackSquad(_)
        | DoGuardObject(_, _)
        | DoGuardPosition(_, _)
        | SetRallyPoint(_, _)
        | DozerConstruct(_, _, _)
        | DozerConstructLine(_, _, _, _)
        | DozerCancelConstruct(_)
        | Sell(_)
        | CombatDropAtLocation(_)
        | CombatDropAtObject(_)
        | GetRepaired(_)
        | GetHealed(_)
        | DoRepair(_)
        | ResumeConstruction(_)
        | Enter(_, _)
        | Dock(_)
        | DoWeapon(_)
        | DoWeaponAtLocation(_, _)
        | DoWeaponAtObject(_, _)
        | DoSpecialPower(_, _, _)
        | DoSpecialPowerAtLocation(_, _, _, _, _, _)
        | DoSpecialPowerAtObject(_, _, _, _)
        | DoSpecialPowerOverrideDestination(_, _, _)
        | InternetHack
        | DoCheer
        | ToggleOvercharge
        | SwitchWeapons(_)
        | ConvertToCarbomb(_, _)
        | CaptureBuilding(_, _)
        | DisableVehicleHack(_, _)
        | StealCashHack(_, _)
        | DisableBuildingHack(_, _)
        | SnipeVehicle(_, _)
        | PurchaseScience(_)
        | QueueUpgrade(_)
        | CancelUpgrade(_)
        | QueueUnitCreate(_)
        | CancelUnitCreate(_) => {
            enqueue(message.clone());
        }
        PlaceBeacon(coord) => {
            info!(
                "Placing beacon at ({:.1}, {:.1}, {:.1})",
                coord.x, coord.y, coord.z
            );
            let player = get_local_player_id();
            beacon_display::record_beacon_placed(player, coord.clone(), None);
            enqueue(message.clone());
        }
        RemoveBeacon(coord) => {
            info!(
                "Removing beacon at ({:.1}, {:.1}, {:.1})",
                coord.x, coord.y, coord.z
            );
            let player = get_local_player_id();
            beacon_display::record_beacon_removed(player, coord.clone());
            enqueue(message.clone());
        }
        SetBeaconText(coord, text) => {
            info!(
                "Setting beacon text at ({:.1}, {:.1}, {:.1}) to '{}'",
                coord.x, coord.y, coord.z, text
            );
            let player = get_local_player_id();
            beacon_display::record_beacon_text(player, coord.clone(), text.clone());
            enqueue(message.clone());
        }
        SetReplayCamera(coord, pitch, zoom) => {
            info!(
                "Setting replay camera to ({:.1}, {:.1}, {:.1}) pitch={:.1} zoom={:.1}",
                coord.x, coord.y, coord.z, pitch, zoom
            );
            enqueue(message.clone());
        }
        ClearInGamePopupMessage => {
            info!("Clearing in-game popup message");
            enqueue(message.clone());
        }
        SelfDestruct(player_id) => {
            info!("Triggering self-destruct for player {}", player_id);
            enqueue(message.clone());
        }
        CreateFormation(units) => {
            info!("Creating formation with {} units", units.len());
            enqueue(message.clone());
        }
        LogicCRC(crc) => {
            info!("Submitting logic CRC {:08X}", crc);
            enqueue(message.clone());
        }
        SetMineClearingDetail(level) => {
            info!("Setting mine clearing detail to {}", level);
            enqueue(message.clone());
        }
        EnableRetaliationMode(player_id, enabled) => {
            info!(
                "Setting retaliation mode for player {} to {}",
                player_id, enabled
            );
            enqueue(message.clone());
        }
        MetaStop => {
            TheInGameUI::issue_stop_command();
            enqueue(DoStop);
        }
        MetaScatter => {
            enqueue(DoScatter);
        }
        MetaCreateFormation => {
            enqueue(CreateFormation(Vec::new()));
        }
        _ => {
            if let Some(visual) = hint_visual_for_message(message) {
                apply_hint_visual(message, &visual);
                TheInGameUI::military_subtitle(&visual.text, 2000);
                return;
            }
            debug!("Unhandled translated message {:?}", message);
        }
    }
}

impl Default for SelectionTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl GameMessageTranslator for SelectionTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        let new_messages = match msg.get_type() {
            GameMessageType::AreaSelection(region) => self.handle_area_selection(region),
            GameMessageType::MetaCreateTeam(group) => self.handle_control_group_create(*group),
            GameMessageType::MetaSelectTeam(group) => self.handle_control_group_select(*group),
            GameMessageType::MetaAddTeam(group) => self.handle_control_group_add(*group),
            GameMessageType::MetaViewTeam(group) => {
                self.handle_control_group_view(*group);
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::MetaSelectAll => self.handle_select_all(false),
            GameMessageType::MetaSelectAllAircraft => self.handle_select_all(true),
            GameMessageType::MetaSelectMatchingUnits => self.handle_select_matching_units(),
            GameMessageType::MetaSelectNextUnit => self.handle_cycle_selection(
                SelectionCycleFilter::Unit,
                SelectionCycleDirection::Backward,
            ),
            GameMessageType::MetaSelectPrevUnit => self.handle_cycle_selection(
                SelectionCycleFilter::Unit,
                SelectionCycleDirection::Forward,
            ),
            GameMessageType::MetaSelectNextWorker => self.handle_cycle_selection(
                SelectionCycleFilter::Worker,
                SelectionCycleDirection::Backward,
            ),
            GameMessageType::MetaSelectPrevWorker => self.handle_cycle_selection(
                SelectionCycleFilter::Worker,
                SelectionCycleDirection::Forward,
            ),
            GameMessageType::MetaSelectHero => self.handle_select_hero(),
            GameMessageType::MetaViewCommandCenter => {
                self.handle_view_command_center();
                return GameMessageDisposition::DestroyMessage;
            }
            GameMessageType::CreateSelectedGroup(create_new, objects) => {
                if *create_new {
                    self.selected_objects.clear();
                }
                self.selected_objects.extend(objects.iter());
                debug!(
                    "Updated selection to {} objects",
                    self.selected_objects.len()
                );
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::CreateSelectedGroupNoSound(create_new, objects) => {
                if *create_new {
                    self.selected_objects.clear();
                }
                self.selected_objects.extend(objects.iter());
                debug!(
                    "Updated selection to {} objects",
                    self.selected_objects.len()
                );
                return GameMessageDisposition::KeepMessage;
            }
            GameMessageType::RemoveFromSelectedGroup(objects) => {
                for id in objects {
                    self.selected_objects.remove(id);
                }
                debug!(
                    "Updated selection to {} objects",
                    self.selected_objects.len()
                );
                return GameMessageDisposition::KeepMessage;
            }
            _ => {
                return GameMessageDisposition::KeepMessage;
            }
        };

        // Log generated messages
        for new_msg in new_messages {
            dispatch_translated_message(&new_msg);
        }

        GameMessageDisposition::KeepMessage
    }
}

/// GUI Command Translator - handles UI-specific commands
pub struct GUICommandTranslator {
    ui_state: HashMap<String, bool>,
}

impl GUICommandTranslator {
    pub fn new() -> Self {
        Self {
            ui_state: HashMap::new(),
        }
    }

    fn toggle_flag(&mut self, key: &str, default: bool) -> bool {
        let current = *self.ui_state.get(key).unwrap_or(&default);
        self.ui_state.insert(key.to_string(), !current);
        !current
    }

    fn handle_toggle_control_bar(&mut self) -> Vec<GameMessageType> {
        let new_state = self.toggle_flag("control_bar_visible", true);
        info!("Toggling control bar to: {}", new_state);
        if let Err(err) = toggle_control_bar(false) {
            warn!("Failed to toggle control bar: {}", err);
        }
        vec![] // UI changes don't generate game messages
    }

    fn handle_toggle_diplomacy(&mut self) -> Vec<GameMessageType> {
        let new_state = self.toggle_flag("diplomacy_visible", false);
        info!("Toggling diplomacy to: {}", new_state);
        if let Err(err) = toggle_diplomacy(false) {
            warn!("Failed to toggle diplomacy: {}", err);
        }
        vec![]
    }

    fn clear_pending_gui_command_mode(&self) {
        TheInGameUI::clear_pending_command();
        TheInGameUI::clear_pending_special_power();
        TheInGameUI::clear_attack_move_to_mode();
    }

    fn handle_pending_non_context_gui_click(
        &mut self,
        region: &IRegion2D,
    ) -> GameMessageDisposition {
        let Some(pending) = TheInGameUI::get_pending_command() else {
            return GameMessageDisposition::KeepMessage;
        };
        if !is_pending_gui_non_context_command(&pending) {
            return GameMessageDisposition::KeepMessage;
        }

        // C++ GUICommandTranslator uses pixelRegion.hi as click location.
        let click_pos = ICoord2D::new(region.x + region.width, region.y + region.height);
        let click_region = IRegion2D {
            x: click_pos.x,
            y: click_pos.y,
            width: 0,
            height: 0,
        };

        let world = screen_to_terrain(&click_pos).unwrap_or(Coord3D {
            x: click_pos.x as f32,
            y: click_pos.y as f32,
            z: 0.0,
        });

        let local_player = get_local_player_id();
        let local_player_u32 = if local_player >= 0 {
            Some(local_player as u32)
        } else {
            None
        };
        let selection_ids = current_local_selection(local_player);
        let target = pick_context_target_for_click(
            &click_region,
            local_player_u32,
            &selection_ids,
            TheInGameUI::is_in_force_attack_mode(),
        );

        let mut translated: Vec<GameMessageType> = Vec::new();
        if let Some(target_id) = target {
            if pending_command_accepts_object(pending.options)
                && pending_command_target_allowed(pending.options, local_player, target_id)
                && pending_command_selection_valid(
                    &pending,
                    local_player_u32,
                    &selection_ids,
                    target_id,
                )
            {
                if let Some(message) = pending_command_for_object(&pending, target_id) {
                    translated.push(message);
                }
            }

            if translated.is_empty() && pending_command_accepts_position(pending.options) {
                if let Some(obj) = OBJECT_REGISTRY.get_object(target_id) {
                    if let Ok(obj_guard) = obj.read() {
                        let position = logic_to_message_coord(obj_guard.get_position());
                        if pending_command_position_valid(
                            &pending,
                            local_player_u32,
                            &selection_ids,
                            &position,
                            Some(target_id),
                        ) {
                            translated = pending_command_messages_for_position(
                                &pending,
                                position,
                                &selection_ids,
                                Some(target_id),
                            );
                        }
                    }
                }
            }
        } else if pending_command_accepts_position(pending.options) {
            if pending_command_position_valid(
                &pending,
                local_player_u32,
                &selection_ids,
                &world,
                None,
            ) {
                translated =
                    pending_command_messages_for_position(&pending, world, &selection_ids, None);
            }
        }

        for message in &translated {
            dispatch_translated_message(message);
        }

        // C++ GUICommandTranslator suppresses one alternate-mouse blank-click deselect
        // after completing a non-context GUI command.
        TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(true);
        // Non-context GUI command clicks complete this mode even when target validation fails.
        self.clear_pending_gui_command_mode();
        GameMessageDisposition::DestroyMessage
    }
}

impl Default for GUICommandTranslator {
    fn default() -> Self {
        Self::new()
    }
}

impl GameMessageTranslator for GUICommandTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        if let Some(pending) = TheInGameUI::get_pending_command() {
            if is_pending_gui_non_context_command(&pending) {
                match msg.get_type() {
                    // Consume raw left input while in pending GUI command mode so selection
                    // translators do not start click/drag selection.
                    GameMessageType::RawMouseLeftButtonDown(..)
                    | GameMessageType::RawMouseLeftButtonUp(..) => {
                        return GameMessageDisposition::DestroyMessage;
                    }
                    GameMessageType::MouseLeftClick(region, _)
                    | GameMessageType::MouseLeftDoubleClick(region, _) => {
                        return self.handle_pending_non_context_gui_click(region);
                    }
                    _ => {}
                }
            }
        }

        match msg.get_type() {
            GameMessageType::MetaToggleControlBar => {
                self.handle_toggle_control_bar();
                GameMessageDisposition::DestroyMessage
            }
            GameMessageType::MetaDiplomacy => {
                self.handle_toggle_diplomacy();
                GameMessageDisposition::DestroyMessage
            }
            GameMessageType::MetaOptions => {
                info!("Toggling quit/options menu");
                toggle_quit_menu();
                GameMessageDisposition::DestroyMessage
            }
            _ => GameMessageDisposition::KeepMessage,
        }
    }
}

/// Hint Spy - processes hint messages for UI feedback
pub struct HintSpy {
    last_hint: Option<String>,
}

struct HintVisual {
    text: String,
    cursor: &'static str,
    radius_cursor: bool,
}

fn is_gui_command_hint_message(msg: &GameMessageType) -> bool {
    matches!(
        msg,
        GameMessageType::ValidGUICommandHint | GameMessageType::InvalidGUICommandHint
    )
}

fn apply_hint_visual(msg: &GameMessageType, hint: &HintVisual) {
    TheInGameUI::set_hint_text(&hint.text);
    TheInGameUI::set_cursor_by_name(hint.cursor);
    if hint.radius_cursor {
        if is_gui_command_hint_message(msg) {
            if let Some(pending) = TheInGameUI::get_pending_command() {
                TheInGameUI::set_radius_cursor_active_with_type(&pending.radius_cursor_type);
            } else {
                TheInGameUI::set_radius_cursor_active();
            }
        } else {
            TheInGameUI::set_radius_cursor_active();
        }
    } else {
        TheInGameUI::set_radius_cursor_none();
    }
}

impl HintSpy {
    pub fn new() -> Self {
        Self { last_hint: None }
    }

    fn process_hint(&mut self, msg: &GameMessageType, hint: HintVisual) {
        debug!("Processing hint: {}", hint.text);
        self.last_hint = Some(hint.text.clone());
        apply_hint_visual(msg, &hint);
    }
}

fn hint_visual_for_message(msg: &GameMessageType) -> Option<HintVisual> {
    use GameMessageType::*;

    fn normalize_cursor_name(cursor_name: &str, fallback: &'static str) -> &'static str {
        match cursor_name {
            "ARROW" => "ARROW",
            "CROSS" => "CROSS",
            "SELECTING" => "SELECTING",
            "MOVETO" => "MOVETO",
            "ATTACKMOVETO" => "ATTACKMOVETO",
            "WAYPOINT" => "WAYPOINT",
            "ATTACK_OBJECT" => "ATTACK_OBJECT",
            "OUTRANGE" => "OUTRANGE",
            "FORCE_ATTACK_OBJECT" => "FORCE_ATTACK_OBJECT",
            "FORCE_ATTACK_GROUND" => "FORCE_ATTACK_GROUND",
            "GET_REPAIRED" => "GET_REPAIRED",
            "DOCK" => "DOCK",
            "GET_HEALED" => "GET_HEALED",
            "DO_REPAIR" => "DO_REPAIR",
            "RESUME_CONSTRUCTION" => "RESUME_CONSTRUCTION",
            "ENTER_FRIENDLY" => "ENTER_FRIENDLY",
            "ENTER_AGGRESSIVELY" => "ENTER_AGGRESSIVELY",
            "DEFECTOR" => "DEFECTOR",
            "CAPTUREBUILDING" => "CAPTUREBUILDING",
            "HACK" => "HACK",
            "GENERIC_INVALID" => "GENERIC_INVALID",
            "SET_RALLY_POINT" => "SET_RALLY_POINT",
            "PARTICLE_UPLINK_CANNON" => "PARTICLE_UPLINK_CANNON",
            _ => fallback,
        }
    }

    fn pending_command_uses_context_cursor_behavior(pending: &PendingCommand) -> bool {
        (pending.options & CMD_CONTEXTMODE_COMMAND) != 0
            || matches!(
                pending.command_type,
                CommandType::SpecialPower
                    | CommandType::DoSpecialPowerAtLocation
                    | CommandType::DoSpecialPowerAtObject
            )
    }

    fn normalize_radius_cursor_type(radius_cursor_type: &str) -> Option<&'static str> {
        let radius_type = radius_cursor_type.trim();
        if radius_type.is_empty() || radius_type.eq_ignore_ascii_case("NONE") {
            return None;
        }

        const KNOWN_TYPES: &[&str] = &[
            "ATTACK_DAMAGE_AREA",
            "ATTACK_SCATTER_AREA",
            "ATTACK_CONTINUE_AREA",
            "CLEARMINES",
            "GUARD_AREA",
            "FRIENDLY_SPECIALPOWER",
            "OFFENSIVE_SPECIALPOWER",
            "SUPERWEAPON_SCATTER_AREA",
            "EMERGENCY_REPAIR",
            "PARTICLECANNON",
            "A10STRIKE",
            "SPECTREGUNSHIP",
            "HELIX_NAPALM_BOMB",
            "DAISYCUTTER",
            "CARPETBOMB",
            "PARADROP",
            "SPYSATELLITE",
            "NUCLEARMISSILE",
            "EMPPULSE",
            "ARTILLERYBARRAGE",
            "FRENZY",
            "NAPALMSTRIKE",
            "CLUSTERMINES",
            "SCUDSTORM",
            "ANTHRAXBOMB",
            "AMBUSH",
            "RADAR",
            "SPYDRONE",
            "AMBULANCE",
        ];

        KNOWN_TYPES
            .iter()
            .copied()
            .find(|known| radius_type.eq_ignore_ascii_case(known))
    }

    fn radius_cursor_requires_special_power_payload(radius_cursor_type: &str) -> bool {
        matches!(
            radius_cursor_type,
            "FRIENDLY_SPECIALPOWER"
                | "OFFENSIVE_SPECIALPOWER"
                | "SUPERWEAPON_SCATTER_AREA"
                | "EMERGENCY_REPAIR"
                | "PARTICLECANNON"
                | "A10STRIKE"
                | "SPECTREGUNSHIP"
                | "HELIX_NAPALM_BOMB"
                | "DAISYCUTTER"
                | "CARPETBOMB"
                | "PARADROP"
                | "SPYSATELLITE"
                | "NUCLEARMISSILE"
                | "EMPPULSE"
                | "ARTILLERYBARRAGE"
                | "FRENZY"
                | "NAPALMSTRIKE"
                | "CLUSTERMINES"
                | "SCUDSTORM"
                | "ANTHRAXBOMB"
                | "AMBUSH"
                | "RADAR"
                | "SPYDRONE"
                | "AMBULANCE"
        )
    }

    fn pending_command_radius_cursor_active(pending: &PendingCommand) -> bool {
        let Some(radius_type) = normalize_radius_cursor_type(&pending.radius_cursor_type) else {
            return false;
        };

        let should_attempt_radius = pending_command_uses_context_cursor_behavior(pending)
            || pending_command_accepts_position(pending.options)
            || pending_command_accepts_object(pending.options);
        if !should_attempt_radius {
            return false;
        }

        if radius_cursor_requires_special_power_payload(radius_type) {
            return TheInGameUI::get_pending_special_power().is_some();
        }

        true
    }

    fn pending_command_hint_cursor(pending: &PendingCommand, valid: bool) -> &'static str {
        let cursor_name = if valid {
            pending.cursor_name.as_str()
        } else if pending_command_uses_context_cursor_behavior(pending) {
            pending.invalid_cursor_name.as_str()
        } else {
            pending.cursor_name.as_str()
        };
        let fallback = "CROSS";
        if cursor_name.trim().is_empty() {
            fallback
        } else {
            normalize_cursor_name(cursor_name, fallback)
        }
    }

    let visual = match msg {
        MouseoverDrawableHint(drawable) => HintVisual {
            text: format!("Mouse over drawable {}", drawable),
            cursor: "ARROW",
            radius_cursor: false,
        },
        MouseoverLocationHint(pos) => HintVisual {
            text: format!("Mouse over location {:?}", pos),
            cursor: "ARROW",
            radius_cursor: false,
        },
        ValidGUICommandHint => {
            let pending = TheInGameUI::get_pending_command();
            let radius_from_pending = pending
                .as_ref()
                .map(pending_command_radius_cursor_active)
                .unwrap_or(false);
            HintVisual {
                text: "Valid GUI command".to_string(),
                cursor: pending
                    .as_ref()
                    .map(|cmd| pending_command_hint_cursor(cmd, true))
                    .unwrap_or("CROSS"),
                radius_cursor: radius_from_pending,
            }
        }
        InvalidGUICommandHint => {
            let pending = TheInGameUI::get_pending_command();
            let radius_from_pending = pending
                .as_ref()
                .map(pending_command_radius_cursor_active)
                .unwrap_or(false);
            HintVisual {
                text: "Invalid GUI command".to_string(),
                cursor: pending
                    .as_ref()
                    .map(|cmd| pending_command_hint_cursor(cmd, false))
                    .unwrap_or("GENERIC_INVALID"),
                radius_cursor: radius_from_pending,
            }
        }
        AreaSelectionHint(region) => HintVisual {
            text: format!("Area selection {:?}", region),
            cursor: "SELECTING",
            radius_cursor: false,
        },
        DoMoveToHint(pos) => HintVisual {
            text: format!("Move to {:?}", pos),
            cursor: "MOVETO",
            radius_cursor: false,
        },
        DoAttackMoveToHint(pos) => HintVisual {
            text: format!("Attack move to {:?}", pos),
            cursor: "ATTACKMOVETO",
            radius_cursor: false,
        },
        AddWaypointHint(pos) => HintVisual {
            text: format!("Add waypoint {:?}", pos),
            cursor: "WAYPOINT",
            radius_cursor: false,
        },
        DoAttackObjectHint(target) => HintVisual {
            text: format!("Attack object {}", target),
            cursor: "ATTACK_OBJECT",
            radius_cursor: false,
        },
        DoAttackObjectAfterMovingHint(target) => HintVisual {
            text: format!("Attack object after moving {}", target),
            cursor: "OUTRANGE",
            radius_cursor: false,
        },
        ImpossibleAttackHint => HintVisual {
            text: "Impossible attack".to_string(),
            cursor: "GENERIC_INVALID",
            radius_cursor: false,
        },
        DoForceAttackObjectHint(target) => HintVisual {
            text: format!("Force attack object {}", target),
            cursor: "FORCE_ATTACK_OBJECT",
            radius_cursor: false,
        },
        DoForceAttackGroundHint(pos) => HintVisual {
            text: format!("Force attack ground {:?}", pos),
            cursor: "FORCE_ATTACK_GROUND",
            radius_cursor: false,
        },
        GetRepairedHint(target) => HintVisual {
            text: format!("Get repaired {}", target),
            cursor: "GET_REPAIRED",
            radius_cursor: false,
        },
        DockHint(target) => HintVisual {
            text: format!("Dock at object {}", target),
            cursor: "DOCK",
            radius_cursor: false,
        },
        GetHealedHint(target) => HintVisual {
            text: format!("Get healed {}", target),
            cursor: "GET_HEALED",
            radius_cursor: false,
        },
        DoRepairHint(target) => HintVisual {
            text: format!("Repair object {}", target),
            cursor: "DO_REPAIR",
            radius_cursor: false,
        },
        ResumeConstructionHint(target) => HintVisual {
            text: format!("Resume construction {}", target),
            cursor: "RESUME_CONSTRUCTION",
            radius_cursor: false,
        },
        EnterHint(target) => HintVisual {
            text: format!("Enter object {}", target),
            cursor: "ENTER_FRIENDLY",
            radius_cursor: false,
        },
        HijackHint(target) => HintVisual {
            text: format!("Hijack object {}", target),
            cursor: "ENTER_AGGRESSIVELY",
            radius_cursor: false,
        },
        SabotageHint(target) => HintVisual {
            text: format!("Sabotage object {}", target),
            cursor: "ENTER_AGGRESSIVELY",
            radius_cursor: false,
        },
        FirebombHint(target) => HintVisual {
            text: format!("Firebomb object {}", target),
            cursor: "ENTER_AGGRESSIVELY",
            radius_cursor: false,
        },
        ConvertToCarbombHint(target) => HintVisual {
            text: format!("Convert to carbomb {}", target),
            cursor: "ENTER_AGGRESSIVELY",
            radius_cursor: false,
        },
        CaptureBuildingHint(target) => HintVisual {
            text: format!("Capture building {}", target),
            cursor: "CAPTUREBUILDING",
            radius_cursor: false,
        },
        SnipeVehicleHint(target) => HintVisual {
            text: format!("Snipe vehicle {}", target),
            cursor: "ATTACK_OBJECT",
            radius_cursor: false,
        },
        DefectorHint(target) => HintVisual {
            text: format!("Defector {}", target),
            cursor: "DEFECTOR",
            radius_cursor: false,
        },
        HackHint(target) => HintVisual {
            text: format!("Hack object {}", target),
            cursor: "HACK",
            radius_cursor: false,
        },
        SetRallyPointHint(pos) => HintVisual {
            text: format!("Set rally point {:?}", pos),
            cursor: "SET_RALLY_POINT",
            radius_cursor: false,
        },
        DoSpecialPowerOverrideDestinationHint(pos) => HintVisual {
            text: format!("Special power destination {:?}", pos),
            cursor: "PARTICLE_UPLINK_CANNON",
            radius_cursor: false,
        },
        DoSalvageHint(pos) => HintVisual {
            text: format!("Salvage {:?}", pos),
            cursor: "MOVETO",
            radius_cursor: false,
        },
        DoInvalidHint => HintVisual {
            text: "Invalid action".to_string(),
            cursor: "GENERIC_INVALID",
            radius_cursor: false,
        },
        _ => return None,
    };

    Some(visual)
}

impl Default for HintSpy {
    fn default() -> Self {
        Self::new()
    }
}

impl GameMessageTranslator for HintSpy {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        if let Some(hint) = hint_visual_for_message(msg.get_type()) {
            self.process_hint(msg.get_type(), hint);
            GameMessageDisposition::DestroyMessage
        } else {
            GameMessageDisposition::KeepMessage
        }
    }
}

/// Translator factory for creating and managing translators
pub struct TranslatorFactory {}

impl TranslatorFactory {
    pub fn new() -> Self {
        Self {}
    }

    /// Create a command translator
    pub fn create_command_translator() -> Arc<RwLock<CommandTranslator>> {
        Arc::new(RwLock::new(CommandTranslator::new()))
    }

    /// Create a selection translator  
    pub fn create_selection_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(SelectionTranslatorXlat::new()))
    }

    /// Create a window translator
    pub fn create_window_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(WindowTranslator::new()))
    }

    /// Create a meta event translator
    pub fn create_meta_event_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(MetaEventTranslator::new()))
    }

    /// Create a look-at translator
    pub fn create_look_at_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(LookAtTranslator::new()))
    }

    /// Create a hot key translator
    pub fn create_hot_key_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(HotKeyTranslator::new()))
    }

    /// Create a placement translator
    pub fn create_place_event_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(PlaceEventTranslator::new()))
    }

    /// Create a GUI command translator
    pub fn create_gui_command_translator() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(GUICommandTranslator::new()))
    }

    /// Create a hint spy translator
    pub fn create_hint_spy() -> Arc<RwLock<dyn GameMessageTranslator>> {
        Arc::new(RwLock::new(HintSpy::new()))
    }

    /// Create the standard set of translators with appropriate priorities
    pub fn create_standard_translator_set() -> Vec<(Arc<RwLock<dyn GameMessageTranslator>>, u32)> {
        let command_translator: Arc<RwLock<dyn GameMessageTranslator>> =
            Self::create_command_translator();

        vec![
            (Self::create_window_translator(), 10), // Window input handling
            (Self::create_meta_event_translator(), 20), // Meta key remapping
            (Self::create_hot_key_translator(), 25), // UI hotkeys
            (Self::create_place_event_translator(), 30), // Placement handling
            (Self::create_gui_command_translator(), 40), // UI commands
            (Self::create_selection_translator(), 50), // Selection handling
            (Self::create_look_at_translator(), 60), // Camera movement
            (command_translator, 70),               // Command processing
            (Self::create_hint_spy(), 100),         // Hints and feedback
        ]
    }
}

impl Default for TranslatorFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::common::{AsciiString, GeometryInfo, Real};
    use gamelogic::player::{player_list, Player};
    use gamelogic::system::game_logic::{get_game_logic, GAME_LAN, GAME_SINGLE_PLAYER};
    use gamelogic::team::Team;
    use gamelogic::thing_template::ThingTemplate;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn test_state_lock() -> MutexGuard<'static, ()> {
        static TEST_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_STATE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    #[derive(Debug)]
    struct TestThingTemplate {
        name: AsciiString,
        geometry: GeometryInfo,
        kinds: Vec<KindOf>,
        build_cost: i32,
    }

    impl TestThingTemplate {
        fn new(name: &str, kinds: Vec<KindOf>) -> Self {
            Self::new_with_cost(name, kinds, 0)
        }

        fn new_with_cost(name: &str, kinds: Vec<KindOf>, build_cost: i32) -> Self {
            Self {
                name: AsciiString::from(name),
                geometry: GeometryInfo::default(),
                kinds,
                build_cost,
            }
        }
    }

    impl ThingTemplate for TestThingTemplate {
        fn get_name(&self) -> &AsciiString {
            &self.name
        }

        fn get_template_geometry_info(&self) -> GeometryInfo {
            self.geometry.clone()
        }

        fn calc_vision_range(&self) -> Real {
            100.0
        }

        fn calc_shroud_clearing_range(&self) -> Real {
            100.0
        }

        fn is_kind_of(&self, kind: KindOf) -> bool {
            self.kinds.contains(&kind)
        }

        fn get_build_cost(&self) -> i32 {
            self.build_cost
        }
    }

    fn setup_local_player_team() -> Arc<RwLock<Team>> {
        crate::message_stream::player_state::set_local_player_id(0);
        {
            let list = player_list();
            let mut guard = list.write().unwrap();
            guard.clear();
            guard.add_player(Arc::new(RwLock::new(Player::new(0))));
            guard.set_local_player_index(0);
        }

        let team = Arc::new(RwLock::new(Team::new(AsciiString::from("teamLocal"), 1)));
        team.write().unwrap().set_controlling_player_id(Some(0));
        team
    }

    fn register_test_object(
        id: ObjectID,
        kinds: Vec<KindOf>,
        team: Arc<RwLock<Team>>,
    ) -> Arc<RwLock<gamelogic::object::Object>> {
        register_test_object_with_cost(id, kinds, team, 0)
    }

    fn register_test_object_with_cost(
        id: ObjectID,
        kinds: Vec<KindOf>,
        team: Arc<RwLock<Team>>,
        build_cost: i32,
    ) -> Arc<RwLock<gamelogic::object::Object>> {
        register_test_object_with_name_and_cost(id, &format!("Object{id}"), kinds, team, build_cost)
    }

    fn register_test_object_with_name(
        id: ObjectID,
        name: &str,
        kinds: Vec<KindOf>,
        team: Arc<RwLock<Team>>,
    ) -> Arc<RwLock<gamelogic::object::Object>> {
        register_test_object_with_name_and_cost(id, name, kinds, team, 0)
    }

    fn register_test_object_with_name_and_cost(
        id: ObjectID,
        name: &str,
        kinds: Vec<KindOf>,
        team: Arc<RwLock<Team>>,
        build_cost: i32,
    ) -> Arc<RwLock<gamelogic::object::Object>> {
        let template: Arc<dyn ThingTemplate> =
            Arc::new(TestThingTemplate::new_with_cost(name, kinds, build_cost));
        let object = Arc::new(RwLock::new(gamelogic::object::Object::new_raw(
            template,
            id,
            LogicObjectStatusMaskType::none(),
            Some(team),
        )));
        object.write().unwrap().set_selectable(true);
        OBJECT_REGISTRY.register_object(id, &object);
        object
    }

    fn set_test_object_position(
        object: &Arc<RwLock<gamelogic::object::Object>>,
        x: Real,
        y: Real,
        z: Real,
    ) {
        let mut geometry = object.read().unwrap().get_geometry_info().clone();
        geometry.position = LogicCoord3D::new(x, y, z);
        object.write().unwrap().set_geometry_info(geometry);
    }

    #[test]
    fn test_command_translator() {
        let _guard = test_state_lock();
        let mut translator = CommandTranslator::new();

        // Test mouse button down
        let down_msg = GameMessage::new(GameMessageType::RawMouseLeftButtonDown(
            ICoord2D { x: 100, y: 50 },
            0,
            1000,
        ));

        let result = translator.translate_game_message(&down_msg);
        assert_eq!(result, GameMessageDisposition::KeepMessage);

        // Test keyboard input
        let key_msg = GameMessage::new(GameMessageType::RawKeyDown(0x53)); // 'S' key
        let result = translator.translate_game_message(&key_msg);
        assert_eq!(result, GameMessageDisposition::KeepMessage);
    }

    #[test]
    fn test_meta_stop_enqueues_do_stop_command() {
        let _guard = test_state_lock();
        get_command_list().write().unwrap().clear_all_commands();

        let mut translator = CommandTranslator::new();
        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MetaStop));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        let messages = get_command_list().read().unwrap().snapshot_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].get_type(), &GameMessageType::DoStop);

        get_command_list().write().unwrap().clear_all_commands();
    }

    #[test]
    fn test_meta_scatter_enqueues_do_scatter_command() {
        let _guard = test_state_lock();
        get_command_list().write().unwrap().clear_all_commands();

        let mut translator = CommandTranslator::new();
        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MetaScatter));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        let messages = get_command_list().read().unwrap().snapshot_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].get_type(), &GameMessageType::DoScatter);

        get_command_list().write().unwrap().clear_all_commands();
    }

    #[test]
    fn test_meta_create_formation_enqueues_create_formation_command() {
        let _guard = test_state_lock();
        get_command_list().write().unwrap().clear_all_commands();

        let mut translator = CommandTranslator::new();
        let disposition = translator
            .translate_game_message(&GameMessage::new(GameMessageType::MetaCreateFormation));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        let messages = get_command_list().read().unwrap().snapshot_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].get_type(),
            &GameMessageType::CreateFormation(Vec::new())
        );

        get_command_list().write().unwrap().clear_all_commands();
    }

    #[test]
    fn test_meta_all_cheer_only_enqueues_in_multiplayer() {
        let _guard = test_state_lock();
        get_command_list().write().unwrap().clear_all_commands();

        get_game_logic()
            .lock()
            .unwrap()
            .set_game_mode(GAME_SINGLE_PLAYER);
        let mut translator = CommandTranslator::new();
        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MetaAllCheer));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        assert!(get_command_list()
            .read()
            .unwrap()
            .snapshot_messages()
            .is_empty());

        get_game_logic().lock().unwrap().set_game_mode(GAME_LAN);
        let mut translator = CommandTranslator::new();
        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MetaAllCheer));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        let messages = get_command_list().read().unwrap().snapshot_messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].get_type(), &GameMessageType::DoCheer);

        get_game_logic()
            .lock()
            .unwrap()
            .set_game_mode(GAME_SINGLE_PLAYER);
        get_command_list().write().unwrap().clear_all_commands();
    }

    #[test]
    fn test_unimplemented_cpp_meta_commands_are_consumed_without_commands() {
        let _guard = test_state_lock();

        for message_type in [
            GameMessageType::MetaDeploy,
            GameMessageType::MetaFollow,
            GameMessageType::MetaChatPlayers,
            GameMessageType::MetaChatAllies,
            GameMessageType::MetaChatEveryone,
        ] {
            get_command_list().write().unwrap().clear_all_commands();
            let mut translator = CommandTranslator::new();

            let disposition = translator.translate_game_message(&GameMessage::new(message_type));

            assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
            assert!(get_command_list()
                .read()
                .unwrap()
                .snapshot_messages()
                .is_empty());
        }
    }

    #[test]
    fn test_selection_translator() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();

        // Test area selection
        let region = IRegion2D {
            x: 10,
            y: 10,
            width: 50,
            height: 50,
        };
        let selection_msg = GameMessage::new(GameMessageType::AreaSelection(region));

        let result = translator.translate_game_message(&selection_msg);
        assert_eq!(result, GameMessageDisposition::KeepMessage);

        // Test control group creation
        let group_msg = GameMessage::new(GameMessageType::MetaCreateTeam(1));
        let result = translator.translate_game_message(&group_msg);
        assert_eq!(result, GameMessageDisposition::KeepMessage);
    }

    #[test]
    fn test_blank_left_selection_region_preserves_selection_for_command_xlat() {
        let _guard = test_state_lock();
        let mut translator = CommandTranslator::new();
        translator.current_selection.insert(7);

        let messages = translator.handle_selection_region(
            &IRegion2D {
                x: 20,
                y: 30,
                width: 0,
                height: 0,
            },
            KeyModifiers::empty(),
        );

        assert!(messages.is_empty());
        assert!(translator.current_selection.contains(&7));
    }

    #[test]
    fn test_gui_command_translator() {
        let _guard = test_state_lock();
        let mut translator = GUICommandTranslator::new();

        // Test control bar toggle
        let toggle_msg = GameMessage::new(GameMessageType::MetaToggleControlBar);
        let result = translator.translate_game_message(&toggle_msg);
        assert_eq!(result, GameMessageDisposition::DestroyMessage);

        // Test diplomacy toggle
        let diplomacy_msg = GameMessage::new(GameMessageType::MetaDiplomacy);
        let result = translator.translate_game_message(&diplomacy_msg);
        assert_eq!(result, GameMessageDisposition::DestroyMessage);

        // Test pass-through message
        let other_msg = GameMessage::new(GameMessageType::Invalid);
        let result = translator.translate_game_message(&other_msg);
        assert_eq!(result, GameMessageDisposition::KeepMessage);
    }

    #[test]
    fn test_gui_command_translator_consumes_pending_non_context_raw_left_input() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command(CommandType::DoAttackMoveTo, CMD_NEED_TARGET_POS, 0);

        let mut translator = GUICommandTranslator::new();
        let down_msg = GameMessage::new(GameMessageType::RawMouseLeftButtonDown(
            ICoord2D { x: 32, y: 48 },
            0,
            1,
        ));
        let up_msg = GameMessage::new(GameMessageType::RawMouseLeftButtonUp(
            ICoord2D { x: 32, y: 48 },
            0,
            2,
        ));

        assert_eq!(
            translator.translate_game_message(&down_msg),
            GameMessageDisposition::DestroyMessage
        );
        assert_eq!(
            translator.translate_game_message(&up_msg),
            GameMessageDisposition::DestroyMessage
        );

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_gui_command_translator_click_executes_and_clears_pending_non_context_command() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            false,
        );
        TheInGameUI::set_pending_command(CommandType::DoAttackMoveTo, CMD_NEED_TARGET_POS, 0);

        let mut translator = GUICommandTranslator::new();
        let click_msg = GameMessage::new(GameMessageType::MouseLeftClick(
            IRegion2D {
                x: 100,
                y: 150,
                width: 0,
                height: 0,
            },
            0,
        ));

        assert_eq!(
            translator.translate_game_message(&click_msg),
            GameMessageDisposition::DestroyMessage
        );
        assert!(TheInGameUI::get_pending_command().is_none());
        assert!(
            TheInGameUI::get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click()
        );
        TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            false,
        );
    }

    #[test]
    fn test_gui_command_translator_click_executes_fire_weapon_pending_command() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            false,
        );
        TheInGameUI::set_pending_command_with_visual(
            CommandType::DoAttackObject,
            CMD_NEED_TARGET_POS,
            0,
            "FORCE_ATTACK_GROUND".to_string(),
            "GENERIC_INVALID".to_string(),
            "ATTACK_DAMAGE_AREA".to_string(),
        );

        let mut translator = GUICommandTranslator::new();
        let click_msg = GameMessage::new(GameMessageType::MouseLeftClick(
            IRegion2D {
                x: 64,
                y: 96,
                width: 0,
                height: 0,
            },
            0,
        ));

        assert_eq!(
            translator.translate_game_message(&click_msg),
            GameMessageDisposition::DestroyMessage
        );
        assert!(TheInGameUI::get_pending_command().is_none());
        assert!(
            TheInGameUI::get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click()
        );
        TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            false,
        );
    }

    #[test]
    fn test_gui_command_translator_click_executes_evacuate_pending_command() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            false,
        );
        TheInGameUI::set_pending_command_with_visual(
            CommandType::Evacuate,
            CMD_NEED_TARGET_POS,
            0,
            "MOVETO".to_string(),
            "GENERIC_INVALID".to_string(),
            "NONE".to_string(),
        );

        let mut translator = GUICommandTranslator::new();
        let click_msg = GameMessage::new(GameMessageType::MouseLeftClick(
            IRegion2D {
                x: 25,
                y: 35,
                width: 0,
                height: 0,
            },
            0,
        ));

        assert_eq!(
            translator.translate_game_message(&click_msg),
            GameMessageDisposition::DestroyMessage
        );
        assert!(TheInGameUI::get_pending_command().is_none());
        assert!(
            TheInGameUI::get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click()
        );
        TheInGameUI::set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
            false,
        );
    }

    #[test]
    fn test_gui_command_translator_respects_context_flag_for_pending_command() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command(
            CommandType::DoAttackMoveTo,
            CMD_NEED_TARGET_POS | CMD_CONTEXTMODE_COMMAND,
            0,
        );

        let mut translator = GUICommandTranslator::new();
        let down_msg = GameMessage::new(GameMessageType::RawMouseLeftButtonDown(
            ICoord2D { x: 10, y: 20 },
            0,
            1,
        ));
        assert_eq!(
            translator.translate_game_message(&down_msg),
            GameMessageDisposition::KeepMessage
        );
        assert!(TheInGameUI::get_pending_command().is_some());

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_gui_command_translator_keeps_context_pending_command_for_command_translator() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command(CommandType::Enter, CMD_NEED_TARGET_ENEMY_OBJECT, 0);

        let mut translator = GUICommandTranslator::new();
        let down_msg = GameMessage::new(GameMessageType::RawMouseLeftButtonDown(
            ICoord2D { x: 10, y: 20 },
            0,
            1,
        ));

        assert_eq!(
            translator.translate_game_message(&down_msg),
            GameMessageDisposition::KeepMessage
        );
        assert!(TheInGameUI::get_pending_command().is_some());

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_hint_spy() {
        let _guard = test_state_lock();
        let mut hint_spy = HintSpy::new();

        assert!(hint_spy.last_hint.is_none());
        TheInGameUI::set_cursor_arrow();
        TheInGameUI::set_radius_cursor_none();

        let cases = [
            (
                GameMessageType::DoMoveToHint(Coord3D::new(1.0, 2.0, 3.0)),
                "Move to",
            ),
            (
                GameMessageType::AddWaypointHint(Coord3D::new(4.0, 5.0, 6.0)),
                "Add waypoint",
            ),
            (
                GameMessageType::DoAttackObjectHint(123),
                "Attack object 123",
            ),
            (
                GameMessageType::DoAttackObjectAfterMovingHint(456),
                "Attack object after moving 456",
            ),
            (GameMessageType::HijackHint(654), "Hijack object 654"),
            (GameMessageType::SabotageHint(987), "Sabotage object 987"),
            (
                GameMessageType::ConvertToCarbombHint(246),
                "Convert to carbomb 246",
            ),
            (
                GameMessageType::CaptureBuildingHint(135),
                "Capture building 135",
            ),
            (
                GameMessageType::SetRallyPointHint(Coord3D::new(9.0, 8.0, 7.0)),
                "Set rally point",
            ),
            (GameMessageType::HackHint(864), "Hack object 864"),
            (GameMessageType::DoRepairHint(789), "Repair object 789"),
            (GameMessageType::GetHealedHint(321), "Get healed 321"),
            (
                GameMessageType::DoSpecialPowerOverrideDestinationHint(Coord3D::new(7.0, 8.0, 9.0)),
                "Special power destination",
            ),
            (GameMessageType::DoInvalidHint, "Invalid action"),
        ];

        for (message_type, expected_text) in cases {
            let message = GameMessage::new(message_type);
            let result = hint_spy.translate_game_message(&message);
            assert_eq!(result, GameMessageDisposition::DestroyMessage);
            assert!(
                hint_spy
                    .last_hint
                    .as_deref()
                    .unwrap_or_default()
                    .contains(expected_text),
                "missing hint text for {:?}",
                message.get_type()
            );
        }

        assert_eq!(TheInGameUI::get_cursor_name(), "GENERIC_INVALID");

        // Test pass-through
        let other_msg = GameMessage::new(GameMessageType::Invalid);
        let result = hint_spy.translate_game_message(&other_msg);
        assert_eq!(result, GameMessageDisposition::KeepMessage);
    }

    #[test]
    fn test_invalid_gui_hint_uses_pending_context_invalid_cursor_and_radius() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command_with_visual(
            CommandType::Enter,
            CMD_NEED_TARGET_ENEMY_OBJECT | CMD_CONTEXTMODE_COMMAND,
            0,
            "ATTACK_OBJECT".to_string(),
            "GENERIC_INVALID".to_string(),
            "GUARD_AREA".to_string(),
        );

        let visual = hint_visual_for_message(&GameMessageType::InvalidGUICommandHint)
            .expect("invalid GUI hint should resolve to a visual");
        assert_eq!(visual.cursor, "GENERIC_INVALID");
        assert!(visual.radius_cursor);

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_invalid_gui_hint_for_non_context_pending_uses_primary_cursor_without_radius() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command_with_visual(
            CommandType::Enter,
            CMD_NEED_TARGET_ENEMY_OBJECT,
            0,
            "ATTACK_OBJECT".to_string(),
            "GENERIC_INVALID".to_string(),
            "NONE".to_string(),
        );

        let visual = hint_visual_for_message(&GameMessageType::InvalidGUICommandHint)
            .expect("invalid GUI hint should resolve to a visual");
        assert_eq!(visual.cursor, "ATTACK_OBJECT");
        assert!(!visual.radius_cursor);

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_valid_gui_hint_empty_radius_type_disables_radius() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command_with_visual(
            CommandType::Enter,
            CMD_NEED_TARGET_ENEMY_OBJECT | CMD_CONTEXTMODE_COMMAND,
            0,
            "ATTACK_OBJECT".to_string(),
            "GENERIC_INVALID".to_string(),
            "   ".to_string(),
        );

        let visual = hint_visual_for_message(&GameMessageType::ValidGUICommandHint)
            .expect("valid GUI hint should resolve to a visual");
        assert_eq!(visual.cursor, "ATTACK_OBJECT");
        assert!(!visual.radius_cursor);

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_valid_gui_hint_unknown_radius_type_disables_radius() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command_with_visual(
            CommandType::Enter,
            CMD_NEED_TARGET_ENEMY_OBJECT | CMD_CONTEXTMODE_COMMAND,
            0,
            "ATTACK_OBJECT".to_string(),
            "GENERIC_INVALID".to_string(),
            "UNKNOWN_RADIUS_TYPE".to_string(),
        );

        let visual = hint_visual_for_message(&GameMessageType::ValidGUICommandHint)
            .expect("valid GUI hint should resolve to a visual");
        assert!(!visual.radius_cursor);

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_invalid_gui_hint_unknown_radius_type_disables_radius() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command_with_visual(
            CommandType::Enter,
            CMD_NEED_TARGET_ENEMY_OBJECT | CMD_CONTEXTMODE_COMMAND,
            0,
            "ATTACK_OBJECT".to_string(),
            "GENERIC_INVALID".to_string(),
            "UNKNOWN_RADIUS_TYPE".to_string(),
        );

        let visual = hint_visual_for_message(&GameMessageType::InvalidGUICommandHint)
            .expect("invalid GUI hint should resolve to a visual");
        assert_eq!(visual.cursor, "GENERIC_INVALID");
        assert!(!visual.radius_cursor);

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_special_power_radius_requires_pending_special_power_payload() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::clear_pending_special_power();
        TheInGameUI::set_pending_command_with_visual(
            CommandType::DoSpecialPowerAtLocation,
            CMD_NEED_TARGET_POS,
            0,
            "PARTICLE_UPLINK_CANNON".to_string(),
            "GENERIC_INVALID".to_string(),
            "PARTICLECANNON".to_string(),
        );

        let without_payload = hint_visual_for_message(&GameMessageType::ValidGUICommandHint)
            .expect("valid GUI hint should resolve to a visual");
        assert!(!without_payload.radius_cursor);

        TheInGameUI::set_pending_special_power(42, CMD_NEED_TARGET_POS, 7);
        let with_payload = hint_visual_for_message(&GameMessageType::ValidGUICommandHint)
            .expect("valid GUI hint should resolve to a visual");
        assert!(with_payload.radius_cursor);

        TheInGameUI::clear_pending_special_power();
        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_non_target_non_context_pending_with_radius_type_disables_radius() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command_with_visual(
            CommandType::DoGuardPosition,
            0,
            0,
            "MOVETO".to_string(),
            "GENERIC_INVALID".to_string(),
            "GUARD_AREA".to_string(),
        );

        let visual = hint_visual_for_message(&GameMessageType::ValidGUICommandHint)
            .expect("valid GUI hint should resolve to a visual");
        assert!(!visual.radius_cursor);

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_valid_and_invalid_gui_hint_share_radius_decision() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::clear_pending_special_power();
        TheInGameUI::set_pending_command_with_visual(
            CommandType::DoSpecialPowerAtLocation,
            CMD_NEED_TARGET_POS,
            0,
            "PARTICLE_UPLINK_CANNON".to_string(),
            "GENERIC_INVALID".to_string(),
            "PARTICLECANNON".to_string(),
        );

        let valid_without = hint_visual_for_message(&GameMessageType::ValidGUICommandHint)
            .expect("valid GUI hint should resolve to a visual");
        let invalid_without = hint_visual_for_message(&GameMessageType::InvalidGUICommandHint)
            .expect("invalid GUI hint should resolve to a visual");
        assert_eq!(valid_without.radius_cursor, invalid_without.radius_cursor);
        assert!(!valid_without.radius_cursor);

        TheInGameUI::set_pending_special_power(99, CMD_NEED_TARGET_POS, 3);
        let valid_with = hint_visual_for_message(&GameMessageType::ValidGUICommandHint)
            .expect("valid GUI hint should resolve to a visual");
        let invalid_with = hint_visual_for_message(&GameMessageType::InvalidGUICommandHint)
            .expect("invalid GUI hint should resolve to a visual");
        assert_eq!(valid_with.radius_cursor, invalid_with.radius_cursor);
        assert!(valid_with.radius_cursor);

        TheInGameUI::clear_pending_special_power();
        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_gui_hint_does_not_force_radius_from_pending_special_power_alone() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::clear_pending_special_power();
        TheInGameUI::set_pending_special_power(99, CMD_NEED_TARGET_POS, 0);

        let valid = hint_visual_for_message(&GameMessageType::ValidGUICommandHint)
            .expect("valid GUI hint should resolve to a visual");
        assert_eq!(valid.cursor, "CROSS");
        assert!(!valid.radius_cursor);

        let invalid = hint_visual_for_message(&GameMessageType::InvalidGUICommandHint)
            .expect("invalid GUI hint should resolve to a visual");
        assert_eq!(invalid.cursor, "GENERIC_INVALID");
        assert!(!invalid.radius_cursor);

        TheInGameUI::clear_pending_special_power();
    }

    #[test]
    fn test_translator_factory() {
        let _guard = test_state_lock();
        let factory = TranslatorFactory::new();

        // Test individual translator creation
        let command_translator = TranslatorFactory::create_command_translator();
        assert!(command_translator.read().is_ok());

        let selection_translator = TranslatorFactory::create_selection_translator();
        assert!(selection_translator.read().is_ok());

        let window_translator = TranslatorFactory::create_window_translator();
        assert!(window_translator.read().is_ok());

        let meta_translator = TranslatorFactory::create_meta_event_translator();
        assert!(meta_translator.read().is_ok());

        let gui_translator = TranslatorFactory::create_gui_command_translator();
        assert!(gui_translator.read().is_ok());

        let look_at = TranslatorFactory::create_look_at_translator();
        assert!(look_at.read().is_ok());

        let hot_key = TranslatorFactory::create_hot_key_translator();
        assert!(hot_key.read().is_ok());

        let place_event = TranslatorFactory::create_place_event_translator();
        assert!(place_event.read().is_ok());

        let hint_spy = TranslatorFactory::create_hint_spy();
        assert!(hint_spy.read().is_ok());

        // Test standard translator set
        let standard_set = TranslatorFactory::create_standard_translator_set();
        assert_eq!(standard_set.len(), 9);

        // Verify priorities are in ascending order
        let priorities: Vec<u32> = standard_set.iter().map(|(_, p)| *p).collect();
        assert_eq!(priorities, vec![10, 20, 25, 30, 40, 50, 60, 70, 100]);
    }

    #[test]
    fn test_command_translator_modes() {
        let _guard = test_state_lock();
        let mut translator = CommandTranslator::new();

        // Test force attack mode
        assert!(!translator.force_attack_mode);

        let alt_down = GameMessage::new(GameMessageType::RawKeyDown(0x12)); // Alt key
        translator.translate_game_message(&alt_down);
        assert!(translator.force_attack_mode);

        let alt_up = GameMessage::new(GameMessageType::RawKeyUp(0x12));
        translator.translate_game_message(&alt_up);
        assert!(!translator.force_attack_mode);

        // Test prefer selection mode
        assert!(!translator.prefer_selection_mode);

        let ctrl_down = GameMessage::new(GameMessageType::RawKeyDown(0x11)); // Ctrl key
        translator.translate_game_message(&ctrl_down);
        assert!(translator.prefer_selection_mode);

        let ctrl_up = GameMessage::new(GameMessageType::RawKeyUp(0x11));
        translator.translate_game_message(&ctrl_up);
        assert!(!translator.prefer_selection_mode);
    }

    #[test]
    fn test_selection_translator_groups() {
        let _guard = test_state_lock();
        let mut translator = SelectionTranslator::new();

        // Simulate having selected objects
        translator.selected_objects.insert(100);
        translator.selected_objects.insert(101);

        // Create control group
        let create_msg = GameMessage::new(GameMessageType::MetaCreateTeam(1));
        translator.translate_game_message(&create_msg);

        assert!(translator.control_groups.contains_key(&1));
        assert_eq!(translator.control_groups[&1].len(), 2);

        // Clear current selection
        translator.selected_objects.clear();
        assert_eq!(translator.selected_objects.len(), 0);

        // Select control group
        let select_msg = GameMessage::new(GameMessageType::MetaSelectTeam(1));
        translator.translate_game_message(&select_msg);

        assert_eq!(translator.selected_objects.len(), 2);
        assert_eq!(translator.last_selected_group, Some(1));
    }

    #[test]
    fn test_live_selection_view_team_centers_on_last_object_without_selecting() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();

        let template: Arc<dyn gamelogic::thing_template::ThingTemplate> = Arc::new(
            gamelogic::thing_template::DefaultThingTemplate::new("ControlGroupUnit".to_string()),
        );
        let first = Arc::new(RwLock::new(gamelogic::object::Object::new_raw(
            Arc::clone(&template),
            200,
            LogicObjectStatusMaskType::none(),
            None,
        )));
        let second = Arc::new(RwLock::new(gamelogic::object::Object::new_raw(
            template,
            201,
            LogicObjectStatusMaskType::none(),
            None,
        )));
        {
            let mut first_guard = first.write().unwrap();
            first_guard
                .set_position(&LogicCoord3D::new(100.0, 200.0, 0.0))
                .unwrap();
        }
        {
            let mut second_guard = second.write().unwrap();
            second_guard
                .set_position(&LogicCoord3D::new(320.0, 460.0, 0.0))
                .unwrap();
        }
        OBJECT_REGISTRY.register_object(200, &first);
        OBJECT_REGISTRY.register_object(201, &second);

        with_tactical_view(|view| {
            view.set_width(120);
            view.set_height(90);
            view.set_position(&Point3::new(0.0, 0.0, 0.0));
        });

        let mut translator = SelectionTranslator::new();
        translator.control_groups.insert(3, vec![200, 201]);
        translator.selected_objects.insert(999);

        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MetaViewTeam(3)));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        assert_eq!(translator.selected_objects, HashSet::from([999]));
        assert_eq!(translator.last_selected_group, None);
        with_tactical_view_ref(|view| {
            assert_eq!(view.position().x, 260.0);
            assert_eq!(view.position().y, 415.0);
        });

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_select_all_filters_cpp_disqualified_units() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();
        let team = setup_local_player_team();

        let _unit = register_test_object(
            301,
            vec![KindOf::Unit, KindOf::Vehicle, KindOf::Selectable],
            Arc::clone(&team),
        );
        let _dozer = register_test_object(
            302,
            vec![
                KindOf::Unit,
                KindOf::Vehicle,
                KindOf::Selectable,
                KindOf::Dozer,
            ],
            Arc::clone(&team),
        );
        let _structure = register_test_object(
            303,
            vec![
                KindOf::Structure,
                KindOf::Building,
                KindOf::Selectable,
                KindOf::Immobile,
            ],
            Arc::clone(&team),
        );
        let _ignored_aircraft = register_test_object(
            304,
            vec![
                KindOf::Unit,
                KindOf::Aircraft,
                KindOf::Selectable,
                KindOf::IgnoresSelectAll,
            ],
            Arc::clone(&team),
        );

        let mut translator = SelectionTranslator::new();
        translator.selected_objects.insert(302);
        translator.selected_objects.insert(303);

        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MetaSelectAll));

        assert_eq!(disposition, GameMessageDisposition::KeepMessage);
        assert_eq!(translator.selected_objects, HashSet::from([301]));

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_select_all_aircraft_requires_aircraft_and_filters_disqualified_units() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();
        let team = setup_local_player_team();

        let _ground_unit = register_test_object(
            401,
            vec![KindOf::Unit, KindOf::Vehicle, KindOf::Selectable],
            Arc::clone(&team),
        );
        let _aircraft = register_test_object(
            402,
            vec![KindOf::Unit, KindOf::Aircraft, KindOf::Selectable],
            Arc::clone(&team),
        );
        let _harvester_aircraft = register_test_object(
            403,
            vec![
                KindOf::Unit,
                KindOf::Aircraft,
                KindOf::Selectable,
                KindOf::Harvester,
            ],
            Arc::clone(&team),
        );

        let mut translator = SelectionTranslator::new();
        translator.selected_objects.insert(401);

        let disposition = translator
            .translate_game_message(&GameMessage::new(GameMessageType::MetaSelectAllAircraft));

        assert_eq!(disposition, GameMessageDisposition::KeepMessage);
        assert_eq!(translator.selected_objects, HashSet::from([402]));

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_select_matching_units_uses_selected_local_templates() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();
        let team = setup_local_player_team();

        let selected = register_test_object_with_name(
            451,
            "TankTemplate",
            vec![KindOf::Unit, KindOf::Vehicle, KindOf::Selectable],
            Arc::clone(&team),
        );
        let matching = register_test_object_with_name(
            452,
            "TankTemplate",
            vec![KindOf::Unit, KindOf::Vehicle, KindOf::Selectable],
            Arc::clone(&team),
        );
        let different = register_test_object_with_name(
            453,
            "DozerTemplate",
            vec![KindOf::Unit, KindOf::Vehicle, KindOf::Selectable],
            Arc::clone(&team),
        );
        set_test_object_position(&selected, 10.0, 10.0, 0.0);
        set_test_object_position(&matching, 20.0, 20.0, 0.0);
        set_test_object_position(&different, 25.0, 25.0, 0.0);

        let mut translator = SelectionTranslator::new();
        translator.selected_objects.insert(451);

        assert_eq!(
            translator.collect_matching_selection(Some(&IRegion2D {
                x: 0,
                y: 0,
                width: 40,
                height: 40,
            })),
            vec![451, 452]
        );

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_select_matching_units_falls_back_to_map_and_adds_selection() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();
        let team = setup_local_player_team();

        let selected = register_test_object_with_name(
            461,
            "RifleTemplate",
            vec![KindOf::Unit, KindOf::Infantry, KindOf::Selectable],
            Arc::clone(&team),
        );
        let matching = register_test_object_with_name(
            462,
            "RifleTemplate",
            vec![KindOf::Unit, KindOf::Infantry, KindOf::Selectable],
            Arc::clone(&team),
        );
        set_test_object_position(&selected, 5000.0, 5000.0, 0.0);
        set_test_object_position(&matching, 5050.0, 5050.0, 0.0);

        with_tactical_view(|view| {
            view.set_width(1);
            view.set_height(1);
            view.set_position(&Point3::new(0.0, 0.0, 0.0));
        });

        let mut translator = SelectionTranslator::new();
        translator.selected_objects.insert(461);

        let disposition = translator
            .translate_game_message(&GameMessage::new(GameMessageType::MetaSelectMatchingUnits));

        assert_eq!(disposition, GameMessageDisposition::KeepMessage);
        assert_eq!(translator.selected_objects, HashSet::from([461, 462]));

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_select_next_prev_unit_cycles_local_mobile_units() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();
        let team = setup_local_player_team();

        let first = register_test_object(
            501,
            vec![KindOf::Unit, KindOf::Vehicle, KindOf::Selectable],
            Arc::clone(&team),
        );
        let second = register_test_object(
            502,
            vec![KindOf::Unit, KindOf::Vehicle, KindOf::Selectable],
            Arc::clone(&team),
        );
        let _no_select = register_test_object(
            503,
            vec![
                KindOf::Unit,
                KindOf::Vehicle,
                KindOf::Selectable,
                KindOf::NoSelect,
            ],
            Arc::clone(&team),
        );
        set_test_object_position(&first, 10.0, 20.0, 5.0);
        set_test_object_position(&second, 30.0, 40.0, 7.0);

        let mut translator = SelectionTranslator::new();
        let disposition = translator
            .translate_game_message(&GameMessage::new(GameMessageType::MetaSelectNextUnit));

        assert_eq!(disposition, GameMessageDisposition::KeepMessage);
        assert_eq!(translator.selected_objects, HashSet::from([502]));

        translator.translate_game_message(&GameMessage::new(GameMessageType::MetaSelectPrevUnit));

        assert_eq!(translator.selected_objects, HashSet::from([501]));

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_select_worker_cycles_only_dozer_objects() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();
        let team = setup_local_player_team();

        let _unit = register_test_object(
            601,
            vec![KindOf::Unit, KindOf::Vehicle, KindOf::Selectable],
            Arc::clone(&team),
        );
        let _worker_a = register_test_object(
            602,
            vec![
                KindOf::Unit,
                KindOf::Vehicle,
                KindOf::Selectable,
                KindOf::Dozer,
            ],
            Arc::clone(&team),
        );
        let _worker_b = register_test_object(
            603,
            vec![
                KindOf::Unit,
                KindOf::Vehicle,
                KindOf::Selectable,
                KindOf::Dozer,
            ],
            Arc::clone(&team),
        );

        let mut translator = SelectionTranslator::new();
        translator.selected_objects.insert(602);

        let disposition = translator
            .translate_game_message(&GameMessage::new(GameMessageType::MetaSelectPrevWorker));

        assert_eq!(disposition, GameMessageDisposition::KeepMessage);
        assert_eq!(translator.selected_objects, HashSet::from([603]));

        translator.translate_game_message(&GameMessage::new(GameMessageType::MetaSelectNextWorker));

        assert_eq!(translator.selected_objects, HashSet::from([602]));

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_select_hero_selects_local_hero_and_recenters_view() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();
        let team = setup_local_player_team();

        let hero = register_test_object(
            701,
            vec![
                KindOf::Unit,
                KindOf::Infantry,
                KindOf::Selectable,
                KindOf::Hero,
            ],
            Arc::clone(&team),
        );
        let _regular_unit = register_test_object(
            702,
            vec![KindOf::Unit, KindOf::Vehicle, KindOf::Selectable],
            Arc::clone(&team),
        );
        set_test_object_position(&hero, 700.0, 740.0, 12.0);
        with_tactical_view(|view| view.set_position(&Point3::new(0.0, 0.0, 0.0)));

        let mut translator = SelectionTranslator::new();
        let disposition =
            translator.translate_game_message(&GameMessage::new(GameMessageType::MetaSelectHero));

        assert_eq!(disposition, GameMessageDisposition::KeepMessage);
        assert_eq!(translator.selected_objects, HashSet::from([701]));
        with_tactical_view_ref(|view| {
            assert_eq!(view.position().x, 700.0 - view.width() as f32 * 0.5);
            assert_eq!(view.position().y, 740.0 - view.height() as f32 * 0.5);
        });

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_view_command_center_prefers_command_center_over_costly_structure() {
        let _guard = test_state_lock();
        OBJECT_REGISTRY.clear();
        let team = setup_local_player_team();

        let _expensive_structure = register_test_object_with_cost(
            801,
            vec![KindOf::Structure, KindOf::Building, KindOf::Selectable],
            Arc::clone(&team),
            5000,
        );
        let command_center = register_test_object_with_cost(
            802,
            vec![
                KindOf::Structure,
                KindOf::Building,
                KindOf::CommandCenter,
                KindOf::Selectable,
            ],
            Arc::clone(&team),
            1000,
        );
        set_test_object_position(&command_center, 900.0, 960.0, 0.0);
        with_tactical_view(|view| view.set_position(&Point3::new(0.0, 0.0, 0.0)));

        let mut translator = SelectionTranslator::new();
        translator.selected_objects.insert(801);
        let disposition = translator
            .translate_game_message(&GameMessage::new(GameMessageType::MetaViewCommandCenter));

        assert_eq!(disposition, GameMessageDisposition::DestroyMessage);
        assert_eq!(translator.selected_objects, HashSet::from([801]));
        with_tactical_view_ref(|view| {
            assert_eq!(view.position().x, 900.0 - view.width() as f32 * 0.5);
            assert_eq!(view.position().y, 960.0 - view.height() as f32 * 0.5);
        });

        OBJECT_REGISTRY.clear();
    }

    #[test]
    fn test_pending_command_for_beacon_position() {
        let _guard = test_state_lock();
        let position = Coord3D::new(123.0, 456.0, 7.0);
        let place = PendingCommand {
            command_type: CommandType::PlaceBeacon,
            options: 0x20,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        let remove = PendingCommand {
            command_type: CommandType::RemoveBeacon,
            options: 0x20,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };

        assert!(matches!(
            pending_command_for_position(&place, position.clone(), None),
            Some(GameMessageType::PlaceBeacon(_))
        ));
        assert!(matches!(
            pending_command_for_position(&remove, position, None),
            Some(GameMessageType::RemoveBeacon(_))
        ));
    }

    #[test]
    fn test_pending_command_for_evacuate_position_emits_location_payload() {
        let _guard = test_state_lock();
        let target = Coord3D::new(11.0, 22.0, 0.0);
        let evac_need_pos = PendingCommand {
            command_type: CommandType::Evacuate,
            options: CMD_NEED_TARGET_POS,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        let evac_no_pos = PendingCommand {
            command_type: CommandType::Evacuate,
            options: 0,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };

        assert_eq!(
            pending_command_for_position(&evac_need_pos, target.clone(), None),
            Some(GameMessageType::EvacuateAtLocation(target))
        );
        assert_eq!(
            pending_command_for_position(&evac_no_pos, Coord3D::new(1.0, 2.0, 0.0), None),
            Some(GameMessageType::Evacuate)
        );
    }

    #[test]
    fn test_pending_command_maps_special_power_and_combatdrop_variants() {
        let _guard = test_state_lock();
        let pos = Coord3D::new(50.0, 60.0, 0.0);
        let target = 222;

        TheInGameUI::clear_pending_special_power();
        TheInGameUI::set_pending_special_power(17, CMD_NEED_TARGET_POS, 88);

        let special_obj = PendingCommand {
            command_type: CommandType::DoSpecialPowerAtObject,
            options: CMD_NEED_TARGET_ENEMY_OBJECT,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        let special_pos = PendingCommand {
            command_type: CommandType::DoSpecialPowerAtLocation,
            options: CMD_NEED_TARGET_POS,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        let combat_obj = PendingCommand {
            command_type: CommandType::CombatDropAtObject,
            options: CMD_NEED_TARGET_ENEMY_OBJECT,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        let combat_pos = PendingCommand {
            command_type: CommandType::CombatDropAtLocation,
            options: CMD_NEED_TARGET_POS,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        let fire_obj = PendingCommand {
            command_type: CommandType::DoAttackObject,
            options: CMD_NEED_TARGET_ENEMY_OBJECT,
            source_object_id: 1,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        let fire_pos = PendingCommand {
            command_type: CommandType::DoAttackObject,
            options: CMD_NEED_TARGET_ENEMY_OBJECT | CMD_ATTACK_OBJECTS_POSITION,
            source_object_id: 2,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };

        assert_eq!(
            pending_command_for_object(&special_obj, target),
            Some(GameMessageType::DoSpecialPowerAtObject(
                17,
                target,
                CMD_NEED_TARGET_POS,
                88
            ))
        );
        assert_eq!(
            pending_command_for_position(&special_pos, pos.clone(), None),
            Some(GameMessageType::DoSpecialPowerAtLocation(
                17,
                pos.clone(),
                -1.0,
                gamelogic::common::INVALID_ID,
                CMD_NEED_TARGET_POS,
                88,
            ))
        );
        assert_eq!(
            pending_command_for_position(&special_pos, pos.clone(), Some(target)),
            Some(GameMessageType::DoSpecialPowerAtLocation(
                17,
                pos.clone(),
                -1.0,
                target,
                CMD_NEED_TARGET_POS,
                88,
            ))
        );
        assert_eq!(
            pending_command_for_object(&combat_obj, target),
            Some(GameMessageType::CombatDropAtObject(target))
        );
        assert_eq!(
            pending_command_for_position(&combat_pos, pos.clone(), None),
            Some(GameMessageType::CombatDropAtLocation(pos))
        );
        assert_eq!(
            pending_command_for_object(&fire_obj, target),
            Some(GameMessageType::DoWeaponAtObject(1, target))
        );
        assert_eq!(pending_command_for_object(&fire_pos, target), None);
        assert_eq!(
            pending_command_for_position(&fire_pos, Coord3D::new(7.0, 8.0, 0.0), Some(target)),
            Some(GameMessageType::DoWeaponAtLocation(
                2,
                Coord3D::new(7.0, 8.0, 0.0)
            ))
        );

        TheInGameUI::clear_pending_special_power();
    }

    #[test]
    fn test_pending_special_power_hover_uses_valid_gui_hint() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::clear_pending_special_power();
        TheInGameUI::set_pending_command(
            CommandType::DoSpecialPowerAtLocation,
            CMD_NEED_TARGET_POS,
            0,
        );
        TheInGameUI::set_pending_special_power(42, CMD_NEED_TARGET_POS, 9);

        let mut translator = CommandTranslator::new();
        translator.current_selection.insert(1);
        let hints = translator.handle_mouseover_location_hint(&Coord3D::new(1.0, 2.0, 0.0));
        assert_eq!(hints, vec![GameMessageType::ValidGUICommandHint]);

        TheInGameUI::clear_pending_command();
        TheInGameUI::clear_pending_special_power();
    }

    #[test]
    fn test_pending_command_helper_masks_and_object_mapping() {
        let _guard = test_state_lock();
        assert!(pending_command_accepts_object(CMD_NEED_TARGET_ENEMY_OBJECT));
        assert!(pending_command_accepts_position(CMD_NEED_TARGET_POS));
        assert!(!pending_command_accepts_object(CMD_NEED_TARGET_POS));
        assert!(!pending_command_accepts_position(
            CMD_NEED_TARGET_ENEMY_OBJECT
        ));

        let pending = PendingCommand {
            command_type: CommandType::Dock,
            options: 0,
            source_object_id: 99,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        assert!(matches!(
            pending_command_for_object(&pending, 321),
            Some(GameMessageType::Dock(321))
        ));
    }

    #[test]
    fn test_pending_command_click_falls_back_to_invalid_gui_hint_when_unresolved() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command(CommandType::Enter, CMD_NEED_TARGET_ENEMY_OBJECT, 0);

        let mut translator = CommandTranslator::new();
        let result = translator.resolve_pending_command_click(
            0,
            Some(0),
            None,
            &Coord3D::new(0.0, 0.0, 0.0),
        );

        assert_eq!(result, vec![GameMessageType::InvalidGUICommandHint]);

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_pending_set_rally_point_fans_out_to_all_selected_sources() {
        let _guard = test_state_lock();
        let pending = PendingCommand {
            command_type: CommandType::SetRallyPoint,
            options: CMD_NEED_TARGET_POS,
            source_object_id: 0,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        let mut selection = HashSet::new();
        selection.insert(7);
        selection.insert(3);
        let position = Coord3D::new(11.0, 22.0, 0.0);

        let messages =
            pending_command_messages_for_position(&pending, position.clone(), &selection, None);
        assert_eq!(
            messages,
            vec![
                GameMessageType::SetRallyPoint(3, position.clone()),
                GameMessageType::SetRallyPoint(7, position),
            ]
        );
    }

    #[test]
    fn test_point_click_is_actionable_matches_cpp_gating() {
        let _guard = test_state_lock();
        assert!(point_click_is_actionable(false, false, false));
        assert!(!point_click_is_actionable(false, true, false));
        assert!(point_click_is_actionable(false, true, true));
        assert!(!point_click_is_actionable(true, false, false));
        assert!(point_click_is_actionable(true, true, false));
        assert!(point_click_is_actionable(true, false, true));
    }

    #[test]
    fn test_raw_right_button_up_does_not_issue_commands() {
        let _guard = test_state_lock();
        let mut translator = CommandTranslator::new();
        translator.current_selection.insert(42);
        translator.force_attack_mode = true;

        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command(CommandType::Enter, CMD_NEED_TARGET_ENEMY_OBJECT, 0);
        let messages =
            translator.handle_mouse_button_up(&ICoord2D::new(50, 75), MouseButton::Right, 0, 2);
        assert!(messages.is_empty());
        assert!(TheInGameUI::get_pending_command().is_some());
        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_raw_right_button_up_click_clears_pending_build_placement() {
        let _guard = test_state_lock();
        let mut translator = CommandTranslator::new();

        TheInGameUI::place_build_available(Some("TestStructure".to_string()), Some(77));
        assert_eq!(TheInGameUI::get_pending_place_source_object_id(), 77);

        let down = GameMessage::new(GameMessageType::RawMouseRightButtonDown(
            ICoord2D::new(10, 20),
            0,
            100,
        ));
        let up = GameMessage::new(GameMessageType::RawMouseRightButtonUp(
            ICoord2D::new(10, 20),
            0,
            120,
        ));

        assert_eq!(
            translator.translate_game_message(&down),
            GameMessageDisposition::KeepMessage
        );
        assert_eq!(
            translator.translate_game_message(&up),
            GameMessageDisposition::KeepMessage
        );
        assert_eq!(TheInGameUI::get_pending_place_source_object_id(), 0);
        assert!(TheInGameUI::get_pending_place_template().is_none());
    }

    #[test]
    fn test_raw_right_button_up_regular_mouse_deselects_current_selection() {
        let _guard = test_state_lock();
        game_engine::common::ini::ini_game_data::init_global_data();
        let previous_alt_mouse = get_global_data()
            .map(|data| data.read().use_alternate_mouse)
            .unwrap_or(false);
        if let Some(data) = get_global_data() {
            data.write().use_alternate_mouse = false;
        }

        let mut translator = CommandTranslator::new();
        translator.current_selection.insert(42);
        TheInGameUI::clear_pending_command();
        TheInGameUI::place_build_available(None, None);

        let down = GameMessage::new(GameMessageType::RawMouseRightButtonDown(
            ICoord2D::new(10, 20),
            0,
            100,
        ));
        let up = GameMessage::new(GameMessageType::RawMouseRightButtonUp(
            ICoord2D::new(10, 20),
            0,
            120,
        ));

        assert_eq!(
            translator.translate_game_message(&down),
            GameMessageDisposition::KeepMessage
        );
        assert_eq!(
            translator.translate_game_message(&up),
            GameMessageDisposition::KeepMessage
        );
        assert!(translator.current_selection.is_empty());

        if let Some(data) = get_global_data() {
            data.write().use_alternate_mouse = previous_alt_mouse;
        }
    }

    #[test]
    fn test_command_translator_keeps_raw_right_down_up_for_cpp_forwarding_parity() {
        let _guard = test_state_lock();
        let mut translator = CommandTranslator::new();

        let down = GameMessage::new(GameMessageType::RawMouseRightButtonDown(
            ICoord2D::new(30, 40),
            0,
            10,
        ));
        let up = GameMessage::new(GameMessageType::RawMouseRightButtonUp(
            ICoord2D::new(30, 40),
            0,
            20,
        ));

        assert_eq!(
            translator.translate_game_message(&down),
            GameMessageDisposition::KeepMessage
        );
        assert_eq!(
            translator.translate_game_message(&up),
            GameMessageDisposition::KeepMessage
        );
    }

    #[test]
    fn test_pending_object_command_hovering_location_returns_invalid_gui_hint() {
        let _guard = test_state_lock();
        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command(CommandType::Enter, CMD_NEED_TARGET_ENEMY_OBJECT, 0);

        let mut translator = CommandTranslator::new();
        translator.current_selection.insert(1);
        let hints = translator.handle_mouseover_location_hint(&Coord3D::new(10.0, 20.0, 0.0));
        assert_eq!(hints, vec![GameMessageType::InvalidGUICommandHint]);

        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_right_click_alt_mouse_does_not_execute_pending_position_command() {
        let _guard = test_state_lock();
        game_engine::common::ini::ini_game_data::init_global_data();
        let previous_alt_mouse = get_global_data()
            .map(|data| data.read().use_alternate_mouse)
            .unwrap_or(false);
        if let Some(data) = get_global_data() {
            data.write().use_alternate_mouse = true;
        }

        TheInGameUI::clear_pending_command();
        TheInGameUI::set_pending_command(CommandType::DoAttackMoveTo, CMD_NEED_TARGET_POS, 0);

        let mut translator = CommandTranslator::new();
        let region = IRegion2D {
            x: 10,
            y: 20,
            width: 0,
            height: 0,
        };
        let messages = translator.handle_point_click(&region, true);
        assert!(messages.is_empty());
        assert!(TheInGameUI::get_pending_command().is_some());

        if let Some(data) = get_global_data() {
            data.write().use_alternate_mouse = previous_alt_mouse;
        }
        TheInGameUI::clear_pending_command();
    }

    #[test]
    fn test_double_click_guard_command_gated_by_mouse_mode() {
        let _guard = test_state_lock();
        game_engine::common::ini::ini_game_data::init_global_data();
        let (previous_alt_mouse, previous_double_click_attack_move) = get_global_data()
            .map(|data| {
                let data = data.read();
                (data.use_alternate_mouse, data.double_click_attack_move)
            })
            .unwrap_or((false, false));

        if let Some(data) = get_global_data() {
            let mut data = data.write();
            data.use_alternate_mouse = true;
            data.double_click_attack_move = true;
        }

        let translator = CommandTranslator::new();
        let region = IRegion2D {
            x: 4,
            y: 6,
            width: 0,
            height: 0,
        };

        let right = translator.try_double_click_guard_command(&region, true);
        assert!(matches!(
            right,
            Some(GameMessageType::DoGuardPosition(_, 0))
        ));
        let left = translator.try_double_click_guard_command(&region, false);
        assert!(left.is_none());

        if let Some(data) = get_global_data() {
            let mut data = data.write();
            data.use_alternate_mouse = previous_alt_mouse;
            data.double_click_attack_move = previous_double_click_attack_move;
        }
    }
}
