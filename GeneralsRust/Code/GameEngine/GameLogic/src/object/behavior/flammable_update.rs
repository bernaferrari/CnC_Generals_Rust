//! FlammableUpdate - Rust conversion of C++ FlammableUpdate
//!
//! Fire spreading and burning behavior.
//! Author: EA Pacific (C++ version)
//! Rust conversion: 2025

use crate::common::{AsciiString, Bool, ModuleData, ObjectID, Real, UnsignedInt, XferVersion};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::Object as GameObject;
use crate::object::behavior::behavior_module::{BehaviorModuleData, xfer_update_module_base_state};
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::Module;
use std::sync::{Arc, RwLock, Weak};

/// Wave 379: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

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
            aflame_duration: 0,
            aflame_damage_delay: 0,
            aflame_damage_amount: 0.0,
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
    setter(INI::parse_duration_unsigned_int(required_value(tokens)?)?);
    Ok(())
}

fn parse_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    setter(INI::parse_real(required_value(tokens)?)?);
    Ok(())
}

fn parse_int_as_real_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    setter(INI::parse_int(required_value(tokens)?)? as Real);
    Ok(())
}

fn parse_ascii_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(AsciiString),
    tokens: &[&str],
) -> Result<(), INIError> {
    let value = INI::parse_ascii_string(required_value(tokens)?)?;
    setter(AsciiString::from(value.as_str()));
    Ok(())
}

fn required_value<'a>(tokens: &'a [&str]) -> Result<&'a str, INIError> {
    match tokens {
        ["=", value, ..] => Ok(*value),
        [value, ..] if *value != "=" => Ok(*value),
        _ => Err(INIError::InvalidData),
    }
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
    object_id: ObjectID,
    module_data: Arc<FlammableUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    status: FlammabilityStatus,
    aflame_end_frame: UnsignedInt,
    burned_end_frame: UnsignedInt,
    damage_end_frame: UnsignedInt,
    flame_damage_limit: Real,
    /// Last `setWakeFrame` request issued on ignite (C++ FlammableUpdate.cpp:196). Test/debug.
    last_wake_sleep: Option<UpdateSleepTime>,
    /// Last fire-spread wake armed on ignite (FireSpreadUpdate.cpp:144). Test/debug.
    last_fire_spread_wake: Option<UpdateSleepTime>,
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
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            status: FlammabilityStatus::Normal,
            aflame_end_frame: 0,
            burned_end_frame: 0,
            damage_end_frame: 0,
            flame_damage_limit: flame_limit,
            last_flame_damage_dealt: 0,
            last_wake_sleep: None,
            last_fire_spread_wake: None,
        })
    }

    /// Try to ignite the object - matches C++ tryToIgnite()
    ///
    /// C++ `FlammableUpdate::tryToIgnite` (FlammableUpdate.cpp:170-197) always
    /// advances `m_status` to `FS_AFLAME` when currently `FS_NORMAL`. Object
    /// status / model-condition side effects are best-effort when the owner is
    /// not in a live registry (unit tests, host-only path).
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
        if let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            if let Ok(mut obj) = object_arc.write() {
                obj.set_status(crate::common::ObjectStatusMaskType::AFLAME, true);
                obj.set_model_condition_state(crate::common::ModelConditionFlags::Aflame);
            }
        }

        // C++ FlammableUpdate.cpp:180-186 — find the owner's FireSpreadUpdate and
        // startFireSpreading(), which re-arms it with
        // UPDATE_SLEEP(calcNextSpreadDelay()) (FireSpreadUpdate.cpp:139-145). The
        // wake is issued first so the owner's own wake below (C++ line 196,
        // setWakeFrame(getObject(), calcSleepTime())) lands last and drives the
        // burn state machine (damage ticks, burned status, burn-out).
        if let Some(delay) = self.fire_spread_wake_delay() {
            let sleep = UpdateSleepTime::Frames(delay);
            self.last_fire_spread_wake = Some(sleep);
            self.issue_wake(sleep);
        }
        self.issue_wake(self.calc_sleep_time());

        log::debug!(
            "Object ignited, will burn until frame {}",
            self.aflame_end_frame
        );
    }

    /// Issue one C++ `setWakeFrame(getObject(), sleep)` request for this
    /// module's owner. `TheGameLogic::set_wake_frame` is the live-path analog
    /// (same pattern as AutoHealBehavior / BaseRegenerateUpdate); host-only
    /// objects without an id skip it.
    fn issue_wake(&mut self, sleep: UpdateSleepTime) {
        self.last_wake_sleep = Some(sleep);
        if self.object_id == crate::common::INVALID_ID {
            return;
        }
        crate::helpers::TheGameLogic::set_wake_frame(self.object_id, sleep);
    }

    /// Delay `FireSpreadUpdate::startFireSpreading` would program when the
    /// owner is aflame (FireSpreadUpdate.cpp:139-145): `Some(calcNextSpreadDelay())`,
    /// or `None` when the owner has no FireSpreadUpdate module or is not aflame.
    ///
    /// Uses `try_with_module`: `ModuleEntry` guards its module with a Mutex, and
    /// ignition from the spread module's own update tick already holds that
    /// lock on this thread. There the module is running right now and re-arms
    /// itself from its returned sleep — identical to C++, where startFireSpreading
    /// inside FireSpreadUpdate::update is subsumed by UPDATE_SLEEP(calcNextSpreadDelay()).
    fn fire_spread_wake_delay(&self) -> Option<UnsignedInt> {
        if self.object_id == crate::common::INVALID_ID {
            return None;
        }
        let object_arc = crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))?;
        let object = object_arc.read().ok()?;
        let is_aflame = object
            .get_status_bits()
            .contains(crate::common::ObjectStatusMaskType::AFLAME);
        let fire_spread = object.find_update_module("FireSpreadUpdate")?;
        fire_spread.try_with_module(|module| {
            module
                .get_fire_spread_control_interface()
                .and_then(|fire_spread| fire_spread.wake_delay_if_aflame(is_aflame))
        })
        .flatten()
    }

    /// Last `setWakeFrame` request issued on ignite (C++ FlammableUpdate.cpp:196). Test/debug.
    pub fn last_wake_sleep(&self) -> Option<UpdateSleepTime> {
        self.last_wake_sleep
    }

    /// Last fire-spread wake armed on ignite (FireSpreadUpdate.cpp:144). Test/debug.
    pub fn last_fire_spread_wake(&self) -> Option<UpdateSleepTime> {
        self.last_fire_spread_wake
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
    /// (FlammableUpdate.cpp:202-213).
    ///
    /// The tick goes through `attemptDamage` with DAMAGE_FLAME + DEATH_BURNED so
    /// armor scales it, a lethal tick kills with DEATH_BURNED, and onDamage
    /// listeners re-enter — direct `set_health` bypasses all three (the Main
    /// host path models this via take_damage_from_typed_death Flame/Burned).
    ///
    /// C++ sources the packet from the burning object itself
    /// (`info.in.m_sourceID = getObject()->getID()`), but the live pipeline
    /// takes the source object's registry read-lock during attemptDamage while
    /// this module already holds the victim's write-lock (same RwLock), so
    /// self-sourcing would self-deadlock. `INVALID_ID` is the codebase's
    /// established source for self-applied pipeline damage
    /// (`Object::kill_with_type`); armor, death, and onDamage are unaffected.
    fn do_aflame_damage(&self) {
        // Wave 379: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        }) {
            if let Ok(mut obj) = object_arc.write() {
                let mut damage_info = DamageInfo::with_simple(
                    self.module_data.aflame_damage_amount,
                    crate::common::INVALID_ID,
                    DamageType::Flame,
                    DeathType::Burned,
                );
                if let Err(err) = obj.attempt_damage(&mut damage_info) {
                    log::debug!(
                        "FlammableUpdate: aflame damage to object {} failed: {}",
                        obj.get_id(),
                        err
                    );
                }
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
        // Wave 379: empty dual-world → Forever.
        if dual_world_registry_unavailable() {
            return UpdateSleepTime::Forever;
        }

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
            if let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            }) {
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
            if let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                    .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            }) {
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_cpp_constructor() {
        let data = FlammableUpdateModuleData::default();

        assert_eq!(data.burned_delay, 0);
        assert_eq!(data.aflame_duration, 0);
        assert_eq!(data.aflame_damage_delay, 0);
        assert_eq!(data.aflame_damage_amount, 0.0);
        assert_eq!(data.burning_sound_name.as_str(), "");
        assert_eq!(data.flame_damage_limit, 20.0);
        assert_eq!(
            data.flame_damage_expiration_delay,
            crate::common::LOGICFRAMES_PER_SECOND * 2
        );
    }

    #[test]
    fn field_parsers_use_cpp_ini_token_handling() {
        let mut ini = INI::new();
        let mut data = FlammableUpdateModuleData::default();

        parse_duration_field(&mut ini, &mut |v| data.burned_delay = v, &["=", "1000ms"]).unwrap();
        parse_duration_field(&mut ini, &mut |v| data.aflame_duration = v, &["=", "2s"]).unwrap();
        parse_duration_field(
            &mut ini,
            &mut |v| data.aflame_damage_delay = v,
            &["=", "500ms"],
        )
        .unwrap();
        parse_int_as_real_field(
            &mut ini,
            &mut |v| data.aflame_damage_amount = v,
            &["=", "7"],
        )
        .unwrap();
        parse_ascii_field(
            &mut ini,
            &mut |v| data.burning_sound_name = v,
            &["=", "FireLoop"],
        )
        .unwrap();
        parse_real_field(
            &mut ini,
            &mut |v| data.flame_damage_limit = v,
            &["=", "42.5"],
        )
        .unwrap();
        parse_duration_field(
            &mut ini,
            &mut |v| data.flame_damage_expiration_delay = v,
            &["=", "3s"],
        )
        .unwrap();

        assert_eq!(
            data.burned_delay,
            INI::parse_duration_unsigned_int("1000ms").unwrap()
        );
        assert_eq!(
            data.aflame_duration,
            INI::parse_duration_unsigned_int("2s").unwrap()
        );
        assert_eq!(
            data.aflame_damage_delay,
            INI::parse_duration_unsigned_int("500ms").unwrap()
        );
        assert_eq!(data.aflame_damage_amount, 7.0);
        assert_eq!(data.burning_sound_name.as_str(), "FireLoop");
        assert_eq!(data.flame_damage_limit, 42.5);
        assert_eq!(
            data.flame_damage_expiration_delay,
            INI::parse_duration_unsigned_int("3s").unwrap()
        );
    }

    #[test]
    fn field_parsers_reject_missing_values() {
        let mut ini = INI::new();
        let mut duration = 0;
        let mut real = 0.0;
        let mut text = AsciiString::new();

        let duration_err =
            parse_duration_field(&mut ini, &mut |v| duration = v, &["="]).unwrap_err();
        let real_err = parse_real_field(&mut ini, &mut |v| real = v, &["="]).unwrap_err();
        let int_err = parse_int_as_real_field(&mut ini, &mut |v| real = v, &["="]).unwrap_err();
        let ascii_err = parse_ascii_field(&mut ini, &mut |v| text = v, &["="]).unwrap_err();

        assert!(matches!(duration_err, INIError::InvalidData));
        assert!(matches!(real_err, INIError::InvalidData));
        assert!(matches!(int_err, INIError::InvalidData));
        assert!(matches!(ascii_err, INIError::InvalidData));
        assert_eq!(duration, 0);
        assert_eq!(real, 0.0);
        assert_eq!(text.as_str(), "");
    }

    #[test]
    fn try_to_ignite_wakes_burn_state_machine_like_cpp() {
        // C++ FlammableUpdate.cpp:196 — setWakeFrame(getObject(), calcSleepTime()).
        let object = Arc::new(RwLock::new(GameObject::new_test(9201, 100.0)));
        let data = Arc::new(FlammableUpdateModuleData {
            aflame_duration: 90,
            aflame_damage_delay: 30,
            aflame_damage_amount: 5.0,
            ..Default::default()
        });
        let mut flammable = FlammableUpdate::new(Arc::clone(&object), data).expect("flammable");

        flammable.try_to_ignite();

        assert_eq!(
            flammable.last_wake_sleep(),
            Some(UpdateSleepTime::Frames(30)),
            "ignite must wake the module at the soonest burn event (damage tick)"
        );
    }

    #[test]
    fn try_to_ignite_arms_fire_spread_update_like_cpp() {
        // C++ FlammableUpdate.cpp:180-186 (startFireSpreading) re-arms
        // FireSpreadUpdate with UPDATE_SLEEP(calcNextSpreadDelay())
        // (FireSpreadUpdate.cpp:139-145).
        let object_id = 9202;
        let object = Arc::new(RwLock::new(GameObject::new_test(object_id, 100.0)));
        crate::object::registry::OBJECT_REGISTRY.register_object(object_id, &object);

        let spread_data = Arc::new(
            crate::object::update::fire_spread_update::FireSpreadUpdateModuleData {
                min_spread_try_delay: 45,
                max_spread_try_delay: 45,
                ..Default::default()
            },
        );
        let behavior = crate::object::update::fire_spread_update::FireSpreadUpdate::new(
            object_id,
            (*spread_data).clone(),
        );
        object.write().unwrap().install_update_module(
            "FireSpreadUpdate",
            Box::new(crate::object::update::fire_spread_update::FireSpreadUpdateModule::new(
                behavior,
                &AsciiString::from("FireSpreadUpdate"),
                Arc::clone(&spread_data),
            )),
            spread_data,
        );

        let mut flammable = FlammableUpdate::new(
            Arc::clone(&object),
            Arc::new(FlammableUpdateModuleData::default()),
        )
        .expect("flammable");
        flammable.try_to_ignite();

        assert_eq!(
            flammable.last_fire_spread_wake(),
            Some(UpdateSleepTime::Frames(45)),
            "ignite must arm FireSpreadUpdate with its spread delay"
        );
        assert!(
            flammable.last_wake_sleep().is_some(),
            "ignite must also wake the burn state machine itself"
        );

        crate::object::registry::OBJECT_REGISTRY.unregister_object(object_id);
    }

    #[test]
    fn do_aflame_damage_goes_through_damage_pipeline() {
        // C++ FlammableUpdate.cpp:202-213 — attemptDamage with
        // DAMAGE_FLAME/DEATH_BURNED: a lethal tick kills via the pipeline with
        // DEATH_BURNED instead of parking health at 0 with no death event.
        let object_id = 9203;
        let object = Arc::new(RwLock::new(GameObject::new_test(object_id, 10.0)));
        crate::object::registry::OBJECT_REGISTRY.register_object(object_id, &object);
        let flammable = FlammableUpdate::new(
            Arc::clone(&object),
            Arc::new(FlammableUpdateModuleData {
                aflame_damage_amount: 25.0,
                ..Default::default()
            }),
        )
        .expect("flammable");

        flammable.do_aflame_damage();

        {
            let obj = object.read().unwrap();
            assert!(
                obj.is_effectively_dead(),
                "lethal burn tick must kill through the damage pipeline"
            );
            assert_eq!(obj.get_last_death_type(), Some(DeathType::Burned));
        }
        crate::object::registry::OBJECT_REGISTRY.unregister_object(object_id);

        let survivor_id = 9204;
        let survivor = Arc::new(RwLock::new(GameObject::new_test(survivor_id, 100.0)));
        crate::object::registry::OBJECT_REGISTRY.register_object(survivor_id, &survivor);
        let survivor_flammable = FlammableUpdate::new(
            Arc::clone(&survivor),
            Arc::new(FlammableUpdateModuleData {
                aflame_damage_amount: 7.0,
                ..Default::default()
            }),
        )
        .expect("flammable");

        survivor_flammable.do_aflame_damage();

        {
            let obj = survivor.read().unwrap();
            assert!(!obj.is_effectively_dead());
            assert_eq!(obj.get_health(), 93.0);
            // C++ ActiveBody records the packet on every hit, lethal or not:
            // the survivor carries the Flame/Burned packet but no death.
            let last = obj
                .get_last_damage_info()
                .expect("non-lethal burn tick must record the damage packet");
            assert_eq!(last.input.damage_type, DamageType::Flame);
            assert_eq!(last.input.death_type, DeathType::Burned);
        }
        crate::object::registry::OBJECT_REGISTRY.unregister_object(survivor_id);
    }

}
