//! SidesList - player/team side definitions with build lists.
//!
//! Port of C++ SidesList.{h,cpp} for map/player side metadata.

use crate::build_list_info::BuildListInfo;
use crate::common::well_known_keys::{
    key_player_allies, key_player_display_name, key_player_enemies, key_player_faction,
    key_player_is_human, key_player_name, key_team_is_singleton, key_team_name, key_team_owner,
};
use crate::common::xfer::Xfer;
use crate::common::Coord3D;
use crate::common::{AsciiString, Bool, Dict, Int, Snapshot, UnicodeString, MAX_PLAYER_COUNT};
use crate::scripting::{
    parse_player_scripts_list_chunk, ScriptList, ScriptListReadInfo, XferSnapshot,
};
use crate::system::game_logic::SubsystemInterface;
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use game_engine::common::system::{DataChunkInfo, DataChunkInput, DataChunkOutput};
use game_engine::system::XferVersion;
use once_cell::sync::Lazy;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, Default)]
pub struct SidesInfo {
    build_list: Option<Box<BuildListInfo>>,
    dict: Dict,
    scripts: Option<Box<ScriptList>>,
}

impl SidesInfo {
    pub fn init(&mut self, dict: Option<&Dict>) {
        self.build_list = None;
        self.scripts = None;
        self.dict.clear();
        if let Some(source) = dict {
            self.dict = source.clone();
        }
    }

    pub fn clear(&mut self) {
        self.init(None);
    }

    pub fn get_dict(&self) -> &Dict {
        &self.dict
    }

    pub fn get_dict_mut(&mut self) -> &mut Dict {
        &mut self.dict
    }

    pub fn add_to_build_list(&mut self, mut entry: BuildListInfo, position: Int) {
        entry.set_next_build_list_boxed(None);
        let position = position.max(0) as usize;

        if self.build_list.is_none() || position == 0 {
            let existing = self.build_list.take();
            entry.set_next_build_list_boxed(existing);
            self.build_list = Some(Box::new(entry));
            return;
        }

        let mut index = 0usize;
        let mut current = self.build_list.as_mut().map(|node| node.as_mut());
        while let Some(node) = current {
            if index + 1 >= position {
                let tail = node.take_next_build_list();
                entry.set_next_build_list_boxed(tail);
                node.set_next_build_list_boxed(Some(Box::new(entry)));
                return;
            }
            index += 1;
            current = node.get_next_mut();
        }

        // Append to end if position exceeds list size.
        fn append_to_end(node: &mut BuildListInfo, entry: Box<BuildListInfo>) {
            if let Some(next) = node.get_next_mut() {
                append_to_end(next, entry);
            } else {
                node.set_next_build_list_boxed(Some(entry));
            }
        }

        if let Some(node) = self.build_list.as_mut().map(|n| n.as_mut()) {
            append_to_end(node, Box::new(entry));
        } else {
            self.build_list = Some(Box::new(entry));
        }
    }

    pub fn remove_from_build_list_at(&mut self, position: usize) -> Option<BuildListInfo> {
        if position == 0 {
            let mut head = self.build_list.take()?;
            let next = head.take_next_build_list();
            self.build_list = next;
            return Some(*head);
        }

        let mut index = 0usize;
        let mut current = self.build_list.as_mut().map(|node| node.as_mut());
        while let Some(node) = current {
            if index + 1 == position {
                let mut target = node.take_next_build_list()?;
                let next = target.take_next_build_list();
                node.set_next_build_list_boxed(next);
                return Some(*target);
            }
            index += 1;
            current = node.get_next_mut();
        }
        None
    }

    pub fn reorder_in_build_list(&mut self, from_position: usize, to_position: Int) {
        if let Some(entry) = self.remove_from_build_list_at(from_position) {
            self.add_to_build_list(entry, to_position);
        }
    }

    pub fn get_build_list(&self) -> Option<&BuildListInfo> {
        self.build_list.as_deref()
    }

    pub fn get_build_list_mut(&mut self) -> Option<&mut BuildListInfo> {
        self.build_list.as_deref_mut()
    }

    pub fn release_build_list(&mut self) {
        self.build_list = None;
    }

    pub fn take_build_list(&mut self) -> Option<Box<BuildListInfo>> {
        self.build_list.take()
    }

    pub fn get_script_list(&self) -> Option<&ScriptList> {
        self.scripts.as_deref()
    }

    pub fn set_script_list(&mut self, scripts: Option<Box<ScriptList>>) {
        self.scripts = scripts;
    }
}

#[derive(Debug, Clone, Default)]
pub struct TeamsInfo {
    dict: Dict,
}

impl TeamsInfo {
    pub fn init(&mut self, dict: Option<&Dict>) {
        self.dict.clear();
        if let Some(source) = dict {
            self.dict = source.clone();
        }
    }

    pub fn clear(&mut self) {
        self.init(None);
    }

    pub fn get_dict(&self) -> &Dict {
        &self.dict
    }

    pub fn get_dict_mut(&mut self) -> &mut Dict {
        &mut self.dict
    }
}

#[derive(Debug, Clone, Default)]
pub struct TeamsInfoRec {
    teams: Vec<TeamsInfo>,
}

impl TeamsInfoRec {
    pub fn clear(&mut self) {
        self.teams.clear();
    }

    pub fn get_num_teams(&self) -> usize {
        self.teams.len()
    }

    pub fn add_team(&mut self, dict: &Dict) {
        let mut team = TeamsInfo::default();
        team.init(Some(dict));
        self.teams.push(team);
    }

    pub fn remove_team(&mut self, index: usize) {
        if index < self.teams.len() {
            self.teams.remove(index);
        }
    }

    pub fn get_team_info(&self, index: usize) -> Option<&TeamsInfo> {
        self.teams.get(index)
    }

    pub fn get_team_info_mut(&mut self, index: usize) -> Option<&mut TeamsInfo> {
        self.teams.get_mut(index)
    }

    pub fn find_team_info(&self, name: &str) -> Option<usize> {
        self.teams
            .iter()
            .position(|team| team.get_dict().get_ascii_string(key_team_name()) == name)
    }
}

#[derive(Debug, Default)]
pub struct SidesList {
    sides: Vec<SidesInfo>,
    skirmish_sides: Vec<SidesInfo>,
    teamrec: TeamsInfoRec,
    skirmish_teamrec: TeamsInfoRec,
}

impl SidesList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.clear();
    }

    pub fn clear(&mut self) {
        self.empty_sides();
        self.empty_teams();
    }

    pub fn get_num_sides(&self) -> usize {
        self.sides.len()
    }

    pub fn get_num_skirmish_sides(&self) -> usize {
        self.skirmish_sides.len()
    }

    pub fn empty_sides(&mut self) {
        self.sides.clear();
        self.skirmish_sides.clear();
    }

    pub fn empty_teams(&mut self) {
        self.teamrec.clear();
        self.skirmish_teamrec.clear();
    }

    pub fn add_side(&mut self, dict: &Dict) {
        if self.sides.len() >= MAX_PLAYER_COUNT {
            return;
        }
        let mut side = SidesInfo::default();
        side.init(Some(dict));
        self.sides.push(side);
    }

    pub fn remove_side(&mut self, index: usize) {
        if self.sides.len() <= 1 || index >= self.sides.len() {
            return;
        }
        self.sides.remove(index);
    }

    pub fn get_side_info(&self, index: usize) -> Option<&SidesInfo> {
        self.sides.get(index)
    }

    pub fn get_side_info_mut(&mut self, index: usize) -> Option<&mut SidesInfo> {
        self.sides.get_mut(index)
    }

    pub fn get_skirmish_side_info(&self, index: usize) -> Option<&SidesInfo> {
        self.skirmish_sides.get(index)
    }

    pub fn get_skirmish_side_info_mut(&mut self, index: usize) -> Option<&mut SidesInfo> {
        self.skirmish_sides.get_mut(index)
    }

    pub fn find_side_info(&self, name: &str) -> Option<usize> {
        self.sides
            .iter()
            .position(|side| side.get_dict().get_ascii_string(key_player_name()) == name)
    }

    pub fn find_skirmish_side_info(&self, name: &str) -> Option<usize> {
        self.skirmish_sides
            .iter()
            .position(|side| side.get_dict().get_ascii_string(key_player_name()) == name)
    }

    pub fn add_team(&mut self, dict: &Dict) {
        self.teamrec.add_team(dict);
    }

    pub fn add_skirmish_team(&mut self, dict: &Dict) {
        self.skirmish_teamrec.add_team(dict);
    }

    pub fn remove_team(&mut self, index: usize) {
        self.teamrec.remove_team(index);
    }

    pub fn get_num_teams(&self) -> usize {
        self.teamrec.get_num_teams()
    }

    pub fn get_num_skirmish_teams(&self) -> usize {
        self.skirmish_teamrec.get_num_teams()
    }

    pub fn get_team_info(&self, index: usize) -> Option<&TeamsInfo> {
        self.teamrec.get_team_info(index)
    }

    pub fn get_team_info_mut(&mut self, index: usize) -> Option<&mut TeamsInfo> {
        self.teamrec.get_team_info_mut(index)
    }

    pub fn get_skirmish_team_info(&self, index: usize) -> Option<&TeamsInfo> {
        self.skirmish_teamrec.get_team_info(index)
    }

    pub fn find_team_info(&self, name: &str) -> Option<usize> {
        self.teamrec.find_team_info(name)
    }

    pub fn prepare_for_mp_or_skirmish(&mut self) {
        self.skirmish_teamrec.clear();
        for team in &self.teamrec.teams {
            self.skirmish_teamrec.add_team(team.get_dict());
        }
        self.teamrec.clear();

        self.skirmish_sides.clear();
        let mut index = 0usize;
        while index < self.sides.len() {
            let side = self.sides[index].clone();
            self.skirmish_sides.push(side);

            let faction = self.sides[index]
                .get_dict()
                .get_ascii_string(key_player_faction());
            if faction == "FactionCivilian" {
                index += 1;
                continue;
            }
            if self.sides.len() == 1 {
                break;
            }
            self.sides.remove(index);
        }

        let mut got_scripts = false;
        for side in &self.skirmish_sides {
            let faction = side.get_dict().get_ascii_string(key_player_faction());
            if faction == "FactionCivilian" {
                continue;
            }
            if let Some(scripts) = side.get_script_list() {
                if scripts.get_script().is_some() || scripts.get_script_group().is_some() {
                    got_scripts = true;
                    break;
                }
            }
        }

        if !got_scripts {
            let path = "Data\\Scripts\\SkirmishScripts.scb";
            self.skirmish_teamrec.clear();
            let data = {
                let file_system_arc = get_file_system();
                let mut file_system = file_system_arc.lock().expect("FileSystem mutex poisoned");
                let mut file = match file_system
                    .open_file(path, FileAccess::READ.combine(FileAccess::BINARY))
                {
                    Some(file) => file,
                    None => return,
                };
                match file.read_entire_and_close() {
                    Ok(data) => data,
                    Err(_) => return,
                }
            };
            let mut input = DataChunkInput::new(data);
            let mut context = SkirmishScriptContext::default();
            input.register_parser("PlayerScriptsList", "", parse_player_scripts_list_chunk);
            input.register_parser("ScriptsPlayers", "", parse_players_data_chunk);
            input.register_parser("ScriptTeams", "", parse_teams_data_chunk);
            if !input.parse(&mut context) {
                return;
            }

            for (index, scripts) in context.script_lists.lists.into_iter().enumerate() {
                if let Some(name) = context.player_names.get(index) {
                    if let Some(side_index) = self.skirmish_sides.iter().position(|side| {
                        side.get_dict().get_ascii_string(key_player_name()) == *name
                    }) {
                        self.skirmish_sides[side_index].set_script_list(Some(scripts));
                    }
                }
            }

            for team_dict in context.team_dicts {
                let owner = team_dict.get_ascii_string(key_team_owner());
                if self.find_skirmish_side_info(&owner).is_some() {
                    self.add_skirmish_team(&team_dict);
                }
            }
        }
    }

    pub fn is_player_default_team(&self, team: &TeamsInfo) -> bool {
        let team_name = team.get_dict().get_ascii_string(key_team_name());
        if let Some(rest) = team_name.strip_prefix("team") {
            for side in &self.sides {
                let player_name = side.get_dict().get_ascii_string(key_player_name());
                if player_name == rest {
                    return true;
                }
            }
        }
        false
    }

    pub fn add_player_by_template(&mut self, player_template_name: &AsciiString) {
        let (player_name, display_name, is_human) = if player_template_name.is_empty() {
            ("".to_string(), "Neutral".to_string(), false)
        } else {
            let mut name = String::from("Plyr");
            if player_template_name.starts_with("Faction") {
                name.push_str(&player_template_name[7..]);
            } else {
                name.push_str(player_template_name);
            }
            let is_human = name != "PlyrCivilian";
            (name.clone(), name, is_human)
        };

        let mut dict = Dict::new();
        dict.set_ascii_string(key_player_name(), player_name.clone());
        dict.set_bool(key_player_is_human(), is_human);
        dict.set_unicode_string(key_player_display_name(), display_name);
        dict.set_ascii_string(key_player_faction(), player_template_name.as_str());
        dict.set_ascii_string(key_player_allies(), String::new());
        dict.set_ascii_string(key_player_enemies(), String::new());
        self.add_side(&dict);

        let mut team_dict = Dict::new();
        let mut team_name = String::from("team");
        team_name.push_str(&player_name);
        team_dict.set_ascii_string(key_team_name(), team_name);
        team_dict.set_ascii_string(key_team_owner(), player_name);
        team_dict.set_bool(key_team_is_singleton(), true);
        self.add_team(&team_dict);
    }

    pub fn validate_sides(&mut self) -> bool {
        let mut modified = false;

        let neutral = self.sides.iter().position(|side| {
            side.get_dict()
                .get_ascii_string(key_player_name())
                .is_empty()
        });
        if neutral.is_none() {
            self.add_player_by_template(&AsciiString::new());
            modified = true;
        }

        let side_names: Vec<String> = self
            .sides
            .iter()
            .map(|side| side.get_dict().get_ascii_string(key_player_name()))
            .collect();
        let mut pending_teams: Vec<Dict> = Vec::new();

        for side_index in 0..self.sides.len() {
            let player_name = self.sides[side_index]
                .get_dict()
                .get_ascii_string(key_player_name());
            let mut default_team = String::from("team");
            default_team.push_str(&player_name);
            if let Some(index) =
                self.teamrec.teams.iter().position(|team| {
                    team.get_dict().get_ascii_string(key_team_name()) == default_team
                })
            {
                if let Some(team) = self.teamrec.teams.get_mut(index) {
                    if team.get_dict().get_ascii_string(key_team_owner()) != player_name {
                        team.get_dict_mut()
                            .set_ascii_string(key_team_owner(), player_name.clone());
                        modified = true;
                    }
                    if !team.get_dict().get_bool(key_team_is_singleton()) {
                        team.get_dict_mut().set_bool(key_team_is_singleton(), true);
                        modified = true;
                    }
                }
            } else {
                let mut dict = Dict::new();
                dict.set_ascii_string(key_team_name(), default_team);
                dict.set_ascii_string(key_team_owner(), player_name.clone());
                dict.set_bool(key_team_is_singleton(), true);
                pending_teams.push(dict);
                modified = true;
            }

            let allies = self.sides[side_index]
                .get_dict()
                .get_ascii_string(key_player_allies());
            let enemies = self.sides[side_index]
                .get_dict()
                .get_ascii_string(key_player_enemies());
            let mut new_allies = allies.clone();
            let mut new_enemies = enemies.clone();

            if Self::validate_ally_enemy_list(&player_name, &allies, &mut new_allies, &side_names) {
                self.sides[side_index]
                    .get_dict_mut()
                    .set_ascii_string(key_player_allies(), new_allies);
                modified = true;
            }

            if Self::validate_ally_enemy_list(&player_name, &enemies, &mut new_enemies, &side_names)
            {
                self.sides[side_index]
                    .get_dict_mut()
                    .set_ascii_string(key_player_enemies(), new_enemies);
                modified = true;
            }
        }

        for dict in pending_teams {
            self.add_team(&dict);
        }

        let mut index = 0usize;
        while index < self.teamrec.teams.len() {
            let team_name = self.teamrec.teams[index]
                .get_dict()
                .get_ascii_string(key_team_name());
            if side_names.iter().any(|name| name == &team_name) {
                self.remove_team(index);
                modified = true;
                continue;
            }
            index += 1;
        }

        for team in &mut self.teamrec.teams {
            let team_name = team.get_dict().get_ascii_string(key_team_name());
            let owner = team.get_dict().get_ascii_string(key_team_owner());
            if !side_names.iter().any(|name| name == &owner) || owner == team_name {
                team.get_dict_mut()
                    .set_ascii_string(key_team_owner(), String::new());
                modified = true;
            }
        }

        modified
    }

    fn validate_ally_enemy_list(
        player_name: &str,
        list: &str,
        output: &mut String,
        side_names: &[String],
    ) -> bool {
        let mut modified = false;
        let mut filtered = Vec::new();
        for token in list.split_whitespace() {
            if token == player_name {
                modified = true;
                continue;
            }
            if !side_names.iter().any(|name| name == token) {
                modified = true;
                continue;
            }
            filtered.push(token);
        }
        *output = filtered.join(" ");
        modified
    }

    fn parse_sides_data_chunk_internal(
        &mut self,
        input: &mut DataChunkInput,
        info: &DataChunkInfo,
        parse_scripts: bool,
    ) -> bool {
        self.clear();

        let count = input.read_int().max(0) as usize;
        for side_index in 0..count.min(MAX_PLAYER_COUNT) {
            let dict = input.read_dict();
            self.add_side(&dict);

            let build_count = input.read_int().max(0) as usize;
            for pos in 0..build_count {
                let mut build = BuildListInfo::new();
                let building_name = input.read_ascii_string();
                build.set_building_name(AsciiString::from(building_name.as_str()));
                let template_name = input.read_ascii_string();
                build.set_template_name(AsciiString::from(template_name.as_str()));
                let x = input.read_real();
                let y = input.read_real();
                let _z = input.read_real();
                let loc = Coord3D::new(x, y, 0.0);
                build.set_location(loc);
                build.set_angle(input.read_real());
                build.set_initially_built(input.read_byte() != 0);
                build.set_num_rebuilds(input.read_int() as u32);

                if info.version >= 3 {
                    let script_name = input.read_ascii_string();
                    build.set_script(AsciiString::from(script_name.as_str()));
                    build.set_health(input.read_int());
                    build.set_whiner(input.read_byte() != 0);
                    build.set_unsellable(input.read_byte() != 0);
                    build.set_repairable(input.read_byte() != 0);
                }

                if let Some(side) = self.get_side_info_mut(side_index) {
                    side.add_to_build_list(build, pos as Int);
                }
            }
        }

        if info.version >= 2 {
            let team_count = input.read_int().max(0) as usize;
            for _ in 0..team_count {
                let dict = input.read_dict();
                self.add_team(&dict);
            }
        }

        if parse_scripts {
            let mut script_read_info = ScriptListReadInfo::default();
            input.register_parser(
                "PlayerScriptsList",
                &info.label,
                parse_player_scripts_list_chunk,
            );
            if !input.parse(&mut script_read_info) {
                return false;
            }

            for (index, scripts) in script_read_info.lists.into_iter().enumerate() {
                if let Some(side) = self.get_side_info_mut(index) {
                    side.set_script_list(Some(scripts));
                }
            }
        }

        self.validate_sides();
        true
    }

    pub fn parse_sides_data_chunk(
        &mut self,
        input: &mut DataChunkInput,
        info: &DataChunkInfo,
    ) -> bool {
        self.parse_sides_data_chunk_internal(input, info, true)
    }

    pub fn parse_sides_data_chunk_without_scripts(
        &mut self,
        input: &mut DataChunkInput,
        info: &DataChunkInfo,
    ) -> bool {
        self.parse_sides_data_chunk_internal(input, info, false)
    }

    pub fn write_sides_data_chunk(&mut self, output: &mut DataChunkOutput) {
        output.open_data_chunk("SidesList", 3);

        output.write_int(self.get_num_sides() as i32);
        for side in &self.sides {
            output.write_dict(side.get_dict());

            let mut count = 0usize;
            let mut current = side.get_build_list();
            while let Some(node) = current {
                count += 1;
                current = node.get_next();
            }
            output.write_int(count as i32);

            let mut current = side.get_build_list();
            while let Some(node) = current {
                output.write_ascii_string(&node.get_building_name());
                output.write_ascii_string(&node.get_template_name());
                let loc = node.get_location();
                output.write_real(loc.x);
                output.write_real(loc.y);
                output.write_real(loc.z);
                output.write_real(node.get_angle());
                output.write_byte(node.is_initially_built() as u8);
                output.write_int(node.get_num_rebuilds() as i32);
                output.write_ascii_string(&node.get_script());
                output.write_int(node.get_health());
                output.write_byte(node.get_whiner() as u8);
                output.write_byte(node.get_unsellable() as u8);
                output.write_byte(node.get_repairable() as u8);
                current = node.get_next();
            }
        }

        output.write_int(self.get_num_teams() as i32);
        for team in &self.teamrec.teams {
            output.write_dict(team.get_dict());
        }

        let mut script_lists = Vec::with_capacity(self.get_num_sides());
        for side in &self.sides {
            script_lists.push(side.get_script_list());
        }
        ScriptList::write_scripts_data_chunk(output, &script_lists);

        output.close_data_chunk();
        let _ = self.validate_sides();
    }
}

impl SubsystemInterface for SidesList {}

impl Snapshot for SidesList {
    fn crc(&self, xfer: &mut dyn Xfer) {
        let mut version: XferVersion = 1;
        let _ = xfer.xfer_version(&mut version, 1);
        let mut v = self.get_num_sides() as i32;
        let _ = xfer.xfer_int(&mut v);
        for side in &self.sides {
            let mut v = side.scripts.is_some();
            let _ = xfer.xfer_bool(&mut v);
        }
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        let _ = xfer.xfer_version(&mut version, current_version);

        let mut side_count = self.get_num_sides() as i32;
        let _ = xfer.xfer_int(&mut side_count);
        if side_count != self.get_num_sides() as i32 {
            panic!("SidesList::xfer - The sides list size has changed, versioning required");
        }

        for idx in 0..side_count.max(0) as usize {
            let Some(side) = self.sides.get_mut(idx) else {
                continue;
            };
            let mut script_list_present = side.scripts.is_some();
            let _ = xfer.xfer_bool(&mut script_list_present);
            if script_list_present {
                if side.scripts.is_none() {
                    panic!("SidesList::xfer - script list missing/present mismatch");
                }
                if let Some(list) = side.scripts.as_mut() {
                    let _ = list.xfer(xfer);
                }
            } else if side.scripts.is_some() {
                panic!("SidesList::xfer - script list missing/present mismatch");
            }
        }
    }

    fn load_post_process(&mut self) {}
}

pub static THE_SIDES_LIST: Lazy<Arc<RwLock<SidesList>>> =
    Lazy::new(|| Arc::new(RwLock::new(SidesList::new())));

pub fn get_sides_list() -> Arc<RwLock<SidesList>> {
    THE_SIDES_LIST.clone()
}

#[derive(Default)]
struct SkirmishScriptContext {
    player_names: Vec<String>,
    team_dicts: Vec<Dict>,
    script_lists: ScriptListReadInfo,
}

fn parse_players_data_chunk(
    input: &mut DataChunkInput,
    info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let context = match user_data.downcast_mut::<SkirmishScriptContext>() {
        Some(context) => context,
        None => return false,
    };
    let mut read_dicts = 0;
    if info.version >= 2 {
        read_dicts = input.read_int().max(0);
    }
    let count = input.read_int().max(0) as usize;
    for _ in 0..count.min(MAX_PLAYER_COUNT) {
        let name = input.read_ascii_string();
        context.player_names.push(name);
        if read_dicts != 0 {
            let _ = input.read_dict();
        }
    }
    input.at_end_of_chunk()
}

fn parse_teams_data_chunk(
    input: &mut DataChunkInput,
    _info: &DataChunkInfo,
    user_data: &mut dyn std::any::Any,
) -> bool {
    let context = match user_data.downcast_mut::<SkirmishScriptContext>() {
        Some(context) => context,
        None => return false,
    };
    while !input.at_end_of_chunk() {
        let dict = input.read_dict();
        context.team_dicts.push(dict);
    }
    true
}
