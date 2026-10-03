//! Runtime CRC/Xfer and pending command storage; field order matches the existing port.

use super::{ChinookAIUpdate, ChinookFlightStatus};
use crate::ai::states::AICommandParmsStorage;
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{Coord3D, INVALID_ID};
use game_engine::common::system::{Snapshotable, Xfer};

impl Snapshotable for ChinookAIUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ ChinookAIUpdate.cpp:1329–1332 extends the SupplyTruck CRC;
        // derived save-version handling belongs only to xfer below.
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());

        let mut version: u8 = 2;
        xfer_io(xfer.xfer_version(&mut version, 2))?;

        self.base.xfer(xfer)?;

        let mut has_pending_command = self.pending_command.is_some();
        xfer_io(xfer.xfer_bool(&mut has_pending_command))?;
        if has_pending_command {
            let mut storage = self
                .pending_command
                .as_ref()
                .map(chinook_command_storage_from_params)
                .unwrap_or_else(chinook_default_command_storage);
            storage.do_xfer(xfer)?;
            if xfer.get_xfer_mode() == game_engine::common::system::XferMode::Load {
                self.pending_command = Some(chinook_command_params_from_storage(&storage));
            }
        } else if xfer.get_xfer_mode() == game_engine::common::system::XferMode::Load {
            self.pending_command = None;
        }

        let mut flight_status = self.flight_status as i32;
        xfer_io(xfer.xfer_int(&mut flight_status))?;
        if xfer.get_xfer_mode() == game_engine::common::system::XferMode::Load {
            self.flight_status = chinook_flight_status_from_i32(flight_status);
        }

        xfer_io(xfer.xfer_unsigned_int(&mut self.airfield_for_healing))?;

        if version >= 2 {
            xfer_io(xfer.xfer_real(&mut self.original_pos.x))?;
            xfer_io(xfer.xfer_real(&mut self.original_pos.y))?;
            xfer_io(xfer.xfer_real(&mut self.original_pos.z))?;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ ChinookAIUpdate.cpp:1366–1369 preserves its base callback.
        self.base.load_post_process()
    }
}

fn chinook_flight_status_from_i32(value: i32) -> ChinookFlightStatus {
    match value {
        0 => ChinookFlightStatus::TakingOff,
        2 => ChinookFlightStatus::DoingCombatDrop,
        3 => ChinookFlightStatus::Landing,
        4 => ChinookFlightStatus::Landed,
        _ => ChinookFlightStatus::Flying,
    }
}

fn chinook_default_command_storage() -> AICommandParmsStorage {
    AICommandParmsStorage {
        cmd: AiCommandType::NoCommand,
        cmd_source: CommandSourceType::FromAi,
        pos: Coord3D::ZERO,
        obj: INVALID_ID,
        other_obj: INVALID_ID,
        team_name: String::new(),
        coords: Vec::new(),
        waypoint: None,
        polygon: None,
        int_value: 0,
        damage: crate::damage::DamageInfo::new(),
        command_button: None,
        command_button_name: String::new(),
        path: None,
    }
}

fn chinook_command_storage_from_params(params: &AiCommandParams) -> AICommandParmsStorage {
    let mut storage = chinook_default_command_storage();
    storage.cmd = params.cmd;
    storage.cmd_source = params.cmd_source;
    storage.pos = params.pos;
    storage.obj = params.obj.unwrap_or(INVALID_ID);
    storage.other_obj = params.other_obj.unwrap_or(INVALID_ID);
    storage.team_name = params.team.clone().unwrap_or_default();
    storage.coords = params.coords.clone();
    storage.int_value = params.int_value;
    storage
}

fn chinook_command_params_from_storage(storage: &AICommandParmsStorage) -> AiCommandParams {
    let mut params = AiCommandParams::new(storage.cmd, storage.cmd_source);
    params.pos = storage.pos;
    if storage.obj != INVALID_ID {
        params.obj = Some(storage.obj);
    }
    if storage.other_obj != INVALID_ID {
        params.other_obj = Some(storage.other_obj);
    }
    if !storage.team_name.is_empty() {
        params.team = Some(storage.team_name.clone());
    }
    params.coords = storage.coords.clone();
    params.waypoint = storage.waypoint.as_ref().map(|waypoint| waypoint.id);
    params.polygon = storage.polygon.as_ref().map(|polygon| polygon.get_id());
    params.int_value = storage.int_value;
    params
}
