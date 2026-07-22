//! Parachute Contain Module
//!
//! Specialized container for parachute drops and airborne deployment

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use super::{ContainerIniParse, ContainerInterface};
use crate::common::audio::AudioEventRts;
use crate::common::{GameResult, ObjectID, PlayerMaskType};
use crate::damage::DamageInfo;
use crate::helpers::TheGameLogic;
use crate::modules::{ContainModuleInterface, ContainWant, UpdateSleepTime};
use crate::object::contain::OpenContain;
use crate::object::{Object, ObjectId};
use game_engine::common::ini::{FieldParse, INIError, INI};

/// Configuration data for ParachuteContain module
#[derive(Debug, Clone)]
pub struct ParachuteContainModuleData {
    /// Configuration from parent OpenContain
    pub base: super::OpenContainModuleData,
    /// Max pitch rate
    pub pitch_rate_max: f32,
    /// Max roll rate
    pub roll_rate_max: f32,
    /// Low altitude damping
    pub low_altitude_damping: f32,
    /// Deploy the parachute when we have traveled this far
    pub para_open_dist: f32,
    /// Free fall damage percent
    pub free_fall_damage_percent: f32,
    /// Kill when landing in water slop threshold
    pub kill_when_landing_in_water_slop: f32,
    /// Parachute open sound
    pub parachute_open_sound: Option<AudioEventRts>,
}

impl Default for ParachuteContainModuleData {
    fn default() -> Self {
        Self {
            base: Default::default(),
            pitch_rate_max: 0.0,
            roll_rate_max: 0.0,
            low_altitude_damping: 0.2,
            para_open_dist: 0.0,
            free_fall_damage_percent: 0.5,
            kill_when_landing_in_water_slop: 10.0,
            parachute_open_sound: None,
        }
    }
}

impl ParachuteContainModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        self.base.parse_from_ini(ini)?;
        ini.init_from_ini_with_fields_allow_unknown(self, PARACHUTE_CONTAIN_FIELDS)
    }

    pub fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        self.base.parse_from_config(config)?;
        super::parse_with_fields_allow_unknown(config, self, PARACHUTE_CONTAIN_FIELDS)
    }
}

impl ContainerIniParse for ParachuteContainModuleData {
    fn parse_from_config(&mut self, config: &str) -> Result<(), INIError> {
        ParachuteContainModuleData::parse_from_config(self, config)
    }
}

fn parse_pitch_rate_max(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.pitch_rate_max = INI::parse_angular_velocity_real(token)?;
    Ok(())
}

fn parse_roll_rate_max(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.roll_rate_max = INI::parse_angular_velocity_real(token)?;
    Ok(())
}

fn parse_low_altitude_damping(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.low_altitude_damping = INI::parse_real(token)?;
    Ok(())
}

fn parse_parachute_open_dist(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.para_open_dist = INI::parse_real(token)?;
    Ok(())
}

fn parse_kill_when_landing_in_water_slop(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.kill_when_landing_in_water_slop = INI::parse_real(token)?;
    Ok(())
}

fn parse_free_fall_damage_percent(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.free_fall_damage_percent = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_parachute_open_sound(
    _ini: &mut INI,
    data: &mut ParachuteContainModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("NONE") {
        data.parachute_open_sound = None;
    } else {
        data.parachute_open_sound = Some(AudioEventRts::new(*token));
    }
    Ok(())
}

const PARACHUTE_CONTAIN_FIELDS: &[FieldParse<ParachuteContainModuleData>] = &[
    FieldParse {
        token: "PitchRateMax",
        parse: parse_pitch_rate_max,
    },
    FieldParse {
        token: "RollRateMax",
        parse: parse_roll_rate_max,
    },
    FieldParse {
        token: "LowAltitudeDamping",
        parse: parse_low_altitude_damping,
    },
    FieldParse {
        token: "ParachuteOpenDist",
        parse: parse_parachute_open_dist,
    },
    FieldParse {
        token: "KillWhenLandingInWaterSlop",
        parse: parse_kill_when_landing_in_water_slop,
    },
    FieldParse {
        token: "FreeFallDamagePercent",
        parse: parse_free_fall_damage_percent,
    },
    FieldParse {
        token: "ParachuteOpenSound",
        parse: parse_parachute_open_sound,
    },
];

/// Parachute contain module - for airborne deployment
#[derive(Debug)]
pub struct ParachuteContain {
    /// Base functionality from OpenContain
    pub base: OpenContain,
    /// Reference to the owning object
    #[allow(dead_code)]
    object_id: ObjectID,
}

impl ParachuteContain {
    /// Create a new ParachuteContain module
    pub fn new(
        object: Weak<RwLock<Object>>,
        module_data: &ParachuteContainModuleData,
    ) -> GameResult<Self> {
        let base = OpenContain::new(object.clone(), &module_data.base)?;

        Ok(Self {
            base,
            object_id: object
                .upgrade()
                .and_then(|arc| arc.read().ok().map(|g| g.get_id()))
                .unwrap_or(crate::common::INVALID_ID),
        })
    }

    /// Deploy parachutes for contained units
    pub fn deploy_parachutes(&mut self) -> GameResult<()> {
        // Implementation would handle parachute deployment
        Ok(())
    }

    /// Serialize state for save/load
    pub fn save_state(&self) -> GameResult<HashMap<String, Vec<u8>>> {
        self.base.save_state()
    }

    /// Deserialize state for save/load
    pub fn load_state(&mut self, state: &HashMap<String, Vec<u8>>) -> GameResult<()> {
        self.base.load_state(state)
    }
}

impl ContainModuleInterface for ParachuteContain {
    fn can_contain(&self, object_id: ObjectID) -> bool {
        if let Some(obj) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj.read() {
                return self.base.is_valid_container_for(&*obj_guard, true);
            }
        }
        false
    }

    fn contain_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = TheGameLogic::find_object_by_id(object_id)
            .ok_or_else(|| format!("Contain object {} not found", object_id))?;
        self.base
            .add_to_contain(
                obj.read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
            )
            .map_err(|e| e.to_string())
    }

    fn release_object(&mut self, object_id: ObjectID) -> Result<(), String> {
        let obj = match TheGameLogic::find_object_by_id(object_id) {
            Some(obj) => obj,
            None => return Ok(()),
        };
        self.base
            .remove_from_contain(
                obj.read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(crate::common::INVALID_ID),
                false,
            )
            .map_err(|e| e.to_string())
    }

    fn get_contained_objects(&self) -> &[ObjectID] {
        ContainModuleInterface::get_contained_objects(&self.base)
    }

    fn get_contained_count(&self) -> usize {
        ContainModuleInterface::get_contained_count(&self.base)
    }

    fn get_player_who_entered(&self) -> PlayerMaskType {
        self.base.get_player_who_entered()
    }

    fn get_max_capacity(&self) -> usize {
        let max = self.base.get_contain_max();
        if max < 0 {
            usize::MAX
        } else {
            max as usize
        }
    }

    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        self.base.update().map_err(|e| e.into())
    }

    fn on_damage(
        &mut self,
        info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_damage(info).map_err(|e| e.into())
    }

    fn on_die(
        &mut self,
        damage_info: Option<&DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_die(damage_info).map_err(|e| e.into())
    }

    fn is_valid_container_for(&self, obj: &Object, check_capacity: bool) -> bool {
        self.base.is_valid_container_for(obj, check_capacity)
    }

    fn add_to_contain(
        &mut self,
        obj: &Object,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.contain_object(obj.get_id()).map_err(|e| e.into())
    }

    fn enable_load_sounds(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.enable_load_sounds(enabled);
        Ok(())
    }

    fn on_object_wants_to_enter_or_exit(
        &mut self,
        obj: &Object,
        want: ContainWant,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.on_object_wants_to_enter_or_exit(obj, want);
        Ok(())
    }

    fn passes_weapon_bonus_to_passengers(&self) -> bool {
        self.base.passes_weapon_bonus_to_passengers()
    }

    fn set_passenger_allowed_to_fire(&mut self, allowed: bool) {
        self.base.set_passenger_allowed_to_fire(allowed);
    }

    fn harm_and_force_exit_all_contained(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base
            .harm_and_force_exit_all_contained(damage_info)
            .map_err(|e| e.into())
    }

    fn kill_all_contained(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.kill_all_contained().map_err(|e| e.into())
    }
}

impl ContainerInterface for ParachuteContain {
    fn can_contain(&self, obj: &Object) -> bool {
        self.base.is_valid_container_for(obj, true)
    }

    fn add_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.base.add_to_contain(obj_id)
    }

    fn remove_object(&mut self, obj_id: ObjectID) -> GameResult<()> {
        self.base.remove_from_contain(obj_id, false)
    }

    fn get_usage(&self) -> (u32, u32) {
        let current = self.base.get_contain_count();
        let max = match self.base.get_contain_max() {
            super::CONTAIN_MAX_UNKNOWN => u32::MAX,
            value if value < 0 => u32::MAX,
            value => value as u32,
        };
        (current, max)
    }
}
