//! RailedTransportAIUpdate - AI update logic for railed transports.
//!
//! Ported from GameLogic/Object/Update/AIUpdate/RailedTransportAIUpdate.cpp.

use std::any::Any;
use std::sync::Arc;

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{AsciiString, Bool, Coord3D, Int, ObjectID, Real, UnsignedInt, WaypointID};
use crate::helpers::TheGameLogic;
use crate::modules::AIUpdateInterface;
use crate::object::update::ai_update_interface::AIUpdateModuleData;
use crate::terrain::get_terrain_logic;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use log::warn;

const INVALID_PATH: Int = -1;
const MAX_WAYPOINT_PATHS: usize = 32;

const AUTO_ACQUIRE_ENEMIES_NAMES: &[&str] = &[
    "YES",
    "STEALTHED",
    "NO",
    "NOTWHILEATTACKING",
    "ATTACK_BUILDINGS",
];

#[derive(Debug, Clone, Copy)]
struct WaypointPathInfo {
    start_waypoint_id: WaypointID,
    end_waypoint_id: WaypointID,
}

impl Default for WaypointPathInfo {
    fn default() -> Self {
        Self {
            start_waypoint_id: 0,
            end_waypoint_id: 0,
        }
    }
}

/// Module data for RailedTransportAIUpdate.
#[derive(Debug, Clone)]
pub struct RailedTransportAIUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub base: AIUpdateModuleData,
    pub path_prefix_name: AsciiString,
}

impl Default for RailedTransportAIUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: AIUpdateModuleData::default(),
            path_prefix_name: AsciiString::new(),
        }
    }
}

impl RailedTransportAIUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, RAILED_TRANSPORT_AI_UPDATE_FIELDS)
    }
}

impl ModuleData for RailedTransportAIUpdateModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }

    fn is_ai_module_data(&self) -> bool {
        true
    }
}

impl Snapshotable for RailedTransportAIUpdateModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());
        self.base.xfer(xfer)?;
        xfer_io(xfer.xfer_ascii_string(self.path_prefix_name.as_mut_string_buffer()))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn value_tokens<'a>(tokens: &'a [&'a str]) -> Vec<&'a str> {
    tokens
        .iter()
        .copied()
        .filter(|token| *token != "=")
        .collect()
}

fn parse_auto_acquire_field(
    _ini: &mut INI,
    data: &mut RailedTransportAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let values = value_tokens(tokens);
    let value = INI::parse_bit_string_32(&values, AUTO_ACQUIRE_ENEMIES_NAMES)?;
    data.base.set_auto_acquire_enemies_when_idle(value);
    Ok(())
}

fn parse_duration_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(Bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_path_prefix_name(
    _ini: &mut INI,
    data: &mut RailedTransportAIUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = required_value(tokens)?;
    data.path_prefix_name = AsciiString::from(&INI::parse_ascii_string(token)?);
    Ok(())
}

const RAILED_TRANSPORT_AI_UPDATE_FIELDS: &[FieldParse<RailedTransportAIUpdateModuleData>] = &[
    FieldParse {
        token: "AutoAcquireEnemiesWhenIdle",
        parse: parse_auto_acquire_field,
    },
    FieldParse {
        token: "MoodAttackCheckRate",
        parse: |_, data, tokens| {
            parse_duration_field(
                &mut |value| data.base.set_mood_attack_check_rate(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "SurrenderDuration",
        parse: |_, data, tokens| {
            parse_duration_field(
                &mut |value| data.base.set_surrender_duration_frames(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "ForbidPlayerCommands",
        parse: |_, data, tokens| {
            parse_bool_field(
                &mut |value| data.base.set_forbid_player_commands(value),
                tokens,
            )
        },
    },
    FieldParse {
        token: "TurretsLinked",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |value| data.base.set_turrets_linked(value), tokens)
        },
    },
    FieldParse {
        token: "PathPrefixName",
        parse: parse_path_prefix_name,
    },
];

/// Module wrapper for RailedTransportAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct RailedTransportAIUpdateModule {
    module_name_key: NameKeyType,
    data: Arc<RailedTransportAIUpdateModuleData>,
}

impl RailedTransportAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: Arc<RailedTransportAIUpdateModuleData>) -> Self {
        Self {
            module_name_key,
            data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn railed_transport_fields_accept_ini_equals_token() {
        let mut ini = INI::new();
        let mut data = RailedTransportAIUpdateModuleData::default();

        parse_auto_acquire_field(&mut ini, &mut data, &["=", "YES", "ATTACK_BUILDINGS"]).unwrap();
        parse_duration_field(
            &mut |value| data.base.set_mood_attack_check_rate(value),
            &["=", "2000"],
        )
        .unwrap();
        parse_duration_field(
            &mut |value| data.base.set_surrender_duration_frames(value),
            &["=", "3000"],
        )
        .unwrap();
        parse_bool_field(
            &mut |value| data.base.set_forbid_player_commands(value),
            &["=", "Yes"],
        )
        .unwrap();
        parse_bool_field(
            &mut |value| data.base.set_turrets_linked(value),
            &["=", "Yes"],
        )
        .unwrap();
        parse_path_prefix_name(&mut ini, &mut data, &["=", "TrainPath"]).unwrap();

        assert_eq!(data.base.auto_acquire_enemies_when_idle(), 0b10001);
        assert_eq!(data.base.mood_attack_check_rate(), 60);
        assert_eq!(data.base.surrender_duration_frames(), 90);
        assert!(data.base.forbid_player_commands());
        assert!(data.base.turrets_linked());
        assert_eq!(data.path_prefix_name.as_str(), "TrainPath");
    }
}

impl Module for RailedTransportAIUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for RailedTransportAIUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.data.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Arc::make_mut(&mut self.data).xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Runtime data for RailedTransportAIUpdate.
#[derive(Debug, Clone)]
pub struct RailedTransportAIUpdateData {
    pub path_prefix_name: AsciiString,
}

impl Default for RailedTransportAIUpdateData {
    fn default() -> Self {
        Self {
            path_prefix_name: AsciiString::new(),
        }
    }
}

/// Railed transport AI update logic.
#[derive(Debug, Clone)]
pub struct RailedTransportAIUpdate {
    data: RailedTransportAIUpdateData,
    owner_id: ObjectID,
    in_transit: Bool,
    path: [WaypointPathInfo; MAX_WAYPOINT_PATHS],
    num_paths: Int,
    current_path: Int,
    waypoint_data_loaded: Bool,
}

impl RailedTransportAIUpdate {
    pub fn new(data: RailedTransportAIUpdateData, owner_id: ObjectID) -> Self {
        Self {
            data,
            owner_id,
            in_transit: false,
            path: [WaypointPathInfo::default(); MAX_WAYPOINT_PATHS],
            num_paths: 0,
            current_path: INVALID_PATH,
            waypoint_data_loaded: false,
        }
    }

    pub fn update(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.waypoint_data_loaded {
            self.load_waypoint_data();
        }

        ai.set_ultra_accurate(true)?;

        if self.current_path == INVALID_PATH && self.num_paths > 0 {
            self.pick_and_move_to_initial_location(ai)?;
        }

        if self.in_transit {
            if self.current_path == INVALID_PATH {
                warn!("RailedTransportAIUpdate: Invalid current path.");
                return Ok(());
            }

            let Some(us) = TheGameLogic::find_object_by_id(self.owner_id) else {
                return Ok(());
            };
            let Ok(us_guard) = us.read() else {
                return Ok(());
            };

            let current_index = self.current_path as usize;
            if current_index >= self.path.len() {
                return Ok(());
            }

            let end_id = self.path[current_index].end_waypoint_id;
            let terrain = get_terrain_logic();
            let Ok(terrain_guard) = terrain.read() else {
                return Ok(());
            };
            let Some(waypoint) = terrain_guard.get_waypoint_by_id(end_id) else {
                warn!("RailedTransportAIUpdate: Invalid target waypoint.");
                return Ok(());
            };

            let start = us_guard.get_position();
            let end = waypoint.get_location();
            let v = Coord3D::new(end.x - start.x, end.y - start.y, end.z - start.z);
            let dist = v.length();
            if dist <= 5.0 || ai.is_idle() {
                self.set_in_transit(false);
            }
        }

        Ok(())
    }

    pub fn handle_execute_railed_transport(
        &mut self,
        cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.private_execute_railed_transport(cmd_source, ai);
        Ok(())
    }

    pub fn handle_evacuate(
        &mut self,
        expose_stealth_units: Int,
        cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.private_evacuate(expose_stealth_units, cmd_source, ai);
        Ok(())
    }

    fn set_in_transit(&mut self, in_transit: Bool) {
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(owner_guard) = owner.read() {
                let _ = owner_guard.with_dock_update_interface(|dock| {
                    dock.set_dock_open(!in_transit);
                });
            }
        }

        self.in_transit = in_transit;
    }

    fn load_waypoint_data(&mut self) {
        let terrain = get_terrain_logic();
        let Ok(terrain_guard) = terrain.read() else {
            return;
        };

        for i in 0..MAX_WAYPOINT_PATHS {
            let mut name = AsciiString::new();
            name.format(format_args!(
                "{}Start{:02}",
                self.data.path_prefix_name.as_str(),
                i + 1
            ));
            let start = terrain_guard.get_waypoint_by_name(&name);

            name.format(format_args!(
                "{}End{:02}",
                self.data.path_prefix_name.as_str(),
                i + 1
            ));
            let end = terrain_guard.get_waypoint_by_name(&name);

            if let (Some(start), Some(end)) = (start, end) {
                self.path[i].start_waypoint_id = start.get_id();
                self.path[i].end_waypoint_id = end.get_id();
                self.num_paths += 1;
            }
        }

        self.waypoint_data_loaded = true;
    }

    fn pick_and_move_to_initial_location(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(us) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return Ok(());
        };
        let Ok(us_guard) = us.read() else {
            return Ok(());
        };
        let our_pos = us_guard.get_position();

        let terrain = get_terrain_logic();
        let Ok(terrain_guard) = terrain.read() else {
            return Ok(());
        };

        let mut closest_path = INVALID_PATH;
        let mut closest_dist = Real::MAX;
        let mut closest_end_id = None;

        for i in 0..self.num_paths as usize {
            let index = i;
            if index >= self.path.len() {
                break;
            }

            let end_id = self.path[index].end_waypoint_id;
            let Some(waypoint) = terrain_guard.get_waypoint_by_id(end_id) else {
                continue;
            };

            let end_pos = waypoint.get_location();
            let v = Coord3D::new(
                end_pos.x - our_pos.x,
                end_pos.y - our_pos.y,
                end_pos.z - our_pos.z,
            );
            let dist = v.length();
            if dist < closest_dist {
                closest_dist = dist;
                closest_path = i as Int;
                closest_end_id = Some(end_id);
            }
        }

        let Some(closest_end_id) = closest_end_id else {
            warn!("No suitable starting waypoint path found for railed transport.");
            return Ok(());
        };

        self.ai_follow_waypoint_path(closest_end_id, CommandSourceType::FromAi, ai)?;
        self.current_path = closest_path;
        self.set_in_transit(true);

        Ok(())
    }

    fn private_execute_railed_transport(
        &mut self,
        _cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };

        let Some(is_loading) = owner_guard
            .with_railed_transport_dock_update_interface(|dock| dock.is_loading_or_unloading())
        else {
            return;
        };
        if is_loading {
            return;
        }

        if self.num_paths <= 0 {
            return;
        }

        self.current_path += 1;
        if self.current_path >= self.num_paths {
            self.current_path = 0;
        }

        let current_index = self.current_path as usize;
        if current_index >= self.path.len() {
            return;
        }

        let start_id = self.path[current_index].start_waypoint_id;
        let terrain = get_terrain_logic();
        let Ok(terrain_guard) = terrain.read() else {
            return;
        };
        if terrain_guard.get_waypoint_by_id(start_id).is_none() {
            warn!("RailedTransportAIUpdate: Start waypoint not found.");
            return;
        }

        let _ = self.ai_follow_waypoint_path(start_id, CommandSourceType::FromAi, ai);
        self.set_in_transit(true);
    }

    fn private_evacuate(
        &mut self,
        _expose_stealth_units: Int,
        _cmd_source: CommandSourceType,
        _ai: &mut dyn AIUpdateInterface,
    ) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };

        if self.in_transit {
            return;
        }

        let Some(is_loading) = owner_guard
            .with_railed_transport_dock_update_interface(|dock| dock.is_loading_or_unloading())
        else {
            return;
        };
        if is_loading {
            return;
        }

        let _ = owner_guard.with_railed_transport_dock_update_interface(|dock| {
            dock.unload_all();
        });
    }

    fn ai_follow_waypoint_path(
        &self,
        waypoint_id: WaypointID,
        cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut params = AiCommandParams::new(AiCommandType::FollowWaypointPath, cmd_source);
        params.waypoint = Some(waypoint_id);
        ai.execute_command(&params)?;
        Ok(())
    }
}
