//! EjectPilotDie - Ejects pilot from vehicle on death
//!
//! Original C++ location: GameLogic/Module/EjectPilotDie.h/.cpp
//! Original C++ Author: Steven Johnson, April 2002
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::common::UnsignedInt;
use crate::damage::DamageInfo;
use crate::helpers::{TheGameLogic, TheObjectCreationListStore};
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Module data for EjectPilotDie
/// (Matches C++ EjectPilotDieModuleData)
#[derive(Debug, Clone)]
pub struct EjectPilotDieModuleData {
    pub base: DieModuleData,
    /// Object creation list for pilot in air (with parachute)
    pub ocl_in_air: Vec<String>,
    /// Object creation list for pilot on ground (no parachute)
    pub ocl_on_ground: Vec<String>,
    /// Time in milliseconds that the pilot is invulnerable after ejection
    pub invulnerable_time: UnsignedInt,
}

impl Default for EjectPilotDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            ocl_in_air: Vec::new(),
            ocl_on_ground: Vec::new(),
            invulnerable_time: 0,
        }
    }
}

impl Snapshotable for EjectPilotDieModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

crate::impl_legacy_module_data_via_base!(EjectPilotDieModuleData, base);

impl EjectPilotDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, EJECT_PILOT_DIE_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut EjectPilotDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut EjectPilotDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut EjectPilotDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut EjectPilotDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_air_creation_list(
    _ini: &mut INI,
    data: &mut EjectPilotDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.ocl_in_air = vec![(*token).to_string()];
    Ok(())
}

fn parse_ground_creation_list(
    _ini: &mut INI,
    data: &mut EjectPilotDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.ocl_on_ground = vec![(*token).to_string()];
    Ok(())
}

fn parse_invulnerable_time(
    _ini: &mut INI,
    data: &mut EjectPilotDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.invulnerable_time = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

const EJECT_PILOT_DIE_FIELDS: &[FieldParse<EjectPilotDieModuleData>] = &[
    FieldParse {
        token: "DeathTypes",
        parse: parse_die_death_types,
    },
    FieldParse {
        token: "VeterancyLevels",
        parse: parse_die_veterancy_levels,
    },
    FieldParse {
        token: "ExemptStatus",
        parse: parse_die_exempt_status,
    },
    FieldParse {
        token: "RequiredStatus",
        parse: parse_die_required_status,
    },
    FieldParse {
        token: "AirCreationList",
        parse: parse_air_creation_list,
    },
    FieldParse {
        token: "GroundCreationList",
        parse: parse_ground_creation_list,
    },
    FieldParse {
        token: "InvulnerableTime",
        parse: parse_invulnerable_time,
    },
];

/// EjectPilotDie - Ejects pilot from dying vehicle
///
/// When a vehicle with this module is destroyed, it spawns a pilot unit.
/// The pilot can be spawned either:
/// - In air (with parachute) if the vehicle is significantly above terrain
/// - On ground (standing) if the vehicle is on the ground
///
/// The ejected pilot can be made temporarily invulnerable after ejection.
/// Voice lines and sound effects are played during ejection.
/// (Matches C++ EjectPilotDie)
#[derive(Debug)]
pub struct EjectPilotDie {
    base: DieModule<EjectPilotDieModuleData>,
}

impl EjectPilotDie {
    /// Create a new EjectPilotDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<EjectPilotDieModuleData>) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "EjectPilotDie"
    }

    /// Eject pilot from the vehicle
    /// (Matches C++ EjectPilotDie::ejectPilot in EjectPilotDie.cpp lines 60-75)
    fn eject_pilot(
        &self,
        ocl_names: &[String],
        dying_object: &Object,
        damage_dealer: Option<&Object>,
    ) {
        let ocl_name = match ocl_names.first() {
            Some(name) => name,
            None => return,
        };

        let Some(ocl) = TheObjectCreationListStore::find_object_creation_list(ocl_name) else {
            return;
        };

        let ctx = crate::object_creation_list::live_creation_context();
        let _ = ocl.create_with_objects(&ctx, dying_object, damage_dealer, 0);
    }

    /// Play ejection voice and sound effects
    /// (Matches C++ EjectPilotDie.cpp lines 67-74)
    fn play_eject_sounds(&self, dying_object: &Object) {
        let Some(audio) = crate::helpers::TheAudio::get() else {
            return;
        };

        let position = dying_object.get_position();
        let pos_tuple = (position.x, position.y, position.z);
        let template = dying_object.get_template();

        if let Some(mut voice) = template.get_per_unit_sound("VoiceEject") {
            voice.set_position(&pos_tuple);
            if let Some(player) = dying_object.get_controlling_player() {
                if let Ok(player_guard) = player.read() {
                    voice.set_player_index(player_guard.get_player_index() as u32);
                }
            }
            audio.add_audio_event(&voice);
        }

        if let Some(mut sound) = template.get_per_unit_sound("SoundEject") {
            sound.set_position(&pos_tuple);
            audio.add_audio_event(&sound);
        }
    }

    /// Check if object is significantly above terrain
    /// (Matches C++ Object::isSignificantlyAboveTerrain)
    fn is_significantly_above_terrain(&self, object: &Object) -> bool {
        // Use the object's built-in terrain height checking
        // C++ uses: object->isSignificantlyAboveTerrain()
        // This determines whether to spawn a pilot with parachute (in air)
        // or standing on the ground (on ground)
        object.is_significantly_above_terrain()
    }
}

impl DieModuleInterface for EjectPilotDie {
    /// Called when the object dies - ejects pilot
    /// (Matches C++ EjectPilotDie::onDie in EjectPilotDie.cpp lines 80-88)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // C++ line 82-83: Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        // C++ line 84: Get the damage dealer (object that killed this vehicle)
        let damage_dealer = TheGameLogic::find_object_by_id(damage_info.input.source_id);
        let damage_dealer_guard = damage_dealer.as_ref().and_then(|h| h.read().ok());

        // C++ line 86: Determine which OCL to use based on height above terrain
        // If significantly above terrain, use air OCL (pilot with parachute)
        // Otherwise use ground OCL (pilot standing/running)
        let ocl = if self.is_significantly_above_terrain(object) {
            &self.base.module_data.ocl_in_air
        } else {
            &self.base.module_data.ocl_on_ground
        };

        // C++ line 87: Eject the pilot using the selected OCL
        self.eject_pilot(ocl, object, damage_dealer_guard.as_deref());
        self.play_eject_sounds(object);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eject_pilot_die_module_data_default() {
        let data = EjectPilotDieModuleData::default();
        assert_eq!(data.ocl_in_air.len(), 0);
        assert_eq!(data.ocl_on_ground.len(), 0);
        assert_eq!(data.invulnerable_time, 0);
    }

    #[test]
    fn test_eject_pilot_die_module_name() {
        assert_eq!(EjectPilotDie::get_module_name(), "EjectPilotDie");
    }

    #[test]
    fn test_eject_pilot_die_with_ocls() {
        let mut data = EjectPilotDieModuleData::default();
        data.ocl_in_air.push("PilotParachute".to_string());
        data.ocl_on_ground.push("PilotStanding".to_string());
        data.invulnerable_time = 3000; // 3 seconds

        assert_eq!(data.ocl_in_air.len(), 1);
        assert_eq!(data.ocl_on_ground.len(), 1);
        assert_eq!(data.invulnerable_time, 3000);
    }
}
