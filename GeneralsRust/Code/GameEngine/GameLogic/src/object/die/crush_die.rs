//! CrushDie - Special death behavior when crushed by vehicles
//!
//! Original C++ location: GameLogic/Module/CrushDie.h/.cpp
//! Original C++ Author: Colin Day, November 2001
//! Rust conversion: 2025

use super::{DieModule, DieModuleData, DieModuleInterface};
use crate::common::audio::AudioEventRts;
use crate::common::Int;
use crate::damage::{DamageInfo, DamageType};
use crate::helpers::TheAudio;
use crate::object::die::{
    parse_die_mux_death_types, parse_die_mux_exempt_status, parse_die_mux_required_status,
    parse_die_mux_veterancy_levels,
};
use crate::object::Object;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock};

/// Crush type enumeration - indicates which part was crushed
/// (Matches C++ CrushEnum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CrushEnum {
    TotalCrush = 0,
    BackEndCrush = 1,
    FrontEndCrush = 2,
    NoCrush = 3,
}

/// Number of crush types
pub const CRUSH_COUNT: usize = 4;

/// Module data for CrushDie
/// (Matches C++ CrushDieModuleData)
#[derive(Debug, Clone)]
pub struct CrushDieModuleData {
    pub base: DieModuleData,
    /// Sound effects for each crush type
    pub crush_sounds: [Option<String>; CRUSH_COUNT],
    /// Percentage chance to play each crush sound (0-100)
    pub crush_sound_percent: [Int; CRUSH_COUNT],
}

impl Default for CrushDieModuleData {
    fn default() -> Self {
        Self {
            base: DieModuleData::default(),
            crush_sounds: [None, None, None, None],
            crush_sound_percent: [100, 100, 100, 100],
        }
    }
}

impl Snapshotable for CrushDieModuleData {
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

crate::impl_legacy_module_data_via_base!(CrushDieModuleData, base);

impl CrushDieModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CRUSH_DIE_FIELDS)
    }
}

fn parse_die_death_types(
    _ini: &mut INI,
    data: &mut CrushDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_death_types(&mut data.base.die_mux_data, tokens)
}

fn parse_die_veterancy_levels(
    _ini: &mut INI,
    data: &mut CrushDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_veterancy_levels(&mut data.base.die_mux_data, tokens)
}

fn parse_die_exempt_status(
    _ini: &mut INI,
    data: &mut CrushDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_exempt_status(&mut data.base.die_mux_data, tokens)
}

fn parse_die_required_status(
    _ini: &mut INI,
    data: &mut CrushDieModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    parse_die_mux_required_status(&mut data.base.die_mux_data, tokens)
}

fn parse_crush_sound(
    _ini: &mut INI,
    data: &mut CrushDieModuleData,
    tokens: &[&str],
    index: usize,
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    if token.eq_ignore_ascii_case("None") {
        data.crush_sounds[index] = None;
    } else {
        data.crush_sounds[index] = Some((*token).to_string());
    }
    Ok(())
}

fn parse_crush_sound_percent(
    _ini: &mut INI,
    data: &mut CrushDieModuleData,
    tokens: &[&str],
    index: usize,
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.crush_sound_percent[index] = token.parse::<Int>().map_err(|_| INIError::InvalidData)?;
    Ok(())
}

const CRUSH_DIE_FIELDS: &[FieldParse<CrushDieModuleData>] = &[
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
        token: "TotalCrushSound",
        parse: |ini, data, tokens| {
            parse_crush_sound(ini, data, tokens, CrushEnum::TotalCrush as usize)
        },
    },
    FieldParse {
        token: "BackEndCrushSound",
        parse: |ini, data, tokens| {
            parse_crush_sound(ini, data, tokens, CrushEnum::BackEndCrush as usize)
        },
    },
    FieldParse {
        token: "FrontEndCrushSound",
        parse: |ini, data, tokens| {
            parse_crush_sound(ini, data, tokens, CrushEnum::FrontEndCrush as usize)
        },
    },
    FieldParse {
        token: "TotalCrushSoundPercent",
        parse: |ini, data, tokens| {
            parse_crush_sound_percent(ini, data, tokens, CrushEnum::TotalCrush as usize)
        },
    },
    FieldParse {
        token: "BackEndCrushSoundPercent",
        parse: |ini, data, tokens| {
            parse_crush_sound_percent(ini, data, tokens, CrushEnum::BackEndCrush as usize)
        },
    },
    FieldParse {
        token: "FrontEndCrushSoundPercent",
        parse: |ini, data, tokens| {
            parse_crush_sound_percent(ini, data, tokens, CrushEnum::FrontEndCrush as usize)
        },
    },
];

/// CrushDie - Handles special death when crushed by vehicles
///
/// When an object (typically infantry) is crushed by a vehicle, this module:
/// - Determines which part was crushed (front, back, or total)
/// - Plays appropriate crush sound effects
/// - Sets visual model conditions to show crushed state
/// - Updates body module crush flags
///
/// The crush location is calculated based on the positions of the crusher
/// and victim, and which crush points have already been hit.
/// (Matches C++ CrushDie)
#[derive(Debug)]
pub struct CrushDie {
    base: DieModule<CrushDieModuleData>,
}

impl CrushDie {
    /// Create a new CrushDie module
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<CrushDieModuleData>) -> Self {
        Self {
            base: DieModule::new(object, module_data),
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "CrushDie"
    }

    /// Determine which part of the object was crushed
    /// (Matches C++ crushLocationCheck in CrushDie.cpp lines 25-121)
    fn crush_location_check(&self, crusher: &Object, victim: &Object) -> CrushEnum {
        // Get crush flags from body module
        let (front_crushed, back_crushed) = if let Some(body) = victim.get_body_module() {
            let body_guard = body.lock().unwrap();
            (
                body_guard.get_front_crushed(),
                body_guard.get_back_crushed(),
            )
        } else {
            (false, false)
        };

        // Get positions
        let crusher_pos = crusher.get_position();
        let victim_pos = victim.get_position();

        // Get geometry info for crush point offset calculation
        // (Matches C++ CrushDie.cpp line 38: getMajorRadius() * 0.5)
        let crush_point_offset_distance = victim.get_geometry_info().get_major_radius() * 0.5;

        // Get unit direction vector - the direction the unit is facing
        // C++ uses: victim->getUnitDirectionVector2D()
        let (victim_dir_x, victim_dir_y) = victim.get_unit_direction_vector_2d();

        let mut best_crush_type = CrushEnum::NoCrush;
        let mut best_distance = 99999.0;

        // Check middle crush point if neither end is crushed
        // (Matches C++ CrushDie.cpp lines 54-66)
        if !front_crushed && !back_crushed {
            let dx = victim_pos.x - crusher_pos.x;
            let dy = victim_pos.y - crusher_pos.y;
            let dist = dx * dx + dy * dy;

            best_crush_type = CrushEnum::TotalCrush;
            best_distance = dist;
        }

        // Check front crush point if not already crushed
        // (Matches C++ CrushDie.cpp lines 68-92)
        if !front_crushed {
            let crush_point_offset_x = victim_dir_x * crush_point_offset_distance;
            let crush_point_offset_y = victim_dir_y * crush_point_offset_distance;

            let front_x = victim_pos.x + crush_point_offset_x;
            let front_y = victim_pos.y + crush_point_offset_y;

            let dx = front_x - crusher_pos.x;
            let dy = front_y - crusher_pos.y;
            let dist = dx * dx + dy * dy;

            if dist < best_distance {
                if back_crushed {
                    best_crush_type = CrushEnum::TotalCrush;
                } else {
                    best_crush_type = CrushEnum::FrontEndCrush;
                }
                best_distance = dist;
            }
        }

        // Check back crush point if not already crushed
        // (Matches C++ CrushDie.cpp lines 94-118)
        if !back_crushed {
            let crush_point_offset_x = victim_dir_x * crush_point_offset_distance;
            let crush_point_offset_y = victim_dir_y * crush_point_offset_distance;

            let back_x = victim_pos.x - crush_point_offset_x;
            let back_y = victim_pos.y - crush_point_offset_y;

            let dx = back_x - crusher_pos.x;
            let dy = back_y - crusher_pos.y;
            let dist = dx * dx + dy * dy;

            if dist < best_distance {
                if front_crushed {
                    best_crush_type = CrushEnum::TotalCrush;
                } else {
                    best_crush_type = CrushEnum::BackEndCrush;
                }
            }
        }

        best_crush_type
    }

    /// Play crush sound effect with random chance
    /// (Matches C++ CrushDie.cpp lines 155-165)
    fn play_crush_sound(&self, crush_type: CrushEnum, _object: &Object) {
        let crush_idx = crush_type as usize;
        let sound_name = match &self.base.module_data.crush_sounds[crush_idx] {
            Some(name) => name,
            None => return,
        };

        // Check random percentage
        // C++ uses: GameLogicRandomValue(0, 99) < crushSoundPercent
        // This generates a value from 0-99, so 0==never, 100==always
        let sound_percent = self.base.module_data.crush_sound_percent[crush_idx];

        let random_value = crate::GameLogicRandomValue!(0, 99) as i32;

        // Only play if random value is less than the percentage
        if random_value >= sound_percent {
            return;
        }

        if let Some(audio) = TheAudio::get() {
            let mut crush_sound = AudioEventRts::new(sound_name.as_str());
            crush_sound.set_object_id(_object.get_id());
            let handle = audio.add_audio_event(&crush_sound);
            crush_sound.set_playing_handle(handle);
        }
    }

    /// Update model conditions to show crushed state
    /// (Matches C++ CrushDie.cpp lines 166-181)
    fn update_model_conditions(&self, object: &mut Object, crush_type: CrushEnum) {
        // C++ lines 171-172: Determine which ends are crushed
        let front_crushed =
            crush_type == CrushEnum::TotalCrush || crush_type == CrushEnum::FrontEndCrush;
        let back_crushed =
            crush_type == CrushEnum::TotalCrush || crush_type == CrushEnum::BackEndCrush;

        // C++ lines 171-172: Update body module crush flags
        if let Some(body) = object.get_body_module() {
            let mut body_guard = body.lock().unwrap();
            if let Err(err) = body_guard.set_front_crushed(front_crushed) {
                log::warn!(
                    "CrushDie: failed to set front-crushed flag for object {}: {}",
                    object.get_id(),
                    err
                );
            }
            if let Err(err) = body_guard.set_back_crushed(back_crushed) {
                log::warn!(
                    "CrushDie: failed to set back-crushed flag for object {}: {}",
                    object.get_id(),
                    err
                );
            }
        }

        let clear_flags = crate::common::ModelConditionFlags::FRONTCRUSHED
            | crate::common::ModelConditionFlags::BACKCRUSHED;
        let mut set_flags = crate::common::ModelConditionFlags::empty();
        if front_crushed {
            set_flags |= crate::common::ModelConditionFlags::FRONTCRUSHED;
        }
        if back_crushed {
            set_flags |= crate::common::ModelConditionFlags::BACKCRUSHED;
        }
        if let Err(err) = object.clear_and_set_model_condition_flags(clear_flags, set_flags) {
            log::warn!(
                "CrushDie: failed to update crushed model conditions for object {}: {}",
                object.get_id(),
                err
            );
        }
    }
}

impl DieModuleInterface for CrushDie {
    /// Called when the object dies - handles crush death
    /// (Matches C++ CrushDie::onDie lines 139-184)
    fn on_die(&mut self, object: &mut Object, damage_info: &DamageInfo) {
        // Check if this die module should activate
        if !self.is_die_applicable(
            object,
            damage_info,
            &self.base.module_data.base.die_mux_data,
        ) {
            return;
        }

        // C++ line 144-146: This should only be used for crush damage
        debug_assert!(
            damage_info.input.damage_type == DamageType::Crush,
            "CrushDie expects crush damage"
        );
        if damage_info.input.damage_type != DamageType::Crush {
            return;
        }

        // C++ line 148: Get the crusher object
        let damage_dealer =
            crate::helpers::TheGameLogic::find_object_by_id(damage_info.input.source_id);
        debug_assert!(
            damage_dealer.is_some(),
            "You must have a damageDealer source for this effect"
        );

        // C++ line 151: Determine crush location - defaults to TOTAL_CRUSH if no crusher
        let crush_type = if let Some(ref crusher_arc) = damage_dealer {
            if let Ok(crusher_guard) = crusher_arc.read() {
                self.crush_location_check(&crusher_guard, object)
            } else {
                CrushEnum::TotalCrush
            }
        } else {
            CrushEnum::TotalCrush
        };

        // C++ lines 153-183: Handle crush effects if valid crush type
        if crush_type != CrushEnum::NoCrush {
            // C++ lines 155-165: Play crush sound with random chance
            self.play_crush_sound(crush_type, object);

            // C++ lines 166-181: Update body module and model conditions
            self.update_model_conditions(object, crush_type);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crush_die_module_data_default() {
        let data = CrushDieModuleData::default();
        assert_eq!(data.crush_sounds[0], None);
        assert_eq!(data.crush_sound_percent[0], 100);
        assert_eq!(data.crush_sound_percent[1], 100);
        assert_eq!(data.crush_sound_percent[2], 100);
        assert_eq!(data.crush_sound_percent[3], 100);
    }

    #[test]
    fn test_crush_die_module_name() {
        assert_eq!(CrushDie::get_module_name(), "CrushDie");
    }

    #[test]
    fn test_crush_enum_values() {
        assert_eq!(CrushEnum::TotalCrush as u32, 0);
        assert_eq!(CrushEnum::BackEndCrush as u32, 1);
        assert_eq!(CrushEnum::FrontEndCrush as u32, 2);
        assert_eq!(CrushEnum::NoCrush as u32, 3);
    }

    #[test]
    fn test_crush_die_with_sounds() {
        let mut data = CrushDieModuleData::default();
        data.crush_sounds[CrushEnum::TotalCrush as usize] = Some("CrushSound".to_string());
        data.crush_sound_percent[CrushEnum::TotalCrush as usize] = 75;

        assert!(data.crush_sounds[0].is_some());
        assert_eq!(data.crush_sound_percent[0], 75);
    }
}
