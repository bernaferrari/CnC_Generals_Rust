//! PoisonedBehavior - Rust conversion of C++ PoisonedBehavior.
//!
//! Reacts to poison damage by applying periodic unresistable damage until the
//! poison duration expires, preserving the C++ timing and save/load state.

use crate::common::{
    xfer::XferExt, AsciiString, DisabledMaskType, ModuleData, ObjectID, Real, TheGameLogic,
    UnsignedInt, XferVersion, INVALID_ID,
};
use crate::damage::{BodyDamageType, DamageInfo, DamageType, DeathType};
use crate::modules::{
    BehaviorModuleInterface, DamageModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::drawable::TintStatus;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module as EngineModule, ModuleData as EngineModuleData, NameKeyType,
};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct PoisonedBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub poison_damage_interval: UnsignedInt,
    pub poison_duration: UnsignedInt,
}

impl Default for PoisonedBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            poison_damage_interval: 0,
            poison_duration: 0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(PoisonedBehaviorModuleData, base);

impl PoisonedBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, POISONED_BEHAVIOR_FIELDS)
    }
}

fn parse_poison_damage_interval(
    _ini: &mut INI,
    data: &mut PoisonedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.poison_damage_interval = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_poison_duration(
    _ini: &mut INI,
    data: &mut PoisonedBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.poison_duration = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

const POISONED_BEHAVIOR_FIELDS: &[FieldParse<PoisonedBehaviorModuleData>] = &[
    FieldParse {
        token: "PoisonDamageInterval",
        parse: parse_poison_damage_interval,
    },
    FieldParse {
        token: "PoisonDuration",
        parse: parse_poison_duration,
    },
];

pub struct PoisonedBehavior {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<PoisonedBehaviorModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    poison_damage_frame: UnsignedInt,
    poison_overall_stop_frame: UnsignedInt,
    poison_damage_amount: Real,
    death_type: DeathType,
}

impl PoisonedBehavior {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<PoisonedBehaviorModuleData>,
    ) -> Self {
        Self {
            object: Arc::downgrade(&object),
            module_data,
            next_call_frame_and_phase: 0,
            poison_damage_frame: 0,
            poison_overall_stop_frame: 0,
            poison_damage_amount: 0.0,
            death_type: DeathType::Poisoned,
        }
    }

    fn set_poison_tint(&self, enabled: bool) {
        let Some(object) = self.object.upgrade() else {
            return;
        };
        let Ok(object) = object.read() else {
            return;
        };
        let Some(drawable) = object.get_drawable() else {
            return;
        };
        let Ok(mut drawable) = drawable.write() else {
            return;
        };
        if enabled {
            drawable.set_tint_status(TintStatus::POISONED);
        } else {
            drawable.clear_tint_status(TintStatus::POISONED);
        }
    }

    fn start_poisoned_effects(&mut self, damage_info: &DamageInfo) {
        let now = TheGameLogic::get_frame();
        self.poison_damage_amount = damage_info.output.actual_damage_dealt;
        self.poison_overall_stop_frame = now + self.module_data.poison_duration;

        let next_damage = now + self.module_data.poison_damage_interval;
        if self.poison_damage_frame != 0 {
            self.poison_damage_frame = self.poison_damage_frame.min(next_damage);
        } else {
            self.poison_damage_frame = next_damage;
        }

        self.death_type = damage_info.input.death_type;
        self.set_poison_tint(true);
    }

    fn stop_poisoned_effects(&mut self) {
        self.poison_damage_frame = 0;
        self.poison_overall_stop_frame = 0;
        self.poison_damage_amount = 0.0;
        self.set_poison_tint(false);
    }

    fn calc_sleep_time(&self, now: UnsignedInt) -> UpdateSleepTime {
        if self.poison_overall_stop_frame == 0 || self.poison_overall_stop_frame == now {
            return UpdateSleepTime::Forever;
        }

        let next_damage = frame_delta(now, self.poison_damage_frame);
        let stop = frame_delta(now, self.poison_overall_stop_frame);
        match (next_damage, stop) {
            (Some(a), Some(b)) => UpdateSleepTime::from_u32(a.min(b)),
            (Some(frames), None) | (None, Some(frames)) => UpdateSleepTime::from_u32(frames),
            (None, None) => UpdateSleepTime::None,
        }
    }
}

fn frame_delta(now: UnsignedInt, frame: UnsignedInt) -> Option<UnsignedInt> {
    if frame == 0 {
        None
    } else if frame > now {
        Some(frame - now)
    } else {
        Some(0)
    }
}

impl UpdateModuleInterface for PoisonedBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let now = TheGameLogic::get_frame();
        if self.poison_overall_stop_frame == 0 {
            return Ok(UpdateSleepTime::Forever);
        }

        if self.poison_damage_frame != 0 && now >= self.poison_damage_frame {
            let mut damage = DamageInfo::with_simple(
                self.poison_damage_amount,
                INVALID_ID,
                DamageType::Unresistable,
                self.death_type,
            );
            damage.input.damage_fx_override = DamageType::Poison;

            if let Some(object) = self.object.upgrade() {
                if let Ok(mut object) = object.write() {
                    object.attempt_damage(&mut damage)?;
                }
            }

            self.poison_damage_frame = now + self.module_data.poison_damage_interval;
        }

        if self.poison_overall_stop_frame != 0 && now >= self.poison_overall_stop_frame {
            let should_stop = self
                .object
                .upgrade()
                .and_then(|object| {
                    object
                        .read()
                        .ok()
                        .map(|object| !object.is_effectively_dead())
                })
                .unwrap_or(true);
            if should_stop {
                self.stop_poisoned_effects();
            }
        }

        Ok(self.calc_sleep_time(now))
    }

    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::all()
    }
}

impl DamageModuleInterface for PoisonedBehavior {
    fn receive_damage(&mut self, _object_id: ObjectID, _damage: &DamageInfo) -> Real {
        0.0
    }

    fn on_damage(
        &mut self,
        damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if damage_info.input.damage_type == DamageType::Poison {
            self.start_poisoned_effects(damage_info);
        }
        Ok(())
    }

    fn on_healing(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.stop_poisoned_effects();
        Ok(())
    }

    fn on_body_damage_state_change(
        &mut self,
        _damage_info: &DamageInfo,
        _old_state: BodyDamageType,
        _new_state: BodyDamageType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

impl BehaviorModuleInterface for PoisonedBehavior {
    fn get_module_name(&self) -> &'static str {
        "PoisonedBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for PoisonedBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|err| err.to_string())?;
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)?;
        let mut poison_damage_frame = self.poison_damage_frame;
        xfer.xfer_unsigned_int(&mut poison_damage_frame)
            .map_err(|err| err.to_string())?;
        let mut poison_overall_stop_frame = self.poison_overall_stop_frame;
        xfer.xfer_unsigned_int(&mut poison_overall_stop_frame)
            .map_err(|err| err.to_string())?;
        let mut poison_damage_amount = self.poison_damage_amount;
        xfer.xfer_real(&mut poison_damage_amount)
            .map_err(|err| err.to_string())?;
        if version >= 2 {
            let mut death_type = self.death_type as u32;
            xfer.xfer_unsigned_int(&mut death_type)
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|err| err.to_string())?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_unsigned_int(&mut self.poison_damage_frame)
            .map_err(|err| err.to_string())?;
        xfer.xfer_unsigned_int(&mut self.poison_overall_stop_frame)
            .map_err(|err| err.to_string())?;
        xfer.xfer_real(&mut self.poison_damage_amount)
            .map_err(|err| err.to_string())?;
        if version >= 2 {
            let mut death_type = self.death_type as u32;
            xfer.xfer_unsigned_int(&mut death_type)
                .map_err(|err| err.to_string())?;
            self.death_type = DeathType::from_u32(death_type);
        }
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

pub struct PoisonedBehaviorModule {
    behavior: PoisonedBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<PoisonedBehaviorModuleData>,
}

impl PoisonedBehaviorModule {
    pub fn new(
        behavior: PoisonedBehavior,
        module_name: &AsciiString,
        module_data: Arc<PoisonedBehaviorModuleData>,
    ) -> Self {
        Self {
            behavior,
            module_name_key: NameKeyGenerator::name_to_key(module_name.as_str()),
            module_data,
        }
    }
}

impl EngineModule for PoisonedBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

impl Snapshotable for PoisonedBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl BehaviorModuleInterface for PoisonedBehaviorModule {
    fn get_module_name(&self) -> &'static str {
        "PoisonedBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        self.behavior.get_update()
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(&mut self.behavior)
    }
}
