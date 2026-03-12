//! Persistent storage thread definitions (C++ PersistentStorageThread.cpp parity).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, OnceLock};

use crate::error::NetworkResult;

pub type PerGeneralMap = HashMap<i32, u32>;

pub const LOC_MIN: i32 = 1;
pub const LOC_MAX: i32 = 37;

#[derive(Debug, Clone, Default)]
pub struct PSPlayerStats {
    pub id: i32,
    pub wins: PerGeneralMap,
    pub losses: PerGeneralMap,
    pub games: PerGeneralMap,
    pub duration: PerGeneralMap,
    pub units_killed: PerGeneralMap,
    pub units_lost: PerGeneralMap,
    pub units_built: PerGeneralMap,
    pub buildings_killed: PerGeneralMap,
    pub buildings_lost: PerGeneralMap,
    pub buildings_built: PerGeneralMap,
    pub earnings: PerGeneralMap,
    pub tech_captured: PerGeneralMap,
    pub discons: PerGeneralMap,
    pub desyncs: PerGeneralMap,
    pub surrenders: PerGeneralMap,
    pub games_of_2p: PerGeneralMap,
    pub games_of_3p: PerGeneralMap,
    pub games_of_4p: PerGeneralMap,
    pub games_of_5p: PerGeneralMap,
    pub games_of_6p: PerGeneralMap,
    pub games_of_7p: PerGeneralMap,
    pub games_of_8p: PerGeneralMap,
    pub custom_games: PerGeneralMap,
    pub qm_games: PerGeneralMap,
    pub locale: i32,
    pub games_as_random: i32,
    pub options: String,
    pub system_spec: String,
    pub last_fps: f32,
    pub last_general: i32,
    pub games_in_row_with_last_general: i32,
    pub challenge_medals: i32,
    pub battle_honors: i32,
    pub qm_wins_in_a_row: i32,
    pub max_qm_wins_in_a_row: i32,
    pub wins_in_a_row: i32,
    pub max_wins_in_a_row: i32,
    pub losses_in_a_row: i32,
    pub max_losses_in_a_row: i32,
    pub discons_in_a_row: i32,
    pub max_discons_in_a_row: i32,
    pub desyncs_in_a_row: i32,
    pub max_desyncs_in_a_row: i32,
    pub built_particle_cannon: i32,
    pub built_nuke: i32,
    pub built_scud: i32,
    pub last_ladder_port: i32,
    pub last_ladder_host: String,
}

impl PSPlayerStats {
    pub fn reset(&mut self) {
        *self = PSPlayerStats::default();
    }

    pub fn incorporate(&mut self, other: &PSPlayerStats) {
        merge_per_general(&mut self.wins, &other.wins);
        merge_per_general(&mut self.losses, &other.losses);
        merge_per_general(&mut self.games, &other.games);
        merge_per_general(&mut self.duration, &other.duration);
        merge_per_general(&mut self.units_killed, &other.units_killed);
        merge_per_general(&mut self.units_lost, &other.units_lost);
        merge_per_general(&mut self.units_built, &other.units_built);
        merge_per_general(&mut self.buildings_killed, &other.buildings_killed);
        merge_per_general(&mut self.buildings_lost, &other.buildings_lost);
        merge_per_general(&mut self.buildings_built, &other.buildings_built);
        merge_per_general(&mut self.earnings, &other.earnings);
        merge_per_general(&mut self.tech_captured, &other.tech_captured);
        merge_per_general(&mut self.discons, &other.discons);
        merge_per_general(&mut self.desyncs, &other.desyncs);
        merge_per_general(&mut self.surrenders, &other.surrenders);
        merge_per_general(&mut self.games_of_2p, &other.games_of_2p);
        merge_per_general(&mut self.games_of_3p, &other.games_of_3p);
        merge_per_general(&mut self.games_of_4p, &other.games_of_4p);
        merge_per_general(&mut self.games_of_5p, &other.games_of_5p);
        merge_per_general(&mut self.games_of_6p, &other.games_of_6p);
        merge_per_general(&mut self.games_of_7p, &other.games_of_7p);
        merge_per_general(&mut self.games_of_8p, &other.games_of_8p);
        merge_per_general(&mut self.custom_games, &other.custom_games);
        merge_per_general(&mut self.qm_games, &other.qm_games);
        self.locale = other.locale;
        self.games_as_random = other.games_as_random;
        self.options = other.options.clone();
        self.system_spec = other.system_spec.clone();
        self.last_fps = other.last_fps;
        self.last_general = other.last_general;
        self.games_in_row_with_last_general = other.games_in_row_with_last_general;
        self.challenge_medals = other.challenge_medals;
        self.battle_honors = other.battle_honors;
        self.qm_wins_in_a_row = other.qm_wins_in_a_row;
        self.max_qm_wins_in_a_row = other.max_qm_wins_in_a_row;
        self.wins_in_a_row = other.wins_in_a_row;
        self.max_wins_in_a_row = other.max_wins_in_a_row;
        self.losses_in_a_row = other.losses_in_a_row;
        self.max_losses_in_a_row = other.max_losses_in_a_row;
        self.discons_in_a_row = other.discons_in_a_row;
        self.max_discons_in_a_row = other.max_discons_in_a_row;
        self.desyncs_in_a_row = other.desyncs_in_a_row;
        self.max_desyncs_in_a_row = other.max_desyncs_in_a_row;
        self.built_particle_cannon = other.built_particle_cannon;
        self.built_nuke = other.built_nuke;
        self.built_scud = other.built_scud;
        self.last_ladder_port = other.last_ladder_port;
        self.last_ladder_host = other.last_ladder_host.clone();
    }
}

fn merge_per_general(target: &mut PerGeneralMap, other: &PerGeneralMap) {
    for (key, value) in other {
        *target.entry(*key).or_insert(0) += *value;
    }
}

#[derive(Debug, Clone)]
pub enum PSRequestType {
    ReadPlayerStats,
    UpdatePlayerStats,
    UpdatePlayerLocale,
    ReadCdKeyStats,
    SendGameResultsToGameSpy,
}

#[derive(Debug, Clone)]
pub struct PSRequest {
    pub request_type: PSRequestType,
    pub player: PSPlayerStats,
    pub cdkey: String,
    pub nick: String,
    pub password: String,
    pub email: String,
    pub add_discon: bool,
    pub add_desync: bool,
    pub last_house: i32,
    pub results: String,
}

impl Default for PSRequest {
    fn default() -> Self {
        Self {
            request_type: PSRequestType::ReadPlayerStats,
            player: PSPlayerStats::default(),
            cdkey: String::new(),
            nick: String::new(),
            password: String::new(),
            email: String::new(),
            add_discon: false,
            add_desync: false,
            last_house: 0,
            results: String::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PSResponseType {
    PlayerStats,
    CouldNotConnect,
    Preorder,
}

#[derive(Debug, Clone)]
pub struct PSResponse {
    pub response_type: PSResponseType,
    pub player: PSPlayerStats,
    pub preorder: bool,
}

impl Default for PSResponse {
    fn default() -> Self {
        Self {
            response_type: PSResponseType::PlayerStats,
            player: PSPlayerStats::default(),
            preorder: false,
        }
    }
}

#[derive(Default)]
pub struct GameSpyPSMessageQueue {
    requests: VecDeque<PSRequest>,
    responses: VecDeque<PSResponse>,
    tracked_stats: HashMap<i32, PSPlayerStats>,
    running: bool,
}

impl GameSpyPSMessageQueue {
    pub fn start_thread(&mut self) {
        self.running = true;
    }

    pub fn end_thread(&mut self) {
        self.running = false;
    }

    pub fn is_thread_running(&self) -> bool {
        self.running
    }

    pub fn add_request(&mut self, req: PSRequest) {
        self.requests.push_back(req);
    }

    pub fn get_request(&mut self) -> Option<PSRequest> {
        self.requests.pop_front()
    }

    pub fn add_response(&mut self, resp: PSResponse) {
        self.responses.push_back(resp);
    }

    pub fn get_response(&mut self) -> Option<PSResponse> {
        self.responses.pop_front()
    }

    pub fn track_player_stats(&mut self, stats: PSPlayerStats) {
        self.tracked_stats.insert(stats.id, stats);
    }

    pub fn find_player_stats_by_id(&self, id: i32) -> PSPlayerStats {
        self.tracked_stats.get(&id).cloned().unwrap_or_default()
    }

    pub fn format_player_kv_pairs(stats: &PSPlayerStats) -> String {
        let mut out = String::new();
        append_kv_map(&mut out, "wins", &stats.wins);
        append_kv_map(&mut out, "losses", &stats.losses);
        append_kv_map(&mut out, "games", &stats.games);
        append_kv_map(&mut out, "duration", &stats.duration);
        append_kv_map(&mut out, "unitsKilled", &stats.units_killed);
        append_kv_map(&mut out, "unitsLost", &stats.units_lost);
        append_kv_map(&mut out, "unitsBuilt", &stats.units_built);
        append_kv_map(&mut out, "buildingsKilled", &stats.buildings_killed);
        append_kv_map(&mut out, "buildingsLost", &stats.buildings_lost);
        append_kv_map(&mut out, "buildingsBuilt", &stats.buildings_built);
        append_kv_map(&mut out, "earnings", &stats.earnings);
        append_kv_map(&mut out, "techCaptured", &stats.tech_captured);
        append_kv_map(&mut out, "discons", &stats.discons);
        append_kv_map(&mut out, "desyncs", &stats.desyncs);
        append_kv_map(&mut out, "surrenders", &stats.surrenders);
        append_kv_map(&mut out, "games2p", &stats.games_of_2p);
        append_kv_map(&mut out, "games3p", &stats.games_of_3p);
        append_kv_map(&mut out, "games4p", &stats.games_of_4p);
        append_kv_map(&mut out, "games5p", &stats.games_of_5p);
        append_kv_map(&mut out, "games6p", &stats.games_of_6p);
        append_kv_map(&mut out, "games7p", &stats.games_of_7p);
        append_kv_map(&mut out, "games8p", &stats.games_of_8p);
        append_kv_map(&mut out, "custom", &stats.custom_games);
        append_kv_map(&mut out, "qm", &stats.qm_games);
        out
    }

    pub fn parse_player_kv_pairs(kv_pairs: &str) -> PSPlayerStats {
        let mut stats = PSPlayerStats::default();
        for part in kv_pairs.split(';') {
            if let Some((key, value)) = part.split_once('=') {
                if let Some(map) = map_for_key(&mut stats, key) {
                    parse_kv_map(map, value);
                }
            }
        }
        stats
    }
}

fn append_kv_map(out: &mut String, name: &str, map: &PerGeneralMap) {
    let mut entries: Vec<String> = map.iter().map(|(k, v)| format!("{k}:{v}")).collect();
    entries.sort();
    out.push_str(name);
    out.push('=');
    out.push_str(&entries.join(","));
    out.push(';');
}

fn parse_kv_map(map: &mut PerGeneralMap, value: &str) {
    for pair in value.split(',') {
        if let Some((k, v)) = pair.split_once(':') {
            if let (Ok(key), Ok(val)) = (k.parse::<i32>(), v.parse::<u32>()) {
                map.insert(key, val);
            }
        }
    }
}

fn map_for_key<'a>(stats: &'a mut PSPlayerStats, key: &str) -> Option<&'a mut PerGeneralMap> {
    match key {
        "wins" => Some(&mut stats.wins),
        "losses" => Some(&mut stats.losses),
        "games" => Some(&mut stats.games),
        "duration" => Some(&mut stats.duration),
        "unitsKilled" => Some(&mut stats.units_killed),
        "unitsLost" => Some(&mut stats.units_lost),
        "unitsBuilt" => Some(&mut stats.units_built),
        "buildingsKilled" => Some(&mut stats.buildings_killed),
        "buildingsLost" => Some(&mut stats.buildings_lost),
        "buildingsBuilt" => Some(&mut stats.buildings_built),
        "earnings" => Some(&mut stats.earnings),
        "techCaptured" => Some(&mut stats.tech_captured),
        "discons" => Some(&mut stats.discons),
        "desyncs" => Some(&mut stats.desyncs),
        "surrenders" => Some(&mut stats.surrenders),
        "games2p" => Some(&mut stats.games_of_2p),
        "games3p" => Some(&mut stats.games_of_3p),
        "games4p" => Some(&mut stats.games_of_4p),
        "games5p" => Some(&mut stats.games_of_5p),
        "games6p" => Some(&mut stats.games_of_6p),
        "games7p" => Some(&mut stats.games_of_7p),
        "games8p" => Some(&mut stats.games_of_8p),
        "custom" => Some(&mut stats.custom_games),
        "qm" => Some(&mut stats.qm_games),
        _ => None,
    }
}

static THE_GAMESPY_PS_QUEUE: OnceLock<Arc<Mutex<GameSpyPSMessageQueue>>> = OnceLock::new();

pub fn init_ps_message_queue() -> Arc<Mutex<GameSpyPSMessageQueue>> {
    THE_GAMESPY_PS_QUEUE
        .get_or_init(|| Arc::new(Mutex::new(GameSpyPSMessageQueue::default())))
        .clone()
}

pub fn get_ps_message_queue() -> Option<Arc<Mutex<GameSpyPSMessageQueue>>> {
    THE_GAMESPY_PS_QUEUE.get().cloned()
}

pub fn teardown_ps_message_queue() {
    if let Some(queue) = THE_GAMESPY_PS_QUEUE.get() {
        if let Ok(mut guard) = queue.lock() {
            guard.requests.clear();
            guard.responses.clear();
        }
    }
}
