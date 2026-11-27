//! Translate command and replay messages for the match-owned Recorder.

use super::*;
use crate::game_logic::GameMode;
use game_engine::common::message_stream::{
    Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, ICoord2D, ObjectID,
    is_network_command_message,
};

mod admitted_batch;
pub(crate) use admitted_batch::{AdmittedCommand, AdmittedCommandBatch};

/// Camera pose carried by `MSG_SET_REPLAY_CAMERA`.
/// C++ `LookAtXlat.cpp:463-467` / `GameLogicDispatch.cpp:1807-1815`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReplayCameraPose {
    pub pos: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub zoom: f32,
    /// C++ `TheMouse->getMouseCursor()` integer recorded with the pose.
    pub cursor: i32,
    /// C++ `LookAtTranslator::m_currentPos` pixel.
    pub pixel: (i32, i32),
    /// C++ `GameMessage::getPlayerIndex()` — original recorder / issuing player.
    pub player_index: i32,
}

/// Playback `MSG_CREATE/SELECT/ADD_TEAM*` for the live host control groups.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplayTeamOp {
    Create {
        player_index: i32,
        slot: u8,
        ids: Vec<ObjectId>,
    },
    Select {
        player_index: i32,
        slot: u8,
    },
    Add {
        player_index: i32,
        slot: u8,
    },
}

/// Control-group commands recorded from a live selection.
pub enum TeamSlotAction<'a> {
    Create(&'a [ObjectId]),
    Select,
    Add,
}

fn team_slot_message(player_index: i32, slot: u8, action: TeamSlotAction<'_>) -> GameMessage {
    let ty = match action {
        TeamSlotAction::Create(_) => GameMessageType::CreateTeamSlot(slot),
        TeamSlotAction::Select => GameMessageType::SelectTeamSlot(slot),
        TeamSlotAction::Add => GameMessageType::AddTeamSlot(slot),
    };
    let mut message = GameMessage::with_player(ty, player_index);
    if let TeamSlotAction::Create(ids) = action {
        for id in ids {
            message.append_object_id_argument(object_id_to_message(*id));
        }
    }
    message
}

fn intern_name(name: &str) -> u32 {
    game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name)
}

fn resolve_name(id: u32) -> String {
    game_engine::common::name_key_generator::NameKeyGenerator::key_to_name(id).unwrap_or_default()
}

fn intern_special(power: &SpecialPowerType) -> u32 {
    intern_name(&format!("SP::{power:?}"))
}

fn resolve_special(id: u32) -> SpecialPowerType {
    let name = resolve_name(id);
    if let Some(power) = crate::command_system::special_power_type_from_template_name(&name) {
        return power;
    }
    let Some(variant) = name.strip_prefix("SP::") else {
        return SpecialPowerType::Invalid;
    };
    // All host power variants are unit variants. Deserialize their existing
    // replay names directly, so IDs follow the receiving name registry even
    // after it resets; no separate registration or mutable reverse cache.
    let variant = serde::de::value::StrDeserializer::<serde::de::value::Error>::new(variant);
    SpecialPowerType::deserialize(variant).unwrap_or(SpecialPowerType::Invalid)
}

fn weapon_slot_to_id(slot: &WeaponSlot) -> u32 {
    match slot {
        WeaponSlot::Primary => 0,
        WeaponSlot::Secondary => 1,
        WeaponSlot::Tertiary => 2,
        WeaponSlot::AntiAir => 3,
        WeaponSlot::Slot(n) => *n,
    }
}

fn weapon_slot_from_id(id: u32) -> WeaponSlot {
    match id {
        0 => WeaponSlot::Primary,
        1 => WeaponSlot::Secondary,
        2 => WeaponSlot::Tertiary,
        3 => WeaponSlot::AntiAir,
        n => WeaponSlot::Slot(n),
    }
}

/// Ordered input and replay messages for the receiving world. Client
/// propagation completes before Recorder update and command dispatch.
pub struct ReplayPendingState {
    recorder: game_engine::common::recorder::Recorder,
    recorded_messages: Vec<GameMessage>,
    lifecycle_messages: Vec<GameMessage>,
    incoming: Vec<GameMessage>,
    commands: Vec<GameCommand>,
    camera: Option<ReplayCameraPose>,
    teams: Vec<ReplayTeamOp>,
    remirror: Vec<i32>,
    last_recorder_update_frame: Option<u32>,
    last_logic_crc: u32,
    last_logic_crc_frame: u32,
}

impl Default for ReplayPendingState {
    fn default() -> Self {
        Self {
            // Detached worlds do not resolve directories, map names or seeds
            // from another live application. Admission supplies these explicitly.
            recorder: game_engine::common::recorder::Recorder::new_owned(
                game_engine::common::recorder::RecorderOwnerInputs {
                    map: String::new(),
                    seed: 0,
                    replay_directory: std::path::PathBuf::new(),
                    exe_crc: 0,
                    ini_crc: 0,
                    save_stats: false,
                    stats_directory: std::path::PathBuf::new(),
                },
            ),
            recorded_messages: Vec::new(),
            lifecycle_messages: Vec::new(),
            incoming: Vec::new(),
            commands: Vec::new(),
            camera: None,
            teams: Vec::new(),
            remirror: Vec::new(),
            last_recorder_update_frame: None,
            last_logic_crc: 0,
            last_logic_crc_frame: u32::MAX,
        }
    }
}

impl ReplayPendingState {
    /// Lend the Recorder observation and the receiving input buffer for a
    /// synchronous client propagation. The callback cannot outlive the loan.
    #[cfg(feature = "game_client")]
    pub(crate) fn client_message_receiver(
        &mut self,
    ) -> (
        &game_engine::common::recorder::Recorder,
        impl FnMut(Vec<GameMessage>) -> Result<(), game_client::core::GameClientError> + '_,
    ) {
        let incoming = &mut self.incoming;
        (&self.recorder, move |messages| {
            incoming.extend(messages);
            Ok(())
        })
    }

    pub(crate) fn receive_client_messages(&mut self, messages: Vec<GameMessage>) {
        self.incoming.extend(messages);
    }

    pub fn clear(&mut self) {
        // GameLogic reset retains the admitted replay reader.
        self.incoming.clear();
        self.recorded_messages.clear();
        self.lifecycle_messages.clear();
        self.commands.clear();
        self.camera = None;
        self.teams.clear();
        self.remirror.clear();
        self.last_recorder_update_frame = None;
        self.last_logic_crc = 0;
        self.last_logic_crc_frame = u32::MAX;
    }

    pub fn take_camera(&mut self) -> Option<ReplayCameraPose> {
        self.camera.take()
    }

    pub fn take_team_ops(&mut self) -> Vec<ReplayTeamOp> {
        std::mem::take(&mut self.teams)
    }

    pub fn queue_selection_remirror(&mut self, player_index: i32) {
        if !self.remirror.contains(&player_index) {
            self.remirror.push(player_index);
        }
    }

    pub fn take_selection_remirror(&mut self) -> Vec<i32> {
        std::mem::take(&mut self.remirror)
    }

    /// A frozen step can inspect the input without taking the batch.
    pub(crate) fn receiving_clear_command_pending(&self) -> bool {
        self.incoming
            .iter()
            .any(|message| matches!(message.get_type(), GameMessageType::ClearGameData))
    }

    fn take_incoming(&mut self) -> Vec<GameMessage> {
        std::mem::take(&mut self.incoming)
    }
}

/// C++ `GameLogic.cpp` / `MessageStream.h` game-mode integers.
fn game_mode_to_new_game_code(mode: GameMode) -> i32 {
    match mode {
        GameMode::SinglePlayer => 0,
        GameMode::Multiplayer | GameMode::Lan => 1,
        GameMode::Skirmish => 2,
        GameMode::Replay => 3,
        GameMode::Shell => 4,
        GameMode::Internet => 5,
        GameMode::None => 6,
    }
}

fn object_id_from_message(id: ObjectID) -> ObjectId {
    ObjectId(id)
}

fn object_id_to_message(id: ObjectId) -> ObjectID {
    id.0
}

fn coord_from_vec3(pos: Vec3) -> Coord3D {
    Coord3D::new(pos.x, pos.y, pos.z)
}

fn vec3_from_coord(coord: &Coord3D) -> Vec3 {
    Vec3::new(coord.x, coord.y, coord.z)
}

fn keep_command_during_playback(msg: &GameMessage) -> bool {
    let ty = msg.get_type();
    !(is_network_command_message(ty) && !matches!(ty, GameMessageType::LogicCRC(_)))
}

fn replay_camera_to_message(pose: ReplayCameraPose) -> GameMessage {
    let coord = coord_from_vec3(pose.pos);
    let mut message = GameMessage::with_player(
        GameMessageType::SetReplayCamera(coord.clone(), pose.yaw, pose.zoom),
        pose.player_index,
    );
    message.append_location_argument(coord);
    message.append_real_argument(pose.yaw);
    message.append_real_argument(pose.pitch);
    message.append_real_argument(pose.zoom);
    message.append_integer_argument(pose.cursor);
    message.append_pixel_argument(ICoord2D {
        x: pose.pixel.0,
        y: pose.pixel.1,
    });
    message
}

pub(crate) fn object_ids_from_message(message: &GameMessage) -> Vec<ObjectId> {
    (0..message.get_argument_count())
        .filter_map(|index| match message.get_argument(index) {
            Some(GameMessageArgumentType::ObjectID(id)) => Some(object_id_from_message(*id)),
            _ => None,
        })
        .collect()
}

fn apply_replay_messages(state: &mut ReplayPendingState, messages: &[GameMessage]) {
    for message in messages {
        let player_index = message.get_player_index();
        match message.get_type() {
            GameMessageType::SetReplayCamera(coord, yaw, zoom) => {
                let angle = match message.get_argument(1) {
                    Some(GameMessageArgumentType::Real(value)) => *value,
                    _ => *yaw,
                };
                let pitch = match message.get_argument(2) {
                    Some(GameMessageArgumentType::Real(value)) => *value,
                    _ => 0.0,
                };
                let zoom_v = match message.get_argument(3) {
                    Some(GameMessageArgumentType::Real(value)) => *value,
                    _ => *zoom,
                };
                let cursor = match message.get_argument(4) {
                    Some(GameMessageArgumentType::Integer(value)) => *value,
                    _ => 0,
                };
                let pixel = match message.get_argument(5) {
                    Some(GameMessageArgumentType::Pixel(value)) => (value.x, value.y),
                    _ => (0, 0),
                };
                state.camera = Some(ReplayCameraPose {
                    pos: vec3_from_coord(coord),
                    yaw: angle,
                    pitch,
                    zoom: zoom_v,
                    cursor,
                    pixel,
                    player_index,
                });
            }
            GameMessageType::NewGame | GameMessageType::ClearGameData => {
                // C++ GameLogicDispatch.cpp:396-440 prepareNewGame/clearGameData.
                state.lifecycle_messages.push(message.clone());
            }
            GameMessageType::CreateTeamSlot(slot) => {
                let ids = object_ids_from_message(message);
                state.teams.push(ReplayTeamOp::Create {
                    player_index,
                    slot: *slot,
                    ids,
                });
                state.queue_selection_remirror(player_index);
            }
            GameMessageType::SelectTeamSlot(slot) => {
                state.teams.push(ReplayTeamOp::Select {
                    player_index,
                    slot: *slot,
                });
                state.queue_selection_remirror(player_index);
            }
            GameMessageType::AddTeamSlot(slot) => {
                state.teams.push(ReplayTeamOp::Add {
                    player_index,
                    slot: *slot,
                });
                state.queue_selection_remirror(player_index);
            }
            GameMessageType::LogicCRC(_) => {}
            _ => {
                if let Some(command) = game_message_to_host_command(message) {
                    state.commands.push(command);
                    state.queue_selection_remirror(player_index);
                }
            }
        }
    }
}

fn game_command_to_message(command: &GameCommand) -> Option<GameMessage> {
    use CommandType::*;
    let player = command.player_id as i32;
    let message_type = match &command.command_type {
        Move { destination } | MoveTo { destination, .. } | ForceMoveTo { destination } => {
            GameMessageType::DoMoveTo(coord_from_vec3(*destination))
        }
        AttackMoveTo { destination, .. } => {
            GameMessageType::DoAttackMoveTo(coord_from_vec3(*destination))
        }
        Attack { target_id } | AttackObject { target_id } => {
            GameMessageType::DoAttackObject(object_id_to_message(*target_id))
        }
        ForceAttackObject { target_id } => {
            GameMessageType::DoForceAttackObject(object_id_to_message(*target_id))
        }
        ForceAttackGround { location } => {
            GameMessageType::DoForceAttackGround(coord_from_vec3(*location))
        }
        Stop => GameMessageType::DoStop,
        Scatter => GameMessageType::DoScatter,
        Guard {
            target: GuardTarget::Position(pos),
            mode,
        } => GameMessageType::DoGuardPosition(coord_from_vec3(*pos), *mode as i32),
        Guard {
            target: GuardTarget::Object(id),
            mode,
        } => GameMessageType::DoGuardObject(object_id_to_message(*id), *mode as i32),
        AddWaypoint { destination } => GameMessageType::AddWaypoint(coord_from_vec3(*destination)),
        CreateSelectedGroup { create_new, units } => GameMessageType::CreateSelectedGroup(
            *create_new,
            units.iter().copied().map(object_id_to_message).collect(),
        ),
        Enter { target_id } => GameMessageType::Enter(0, object_id_to_message(*target_id)),
        Dock { target_id } => GameMessageType::Dock(object_id_to_message(*target_id)),
        Repair { target_id } => GameMessageType::DoRepair(object_id_to_message(*target_id)),
        GetRepaired { target_id } => GameMessageType::GetRepaired(object_id_to_message(*target_id)),
        GetHealed { target_id } => GameMessageType::GetHealed(object_id_to_message(*target_id)),
        ResumeConstruction { target_id } => {
            GameMessageType::ResumeConstruction(object_id_to_message(*target_id))
        }
        DoSalvage { destination } => GameMessageType::DoSalvage(coord_from_vec3(*destination)),
        EnableRetaliationMode {
            player_index,
            enabled,
        } => GameMessageType::EnableRetaliationMode(*player_index, *enabled),
        SelfDestruct { transfer_to_ally } => {
            GameMessageType::SelfDestruct(if *transfer_to_ally { 1 } else { 0 })
        }
        Build {
            template_name,
            location,
        }
        | DozerConstruct {
            template_name,
            location,
            ..
        } => GameMessageType::DozerConstruct(
            intern_name(template_name),
            coord_from_vec3(*location),
            match &command.command_type {
                DozerConstruct { orientation, .. } => *orientation,
                _ => 0.0,
            },
        ),
        DozerConstructLine {
            template_name,
            start,
            end,
        } => GameMessageType::DozerConstructLine(
            intern_name(template_name),
            coord_from_vec3(*start),
            coord_from_vec3(*end),
            (end.z - start.z).atan2(end.x - start.x),
        ),
        DozerConstructLineFacing {
            template_name,
            start,
            end,
            orientation,
        } => GameMessageType::DozerConstructLine(
            intern_name(template_name),
            coord_from_vec3(*start),
            coord_from_vec3(*end),
            *orientation,
        ),
        DozerCancelConstruct { object_id } => {
            GameMessageType::DozerCancelConstruct(object_id_to_message(*object_id))
        }
        Sell { object_id } => GameMessageType::Sell(object_id_to_message(*object_id)),
        QueueUnitCreate {
            template_name,
            quantity,
        } => GameMessageType::QueueUnitCreate(intern_name(template_name), *quantity),
        CancelUnitCreate { template_name } => {
            GameMessageType::CancelUnitCreate(intern_name(template_name))
        }
        QueueUpgrade { upgrade_name } => GameMessageType::QueueUpgrade(intern_name(upgrade_name)),
        CancelUpgrade { upgrade_name } => GameMessageType::CancelUpgrade(intern_name(upgrade_name)),
        PurchaseScience { science_name } => {
            GameMessageType::PurchaseScience(intern_name(science_name))
        }
        DoSpecialPower { power_type, target } => {
            let power_id = intern_special(power_type);
            match target {
                PowerTarget::None => GameMessageType::DoSpecialPower(power_id, 0, 0),
                PowerTarget::Object(id) => GameMessageType::DoSpecialPowerAtObject(
                    power_id,
                    object_id_to_message(*id),
                    0,
                    0,
                ),
                PowerTarget::Location(pos) => GameMessageType::DoSpecialPowerAtLocation(
                    power_id,
                    coord_from_vec3(*pos),
                    0.0,
                    0,
                    0,
                    0,
                ),
                PowerTarget::LocationFacing { pos, angle } => {
                    GameMessageType::DoSpecialPowerAtLocation(
                        power_id,
                        coord_from_vec3(*pos),
                        *angle,
                        0,
                        0,
                        0,
                    )
                }
            }
        }
        DoSpecialPowerWithMetadata {
            power_type: _,
            special_power_name,
            command_button_name: _,
            options_bits,
            source_object_id,
            target,
        } => {
            let power_id = intern_name(special_power_name);
            match target {
                PowerTarget::LocationFacing { pos, angle } => {
                    GameMessageType::DoSpecialPowerAtLocation(
                        power_id,
                        coord_from_vec3(*pos),
                        *angle,
                        0,
                        *options_bits,
                        *source_object_id,
                    )
                }
                PowerTarget::Location(pos) => GameMessageType::DoSpecialPowerAtLocation(
                    power_id,
                    coord_from_vec3(*pos),
                    0.0,
                    0,
                    *options_bits,
                    *source_object_id,
                ),
                PowerTarget::Object(id) => GameMessageType::DoSpecialPowerAtObject(
                    power_id,
                    object_id_to_message(*id),
                    *options_bits,
                    *source_object_id,
                ),
                PowerTarget::None => {
                    GameMessageType::DoSpecialPower(power_id, *options_bits, *source_object_id)
                }
            }
        }
        DoWeapon {
            weapon_slot,
            target,
            ..
        } => {
            let slot = weapon_slot_to_id(weapon_slot);
            match target {
                WeaponTarget::Location(pos) => {
                    GameMessageType::DoWeaponAtLocation(slot, coord_from_vec3(*pos))
                }
                WeaponTarget::Object(id) => {
                    GameMessageType::DoWeaponAtObject(slot, object_id_to_message(*id))
                }
            }
        }
        Evacuate => GameMessageType::Evacuate,
        CombatDrop {
            target: DropTarget::Location(pos),
        } => GameMessageType::CombatDropAtLocation(coord_from_vec3(*pos)),
        CombatDrop {
            target: DropTarget::Object(id),
        } => GameMessageType::CombatDropAtObject(object_id_to_message(*id)),
        SetRallyPoint { location } => {
            let unit = command
                .selected_units
                .first()
                .copied()
                .map(object_id_to_message)
                .unwrap_or(0);
            GameMessageType::SetRallyPoint(unit, coord_from_vec3(*location))
        }
        Cheer => GameMessageType::DoCheer,
        PlaceBeacon { location, .. } => GameMessageType::PlaceBeacon(coord_from_vec3(*location)),
        RemoveBeacon => GameMessageType::RemoveBeacon(Coord3D::new(0.0, 0.0, 0.0)),
        SetBeaconText { text } => {
            GameMessageType::SetBeaconText(Coord3D::new(0.0, 0.0, 0.0), text.clone())
        }
        ExecuteRailedTransport => GameMessageType::ExecuteRailedTransport,
        HackInternet => GameMessageType::InternetHack,
        ToggleOvercharge => GameMessageType::ToggleOvercharge,
        SwitchWeapons { slot } => GameMessageType::SwitchWeapons(u32::from(*slot)),
        DestroySelectedGroup { team_id } => GameMessageType::DestroySelectedGroup(*team_id),
        RemoveFromSelectedGroup { units } => GameMessageType::RemoveFromSelectedGroup(
            units.iter().copied().map(object_id_to_message).collect(),
        ),
        CreateFormation => GameMessageType::CreateFormation(
            command
                .selected_units
                .iter()
                .copied()
                .map(object_id_to_message)
                .collect(),
        ),
        Exit => GameMessageType::Exit(0),
        _ => return None,
    };
    Some(GameMessage::with_player(message_type, player))
}

fn game_message_to_host_command(message: &GameMessage) -> Option<GameCommand> {
    use GameMessageType::*;
    let command_type = match message.get_type() {
        DoMoveTo(coord) => CommandType::MoveTo {
            destination: vec3_from_coord(coord),
            waypoints: Vec::new(),
        },
        DoAttackMoveTo(coord) => CommandType::AttackMoveTo {
            destination: vec3_from_coord(coord),
            max_shots: -1,
        },
        DoForceMoveTO(coord) => CommandType::ForceMoveTo {
            destination: vec3_from_coord(coord),
        },
        DoAttackObject(id) => CommandType::AttackObject {
            target_id: object_id_from_message(*id),
        },
        DoForceAttackObject(id) => CommandType::ForceAttackObject {
            target_id: object_id_from_message(*id),
        },
        DoForceAttackGround(coord) => CommandType::ForceAttackGround {
            location: vec3_from_coord(coord),
        },
        DoStop => CommandType::Stop,
        DoScatter => CommandType::Scatter,
        DoGuardPosition(coord, mode) => CommandType::Guard {
            target: GuardTarget::Position(vec3_from_coord(coord)),
            mode: guard_mode_from_i32(*mode),
        },
        DoGuardObject(id, mode) => CommandType::Guard {
            target: GuardTarget::Object(object_id_from_message(*id)),
            mode: guard_mode_from_i32(*mode),
        },
        CreateSelectedGroup(create_new, units) | CreateSelectedGroupNoSound(create_new, units) => {
            CommandType::CreateSelectedGroup {
                create_new: *create_new,
                units: units.iter().copied().map(object_id_from_message).collect(),
            }
        }
        Enter(_selector, id) => CommandType::Enter {
            target_id: object_id_from_message(*id),
        },
        Dock(id) => CommandType::Dock {
            target_id: object_id_from_message(*id),
        },
        DoRepair(id) => CommandType::Repair {
            target_id: object_id_from_message(*id),
        },
        GetRepaired(id) => CommandType::GetRepaired {
            target_id: object_id_from_message(*id),
        },
        GetHealed(id) => CommandType::GetHealed {
            target_id: object_id_from_message(*id),
        },
        ResumeConstruction(id) => CommandType::ResumeConstruction {
            target_id: object_id_from_message(*id),
        },
        DoSalvage(coord) => CommandType::DoSalvage {
            destination: vec3_from_coord(coord),
        },
        EnableRetaliationMode(player_index, enabled) => CommandType::EnableRetaliationMode {
            player_index: *player_index,
            enabled: *enabled,
        },
        SelfDestruct(flag) => CommandType::SelfDestruct {
            transfer_to_ally: *flag != 0,
        },
        DozerConstruct(building_type, coord, angle) => CommandType::DozerConstruct {
            template_name: resolve_name(*building_type),
            location: vec3_from_coord(coord),
            orientation: *angle,
        },
        DozerConstructLine(building_type, start, end, angle) => {
            CommandType::DozerConstructLineFacing {
                template_name: resolve_name(*building_type),
                start: vec3_from_coord(start),
                end: vec3_from_coord(end),
                orientation: *angle,
            }
        }
        DozerCancelConstruct(id) => CommandType::DozerCancelConstruct {
            object_id: object_id_from_message(*id),
        },
        Sell(id) => CommandType::Sell {
            object_id: object_id_from_message(*id),
        },
        QueueUnitCreate(unit_type_id, quantity) => CommandType::QueueUnitCreate {
            template_name: resolve_name(*unit_type_id),
            quantity: *quantity,
        },
        CancelUnitCreate(unit_type_id) => CommandType::CancelUnitCreate {
            template_name: resolve_name(*unit_type_id),
        },
        QueueUpgrade(upgrade_id) => CommandType::QueueUpgrade {
            upgrade_name: resolve_name(*upgrade_id),
        },
        CancelUpgrade(upgrade_id) => CommandType::CancelUpgrade {
            upgrade_name: resolve_name(*upgrade_id),
        },
        PurchaseScience(science_id) => CommandType::PurchaseScience {
            science_name: resolve_name(*science_id),
        },
        DoSpecialPower(power_id, options, source) => CommandType::DoSpecialPowerWithMetadata {
            power_type: resolve_special(*power_id),
            special_power_name: resolve_name(*power_id),
            command_button_name: String::new(),
            options_bits: *options,
            source_object_id: *source,
            target: PowerTarget::None,
        },
        DoSpecialPowerAtLocation(power_id, coord, angle, _in_way, options, source) => {
            CommandType::DoSpecialPowerWithMetadata {
                power_type: resolve_special(*power_id),
                special_power_name: resolve_name(*power_id),
                command_button_name: String::new(),
                options_bits: *options,
                source_object_id: *source,
                target: PowerTarget::from_location_and_angle(vec3_from_coord(coord), *angle),
            }
        }
        DoSpecialPowerAtObject(power_id, target, options, source) => {
            CommandType::DoSpecialPowerWithMetadata {
                power_type: resolve_special(*power_id),
                special_power_name: resolve_name(*power_id),
                command_button_name: String::new(),
                options_bits: *options,
                source_object_id: *source,
                target: PowerTarget::Object(object_id_from_message(*target)),
            }
        }
        DoWeaponAtLocation(slot, coord) => CommandType::DoWeapon {
            weapon_slot: weapon_slot_from_id(*slot),
            max_shots_to_fire: -1,
            target: WeaponTarget::Location(vec3_from_coord(coord)),
        },
        DoWeaponAtObject(slot, id) => CommandType::DoWeapon {
            weapon_slot: weapon_slot_from_id(*slot),
            max_shots_to_fire: -1,
            target: WeaponTarget::Object(object_id_from_message(*id)),
        },
        Evacuate | EvacuateAtLocation(_) => CommandType::Evacuate,
        CombatDropAtLocation(coord) => CommandType::CombatDrop {
            target: DropTarget::Location(vec3_from_coord(coord)),
        },
        CombatDropAtObject(id) => CommandType::CombatDrop {
            target: DropTarget::Object(object_id_from_message(*id)),
        },
        SetRallyPoint(_unit, coord) => CommandType::SetRallyPoint {
            location: vec3_from_coord(coord),
        },
        DoCheer => CommandType::Cheer,
        PlaceBeacon(coord) => CommandType::PlaceBeacon {
            location: vec3_from_coord(coord),
            text: String::new(),
        },
        RemoveBeacon(_) => CommandType::RemoveBeacon,
        SetBeaconText(_coord, text) => CommandType::SetBeaconText { text: text.clone() },
        ExecuteRailedTransport => CommandType::ExecuteRailedTransport,
        InternetHack => CommandType::HackInternet,
        ToggleOvercharge => CommandType::ToggleOvercharge,
        SwitchWeapons(slot) => CommandType::SwitchWeapons {
            slot: u8::try_from(*slot).unwrap_or(0),
        },
        DestroySelectedGroup(team_id) => CommandType::DestroySelectedGroup { team_id: *team_id },
        RemoveFromSelectedGroup(units) => CommandType::RemoveFromSelectedGroup {
            units: units.iter().copied().map(object_id_from_message).collect(),
        },
        CreateFormation(_) => CommandType::CreateFormation,
        Exit(_) => CommandType::Exit,
        _ => return None,
    };
    let selected_units = match &command_type {
        CommandType::DoSpecialPowerWithMetadata {
            source_object_id, ..
        } if *source_object_id != 0 => vec![ObjectId(*source_object_id)],
        _ => Vec::new(),
    };
    Some(GameCommand {
        command_type,
        player_id: message.get_player_index() as u32,
        command_id: 0,
        timestamp: SystemTime::now(),
        selected_units,
        modifier_keys: ModifierKeys::default(),
    })
}

fn guard_mode_from_i32(mode: i32) -> crate::game_logic::GuardMode {
    match mode {
        1 => crate::game_logic::GuardMode::WithoutPursuit,
        2 => crate::game_logic::GuardMode::FlyingUnitsOnly,
        _ => crate::game_logic::GuardMode::Normal,
    }
}

#[cfg(test)]
#[path = "record_tap_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "record_tap_special_power_tests.rs"]
mod special_power_tests;

#[cfg(test)]
#[path = "record_tap/line_build_input_tests.rs"]
mod line_build_input_tests;

#[path = "record_tap/owner.rs"]
mod owner;
