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
    owner: Weak<RwLock<GameObject>>,
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
    audio_handle: u32,
    needs_aflame_side_effects: bool,
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
            owner: Arc::downgrade(&object),
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
            audio_handle: 0,
            needs_aflame_side_effects: false,
        })
    }

    /// Try to ignite the object - matches C++ tryToIgnite()
    ///
    /// C++ `FlammableUpdate::tryToIgnite` (FlammableUpdate.cpp:170-197) always
    /// advances `m_status` to `FS_AFLAME` when currently `FS_NORMAL`. Object
    /// status / model-condition effects target the exact constructor owner.
    /// A compatibility callback holding its Object loan records deferred work.
    fn object_arc(&self) -> Option<Arc<RwLock<GameObject>>> {
        (self.object_id != crate::common::INVALID_ID)
            .then(|| self.owner.upgrade())
            .flatten()
    }

    pub fn try_to_ignite(&mut self) {
        let frame = crate::helpers::TheGameLogic::get_frame();
        let mut audio = |event: &crate::common::audio::AudioEventRts| {
            crate::helpers::TheAudio::get().map_or(0, |audio| audio.add_audio_event(event))
        };
        let mut spread = |object: &GameObject| -> Result<Option<UnsignedInt>, String> {
            let is_aflame = object
                .get_status_bits()
                .contains(crate::common::ObjectStatusMaskType::AFLAME);
            let Some(module) = object.find_update_module("FireSpreadUpdate") else {
                return Ok(None);
            };
            Ok(module
                .try_with_module(|module| {
                    module
                        .get_fire_spread_control_interface()
                        .and_then(|spread| spread.wake_delay_if_aflame(is_aflame))
                })
                .flatten())
        };
        let mut wake = |object: &GameObject, name: &str, sleep: UpdateSleepTime| {
            let wake_frame = match sleep {
                UpdateSleepTime::None => frame.saturating_add(1),
                UpdateSleepTime::Forever => UpdateSleepTime::Forever.to_u32(),
                UpdateSleepTime::Frames(frames) => frame.saturating_add(frames),
            };
            object.reschedule_named_update(name, wake_frame);
            Ok(())
        };
        if let Err(error) =
            self.try_to_ignite_with_services_at(frame, &mut audio, &mut spread, &mut wake)
        {
            log::debug!("Flammable ignition unavailable: {error}");
        }
    }

    /// CPP status/body/model → sound → FireSpread → timers → own wake.
    /// The caller supplies service effects; all self access retains the exact
    /// constructor owner. No registry or frame publication is performed.
    fn try_to_ignite_with_services_at(
        &mut self,
        frame: UnsignedInt,
        audio: &mut dyn FnMut(&crate::common::audio::AudioEventRts) -> u32,
        spread: &mut dyn FnMut(&GameObject) -> Result<Option<UnsignedInt>, String>,
        wake: &mut dyn FnMut(&GameObject, &str, UpdateSleepTime) -> Result<(), String>,
    ) -> Result<(), String> {
        if self.status != FlammabilityStatus::Normal {
            return Ok(());
        }
        let owner = self
            .object_arc()
            .ok_or("Flammable owning object has retired")?;
        if let Ok(mut object) = owner.try_write() {
            object.set_status(crate::common::ObjectStatusMaskType::AFLAME, true);
            if let Some(body) = object.get_body_module() {
                body.lock()
                    .map_err(|_| "Flammable body poisoned")?
                    .set_aflame(true)
                    .map_err(|error| format!("Flammable body ignition: {error}"))?;
            }
            object.set_model_condition_state(crate::common::ModelConditionFlags::Aflame);
            self.needs_aflame_side_effects = false;
        } else {
            self.needs_aflame_side_effects = true;
        }
        let mut event =
            crate::common::audio::AudioEventRts::new(self.module_data.burning_sound_name.as_str());
        event.set_object_id(self.object_id);
        self.audio_handle = audio(&event);
        if let Ok(object) = owner.try_read() {
            if let Some(delay) = spread(&object)? {
                self.last_fire_spread_wake = Some(UpdateSleepTime::Frames(delay));
                wake(&object, "FireSpreadUpdate", UpdateSleepTime::Frames(delay))?;
            }
        }
        self.status = FlammabilityStatus::Aflame;
        self.aflame_end_frame = frame.wrapping_add(self.module_data.aflame_duration);
        self.burned_end_frame = if self.module_data.burned_delay > 0 {
            frame.wrapping_add(self.module_data.burned_delay)
        } else {
            0
        };
        self.damage_end_frame = if self.module_data.aflame_damage_delay > 0 {
            frame.wrapping_add(self.module_data.aflame_damage_delay)
        } else {
            0
        };
        let sleep = if self.needs_aflame_side_effects {
            UpdateSleepTime::None
        } else {
            self.calc_sleep_time_at(frame)
        };
        self.last_wake_sleep = Some(sleep);
        if let Ok(object) = owner.try_read() {
            wake(&object, "FlammableUpdate", sleep)?;
        }
        Ok(())
    }

    fn stop_burning_sound(&mut self) {
        if self.audio_handle == 0 {
            return;
        }
        if let Some(audio) = crate::helpers::TheAudio::get() {
            audio.remove_audio_event(self.audio_handle);
        }
        self.audio_handle = 0;
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
        if current_frame.wrapping_sub(self.module_data.flame_damage_expiration_delay)
            > self.last_flame_damage_dealt
        {
            self.flame_damage_limit = self.module_data.flame_damage_limit;
        }
        self.last_flame_damage_dealt = current_frame;

        let owner = self.object_arc();
        let can_ignite = match owner {
            None => false,
            Some(object_arc) => match object_arc.try_read() {
                Ok(obj) => {
                    let bits = obj.get_status_bits();
                    !bits.contains(crate::common::ObjectStatusMaskType::AFLAME)
                        && !bits.contains(crate::common::ObjectStatusMaskType::BURNED)
                }
                Err(_) => self.status == FlammabilityStatus::Normal,
            },
        };
        if can_ignite {
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
        // A retired constructor owner cannot receive a burn hit.
        if self.object_arc().is_none() {
            return;
        }

        if let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            self.object_arc()
        }) {
            if let Ok(mut obj) = object_arc.write() {
                let mut damage_info = DamageInfo::with_simple(
                    self.module_data.aflame_damage_amount,
                    self.object_id,
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
        self.calc_sleep_time_at(crate::helpers::TheGameLogic::get_frame())
    }
    fn calc_sleep_time_at(&self, current_frame: UnsignedInt) -> UpdateSleepTime {
        if self.status != FlammabilityStatus::Aflame || self.aflame_end_frame == 0 {
            return UpdateSleepTime::Forever;
        }

        if self.aflame_end_frame <= current_frame {
            // C++ calcSleepTime returns FOREVER when the unsigned deadline
            // is not in the future, including after frame addition wraps.
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

    fn apply_deferred_aflame_side_effects(&mut self) {
        let Some(object_arc) = self.object_arc() else {
            return;
        };
        let Ok(mut obj) = object_arc.try_write() else {
            return;
        };
        let bits = obj.get_status_bits();
        if bits.contains(crate::common::ObjectStatusMaskType::AFLAME) {
            self.needs_aflame_side_effects = false;
            return;
        }
        obj.set_status(crate::common::ObjectStatusMaskType::AFLAME, true);
        obj.set_model_condition_state(crate::common::ModelConditionFlags::Aflame);
        if let Some(body) = obj.get_body_module() {
            if let Ok(mut body_guard) = body.lock() {
                let _ = body_guard.set_aflame(true);
            }
        }
        self.needs_aflame_side_effects = false;
    }
}

impl UpdateModuleInterface for FlammableUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // Missing/retired owners have no runnable burn state.
        if self.object_arc().is_none() {
            return UpdateSleepTime::Forever;
        }
        if self.needs_aflame_side_effects || self.status == FlammabilityStatus::Aflame {
            self.apply_deferred_aflame_side_effects();
        }

        if self.status != FlammabilityStatus::Aflame {
            return UpdateSleepTime::Forever;
        }

        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let data = &self.module_data;

        // Check damage timer
        if self.damage_end_frame > 0 && current_frame >= self.damage_end_frame {
            self.damage_end_frame = current_frame.wrapping_add(data.aflame_damage_delay);
            self.do_aflame_damage();
            if self.needs_aflame_side_effects {
                self.apply_deferred_aflame_side_effects();
            }
        }

        // Check burned timer (sets permanent burned status)
        if self.burned_end_frame > 0 && current_frame >= self.burned_end_frame {
            if let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                self.object_arc()
            }) {
                if let Ok(mut obj) = object_arc.write() {
                    obj.set_status(crate::common::ObjectStatusMaskType::BURNED, true);
                    obj.set_model_condition_state(crate::common::ModelConditionFlags::SMOLDERING);
                    self.burned_end_frame = 0;
                }
            }
        }

        if self.aflame_end_frame > 0 && current_frame >= self.aflame_end_frame {
            if let Some(object_arc) = (if self.object_id == crate::common::INVALID_ID {
                None
            } else {
                self.object_arc()
            }) {
                let Ok(mut obj) = object_arc.write() else {
                    return self.calc_sleep_time();
                };
                let is_burned = obj
                    .get_status_bits()
                    .contains(crate::common::ObjectStatusMaskType::BURNED);
                self.status = if is_burned {
                    FlammabilityStatus::Burned
                } else {
                    FlammabilityStatus::Normal
                };
                obj.set_status(crate::common::ObjectStatusMaskType::AFLAME, false);
                obj.clear_model_condition_state(crate::common::ModelConditionFlags::Aflame);
                if let Some(body) = obj.get_body_module() {
                    if let Ok(mut body_guard) = body.lock() {
                        let _ = body_guard.set_aflame(false);
                    }
                }
                drop(obj);
                self.stop_burning_sound();
            }
        }

        self.calc_sleep_time()
    }
}
impl Drop for FlammableUpdate {
    fn drop(&mut self) {
        self.stop_burning_sound();
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
    /// Sleepy-update table participant (see `ActiveBehaviorModule`).
    fn get_sleepy_update_interface(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_damage(&mut self) -> Option<&mut dyn crate::modules::DamageModuleInterface> {
        Some(self)
    }
}

impl crate::modules::DamageModuleInterface for FlammableUpdate {
    fn receive_damage(
        &mut self,
        _object_id: ObjectID,
        _damage: &crate::damage::DamageInfo,
    ) -> Real {
        0.0
    }

    fn on_damage(
        &mut self,
        damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.on_damage(
            damage_info.output.actual_damage_dealt,
            damage_info.input.damage_type as u32,
        );
        Ok(())
    }
}

impl Snapshotable for FlammableUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let _ = xfer;
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

/// Complete silent ignition on the installed canonical module. The original
/// FireSpread wake is immediate; this module's final wake executes immediately
/// after its own loan ends, allowing the real proxy to stamp its saved state.
/// Named sound lifetimes and non-ignition burn ticks remain separate migrations.
pub(crate) fn ignite_installed_silent_at(
    owner: &Arc<RwLock<GameObject>>,
    scheduler: &mut crate::system::game_logic::GameLogic,
    random: &mut game_engine::common::random_value::RandomState,
    frame: UnsignedInt,
) -> Result<(), String> {
    let (id, module, self_proxy, spread_proxies) = {
        let object = owner.read().map_err(|_| "ignition owner poisoned")?;
        let module = object
            .find_update_module("FlammableUpdate")
            .ok_or("installed Flammable absent")?;
        let mut proxies = object.named_update_registrations("FlammableUpdate");
        if proxies.len() != 1 {
            return Err("Flammable scheduling identity mismatch".into());
        }
        let spread = object.named_update_registrations("FireSpreadUpdate");
        (object.get_id(), module, proxies.pop().unwrap(), spread)
    };
    {
        let object = owner.read().map_err(|_| "ignition owner poisoned")?;
        if let Some(spread) = object.find_update_module("FireSpreadUpdate") {
            if spread_proxies.len() != 1 {
                return Err("FireSpread scheduling identity mismatch".into());
            }
            let supported = spread
                .try_with_module(|module| {
                    module
                        .as_any()
                        .is::<crate::object::update::fire_spread_update::FireSpreadUpdateModule>()
                })
                .ok_or("owned FireSpread callback is busy")?;
            if !supported {
                return Err("owned ignition requires canonical FireSpread control".into());
            }
        } else if !spread_proxies.is_empty() {
            return Err("FireSpread proxy has no installed module".into());
        }
    }
    scheduler.validate_scheduling_owner(id, owner)?;
    scheduler.validate_owned_update_wake(&self_proxy)?;
    for proxy in &spread_proxies {
        scheduler.validate_owned_update_wake(proxy)?;
    }
    let mut pending = None;
    module.with_module(|module| -> Result<(), String> {
        let flammable = module
            .as_any_mut()
            .downcast_mut::<crate::contain_module_overrides::ActiveBehaviorModule<FlammableUpdate>>(
            )
            .ok_or("canonical Flammable module mismatch")?
            .behavior_mut();
        if !Weak::ptr_eq(&flammable.owner, &Arc::downgrade(owner)) {
            return Err("Flammable belongs to another owner".into());
        }
        if !flammable.module_data.burning_sound_name.is_empty() {
            return Err("named burning sound requires its supplied stop/delete lifetime".into());
        }
        let mut audio = |_event: &crate::common::audio::AudioEventRts| 0;
        let mut spread = |object: &GameObject| {
            let Some(module) = object.find_update_module("FireSpreadUpdate") else {
                return Ok(None);
            };
            module
                .try_with_module(|module| {
                    module
                        .get_fire_spread_control_interface()
                        .ok_or("installed FireSpread control absent")?
                        .wake_delay_if_aflame_with_random(
                            object
                                .get_status_bits()
                                .contains(crate::common::ObjectStatusMaskType::AFLAME),
                            random,
                        )
                })
                .ok_or("owned FireSpread callback is busy")?
        };
        let mut wake = |_object: &GameObject, name: &str, sleep: UpdateSleepTime| {
            if name == "FlammableUpdate" {
                pending = Some(sleep);
                return Ok(());
            }
            for proxy in &spread_proxies {
                scheduler.awaken_update_module_with_current(proxy, None, frame, sleep.to_u32())?;
            }
            Ok(())
        };
        flammable.try_to_ignite_with_services_at(frame, &mut audio, &mut spread, &mut wake)
    })?;
    if let Some(sleep) = pending {
        scheduler.awaken_update_module_with_current(&self_proxy, None, frame, sleep.to_u32())?;
    }
    Ok(())
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

    fn ignition_fixture(
        id: ObjectID,
        data: FlammableUpdateModuleData,
        spread_delay: Option<u32>,
    ) -> (
        Arc<RwLock<GameObject>>,
        crate::system::game_logic::GameLogic,
    ) {
        let object = Arc::new(RwLock::new(GameObject::new_test(id, 100.0)));
        let data = Arc::new(data);
        let module_data: Arc<dyn game_engine::common::thing::module::ModuleData> = data;
        let mut factory = game_engine::common::thing::module_factory::ModuleFactory::new();
        crate::contain_module_overrides::register_native_module_overrides(&mut factory).unwrap();
        let thing: Arc<dyn game_engine::common::thing::module::Thing> =
            Arc::new(crate::object::object_thing::ObjectThingHandle::new(&object));
        let module = factory
            .new_module(
                thing,
                "FlammableUpdate",
                module_data.clone(),
                game_engine::common::thing::module::ModuleType::Behavior,
            )
            .unwrap();
        assert!(
            module
                .as_ref()
                .as_any()
                .is::<crate::contain_module_overrides::ActiveBehaviorModule<FlammableUpdate>>()
        );
        assert!(
            module
                .get_module_data()
                .as_any()
                .is::<FlammableUpdateModuleData>()
        );
        assert_eq!(
            module.get_module_tag_name_key(),
            module_data.get_module_tag_name_key()
        );
        object
            .write()
            .unwrap()
            .install_update_module("FlammableUpdate", module, module_data);
        if let Some(delay) = spread_delay {
            let data = Arc::new(
                crate::object::update::fire_spread_update::FireSpreadUpdateModuleData {
                    min_spread_try_delay: delay,
                    max_spread_try_delay: delay,
                    ..Default::default()
                },
            );
            let behavior = crate::object::update::fire_spread_update::FireSpreadUpdate::new(
                id,
                (*data).clone(),
            );
            let module = crate::object::update::fire_spread_update::FireSpreadUpdateModule::new(
                behavior,
                &AsciiString::from("FireSpreadUpdate"),
                data.clone(),
            );
            object.write().unwrap().install_update_module(
                "FireSpreadUpdate",
                Box::new(module),
                data,
            );
        }
        let entries: Vec<_> = object.read().unwrap().modules.iter().cloned().collect();
        let mut scheduler = crate::system::game_logic::GameLogic::new();
        scheduler.admit_scheduling_owner(id, object.clone());
        for entry in entries {
            let proxy: game_engine::common::thing::update_module::UpdateModulePtr = Arc::new(
                RwLock::new(crate::object::ModuleUpdateProxy::new(entry.clone(), id)),
            );
            object
                .write()
                .unwrap()
                .attach_update_module_registration(proxy.clone(), Some(&entry))
                .unwrap();
            scheduler.register_sleepy_update_module(id, proxy, UpdateSleepTime::Forever.to_u32());
        }
        (object, scheduler)
    }

    fn observe_flammable<R>(
        object: &Arc<RwLock<GameObject>>,
        inspect: impl FnOnce(&FlammableUpdate) -> R,
    ) -> R {
        let module = object
            .read()
            .unwrap()
            .find_update_module("FlammableUpdate")
            .unwrap();
        module.with_module(|module| inspect(module.as_any().downcast_ref::<
            crate::contain_module_overrides::ActiveBehaviorModule<FlammableUpdate>>().unwrap().behavior()))
    }

    #[test]
    fn actual_generic_factory_pins_supplied_owner_and_never_rebinds_retired_id() {
        use game_engine::common::thing::module::{ModuleType, Thing as FactoryThing};
        let owner = Arc::new(RwLock::new(GameObject::new_test(9203, 100.0)));
        let peer = Arc::new(RwLock::new(GameObject::new_test(9203, 250.0)));
        let thing: Arc<dyn FactoryThing> =
            Arc::new(crate::object::object_thing::ObjectThingHandle::new(&owner));
        let data: Arc<dyn game_engine::common::thing::module::ModuleData> =
            Arc::new(FlammableUpdateModuleData::default());
        let mut factory = game_engine::common::thing::module_factory::ModuleFactory::new();
        crate::contain_module_overrides::register_native_module_overrides(&mut factory).unwrap();
        let module = factory
            .new_module(
                thing.clone(),
                "FlammableUpdate",
                data.clone(),
                ModuleType::Behavior,
            )
            .unwrap();
        let behavior = module
            .as_ref()
            .as_any()
            .downcast_ref::<crate::contain_module_overrides::ActiveBehaviorModule<FlammableUpdate>>(
            )
            .expect("actual generic factory must construct FlammableUpdate")
            .behavior();
        assert!(Weak::ptr_eq(&behavior.owner, &Arc::downgrade(&owner)));
        assert!(!Weak::ptr_eq(&behavior.owner, &Arc::downgrade(&peer)));
        assert!(
            module
                .get_module_data()
                .as_any()
                .is::<FlammableUpdateModuleData>()
        );
        drop(owner);
        assert!(
            behavior.object_arc().is_none(),
            "retired construction owner cannot select the same-ID peer"
        );
        assert!(crate::object::object_thing::supplied_module_owner(thing.as_ref()).is_err());
        // The old infallible create-proc adapter remains an unavailable module;
        // it must never turn the retained peer into this retired Thing's owner.
        let retired = factory
            .new_module(thing, "FlammableUpdate", data, ModuleType::Behavior)
            .unwrap();
        assert!(
            !retired
                .as_ref()
                .as_any()
                .is::<crate::contain_module_overrides::ActiveBehaviorModule<FlammableUpdate>>()
        );
        assert!(
            !peer
                .read()
                .unwrap()
                .get_status_bits()
                .contains(crate::common::ObjectStatusMaskType::AFLAME)
        );
    }

    #[test]
    fn try_to_ignite_wakes_burn_state_machine_like_cpp() {
        let data = FlammableUpdateModuleData {
            aflame_duration: 90,
            burned_delay: 45,
            aflame_damage_delay: 30,
            aflame_damage_amount: 5.0,
            ..Default::default()
        };
        for frame in [0, u32::MAX - 10] {
            let (object, mut scheduler) = ignition_fixture(9201, data.clone(), None);
            ignite_installed_silent_at(&object, &mut scheduler, &mut Default::default(), frame)
                .unwrap();
            observe_flammable(&object, |flammable| {
                assert_eq!(flammable.status, FlammabilityStatus::Aflame);
                assert_eq!(flammable.aflame_end_frame, frame.wrapping_add(90));
                assert_eq!(flammable.burned_end_frame, frame.wrapping_add(45));
                assert_eq!(flammable.damage_end_frame, frame.wrapping_add(30));
                assert_eq!(
                    flammable.last_wake_sleep(),
                    Some(if frame == 0 {
                        UpdateSleepTime::Frames(30)
                    } else {
                        UpdateSleepTime::Forever
                    })
                );
            });
            if frame == 0 {
                let proxy = object
                    .read()
                    .unwrap()
                    .named_update_registrations("FlammableUpdate")
                    .pop()
                    .unwrap();
                assert_eq!(scheduler.sleepy_entry_for(&proxy).unwrap().1, 30);
            }
        }
    }

    #[test]
    fn try_to_ignite_arms_fire_spread_update_like_cpp() {
        let (object, mut scheduler) =
            ignition_fixture(9202, FlammableUpdateModuleData::default(), Some(45));
        let (peer, peer_scheduler) =
            ignition_fixture(9202, FlammableUpdateModuleData::default(), Some(80));
        let peer_proxy = peer
            .read()
            .unwrap()
            .named_update_registrations("FireSpreadUpdate")
            .pop()
            .unwrap();
        let peer_before = peer_scheduler.sleepy_entry_for(&peer_proxy).unwrap().1;
        ignite_installed_silent_at(&object, &mut scheduler, &mut Default::default(), 0).unwrap();
        observe_flammable(&object, |flammable| {
            assert_eq!(
                flammable.last_fire_spread_wake(),
                Some(UpdateSleepTime::Frames(45)),
                "ignite must arm FireSpreadUpdate with its spread delay"
            );
            assert!(
                flammable.last_wake_sleep().is_some(),
                "ignite must also wake its own machine"
            );
        });
        assert!(
            object
                .read()
                .unwrap()
                .get_status_bits()
                .contains(crate::common::ObjectStatusMaskType::AFLAME)
        );
        assert!(
            !peer
                .read()
                .unwrap()
                .get_status_bits()
                .contains(crate::common::ObjectStatusMaskType::AFLAME)
        );
        let proxy = object
            .read()
            .unwrap()
            .named_update_registrations("FireSpreadUpdate")
            .pop()
            .unwrap();
        assert_eq!(scheduler.sleepy_entry_for(&proxy).unwrap().1, 45);
        assert_eq!(
            peer_scheduler.sleepy_entry_for(&peer_proxy).unwrap().1,
            peer_before
        );
        observe_flammable(&peer, |flammable| {
            assert_eq!(flammable.status, FlammabilityStatus::Normal)
        });
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
