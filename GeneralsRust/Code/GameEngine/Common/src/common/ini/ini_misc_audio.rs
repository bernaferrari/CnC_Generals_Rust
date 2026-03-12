//! INI parsing for MiscAudio definitions
//!
//! This module handles parsing MiscAudio entries from INI files.
//! MiscAudio contains miscellaneous sound hooks that don't have another home.
//!
//! Rust port: 2025

use crate::common::audio::game_audio::{get_global_audio_manager, initialize_global_audio_manager};
use crate::common::audio::AudioEventRts as EngineAudioEventRts;
use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Audio event for RTS games
///
/// Represents an audio event that can be played in response to game actions.
/// Contains the sound file reference and playback parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioEventRTS {
    /// Name/path of the audio file
    pub sound_file: String,
    /// Volume level (0.0 - 1.0)
    pub volume: f32,
    /// Minimum delay between plays (milliseconds)
    pub min_delay: u32,
    /// Maximum delay for randomization (milliseconds)
    pub max_delay: u32,
    /// 3D positional audio enabled
    pub is_3d: bool,
    /// Loop the audio
    pub is_looped: bool,
    /// Priority level for audio mixing
    pub priority: i32,
    /// Player index for player-specific audio
    pub player_index: i32,
}

impl Default for AudioEventRTS {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for AudioEventRTS {
    fn from(sound_file: String) -> Self {
        Self::from_sound_file(sound_file)
    }
}

impl AudioEventRTS {
    /// Create a new AudioEventRTS
    pub fn new() -> Self {
        Self {
            sound_file: String::new(),
            volume: 1.0,
            min_delay: 0,
            max_delay: 0,
            is_3d: false,
            is_looped: false,
            priority: 0,
            player_index: -1,
        }
    }

    /// Create from sound file name
    pub fn from_sound_file(sound_file: String) -> Self {
        Self {
            sound_file,
            ..Self::new()
        }
    }

    /// Set the player index for this audio event
    pub fn set_player_index(&mut self, index: i32) {
        self.player_index = index;
    }

    /// Get the player index
    pub fn get_player_index(&self) -> i32 {
        self.player_index
    }

    /// Set volume (0.0 - 1.0)
    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    /// Set 3D audio flag
    pub fn set_3d(&mut self, is_3d: bool) {
        self.is_3d = is_3d;
    }

    /// Set loop flag
    pub fn set_looped(&mut self, is_looped: bool) {
        self.is_looped = is_looped;
    }

    /// Set priority
    pub fn set_priority(&mut self, priority: i32) {
        self.priority = priority;
    }

    /// Check if this is a valid audio event
    pub fn is_valid(&self) -> bool {
        !self.sound_file.is_empty()
    }

    /// Play this audio event through the global audio manager.
    pub fn play(&self) {
        if self.is_valid() {
            let mut event = EngineAudioEventRts::with_event_name(&self.sound_file);
            event.set_volume(self.volume);
            if self.is_looped {
                event.set_loop_count(2);
            }

            let manager =
                get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
            let mut manager = manager.lock().expect("audio manager mutex poisoned");
            if let Some(info) = manager
                .find_audio_event_info(event.get_event_name())
                .or_else(|| manager.new_audio_event_info(event.get_event_name().to_string()))
            {
                event.set_audio_event_info(info.clone());
                event.set_volume(info.volume);
            }
            let _ = manager.add_audio_event(&event);
        }
    }

    /// Parse from INI string value
    pub fn parse_from_string(&mut self, value: &str) -> INIResult<()> {
        // Simple parsing - just set the sound file
        if !value.is_empty() {
            self.sound_file = value.to_string();
            Ok(())
        } else {
            Err(INIError::InvalidData)
        }
    }
}

/// Miscellaneous audio events structure
///
/// Contains all miscellaneous sound hooks that don't have another happy home.
/// These are global audio events triggered by various game actions.
#[derive(Debug, Clone)]
pub struct MiscAudio {
    /// Radar sounds to play when unit under attack
    pub radar_unit_under_attack_sound: AudioEventRTS,
    /// Radar sounds to play when harvester under attack
    pub radar_harvester_under_attack_sound: AudioEventRTS,
    /// Radar sounds to play when structure under attack
    pub radar_structure_under_attack_sound: AudioEventRTS,
    /// Radar sounds to play when ? under attack
    pub radar_under_attack_sound: AudioEventRTS,
    /// Radar sounds to play when something is infiltrated
    pub radar_infiltration_sound: AudioEventRTS,
    /// Radar sounds to play when radar goes online
    pub radar_online_sound: AudioEventRTS,
    /// Radar sounds to play when radar goes offline
    pub radar_offline_sound: AudioEventRTS,
    /// Sound to play during transient invulnerability while defecting
    pub defector_timer_tick_sound: AudioEventRTS,
    /// Sound to play when you become vulnerable again
    pub defector_timer_ding_sound: AudioEventRTS,
    /// Sound to play during stealth-fighter-lockon period
    pub lockon_tick_sound: AudioEventRTS,
    /// Sound to play when user presses 'cheer' key
    pub all_cheer_sound: AudioEventRTS,
    /// Sound to play when user presses 'battlecry' key
    pub battle_cry_sound: AudioEventRTS,
    /// Sound to play when user presses button in GUI
    pub gui_click_sound: AudioEventRTS,
    /// Global "No Can Do" sound
    pub no_can_do_sound: AudioEventRTS,
    /// I have just discovered an enemy stealth unit
    pub stealth_discovered_sound: AudioEventRTS,
    /// One of my stealthed units has just been discovered by the enemy
    pub stealth_neutralized_sound: AudioEventRTS,
    /// Money was deposited in my bank
    pub money_deposit_sound: AudioEventRTS,
    /// Money was withdrawn from my bank
    pub money_withdraw_sound: AudioEventRTS,
    /// Building has lost power, been hit with an EMP, or disable hacked
    pub building_disabled: AudioEventRTS,
    /// Building has recovered from being disabled
    pub building_reenabled: AudioEventRTS,
    /// Vehicle has been disabled via EMP or hacker attack
    pub vehicle_disabled: AudioEventRTS,
    /// Vehicle has recovered from being disabled
    pub vehicle_reenabled: AudioEventRTS,
    /// Pilot has been sniped by Jarmen Kell
    pub splatter_vehicle_pilots_brain: AudioEventRTS,
    /// Terrorist issues a move order while in a car
    pub terrorist_in_car_move_voice: AudioEventRTS,
    /// Terrorist issues attack order while in a car
    pub terrorist_in_car_attack_voice: AudioEventRTS,
    /// Terrorist is selected while in a car
    pub terrorist_in_car_select_voice: AudioEventRTS,
    /// When heal crate is picked up
    pub crate_heal: AudioEventRTS,
    /// When shroud crate is picked up
    pub crate_shroud: AudioEventRTS,
    /// When salvage crate is picked up
    pub crate_salvage: AudioEventRTS,
    /// When free unit crate is picked up
    pub crate_free_unit: AudioEventRTS,
    /// When money crate is picked up
    pub crate_money: AudioEventRTS,
    /// Unit is promoted
    pub unit_promoted: AudioEventRTS,
    /// Battle drone repairs unit
    pub repair_sparks: AudioEventRTS,
    /// When Saboteur hits a building
    pub sabotage_shut_down_building: AudioEventRTS,
    /// When Saboteur hits a building
    pub sabotage_reset_timer_building: AudioEventRTS,
    /// When a jet lands on a runway
    pub aircraft_wheel_screech: AudioEventRTS,
}

impl Default for MiscAudio {
    fn default() -> Self {
        Self::new()
    }
}

impl MiscAudio {
    /// Create a new MiscAudio with default values
    pub fn new() -> Self {
        Self {
            radar_unit_under_attack_sound: AudioEventRTS::new(),
            radar_harvester_under_attack_sound: AudioEventRTS::new(),
            radar_structure_under_attack_sound: AudioEventRTS::new(),
            radar_under_attack_sound: AudioEventRTS::new(),
            radar_infiltration_sound: AudioEventRTS::new(),
            radar_online_sound: AudioEventRTS::new(),
            radar_offline_sound: AudioEventRTS::new(),
            defector_timer_tick_sound: AudioEventRTS::new(),
            defector_timer_ding_sound: AudioEventRTS::new(),
            lockon_tick_sound: AudioEventRTS::new(),
            all_cheer_sound: AudioEventRTS::new(),
            battle_cry_sound: AudioEventRTS::new(),
            gui_click_sound: AudioEventRTS::new(),
            no_can_do_sound: AudioEventRTS::new(),
            stealth_discovered_sound: AudioEventRTS::new(),
            stealth_neutralized_sound: AudioEventRTS::new(),
            money_deposit_sound: AudioEventRTS::new(),
            money_withdraw_sound: AudioEventRTS::new(),
            building_disabled: AudioEventRTS::new(),
            building_reenabled: AudioEventRTS::new(),
            vehicle_disabled: AudioEventRTS::new(),
            vehicle_reenabled: AudioEventRTS::new(),
            splatter_vehicle_pilots_brain: AudioEventRTS::new(),
            terrorist_in_car_move_voice: AudioEventRTS::new(),
            terrorist_in_car_attack_voice: AudioEventRTS::new(),
            terrorist_in_car_select_voice: AudioEventRTS::new(),
            crate_heal: AudioEventRTS::new(),
            crate_shroud: AudioEventRTS::new(),
            crate_salvage: AudioEventRTS::new(),
            crate_free_unit: AudioEventRTS::new(),
            crate_money: AudioEventRTS::new(),
            unit_promoted: AudioEventRTS::new(),
            repair_sparks: AudioEventRTS::new(),
            sabotage_shut_down_building: AudioEventRTS::new(),
            sabotage_reset_timer_building: AudioEventRTS::new(),
            aircraft_wheel_screech: AudioEventRTS::new(),
        }
    }

    /// Get audio event by name (for dynamic access)
    pub fn get_audio_event(&self, token: &str) -> Option<&AudioEventRTS> {
        match token {
            "RadarNotifyUnitUnderAttackSound" => Some(&self.radar_unit_under_attack_sound),
            "RadarNotifyHarvesterUnderAttackSound" => {
                Some(&self.radar_harvester_under_attack_sound)
            }
            "RadarNotifyStructureUnderAttackSound" => {
                Some(&self.radar_structure_under_attack_sound)
            }
            "RadarNotifyUnderAttackSound" => Some(&self.radar_under_attack_sound),
            "RadarNotifyInfiltrationSound" => Some(&self.radar_infiltration_sound),
            "RadarNotifyOnlineSound" => Some(&self.radar_online_sound),
            "RadarNotifyOfflineSound" => Some(&self.radar_offline_sound),
            "DefectorTimerTickSound" => Some(&self.defector_timer_tick_sound),
            "DefectorTimerDingSound" => Some(&self.defector_timer_ding_sound),
            "LockonTickSound" => Some(&self.lockon_tick_sound),
            "AllCheerSound" => Some(&self.all_cheer_sound),
            "BattleCrySound" => Some(&self.battle_cry_sound),
            "GUIClickSound" => Some(&self.gui_click_sound),
            "NoCanDoSound" => Some(&self.no_can_do_sound),
            "StealthDiscoveredSound" => Some(&self.stealth_discovered_sound),
            "StealthNeutralizedSound" => Some(&self.stealth_neutralized_sound),
            "MoneyDepositSound" => Some(&self.money_deposit_sound),
            "MoneyWithdrawSound" => Some(&self.money_withdraw_sound),
            "BuildingDisabled" => Some(&self.building_disabled),
            "BuildingReenabled" => Some(&self.building_reenabled),
            "VehicleDisabled" => Some(&self.vehicle_disabled),
            "VehicleReenabled" => Some(&self.vehicle_reenabled),
            "SplatterVehiclePilotsBrain" => Some(&self.splatter_vehicle_pilots_brain),
            "TerroristInCarMoveVoice" => Some(&self.terrorist_in_car_move_voice),
            "TerroristInCarAttackVoice" => Some(&self.terrorist_in_car_attack_voice),
            "TerroristInCarSelectVoice" => Some(&self.terrorist_in_car_select_voice),
            "CrateHeal" => Some(&self.crate_heal),
            "CrateShroud" => Some(&self.crate_shroud),
            "CrateSalvage" => Some(&self.crate_salvage),
            "CrateFreeUnit" => Some(&self.crate_free_unit),
            "CrateMoney" => Some(&self.crate_money),
            "UnitPromoted" => Some(&self.unit_promoted),
            "RepairSparks" => Some(&self.repair_sparks),
            "SabotageShutDownBuilding" => Some(&self.sabotage_shut_down_building),
            "SabotageResetTimeBuilding" => Some(&self.sabotage_reset_timer_building),
            "AircraftWheelScreech" => Some(&self.aircraft_wheel_screech),
            _ => None,
        }
    }

    /// Get mutable audio event by name (for dynamic modification)
    pub fn get_audio_event_mut(&mut self, token: &str) -> Option<&mut AudioEventRTS> {
        match token {
            "RadarNotifyUnitUnderAttackSound" => Some(&mut self.radar_unit_under_attack_sound),
            "RadarNotifyHarvesterUnderAttackSound" => {
                Some(&mut self.radar_harvester_under_attack_sound)
            }
            "RadarNotifyStructureUnderAttackSound" => {
                Some(&mut self.radar_structure_under_attack_sound)
            }
            "RadarNotifyUnderAttackSound" => Some(&mut self.radar_under_attack_sound),
            "RadarNotifyInfiltrationSound" => Some(&mut self.radar_infiltration_sound),
            "RadarNotifyOnlineSound" => Some(&mut self.radar_online_sound),
            "RadarNotifyOfflineSound" => Some(&mut self.radar_offline_sound),
            "DefectorTimerTickSound" => Some(&mut self.defector_timer_tick_sound),
            "DefectorTimerDingSound" => Some(&mut self.defector_timer_ding_sound),
            "LockonTickSound" => Some(&mut self.lockon_tick_sound),
            "AllCheerSound" => Some(&mut self.all_cheer_sound),
            "BattleCrySound" => Some(&mut self.battle_cry_sound),
            "GUIClickSound" => Some(&mut self.gui_click_sound),
            "NoCanDoSound" => Some(&mut self.no_can_do_sound),
            "StealthDiscoveredSound" => Some(&mut self.stealth_discovered_sound),
            "StealthNeutralizedSound" => Some(&mut self.stealth_neutralized_sound),
            "MoneyDepositSound" => Some(&mut self.money_deposit_sound),
            "MoneyWithdrawSound" => Some(&mut self.money_withdraw_sound),
            "BuildingDisabled" => Some(&mut self.building_disabled),
            "BuildingReenabled" => Some(&mut self.building_reenabled),
            "VehicleDisabled" => Some(&mut self.vehicle_disabled),
            "VehicleReenabled" => Some(&mut self.vehicle_reenabled),
            "SplatterVehiclePilotsBrain" => Some(&mut self.splatter_vehicle_pilots_brain),
            "TerroristInCarMoveVoice" => Some(&mut self.terrorist_in_car_move_voice),
            "TerroristInCarAttackVoice" => Some(&mut self.terrorist_in_car_attack_voice),
            "TerroristInCarSelectVoice" => Some(&mut self.terrorist_in_car_select_voice),
            "CrateHeal" => Some(&mut self.crate_heal),
            "CrateShroud" => Some(&mut self.crate_shroud),
            "CrateSalvage" => Some(&mut self.crate_salvage),
            "CrateFreeUnit" => Some(&mut self.crate_free_unit),
            "CrateMoney" => Some(&mut self.crate_money),
            "UnitPromoted" => Some(&mut self.unit_promoted),
            "RepairSparks" => Some(&mut self.repair_sparks),
            "SabotageShutDownBuilding" => Some(&mut self.sabotage_shut_down_building),
            "SabotageResetTimeBuilding" => Some(&mut self.sabotage_reset_timer_building),
            "AircraftWheelScreech" => Some(&mut self.aircraft_wheel_screech),
            _ => None,
        }
    }

    /// Play an audio event by name
    pub fn play_audio_event(&self, token: &str) {
        if let Some(audio_event) = self.get_audio_event(token) {
            audio_event.play();
        } else {
            println!("Audio event '{}' not found", token);
        }
    }

    /// Get list of all audio event names
    pub fn get_audio_event_names(&self) -> Vec<&'static str> {
        vec![
            "RadarNotifyUnitUnderAttackSound",
            "RadarNotifyHarvesterUnderAttackSound",
            "RadarNotifyStructureUnderAttackSound",
            "RadarNotifyUnderAttackSound",
            "RadarNotifyInfiltrationSound",
            "RadarNotifyOnlineSound",
            "RadarNotifyOfflineSound",
            "DefectorTimerTickSound",
            "DefectorTimerDingSound",
            "LockonTickSound",
            "AllCheerSound",
            "BattleCrySound",
            "GUIClickSound",
            "NoCanDoSound",
            "StealthDiscoveredSound",
            "StealthNeutralizedSound",
            "MoneyDepositSound",
            "MoneyWithdrawSound",
            "BuildingDisabled",
            "BuildingReenabled",
            "VehicleDisabled",
            "VehicleReenabled",
            "SplatterVehiclePilotsBrain",
            "TerroristInCarMoveVoice",
            "TerroristInCarAttackVoice",
            "TerroristInCarSelectVoice",
            "CrateHeal",
            "CrateShroud",
            "CrateSalvage",
            "CrateFreeUnit",
            "CrateMoney",
            "UnitPromoted",
            "RepairSparks",
            "SabotageShutDownBuilding",
            "SabotageResetTimeBuilding",
            "AircraftWheelScreech",
        ]
    }
}

/// Field parser definition (using FieldParse from ini.rs)
pub type FieldParser = FieldParse<MiscAudio>;

/// Field parse table for MiscAudio (matches C++ table)
pub const FIELD_PARSE_TABLE: &[FieldParser] = &[
    FieldParser {
        token: "RadarNotifyUnitUnderAttackSound",
        parse: parse_radar_unit_under_attack_sound,
    },
    FieldParser {
        token: "RadarNotifyHarvesterUnderAttackSound",
        parse: parse_radar_harvester_under_attack_sound,
    },
    FieldParser {
        token: "RadarNotifyStructureUnderAttackSound",
        parse: parse_radar_structure_under_attack_sound,
    },
    FieldParser {
        token: "RadarNotifyUnderAttackSound",
        parse: parse_radar_under_attack_sound,
    },
    FieldParser {
        token: "RadarNotifyInfiltrationSound",
        parse: parse_radar_infiltration_sound,
    },
    FieldParser {
        token: "RadarNotifyOnlineSound",
        parse: parse_radar_online_sound,
    },
    FieldParser {
        token: "RadarNotifyOfflineSound",
        parse: parse_radar_offline_sound,
    },
    FieldParser {
        token: "DefectorTimerTickSound",
        parse: parse_defector_timer_tick_sound,
    },
    FieldParser {
        token: "DefectorTimerDingSound",
        parse: parse_defector_timer_ding_sound,
    },
    FieldParser {
        token: "LockonTickSound",
        parse: parse_lockon_tick_sound,
    },
    FieldParser {
        token: "AllCheerSound",
        parse: parse_all_cheer_sound,
    },
    FieldParser {
        token: "BattleCrySound",
        parse: parse_battle_cry_sound,
    },
    FieldParser {
        token: "GUIClickSound",
        parse: parse_gui_click_sound,
    },
    FieldParser {
        token: "NoCanDoSound",
        parse: parse_no_can_do_sound,
    },
    FieldParser {
        token: "StealthDiscoveredSound",
        parse: parse_stealth_discovered_sound,
    },
    FieldParser {
        token: "StealthNeutralizedSound",
        parse: parse_stealth_neutralized_sound,
    },
    FieldParser {
        token: "MoneyDepositSound",
        parse: parse_money_deposit_sound,
    },
    FieldParser {
        token: "MoneyWithdrawSound",
        parse: parse_money_withdraw_sound,
    },
    FieldParser {
        token: "BuildingDisabled",
        parse: parse_building_disabled,
    },
    FieldParser {
        token: "BuildingReenabled",
        parse: parse_building_reenabled,
    },
    FieldParser {
        token: "VehicleDisabled",
        parse: parse_vehicle_disabled,
    },
    FieldParser {
        token: "VehicleReenabled",
        parse: parse_vehicle_reenabled,
    },
    FieldParser {
        token: "SplatterVehiclePilotsBrain",
        parse: parse_splatter_vehicle_pilots_brain,
    },
    FieldParser {
        token: "TerroristInCarMoveVoice",
        parse: parse_terrorist_in_car_move_voice,
    },
    FieldParser {
        token: "TerroristInCarAttackVoice",
        parse: parse_terrorist_in_car_attack_voice,
    },
    FieldParser {
        token: "TerroristInCarSelectVoice",
        parse: parse_terrorist_in_car_select_voice,
    },
    FieldParser {
        token: "CrateHeal",
        parse: parse_crate_heal,
    },
    FieldParser {
        token: "CrateShroud",
        parse: parse_crate_shroud,
    },
    FieldParser {
        token: "CrateSalvage",
        parse: parse_crate_salvage,
    },
    FieldParser {
        token: "CrateFreeUnit",
        parse: parse_crate_free_unit,
    },
    FieldParser {
        token: "CrateMoney",
        parse: parse_crate_money,
    },
    FieldParser {
        token: "UnitPromoted",
        parse: parse_unit_promoted,
    },
    FieldParser {
        token: "RepairSparks",
        parse: parse_repair_sparks,
    },
    FieldParser {
        token: "SabotageShutDownBuilding",
        parse: parse_sabotage_shut_down_building,
    },
    FieldParser {
        token: "SabotageResetTimeBuilding",
        parse: parse_sabotage_reset_timer_building,
    },
    FieldParser {
        token: "AircraftWheelScreech",
        parse: parse_aircraft_wheel_screech,
    },
];

fn parse_audio_event_from_args(args: &[&str]) -> INIResult<AudioEventRTS> {
    let token = args.first().ok_or(INIError::InvalidData)?;
    let event_name = INI::parse_ascii_string(token)?;
    if event_name.eq_ignore_ascii_case("NoSound") {
        return Ok(AudioEventRTS::new());
    }

    Ok(AudioEventRTS::from_sound_file(event_name))
}

macro_rules! audio_event_parser {
    ($fn_name:ident, $field:ident) => {
        pub fn $fn_name(
            _ini: &mut INI,
            misc_audio: &mut MiscAudio,
            args: &[&str],
        ) -> INIResult<()> {
            let audio_event = parse_audio_event_from_args(args)?;
            misc_audio.$field = audio_event;
            Ok(())
        }
    };
}

audio_event_parser!(
    parse_radar_unit_under_attack_sound,
    radar_unit_under_attack_sound
);
audio_event_parser!(
    parse_radar_harvester_under_attack_sound,
    radar_harvester_under_attack_sound
);
audio_event_parser!(
    parse_radar_structure_under_attack_sound,
    radar_structure_under_attack_sound
);
audio_event_parser!(parse_radar_under_attack_sound, radar_under_attack_sound);
audio_event_parser!(parse_radar_infiltration_sound, radar_infiltration_sound);
audio_event_parser!(parse_radar_online_sound, radar_online_sound);
audio_event_parser!(parse_radar_offline_sound, radar_offline_sound);
audio_event_parser!(parse_defector_timer_tick_sound, defector_timer_tick_sound);
audio_event_parser!(parse_defector_timer_ding_sound, defector_timer_ding_sound);
audio_event_parser!(parse_lockon_tick_sound, lockon_tick_sound);
audio_event_parser!(parse_all_cheer_sound, all_cheer_sound);
audio_event_parser!(parse_battle_cry_sound, battle_cry_sound);
audio_event_parser!(parse_gui_click_sound, gui_click_sound);
audio_event_parser!(parse_no_can_do_sound, no_can_do_sound);
audio_event_parser!(parse_stealth_discovered_sound, stealth_discovered_sound);
audio_event_parser!(parse_stealth_neutralized_sound, stealth_neutralized_sound);
audio_event_parser!(parse_money_deposit_sound, money_deposit_sound);
audio_event_parser!(parse_money_withdraw_sound, money_withdraw_sound);
audio_event_parser!(parse_building_disabled, building_disabled);
audio_event_parser!(parse_building_reenabled, building_reenabled);
audio_event_parser!(parse_vehicle_disabled, vehicle_disabled);
audio_event_parser!(parse_vehicle_reenabled, vehicle_reenabled);
audio_event_parser!(
    parse_splatter_vehicle_pilots_brain,
    splatter_vehicle_pilots_brain
);
audio_event_parser!(
    parse_terrorist_in_car_move_voice,
    terrorist_in_car_move_voice
);
audio_event_parser!(
    parse_terrorist_in_car_attack_voice,
    terrorist_in_car_attack_voice
);
audio_event_parser!(
    parse_terrorist_in_car_select_voice,
    terrorist_in_car_select_voice
);
audio_event_parser!(parse_crate_heal, crate_heal);
audio_event_parser!(parse_crate_shroud, crate_shroud);
audio_event_parser!(parse_crate_salvage, crate_salvage);
audio_event_parser!(parse_crate_free_unit, crate_free_unit);
audio_event_parser!(parse_crate_money, crate_money);
audio_event_parser!(parse_unit_promoted, unit_promoted);
audio_event_parser!(parse_repair_sparks, repair_sparks);
audio_event_parser!(
    parse_sabotage_shut_down_building,
    sabotage_shut_down_building
);
audio_event_parser!(
    parse_sabotage_reset_timer_building,
    sabotage_reset_timer_building
);
audio_event_parser!(parse_aircraft_wheel_screech, aircraft_wheel_screech);

/// Global MiscAudio instance (thread-safe)
static MISC_AUDIO: OnceCell<Arc<RwLock<MiscAudio>>> = OnceCell::new();

/// Ensure the misc audio collection exists and return a handle to it
pub fn ensure_misc_audio() -> Arc<RwLock<MiscAudio>> {
    MISC_AUDIO
        .get_or_init(|| Arc::new(RwLock::new(MiscAudio::new())))
        .clone()
}

/// Initialize (or reinitialize) the global misc audio
pub fn init_global_misc_audio() {
    let misc_audio = ensure_misc_audio();
    *misc_audio.write() = MiscAudio::new();
}

/// Get a handle to the global misc audio if initialized
pub fn get_misc_audio() -> Option<Arc<RwLock<MiscAudio>>> {
    MISC_AUDIO.get().cloned()
}

/// INI parsing function for MiscAudio definition (matches C++ interface)
///
/// This is the main entry point for parsing MiscAudio definitions from INI files
pub fn parse_misc_audio(ini: &mut INI) -> INIResult<()> {
    // Get or create global misc audio
    let misc_audio_handle = ensure_misc_audio();
    {
        let mut misc_audio = misc_audio_handle.write();

        // Parse using field table
        ini.init_from_ini_with_fields(&mut *misc_audio, FIELD_PARSE_TABLE)?;

        println!("Parsed MiscAudio definition");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_event_rts_creation() {
        let mut audio_event = AudioEventRTS::new();
        assert!(audio_event.sound_file.is_empty());
        assert_eq!(audio_event.volume, 1.0);
        assert!(!audio_event.is_3d);
        assert!(!audio_event.is_looped);
        assert_eq!(audio_event.player_index, -1);
        assert!(!audio_event.is_valid());

        audio_event.sound_file = "test_sound.wav".to_string();
        assert!(audio_event.is_valid());
    }

    #[test]
    fn test_audio_event_rts_properties() {
        let mut audio_event = AudioEventRTS::from_sound_file("battle.wav".to_string());
        assert_eq!(audio_event.sound_file, "battle.wav");
        assert!(audio_event.is_valid());

        audio_event.set_volume(0.5);
        assert_eq!(audio_event.volume, 0.5);

        audio_event.set_volume(2.0); // Should be clamped
        assert_eq!(audio_event.volume, 1.0);

        audio_event.set_3d(true);
        assert!(audio_event.is_3d);

        audio_event.set_looped(true);
        assert!(audio_event.is_looped);

        audio_event.set_priority(10);
        assert_eq!(audio_event.priority, 10);

        audio_event.set_player_index(5);
        assert_eq!(audio_event.get_player_index(), 5);
    }

    #[test]
    fn test_misc_audio_creation() {
        let misc_audio = MiscAudio::new();

        // Test that all audio events are initialized
        assert!(misc_audio
            .radar_unit_under_attack_sound
            .sound_file
            .is_empty());
        assert!(misc_audio.gui_click_sound.sound_file.is_empty());
        assert!(misc_audio.money_deposit_sound.sound_file.is_empty());
    }

    #[test]
    fn test_misc_audio_event_access() {
        let mut misc_audio = MiscAudio::new();

        // Test getting audio event by name
        assert!(misc_audio
            .get_audio_event("RadarNotifyUnitUnderAttackSound")
            .is_some());
        assert!(misc_audio.get_audio_event("NonexistentSound").is_none());

        // Test mutable access
        if let Some(audio_event) = misc_audio.get_audio_event_mut("GUIClickSound") {
            audio_event.sound_file = "click.wav".to_string();
        }

        // Verify the change
        assert_eq!(misc_audio.gui_click_sound.sound_file, "click.wav");
    }

    #[test]
    fn test_misc_audio_event_names() {
        let misc_audio = MiscAudio::new();
        let names = misc_audio.get_audio_event_names();

        assert!(!names.is_empty());
        assert!(names.contains(&"RadarNotifyUnitUnderAttackSound"));
        assert!(names.contains(&"GUIClickSound"));
        assert!(names.contains(&"MoneyDepositSound"));
        assert!(names.contains(&"AircraftWheelScreech"));

        // Verify we have all expected events
        assert_eq!(names.len(), 36); // Should match number of audio events
    }

    #[test]
    fn test_audio_event_parsing() {
        let mut audio_event = AudioEventRTS::new();

        let result = audio_event.parse_from_string("explosion.wav");
        assert!(result.is_ok());
        assert_eq!(audio_event.sound_file, "explosion.wav");
        assert!(audio_event.is_valid());

        let result = audio_event.parse_from_string("");
        assert!(result.is_err());
    }

    #[test]
    fn test_field_parse_table() {
        assert!(!FIELD_PARSE_TABLE.is_empty());
        assert_eq!(FIELD_PARSE_TABLE.len(), 36); // Should match number of audio events

        // Check that expected fields are present
        let field_names: Vec<&str> = FIELD_PARSE_TABLE.iter().map(|f| f.token).collect();
        assert!(field_names.contains(&"RadarNotifyUnitUnderAttackSound"));
        assert!(field_names.contains(&"GUIClickSound"));
        assert!(field_names.contains(&"MoneyDepositSound"));
        assert!(field_names.contains(&"AircraftWheelScreech"));
    }

    #[test]
    fn test_global_misc_audio() {
        init_global_misc_audio();
        let handle = ensure_misc_audio();

        {
            let mut misc_audio = handle.write();
            misc_audio.gui_click_sound.sound_file = "global_click.wav".to_string();
        }

        let misc_audio = handle.read();
        assert_eq!(misc_audio.gui_click_sound.sound_file, "global_click.wav");
    }

    #[test]
    fn test_play_audio_event() {
        let mut misc_audio = MiscAudio::new();
        misc_audio.gui_click_sound.sound_file = "click.wav".to_string();

        // This should not panic and should print to console
        misc_audio.play_audio_event("GUIClickSound");
        misc_audio.play_audio_event("NonexistentSound");
    }
}
