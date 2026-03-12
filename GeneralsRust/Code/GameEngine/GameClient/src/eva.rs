//! EVA voice system (GameClient/Eva.cpp).

use std::sync::{Mutex, OnceLock};

use game_engine::common::ini::{
    register_block_parser, FieldParse, INIError, INILoadType, INIResult, INI,
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
            .and_then(|index| Self::from_index(index))
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
    target.frames_to_expire = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_side_sounds_list(
    ini: &mut INI,
    target: &mut EvaCheckInfo,
    _tokens: &[&str],
) -> INIResult<()> {
    let mut side_sounds = EvaSideSounds::default();
    ini.init_from_ini_with_fields(&mut side_sounds, EvaSideSounds::field_parse())?;
    target.eva_side_sounds.push(side_sounds);
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
        if !self.enabled {
            return;
        }

        let current_frame = TheGameLogic::get_frame();
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

    fn is_time_for_check(&self, message: EvaMessage) -> bool {
        !self.checks.iter().any(|check| check.eva_info == message)
    }

    fn message_should_play(&mut self, message: EvaMessage) -> bool {
        self.message_being_tested = message;
        match message {
            EvaMessage::LowPower => self.should_play_low_power(),
            _ => self.should_play_generic(),
        }
    }

    fn should_play_low_power(&self) -> bool {
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

        if self.eva_speech.is_currently_playing() {
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
        LogicEvaEvent::BuildingSabotaged => Some(EvaMessage::BuildingSabotaged),
        LogicEvaEvent::BuildingLost => Some(EvaMessage::BuildingLost),
        LogicEvaEvent::CashStolen => Some(EvaMessage::CashStolen),
        LogicEvaEvent::UnitLost => Some(EvaMessage::UnitLost),
        LogicEvaEvent::VehicleStolen => Some(EvaMessage::VehicleStolen),
        LogicEvaEvent::SuperweaponDetectedOwnParticleCannon => {
            Some(EvaMessage::SuperweaponDetectedOwnParticleCannon)
        }
        LogicEvaEvent::SuperweaponDetectedAllyParticleCannon => {
            Some(EvaMessage::SuperweaponDetectedAllyParticleCannon)
        }
        LogicEvaEvent::SuperweaponDetectedEnemyParticleCannon => {
            Some(EvaMessage::SuperweaponDetectedEnemyParticleCannon)
        }
        LogicEvaEvent::SuperweaponDetectedOwnNuke => Some(EvaMessage::SuperweaponDetectedOwnNuke),
        LogicEvaEvent::SuperweaponDetectedAllyNuke => Some(EvaMessage::SuperweaponDetectedAllyNuke),
        LogicEvaEvent::SuperweaponDetectedEnemyNuke => {
            Some(EvaMessage::SuperweaponDetectedEnemyNuke)
        }
        LogicEvaEvent::SuperweaponDetectedOwnScudStorm => {
            Some(EvaMessage::SuperweaponDetectedOwnScudStorm)
        }
        LogicEvaEvent::SuperweaponDetectedAllyScudStorm => {
            Some(EvaMessage::SuperweaponDetectedAllyScudStorm)
        }
        LogicEvaEvent::SuperweaponDetectedEnemyScudStorm => {
            Some(EvaMessage::SuperweaponDetectedEnemyScudStorm)
        }
        LogicEvaEvent::SuperweaponLaunchedOwnParticleCannon => {
            Some(EvaMessage::SuperweaponLaunchedOwnParticleCannon)
        }
        LogicEvaEvent::SuperweaponLaunchedAllyParticleCannon => {
            Some(EvaMessage::SuperweaponLaunchedAllyParticleCannon)
        }
        LogicEvaEvent::SuperweaponLaunchedEnemyParticleCannon => {
            Some(EvaMessage::SuperweaponLaunchedEnemyParticleCannon)
        }
        LogicEvaEvent::SuperweaponLaunchedOwnNuke => Some(EvaMessage::SuperweaponLaunchedOwnNuke),
        LogicEvaEvent::SuperweaponLaunchedAllyNuke => Some(EvaMessage::SuperweaponLaunchedAllyNuke),
        LogicEvaEvent::SuperweaponLaunchedEnemyNuke => {
            Some(EvaMessage::SuperweaponLaunchedEnemyNuke)
        }
        LogicEvaEvent::SuperweaponLaunchedOwnScudStorm => {
            Some(EvaMessage::SuperweaponLaunchedOwnScudStorm)
        }
        LogicEvaEvent::SuperweaponLaunchedAllyScudStorm => {
            Some(EvaMessage::SuperweaponLaunchedAllyScudStorm)
        }
        LogicEvaEvent::SuperweaponLaunchedEnemyScudStorm => {
            Some(EvaMessage::SuperweaponLaunchedEnemyScudStorm)
        }
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
        LogicEvaEvent::GeneralLevelUp => Some(EvaMessage::GeneralLevelUp),
        LogicEvaEvent::BeaconDetected => Some(EvaMessage::BeaconDetected),
    }
}

pub fn parse_eva_event(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let Some(name) = tokens.get(1) else {
        return Err(INIError::InvalidData);
    };

    let Some(eva) = THE_EVA.get() else {
        return Err(INIError::InvalidData);
    };
    let mut eva = eva.lock().map_err(|_| INIError::InvalidData)?;

    let Some(check) = eva.new_eva_check_info(name) else {
        return Ok(());
    };

    ini.init_from_ini_with_fields(check, EvaCheckInfo::field_parse())?;
    Ok(())
}

static THE_EVA: OnceLock<Mutex<Eva>> = OnceLock::new();

pub fn get_eva() -> &'static Mutex<Eva> {
    THE_EVA.get_or_init(|| Mutex::new(Eva::new()))
}

pub fn initialize_eva_system() -> INIResult<()> {
    let eva = get_eva();
    eva.lock().map_err(|_| INIError::InvalidData)?.init()
}

pub fn reset_eva_system() {
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
