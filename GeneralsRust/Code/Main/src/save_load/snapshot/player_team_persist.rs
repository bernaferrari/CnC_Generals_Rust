//! Live `CHUNK_Players` + `CHUNK_TeamFactory` persist.
//!
//! C++ writes PlayerList::xfer v1 / TeamFactory::xfer v1. Live used to emit
//! NullSnapshot, so science hide/disable, team-relation overrides, build/radar
//! script locks, team OnCreate/OnDestroyed latches, leftover Team::xfer
//! `entered_or_exited` / `check_enemy_sighted`, leftover TeamPrototype::xfer
//! `attack_priority_name`, and leftover TeamTemplateInfo::xfer
//! `production_priority` reset after load.
//!
//! No WorldSnapshot version bump: pending bytes ride the named chunks.

use crate::game_logic::{GameLogic, ObjectId};
use crate::save_load::{SaveLoadError, SaveLoadResult};
use game_engine::common::system::xfer::Xfer as CommonXfer;
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use std::io::{Cursor, Seek, Write};
use std::sync::Mutex;

pub const CHUNK_PLAYERS: &str = "CHUNK_Players";
pub const CHUNK_TEAM_FACTORY: &str = "CHUNK_TeamFactory";

const PLAYERS_CHUNK_VERSION: u8 = 3;
const TEAM_FACTORY_CHUNK_VERSION: u8 = 4;
const MAX_ATTACKED_BY: usize = 16;
const MAX_GENERIC_SCRIPTS: usize = 16;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamRelPersist {
    pub team_name: String,
    pub team_id: u32,
    pub relationship: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerRelPersist {
    pub player_index: i32,
    pub relationship: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct KindOfChangePersist {
    pub kind_of_name: String,
    pub kind_of_bits: u128,
    pub percent: f32,
    pub refs: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerRuntimePersist {
    pub player_id: u32,
    pub sciences: Vec<String>,
    pub sciences_disabled: Vec<String>,
    pub sciences_hidden: Vec<String>,
    pub team_relations: Vec<TeamRelPersist>,
    pub player_relations: Vec<PlayerRelPersist>,
    pub can_build_units: bool,
    pub can_build_base: bool,
    pub is_observer: bool,
    pub skill_points_modifier: f32,
    pub list_in_score_screen: bool,
    pub attacked_by: [bool; MAX_ATTACKED_BY],
    pub radar_count: i32,
    pub is_player_dead: bool,
    pub disable_proof_radar_count: i32,
    pub radar_disabled: bool,
    pub cash_bounty_percent: f32,
    pub kind_of_changes: Vec<KindOfChangePersist>,
    pub units_should_hunt: bool,
    pub current_selection: Vec<u32>,
    pub did_preorder: bool,
}

impl Default for PlayerRuntimePersist {
    fn default() -> Self {
        Self {
            player_id: 0,
            sciences: Vec::new(),
            sciences_disabled: Vec::new(),
            sciences_hidden: Vec::new(),
            team_relations: Vec::new(),
            player_relations: Vec::new(),
            can_build_units: true,
            can_build_base: true,
            is_observer: false,
            skill_points_modifier: 1.0,
            list_in_score_screen: true,
            attacked_by: [false; MAX_ATTACKED_BY],
            radar_count: 0,
            is_player_dead: false,
            disable_proof_radar_count: 0,
            radar_disabled: false,
            cash_bounty_percent: 0.0,
            kind_of_changes: Vec::new(),
            units_should_hunt: false,
            current_selection: Vec::new(),
            did_preorder: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TeamRuntimePersist {
    pub team_name: String,
    pub team_id: u32,
    pub created: bool,
    pub active: bool,
    pub see_enemy: bool,
    pub prev_see_enemy: bool,
    pub was_idle: bool,
    pub destroy_threshold: i32,
    pub cur_units: i32,
    pub current_waypoint_id: u32,
    pub generic_script_attempts: Vec<bool>,
    pub recruitability_set: bool,
    pub recruitable: bool,
    pub state: String,
    pub entered_or_exited: bool,
    pub check_enemy_sighted: bool,
    /// True when v3+ bytes carried the two leftover Team::xfer flags.
    pub persist_edge_flags: bool,
    pub team_relations: Vec<TeamRelPersist>,
    pub player_relations: Vec<PlayerRelPersist>,
}

impl Default for TeamRuntimePersist {
    fn default() -> Self {
        Self {
            team_name: String::new(),
            team_id: 0,
            created: false,
            active: false,
            see_enemy: false,
            prev_see_enemy: false,
            was_idle: false,
            destroy_threshold: 0,
            cur_units: 0,
            current_waypoint_id: 0,
            generic_script_attempts: vec![true; MAX_GENERIC_SCRIPTS],
            recruitability_set: false,
            recruitable: false,
            state: String::new(),
            entered_or_exited: false,
            check_enemy_sighted: false,
            persist_edge_flags: false,
            team_relations: Vec::new(),
            player_relations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayersChunkPersist {
    pub players: Vec<PlayerRuntimePersist>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamFactoryChunkPersist {
    pub unique_team_id: u32,
    pub teams: Vec<TeamRuntimePersist>,
    pub prototypes: Vec<TeamPrototypePersist>,
}

/// Leftover `TeamPrototype::xfer` v2 + `TeamTemplateInfo::xfer` v1 fields.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TeamPrototypePersist {
    pub team_name: String,
    pub attack_priority_name: String,
    pub production_priority: i32,
}

static PENDING_PLAYERS: Mutex<Option<PlayersChunkPersist>> = Mutex::new(None);
static PENDING_TEAMS: Mutex<Option<TeamFactoryChunkPersist>> = Mutex::new(None);

fn map_xfer<T>(result: std::io::Result<T>) -> SaveLoadResult<T> {
    result.map_err(|e| SaveLoadError::Serialization(e.to_string()))
}

fn relationship_from_i32(raw: i32) -> gamelogic::common::Relationship {
    match raw {
        0 => gamelogic::common::Relationship::Enemies,
        2 => gamelogic::common::Relationship::Allies,
        _ => gamelogic::common::Relationship::Neutral,
    }
}

fn relationship_to_i32(rel: gamelogic::common::Relationship) -> i32 {
    match rel {
        gamelogic::common::Relationship::Enemies => 0,
        gamelogic::common::Relationship::Neutral => 1,
        gamelogic::common::Relationship::Allies => 2,
    }
}

fn leftover_relationship_from_i32(raw: i32) -> gamelogic::common::Relationship {
    relationship_from_i32(raw)
}

fn science_name_from_type(science: game_engine::common::rts::ScienceType) -> String {
    if let Some(store) = game_engine::common::rts::get_science_store() {
        let name = store.get_internal_name_for_science(science);
        if !name.as_str().is_empty() {
            return name.to_string();
        }
    }
    if let Some(name) = game_engine::common::ini::ini_science::get_science_store()
        .get_internal_name_for_science(game_engine::common::ini::ini_science::ScienceType(science))
    {
        if !name.as_str().is_empty() {
            return name.to_string();
        }
    }
    String::new()
}

fn science_type_from_name(name: &str) -> game_engine::common::rts::ScienceType {
    if let Some(store) = game_engine::common::rts::get_science_store() {
        let science = store.get_science_from_internal_name(name);
        if science != game_engine::common::rts::SCIENCE_INVALID {
            return science;
        }
    }
    if let Some(science) = game_engine::common::ini::ini_science::get_science_store()
        .get_science_from_internal_name(name)
    {
        return science.0;
    }
    game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name)
        as game_engine::common::rts::ScienceType
}

fn xfer_string_list<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    values: &[String],
) -> SaveLoadResult<()> {
    let mut count = values.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    for value in values {
        let mut owned = value.clone();
        map_xfer(xfer.xfer_ascii_string(&mut owned))?;
    }
    Ok(())
}

fn parse_string_list(xfer: &mut CommonXferLoad<Cursor<&[u8]>>) -> SaveLoadResult<Vec<String>> {
    let mut count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut value = String::new();
        map_xfer(xfer.xfer_ascii_string(&mut value))?;
        if !value.is_empty() {
            out.push(value);
        }
    }
    Ok(out)
}

fn write_player_entry<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    player: &PlayerRuntimePersist,
) -> SaveLoadResult<()> {
    let mut player_id = player.player_id;
    map_xfer(xfer.xfer_unsigned_int(&mut player_id))?;
    xfer_string_list(xfer, &player.sciences)?;
    xfer_string_list(xfer, &player.sciences_disabled)?;
    xfer_string_list(xfer, &player.sciences_hidden)?;
    let mut team_rel_count = player.team_relations.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut team_rel_count))?;
    for rel in &player.team_relations {
        let mut name = rel.team_name.clone();
        let mut team_id = rel.team_id;
        let mut relationship = rel.relationship;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_unsigned_int(&mut team_id))?;
        map_xfer(xfer.xfer_int(&mut relationship))?;
    }
    let mut player_rel_count = player.player_relations.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut player_rel_count))?;
    for rel in &player.player_relations {
        let mut player_index = rel.player_index;
        let mut relationship = rel.relationship;
        map_xfer(xfer.xfer_int(&mut player_index))?;
        map_xfer(xfer.xfer_int(&mut relationship))?;
    }
    let mut can_build_units = player.can_build_units;
    let mut can_build_base = player.can_build_base;
    let mut is_observer = player.is_observer;
    let mut skill_points_modifier = player.skill_points_modifier;
    let mut list_in_score_screen = player.list_in_score_screen;
    map_xfer(xfer.xfer_bool(&mut can_build_units))?;
    map_xfer(xfer.xfer_bool(&mut can_build_base))?;
    map_xfer(xfer.xfer_bool(&mut is_observer))?;
    map_xfer(xfer.xfer_real(&mut skill_points_modifier))?;
    map_xfer(xfer.xfer_bool(&mut list_in_score_screen))?;
    for flag in &player.attacked_by {
        let mut value = *flag;
        map_xfer(xfer.xfer_bool(&mut value))?;
    }
    let mut radar_count = player.radar_count;
    let mut is_player_dead = player.is_player_dead;
    let mut disable_proof_radar_count = player.disable_proof_radar_count;
    let mut radar_disabled = player.radar_disabled;
    let mut cash_bounty_percent = player.cash_bounty_percent;
    map_xfer(xfer.xfer_int(&mut radar_count))?;
    map_xfer(xfer.xfer_bool(&mut is_player_dead))?;
    map_xfer(xfer.xfer_int(&mut disable_proof_radar_count))?;
    map_xfer(xfer.xfer_bool(&mut radar_disabled))?;
    map_xfer(xfer.xfer_real(&mut cash_bounty_percent))?;
    let mut kind_count = player.kind_of_changes.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut kind_count))?;
    for entry in &player.kind_of_changes {
        let mut name = entry.kind_of_name.clone();
        let mut bits_lo = entry.kind_of_bits as u64;
        let mut bits_hi = (entry.kind_of_bits >> 64) as u64;
        let mut percent = entry.percent;
        let mut refs = entry.refs;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_u64(&mut bits_lo))?;
        map_xfer(xfer.xfer_u64(&mut bits_hi))?;
        map_xfer(xfer.xfer_real(&mut percent))?;
        map_xfer(xfer.xfer_unsigned_int(&mut refs))?;
    }
    let mut units_should_hunt = player.units_should_hunt;
    map_xfer(xfer.xfer_bool(&mut units_should_hunt))?;
    let mut sel_count = player.current_selection.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut sel_count))?;
    for id in &player.current_selection {
        let mut value = *id;
        map_xfer(xfer.xfer_unsigned_int(&mut value))?;
    }
    let mut did_preorder = player.did_preorder;
    map_xfer(xfer.xfer_bool(&mut did_preorder))?;
    Ok(())
}

fn parse_player_entry(
    xfer: &mut CommonXferLoad<Cursor<&[u8]>>,
    version: u8,
) -> SaveLoadResult<PlayerRuntimePersist> {
    let mut player = PlayerRuntimePersist::default();
    map_xfer(xfer.xfer_unsigned_int(&mut player.player_id))?;
    player.sciences = parse_string_list(xfer)?;
    player.sciences_disabled = parse_string_list(xfer)?;
    player.sciences_hidden = parse_string_list(xfer)?;
    let mut team_rel_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut team_rel_count))?;
    for _ in 0..team_rel_count {
        let mut team_name = String::new();
        let mut team_id = 0u32;
        let mut relationship = 0i32;
        map_xfer(xfer.xfer_ascii_string(&mut team_name))?;
        map_xfer(xfer.xfer_unsigned_int(&mut team_id))?;
        map_xfer(xfer.xfer_int(&mut relationship))?;
        player.team_relations.push(TeamRelPersist {
            team_name,
            team_id,
            relationship,
        });
    }
    let mut player_rel_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut player_rel_count))?;
    for _ in 0..player_rel_count {
        let mut player_index = 0i32;
        let mut relationship = 0i32;
        map_xfer(xfer.xfer_int(&mut player_index))?;
        map_xfer(xfer.xfer_int(&mut relationship))?;
        player.player_relations.push(PlayerRelPersist {
            player_index,
            relationship,
        });
    }
    map_xfer(xfer.xfer_bool(&mut player.can_build_units))?;
    map_xfer(xfer.xfer_bool(&mut player.can_build_base))?;
    map_xfer(xfer.xfer_bool(&mut player.is_observer))?;
    map_xfer(xfer.xfer_real(&mut player.skill_points_modifier))?;
    map_xfer(xfer.xfer_bool(&mut player.list_in_score_screen))?;
    for flag in &mut player.attacked_by {
        map_xfer(xfer.xfer_bool(flag))?;
    }
    map_xfer(xfer.xfer_int(&mut player.radar_count))?;
    map_xfer(xfer.xfer_bool(&mut player.is_player_dead))?;
    map_xfer(xfer.xfer_int(&mut player.disable_proof_radar_count))?;
    map_xfer(xfer.xfer_bool(&mut player.radar_disabled))?;
    map_xfer(xfer.xfer_real(&mut player.cash_bounty_percent))?;
    let mut kind_count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut kind_count))?;
    for _ in 0..kind_count {
        let mut kind_of_name = String::new();
        let mut bits_lo = 0u64;
        let mut bits_hi = 0u64;
        let mut percent = 0.0f32;
        let mut refs = 0u32;
        map_xfer(xfer.xfer_ascii_string(&mut kind_of_name))?;
        map_xfer(xfer.xfer_u64(&mut bits_lo))?;
        map_xfer(xfer.xfer_u64(&mut bits_hi))?;
        map_xfer(xfer.xfer_real(&mut percent))?;
        map_xfer(xfer.xfer_unsigned_int(&mut refs))?;
        player.kind_of_changes.push(KindOfChangePersist {
            kind_of_name,
            kind_of_bits: (bits_hi as u128) << 64 | bits_lo as u128,
            percent,
            refs,
        });
    }
    if version >= 2 {
        map_xfer(xfer.xfer_bool(&mut player.units_should_hunt))?;
        let mut sel_count = 0u16;
        map_xfer(xfer.xfer_unsigned_short(&mut sel_count))?;
        player.current_selection.reserve(sel_count as usize);
        for _ in 0..sel_count {
            let mut id = 0u32;
            map_xfer(xfer.xfer_unsigned_int(&mut id))?;
            player.current_selection.push(id);
        }
    }
    if version >= 3 {
        map_xfer(xfer.xfer_bool(&mut player.did_preorder))?;
    }
    Ok(player)
}

pub fn write_players_block<W: Write + Seek>(xfer: &mut CommonXferSave<W>) -> SaveLoadResult<()> {
    let persist = peek_pending_players().unwrap_or_else(|| capture_players_chunk(None));
    let mut version = PLAYERS_CHUNK_VERSION;
    map_xfer(xfer.xfer_version(&mut version, PLAYERS_CHUNK_VERSION))?;
    let mut count = persist.players.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    for player in &persist.players {
        write_player_entry(xfer, player)?;
    }
    Ok(())
}

pub fn parse_players_block(payload: &[u8]) -> SaveLoadResult<PlayersChunkPersist> {
    if payload.is_empty() {
        return Ok(PlayersChunkPersist::default());
    }
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), 1);
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, PLAYERS_CHUNK_VERSION))?;
    if version < 1 {
        return Ok(PlayersChunkPersist::default());
    }
    let mut count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    let mut players = Vec::with_capacity(count as usize);
    for _ in 0..count {
        players.push(parse_player_entry(&mut xfer, version)?);
    }
    Ok(PlayersChunkPersist { players })
}

pub fn write_team_factory_block<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
) -> SaveLoadResult<()> {
    let persist = peek_pending_teams().unwrap_or_else(capture_team_factory_chunk);
    let mut version = TEAM_FACTORY_CHUNK_VERSION;
    map_xfer(xfer.xfer_version(&mut version, TEAM_FACTORY_CHUNK_VERSION))?;
    let mut unique_team_id = persist.unique_team_id;
    map_xfer(xfer.xfer_unsigned_int(&mut unique_team_id))?;
    let mut count = persist.teams.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    for team in &persist.teams {
        let mut name = team.team_name.clone();
        let mut team_id = team.team_id;
        let mut created = team.created;
        let mut active = team.active;
        let mut see_enemy = team.see_enemy;
        let mut prev_see_enemy = team.prev_see_enemy;
        let mut was_idle = team.was_idle;
        let mut destroy_threshold = team.destroy_threshold;
        let mut cur_units = team.cur_units;
        let mut waypoint = team.current_waypoint_id;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_unsigned_int(&mut team_id))?;
        map_xfer(xfer.xfer_bool(&mut created))?;
        map_xfer(xfer.xfer_bool(&mut active))?;
        map_xfer(xfer.xfer_bool(&mut see_enemy))?;
        map_xfer(xfer.xfer_bool(&mut prev_see_enemy))?;
        map_xfer(xfer.xfer_bool(&mut was_idle))?;
        map_xfer(xfer.xfer_int(&mut destroy_threshold))?;
        map_xfer(xfer.xfer_int(&mut cur_units))?;
        map_xfer(xfer.xfer_unsigned_int(&mut waypoint))?;
        let mut generic_count = MAX_GENERIC_SCRIPTS as u16;
        map_xfer(xfer.xfer_unsigned_short(&mut generic_count))?;
        for i in 0..MAX_GENERIC_SCRIPTS {
            let mut flag = team.generic_script_attempts.get(i).copied().unwrap_or(true);
            map_xfer(xfer.xfer_bool(&mut flag))?;
        }
        let mut recruitability_set = team.recruitability_set;
        let mut recruitable = team.recruitable;
        map_xfer(xfer.xfer_bool(&mut recruitability_set))?;
        map_xfer(xfer.xfer_bool(&mut recruitable))?;
        let mut state = team.state.clone();
        map_xfer(xfer.xfer_ascii_string(&mut state))?;
        let mut team_rel_count = team.team_relations.len() as u16;
        map_xfer(xfer.xfer_unsigned_short(&mut team_rel_count))?;
        for rel in &team.team_relations {
            let mut rel_name = rel.team_name.clone();
            let mut rel_team_id = rel.team_id;
            let mut relationship = rel.relationship;
            map_xfer(xfer.xfer_ascii_string(&mut rel_name))?;
            map_xfer(xfer.xfer_unsigned_int(&mut rel_team_id))?;
            map_xfer(xfer.xfer_int(&mut relationship))?;
        }
        let mut player_rel_count = team.player_relations.len() as u16;
        map_xfer(xfer.xfer_unsigned_short(&mut player_rel_count))?;
        for rel in &team.player_relations {
            let mut player_index = rel.player_index;
            let mut relationship = rel.relationship;
            map_xfer(xfer.xfer_int(&mut player_index))?;
            map_xfer(xfer.xfer_int(&mut relationship))?;
        }
        let mut entered_or_exited = team.entered_or_exited;
        let mut check_enemy_sighted = team.check_enemy_sighted;
        map_xfer(xfer.xfer_bool(&mut entered_or_exited))?;
        map_xfer(xfer.xfer_bool(&mut check_enemy_sighted))?;
    }
    let mut proto_count = persist.prototypes.len() as u16;
    map_xfer(xfer.xfer_unsigned_short(&mut proto_count))?;
    for proto in &persist.prototypes {
        let mut name = proto.team_name.clone();
        let mut attack_priority_name = proto.attack_priority_name.clone();
        let mut production_priority = proto.production_priority;
        map_xfer(xfer.xfer_ascii_string(&mut name))?;
        map_xfer(xfer.xfer_ascii_string(&mut attack_priority_name))?;
        map_xfer(xfer.xfer_int(&mut production_priority))?;
    }
    Ok(())
}

pub fn parse_team_factory_block(payload: &[u8]) -> SaveLoadResult<TeamFactoryChunkPersist> {
    if payload.is_empty() {
        return Ok(TeamFactoryChunkPersist::default());
    }
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), 1);
    let mut version = 0u8;
    map_xfer(xfer.xfer_version(&mut version, TEAM_FACTORY_CHUNK_VERSION))?;
    if version < 1 {
        return Ok(TeamFactoryChunkPersist::default());
    }
    let mut unique_team_id = 0u32;
    map_xfer(xfer.xfer_unsigned_int(&mut unique_team_id))?;
    let mut count = 0u16;
    map_xfer(xfer.xfer_unsigned_short(&mut count))?;
    let mut teams = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let mut team = TeamRuntimePersist::default();
        map_xfer(xfer.xfer_ascii_string(&mut team.team_name))?;
        map_xfer(xfer.xfer_unsigned_int(&mut team.team_id))?;
        map_xfer(xfer.xfer_bool(&mut team.created))?;
        map_xfer(xfer.xfer_bool(&mut team.active))?;
        map_xfer(xfer.xfer_bool(&mut team.see_enemy))?;
        map_xfer(xfer.xfer_bool(&mut team.prev_see_enemy))?;
        map_xfer(xfer.xfer_bool(&mut team.was_idle))?;
        map_xfer(xfer.xfer_int(&mut team.destroy_threshold))?;
        map_xfer(xfer.xfer_int(&mut team.cur_units))?;
        map_xfer(xfer.xfer_unsigned_int(&mut team.current_waypoint_id))?;
        let mut generic_count = 0u16;
        map_xfer(xfer.xfer_unsigned_short(&mut generic_count))?;
        team.generic_script_attempts = vec![true; MAX_GENERIC_SCRIPTS];
        for i in 0..generic_count as usize {
            let mut flag = true;
            map_xfer(xfer.xfer_bool(&mut flag))?;
            if i < MAX_GENERIC_SCRIPTS {
                team.generic_script_attempts[i] = flag;
            }
        }
        map_xfer(xfer.xfer_bool(&mut team.recruitability_set))?;
        map_xfer(xfer.xfer_bool(&mut team.recruitable))?;
        if version >= 2 {
            map_xfer(xfer.xfer_ascii_string(&mut team.state))?;
            let mut team_rel_count = 0u16;
            map_xfer(xfer.xfer_unsigned_short(&mut team_rel_count))?;
            for _ in 0..team_rel_count {
                let mut team_name = String::new();
                let mut team_id = 0u32;
                let mut relationship = 0i32;
                map_xfer(xfer.xfer_ascii_string(&mut team_name))?;
                map_xfer(xfer.xfer_unsigned_int(&mut team_id))?;
                map_xfer(xfer.xfer_int(&mut relationship))?;
                team.team_relations.push(TeamRelPersist {
                    team_name,
                    team_id,
                    relationship,
                });
            }
            let mut player_rel_count = 0u16;
            map_xfer(xfer.xfer_unsigned_short(&mut player_rel_count))?;
            for _ in 0..player_rel_count {
                let mut player_index = 0i32;
                let mut relationship = 0i32;
                map_xfer(xfer.xfer_int(&mut player_index))?;
                map_xfer(xfer.xfer_int(&mut relationship))?;
                team.player_relations.push(PlayerRelPersist {
                    player_index,
                    relationship,
                });
            }
        }
        if version >= 3 {
            map_xfer(xfer.xfer_bool(&mut team.entered_or_exited))?;
            map_xfer(xfer.xfer_bool(&mut team.check_enemy_sighted))?;
            team.persist_edge_flags = true;
        }
        teams.push(team);
    }
    let mut prototypes = Vec::new();
    if version >= 4 {
        let mut proto_count = 0u16;
        map_xfer(xfer.xfer_unsigned_short(&mut proto_count))?;
        prototypes.reserve(proto_count as usize);
        for _ in 0..proto_count {
            let mut proto = TeamPrototypePersist::default();
            map_xfer(xfer.xfer_ascii_string(&mut proto.team_name))?;
            map_xfer(xfer.xfer_ascii_string(&mut proto.attack_priority_name))?;
            map_xfer(xfer.xfer_int(&mut proto.production_priority))?;
            prototypes.push(proto);
        }
    }
    Ok(TeamFactoryChunkPersist {
        unique_team_id,
        teams,
        prototypes,
    })
}

fn capture_leftover_players() -> Vec<PlayerRuntimePersist> {
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for idx in 0..list.get_player_count() {
        let Some(player_arc) = list
            .get_player(idx as gamelogic::player::PlayerIndex)
            .cloned()
        else {
            continue;
        };
        let Ok(player) = player_arc.read() else {
            continue;
        };
        let mut persist = PlayerRuntimePersist {
            player_id: player.get_player_index() as u32,
            can_build_units: player.get_can_build_units(),
            can_build_base: player.get_can_build_base(),
            is_observer: player.is_player_observer(),
            skill_points_modifier: player.get_skill_points_modifier(),
            list_in_score_screen: player.get_list_in_score_screen(),
            radar_count: player.get_radar_count(),
            is_player_dead: player.is_player_dead(),
            disable_proof_radar_count: player.get_disable_proof_radar_count(),
            radar_disabled: player.is_radar_disabled(),
            cash_bounty_percent: player.get_cash_bounty(),
            units_should_hunt: player.get_units_should_hunt(),
            current_selection: player.get_current_selection_ids(),
            did_preorder: player.did_player_preorder(),
            ..PlayerRuntimePersist::default()
        };
        persist.sciences = player
            .get_sciences()
            .iter()
            .map(|science| science_name_from_type(*science))
            .filter(|name| !name.is_empty())
            .collect();
        persist.sciences_disabled = player
            .sciences_disabled_types()
            .iter()
            .map(|science| science_name_from_type(*science))
            .filter(|name| !name.is_empty())
            .collect();
        persist.sciences_hidden = player
            .sciences_hidden_types()
            .iter()
            .map(|science| science_name_from_type(*science))
            .filter(|name| !name.is_empty())
            .collect();
        persist.team_relations = player
            .team_relation_pairs()
            .into_iter()
            .map(|(team_id, relationship)| TeamRelPersist {
                team_name: leftover_team_name(team_id),
                team_id,
                relationship,
            })
            .collect();
        persist.player_relations = player
            .player_relation_pairs()
            .into_iter()
            .map(|(player_index, relationship)| PlayerRelPersist {
                player_index,
                relationship,
            })
            .collect();
        for (i, flag) in persist.attacked_by.iter_mut().enumerate() {
            *flag = player.get_attacked_by(i as i32);
        }
        persist.kind_of_changes = player
            .kind_of_production_change_entries()
            .into_iter()
            .map(|(bits, percent, refs)| KindOfChangePersist {
                kind_of_name: String::new(),
                kind_of_bits: bits,
                percent,
                refs,
            })
            .collect();
        out.push(persist);
    }
    out
}

fn leftover_team_name(team_id: u32) -> String {
    let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
        return String::new();
    };
    let Some(team) = factory.find_team_by_id(team_id) else {
        return String::new();
    };
    team.read()
        .map(|guard| guard.get_name().to_string())
        .unwrap_or_default()
}

fn leftover_team_id(team_name: &str) -> u32 {
    if team_name.trim().is_empty() {
        return 0;
    }
    let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
        return 0;
    };
    factory
        .find_team_instances(team_name)
        .into_iter()
        .next()
        .and_then(|team| team.read().ok().map(|guard| guard.get_id()))
        .unwrap_or(0)
}

fn capture_players_chunk(game_logic: Option<&GameLogic>) -> PlayersChunkPersist {
    let mut persist = PlayersChunkPersist {
        players: capture_leftover_players(),
    };
    let Some(game_logic) = game_logic else {
        return persist;
    };
    for (id, player) in game_logic.get_players() {
        let slot = if let Some(existing) = persist
            .players
            .iter_mut()
            .find(|entry| entry.player_id == *id)
        {
            existing
        } else {
            persist.players.push(PlayerRuntimePersist {
                player_id: *id,
                ..PlayerRuntimePersist::default()
            });
            persist.players.last_mut().expect("just pushed")
        };
        slot.sciences = player.unlocked_sciences.iter().cloned().collect();
        slot.sciences_disabled = player.sciences_disabled.iter().cloned().collect();
        slot.sciences_hidden = player.sciences_hidden.iter().cloned().collect();
        slot.team_relations = player
            .team_relations
            .iter()
            .map(|(name, rel)| TeamRelPersist {
                team_name: name.clone(),
                team_id: leftover_team_id(name),
                relationship: relationship_to_i32(*rel),
            })
            .collect();
        slot.player_relations = player
            .map_side
            .relations
            .iter()
            .map(|(other, rel)| PlayerRelPersist {
                player_index: *other as i32,
                relationship: relationship_to_i32(*rel),
            })
            .collect();
        slot.can_build_units = player.can_build_units;
        slot.can_build_base = player.can_build_base;
        slot.is_observer = player.is_observer;
        slot.skill_points_modifier = player.skill_points_modifier;
        slot.list_in_score_screen = true;
        slot.attacked_by = player.attacked_by;
        slot.radar_count = player.radar_count;
        slot.is_player_dead = !player.is_alive;
        slot.disable_proof_radar_count = player.disable_proof_radar_count;
        slot.radar_disabled = player.radar_disabled;
        slot.cash_bounty_percent = player.cash_bounty_percent;
        slot.kind_of_changes = player
            .kind_of_production_cost_changes
            .iter()
            .map(|entry| KindOfChangePersist {
                kind_of_name: entry.kind_of.clone(),
                kind_of_bits: 0,
                percent: entry.percent,
                refs: entry.ref_count,
            })
            .collect();
        slot.units_should_hunt = player.units_should_hunt;
        slot.current_selection = player.selected_objects.iter().map(|id| id.0).collect();
        slot.did_preorder = player.did_preorder;
    }
    persist
}

fn capture_team_factory_chunk() -> TeamFactoryChunkPersist {
    let Ok(factory) = gamelogic::team::get_team_factory().lock() else {
        return TeamFactoryChunkPersist::default();
    };
    let unique_team_id = factory.get_next_team_id();
    let mut teams = Vec::new();
    for team_arc in factory.get_all_teams() {
        let Ok(team) = team_arc.read() else {
            continue;
        };
        let mut generic_script_attempts = Vec::with_capacity(MAX_GENERIC_SCRIPTS);
        for i in 0..MAX_GENERIC_SCRIPTS {
            generic_script_attempts.push(team.should_attempt_generic_script(i));
        }
        teams.push(TeamRuntimePersist {
            team_name: team.get_name().to_string(),
            team_id: team.get_id(),
            created: team.is_created(),
            active: team.is_active(),
            see_enemy: team.get_see_enemy(),
            prev_see_enemy: team.get_prev_see_enemy(),
            was_idle: team.get_was_idle(),
            destroy_threshold: team.get_destroy_threshold(),
            cur_units: team.get_cur_units_count(),
            current_waypoint_id: team.get_current_waypoint_id().unwrap_or(0),
            generic_script_attempts,
            recruitability_set: team.is_recruitability_set(),
            recruitable: team.is_recruitable(),
            state: team.get_state().to_string(),
            entered_or_exited: team.did_enter_or_exit(),
            check_enemy_sighted: team.get_check_enemy_sighted(),
            persist_edge_flags: true,
            team_relations: team
                .team_relation_override_pairs()
                .into_iter()
                .map(|(team_id, relationship)| TeamRelPersist {
                    team_name: String::new(),
                    team_id,
                    relationship: relationship_to_i32(relationship),
                })
                .collect(),
            player_relations: team
                .player_relation_override_pairs()
                .into_iter()
                .map(|(player_index, relationship)| PlayerRelPersist {
                    player_index,
                    relationship: relationship_to_i32(relationship),
                })
                .collect(),
        });
    }
    let mut prototypes: Vec<TeamPrototypePersist> = factory
        .list_team_prototypes()
        .into_iter()
        .map(|proto| TeamPrototypePersist {
            team_name: proto.get_name().to_string(),
            attack_priority_name: proto.get_attack_priority_name().to_string(),
            production_priority: proto.get_production_priority(),
        })
        .collect();
    prototypes.sort_by(|a, b| a.team_name.cmp(&b.team_name));
    TeamFactoryChunkPersist {
        unique_team_id,
        teams,
        prototypes,
    }
}

pub fn stamp_from_live(game_logic: &GameLogic) {
    if let Ok(mut guard) = PENDING_PLAYERS.lock() {
        *guard = Some(capture_players_chunk(Some(game_logic)));
    }
    if let Ok(mut guard) = PENDING_TEAMS.lock() {
        *guard = Some(capture_team_factory_chunk());
    }
}

pub fn stash_loaded_chunks(players: Option<&[u8]>, teams: Option<&[u8]>) {
    if let Some(payload) = players {
        if payload.len() > 1 {
            if let Ok(parsed) = parse_players_block(payload) {
                if let Ok(mut guard) = PENDING_PLAYERS.lock() {
                    *guard = Some(parsed);
                }
            }
        }
    }
    if let Some(payload) = teams {
        if payload.len() > 1 {
            if let Ok(parsed) = parse_team_factory_block(payload) {
                if let Ok(mut guard) = PENDING_TEAMS.lock() {
                    *guard = Some(parsed);
                }
            }
        }
    }
}

fn peek_pending_players() -> Option<PlayersChunkPersist> {
    PENDING_PLAYERS.lock().ok().and_then(|guard| guard.clone())
}

fn peek_pending_teams() -> Option<TeamFactoryChunkPersist> {
    PENDING_TEAMS.lock().ok().and_then(|guard| guard.clone())
}

fn take_pending_players() -> Option<PlayersChunkPersist> {
    PENDING_PLAYERS
        .lock()
        .ok()
        .and_then(|mut guard| guard.take())
}

fn take_pending_teams() -> Option<TeamFactoryChunkPersist> {
    PENDING_TEAMS.lock().ok().and_then(|mut guard| guard.take())
}

fn apply_player_to_live(game_logic: &mut GameLogic, persist: &PlayerRuntimePersist) {
    let Some(player) = game_logic.get_player_mut(persist.player_id) else {
        return;
    };
    player.sciences_disabled = persist.sciences_disabled.iter().cloned().collect();
    player.sciences_hidden = persist.sciences_hidden.iter().cloned().collect();
    for name in &persist.sciences {
        player.unlocked_sciences.insert(name.clone());
    }
    player.team_relations = persist
        .team_relations
        .iter()
        .filter(|rel| !rel.team_name.trim().is_empty())
        .map(|rel| {
            (
                rel.team_name.clone(),
                relationship_from_i32(rel.relationship),
            )
        })
        .collect();
    for rel in &persist.player_relations {
        if rel.player_index >= 0 {
            player.map_side.relations.insert(
                rel.player_index as u32,
                relationship_from_i32(rel.relationship),
            );
        }
    }
    player.can_build_units = persist.can_build_units;
    player.can_build_base = persist.can_build_base;
    player.is_observer = persist.is_observer;
    player.skill_points_modifier = persist.skill_points_modifier;
    player.attacked_by = persist.attacked_by;
    player.radar_count = persist.radar_count;
    player.is_alive = !persist.is_player_dead;
    player.disable_proof_radar_count = persist.disable_proof_radar_count;
    player.radar_disabled = persist.radar_disabled;
    player.cash_bounty_percent = persist.cash_bounty_percent;
    player.units_should_hunt = persist.units_should_hunt;
    player.selected_objects = persist
        .current_selection
        .iter()
        .copied()
        .map(ObjectId)
        .collect();
    player.did_preorder = persist.did_preorder;

    player.kind_of_production_cost_changes.clear();
    for entry in &persist.kind_of_changes {
        if entry.kind_of_name.trim().is_empty() {
            continue;
        }
        player.add_kind_of_production_cost_change(&entry.kind_of_name, entry.percent);
        if let Some(last) = player.kind_of_production_cost_changes.last_mut() {
            last.ref_count = entry.refs.max(1);
        }
    }
}

fn apply_player_to_leftover(persist: &PlayerRuntimePersist) {
    let Ok(list) = gamelogic::player::ThePlayerList().read() else {
        return;
    };
    let Some(player_arc) = list
        .get_player(persist.player_id as gamelogic::player::PlayerIndex)
        .cloned()
    else {
        return;
    };
    drop(list);
    let Ok(mut player) = player_arc.write() else {
        return;
    };
    for name in &persist.sciences_disabled {
        let science = science_type_from_name(name);
        player.set_science_availability(
            science,
            gamelogic::player::ScienceAvailabilityType::Disabled,
        );
    }
    for name in &persist.sciences_hidden {
        let science = science_type_from_name(name);
        player
            .set_science_availability(science, gamelogic::player::ScienceAvailabilityType::Hidden);
    }
    for name in &persist.sciences {
        let science = science_type_from_name(name);
        let _ = player.add_science(science);
    }
    for rel in &persist.team_relations {
        let relationship = leftover_relationship_from_i32(rel.relationship);
        if rel.team_id != 0 {
            player.set_team_relationship_by_id(rel.team_id, relationship);
        } else if !rel.team_name.trim().is_empty() {
            let team_id = leftover_team_id(&rel.team_name);
            if team_id != 0 {
                player.set_team_relationship_by_id(team_id, relationship);
            }
        }
    }
    for rel in &persist.player_relations {
        player.set_player_relationship_by_index(
            rel.player_index,
            leftover_relationship_from_i32(rel.relationship),
        );
    }
    player.set_can_build_units(persist.can_build_units);
    player.set_can_build_base(persist.can_build_base);
    player.set_observer(persist.is_observer);
    player.set_skill_points_modifier(persist.skill_points_modifier);
    player.set_list_in_score_screen(persist.list_in_score_screen);
    player.set_defeated(persist.is_player_dead);
    player.restore_radar_state(
        persist.radar_count,
        persist.disable_proof_radar_count,
        persist.radar_disabled,
    );
    player.set_cash_bounty(persist.cash_bounty_percent);
    player.set_is_preorder(persist.did_preorder);
    player.restore_units_should_hunt(persist.units_should_hunt);
    player.set_currently_selected_ai_group(None);
    for &id in &persist.current_selection {
        player.add_object_to_current_selection(id);
    }
    for (i, &flag) in persist.attacked_by.iter().enumerate() {
        if flag {
            player.set_attacked_by(i as i32);
        }
    }
    let leftover_kind: Vec<(u128, f32, u32)> = persist
        .kind_of_changes
        .iter()
        .filter(|entry| entry.kind_of_bits != 0)
        .map(|entry| (entry.kind_of_bits, entry.percent, entry.refs.max(1)))
        .collect();
    if !leftover_kind.is_empty() {
        player.replace_kind_of_production_changes(&leftover_kind);
    }
}

fn apply_teams_to_leftover(persist: &TeamFactoryChunkPersist) {
    let Ok(mut factory) = gamelogic::team::get_team_factory().lock() else {
        return;
    };
    if persist.unique_team_id != 0 {
        factory.set_next_team_id(persist.unique_team_id);
    }
    for proto_persist in &persist.prototypes {
        if proto_persist.team_name.trim().is_empty() {
            continue;
        }
        let Some(prototype) = factory.find_team_prototype(&proto_persist.team_name) else {
            continue;
        };
        let mut updated = (*prototype).clone();
        updated.set_attack_priority_name(proto_persist.attack_priority_name.clone().into());
        updated.set_production_priority(proto_persist.production_priority);
        factory.replace_team_prototype(updated);
    }
    for team_persist in &persist.teams {
        let team_arc = if team_persist.team_id != 0 {
            factory.find_team_by_id(team_persist.team_id)
        } else {
            None
        }
        .or_else(|| {
            factory
                .find_team_instances(&team_persist.team_name)
                .into_iter()
                .next()
        });
        let Some(team_arc) = team_arc else {
            continue;
        };
        let Ok(mut team) = team_arc.write() else {
            continue;
        };
        let waypoint = if team_persist.current_waypoint_id == 0 {
            None
        } else {
            Some(team_persist.current_waypoint_id)
        };
        team.restore_save_script_state(
            team_persist.created,
            team_persist.active,
            team_persist.see_enemy,
            team_persist.prev_see_enemy,
            team_persist.was_idle,
            team_persist.destroy_threshold,
            team_persist.cur_units,
            waypoint,
            &team_persist.generic_script_attempts,
            team_persist.recruitability_set,
            team_persist.recruitable,
            &team_persist.state,
        );
        if team_persist.persist_edge_flags {
            team.restore_save_edge_flags(
                team_persist.entered_or_exited,
                team_persist.check_enemy_sighted,
            );
        }
        for rel in &team_persist.team_relations {
            let team_id = if rel.team_id != 0 {
                rel.team_id
            } else if !rel.team_name.trim().is_empty() {
                factory
                    .find_team_instances(&rel.team_name)
                    .into_iter()
                    .next()
                    .and_then(|other| other.read().ok().map(|guard| guard.get_id()))
                    .unwrap_or(0)
            } else {
                0
            };
            if team_id != 0 {
                team.set_override_team_relationship(
                    team_id,
                    leftover_relationship_from_i32(rel.relationship),
                );
            }
        }
        for rel in &team_persist.player_relations {
            team.set_override_player_relationship(
                rel.player_index,
                leftover_relationship_from_i32(rel.relationship),
            );
        }
    }
}

pub fn apply_pending(game_logic: &mut GameLogic) {
    if let Some(players) = take_pending_players() {
        for player in &players.players {
            apply_player_to_live(game_logic, player);
            apply_player_to_leftover(player);
        }
    }
    if let Some(teams) = take_pending_teams() {
        apply_teams_to_leftover(&teams);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{ObjectId, Player, Team};
    use game_engine::common::system::xfer::Xfer as CommonXfer;

    #[test]
    fn players_and_team_factory_chunks_round_trip_sciences_relations_and_script_latches() {
        let mut players = PlayersChunkPersist::default();
        let mut player = PlayerRuntimePersist {
            player_id: 1,
            sciences_disabled: vec!["SCIENCE_PaladinTank".into()],
            sciences_hidden: vec!["SCIENCE_StealthFighter".into()],
            can_build_units: false,
            radar_disabled: true,
            skill_points_modifier: 1.5,
            cash_bounty_percent: 0.2,
            units_should_hunt: true,
            current_selection: vec![11, 22],
            did_preorder: true,
            ..PlayerRuntimePersist::default()
        };
        player.team_relations.push(TeamRelPersist {
            team_name: "CivilianConvoy".into(),
            team_id: 7,
            relationship: 2,
        });
        player.attacked_by[2] = true;
        players.players.push(player);

        let mut teams = TeamFactoryChunkPersist {
            unique_team_id: 12,
            teams: vec![TeamRuntimePersist {
                team_name: "HuntWave".into(),
                team_id: 4,
                created: false,
                active: true,
                see_enemy: true,
                prev_see_enemy: true,
                was_idle: false,
                destroy_threshold: -1,
                cur_units: 2,
                current_waypoint_id: 9,
                generic_script_attempts: vec![false; MAX_GENERIC_SCRIPTS],
                recruitability_set: true,
                recruitable: false,
                state: "Attacking".into(),
                entered_or_exited: true,
                check_enemy_sighted: true,
                persist_edge_flags: true,
                team_relations: vec![TeamRelPersist {
                    team_name: "TeamB".into(),
                    team_id: 8,
                    relationship: 1,
                }],
                player_relations: vec![PlayerRelPersist {
                    player_index: 2,
                    relationship: 2,
                }],
            }],
            prototypes: vec![TeamPrototypePersist {
                team_name: "HuntWave".into(),
                attack_priority_name: "PrioritySetA".into(),
                production_priority: 42,
            }],
        };
        teams.teams[0].generic_script_attempts[0] = true;

        if let Ok(mut guard) = PENDING_PLAYERS.lock() {
            *guard = Some(players.clone());
        }
        if let Ok(mut guard) = PENDING_TEAMS.lock() {
            *guard = Some(teams.clone());
        }

        let mut players_only = Cursor::new(Vec::<u8>::new());
        {
            let mut xfer = CommonXferSave::new(&mut players_only, 1);
            write_players_block(&mut xfer).expect("write players");
        }
        let player_bytes = players_only.into_inner();
        assert!(player_bytes.len() > 2, "must not be NullSnapshot");
        let parsed_players = parse_players_block(&player_bytes).expect("parse players");
        assert_eq!(
            parsed_players.players[0].sciences_disabled,
            ["SCIENCE_PaladinTank"]
        );
        assert_eq!(
            parsed_players.players[0].sciences_hidden,
            ["SCIENCE_StealthFighter"]
        );
        assert!(!parsed_players.players[0].can_build_units);
        assert!(parsed_players.players[0].radar_disabled);
        assert!((parsed_players.players[0].skill_points_modifier - 1.5).abs() < f32::EPSILON);
        assert_eq!(
            parsed_players.players[0].team_relations[0].team_name,
            "CivilianConvoy"
        );
        assert!(parsed_players.players[0].attacked_by[2]);
        assert!(parsed_players.players[0].units_should_hunt);
        assert_eq!(parsed_players.players[0].current_selection, [11, 22]);
        assert!(parsed_players.players[0].did_preorder);

        let mut teams_only = Cursor::new(Vec::<u8>::new());
        {
            let mut xfer = CommonXferSave::new(&mut teams_only, 1);
            write_team_factory_block(&mut xfer).expect("rewrite teams");
        }
        let parsed_teams = parse_team_factory_block(&teams_only.into_inner()).expect("parse teams");
        assert_eq!(parsed_teams.unique_team_id, 12);
        assert!(!parsed_teams.teams[0].created);
        assert_eq!(parsed_teams.teams[0].destroy_threshold, -1);
        assert!(parsed_teams.teams[0].see_enemy);
        assert_eq!(parsed_teams.teams[0].current_waypoint_id, 9);
        assert_eq!(parsed_teams.teams[0].state, "Attacking");
        assert_eq!(parsed_teams.teams[0].team_relations[0].team_id, 8);
        assert_eq!(parsed_teams.teams[0].team_relations[0].relationship, 1);
        assert_eq!(parsed_teams.teams[0].player_relations[0].player_index, 2);
        assert_eq!(parsed_teams.teams[0].player_relations[0].relationship, 2);
        assert!(parsed_teams.teams[0].entered_or_exited);
        assert!(parsed_teams.teams[0].check_enemy_sighted);
        assert!(parsed_teams.teams[0].persist_edge_flags);
        assert_eq!(parsed_teams.prototypes[0].team_name, "HuntWave");
        assert_eq!(
            parsed_teams.prototypes[0].attack_priority_name,
            "PrioritySetA"
        );
        assert_eq!(parsed_teams.prototypes[0].production_priority, 42);
        let _ = parsed_players;
    }

    #[test]
    fn stamp_and_apply_restore_live_science_hide_and_team_relation() {
        let mut logic = GameLogic::new();
        let mut usa = Player::new(1, Team::USA, "USA", true);
        usa.set_science_availability("SCIENCE_PaladinTank", "Hidden");
        usa.set_team_relationship_override(
            "CivilianConvoy",
            gamelogic::common::Relationship::Allies,
        );
        usa.can_build_units = false;
        usa.radar_disabled = true;
        usa.set_attacked_by(3);
        usa.units_should_hunt = true;
        usa.selected_objects = vec![ObjectId(11), ObjectId(22)];
        usa.did_preorder = true;
        logic.add_player(usa);

        stamp_from_live(&logic);

        let mut loaded = GameLogic::new();
        loaded.add_player(Player::new(1, Team::USA, "USA", true));
        apply_pending(&mut loaded);

        let player = loaded.get_player(1).expect("player");
        assert!(player.is_science_hidden("SCIENCE_PaladinTank"));
        assert_eq!(
            player.team_relationship_override("CivilianConvoy"),
            Some(gamelogic::common::Relationship::Allies)
        );
        assert!(!player.can_build_units);
        assert!(player.radar_disabled);
        assert!(player.get_attacked_by(3));
        assert!(player.units_should_hunt);
        assert_eq!(player.selected_objects, vec![ObjectId(11), ObjectId(22)]);
        assert!(player.did_preorder);
    }
}
