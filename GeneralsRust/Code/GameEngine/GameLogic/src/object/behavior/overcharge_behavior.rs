//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/OverchargeBehavior.cpp`.
//!
//! OverchargeBehavior - Rust conversion of C++ OverchargeBehavior class
//!
//! Objects with this behavior module get the ability to produce more power
//! for a short amount of time. During this "overcharge" state, object health
//! is slowly reduced. The behavior includes safety mechanisms to prevent
//! overcharging when health is too low.
//!
//! Author: Colin Day, June 2002 (C++ version)
//! Rust conversion: 2026

use std::any::Any;
use std::sync::{Arc, RwLock};

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::AsciiString;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData, OverchargeControlInterface, Thing as ModuleThing,
};

use crate::common::{
    Bool, NameKeyType, ObjectID, Real, UnsignedInt, XferVersion, LOGICFRAMES_PER_SECOND,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{TheGameLogic, TheInGameUI, TheRadar};
use crate::modules::{
    BehaviorModuleInterface, DamageModuleInterface, PowerPlantUpdateInterface,
    UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{
    xfer_update_module_base_state, OverchargeBehaviorInterface,
};
use crate::object::Object;
use crate::player::{player_list, Player};

const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;
const RADAR_EVENT_LIFETIME: Real = 1.0;

/// Configuration data for OverchargeBehavior
#[derive(Debug, Clone)]
pub struct OverchargeBehaviorModuleData {
    module_tag_name_key: NameKeyType,
    /// When active, this much health is drained per second (as percentage)
    pub health_percent_to_drain_per_second: Real,
    /// You cannot overcharge when object is below this health percentage
    pub not_allowed_when_health_below_percent: Real,
}

impl Default for OverchargeBehaviorModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            health_percent_to_drain_per_second: 0.0,
            not_allowed_when_health_below_percent: 0.0,
        }
    }
}

impl OverchargeBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, OVERCHARGE_BEHAVIOR_FIELDS)
    }
}

impl Snapshotable for OverchargeBehaviorModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        let mut health_percent_to_drain_per_second = self.health_percent_to_drain_per_second;
        xfer.xfer_real(&mut health_percent_to_drain_per_second)
            .map_err(|e| e.to_string())?;
        let mut not_allowed_when_health_below_percent = self.not_allowed_when_health_below_percent;
        xfer.xfer_real(&mut not_allowed_when_health_below_percent)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.health_percent_to_drain_per_second)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.not_allowed_when_health_below_percent)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(OverchargeBehaviorModuleData, module_tag_name_key);

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_percent_field(
    _ini: &mut INI,
    setter: &mut dyn FnMut(Real),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = first_value_token(tokens)?;
    setter(INI::parse_percent_to_real(token)?);
    Ok(())
}

const OVERCHARGE_BEHAVIOR_FIELDS: &[FieldParse<OverchargeBehaviorModuleData>] = &[
    FieldParse {
        token: "HealthPercentToDrainPerSecond",
        parse: |ini, data, tokens| {
            parse_percent_field(
                ini,
                &mut |value| data.health_percent_to_drain_per_second = value,
                tokens,
            )
        },
    },
    FieldParse {
        token: "NotAllowedWhenHealthBelowPercent",
        parse: |ini, data, tokens| {
            parse_percent_field(
                ini,
                &mut |value| data.not_allowed_when_health_below_percent = value,
                tokens,
            )
        },
    },
];

/// OverchargeBehavior - Handles power plant overcharge functionality
pub struct OverchargeBehavior {
    object_id: ObjectID,
    module_data: Arc<OverchargeBehaviorModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    overcharge_active: Bool,
}

impl OverchargeBehavior {
    pub fn new(object_id: ObjectID, module_data: Arc<OverchargeBehaviorModuleData>) -> Self {
        // start off sleeping forever until we become active (matches C++)
        if object_id != 0 {
            TheGameLogic::set_wake_frame(object_id, UPDATE_SLEEP_FOREVER);
        }
        Self {
            object_id,
            module_data,
            next_call_frame_and_phase: 0,
            overcharge_active: false,
        }
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<OverchargeBehaviorModuleData>,
    ) -> Self {
        let object_id = thing
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or(0);
        Self::new(object_id, module_data)
    }

    fn get_object_id(&self) -> ObjectID {
        self.object_id
    }

    fn with_object<R>(&self, f: impl FnOnce(&Object) -> R) -> Option<R> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object(id, f)
    }

    fn with_object_mut<R>(&self, f: impl FnOnce(&mut Object) -> R) -> Option<R> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::object::registry::OBJECT_REGISTRY.with_object_mut(id, f)
    }

    fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        let id = self.get_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }

    pub fn is_overcharge_active(&self) -> Bool {
        self.overcharge_active
    }

    fn set_rod_state(&self, extend: Bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = self.with_object(|obj_guard| {
            obj_guard.with_power_plant_update_interface(|pp| {
                pp.extend_rods(extend);
            })
        });
        Ok(())
    }

    fn add_power_bonus(&self) {
        let Some((player, object_id)) = self
            .with_object(|obj_guard| {
                obj_guard
                    .get_controlling_player()
                    .map(|player| (player, obj_guard.get_id()))
            })
            .flatten()
        else {
            return;
        };
        if let Ok(mut player_guard) = player.write() {
            player_guard.add_power_bonus(object_id);
        };
    }

    fn remove_power_bonus(&self) {
        let Some((player, object_id)) = self
            .with_object(|obj_guard| {
                obj_guard
                    .get_controlling_player()
                    .map(|player| (player, obj_guard.get_id()))
            })
            .flatten()
        else {
            return;
        };
        if let Ok(mut player_guard) = player.write() {
            player_guard.remove_power_bonus(object_id);
        };
    }

    pub fn on_delete(&mut self) {
        if !self.overcharge_active {
            return;
        }
        self.remove_power_bonus();
        self.overcharge_active = false;
    }

    pub fn on_capture(
        &mut self,
        old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) {
        if !self.overcharge_active {
            return;
        }
        if self
            .with_object(|guard| guard.is_disabled())
            .unwrap_or(true)
        {
            return;
        }
        if let Some(old_player) = old_owner {
            if let Ok(mut player_guard) = old_player.write() {
                player_guard.remove_power_bonus(self.object_id);
            }
        }
        if let Some(new_player) = new_owner {
            if let Ok(mut player_guard) = new_player.write() {
                player_guard.add_power_bonus(self.object_id);
            }
        }
    }

    pub fn load_post_process(&mut self) {
        if self.overcharge_active {
            self.add_power_bonus();
        }
    }
}

impl UpdateModuleInterface for OverchargeBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        if !self.overcharge_active {
            return Ok(UPDATE_SLEEP_NONE);
        }

        let Some(max_health) = self
            .with_object(|obj_read| {
                obj_read.get_body_module().and_then(|body| {
                    body.lock()
                        .ok()
                        .map(|body_guard| body_guard.get_max_health())
                })
            })
            .flatten()
        else {
            return Ok(UPDATE_SLEEP_NONE);
        };

        let drain_amount = (max_health * self.module_data.health_percent_to_drain_per_second)
            / LOGICFRAMES_PER_SECOND as Real;

        if drain_amount > 0.0 {
            let _ = self.with_object_mut(|obj_write| {
                let mut damage_info = DamageInfo::with_simple(
                    drain_amount,
                    obj_write.get_id(),
                    DamageType::Penalty,
                    DeathType::Normal,
                );
                damage_info.sync_from_input();
                let _ = obj_write.attempt_damage(&mut damage_info);
            });
        }

        let Some(current_health) = self
            .with_object(|obj_read| {
                obj_read
                    .get_body_module()
                    .and_then(|body| body.lock().ok().map(|body_guard| body_guard.get_health()))
            })
            .flatten()
        else {
            return Ok(UPDATE_SLEEP_NONE);
        };

        let min_health_threshold =
            max_health * self.module_data.not_allowed_when_health_below_percent;
        if current_health < min_health_threshold {
            self.enable(false)?;

            let controlling_player = self
                .with_object(|guard| guard.get_controlling_player())
                .flatten();
            let local_player = player_list()
                .read()
                .ok()
                .and_then(|list| list.get_local_player().cloned());

            if let (Some(owner), Some(local)) = (controlling_player, local_player) {
                if Arc::ptr_eq(&owner, &local) {
                    TheInGameUI::display_message("GUI:OverchargeExhausted");
                    if let Some(radar) = TheRadar::get() {
                        if let Some(pos) = self.with_object(|obj_guard| *obj_guard.get_position()) {
                            radar.create_event(
                                &pos,
                                game_engine::common::system::radar::RadarEventType::Information,
                                RADAR_EVENT_LIFETIME,
                            );
                        }
                    }
                }
            }
        }

        Ok(UPDATE_SLEEP_NONE)
    }
}

impl DamageModuleInterface for OverchargeBehavior {
    fn on_damage(
        &mut self,
        _damage_info: &mut DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn receive_damage(&mut self, _object_id: ObjectID, _damage: &DamageInfo) -> Real {
        0.0
    }
}

impl OverchargeBehaviorInterface for OverchargeBehavior {
    fn toggle(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.enable(!self.overcharge_active)
    }

    fn enable(&mut self, enable: Bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !enable {
            if self.overcharge_active {
                self.set_rod_state(false)?;
                self.remove_power_bonus();
                self.overcharge_active = false;
                if self.object_id != 0 {
                    TheGameLogic::set_wake_frame(self.object_id, UPDATE_SLEEP_FOREVER);
                }
            }
        } else if !self.overcharge_active {
            self.set_rod_state(true)?;
            self.add_power_bonus();
            self.overcharge_active = true;
            if self.object_id != 0 {
                TheGameLogic::set_wake_frame(self.object_id, UPDATE_SLEEP_NONE);
            }
        }
        Ok(())
    }

    fn is_overcharge_active(&self) -> Bool {
        self.overcharge_active
    }
}

impl BehaviorModuleInterface for OverchargeBehavior {
    fn get_module_name(&self) -> &'static str {
        "OverchargeBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_damage(&mut self) -> Option<&mut dyn DamageModuleInterface> {
        Some(self)
    }

    fn get_overcharge_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn OverchargeBehaviorInterface> {
        Some(self)
    }

    fn on_capture(
        &mut self,
        old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) {
        self.on_capture(old_owner, new_owner);
    }
}

impl Snapshotable for OverchargeBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("xfer version failed: {e:?}"))?;
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)
            .map_err(|e| format!("xfer update module base state failed: {e}"))?;
        let mut overcharge_active = self.overcharge_active;
        xfer.xfer_bool(&mut overcharge_active)
            .map_err(|e| format!("xfer overcharge_active failed: {e:?}"))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("xfer version failed: {e:?}"))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|e| format!("xfer update module base state failed: {e}"))?;
        xfer.xfer_bool(&mut self.overcharge_active)
            .map_err(|e| format!("xfer overcharge_active failed: {e:?}"))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.load_post_process();
        Ok(())
    }
}

/// Module wrapper for engine registry
pub struct OverchargeBehaviorModule {
    behavior: OverchargeBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<OverchargeBehaviorModuleData>,
}

impl OverchargeBehaviorModule {
    pub fn new(
        behavior: OverchargeBehavior,
        module_name: &AsciiString,
        module_data: Arc<OverchargeBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &OverchargeBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut OverchargeBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for OverchargeBehaviorModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process();
        Ok(())
    }
}

impl Module for OverchargeBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {}

    fn on_delete(&mut self) {
        self.behavior.on_delete();
    }

    fn get_overcharge_control_interface(&mut self) -> Option<&mut dyn OverchargeControlInterface> {
        Some(self)
    }
}

impl OverchargeControlInterface for OverchargeBehaviorModule {
    fn toggle(&mut self) -> Result<(), String> {
        OverchargeBehaviorInterface::toggle(&mut self.behavior).map_err(|err| err.to_string())
    }
}
