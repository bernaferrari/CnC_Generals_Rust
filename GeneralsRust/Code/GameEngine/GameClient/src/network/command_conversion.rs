use std::collections::HashMap;

use game_network::commands::{CommandParameter, GameCommandData};
use gamelogic::commands::command::CommandType;
use log::warn;

use crate::message_stream::game_message::{
    Coord3D, GameMessage, GameMessageType, IRegion2D, ObjectID,
};

const ARG_PREFIX: &str = "arg";

fn encode_params(params: Vec<CommandParameter>) -> HashMap<String, CommandParameter> {
    params
        .into_iter()
        .enumerate()
        .map(|(idx, param)| (format!("{ARG_PREFIX}{idx:03}"), param))
        .collect()
}

fn ordered_params(data: &GameCommandData) -> Vec<CommandParameter> {
    let mut entries: Vec<_> = data.parameters.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
        .into_iter()
        .map(|(_, value)| value.clone())
        .collect()
}

fn coord_from_tuple(tuple: &(f32, f32, f32)) -> Coord3D {
    Coord3D {
        x: tuple.0,
        y: tuple.1,
        z: tuple.2,
    }
}

fn parameter_to_object_id(param: &CommandParameter) -> Option<ObjectID> {
    if let CommandParameter::ObjectId(id) = param {
        Some(*id)
    } else {
        None
    }
}

fn parameter_to_coord(param: &CommandParameter) -> Option<Coord3D> {
    if let CommandParameter::Position(x, y, z) = param {
        Some(Coord3D {
            x: *x,
            y: *y,
            z: *z,
        })
    } else {
        None
    }
}

fn parameter_to_int(param: &CommandParameter) -> Option<i32> {
    if let CommandParameter::Int(value) = param {
        Some(*value)
    } else {
        None
    }
}

fn parameter_to_float(param: &CommandParameter) -> Option<f32> {
    if let CommandParameter::Float(value) = param {
        Some(*value)
    } else {
        None
    }
}

fn parameter_to_bool(param: &CommandParameter) -> Option<bool> {
    if let CommandParameter::Bool(value) = param {
        Some(*value)
    } else {
        None
    }
}

fn parameter_to_string(param: &CommandParameter) -> Option<String> {
    if let CommandParameter::String(value) = param {
        Some(value.clone())
    } else {
        None
    }
}

/// Returns `true` when the message is a gameplay command that should be
/// replicated across the network.
pub fn is_network_command_message(message_type: &GameMessageType) -> bool {
    game_engine::common::message_stream::is_network_command_message(message_type)
}

/// Encode a `GameMessage` into a `GameCommandData` payload that the
/// networking layer understands. Returns `None` for message types that aren't
/// replicated over the network yet.
pub fn encode_game_message(message: &GameMessage) -> Option<GameCommandData> {
    let msg_type = message.get_type().clone();
    match msg_type {
        GameMessageType::CreateSelectedGroup(create_new, units) => Some(GameCommandData {
            command_type: CommandType::CreateSelectedGroup as u32,
            target_id: None,
            position: None,
            parameters: encode_params(
                std::iter::once(CommandParameter::Bool(create_new))
                    .chain(units.iter().map(|unit| CommandParameter::ObjectId(*unit)))
                    .collect(),
            ),
            checksum: 0,
        }),
        GameMessageType::CreateSelectedGroupNoSound(create_new, units) => Some(GameCommandData {
            command_type: CommandType::CreateSelectedGroupNoSound as u32,
            target_id: None,
            position: None,
            parameters: encode_params(
                std::iter::once(CommandParameter::Bool(create_new))
                    .chain(units.iter().map(|unit| CommandParameter::ObjectId(*unit)))
                    .collect(),
            ),
            checksum: 0,
        }),
        GameMessageType::DestroySelectedGroup(team_id) => Some(GameCommandData {
            command_type: CommandType::DestroySelectedGroup as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(team_id as i32)]),
            checksum: 0,
        }),
        GameMessageType::RemoveFromSelectedGroup(units) => Some(GameCommandData {
            command_type: CommandType::RemoveFromSelectedGroup as u32,
            target_id: None,
            position: None,
            parameters: encode_params(
                units
                    .iter()
                    .map(|unit| CommandParameter::ObjectId(*unit))
                    .collect(),
            ),
            checksum: 0,
        }),
        GameMessageType::SelectedGroupCommand(team_id) => Some(GameCommandData {
            command_type: CommandType::SelectedGroupCommand as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(team_id as i32)]),
            checksum: 0,
        }),
        GameMessageType::CreateTeamSlot(slot) => Some(GameCommandData {
            command_type: match slot {
                0 => CommandType::CreateTeam0,
                1 => CommandType::CreateTeam1,
                2 => CommandType::CreateTeam2,
                3 => CommandType::CreateTeam3,
                4 => CommandType::CreateTeam4,
                5 => CommandType::CreateTeam5,
                6 => CommandType::CreateTeam6,
                7 => CommandType::CreateTeam7,
                8 => CommandType::CreateTeam8,
                _ => CommandType::CreateTeam9,
            } as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::SelectTeamSlot(slot) => Some(GameCommandData {
            command_type: match slot {
                0 => CommandType::SelectTeam0,
                1 => CommandType::SelectTeam1,
                2 => CommandType::SelectTeam2,
                3 => CommandType::SelectTeam3,
                4 => CommandType::SelectTeam4,
                5 => CommandType::SelectTeam5,
                6 => CommandType::SelectTeam6,
                7 => CommandType::SelectTeam7,
                8 => CommandType::SelectTeam8,
                _ => CommandType::SelectTeam9,
            } as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::AddTeamSlot(slot) => Some(GameCommandData {
            command_type: match slot {
                0 => CommandType::AddTeam0,
                1 => CommandType::AddTeam1,
                2 => CommandType::AddTeam2,
                3 => CommandType::AddTeam3,
                4 => CommandType::AddTeam4,
                5 => CommandType::AddTeam5,
                6 => CommandType::AddTeam6,
                7 => CommandType::AddTeam7,
                8 => CommandType::AddTeam8,
                _ => CommandType::AddTeam9,
            } as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::Exit(object) => Some(GameCommandData {
            command_type: CommandType::Exit as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(object)]),
            checksum: 0,
        }),
        GameMessageType::Evacuate => Some(GameCommandData {
            command_type: CommandType::Evacuate as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::EvacuateAtLocation(coord) => Some(GameCommandData {
            command_type: CommandType::Evacuate as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::ExecuteRailedTransport => Some(GameCommandData {
            command_type: CommandType::ExecuteRailedTransport as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::DoAttackSquad(units) => Some(GameCommandData {
            command_type: CommandType::DoAttackSquad as u32,
            target_id: None,
            position: None,
            parameters: encode_params(
                units
                    .iter()
                    .map(|unit| CommandParameter::ObjectId(*unit))
                    .collect(),
            ),
            checksum: 0,
        }),
        GameMessageType::DoMoveTo(pos) => Some(GameCommandData {
            command_type: CommandType::DoMoveTo as u32,
            target_id: None,
            position: Some((pos.x, pos.y, pos.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::DoAttackMoveTo(pos) => Some(GameCommandData {
            command_type: CommandType::DoAttackMoveTo as u32,
            target_id: None,
            position: Some((pos.x, pos.y, pos.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::DoForceMoveTO(pos) => Some(GameCommandData {
            command_type: CommandType::DoForceMoveTo as u32,
            target_id: None,
            position: Some((pos.x, pos.y, pos.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::DoAttackObject(target) => Some(GameCommandData {
            command_type: CommandType::DoAttackObject as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(target)]),
            checksum: 0,
        }),
        GameMessageType::DoForceAttackObject(target) => Some(GameCommandData {
            command_type: CommandType::DoForceAttackObject as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(target)]),
            checksum: 0,
        }),
        GameMessageType::DoForceAttackGround(coord) => Some(GameCommandData {
            command_type: CommandType::DoForceAttackGround as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::AddWaypoint(coord) => Some(GameCommandData {
            command_type: CommandType::AddWaypoint as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::DoStop => Some(GameCommandData {
            command_type: CommandType::DoStop as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::DoScatter => Some(GameCommandData {
            command_type: CommandType::DoScatter as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::DoGuardObject(target, guard_mode) => Some(GameCommandData {
            command_type: CommandType::DoGuardObject as u32,
            target_id: Some(target),
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(guard_mode)]),
            checksum: 0,
        }),
        GameMessageType::DoGuardPosition(coord, guard_mode) => Some(GameCommandData {
            command_type: CommandType::DoGuardPosition as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: encode_params(vec![CommandParameter::Int(guard_mode)]),
            checksum: 0,
        }),
        GameMessageType::SetRallyPoint(unit, coord) => Some(GameCommandData {
            command_type: CommandType::SetRallyPoint as u32,
            target_id: Some(unit),
            position: Some((coord.x, coord.y, coord.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::DoWeapon(weapon_id) => Some(GameCommandData {
            command_type: CommandType::DoWeapon as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(weapon_id as i32)]),
            checksum: 0,
        }),
        GameMessageType::DoWeaponAtLocation(weapon_id, coord) => Some(GameCommandData {
            command_type: CommandType::DoWeaponAtLocation as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: encode_params(vec![CommandParameter::Int(weapon_id as i32)]),
            checksum: 0,
        }),
        GameMessageType::DoWeaponAtObject(weapon_id, target) => Some(GameCommandData {
            command_type: CommandType::DoWeaponAtObject as u32,
            target_id: Some(target),
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(weapon_id as i32)]),
            checksum: 0,
        }),
        GameMessageType::DoSpecialPower(power_id, options, source) => Some(GameCommandData {
            command_type: CommandType::DoSpecialPower as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![
                CommandParameter::Int(power_id as i32),
                CommandParameter::Int(options as i32),
                CommandParameter::ObjectId(source),
            ]),
            checksum: 0,
        }),
        GameMessageType::DoSpecialPowerAtLocation(
            power_id,
            coord,
            angle,
            object_in_way,
            options,
            source,
        ) => Some(GameCommandData {
            command_type: CommandType::DoSpecialPowerAtLocation as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: encode_params(vec![
                CommandParameter::Int(power_id as i32),
                CommandParameter::Float(angle),
                CommandParameter::ObjectId(object_in_way),
                CommandParameter::Int(options as i32),
                CommandParameter::ObjectId(source),
            ]),
            checksum: 0,
        }),
        GameMessageType::DoSpecialPowerAtObject(power_id, target, options, source) => {
            Some(GameCommandData {
                command_type: CommandType::DoSpecialPowerAtObject as u32,
                target_id: Some(target),
                position: None,
                parameters: encode_params(vec![
                    CommandParameter::Int(power_id as i32),
                    CommandParameter::Int(options as i32),
                    CommandParameter::ObjectId(source),
                ]),
                checksum: 0,
            })
        }
        GameMessageType::PurchaseScience(science_id) => Some(GameCommandData {
            command_type: CommandType::PurchaseScience as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(science_id as i32)]),
            checksum: 0,
        }),
        GameMessageType::QueueUpgrade(upgrade_id) => Some(GameCommandData {
            command_type: CommandType::QueueUpgrade as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(upgrade_id as i32)]),
            checksum: 0,
        }),
        GameMessageType::CancelUpgrade(upgrade_id) => Some(GameCommandData {
            command_type: CommandType::CancelUpgrade as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(upgrade_id as i32)]),
            checksum: 0,
        }),
        GameMessageType::QueueUnitCreate(unit_type) => Some(GameCommandData {
            command_type: CommandType::QueueUnitCreate as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(unit_type as i32)]),
            checksum: 0,
        }),
        GameMessageType::CancelUnitCreate(unit_type) => Some(GameCommandData {
            command_type: CommandType::CancelUnitCreate as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(unit_type as i32)]),
            checksum: 0,
        }),
        GameMessageType::DozerConstruct(building_type, coord, angle) => Some(GameCommandData {
            command_type: CommandType::DozerConstruct as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: encode_params(vec![
                CommandParameter::Int(building_type as i32),
                CommandParameter::Float(angle),
            ]),
            checksum: 0,
        }),
        GameMessageType::DozerConstructLine(building_type, start, end, angle) => {
            Some(GameCommandData {
                command_type: CommandType::DozerConstructLine as u32,
                target_id: None,
                position: Some((start.x, start.y, start.z)),
                parameters: encode_params(vec![
                    CommandParameter::Int(building_type as i32),
                    CommandParameter::Float(angle),
                    CommandParameter::Position(end.x, end.y, end.z),
                ]),
                checksum: 0,
            })
        }
        GameMessageType::DozerCancelConstruct(object_id) => Some(GameCommandData {
            command_type: CommandType::DozerCancelConstruct as u32,
            target_id: Some(object_id),
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::Sell(object_id) => Some(GameCommandData {
            command_type: CommandType::Sell as u32,
            target_id: Some(object_id),
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::PlaceBeacon(coord) => Some(GameCommandData {
            command_type: CommandType::PlaceBeacon as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::RemoveBeacon(coord) => Some(GameCommandData {
            command_type: CommandType::RemoveBeacon as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::SetBeaconText(coord, text) => Some(GameCommandData {
            command_type: CommandType::SetBeaconText as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: encode_params(vec![CommandParameter::String(text)]),
            checksum: 0,
        }),
        GameMessageType::SetReplayCamera(coord, pitch, zoom) => Some(GameCommandData {
            command_type: CommandType::SetReplayCamera as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: encode_params(vec![
                CommandParameter::Float(pitch),
                CommandParameter::Float(zoom),
            ]),
            checksum: 0,
        }),
        GameMessageType::ClearInGamePopupMessage => Some(GameCommandData {
            command_type: CommandType::ClearInGamePopupMessage as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::AreaSelection(region) => Some(GameCommandData {
            command_type: CommandType::AreaSelection as u32,
            target_id: None,
            position: Some((region.x as f32, region.y as f32, region.width as f32)),
            parameters: encode_params(vec![CommandParameter::Int(region.height)]),
            checksum: 0,
        }),
        GameMessageType::CombatDropAtLocation(coord) => Some(GameCommandData {
            command_type: CommandType::CombatDropAtLocation as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::CombatDropAtObject(target) => Some(GameCommandData {
            command_type: CommandType::CombatDropAtObject as u32,
            target_id: Some(target),
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::GetRepaired(facility) => Some(GameCommandData {
            command_type: CommandType::GetRepaired as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(facility)]),
            checksum: 0,
        }),
        GameMessageType::GetHealed(facility) => Some(GameCommandData {
            command_type: CommandType::GetHealed as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(facility)]),
            checksum: 0,
        }),
        GameMessageType::DoRepair(target) => Some(GameCommandData {
            command_type: CommandType::DoRepair as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(target)]),
            checksum: 0,
        }),
        GameMessageType::ResumeConstruction(building) => Some(GameCommandData {
            command_type: CommandType::ResumeConstruction as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(building)]),
            checksum: 0,
        }),
        GameMessageType::Enter(unit, container) => Some(GameCommandData {
            command_type: CommandType::Enter as u32,
            target_id: Some(unit),
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(container)]),
            checksum: 0,
        }),
        GameMessageType::Dock(dock_target) => Some(GameCommandData {
            command_type: CommandType::Dock as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(dock_target)]),
            checksum: 0,
        }),
        GameMessageType::DoSpecialPowerOverrideDestination(coord, power_type, source) => {
            Some(GameCommandData {
                command_type: CommandType::DoSpecialPowerOverrideDestination as u32,
                target_id: None,
                position: Some((coord.x, coord.y, coord.z)),
                parameters: encode_params(vec![
                    CommandParameter::Int(power_type as i32),
                    CommandParameter::ObjectId(source),
                ]),
                checksum: 0,
            })
        }
        GameMessageType::DoSalvage(coord) => Some(GameCommandData {
            command_type: CommandType::DoSalvage as u32,
            target_id: None,
            position: Some((coord.x, coord.y, coord.z)),
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::InternetHack => Some(GameCommandData {
            command_type: CommandType::InternetHack as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::DoCheer => Some(GameCommandData {
            command_type: CommandType::DoCheer as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::ToggleOvercharge => Some(GameCommandData {
            command_type: CommandType::ToggleOvercharge as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        }),
        GameMessageType::SwitchWeapons(slot) => Some(GameCommandData {
            command_type: CommandType::SwitchWeapons as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(slot as i32)]),
            checksum: 0,
        }),
        GameMessageType::ConvertToCarbomb(unit, target) => Some(GameCommandData {
            command_type: CommandType::ConvertToCarbomb as u32,
            target_id: Some(unit),
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(target)]),
            checksum: 0,
        }),
        GameMessageType::CaptureBuilding(unit, target) => Some(GameCommandData {
            command_type: CommandType::CaptureBuilding as u32,
            target_id: Some(unit),
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(target)]),
            checksum: 0,
        }),
        GameMessageType::DisableVehicleHack(unit, target) => Some(GameCommandData {
            command_type: CommandType::DisableVehicleHack as u32,
            target_id: Some(unit),
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(target)]),
            checksum: 0,
        }),
        GameMessageType::StealCashHack(unit, target) => Some(GameCommandData {
            command_type: CommandType::StealCashHack as u32,
            target_id: Some(unit),
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(target)]),
            checksum: 0,
        }),
        GameMessageType::DisableBuildingHack(unit, target) => Some(GameCommandData {
            command_type: CommandType::DisableBuildingHack as u32,
            target_id: Some(unit),
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(target)]),
            checksum: 0,
        }),
        GameMessageType::SnipeVehicle(unit, target) => Some(GameCommandData {
            command_type: CommandType::SnipeVehicle as u32,
            target_id: Some(unit),
            position: None,
            parameters: encode_params(vec![CommandParameter::ObjectId(target)]),
            checksum: 0,
        }),
        GameMessageType::SelfDestruct(player_id) => Some(GameCommandData {
            command_type: CommandType::SelfDestruct as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(player_id as i32)]),
            checksum: 0,
        }),
        GameMessageType::CreateFormation(units) => Some(GameCommandData {
            command_type: CommandType::CreateFormation as u32,
            target_id: None,
            position: None,
            parameters: encode_params(
                units
                    .iter()
                    .map(|unit| CommandParameter::ObjectId(*unit))
                    .collect(),
            ),
            checksum: 0,
        }),
        GameMessageType::LogicCRC(crc) => Some(GameCommandData {
            command_type: CommandType::LogicCrc as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(crc as i32)]),
            checksum: 0,
        }),
        GameMessageType::SetMineClearingDetail(level) => Some(GameCommandData {
            command_type: CommandType::SetMineClearingDetail as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![CommandParameter::Int(level as i32)]),
            checksum: 0,
        }),
        GameMessageType::EnableRetaliationMode(player_id, enabled) => Some(GameCommandData {
            command_type: CommandType::EnableRetaliationMode as u32,
            target_id: None,
            position: None,
            parameters: encode_params(vec![
                CommandParameter::Int(player_id as i32),
                CommandParameter::Bool(enabled),
            ]),
            checksum: 0,
        }),
        _ => None,
    }
}

fn set_player(message: &mut GameMessage, player_id: u8) {
    message.set_player_index(i32::from(player_id));
}

/// Convert an incoming `GameCommandData` payload into a `GameMessage` so the
/// existing client command pipeline can consume it.
pub fn decode_game_command(data: &GameCommandData, player_id: u8) -> Option<GameMessage> {
    let command_type = CommandType::try_from(data.command_type as u16)
        .map_err(|_| data.command_type)
        .ok()?;

    let mut message = match command_type {
        CommandType::CreateSelectedGroup => {
            let params = ordered_params(data);
            let create_new = parameter_to_bool(params.get(0)?)?;
            let units = params
                .iter()
                .skip(1)
                .filter_map(parameter_to_object_id)
                .collect::<Vec<_>>();
            Some(GameMessage::new(GameMessageType::CreateSelectedGroup(
                create_new, units,
            )))
        }
        CommandType::CreateSelectedGroupNoSound => {
            let params = ordered_params(data);
            let create_new = parameter_to_bool(params.get(0)?)?;
            let units = params
                .iter()
                .skip(1)
                .filter_map(parameter_to_object_id)
                .collect::<Vec<_>>();
            Some(GameMessage::new(
                GameMessageType::CreateSelectedGroupNoSound(create_new, units),
            ))
        }
        CommandType::DestroySelectedGroup => {
            let params = ordered_params(data);
            let team_id = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::DestroySelectedGroup(
                team_id,
            )))
        }
        CommandType::RemoveFromSelectedGroup => {
            let params = ordered_params(data);
            let units = params
                .iter()
                .filter_map(parameter_to_object_id)
                .collect::<Vec<_>>();
            Some(GameMessage::new(GameMessageType::RemoveFromSelectedGroup(
                units,
            )))
        }
        CommandType::SelectedGroupCommand => {
            let params = ordered_params(data);
            let team_id = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::SelectedGroupCommand(
                team_id,
            )))
        }
        CommandType::CreateTeam0 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(0))),
        CommandType::CreateTeam1 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(1))),
        CommandType::CreateTeam2 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(2))),
        CommandType::CreateTeam3 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(3))),
        CommandType::CreateTeam4 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(4))),
        CommandType::CreateTeam5 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(5))),
        CommandType::CreateTeam6 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(6))),
        CommandType::CreateTeam7 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(7))),
        CommandType::CreateTeam8 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(8))),
        CommandType::CreateTeam9 => Some(GameMessage::new(GameMessageType::CreateTeamSlot(9))),
        CommandType::SelectTeam0 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(0))),
        CommandType::SelectTeam1 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(1))),
        CommandType::SelectTeam2 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(2))),
        CommandType::SelectTeam3 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(3))),
        CommandType::SelectTeam4 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(4))),
        CommandType::SelectTeam5 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(5))),
        CommandType::SelectTeam6 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(6))),
        CommandType::SelectTeam7 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(7))),
        CommandType::SelectTeam8 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(8))),
        CommandType::SelectTeam9 => Some(GameMessage::new(GameMessageType::SelectTeamSlot(9))),
        CommandType::AddTeam0 => Some(GameMessage::new(GameMessageType::AddTeamSlot(0))),
        CommandType::AddTeam1 => Some(GameMessage::new(GameMessageType::AddTeamSlot(1))),
        CommandType::AddTeam2 => Some(GameMessage::new(GameMessageType::AddTeamSlot(2))),
        CommandType::AddTeam3 => Some(GameMessage::new(GameMessageType::AddTeamSlot(3))),
        CommandType::AddTeam4 => Some(GameMessage::new(GameMessageType::AddTeamSlot(4))),
        CommandType::AddTeam5 => Some(GameMessage::new(GameMessageType::AddTeamSlot(5))),
        CommandType::AddTeam6 => Some(GameMessage::new(GameMessageType::AddTeamSlot(6))),
        CommandType::AddTeam7 => Some(GameMessage::new(GameMessageType::AddTeamSlot(7))),
        CommandType::AddTeam8 => Some(GameMessage::new(GameMessageType::AddTeamSlot(8))),
        CommandType::AddTeam9 => Some(GameMessage::new(GameMessageType::AddTeamSlot(9))),
        CommandType::Exit => {
            let params = ordered_params(data);
            let object = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::Exit(object)))
        }
        CommandType::Evacuate => {
            if let Some(position_tuple) = data.position {
                Some(GameMessage::new(GameMessageType::EvacuateAtLocation(
                    coord_from_tuple(&position_tuple),
                )))
            } else {
                Some(GameMessage::new(GameMessageType::Evacuate))
            }
        }
        CommandType::ExecuteRailedTransport => {
            Some(GameMessage::new(GameMessageType::ExecuteRailedTransport))
        }
        CommandType::DoAttackSquad => {
            let params = ordered_params(data);
            let units = params
                .iter()
                .filter_map(parameter_to_object_id)
                .collect::<Vec<_>>();
            Some(GameMessage::new(GameMessageType::DoAttackSquad(units)))
        }
        CommandType::DoMoveTo => {
            let position_tuple = data.position?;
            let msg =
                GameMessage::new(GameMessageType::DoMoveTo(coord_from_tuple(&position_tuple)));
            Some(msg)
        }
        CommandType::DoForceMoveTo => {
            let position_tuple = data.position?;
            let msg = GameMessage::new(GameMessageType::DoForceMoveTO(coord_from_tuple(
                &position_tuple,
            )));
            Some(msg)
        }
        CommandType::DoAttackObject => {
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::DoAttackObject(target)))
        }
        CommandType::DoForceAttackObject => {
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::DoForceAttackObject(
                target,
            )))
        }
        CommandType::DoForceAttackGround => {
            let position_tuple = data.position?;
            Some(GameMessage::new(GameMessageType::DoForceAttackGround(
                coord_from_tuple(&position_tuple),
            )))
        }
        CommandType::AddWaypoint => {
            let position_tuple = data.position?;
            Some(GameMessage::new(GameMessageType::AddWaypoint(
                coord_from_tuple(&position_tuple),
            )))
        }
        CommandType::DoStop => Some(GameMessage::new(GameMessageType::DoStop)),
        CommandType::DoScatter => Some(GameMessage::new(GameMessageType::DoScatter)),
        CommandType::DoAttackMoveTo => {
            let position_tuple = data.position?;
            Some(GameMessage::new(GameMessageType::DoAttackMoveTo(
                coord_from_tuple(&position_tuple),
            )))
        }
        CommandType::DoGuardObject => {
            let target = data.target_id?;
            let params = ordered_params(data);
            let guard_mode = parameter_to_int(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::DoGuardObject(
                target, guard_mode,
            )))
        }
        CommandType::DoGuardPosition => {
            let position_tuple = data.position?;
            let params = ordered_params(data);
            let guard_mode = parameter_to_int(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::DoGuardPosition(
                coord_from_tuple(&position_tuple),
                guard_mode,
            )))
        }
        CommandType::SetRallyPoint => {
            let unit = data.target_id?;
            let position_tuple = data.position?;
            Some(GameMessage::new(GameMessageType::SetRallyPoint(
                unit,
                coord_from_tuple(&position_tuple),
            )))
        }
        CommandType::DoWeapon => {
            let params = ordered_params(data);
            let weapon_id = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::DoWeapon(weapon_id)))
        }
        CommandType::DoWeaponAtLocation => {
            let position_tuple = data.position?;
            let params = ordered_params(data);
            let weapon_id = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::DoWeaponAtLocation(
                weapon_id,
                coord_from_tuple(&position_tuple),
            )))
        }
        CommandType::DoWeaponAtObject => {
            let target = data.target_id?;
            let params = ordered_params(data);
            let weapon_id = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::DoWeaponAtObject(
                weapon_id, target,
            )))
        }
        CommandType::DoSpecialPower => {
            let params = ordered_params(data);
            let power_id = parameter_to_int(params.get(0)?)? as u32;
            let options = params.get(1).and_then(parameter_to_int).unwrap_or(0) as u32;
            let source = params.get(2).and_then(parameter_to_object_id).unwrap_or(0);
            Some(GameMessage::new(GameMessageType::DoSpecialPower(
                power_id, options, source,
            )))
        }
        CommandType::DoSpecialPowerAtLocation => {
            let position_tuple = data.position?;
            let params = ordered_params(data);
            let power_id = parameter_to_int(params.get(0)?)? as u32;
            let angle = params.get(1).and_then(parameter_to_float).unwrap_or(0.0);
            let object_in_way = params.get(2).and_then(parameter_to_object_id).unwrap_or(0);
            let options = params.get(3).and_then(parameter_to_int).unwrap_or(0) as u32;
            let source = params.get(4).and_then(parameter_to_object_id).unwrap_or(0);
            Some(GameMessage::new(GameMessageType::DoSpecialPowerAtLocation(
                power_id,
                coord_from_tuple(&position_tuple),
                angle,
                object_in_way,
                options,
                source,
            )))
        }
        CommandType::DoSpecialPowerAtObject => {
            let target = data.target_id?;
            let params = ordered_params(data);
            let power_id = parameter_to_int(params.get(0)?)? as u32;
            let options = params.get(1).and_then(parameter_to_int).unwrap_or(0) as u32;
            let source = params.get(2).and_then(parameter_to_object_id).unwrap_or(0);
            Some(GameMessage::new(GameMessageType::DoSpecialPowerAtObject(
                power_id, target, options, source,
            )))
        }
        CommandType::PurchaseScience => {
            let params = ordered_params(data);
            let science_id = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::PurchaseScience(
                science_id,
            )))
        }
        CommandType::QueueUpgrade => {
            let params = ordered_params(data);
            let upgrade_id = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::QueueUpgrade(upgrade_id)))
        }
        CommandType::CancelUpgrade => {
            let params = ordered_params(data);
            let upgrade_id = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::CancelUpgrade(upgrade_id)))
        }
        CommandType::QueueUnitCreate => {
            let params = ordered_params(data);
            let unit_type = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::QueueUnitCreate(
                unit_type,
            )))
        }
        CommandType::CancelUnitCreate => {
            let params = ordered_params(data);
            let unit_type = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::CancelUnitCreate(
                unit_type,
            )))
        }
        CommandType::DozerConstruct => {
            let position_tuple = data.position?;
            let params = ordered_params(data);
            let building_type = parameter_to_int(params.get(0)?)?;
            let angle = parameter_to_float(params.get(1)?).unwrap_or(0.0);
            Some(GameMessage::new(GameMessageType::DozerConstruct(
                building_type as u32,
                coord_from_tuple(&position_tuple),
                angle,
            )))
        }
        CommandType::DozerConstructLine => {
            let position_tuple = data.position?;
            let params = ordered_params(data);
            let building_type = parameter_to_int(params.get(0)?)?;
            let angle = parameter_to_float(params.get(1)?).unwrap_or(0.0);
            let end_coord = parameter_to_coord(params.get(2)?)?;
            Some(GameMessage::new(GameMessageType::DozerConstructLine(
                building_type as u32,
                coord_from_tuple(&position_tuple),
                end_coord,
                angle,
            )))
        }
        CommandType::DozerCancelConstruct => {
            let object_id = data.target_id?;
            Some(GameMessage::new(GameMessageType::DozerCancelConstruct(
                object_id,
            )))
        }
        CommandType::Sell => {
            let object_id = data.target_id?;
            Some(GameMessage::new(GameMessageType::Sell(object_id)))
        }
        CommandType::PlaceBeacon => {
            let position_tuple = data.position?;
            Some(GameMessage::new(GameMessageType::PlaceBeacon(
                coord_from_tuple(&position_tuple),
            )))
        }
        CommandType::RemoveBeacon => {
            let position_tuple = data.position?;
            Some(GameMessage::new(GameMessageType::RemoveBeacon(
                coord_from_tuple(&position_tuple),
            )))
        }
        CommandType::SetBeaconText => {
            let position_tuple = data.position?;
            let params = ordered_params(data);
            let text = parameter_to_string(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::SetBeaconText(
                coord_from_tuple(&position_tuple),
                text,
            )))
        }
        CommandType::SetReplayCamera => {
            let position_tuple = data.position?;
            let params = ordered_params(data);
            let pitch = parameter_to_float(params.get(0)?)?;
            let zoom = parameter_to_float(params.get(1)?)?;
            Some(GameMessage::new(GameMessageType::SetReplayCamera(
                coord_from_tuple(&position_tuple),
                pitch,
                zoom,
            )))
        }
        CommandType::ClearInGamePopupMessage => {
            Some(GameMessage::new(GameMessageType::ClearInGamePopupMessage))
        }
        CommandType::AreaSelection => {
            let position_tuple = data.position?;
            let params = ordered_params(data);
            let height = params
                .get(0)
                .and_then(|param| match param {
                    CommandParameter::Int(value) => Some(*value),
                    _ => None,
                })
                .unwrap_or_default();
            let region = IRegion2D {
                x: position_tuple.0 as i32,
                y: position_tuple.1 as i32,
                width: position_tuple.2 as i32,
                height,
            };
            Some(GameMessage::new(GameMessageType::AreaSelection(region)))
        }
        CommandType::CombatDropAtLocation => {
            let position_tuple = data.position?;
            Some(GameMessage::new(GameMessageType::CombatDropAtLocation(
                coord_from_tuple(&position_tuple),
            )))
        }
        CommandType::CombatDropAtObject => {
            let target = data.target_id?;
            Some(GameMessage::new(GameMessageType::CombatDropAtObject(
                target,
            )))
        }
        CommandType::GetRepaired => {
            let params = ordered_params(data);
            let facility = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::GetRepaired(facility)))
        }
        CommandType::GetHealed => {
            let params = ordered_params(data);
            let facility = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::GetHealed(facility)))
        }
        CommandType::DoRepair => {
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::DoRepair(target)))
        }
        CommandType::ResumeConstruction => {
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::ResumeConstruction(
                target,
            )))
        }
        CommandType::Enter => {
            let unit = data.target_id?;
            let params = ordered_params(data);
            let container = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::Enter(unit, container)))
        }
        CommandType::Dock => {
            let params = ordered_params(data);
            let dock_target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::Dock(dock_target)))
        }
        CommandType::DoSpecialPowerOverrideDestination => {
            let position_tuple = data.position?;
            let params = ordered_params(data);
            let power_type = params.get(0).and_then(parameter_to_int).unwrap_or(0) as u32;
            let source = params.get(1).and_then(parameter_to_object_id).unwrap_or(0);
            Some(GameMessage::new(
                GameMessageType::DoSpecialPowerOverrideDestination(
                    coord_from_tuple(&position_tuple),
                    power_type,
                    source,
                ),
            ))
        }
        CommandType::DoSalvage => {
            let position_tuple = data.position?;
            Some(GameMessage::new(GameMessageType::DoSalvage(
                coord_from_tuple(&position_tuple),
            )))
        }
        CommandType::InternetHack => Some(GameMessage::new(GameMessageType::InternetHack)),
        CommandType::DoCheer => Some(GameMessage::new(GameMessageType::DoCheer)),
        CommandType::ToggleOvercharge => Some(GameMessage::new(GameMessageType::ToggleOvercharge)),
        CommandType::SwitchWeapons => {
            let params = ordered_params(data);
            let slot = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::SwitchWeapons(slot)))
        }
        CommandType::ConvertToCarbomb => {
            let unit = data.target_id?;
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::ConvertToCarbomb(
                unit, target,
            )))
        }
        CommandType::CaptureBuilding => {
            let unit = data.target_id?;
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::CaptureBuilding(
                unit, target,
            )))
        }
        CommandType::DisableVehicleHack => {
            let unit = data.target_id?;
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::DisableVehicleHack(
                unit, target,
            )))
        }
        CommandType::StealCashHack => {
            let unit = data.target_id?;
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::StealCashHack(
                unit, target,
            )))
        }
        CommandType::DisableBuildingHack => {
            let unit = data.target_id?;
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::DisableBuildingHack(
                unit, target,
            )))
        }
        CommandType::SnipeVehicle => {
            let unit = data.target_id?;
            let params = ordered_params(data);
            let target = parameter_to_object_id(params.get(0)?)?;
            Some(GameMessage::new(GameMessageType::SnipeVehicle(
                unit, target,
            )))
        }
        CommandType::SelfDestruct => {
            let params = ordered_params(data);
            let player_id = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::SelfDestruct(player_id)))
        }
        CommandType::CreateFormation => {
            let params = ordered_params(data);
            let units = params
                .iter()
                .filter_map(parameter_to_object_id)
                .collect::<Vec<_>>();
            Some(GameMessage::new(GameMessageType::CreateFormation(units)))
        }
        CommandType::LogicCrc => {
            let params = ordered_params(data);
            let crc = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::LogicCRC(crc)))
        }
        CommandType::SetMineClearingDetail => {
            let params = ordered_params(data);
            let detail = parameter_to_int(params.get(0)?)? as u32;
            Some(GameMessage::new(GameMessageType::SetMineClearingDetail(
                detail,
            )))
        }
        CommandType::EnableRetaliationMode => {
            let params = ordered_params(data);
            let player_id = parameter_to_int(params.get(0)?)? as u32;
            let enabled = parameter_to_bool(params.get(1)?)?;
            Some(GameMessage::new(GameMessageType::EnableRetaliationMode(
                player_id, enabled,
            )))
        }
        _ => None,
    }?;

    set_player(&mut message, player_id);
    Some(message)
}

/// Log and ignore unsupported commands.
pub fn log_unsupported_command(data: &GameCommandData) {
    warn!(
        "Unsupported network command type {} (target: {:?})",
        data.command_type, data.target_id
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_evacuate_at_location_sets_position_payload() {
        let message = GameMessage::with_player(
            GameMessageType::EvacuateAtLocation(Coord3D::new(11.0, 22.0, 1.5)),
            1,
        );

        let encoded = encode_game_message(&message).expect("evacuate should encode");

        assert_eq!(encoded.command_type, CommandType::Evacuate as u32);
        assert_eq!(encoded.position, Some((11.0, 22.0, 1.5)));
        assert!(encoded.parameters.is_empty());
    }

    #[test]
    fn encode_then_decode_evacuate_at_location_round_trip() {
        let source = GameMessage::new(GameMessageType::EvacuateAtLocation(Coord3D::new(
            15.0, 25.0, 5.0,
        )));
        let encoded = encode_game_message(&source).expect("evacuate should encode");
        assert_eq!(encoded.command_type, CommandType::Evacuate as u32);
        assert_eq!(encoded.position, Some((15.0, 25.0, 5.0)));

        let decoded =
            decode_game_command(&encoded, 3).expect("evacuate command should decode to message");
        assert_eq!(
            decoded.get_type(),
            &GameMessageType::EvacuateAtLocation(Coord3D::new(15.0, 25.0, 5.0))
        );
        assert_eq!(decoded.get_player_index(), 3);
    }

    #[test]
    fn decode_evacuate_with_position_uses_location_variant() {
        let data = GameCommandData {
            command_type: CommandType::Evacuate as u32,
            target_id: None,
            position: Some((7.0, 8.0, 9.0)),
            parameters: HashMap::new(),
            checksum: 0,
        };

        let decoded = decode_game_command(&data, 2).expect("evacuate should decode");

        assert_eq!(decoded.get_player_index(), 2);
        assert_eq!(
            decoded.get_type(),
            &GameMessageType::EvacuateAtLocation(Coord3D::new(7.0, 8.0, 9.0))
        );
    }

    #[test]
    fn decode_evacuate_without_position_keeps_plain_variant() {
        let data = GameCommandData {
            command_type: CommandType::Evacuate as u32,
            target_id: None,
            position: None,
            parameters: HashMap::new(),
            checksum: 0,
        };

        let decoded = decode_game_command(&data, 4).expect("evacuate should decode");

        assert_eq!(decoded.get_player_index(), 4);
        assert_eq!(decoded.get_type(), &GameMessageType::Evacuate);
    }
}
