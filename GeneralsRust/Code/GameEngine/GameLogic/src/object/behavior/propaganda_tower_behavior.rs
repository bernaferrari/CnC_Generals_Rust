//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/PropagandaTowerBehavior.cpp`.
//!
//! Propaganda Tower Behavior Module
//!
//! Behavior module for propaganda towers that provide area-of-effect bonuses
//! to allied units within range. Converted from PropagandaTowerBehavior.cpp/h.

use std::any::Any;
use std::sync::{Arc, RwLock, Weak};

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, DisabledMaskType, KindOf, ModuleData, ObjectID, ObjectStatusTypes, Real,
    Relationship, UnsignedInt, WeaponBonusConditionFlags, WeaponBonusConditionType,
    LOGICFRAMES_PER_SECOND,
};
use crate::effects::FXList;
use crate::helpers::{TheFXListStore, TheGameLogic, ThePartitionManager};
use crate::modules::{
    BehaviorModuleInterface, DieModuleInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object;
use crate::player::{player_list, Player};
use crate::upgrade::{center::get_upgrade_center, UpgradeTemplate, UpgradeType};
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferMode, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};

/// Configuration data for propaganda tower behavior module
#[derive(Debug, Clone)]
pub struct PropagandaTowerBehaviorModuleData {
    pub base: BehaviorModuleData,
    /// Radius of scan area
    pub scan_radius: Real,
    /// How frequently we do an update scan (in frames)
    pub scan_delay_in_frames: UnsignedInt,
    /// How much % of max health we heal per second
    pub auto_heal_percent_per_second: Real,
    /// Different percent to use for healing if upgraded
    pub upgraded_auto_heal_percent_per_second: Real,
    /// FX list to play when scan is updated
    pub pulse_fx: Option<Arc<FXList>>,
    /// Upgrade required to use the upgraded pulse FX
    pub upgrade_required: AsciiString,
    /// FX list to play for pulse when upgraded
    pub upgraded_pulse_fx: Option<Arc<FXList>>,
    /// Allow effect to affect ourselves
    pub affects_self: Bool,
}

impl Default for PropagandaTowerBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            scan_radius: 1.0,
            scan_delay_in_frames: 100,
            auto_heal_percent_per_second: 0.01,
            upgraded_auto_heal_percent_per_second: 0.02,
            pulse_fx: None,
            upgrade_required: AsciiString::from(""),
            upgraded_pulse_fx: None,
            affects_self: false,
        }
    }
}

crate::impl_behavior_module_data_via_base!(PropagandaTowerBehaviorModuleData, base);

impl PropagandaTowerBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, PROPAGANDA_TOWER_FIELDS)
    }
}

fn parse_radius(
    _ini: &mut INI,
    data: &mut PropagandaTowerBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.scan_radius = INI::parse_real(required_value(tokens)?)?;
    Ok(())
}

fn required_value<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_delay_between_updates(
    _ini: &mut INI,
    data: &mut PropagandaTowerBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.scan_delay_in_frames = INI::parse_duration_unsigned_int(required_value(tokens)?)?;
    Ok(())
}

fn parse_heal_percent_each_second(
    _ini: &mut INI,
    data: &mut PropagandaTowerBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.auto_heal_percent_per_second = INI::parse_percent_to_real(required_value(tokens)?)?;
    Ok(())
}

fn parse_upgraded_heal_percent_each_second(
    _ini: &mut INI,
    data: &mut PropagandaTowerBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.upgraded_auto_heal_percent_per_second =
        INI::parse_percent_to_real(required_value(tokens)?)?;
    Ok(())
}

fn parse_pulse_fx(
    _ini: &mut INI,
    data: &mut PropagandaTowerBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.pulse_fx = TheFXListStore::find_fx_list(required_value(tokens)?);
    Ok(())
}

fn parse_upgrade_required(
    _ini: &mut INI,
    data: &mut PropagandaTowerBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.upgrade_required = AsciiString::from(required_value(tokens)?);
    Ok(())
}

fn parse_upgraded_pulse_fx(
    _ini: &mut INI,
    data: &mut PropagandaTowerBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.upgraded_pulse_fx = TheFXListStore::find_fx_list(required_value(tokens)?);
    Ok(())
}

fn parse_affects_self(
    _ini: &mut INI,
    data: &mut PropagandaTowerBehaviorModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    data.affects_self = INI::parse_bool(required_value(tokens)?)?;
    Ok(())
}

const PROPAGANDA_TOWER_FIELDS: &[FieldParse<PropagandaTowerBehaviorModuleData>] = &[
    FieldParse {
        token: "Radius",
        parse: parse_radius,
    },
    FieldParse {
        token: "DelayBetweenUpdates",
        parse: parse_delay_between_updates,
    },
    FieldParse {
        token: "HealPercentEachSecond",
        parse: parse_heal_percent_each_second,
    },
    FieldParse {
        token: "UpgradedHealPercentEachSecond",
        parse: parse_upgraded_heal_percent_each_second,
    },
    FieldParse {
        token: "PulseFX",
        parse: parse_pulse_fx,
    },
    FieldParse {
        token: "UpgradeRequired",
        parse: parse_upgrade_required,
    },
    FieldParse {
        token: "UpgradedPulseFX",
        parse: parse_upgraded_pulse_fx,
    },
    FieldParse {
        token: "AffectsSelf",
        parse: parse_affects_self,
    },
];

/// Propaganda tower behavior module that provides area-of-effect bonuses
pub struct PropagandaTowerBehavior {
    object_id: ObjectID,
    module_data: Arc<PropagandaTowerBehaviorModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    last_scan_frame: UnsignedInt,
    inside_list: Vec<ObjectID>,
    upgrade_required: Option<Arc<UpgradeTemplate>>,
}

impl PropagandaTowerBehavior {
    pub fn new(
        object: Arc<RwLock<Object>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<PropagandaTowerBehaviorModuleData>()
            .ok_or("Invalid module data type for PropagandaTowerBehavior")?;

        let object_id = object
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or_default();
        TheGameLogic::set_wake_frame(object_id, UpdateSleepTime::None);

        Ok(Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            last_scan_frame: 0,
            inside_list: Vec::new(),
            upgrade_required: None,
        })
    }

    fn resolve_object(
        &self,
    ) -> Result<Arc<RwLock<Object>>, Box<dyn std::error::Error + Send + Sync>> {
        if self.object_id == crate::common::INVALID_ID {
            return Err("PropagandaTowerBehavior object not set".into());
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            .ok_or_else(|| "PropagandaTowerBehavior object not set".into())
    }

    fn handle_object_created(&mut self) {
        if self.module_data.upgrade_required.is_empty() {
            self.upgrade_required = None;
            return;
        }

        let upgrade_center = get_upgrade_center();
        let upgrade = upgrade_center
            .read()
            .ok()
            .and_then(|center| center.find_upgrade(self.module_data.upgrade_required.as_str()));
        self.upgrade_required = upgrade;
    }

    fn remove_all_influence(&mut self, tower: &Object) {
        for id in self.inside_list.iter().copied() {
            if let Some(obj) = TheGameLogic::find_object_by_id(id) {
                if let Ok(mut guard) = obj.write() {
                    self.effect_logic(tower, &mut guard, false);
                }
            }
        }
        self.inside_list.clear();
    }

    #[allow(unreachable_patterns)]
    fn is_upgrade_present(&self, tower: &Object) -> Bool {
        let Some(upgrade) = &self.upgrade_required else {
            return false;
        };

        match upgrade.get_upgrade_type() {
            UpgradeType::Player => {
                if let Some(player) = tower.get_controlling_player() {
                    if let Ok(player_guard) = player.read() {
                        player_guard.has_upgrade_complete(upgrade)
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            UpgradeType::Object => tower.has_upgrade(upgrade),
            _ => false,
        }
    }

    fn should_play_fx(&self, tower: &Object) -> Bool {
        let local_player = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_local_player().cloned());
        let controlling_player = tower.get_controlling_player();
        let is_local_owner = match (&controlling_player, &local_player) {
            (Some(owner), Some(local)) => Arc::ptr_eq(owner, local),
            _ => false,
        };

        if let Some(container_id) = tower.get_contained_by() {
            if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                if let Ok(container_guard) = container.read() {
                    if !is_local_owner
                        && container_guard.test_status(ObjectStatusTypes::Stealthed)
                        && !container_guard.test_status(ObjectStatusTypes::Detected)
                    {
                        return false;
                    }

                    if tower.is_kind_of(KindOf::Vehicle)
                        && !tower.is_kind_of(KindOf::PortableStructure)
                    {
                        return false;
                    }

                    if container_guard.get_contained_by().is_some() {
                        return false;
                    }
                }
            }
        }

        if !is_local_owner
            && tower.test_status(ObjectStatusTypes::Stealthed)
            && !tower.test_status(ObjectStatusTypes::Detected)
        {
            return false;
        }

        true
    }

    fn effect_logic(&self, tower: &Object, target: &mut Object, giving: Bool) {
        let effect_upgraded = self.is_upgrade_present(tower);

        if giving {
            if target.has_any_damage_weapon() {
                let flags = target.get_weapon_bonus_condition();
                if !flags.contains(WeaponBonusConditionFlags::ENTHUSIASTIC) {
                    target.set_weapon_bonus_condition(WeaponBonusConditionType::Enthusiastic);
                }
                if effect_upgraded && !flags.contains(WeaponBonusConditionFlags::SUBLIMINAL) {
                    target.set_weapon_bonus_condition(WeaponBonusConditionType::Subliminal);
                }
            }

            if let Some(body) = target.get_body_module() {
                if let Ok(body_guard) = body.lock() {
                    let health_percent = if effect_upgraded {
                        self.module_data.upgraded_auto_heal_percent_per_second
                    } else {
                        self.module_data.auto_heal_percent_per_second
                    };
                    let amount = (health_percent / LOGICFRAMES_PER_SECOND as f32)
                        * body_guard.get_max_health();
                    let _ = target.attempt_healing_from_sole_benefactor(
                        amount,
                        Some(tower),
                        self.module_data.scan_delay_in_frames,
                    );
                }
            }
        } else {
            target.clear_weapon_bonus_condition(WeaponBonusConditionType::Enthusiastic);
            target.clear_weapon_bonus_condition(WeaponBonusConditionType::Subliminal);
        }
    }

    fn should_affect(&self, tower: &Object, target: &Object) -> Bool {
        if target.is_effectively_dead() {
            return false;
        }
        if tower.is_off_map() != target.is_off_map() {
            return false;
        }
        if target.is_kind_of(KindOf::Structure) {
            return false;
        }
        matches!(tower.relationship_to(target), Relationship::Allies)
    }

    fn do_scan(&mut self, tower: &Object) {
        let upgrade_present = self.is_upgrade_present(tower);

        if self.should_play_fx(tower) {
            let fx = if upgrade_present {
                self.module_data.upgraded_pulse_fx.as_ref()
            } else {
                self.module_data.pulse_fx.as_ref()
            };
            if let Some(fx) = fx {
                if let Some(tower_arc) =
                    crate::helpers::TheGameLogic::find_object_by_id(self.object_id).or_else(|| {
                        crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id)
                    })
                {
                    let _ = fx.do_fx_obj(&tower_arc, None);
                }
            }
        }

        let Some(partition) = ThePartitionManager::get() else {
            return;
        };
        let position = *tower.get_position();
        let nearby = partition.get_objects_in_range(&position, self.module_data.scan_radius);

        let mut new_inside = Vec::new();
        for id in nearby {
            if id == self.object_id && !self.module_data.affects_self {
                continue;
            }
            let Some(obj) = TheGameLogic::find_object_by_id(id) else {
                continue;
            };
            let Ok(obj_guard) = obj.read() else {
                continue;
            };
            if !self.should_affect(tower, &obj_guard) {
                continue;
            }
            new_inside.push(id);
        }

        for id in self.inside_list.iter().copied() {
            if !new_inside.iter().any(|next_id| *next_id == id) {
                if let Some(obj) = TheGameLogic::find_object_by_id(id) {
                    if let Ok(mut guard) = obj.write() {
                        self.effect_logic(tower, &mut guard, false);
                    }
                }
            }
        }

        self.inside_list = new_inside;
    }

    fn refresh_influence(&mut self, tower: &Object) {
        let mut new_list = Vec::new();
        for id in self.inside_list.iter().copied() {
            let Some(obj) = TheGameLogic::find_object_by_id(id) else {
                continue;
            };
            let Ok(mut guard) = obj.write() else {
                continue;
            };
            if !self.should_affect(tower, &guard) {
                self.effect_logic(tower, &mut guard, false);
                continue;
            }
            self.effect_logic(tower, &mut guard, true);
            new_list.push(id);
        }
        self.inside_list = new_list;
    }
}

impl UpdateModuleInterface for PropagandaTowerBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        let tower_arc = self.resolve_object()?;
        let tower_guard = tower_arc.read().map_err(|_| "tower lock poisoned")?;

        if tower_guard
            .get_status_bits()
            .test(ObjectStatusTypes::UnderConstruction)
        {
            return Ok(UpdateSleepTime::None);
        }

        if tower_guard.test_status(ObjectStatusTypes::Sold) {
            drop(tower_guard);
            if let Ok(tower_guard) = tower_arc.read() {
                self.remove_all_influence(&tower_guard);
            }
            return Ok(UpdateSleepTime::Forever);
        }

        if tower_guard.is_effectively_dead() {
            return Ok(UpdateSleepTime::Forever);
        }

        if tower_guard.is_disabled() {
            let mut all_but_held = DisabledMaskType::all();
            all_but_held.remove(DisabledMaskType::HELD);
            if tower_guard.get_disabled_flags().intersects(all_but_held) {
                drop(tower_guard);
                if let Ok(tower_guard) = tower_arc.read() {
                    self.remove_all_influence(&tower_guard);
                }
                return Ok(UpdateSleepTime::None);
            }
        }

        if let Some(container_id) = tower_guard.get_contained_by() {
            if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                if let Ok(container_guard) = container.read() {
                    if container_guard.get_contained_by().is_some() {
                        drop(tower_guard);
                        if let Ok(tower_guard) = tower_arc.read() {
                            self.remove_all_influence(&tower_guard);
                        }
                        return Ok(UpdateSleepTime::None);
                    }
                }
            }
        }

        let current_frame = TheGameLogic::get_frame();
        if current_frame.saturating_sub(self.last_scan_frame)
            >= self.module_data.scan_delay_in_frames
        {
            self.do_scan(&tower_guard);
            self.last_scan_frame = current_frame;
        }

        self.refresh_influence(&tower_guard);
        Ok(UpdateSleepTime::None)
    }

    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::all()
    }
}

impl DieModuleInterface for PropagandaTowerBehavior {
    fn on_die(
        &mut self,
        _damage_info: &crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(tower_guard) = self.resolve_object()?.read() {
            self.remove_all_influence(&tower_guard);
        }
        Ok(())
    }
}

impl BehaviorModuleInterface for PropagandaTowerBehavior {
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_die(&mut self) -> Option<&mut dyn DieModuleInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.handle_object_created();
        Ok(())
    }

    fn on_capture(
        &mut self,
        _old_owner: Option<&Arc<RwLock<Player>>>,
        new_owner: Option<&Arc<RwLock<Player>>>,
    ) {
        let neutral_player = player_list()
            .read()
            .ok()
            .and_then(|list| list.get_neutral_player());
        let is_neutral = match (&new_owner, &neutral_player) {
            (Some(new_owner), Some(neutral)) => Arc::ptr_eq(new_owner, &neutral),
            (None, _) => true,
            _ => false,
        };

        if is_neutral {
            if let Ok(tower_arc) = self.resolve_object() {
                if let Ok(tower_guard) = tower_arc.read() {
                    self.remove_all_influence(&tower_guard);
                }
            }
            TheGameLogic::set_wake_frame(self.object_id, UpdateSleepTime::Forever);
        } else {
            TheGameLogic::set_wake_frame(self.object_id, UpdateSleepTime::None);
        }
    }
}

impl Snapshotable for PropagandaTowerBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)
            .map_err(|e| format!("Failed to xfer UpdateModule base state: {}", e))?;

        let mut last_scan_frame = self.last_scan_frame;
        xfer.xfer_unsigned_int(&mut last_scan_frame)
            .map_err(|e| e.to_string())?;

        let mut inside_count: u16 = self.inside_list.len().min(u16::MAX as usize) as u16;
        xfer.xfer_unsigned_short(&mut inside_count)
            .map_err(|e| e.to_string())?;

        for id in self.inside_list.iter().copied().take(inside_count as usize) {
            let mut id_copy = id;
            xfer.xfer_object_id(&mut id_copy)
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|e| format!("Failed to xfer UpdateModule base state: {}", e))?;

        xfer.xfer_unsigned_int(&mut self.last_scan_frame)
            .map_err(|e| e.to_string())?;

        let mut inside_count: u16 = self.inside_list.len().min(u16::MAX as usize) as u16;
        xfer.xfer_unsigned_short(&mut inside_count)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == XferMode::Save {
            for id in self.inside_list.iter().copied().take(inside_count as usize) {
                let mut id_copy = id;
                xfer.xfer_object_id(&mut id_copy)
                    .map_err(|e| e.to_string())?;
            }
        } else {
            self.inside_list.clear();
            for _ in 0..inside_count {
                let mut id: ObjectID = 0;
                xfer.xfer_object_id(&mut id).map_err(|e| e.to_string())?;
                self.inside_list.push(id);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Module wrapper for PropagandaTowerBehavior.
pub struct PropagandaTowerBehaviorModule {
    behavior: PropagandaTowerBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<PropagandaTowerBehaviorModuleData>,
}

impl PropagandaTowerBehaviorModule {
    pub fn new(
        behavior: PropagandaTowerBehavior,
        module_name: &AsciiString,
        module_data: Arc<PropagandaTowerBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut PropagandaTowerBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for PropagandaTowerBehaviorModule {
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

impl Module for PropagandaTowerBehaviorModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.behavior.handle_object_created();
    }

    fn on_delete(&mut self) {
        if let Ok(tower_arc) = self.behavior.resolve_object() {
            if let Ok(tower_guard) = tower_arc.read() {
                self.behavior.remove_all_influence(&tower_guard);
            }
        }
    }
}

// Thread safety
unsafe impl Send for PropagandaTowerBehavior {}
unsafe impl Sync for PropagandaTowerBehavior {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn propaganda_tower_fields_use_cpp_ini_token_handling() {
        let mut data = PropagandaTowerBehaviorModuleData::default();
        let mut ini = INI::new();

        parse_radius(&mut ini, &mut data, &["=", "175.5f"]).expect("radius");
        parse_delay_between_updates(&mut ini, &mut data, &["=", "2s"]).expect("delay");
        parse_heal_percent_each_second(&mut ini, &mut data, &["=", "5%"]).expect("heal percent");
        parse_upgraded_heal_percent_each_second(&mut ini, &mut data, &["=", "12.5%"])
            .expect("upgraded heal percent");
        parse_upgrade_required(&mut ini, &mut data, &["=", "Upgrade_Nationalism"])
            .expect("upgrade required");
        parse_affects_self(&mut ini, &mut data, &["=", "true"]).expect("affects self");

        assert!((data.scan_radius - 175.5).abs() < f32::EPSILON);
        assert_eq!(data.scan_delay_in_frames, 60);
        assert!((data.auto_heal_percent_per_second - 0.05).abs() < f32::EPSILON);
        assert!((data.upgraded_auto_heal_percent_per_second - 0.125).abs() < f32::EPSILON);
        assert_eq!(data.upgrade_required.as_str(), "Upgrade_Nationalism");
        assert!(data.affects_self);
    }

    #[test]
    fn propaganda_tower_rejects_missing_values_and_invalid_bool_like_cpp_parsers() {
        let mut data = PropagandaTowerBehaviorModuleData::default();
        let mut ini = INI::new();

        let err = parse_radius(&mut ini, &mut data, &["="]).expect_err("missing real");
        assert!(matches!(err, INIError::InvalidData));

        let err = parse_affects_self(&mut ini, &mut data, &["maybe"]).expect_err("invalid bool");
        assert!(matches!(err, INIError::InvalidData));
    }
}
