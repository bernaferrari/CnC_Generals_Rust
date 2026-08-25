//! EVA voice system (GameClient/Eva.cpp).

use std::sync::{Mutex, OnceLock};

use game_engine::common::ini::{
    FieldParse, INI, INIError, INILoadType, INIResult, register_block_parser,
};
use game_engine::common::random_value::get_game_client_random_value;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::{EvaEvent as LogicEvaEvent, TheAudio, TheEva as LogicEva, TheGameLogic};
use gamelogic::player::ThePlayerList;

const EVA_MESSAGE_NAMES: [&str; 53] = [
    "LOWPOWER",
    "INSUFFICIENTFUNDS",
    "SUPERWEAPONDETECTED_OWN_PARTICLECANNON",
    "SUPERWEAPONDETECTED_OWN_NUKE",
    "SUPERWEAPONDETECTED_OWN_SCUDSTORM",
    "SUPERWEAPONDETECTED_ALLY_PARTICLECANNON",
    "SUPERWEAPONDETECTED_ALLY_NUKE",
    "SUPERWEAPONDETECTED_ALLY_SCUDSTORM",
    "SUPERWEAPONDETECTED_ENEMY_PARTICLECANNON",
    "SUPERWEAPONDETECTED_ENEMY_NUKE",
    "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM",
    "SUPERWEAPONLAUNCHED_OWN_PARTICLECANNON",
    "SUPERWEAPONLAUNCHED_OWN_NUKE",
    "SUPERWEAPONLAUNCHED_OWN_SCUDSTORM",
    "SUPERWEAPONLAUNCHED_ALLY_PARTICLECANNON",
    "SUPERWEAPONLAUNCHED_ALLY_NUKE",
    "SUPERWEAPONLAUNCHED_ALLY_SCUDSTORM",
    "SUPERWEAPONLAUNCHED_ENEMY_PARTICLECANNON",
    "SUPERWEAPONLAUNCHED_ENEMY_NUKE",
    "SUPERWEAPONLAUNCHED_ENEMY_SCUDSTORM",
    "SUPERWEAPONREADY_OWN_PARTICLECANNON",
    "SUPERWEAPONREADY_OWN_NUKE",
    "SUPERWEAPONREADY_OWN_SCUDSTORM",
    "SUPERWEAPONREADY_ALLY_PARTICLECANNON",
    "SUPERWEAPONREADY_ALLY_NUKE",
    "SUPERWEAPONREADY_ALLY_SCUDSTORM",
    "SUPERWEAPONREADY_ENEMY_PARTICLECANNON",
    "SUPERWEAPONREADY_ENEMY_NUKE",
    "SUPERWEAPONREADY_ENEMY_SCUDSTORM",
    "BUILDINGLOST",
    "BASEUNDERATTACK",
    "ALLYUNDERATTACK",
    "BEACONDETECTED",
    "ENEMYBLACKLOTUSDETECTED",
    "ENEMYJARMENKELLDETECTED",
    "ENEMYCOLONELBURTONDETECTED",
    "OWNBLACKLOTUSDETECTED",
    "OWNJARMENKELLDETECTED",
    "OWNCOLONELBURTONDETECTED",
    "UNITLOST",
    "GENERALLEVELUP",
    "VEHICLESTOLEN",
    "BUILDINGSTOLEN",
    "CASHSTOLEN",
    "UPGRADECOMPLETE",
    "BUILDINGBEINGSTOLEN",
    "BUILDINGSABOTAGED",
    "SUPERWEAPONLAUNCHED_OWN_GPS_SCRAMBLER",
    "SUPERWEAPONLAUNCHED_ALLY_GPS_SCRAMBLER",
    "SUPERWEAPONLAUNCHED_ENEMY_GPS_SCRAMBLER",
    "SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK",
    "SUPERWEAPONLAUNCHED_ALLY_SNEAK_ATTACK",
    "SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK",
];

const EVA_COUNT: usize = EVA_MESSAGE_NAMES.len();
const FOREVER_FRAMES: u32 = 0x3fffffff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EvaMessage {
    LowPower = 0,
    InsufficientFunds,
    SuperweaponDetectedOwnParticleCannon,
    SuperweaponDetectedOwnNuke,
    SuperweaponDetectedOwnScudStorm,
    SuperweaponDetectedAllyParticleCannon,
    SuperweaponDetectedAllyNuke,
    SuperweaponDetectedAllyScudStorm,
    SuperweaponDetectedEnemyParticleCannon,
    SuperweaponDetectedEnemyNuke,
    SuperweaponDetectedEnemyScudStorm,
    SuperweaponLaunchedOwnParticleCannon,
    SuperweaponLaunchedOwnNuke,
    SuperweaponLaunchedOwnScudStorm,
    SuperweaponLaunchedAllyParticleCannon,
    SuperweaponLaunchedAllyNuke,
    SuperweaponLaunchedAllyScudStorm,
    SuperweaponLaunchedEnemyParticleCannon,
    SuperweaponLaunchedEnemyNuke,
    SuperweaponLaunchedEnemyScudStorm,
    SuperweaponReadyOwnParticleCannon,
    SuperweaponReadyOwnNuke,
    SuperweaponReadyOwnScudStorm,
    SuperweaponReadyAllyParticleCannon,
    SuperweaponReadyAllyNuke,
    SuperweaponReadyAllyScudStorm,
    SuperweaponReadyEnemyParticleCannon,
    SuperweaponReadyEnemyNuke,
    SuperweaponReadyEnemyScudStorm,
    BuildingLost,
    BaseUnderAttack,
    AllyUnderAttack,
    BeaconDetected,
    EnemyBlackLotusDetected,
    EnemyJarmenKellDetected,
    EnemyColonelBurtonDetected,
    OwnBlackLotusDetected,
    OwnJarmenKellDetected,
    OwnColonelBurtonDetected,
    UnitLost,
    GeneralLevelUp,
    VehicleStolen,
    BuildingStolen,
    CashStolen,
    UpgradeComplete,
    BuildingBeingStolen,
    BuildingSabotaged,
    SuperweaponLaunchedOwnGpsScrambler,
    SuperweaponLaunchedAllyGpsScrambler,
    SuperweaponLaunchedEnemyGpsScrambler,
    SuperweaponLaunchedOwnSneakAttack,
    SuperweaponLaunchedAllySneakAttack,
    SuperweaponLaunchedEnemySneakAttack,
}

impl EvaMessage {
    pub fn from_name(name: &str) -> Option<Self> {
        EVA_MESSAGE_NAMES
            .iter()
            .position(|entry| entry.eq_ignore_ascii_case(name))
            .and_then(Self::from_index)
    }

    pub fn to_name(self) -> &'static str {
        EVA_MESSAGE_NAMES[self.as_index()]
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::LowPower),
            1 => Some(Self::InsufficientFunds),
            2 => Some(Self::SuperweaponDetectedOwnParticleCannon),
            3 => Some(Self::SuperweaponDetectedOwnNuke),
            4 => Some(Self::SuperweaponDetectedOwnScudStorm),
            5 => Some(Self::SuperweaponDetectedAllyParticleCannon),
            6 => Some(Self::SuperweaponDetectedAllyNuke),
            7 => Some(Self::SuperweaponDetectedAllyScudStorm),
            8 => Some(Self::SuperweaponDetectedEnemyParticleCannon),
            9 => Some(Self::SuperweaponDetectedEnemyNuke),
            10 => Some(Self::SuperweaponDetectedEnemyScudStorm),
            11 => Some(Self::SuperweaponLaunchedOwnParticleCannon),
            12 => Some(Self::SuperweaponLaunchedOwnNuke),
            13 => Some(Self::SuperweaponLaunchedOwnScudStorm),
            14 => Some(Self::SuperweaponLaunchedAllyParticleCannon),
            15 => Some(Self::SuperweaponLaunchedAllyNuke),
            16 => Some(Self::SuperweaponLaunchedAllyScudStorm),
            17 => Some(Self::SuperweaponLaunchedEnemyParticleCannon),
            18 => Some(Self::SuperweaponLaunchedEnemyNuke),
            19 => Some(Self::SuperweaponLaunchedEnemyScudStorm),
            20 => Some(Self::SuperweaponReadyOwnParticleCannon),
            21 => Some(Self::SuperweaponReadyOwnNuke),
            22 => Some(Self::SuperweaponReadyOwnScudStorm),
            23 => Some(Self::SuperweaponReadyAllyParticleCannon),
            24 => Some(Self::SuperweaponReadyAllyNuke),
            25 => Some(Self::SuperweaponReadyAllyScudStorm),
            26 => Some(Self::SuperweaponReadyEnemyParticleCannon),
            27 => Some(Self::SuperweaponReadyEnemyNuke),
            28 => Some(Self::SuperweaponReadyEnemyScudStorm),
            29 => Some(Self::BuildingLost),
            30 => Some(Self::BaseUnderAttack),
            31 => Some(Self::AllyUnderAttack),
            32 => Some(Self::BeaconDetected),
            33 => Some(Self::EnemyBlackLotusDetected),
            34 => Some(Self::EnemyJarmenKellDetected),
            35 => Some(Self::EnemyColonelBurtonDetected),
            36 => Some(Self::OwnBlackLotusDetected),
            37 => Some(Self::OwnJarmenKellDetected),
            38 => Some(Self::OwnColonelBurtonDetected),
            39 => Some(Self::UnitLost),
            40 => Some(Self::GeneralLevelUp),
            41 => Some(Self::VehicleStolen),
            42 => Some(Self::BuildingStolen),
            43 => Some(Self::CashStolen),
            44 => Some(Self::UpgradeComplete),
            45 => Some(Self::BuildingBeingStolen),
            46 => Some(Self::BuildingSabotaged),
            47 => Some(Self::SuperweaponLaunchedOwnGpsScrambler),
            48 => Some(Self::SuperweaponLaunchedAllyGpsScrambler),
            49 => Some(Self::SuperweaponLaunchedEnemyGpsScrambler),
            50 => Some(Self::SuperweaponLaunchedOwnSneakAttack),
            51 => Some(Self::SuperweaponLaunchedAllySneakAttack),
            52 => Some(Self::SuperweaponLaunchedEnemySneakAttack),
            _ => None,
        }
    }

    pub fn as_index(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Default)]
pub struct EvaSideSounds {
    side: String,
    sound_names: Vec<String>,
}

impl EvaSideSounds {
    fn field_parse() -> &'static [FieldParse<EvaSideSounds>] {
        &[
            FieldParse {
                token: "Side",
                parse: parse_side,
            },
            FieldParse {
                token: "Sounds",
                parse: parse_sounds,
            },
        ]
    }
}

fn parse_side(_ini: &mut INI, target: &mut EvaSideSounds, tokens: &[&str]) -> INIResult<()> {
    let Some(token) = tokens.first() else {
        return Err(INIError::InvalidData);
    };
    target.side = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_sounds(_ini: &mut INI, target: &mut EvaSideSounds, tokens: &[&str]) -> INIResult<()> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }
    target.sound_names = tokens
        .iter()
        .filter_map(|token| INI::parse_ascii_string(token).ok())
        .collect();
    Ok(())
}

#[derive(Debug, Clone)]
pub struct EvaCheckInfo {
    message: EvaMessage,
    priority: u32,
    frames_between_checks: u32,
    frames_to_expire: u32,
    eva_side_sounds: Vec<EvaSideSounds>,
}

impl EvaCheckInfo {
    fn new(message: EvaMessage) -> Self {
        Self {
            message,
            priority: 1,
            frames_between_checks: 900,
            frames_to_expire: 150,
            eva_side_sounds: Vec::new(),
        }
    }

    fn field_parse() -> &'static [FieldParse<EvaCheckInfo>] {
        &[
            FieldParse {
                token: "Priority",
                parse: parse_priority,
            },
            FieldParse {
                token: "TimeBetweenChecksMS",
                parse: parse_time_between_checks,
            },
            FieldParse {
                token: "ExpirationTimeMS",
                parse: parse_expiration_time,
            },
            FieldParse {
                token: "SideSounds",
                parse: parse_side_sounds_list,
            },
        ]
    }

    pub fn message(&self) -> EvaMessage {
        self.message
    }

    pub fn priority(&self) -> u32 {
        self.priority
    }

    pub fn frames_between_checks(&self) -> u32 {
        self.frames_between_checks
    }

    pub fn frames_to_expire(&self) -> u32 {
        self.frames_to_expire
    }
}

fn parse_priority(_ini: &mut INI, target: &mut EvaCheckInfo, tokens: &[&str]) -> INIResult<()> {
    let Some(token) = tokens.first() else {
        return Err(INIError::InvalidData);
    };
    target.priority = INI::parse_unsigned_int(token)?;
    Ok(())
}

fn parse_time_between_checks(
    _ini: &mut INI,
    target: &mut EvaCheckInfo,
    tokens: &[&str],
) -> INIResult<()> {
    let Some(token) = tokens.first() else {
        return Err(INIError::InvalidData);
    };
    target.frames_between_checks = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_expiration_time(
    _ini: &mut INI,
    target: &mut EvaCheckInfo,
    tokens: &[&str],
) -> INIResult<()> {
    let Some(token) = tokens.first() else {
        return Err(INIError::InvalidData);
    };
    target.frames_to_expire = if *token == "-1" {
        FOREVER_FRAMES
    } else {
        INI::parse_duration_unsigned_int(token)?
    };
    Ok(())
}

fn parse_side_sounds_list(
    ini: &mut INI,
    target: &mut EvaCheckInfo,
    _tokens: &[&str],
) -> INIResult<()> {
    let mut side_sounds = EvaSideSounds::default();
    parse_eva_side_sounds_fields(ini, &mut side_sounds)?;
    target.eva_side_sounds.push(side_sounds);
    Ok(())
}

fn parse_eva_field_line(line: &str) -> Option<(&str, Vec<&str>)> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let key_end = trimmed
        .find(|c: char| c.is_whitespace() || c == '=')
        .unwrap_or(trimmed.len());
    let key = &trimmed[..key_end];
    if key.is_empty() {
        return None;
    }

    let values = trimmed[key_end..]
        .split(|c: char| c.is_whitespace() || c == '=')
        .filter(|token| !token.is_empty())
        .collect();

    Some((key, values))
}

fn parse_eva_side_sounds_fields(ini: &mut INI, target: &mut EvaSideSounds) -> INIResult<()> {
    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::MissingEndToken);
        }

        let line = ini.get_buffer().to_string();
        let Some((key, value_tokens)) = parse_eva_field_line(&line) else {
            continue;
        };

        if key.eq_ignore_ascii_case("End") {
            break;
        }

        if key.eq_ignore_ascii_case("Side") {
            parse_side(ini, target, &value_tokens)?;
        } else if key.eq_ignore_ascii_case("Sounds") {
            parse_sounds(ini, target, &value_tokens)?;
        }
    }

    Ok(())
}

fn parse_eva_check_info_fields(ini: &mut INI, target: &mut EvaCheckInfo) -> INIResult<()> {
    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::MissingEndToken);
        }

        let line = ini.get_buffer().to_string();
        let Some((key, value_tokens)) = parse_eva_field_line(&line) else {
            continue;
        };

        if key.eq_ignore_ascii_case("End") {
            break;
        }

        if key.eq_ignore_ascii_case("Priority") {
            parse_priority(ini, target, &value_tokens)?;
        } else if key.eq_ignore_ascii_case("TimeBetweenChecksMS") {
            parse_time_between_checks(ini, target, &value_tokens)?;
        } else if key.eq_ignore_ascii_case("ExpirationTimeMS") {
            parse_expiration_time(ini, target, &value_tokens)?;
        } else if key.eq_ignore_ascii_case("SideSounds") {
            parse_side_sounds_list(ini, target, &value_tokens)?;
        }
    }

    Ok(())
}

fn skip_eva_event_block(ini: &mut INI) -> INIResult<()> {
    let mut depth = 0usize;

    loop {
        ini.read_line()?;
        if ini.is_eof() {
            return Err(INIError::MissingEndToken);
        }

        let line = ini.get_buffer().to_string();
        let Some((key, _)) = parse_eva_field_line(&line) else {
            continue;
        };

        if key.eq_ignore_ascii_case("End") {
            if depth == 0 {
                break;
            }
            depth -= 1;
        } else if key.eq_ignore_ascii_case("SideSounds") {
            depth += 1;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct EvaCheck {
    eva_info: EvaMessage,
    triggered_on_frame: u32,
    time_for_next_check: u32,
    already_played: bool,
}

#[derive(Debug)]
pub struct Eva {
    checks: Vec<EvaCheck>,
    all_check_infos: Vec<EvaCheckInfo>,
    should_play: [bool; EVA_COUNT],
    message_being_tested: EvaMessage,
    enabled: bool,
    eva_speech: AudioEventRts,
}

impl Default for Eva {
    fn default() -> Self {
        Self::new()
    }
}

impl Eva {
    pub fn new() -> Self {
        Self {
            checks: Vec::new(),
            all_check_infos: Vec::new(),
            should_play: [false; EVA_COUNT],
            message_being_tested: EvaMessage::LowPower,
            enabled: true,
            eva_speech: AudioEventRts::default(),
        }
    }

    pub fn init(&mut self) -> INIResult<()> {
        let _ = register_block_parser("EvaEvent", parse_eva_event);
        let mut ini = INI::new();
        ini.load("Data/INI/Eva.ini", INILoadType::Overwrite)?;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.checks.clear();
        for flag in &mut self.should_play {
            *flag = false;
        }
        self.enabled = true;
    }

    pub fn update(&mut self) {
        self.sync_enabled_from_logic();

        if !self.enabled {
            return;
        }

        // C++ Eva.cpp:271 `TheGameLogic->getFrame()`. Live AuthorityOnly does
        // not tick the crate GameLogic clock, so prefer the host-published frame.
        let current_frame = eva_logic_frame();
        if current_frame < 2 {
            return;
        }

        self.ingest_logic_events();

        for index in 0..EVA_COUNT {
            let Some(message) = EvaMessage::from_index(index) else {
                continue;
            };
            if self.is_time_for_check(message) && self.message_should_play(message) {
                self.play_message(message, current_frame);
            }
        }

        self.process_playing_messages(current_frame);

        for flag in &mut self.should_play {
            *flag = false;
        }
    }

    pub fn set_should_play(&mut self, message: EvaMessage) {
        self.should_play[message.as_index()] = true;
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        for flag in &mut self.should_play {
            *flag = false;
        }
        self.enabled = enabled;
    }

    fn sync_enabled_from_logic(&mut self) {
        let Ok(enabled) = LogicEva::is_enabled() else {
            return;
        };
        if self.enabled != enabled {
            self.set_enabled(enabled);
        }
    }

    pub fn new_eva_check_info(&mut self, name: &str) -> Option<&mut EvaCheckInfo> {
        let message = EvaMessage::from_name(name)?;
        if self
            .all_check_infos
            .iter()
            .any(|info| info.message == message)
        {
            return None;
        }

        self.all_check_infos.push(EvaCheckInfo::new(message));
        self.all_check_infos.last_mut()
    }

    fn get_eva_check_info(&self, message: EvaMessage) -> Option<&EvaCheckInfo> {
        self.all_check_infos
            .iter()
            .find(|info| info.message == message)
    }

    pub fn get_eva_check_info_by_name(&self, name: &str) -> Option<&EvaCheckInfo> {
        let message = EvaMessage::from_name(name)?;
        self.get_eva_check_info(message)
    }

    fn is_time_for_check(&self, message: EvaMessage) -> bool {
        !self.checks.iter().any(|check| check.eva_info == message)
    }

    fn message_should_play(&mut self, message: EvaMessage) -> bool {
        self.message_should_play_with_local_player(message, Self::has_local_player())
    }

    fn message_should_play_with_local_player(
        &mut self,
        message: EvaMessage,
        has_local_player: bool,
    ) -> bool {
        if !has_local_player {
            return false;
        }

        self.message_being_tested = message;
        match message {
            EvaMessage::LowPower => self.should_play_low_power(),
            _ => self.should_play_generic(),
        }
    }

    fn has_local_player() -> bool {
        ThePlayerList()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned())
            .is_some()
    }

    fn should_play_low_power(&self) -> bool {
        // C++ Eva.cpp:422 polls localPlayer->getEnergy()->hasSufficientPower().
        // Live leftover ThePlayerList Energy is not host power_available.
        if let Some(sufficient) = eva_host_sufficient_power() {
            return !sufficient;
        }
        let Ok(list) = ThePlayerList().read() else {
            return false;
        };
        let Some(local_player) = list.get_local_player().cloned() else {
            return false;
        };
        let Ok(local_player) = local_player.read() else {
            return false;
        };
        !local_player.get_energy().has_sufficient_power()
    }

    fn should_play_generic(&mut self) -> bool {
        let index = self.message_being_tested.as_index();
        if self.should_play[index] {
            self.should_play[index] = false;
            return true;
        }
        false
    }

    fn play_message(&mut self, message: EvaMessage, current_frame: u32) {
        let Some(info) = self.get_eva_check_info(message) else {
            return;
        };

        self.checks.push(EvaCheck {
            eva_info: message,
            triggered_on_frame: current_frame,
            time_for_next_check: current_frame + info.frames_between_checks,
            already_played: false,
        });
    }

    fn process_playing_messages(&mut self, current_frame: u32) {
        let mut idx = 0;
        while idx < self.checks.len() {
            let check = &self.checks[idx];
            if check.already_played && check.time_for_next_check <= current_frame + 1 {
                self.checks.remove(idx);
                continue;
            }
            if !check.already_played {
                if let Some(info) = self.get_eva_check_info(check.eva_info) {
                    if check.triggered_on_frame + info.frames_to_expire <= current_frame {
                        self.checks.remove(idx);
                        continue;
                    }
                }
            }
            idx += 1;
        }

        if self.checks.is_empty() {
            return;
        }

        if TheAudio::get()
            .map(|audio| audio.is_currently_playing(self.eva_speech.get_playing_handle()))
            .unwrap_or(false)
        {
            return;
        }

        let mut best_index: Option<usize> = None;
        let mut best_priority = 0;
        for (index, check) in self.checks.iter().enumerate() {
            if check.already_played {
                continue;
            }
            let Some(info) = self.get_eva_check_info(check.eva_info) else {
                continue;
            };
            if info.priority > best_priority {
                best_priority = info.priority;
                best_index = Some(index);
            }
        }

        let Some(best_index) = best_index else {
            return;
        };

        let (frames_between_checks, eva_side_sounds) =
            match self.get_eva_check_info(self.checks[best_index].eva_info) {
                Some(info) => (info.frames_between_checks, info.eva_side_sounds.clone()),
                None => return,
            };

        let Ok(list) = ThePlayerList().read() else {
            return;
        };
        let Some(local_player) = list.get_local_player().cloned() else {
            return;
        };
        let Ok(local_player) = local_player.read() else {
            return;
        };
        let side = local_player.get_side();

        self.eva_speech.set_event_name(String::new());
        for side_sounds in &eva_side_sounds {
            if side_sounds.side.eq_ignore_ascii_case(side) {
                if !side_sounds.sound_names.is_empty() {
                    let choice = get_game_client_random_value(
                        0,
                        side_sounds.sound_names.len().saturating_sub(1) as i32,
                    ) as usize;
                    if let Some(sound) = side_sounds.sound_names.get(choice) {
                        self.eva_speech.set_event_name(sound.to_string());
                    }
                }
                break;
            }
        }

        self.checks[best_index].already_played = true;
        self.checks[best_index].time_for_next_check = current_frame + frames_between_checks;

        self.eva_speech
            .set_player_index(local_player.get_player_index() as u32);

        let handle = TheAudio::get()
            .map(|audio| audio.add_audio_event(&self.eva_speech))
            .unwrap_or(0);
        self.eva_speech.set_playing_handle(handle);
    }

    fn ingest_logic_events(&mut self) {
        let Ok(events) = LogicEva::drain_events() else {
            return;
        };
        for event in events {
            if let Some(message) = map_logic_event(event) {
                self.set_should_play(message);
            }
        }
    }
}

fn map_logic_event(event: LogicEvaEvent) -> Option<EvaMessage> {
    match event {
        LogicEvaEvent::LowPower => Some(EvaMessage::LowPower),
        LogicEvaEvent::InsufficientFunds => Some(EvaMessage::InsufficientFunds),
        LogicEvaEvent::SuperweaponDetectedOwnParticleCannon => {
            Some(EvaMessage::SuperweaponDetectedOwnParticleCannon)
        }
        LogicEvaEvent::SuperweaponDetectedOwnNuke => Some(EvaMessage::SuperweaponDetectedOwnNuke),
        LogicEvaEvent::SuperweaponDetectedOwnScudStorm => {
            Some(EvaMessage::SuperweaponDetectedOwnScudStorm)
        }
        LogicEvaEvent::SuperweaponDetectedAllyParticleCannon => {
            Some(EvaMessage::SuperweaponDetectedAllyParticleCannon)
        }
        LogicEvaEvent::SuperweaponDetectedAllyNuke => Some(EvaMessage::SuperweaponDetectedAllyNuke),
        LogicEvaEvent::SuperweaponDetectedAllyScudStorm => {
            Some(EvaMessage::SuperweaponDetectedAllyScudStorm)
        }
        LogicEvaEvent::SuperweaponDetectedEnemyParticleCannon => {
            Some(EvaMessage::SuperweaponDetectedEnemyParticleCannon)
        }
        LogicEvaEvent::SuperweaponDetectedEnemyNuke => {
            Some(EvaMessage::SuperweaponDetectedEnemyNuke)
        }
        LogicEvaEvent::SuperweaponDetectedEnemyScudStorm => {
            Some(EvaMessage::SuperweaponDetectedEnemyScudStorm)
        }
        LogicEvaEvent::SuperweaponLaunchedOwnParticleCannon => {
            Some(EvaMessage::SuperweaponLaunchedOwnParticleCannon)
        }
        LogicEvaEvent::SuperweaponLaunchedOwnNuke => Some(EvaMessage::SuperweaponLaunchedOwnNuke),
        LogicEvaEvent::SuperweaponLaunchedOwnScudStorm => {
            Some(EvaMessage::SuperweaponLaunchedOwnScudStorm)
        }
        LogicEvaEvent::SuperweaponLaunchedAllyParticleCannon => {
            Some(EvaMessage::SuperweaponLaunchedAllyParticleCannon)
        }
        LogicEvaEvent::SuperweaponLaunchedAllyNuke => Some(EvaMessage::SuperweaponLaunchedAllyNuke),
        LogicEvaEvent::SuperweaponLaunchedAllyScudStorm => {
            Some(EvaMessage::SuperweaponLaunchedAllyScudStorm)
        }
        LogicEvaEvent::SuperweaponLaunchedEnemyParticleCannon => {
            Some(EvaMessage::SuperweaponLaunchedEnemyParticleCannon)
        }
        LogicEvaEvent::SuperweaponLaunchedEnemyNuke => {
            Some(EvaMessage::SuperweaponLaunchedEnemyNuke)
        }
        LogicEvaEvent::SuperweaponLaunchedEnemyScudStorm => {
            Some(EvaMessage::SuperweaponLaunchedEnemyScudStorm)
        }
        LogicEvaEvent::SuperweaponReadyOwnParticleCannon => {
            Some(EvaMessage::SuperweaponReadyOwnParticleCannon)
        }
        LogicEvaEvent::SuperweaponReadyOwnNuke => Some(EvaMessage::SuperweaponReadyOwnNuke),
        LogicEvaEvent::SuperweaponReadyOwnScudStorm => {
            Some(EvaMessage::SuperweaponReadyOwnScudStorm)
        }
        LogicEvaEvent::SuperweaponReadyAllyParticleCannon => {
            Some(EvaMessage::SuperweaponReadyAllyParticleCannon)
        }
        LogicEvaEvent::SuperweaponReadyAllyNuke => Some(EvaMessage::SuperweaponReadyAllyNuke),
        LogicEvaEvent::SuperweaponReadyAllyScudStorm => {
            Some(EvaMessage::SuperweaponReadyAllyScudStorm)
        }
        LogicEvaEvent::SuperweaponReadyEnemyParticleCannon => {
            Some(EvaMessage::SuperweaponReadyEnemyParticleCannon)
        }
        LogicEvaEvent::SuperweaponReadyEnemyNuke => Some(EvaMessage::SuperweaponReadyEnemyNuke),
        LogicEvaEvent::SuperweaponReadyEnemyScudStorm => {
            Some(EvaMessage::SuperweaponReadyEnemyScudStorm)
        }
        LogicEvaEvent::BuildingLost => Some(EvaMessage::BuildingLost),
        LogicEvaEvent::BaseUnderAttack => Some(EvaMessage::BaseUnderAttack),
        LogicEvaEvent::AllyUnderAttack => Some(EvaMessage::AllyUnderAttack),
        LogicEvaEvent::BeaconDetected => Some(EvaMessage::BeaconDetected),
        LogicEvaEvent::EnemyBlackLotusDetected => Some(EvaMessage::EnemyBlackLotusDetected),
        LogicEvaEvent::EnemyJarmenKellDetected => Some(EvaMessage::EnemyJarmenKellDetected),
        LogicEvaEvent::EnemyColonelBurtonDetected => Some(EvaMessage::EnemyColonelBurtonDetected),
        LogicEvaEvent::OwnBlackLotusDetected => Some(EvaMessage::OwnBlackLotusDetected),
        LogicEvaEvent::OwnJarmenKellDetected => Some(EvaMessage::OwnJarmenKellDetected),
        LogicEvaEvent::OwnColonelBurtonDetected => Some(EvaMessage::OwnColonelBurtonDetected),
        LogicEvaEvent::UnitLost => Some(EvaMessage::UnitLost),
        LogicEvaEvent::GeneralLevelUp => Some(EvaMessage::GeneralLevelUp),
        LogicEvaEvent::VehicleStolen => Some(EvaMessage::VehicleStolen),
        LogicEvaEvent::BuildingStolen => Some(EvaMessage::BuildingStolen),
        LogicEvaEvent::CashStolen => Some(EvaMessage::CashStolen),
        LogicEvaEvent::UpgradeComplete => Some(EvaMessage::UpgradeComplete),
        LogicEvaEvent::BuildingBeingStolen => Some(EvaMessage::BuildingBeingStolen),
        LogicEvaEvent::BuildingSabotaged => Some(EvaMessage::BuildingSabotaged),
        LogicEvaEvent::SuperweaponLaunchedOwnGpsScrambler => {
            Some(EvaMessage::SuperweaponLaunchedOwnGpsScrambler)
        }
        LogicEvaEvent::SuperweaponLaunchedAllyGpsScrambler => {
            Some(EvaMessage::SuperweaponLaunchedAllyGpsScrambler)
        }
        LogicEvaEvent::SuperweaponLaunchedEnemyGpsScrambler => {
            Some(EvaMessage::SuperweaponLaunchedEnemyGpsScrambler)
        }
        LogicEvaEvent::SuperweaponLaunchedOwnSneakAttack => {
            Some(EvaMessage::SuperweaponLaunchedOwnSneakAttack)
        }
        LogicEvaEvent::SuperweaponLaunchedAllySneakAttack => {
            Some(EvaMessage::SuperweaponLaunchedAllySneakAttack)
        }
        LogicEvaEvent::SuperweaponLaunchedEnemySneakAttack => {
            Some(EvaMessage::SuperweaponLaunchedEnemySneakAttack)
        }
    }
}

pub fn parse_eva_event(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let Some(name) = tokens.get(1) else {
        return Err(INIError::InvalidData);
    };
    if EvaMessage::from_name(name).is_none() {
        return Err(INIError::InvalidData);
    }

    // Always use the OnceLock singleton. Do not hold THE_EVA across INI load
    // (parse callbacks re-enter this function).
    let eva = get_eva();
    let mut eva = eva.lock().map_err(|_| INIError::InvalidData)?;
    let Some(check) = eva.new_eva_check_info(name) else {
        drop(eva);
        skip_eva_event_block(ini)?;
        return Ok(());
    };
    parse_eva_check_info_fields(ini, check)?;
    Ok(())
}

static THE_EVA: OnceLock<Mutex<Eva>> = OnceLock::new();

thread_local! {
    /// Live host logic frame (Main `GameLogic::get_frame` / presentation freeze).
    /// Zero means "unset" so crate `TheGameLogic::get_frame` remains the fallback.
    static HOST_EVA_FRAME: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// Host `hasSufficientPower` snapshot. `None` = leftover Energy fallback.
    static HOST_EVA_SUFFICIENT_POWER: std::cell::Cell<Option<bool>> =
        const { std::cell::Cell::new(None) };
}

/// Publish the live host/sim frame Eva.cpp:271 reads as `TheGameLogic->getFrame()`.
pub fn set_eva_host_frame(frame: u32) {
    HOST_EVA_FRAME.with(|cell| cell.set(frame));
}

/// Last host frame published by Main / presentation. Zero if none.
pub fn eva_host_frame() -> u32 {
    HOST_EVA_FRAME.with(|cell| cell.get())
}

/// Frame Eva::update uses: host clock when published, else crate GameLogic.
pub fn eva_logic_frame() -> u32 {
    let host = eva_host_frame();
    if host != 0 {
        host
    } else {
        TheGameLogic::get_frame()
    }
}

/// Publish host Energy::hasSufficientPower for Eva.cpp:408-422 LowPower poll.
pub fn set_eva_host_sufficient_power(sufficient: bool) {
    HOST_EVA_SUFFICIENT_POWER.with(|cell| cell.set(Some(sufficient)));
}

/// Drop the host power snapshot so leftover ThePlayerList Energy is used.
pub fn clear_eva_host_sufficient_power() {
    HOST_EVA_SUFFICIENT_POWER.with(|cell| cell.set(None));
}

/// Host sufficient-power snapshot. `None` when Main has not published.
pub fn eva_host_sufficient_power() -> Option<bool> {
    HOST_EVA_SUFFICIENT_POWER.with(|cell| cell.get())
}

pub fn get_eva() -> &'static Mutex<Eva> {
    THE_EVA.get_or_init(|| Mutex::new(Eva::new()))
}

pub fn initialize_eva_system() -> INIResult<()> {
    let _ = get_eva();
    if eva_check_info_count() > 0 {
        return Ok(());
    }
    // Register + load without holding THE_EVA; parse_eva_event locks per event.
    let _ = register_block_parser("EvaEvent", parse_eva_event);
    let mut ini = INI::new();
    ini.load("Data/INI/Eva.ini", INILoadType::Overwrite)
}

/// Number of Eva.ini `EvaEvent` blocks currently on the live singleton.
pub fn eva_check_info_count() -> usize {
    get_eva()
        .lock()
        .map(|eva| eva.all_check_infos.len())
        .unwrap_or(0)
}

pub fn reset_eva_system() {
    set_eva_host_frame(0);
    clear_eva_host_sufficient_power();
    let eva = get_eva();
    if let Ok(mut guard) = eva.lock() {
        guard.reset();
    }
}

pub fn update_eva_system() {
    let eva = get_eva();
    if let Ok(mut guard) = eva.lock() {
        guard.update();
    }
}

pub fn set_eva_should_play(message: EvaMessage) {
    let eva = get_eva();
    if let Ok(mut guard) = eva.lock() {
        guard.set_should_play(message);
    }
}

pub fn set_eva_enabled(enabled: bool) {
    let eva = get_eva();
    if let Ok(mut guard) = eva.lock() {
        guard.set_enabled(enabled);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn repo_root() -> Option<PathBuf> {
        let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        loop {
            if dir.join("GeneralsMD").is_dir() && dir.join("windows_game").is_dir() {
                return Some(dir);
            }
            if !dir.pop() {
                return None;
            }
        }
    }

    fn load_eva_file_for_test(path: &Path) -> INIResult<()> {
        // Shipped parse path writes THE_EVA (OnceLock), not a raw TLS pointer.
        reset_eva_system();
        let _ = get_eva();
        let _ = register_block_parser("EvaEvent", parse_eva_event);
        let mut ini = INI::new();
        ini.load(path, INILoadType::Overwrite)
    }

    #[test]
    fn eva_message_names_match_cpp_indices() {
        for (index, name) in EVA_MESSAGE_NAMES.iter().enumerate() {
            let message = EvaMessage::from_index(index).expect("valid EVA index");

            assert_eq!(EvaMessage::from_name(name), Some(message));
            assert_eq!(
                EvaMessage::from_name(&name.to_ascii_lowercase()),
                Some(message)
            );
            assert_eq!(message.to_name(), *name);
            assert_eq!(message.as_index(), index);
        }

        assert_eq!(EvaMessage::from_index(EVA_COUNT), None);
        assert_eq!(EvaMessage::from_name("EVA_INVALID"), None);
        assert_eq!(EvaMessage::from_name("UNKNOWN"), None);
    }

    #[test]
    fn new_eva_check_info_rejects_duplicates_like_cpp() {
        let mut eva = Eva::new();

        let first = eva
            .new_eva_check_info("LOWPOWER")
            .expect("first LOWPOWER check info");
        assert_eq!(first.message(), EvaMessage::LowPower);
        assert_eq!(first.priority(), 1);
        assert_eq!(first.frames_between_checks(), 900);
        assert_eq!(first.frames_to_expire(), 150);

        assert!(eva.new_eva_check_info("lowpower").is_none());
        assert!(eva.new_eva_check_info("EVA_INVALID").is_none());
        assert!(eva.new_eva_check_info("UNKNOWN").is_none());
    }

    #[test]
    fn eva_check_info_can_be_looked_up_by_name() {
        let mut eva = Eva::new();
        eva.new_eva_check_info("BUILDINGLOST")
            .expect("BUILDINGLOST check info");

        let info = eva
            .get_eva_check_info_by_name("buildinglost")
            .expect("case-insensitive lookup");
        assert_eq!(info.message(), EvaMessage::BuildingLost);
        assert_eq!(info.priority(), 1);

        assert!(eva.get_eva_check_info_by_name("EVA_INVALID").is_none());
        assert!(eva.get_eva_check_info_by_name("UNKNOWN").is_none());
    }

    #[test]
    fn retail_eva_ini_loads_all_events() {
        let Some(root) = repo_root() else {
            return;
        };
        let path = root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/Eva.ini");
        if !path.exists() {
            return;
        }

        load_eva_file_for_test(&path).expect("retail Eva.ini should parse");

        // Assert the shipped singleton the parser actually writes.
        let eva = get_eva();
        let eva = eva.lock().expect("THE_EVA lock");
        assert_eq!(eva.all_check_infos.len(), 49);
        assert!(
            eva.get_eva_check_info_by_name("SuperweaponLaunched_Enemy_GPS_Scrambler")
                .is_some()
        );
        assert!(
            eva.get_eva_check_info_by_name("SuperweaponLaunched_Enemy_Sneak_Attack")
                .is_some()
        );
    }

    #[test]
    fn eva_enabled_state_mirrors_logic_script_toggle() {
        let _ = LogicEva::set_enabled(true);
        let mut eva = Eva::new();
        eva.set_should_play(EvaMessage::BuildingLost);

        let _ = LogicEva::set_enabled(false);
        eva.sync_enabled_from_logic();
        assert!(!eva.enabled);
        assert!(!eva.should_play[EvaMessage::BuildingLost.as_index()]);

        let _ = LogicEva::set_enabled(true);
        eva.sync_enabled_from_logic();
        assert!(eva.enabled);
    }

    #[test]
    fn generic_eva_messages_do_not_probe_without_local_player() {
        let mut eva = Eva::new();
        eva.set_should_play(EvaMessage::BuildingLost);

        assert!(!eva.message_should_play_with_local_player(EvaMessage::BuildingLost, false));
        assert!(eva.should_play[EvaMessage::BuildingLost.as_index()]);
        assert_eq!(eva.message_being_tested, EvaMessage::LowPower);
    }

    #[test]
    fn eva_update_uses_host_frame_when_crate_clock_is_starved() {
        // C++ Eva.cpp:275 returns before probing when frame < 2. A starved crate
        // clock stays at 0; the live host frame must open that gate.
        set_eva_host_frame(0);
        let mut eva = Eva::new();
        eva.set_should_play(EvaMessage::BuildingLost);
        eva.update();
        assert!(
            eva.should_play[EvaMessage::BuildingLost.as_index()],
            "frame 0 must skip Eva.cpp:275 and leave shouldPlay latched"
        );

        set_eva_host_frame(5);
        assert_eq!(eva_logic_frame(), 5);
        eva.update();
        assert!(
            !eva.should_play[EvaMessage::BuildingLost.as_index()],
            "host frame >= 2 must run Eva::update and clear unprobed flags"
        );
        set_eva_host_frame(0);
    }

    #[test]
    fn low_power_polls_host_energy_not_leftover_flag() {
        // C++ Eva.cpp:408-422 polls Energy, never m_shouldPlay[LowPower].
        clear_eva_host_sufficient_power();
        let mut eva = Eva::new();
        eva.set_should_play(EvaMessage::LowPower);
        set_eva_host_sufficient_power(true);
        assert!(
            !eva.message_should_play_with_local_player(EvaMessage::LowPower, true),
            "host sufficient power must not speak LowPower"
        );
        set_eva_host_sufficient_power(false);
        assert!(
            eva.message_should_play_with_local_player(EvaMessage::LowPower, true),
            "host brownout must speak LowPower"
        );
        clear_eva_host_sufficient_power();
    }
}

/// Residual: last EVA action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualEvaAction {
    None = 0,
    Enable = 1,
    Disable = 2,
    ShouldPlay = 3,
    Reset = 4,
    Update = 5,
}

static RESIDUAL_EVA_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_EVA_ENABLED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);
static RESIDUAL_EVA_LAST_MESSAGE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(usize::MAX);

fn residual_eva_action_store(action: ResidualEvaAction) {
    RESIDUAL_EVA_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last EVA residual action.
pub fn residual_eva_last_action() -> ResidualEvaAction {
    match RESIDUAL_EVA_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualEvaAction::Enable,
        2 => ResidualEvaAction::Disable,
        3 => ResidualEvaAction::ShouldPlay,
        4 => ResidualEvaAction::Reset,
        5 => ResidualEvaAction::Update,
        _ => ResidualEvaAction::None,
    }
}

/// Residual: EVA enabled latch.
pub fn residual_eva_is_enabled() -> bool {
    RESIDUAL_EVA_ENABLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: last EvaMessage index flagged for play (None if none).
pub fn residual_eva_last_message_index() -> Option<usize> {
    let idx = RESIDUAL_EVA_LAST_MESSAGE.load(std::sync::atomic::Ordering::Relaxed);
    if idx == usize::MAX { None } else { Some(idx) }
}

/// Residual: enable EVA without INI reload.
pub fn simulate_eva_enable() -> bool {
    set_eva_enabled(true);
    RESIDUAL_EVA_ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
    residual_eva_action_store(ResidualEvaAction::Enable);
    residual_eva_is_enabled()
}

/// Residual: disable EVA without INI reload.
pub fn simulate_eva_disable() -> bool {
    set_eva_enabled(false);
    RESIDUAL_EVA_ENABLED.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_eva_action_store(ResidualEvaAction::Disable);
    !residual_eva_is_enabled()
}

/// Residual: flag a named EVA message should play without audio.
pub fn simulate_eva_set_should_play_by_name(name: &str) -> bool {
    let Some(message) = EvaMessage::from_name(name) else {
        return false;
    };
    set_eva_should_play(message);
    RESIDUAL_EVA_LAST_MESSAGE.store(message.as_index(), std::sync::atomic::Ordering::Relaxed);
    residual_eva_action_store(ResidualEvaAction::ShouldPlay);
    residual_eva_last_message_index() == Some(message.as_index())
}

/// Residual: flag LowPower residual (common combat alert).
pub fn simulate_eva_set_should_play_low_power() -> bool {
    simulate_eva_set_should_play_by_name("LOWPOWER")
}

/// Residual: reset EVA residual flags without INI reload.
pub fn simulate_eva_reset() -> bool {
    let eva = get_eva();
    if let Ok(mut guard) = eva.lock() {
        guard.reset();
    }
    RESIDUAL_EVA_ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_EVA_LAST_MESSAGE.store(usize::MAX, std::sync::atomic::Ordering::Relaxed);
    residual_eva_action_store(ResidualEvaAction::Reset);
    true
}

/// Residual: update EVA system residual (no INI required).
pub fn simulate_eva_update() -> bool {
    update_eva_system();
    residual_eva_action_store(ResidualEvaAction::Update);
    true
}

/// Residual: enable + LowPower should-play composite.
pub fn simulate_eva_prepare_low_power_alert() -> bool {
    if !simulate_eva_enable() {
        return false;
    }
    simulate_eva_set_should_play_low_power()
}
