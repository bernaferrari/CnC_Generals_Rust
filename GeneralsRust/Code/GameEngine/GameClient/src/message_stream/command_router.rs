/*
**  Command & Conquer Generals Zero Hour(tm)
**  Routes processed GameMessages into the GameLogic command system.
*/

use glam::{IVec2, Vec3};
use log::warn;
use thiserror::Error;

use gamelogic::commands::command::{Command, CommandType};
use gamelogic::commands::command_queue::{
    get_command_queue_manager, CommandPriority, QueuedCommand,
};
use gamelogic::common::{
    AsciiString, Coord3D as LogicCoord3D, IRegion2D as LogicRegion2D, Int, ObjectID,
};

use super::game_message::{
    Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, IRegion2D,
};

/// Errors that can occur while routing commands to the legacy command queue.
#[derive(Debug, Error)]
pub enum CommandRoutingError {
    #[error("Command queue lock poisoned")]
    QueueLock,
    #[error("Failed to queue command: {0}")]
    QueueError(String),
}

/// Convert the supplied messages into GameLogic commands and queue them for execution.
pub fn route_commands_to_gamelogic(
    messages: Vec<GameMessage>,
    current_frame: u32,
) -> Result<usize, CommandRoutingError> {
    let mut pending = Vec::with_capacity(messages.len());

    for message in messages {
        let message_type = message.get_type().clone();
        if let Some(command) = convert_game_message(&message) {
            pending.push((command, message_type, message.get_player_index()));
        } else {
            warn!("No GameLogic mapping for message {:?}", message.get_type());
        }
    }

    if pending.is_empty() {
        return Ok(0);
    }

    let queue_manager = get_command_queue_manager();
    let mut manager = queue_manager
        .lock()
        .map_err(|_| CommandRoutingError::QueueLock)?;

    let mut routed = 0;
    for (command, message_type, player_id) in pending {
        let priority = determine_priority(&message_type);
        let queued = QueuedCommand::new(command, priority, current_frame);
        manager
            .queue_player_command(player_id, queued)
            .map_err(|err| CommandRoutingError::QueueError(err.to_string()))?;
        routed += 1;
    }

    Ok(routed)
}

fn determine_priority(message_type: &GameMessageType) -> CommandPriority {
    use GameMessageType::*;

    match message_type {
        DoStop | DoScatter => CommandPriority::Critical,
        DoAttackObject(_)
        | DoForceAttackObject(_)
        | DoForceAttackGround(_)
        | DoGuardObject(_, _)
        | DoGuardPosition(_, _)
        | DoSpecialPower(_, _, _)
        | DoSpecialPowerAtObject(_, _, _, _)
        | DoSpecialPowerAtLocation(_, _, _, _, _, _)
        | DoWeaponAtLocation(_, _)
        | DoWeaponAtObject(_, _)
        | ConvertToCarbomb(_, _)
        | CaptureBuilding(_, _)
        | DisableVehicleHack(_, _)
        | DisableBuildingHack(_, _)
        | StealCashHack(_, _)
        | SnipeVehicle(_, _) => CommandPriority::High,
        AreaSelection(_)
        | CreateSelectedGroup(_, _)
        | CreateSelectedGroupNoSound(_, _)
        | DestroySelectedGroup(_)
        | RemoveFromSelectedGroup(_)
        | CreateTeamSlot(_)
        | SelectTeamSlot(_)
        | AddTeamSlot(_) => CommandPriority::Low,
        _ => CommandPriority::Normal,
    }
}

fn convert_game_message(message: &GameMessage) -> Option<Command> {
    use GameMessageType::*;

    let player = message.get_player_index();

    match message.get_type() {
        ClearGameData => Some(basic_command(CommandType::ClearGameData, player)),
        NewGame => {
            let mut command = basic_command(CommandType::NewGame, player);
            append_integer_message_arguments(message, &mut command);
            Some(command)
        }
        CreateSelectedGroup(create_new, objects) => {
            let mut command = Command::new(CommandType::CreateSelectedGroup);
            command.set_player_index(player);
            command.append_boolean_argument(*create_new);
            for object_id in objects {
                command.append_object_id_argument(*object_id);
            }
            Some(command)
        }
        CreateSelectedGroupNoSound(create_new, objects) => {
            let mut command = Command::new(CommandType::CreateSelectedGroupNoSound);
            command.set_player_index(player);
            command.append_boolean_argument(*create_new);
            for object_id in objects {
                command.append_object_id_argument(*object_id);
            }
            Some(command)
        }
        DestroySelectedGroup(_team_id) => {
            Some(basic_command(CommandType::DestroySelectedGroup, player))
        }
        RemoveFromSelectedGroup(objects) => {
            let mut command = Command::new(CommandType::RemoveFromSelectedGroup);
            command.set_player_index(player);
            for object_id in objects {
                command.append_object_id_argument(*object_id);
            }
            Some(command)
        }
        Exit(unit) => Some(single_object_command(CommandType::Exit, *unit, player)),
        Evacuate => Some(basic_command(CommandType::Evacuate, player)),
        ExecuteRailedTransport => Some(basic_command(CommandType::ExecuteRailedTransport, player)),
        DoAttackSquad(units) => Some(list_object_command(
            CommandType::DoAttackSquad,
            units,
            player,
        )),
        DoMoveTo(coord) => {
            let mut command = Command::new(CommandType::DoMoveTo);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        DoAttackMoveTo(coord) => {
            let mut command = Command::new(CommandType::DoAttackMoveTo);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        DoForceMoveTO(coord) => {
            let mut command = Command::new(CommandType::DoForceMoveTo);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        AddWaypoint(coord) => {
            let mut command = Command::new(CommandType::AddWaypoint);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        DoForceAttackGround(coord) => {
            let mut command = Command::new(CommandType::DoForceAttackGround);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        DoAttackObject(target) => {
            // C++ MSG_DO_ATTACK_OBJECT carries only the enemy object id and applies to the
            // currently selected group.
            let mut command = Command::new(CommandType::DoAttackObject);
            command.set_player_index(player);
            command.append_object_id_argument(*target);
            Some(command)
        }
        DoForceAttackObject(target) => {
            // C++ MSG_DO_FORCE_ATTACK_OBJECT carries only the enemy object id and applies to the
            // currently selected group.
            let mut command = Command::new(CommandType::DoForceAttackObject);
            command.set_player_index(player);
            command.append_object_id_argument(*target);
            Some(command)
        }
        DoGuardPosition(coord, guard_mode) => {
            let mut command = Command::new(CommandType::DoGuardPosition);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            command.append_integer_argument(*guard_mode as Int);
            Some(command)
        }
        DoGuardObject(target, guard_mode) => {
            let mut command = Command::new(CommandType::DoGuardObject);
            command.set_player_index(player);
            command.append_object_id_argument(*target);
            command.append_integer_argument(*guard_mode as Int);
            Some(command)
        }
        SetRallyPoint(object, coord) => {
            let mut command = Command::new(CommandType::SetRallyPoint);
            command.set_player_index(player);
            command.append_object_id_argument(*object);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        DoStop => Some(basic_command(CommandType::DoStop, player)),
        DoScatter => Some(basic_command(CommandType::DoScatter, player)),
        AreaSelection(region) => {
            let mut command = Command::new(CommandType::AreaSelection);
            command.set_player_index(player);
            command.append_pixel_region_argument(to_logic_region(region));
            Some(command)
        }
        CreateTeamSlot(slot) => Some(basic_command(map_team_slot_create(*slot)?, player)),
        SelectTeamSlot(slot) => Some(basic_command(map_team_slot_select(*slot)?, player)),
        AddTeamSlot(slot) => Some(basic_command(map_team_slot_add(*slot)?, player)),
        PlaceBeacon(coord) => {
            let mut command = Command::new(CommandType::PlaceBeacon);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        RemoveBeacon(coord) => {
            let mut command = Command::new(CommandType::RemoveBeacon);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        SetBeaconText(coord, text) => {
            let mut command = Command::new(CommandType::SetBeaconText);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            command.append_ascii_string_argument(AsciiString::from(text.as_str()));
            Some(command)
        }
        SetReplayCamera(coord, pitch, zoom) => {
            let mut command = Command::new(CommandType::SetReplayCamera);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            command.append_real_argument(*pitch);
            command.append_real_argument(*zoom);
            Some(command)
        }
        ClearInGamePopupMessage => {
            Some(basic_command(CommandType::ClearInGamePopupMessage, player))
        }
        DozerConstruct(building_type, coord, angle) => {
            let mut command = Command::new(CommandType::DozerConstruct);
            command.set_player_index(player);
            command.append_integer_argument(*building_type as Int);
            command.append_location_argument(to_logic_coord(coord));
            command.append_real_argument(*angle);
            Some(command)
        }
        DozerConstructLine(building_type, start, end, angle) => {
            let mut command = Command::new(CommandType::DozerConstructLine);
            command.set_player_index(player);
            command.append_integer_argument(*building_type as Int);
            command.append_location_argument(to_logic_coord(start));
            command.append_real_argument(*angle);
            command.append_location_argument(to_logic_coord(end));
            Some(command)
        }
        DozerCancelConstruct(object) => Some(single_object_command(
            CommandType::DozerCancelConstruct,
            *object,
            player,
        )),
        Sell(object) => Some(single_object_command(CommandType::Sell, *object, player)),
        CombatDropAtLocation(coord) => {
            let mut command = Command::new(CommandType::CombatDropAtLocation);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        CombatDropAtObject(target) => Some(single_object_command(
            CommandType::CombatDropAtObject,
            *target,
            player,
        )),
        // C++ GameLogicDispatch.cpp: these commands act on the currently selected group and
        // carry only the target object ID.
        GetRepaired(facility) => Some(selection_target_command(
            CommandType::GetRepaired,
            *facility,
            player,
        )),
        GetHealed(facility) => Some(selection_target_command(
            CommandType::GetHealed,
            *facility,
            player,
        )),
        DoRepair(target) => Some(selection_target_command(
            CommandType::DoRepair,
            *target,
            player,
        )),
        ResumeConstruction(building) => Some(selection_target_command(
            CommandType::ResumeConstruction,
            *building,
            player,
        )),
        Enter(_unit, container) => Some(selection_target_command(
            CommandType::Enter,
            *container,
            player,
        )),
        Dock(dock_target) => Some(selection_target_command(
            CommandType::Dock,
            *dock_target,
            player,
        )),
        DoWeapon(weapon_id) => Some(integer_command(
            CommandType::DoWeapon,
            *weapon_id as Int,
            player,
        )),
        DoWeaponAtLocation(weapon_id, coord) => {
            let mut command = Command::new(CommandType::DoWeaponAtLocation);
            command.set_player_index(player);
            command.append_integer_argument(*weapon_id as Int);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        DoWeaponAtObject(weapon_id, target) => {
            let mut command = Command::new(CommandType::DoWeaponAtObject);
            command.set_player_index(player);
            command.append_integer_argument(*weapon_id as Int);
            command.append_object_id_argument(*target);
            Some(command)
        }
        DoSpecialPower(power_id, options, source) => {
            let mut command = Command::new(CommandType::DoSpecialPower);
            command.set_player_index(player);
            command.append_integer_argument(*power_id as Int);
            command.append_integer_argument(*options as Int);
            command.append_object_id_argument(*source);
            Some(command)
        }
        DoSpecialPowerAtLocation(power_id, coord, angle, object_in_way, options, source) => {
            let mut command = Command::new(CommandType::DoSpecialPowerAtLocation);
            command.set_player_index(player);
            command.append_integer_argument(*power_id as Int);
            command.append_location_argument(to_logic_coord(coord));
            command.append_real_argument(*angle);
            command.append_object_id_argument(*object_in_way);
            command.append_integer_argument(*options as Int);
            command.append_object_id_argument(*source);
            Some(command)
        }
        DoSpecialPowerAtObject(power_id, target, options, source) => {
            let mut command = Command::new(CommandType::DoSpecialPowerAtObject);
            command.set_player_index(player);
            command.append_integer_argument(*power_id as Int);
            command.append_object_id_argument(*target);
            command.append_integer_argument(*options as Int);
            command.append_object_id_argument(*source);
            Some(command)
        }
        DoSpecialPowerOverrideDestination(coord, power_type, source) => {
            let mut command = Command::new(CommandType::DoSpecialPowerOverrideDestination);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            command.append_integer_argument(*power_type as Int);
            command.append_object_id_argument(*source);
            Some(command)
        }
        PurchaseScience(id) => Some(integer_command(
            CommandType::PurchaseScience,
            *id as Int,
            player,
        )),
        QueueUpgrade(id) => Some(integer_command(
            CommandType::QueueUpgrade,
            *id as Int,
            player,
        )),
        CancelUpgrade(id) => Some(integer_command(
            CommandType::CancelUpgrade,
            *id as Int,
            player,
        )),
        QueueUnitCreate(id) => Some(integer_command(
            CommandType::QueueUnitCreate,
            *id as Int,
            player,
        )),
        CancelUnitCreate(id) => Some(integer_command(
            CommandType::CancelUnitCreate,
            *id as Int,
            player,
        )),
        DoSalvage(coord) => {
            let mut command = Command::new(CommandType::DoSalvage);
            command.set_player_index(player);
            command.append_location_argument(to_logic_coord(coord));
            Some(command)
        }
        InternetHack => Some(basic_command(CommandType::InternetHack, player)),
        DoCheer => Some(basic_command(CommandType::DoCheer, player)),
        ToggleOvercharge => Some(basic_command(CommandType::ToggleOvercharge, player)),
        SwitchWeapons(slot) => {
            let mut command = Command::new(CommandType::SwitchWeapons);
            command.set_player_index(player);
            command.append_integer_argument(*slot as i32);
            Some(command)
        }
        ConvertToCarbomb(_, target) => Some(selection_target_command(
            CommandType::ConvertToCarbomb,
            *target,
            player,
        )),
        CaptureBuilding(_, target) => Some(selection_target_command(
            CommandType::CaptureBuilding,
            *target,
            player,
        )),
        DisableVehicleHack(_, target) => Some(selection_target_command(
            CommandType::DisableVehicleHack,
            *target,
            player,
        )),
        StealCashHack(_, target) => Some(selection_target_command(
            CommandType::StealCashHack,
            *target,
            player,
        )),
        DisableBuildingHack(_, target) => Some(selection_target_command(
            CommandType::DisableBuildingHack,
            *target,
            player,
        )),
        SnipeVehicle(_, target) => Some(selection_target_command(
            CommandType::SnipeVehicle,
            *target,
            player,
        )),
        SelfDestruct(target_player) => {
            let mut command = basic_command(CommandType::SelfDestruct, player);
            command.append_integer_argument(*target_player as Int);
            Some(command)
        }
        CreateFormation(units) => {
            let mut command = basic_command(CommandType::CreateFormation, player);
            for unit in units {
                command.append_object_id_argument(*unit);
            }
            Some(command)
        }
        LogicCRC(crc) => {
            let mut command = basic_command(CommandType::LogicCrc, player);
            command.append_integer_argument(*crc as Int);
            Some(command)
        }
        SetMineClearingDetail(detail) => {
            let mut command = basic_command(CommandType::SetMineClearingDetail, player);
            command.append_integer_argument(*detail as Int);
            Some(command)
        }
        EnableRetaliationMode(target_player, enabled) => {
            let mut command = basic_command(CommandType::EnableRetaliationMode, player);
            command.append_integer_argument(*target_player as Int);
            command.append_boolean_argument(*enabled);
            Some(command)
        }
        _ => None,
    }
}

fn basic_command(cmd_type: CommandType, player: Int) -> Command {
    let mut command = Command::new(cmd_type);
    command.set_player_index(player);
    command
}

fn single_object_command(cmd_type: CommandType, object: ObjectID, player: Int) -> Command {
    let mut command = Command::new(cmd_type);
    command.set_player_index(player);
    command.append_object_id_argument(object);
    command
}

fn double_object_command(
    cmd_type: CommandType,
    first: ObjectID,
    second: ObjectID,
    player: Int,
) -> Command {
    let mut command = Command::new(cmd_type);
    command.set_player_index(player);
    command.append_object_id_argument(first);
    command.append_object_id_argument(second);
    command
}

fn selection_target_command(cmd_type: CommandType, target: ObjectID, player: Int) -> Command {
    const INVALID_OBJECT_ID: ObjectID = 0xFFFF_FFFF;

    let mut command = Command::new(cmd_type);
    command.set_player_index(player);
    command.append_object_id_argument(INVALID_OBJECT_ID);
    command.append_object_id_argument(target);
    command
}

fn integer_command(cmd_type: CommandType, value: Int, player: Int) -> Command {
    let mut command = Command::new(cmd_type);
    command.set_player_index(player);
    command.append_integer_argument(value);
    command
}

fn list_object_command(cmd_type: CommandType, objects: &[ObjectID], player: Int) -> Command {
    let mut command = Command::new(cmd_type);
    command.set_player_index(player);
    for object in objects {
        command.append_object_id_argument(*object);
    }
    command
}

fn map_team_slot_create(slot: u8) -> Option<CommandType> {
    match slot {
        0 => Some(CommandType::CreateTeam0),
        1 => Some(CommandType::CreateTeam1),
        2 => Some(CommandType::CreateTeam2),
        3 => Some(CommandType::CreateTeam3),
        4 => Some(CommandType::CreateTeam4),
        5 => Some(CommandType::CreateTeam5),
        6 => Some(CommandType::CreateTeam6),
        7 => Some(CommandType::CreateTeam7),
        8 => Some(CommandType::CreateTeam8),
        9 => Some(CommandType::CreateTeam9),
        _ => None,
    }
}

fn map_team_slot_select(slot: u8) -> Option<CommandType> {
    match slot {
        0 => Some(CommandType::SelectTeam0),
        1 => Some(CommandType::SelectTeam1),
        2 => Some(CommandType::SelectTeam2),
        3 => Some(CommandType::SelectTeam3),
        4 => Some(CommandType::SelectTeam4),
        5 => Some(CommandType::SelectTeam5),
        6 => Some(CommandType::SelectTeam6),
        7 => Some(CommandType::SelectTeam7),
        8 => Some(CommandType::SelectTeam8),
        9 => Some(CommandType::SelectTeam9),
        _ => None,
    }
}

fn map_team_slot_add(slot: u8) -> Option<CommandType> {
    match slot {
        0 => Some(CommandType::AddTeam0),
        1 => Some(CommandType::AddTeam1),
        2 => Some(CommandType::AddTeam2),
        3 => Some(CommandType::AddTeam3),
        4 => Some(CommandType::AddTeam4),
        5 => Some(CommandType::AddTeam5),
        6 => Some(CommandType::AddTeam6),
        7 => Some(CommandType::AddTeam7),
        8 => Some(CommandType::AddTeam8),
        9 => Some(CommandType::AddTeam9),
        _ => None,
    }
}

fn to_logic_coord(coord: &Coord3D) -> LogicCoord3D {
    Vec3::new(coord.x, coord.y, coord.z)
}

fn append_integer_message_arguments(message: &GameMessage, command: &mut Command) {
    for i in 0..message.get_argument_count() {
        if let Some(GameMessageArgumentType::Integer(value)) = message.get_argument(i) {
            command.append_integer_argument(*value);
        }
    }
}

fn to_logic_region(region: &IRegion2D) -> LogicRegion2D {
    let lo = IVec2::new(region.x, region.y);
    let hi = IVec2::new(region.x + region.width, region.y + region.height);
    LogicRegion2D::new(lo, hi)
}
