//! FlammableUpdate - Rust conversion of C++ FlammableUpdate
//!
//! Fire spreading and burning behavior.
//! Author: EA Pacific (C++ version)
//! Rust conversion: 2025

use crate::common::{AsciiString, Bool, ModuleData, Real, UnsignedInt, XferVersion};
use crate::damage::DamageType;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use std::sync::{Arc, RwLock, Weak};

/// Flammability status types - matches C++ FlammabilityStatusType
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlammabilityStatus {
    Normal,
    Aflame,
    Burned,
}

#[derive(Clone, Debug)]
pub struct FlammableUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Delay before object becomes "burned" (permanent state)
    pub burned_delay: UnsignedInt,
    /// Duration of aflame state
    pub aflame_duration: UnsignedInt,
    /// Delay between aflame damage ticks
    pub aflame_damage_delay: UnsignedInt,
    /// Damage dealt per aflame tick
    pub aflame_damage_amount: Real,
    /// Damage threshold to catch fire
    pub flame_damage_limit: Real,
    /// Time before flame damage threshold resets
    pub flame_damage_expiration_delay: UnsignedInt,
    /// C++ data includes this; audio playback is still handled by the sound runtime layer.
    pub burning_sound_name: AsciiString,
}

impl Default for FlammableUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            burned_delay: 0,
            aflame_duration: 300,
            aflame_damage_delay: 30,
            aflame_damage_amount: 1.0,
            flame_damage_limit: 20.0,
            flame_damage_expiration_delay: 60, // 2 seconds at 30 FPS
            burning_sound_name: AsciiString::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(FlammableUpdateModuleData, base);

impl FlammableUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FLAMMABLE_UPDATE_FIELDS)
    }
}

fn parse_duration_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

fn parse_int_as_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_int(token)? as Real);
    Ok(())
}

fn parse_ascii_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(AsciiString),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(AsciiString::from(*token));
    Ok(())
}

const FLAMMABLE_UPDATE_FIELDS: &[FieldParse<FlammableUpdateModuleData>] = &[
    FieldParse {
        token: "BurnedDelay",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.burned_delay = v, tokens)
        },
    },
    FieldParse {
        token: "AflameDuration",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.aflame_duration = v, tokens)
        },
    },
    FieldParse {
        token: "AflameDamageDelay",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.aflame_damage_delay = v, tokens)
        },
    },
    FieldParse {
        token: "AflameDamageAmount",
        parse: |ini, data, tokens| {
            parse_int_as_real_field(ini, &mut |v| data.aflame_damage_amount = v, tokens)
        },
    },
    FieldParse {
        token: "BurningSoundName",
        parse: |ini, data, tokens| {
            parse_ascii_field(ini, &mut |v| data.burning_sound_name = v, tokens)
        },
    },
    FieldParse {
        token: "FlameDamageLimit",
        parse: |ini, data, tokens| {
            parse_real_field(ini, &mut |v| data.flame_damage_limit = v, tokens)
        },
    },
    FieldParse {
        token: "FlameDamageExpiration",
        parse: |ini, data, tokens| {
            parse_duration_field(ini, &mut |v| data.flame_damage_expiration_delay = v, tokens)
        },
    },
];

pub struct FlammableUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<FlammableUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    status: FlammabilityStatus,
    aflame_end_frame: UnsignedInt,
    burned_end_frame: UnsignedInt,
    damage_end_frame: UnsignedInt,
    flame_damage_limit: Real,
    last_flame_damage_dealt: UnsignedInt,
}

impl FlammableUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<FlammableUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let flame_limit = specific_data.flame_damage_limit;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            status: FlammabilityStatus::Normal,
            aflame_end_frame: 0,
            burned_end_frame: 0,
            damage_end_frame: 0,
            flame_damage_limit: flame_limit,
            last_flame_damage_dealt: 0,
        })
    }

    /// Try to ignite the object - matches C++ tryToIgnite()
    pub fn try_to_ignite(&mut self) {
        if self.status != FlammabilityStatus::Normal {
            return;
        }

        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let data = &self.module_data;

        // Set aflame state
        self.status = FlammabilityStatus::Aflame;
        self.aflame_end_frame = current_frame + data.aflame_duration;
        self.burned_end_frame = if data.burned_delay > 0 {
            current_frame + data.burned_delay
        } else {
            0
        };
        self.damage_end_frame = if data.aflame_damage_delay > 0 {
            current_frame + data.aflame_damage_delay
        } else {
            0
        };

        // Set object status and model condition
        if let Some(object_arc) = self.object.upgrade() {
            if let Ok(mut obj) = object_arc.write() {
                obj.set_status(crate::common::ObjectStatusMaskType::AFLAME, true);
                obj.set_model_condition_state(crate::common::ModelConditionFlags::Aflame);
            }
        }

        log::debug!(
            "Object ignited, will burn until frame {}",
            self.aflame_end_frame
        );
    }

    /// Check if object would ignite (for fire spread checking)
    pub fn would_ignite(&self) -> Bool {
        self.status == FlammabilityStatus::Normal
    }

    /// Check if object is on fire
    pub fn is_on_fire(&self) -> Bool {
        self.status == FlammabilityStatus::Aflame
    }

    /// Check if object is burned out (permanent state)
    pub fn is_burned(&self) -> Bool {
        self.status == FlammabilityStatus::Burned
    }

    /// Handle damage received - C++ onDamage()
    pub fn on_damage(&mut self, damage_amount: Real, damage_type: u32) {
        // Only react to flame damage (damage_type would be DAMAGE_FLAME or DAMAGE_PARTICLE_BEAM)
        const DAMAGE_FLAME: u32 = DamageType::Flame as u32;
        const DAMAGE_PARTICLE_BEAM: u32 = DamageType::ParticleBeam as u32;

        if damage_type != DAMAGE_FLAME && damage_type != DAMAGE_PARTICLE_BEAM {
            return;
        }

        let current_frame = crate::helpers::TheGameLogic::get_frame();

        // Reset threshold if it's been a long time since last flame damage
        if current_frame.saturating_sub(self.module_data.flame_damage_expiration_delay)
            > self.last_flame_damage_dealt
        {
            self.flame_damage_limit = self.module_data.flame_damage_limit;
        }
        self.last_flame_damage_dealt = current_frame;

        // Check if we should catch fire
        if self.status == FlammabilityStatus::Normal {
            self.flame_damage_limit -= damage_amount;
            if self.flame_damage_limit <= 0.0 {
                self.try_to_ignite();
            }
        }
    }

    /// Apply aflame damage to the object - C++ doAflameDamage()
    fn do_aflame_damage(&self) {
        if let Some(object_arc) = self.object.upgrade() {
            if let Ok(mut obj) = object_arc.write() {
                let damage = self.module_data.aflame_damage_amount;
                let current_health = obj.get_health();
                let new_health = (current_health - damage).max(0.0);
                if let Err(err) = obj.set_health(new_health) {
                    log::warn!(
                        "FlammableUpdate: failed to apply aflame damage to object {}: {}",
                        obj.get_id(),
                        err
                    );
                }
                log::trace!("Aflame damage: {} -> {}", current_health, new_health);
            }
        }
    }

    /// Calculate sleep time until next important event - C++ calcSleepTime()
    fn calc_sleep_time(&self) -> UpdateSleepTime {
        if self.status != FlammabilityStatus::Aflame || self.aflame_end_frame == 0 {
            return UpdateSleepTime::Forever;
        }

        let current_frame = crate::helpers::TheGameLogic::get_frame();
        if self.aflame_end_frame <= current_frame {
            return UpdateSleepTime::Forever;
        }

        // Find soonest event
        let mut soonest = self.aflame_end_frame;
        if self.burned_end_frame > current_frame && self.burned_end_frame < soonest {
            soonest = self.burned_end_frame;
        }
        if self.damage_end_frame > current_frame && self.damage_end_frame < soonest {
            soonest = self.damage_end_frame;
        }

        UpdateSleepTime::Frames(soonest.saturating_sub(current_frame))
    }
}

impl UpdateModuleInterface for FlammableUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if self.status != FlammabilityStatus::Aflame {
            return UpdateSleepTime::Forever;
        }

        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let data = &self.module_data;

        // Check damage timer
        if self.damage_end_frame > 0 && current_frame >= self.damage_end_frame {
            self.damage_end_frame = current_frame + data.aflame_damage_delay;
            self.do_aflame_damage();
        }

        // Check burned timer (sets permanent burned status)
        if self.burned_end_frame > 0 && current_frame >= self.burned_end_frame {
            if let Some(object_arc) = self.object.upgrade() {
                if let Ok(mut obj) = object_arc.write() {
                    obj.set_status(crate::common::ObjectStatusMaskType::BURNED, true);
                    obj.set_model_condition_state(crate::common::ModelConditionFlags::SMOLDERING);
                }
            }
            self.burned_end_frame = 0; // Only set once
        }

        // Check aflame timer (fire goes out)
        if self.aflame_end_frame > 0 && current_frame >= self.aflame_end_frame {
            // Determine final state
            if let Some(object_arc) = self.object.upgrade() {
                if let Ok(mut obj) = object_arc.write() {
                    let is_burned = obj
                        .get_status_bits()
                        .contains(crate::common::ObjectStatusMaskType::BURNED);

                    if is_burned {
                        self.status = FlammabilityStatus::Burned;
                    } else {
                        self.status = FlammabilityStatus::Normal;
                    }

                    // Clear aflame status
                    obj.set_status(crate::common::ObjectStatusMaskType::AFLAME, false);
                    obj.clear_model_condition_state(crate::common::ModelConditionFlags::Aflame);
                }
            }

            log::debug!("Object stopped burning, status: {:?}", self.status);
            return UpdateSleepTime::Forever;
        }

        self.calc_sleep_time()
    }
}

impl BehaviorModuleInterface for FlammableUpdate {
    fn get_module_name(&self) -> &'static str {
        "FlammableUpdate"
    }

    fn try_to_ignite_flammable(&mut self) {
        self.try_to_ignite();
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for FlammableUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("FlammableUpdate xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        let mut status = self.status as i32;
        xfer.xfer_int(&mut status)
            .map_err(|e| format!("FlammableUpdate xfer status: {:?}", e))?;
        self.status = match status {
            0 => FlammabilityStatus::Normal,
            1 => FlammabilityStatus::Aflame,
            _ => FlammabilityStatus::Burned,
        };
        xfer.xfer_unsigned_int(&mut self.aflame_end_frame)
            .map_err(|e| format!("FlammableUpdate xfer aflame_end_frame: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.burned_end_frame)
            .map_err(|e| format!("FlammableUpdate xfer burned_end_frame: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.damage_end_frame)
            .map_err(|e| format!("FlammableUpdate xfer damage_end_frame: {:?}", e))?;
        xfer.xfer_real(&mut self.flame_damage_limit)
            .map_err(|e| format!("FlammableUpdate xfer flame_damage_limit: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.last_flame_damage_dealt)
            .map_err(|e| format!("FlammableUpdate xfer last_flame_damage_dealt: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct FlammableUpdateFactory;
impl FlammableUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(FlammableUpdate::new(thing, module_data)?))
    }
}
