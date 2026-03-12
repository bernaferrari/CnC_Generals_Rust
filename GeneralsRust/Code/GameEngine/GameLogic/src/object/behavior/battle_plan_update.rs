//! BattlePlanUpdate - Handle building states and battle plan execution & changes
//! Author: Kris Morness, September 2002 (C++ version) | Rust conversion: 2025

use crate::common::{
    AsciiString, CommandSourceType, Coord3D, DisabledType, KindOfMask, ModelConditionFlag,
    ModuleData, ObjectID, SpecialPowerTemplateId, TurretType, UnsignedInt,
};
use crate::helpers::TheGameLogic;
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, SpecialPowerCommandOptions,
    SpecialPowerUpdateInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::body::body_module::MaxHealthChangeType;
use crate::object::special_power_template::SpecialPowerTemplate;
use crate::object::Object as GameObject;
use crate::player::{BattlePlanType, PlayerArcExt};
use crate::waypoint::Waypoint;
use crate::weapon::{WeaponLockType, WeaponSetType, WeaponSlotType};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattlePlanStatus {
    None,
    Bombardment,
    HoldTheLine,
    SearchAndDestroy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionStatus {
    Idle,
    Unpacking,
    Active,
    Packing,
}

#[derive(Debug, Clone)]
pub struct BattlePlanBonuses {
    pub armor_scalar: f32,
    pub sight_range_scalar: f32,
    pub bombardment: i32,
    pub search_and_destroy: i32,
    pub hold_the_line: i32,
    pub valid_kind_of: KindOfMask,
    pub invalid_kind_of: KindOfMask,
}

#[derive(Debug, Clone)]
pub struct BattlePlanUpdateModuleData {
    pub base: BehaviorModuleData,
    pub special_power_template: Option<SpecialPowerTemplateId>,
    pub bombardment_plan_animation_frames: u32,
    pub hold_the_line_plan_animation_frames: u32,
    pub search_and_destroy_plan_animation_frames: u32,
    pub transition_idle_frames: u32,
    pub battle_plan_paralyze_frames: u32,
    pub hold_the_line_armor_damage_scalar: f32,
    pub search_and_destroy_sight_range_scalar: f32,
    pub strategy_center_search_and_destroy_sight_range_scalar: f32,
    pub strategy_center_search_and_destroy_detects_stealth: bool,
    pub strategy_center_hold_the_line_max_health_scalar: f32,
    pub strategy_center_hold_the_line_max_health_change_type: MaxHealthChangeType,
    pub valid_member_kind_of: KindOfMask,
    pub invalid_member_kind_of: KindOfMask,
    pub vision_object_name: String,
    pub bombardment_unpack_name: String,
    pub bombardment_pack_name: String,
    pub bombardment_message_label: String,
    pub bombardment_announcement_name: String,
    pub search_and_destroy_unpack_name: String,
    pub search_and_destroy_idle_name: String,
    pub search_and_destroy_pack_name: String,
    pub search_and_destroy_message_label: String,
    pub search_and_destroy_announcement_name: String,
    pub hold_the_line_unpack_name: String,
    pub hold_the_line_pack_name: String,
    pub hold_the_line_message_label: String,
    pub hold_the_line_announcement_name: String,
}

impl Default for BattlePlanUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            special_power_template: None,
            bombardment_plan_animation_frames: 0,
            hold_the_line_plan_animation_frames: 0,
            search_and_destroy_plan_animation_frames: 0,
            transition_idle_frames: 0,
            battle_plan_paralyze_frames: 0,
            hold_the_line_armor_damage_scalar: 1.0,
            search_and_destroy_sight_range_scalar: 1.0,
            strategy_center_search_and_destroy_sight_range_scalar: 1.0,
            strategy_center_search_and_destroy_detects_stealth: true,
            strategy_center_hold_the_line_max_health_scalar: 1.0,
            strategy_center_hold_the_line_max_health_change_type:
                MaxHealthChangeType::PreserveRatio,
            valid_member_kind_of: 0u64,
            invalid_member_kind_of: 0u64,
            vision_object_name: String::new(),
            bombardment_unpack_name: String::new(),
            bombardment_pack_name: String::new(),
            bombardment_message_label: String::new(),
            bombardment_announcement_name: String::new(),
            search_and_destroy_unpack_name: String::new(),
            search_and_destroy_idle_name: String::new(),
            search_and_destroy_pack_name: String::new(),
            search_and_destroy_message_label: String::new(),
            search_and_destroy_announcement_name: String::new(),
            hold_the_line_unpack_name: String::new(),
            hold_the_line_pack_name: String::new(),
            hold_the_line_message_label: String::new(),
            hold_the_line_announcement_name: String::new(),
        }
    }
}

crate::impl_behavior_module_data_via_base!(BattlePlanUpdateModuleData, base);

pub struct BattlePlanUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<BattlePlanUpdateModuleData>,
    status: TransitionStatus,
    current_plan: BattlePlanStatus,
    desired_plan: BattlePlanStatus,
    plan_affecting_army: BattlePlanStatus,
    next_ready_frame: u32,
    invalid_settings: bool,
    centering_turret: bool,
    bonuses: BattlePlanBonuses,
    vision_object_id: Option<ObjectID>,
    special_power_module: Option<SpecialPowerTemplateId>,
}

impl BattlePlanUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<BattlePlanUpdateModuleData>()
            .ok_or("Invalid module data")?;

        let bonuses = BattlePlanBonuses {
            armor_scalar: 1.0,
            sight_range_scalar: 1.0,
            bombardment: 0,
            search_and_destroy: 0,
            hold_the_line: 0,
            valid_kind_of: specific_data.valid_member_kind_of,
            invalid_kind_of: specific_data.invalid_member_kind_of,
        };

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            status: TransitionStatus::Idle,
            current_plan: BattlePlanStatus::None,
            desired_plan: BattlePlanStatus::None,
            plan_affecting_army: BattlePlanStatus::None,
            next_ready_frame: 0,
            invalid_settings: false,
            centering_turret: false,
            bonuses,
            vision_object_id: None,
            special_power_module: None,
        })
    }

    fn object_arc(&self) -> Option<Arc<RwLock<GameObject>>> {
        self.object.upgrade()
    }

    fn with_object<R>(&self, func: impl FnOnce(&GameObject) -> R) -> Option<R> {
        let obj = self.object_arc()?;
        let guard = obj.read().ok()?;
        Some(func(&*guard))
    }

    fn with_object_mut<R>(&self, func: impl FnOnce(&mut GameObject) -> R) -> Option<R> {
        let obj = self.object_arc()?;
        let mut guard = obj.write().ok()?;
        Some(func(&mut *guard))
    }

    fn enable_turret(&self, enable: bool) {
        self.with_object(|object| {
            if let Some(ai) = object.get_ai() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let turret = ai_guard.get_which_turret_for_cur_weapon();
                    if turret != TurretType::Invalid {
                        ai_guard.set_turret_enabled(turret, enable);
                    }
                }
            }
        });
    }

    fn recenter_turret(&self) {
        self.with_object(|object| {
            if let Some(ai) = object.get_ai() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let turret = ai_guard.get_which_turret_for_cur_weapon();
                    if turret != TurretType::Invalid {
                        ai_guard.recenter_turret(turret);
                    }
                }
            }
        });
    }

    fn is_turret_in_natural_position(&self) -> bool {
        self.with_object(|object| {
            if let Some(ai) = object.get_ai() {
                if let Ok(ai_guard) = ai.lock() {
                    let turret = ai_guard.get_which_turret_for_cur_weapon();
                    if turret != TurretType::Invalid {
                        return ai_guard.is_turret_in_natural_position(turret);
                    }
                }
            }
            false
        })
        .unwrap_or(false)
    }

    fn set_status(&mut self, new_status: TransitionStatus) {
        if self.status == new_status {
            return;
        }

        let old_status = self.status;
        self.clear_old_status_states(old_status);

        let now = TheGameLogic::get_frame();
        match new_status {
            TransitionStatus::Idle => {
                self.current_plan = BattlePlanStatus::None;
                self.next_ready_frame = now + self.module_data.transition_idle_frames;
            }
            TransitionStatus::Unpacking => {
                self.apply_unpacking_states(now);
            }
            TransitionStatus::Active => {
                self.set_battle_plan(self.current_plan);
                self.apply_active_states();
            }
            TransitionStatus::Packing => {
                self.set_battle_plan(BattlePlanStatus::None);
                self.apply_packing_states(now);
            }
        }

        self.status = new_status;
    }

    fn clear_old_status_states(&self, old_status: TransitionStatus) {
        self.with_object_mut(|object| match old_status {
            TransitionStatus::Unpacking => match self.current_plan {
                BattlePlanStatus::Bombardment => {
                    object.clear_model_condition_state(ModelConditionFlag::Door1Opening);
                }
                BattlePlanStatus::HoldTheLine => {
                    object.clear_model_condition_state(ModelConditionFlag::Door2Opening);
                }
                BattlePlanStatus::SearchAndDestroy => {
                    object.clear_model_condition_state(ModelConditionFlag::Door3Opening);
                }
                _ => {}
            },
            TransitionStatus::Active => match self.current_plan {
                BattlePlanStatus::Bombardment => {
                    object.clear_model_condition_state(ModelConditionFlag::Door1WaitingToClose);
                }
                BattlePlanStatus::HoldTheLine => {
                    object.clear_model_condition_state(ModelConditionFlag::Door2WaitingToClose);
                }
                BattlePlanStatus::SearchAndDestroy => {
                    object.clear_model_condition_state(ModelConditionFlag::Door3WaitingToClose);
                }
                _ => {}
            },
            TransitionStatus::Packing => match self.current_plan {
                BattlePlanStatus::Bombardment => {
                    object.clear_model_condition_state(ModelConditionFlag::Door1Closing);
                }
                BattlePlanStatus::HoldTheLine => {
                    object.clear_model_condition_state(ModelConditionFlag::Door2Closing);
                }
                BattlePlanStatus::SearchAndDestroy => {
                    object.clear_model_condition_state(ModelConditionFlag::Door3Closing);
                }
                _ => {}
            },
            _ => {}
        });
    }

    fn apply_unpacking_states(&mut self, now: u32) {
        let mut next_ready_frame = self.next_ready_frame;
        self.with_object_mut(|object| match self.current_plan {
            BattlePlanStatus::Bombardment => {
                object.set_model_condition_state(ModelConditionFlag::Door1Opening);
                next_ready_frame = now + self.module_data.bombardment_plan_animation_frames;
            }
            BattlePlanStatus::HoldTheLine => {
                object.set_model_condition_state(ModelConditionFlag::Door2Opening);
                next_ready_frame = now + self.module_data.hold_the_line_plan_animation_frames;
            }
            BattlePlanStatus::SearchAndDestroy => {
                object.set_model_condition_state(ModelConditionFlag::Door3Opening);
                next_ready_frame = now + self.module_data.search_and_destroy_plan_animation_frames;
            }
            _ => {}
        });
        self.next_ready_frame = next_ready_frame;
    }

    fn apply_active_states(&self) {
        self.with_object_mut(|object| match self.current_plan {
            BattlePlanStatus::Bombardment => {
                object.set_model_condition_state(ModelConditionFlag::Door1WaitingToClose);
            }
            BattlePlanStatus::HoldTheLine => {
                object.set_model_condition_state(ModelConditionFlag::Door2WaitingToClose);
            }
            BattlePlanStatus::SearchAndDestroy => {
                object.set_model_condition_state(ModelConditionFlag::Door3WaitingToClose);
            }
            _ => {}
        });
    }

    fn apply_packing_states(&mut self, now: u32) {
        let mut next_ready_frame = self.next_ready_frame;
        self.with_object_mut(|object| match self.current_plan {
            BattlePlanStatus::Bombardment => {
                object.set_model_condition_state(ModelConditionFlag::Door1Closing);
                next_ready_frame = now + self.module_data.bombardment_plan_animation_frames;
            }
            BattlePlanStatus::HoldTheLine => {
                object.set_model_condition_state(ModelConditionFlag::Door2Closing);
                next_ready_frame = now + self.module_data.hold_the_line_plan_animation_frames;
            }
            BattlePlanStatus::SearchAndDestroy => {
                object.set_model_condition_state(ModelConditionFlag::Door3Closing);
                next_ready_frame = now + self.module_data.search_and_destroy_plan_animation_frames;
            }
            _ => {}
        });
        self.next_ready_frame = next_ready_frame;
    }

    fn set_battle_plan(&mut self, plan: BattlePlanStatus) {
        let Some(player) = self.with_object(|object| object.get_controlling_player()) else {
            return;
        };
        let Some(player) = player else {
            return;
        };

        // Remove old plan bonuses.
        if self.plan_affecting_army != BattlePlanStatus::None {
            let plan_type = match self.plan_affecting_army {
                BattlePlanStatus::Bombardment => BattlePlanType::Bombard,
                BattlePlanStatus::HoldTheLine => BattlePlanType::HoldTheLine,
                BattlePlanStatus::SearchAndDestroy => BattlePlanType::SearchAndDestroy,
                BattlePlanStatus::None => unreachable!(),
            };
            player.change_battle_plan(plan_type, -1, &self.bonuses);
            self.remove_building_bonuses();
        }

        self.bonuses.armor_scalar = 1.0;
        self.bonuses.sight_range_scalar = 1.0;
        self.bonuses.bombardment = 0;
        self.bonuses.search_and_destroy = 0;
        self.bonuses.hold_the_line = 0;

        match plan {
            BattlePlanStatus::None => {
                let now = TheGameLogic::get_frame();
                let _ = player.iterate_objects(|obj| {
                    if let Ok(mut guard) = obj.write() {
                        let kind_of = guard.get_kind_of();
                        if (kind_of & self.module_data.valid_member_kind_of) != 0
                            && (kind_of & self.module_data.invalid_member_kind_of) == 0
                        {
                            guard.set_disabled_until(
                                DisabledType::Paralyzed,
                                now + self.module_data.battle_plan_paralyze_frames,
                            );
                        }
                    }
                    Ok(())
                });
            }
            BattlePlanStatus::Bombardment => {
                self.bonuses.bombardment = 1;
                player.change_battle_plan(BattlePlanType::Bombard, 1, &self.bonuses);
            }
            BattlePlanStatus::HoldTheLine => {
                self.apply_hold_the_line_bonuses();
                self.bonuses.armor_scalar = self.module_data.hold_the_line_armor_damage_scalar;
                self.bonuses.hold_the_line = 1;
                player.change_battle_plan(BattlePlanType::HoldTheLine, 1, &self.bonuses);
            }
            BattlePlanStatus::SearchAndDestroy => {
                self.apply_search_and_destroy_bonuses();
                self.bonuses.search_and_destroy = 1;
                self.bonuses.sight_range_scalar =
                    self.module_data.search_and_destroy_sight_range_scalar;
                player.change_battle_plan(BattlePlanType::SearchAndDestroy, 1, &self.bonuses);
            }
        }

        self.plan_affecting_army = plan;
    }

    fn apply_hold_the_line_bonuses(&self) {
        if self
            .module_data
            .strategy_center_hold_the_line_max_health_scalar
            == 1.0
        {
            return;
        }

        self.with_object(|object| {
            if let Some(body) = object.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    let current_max = body_guard.get_max_health();
                    let new_max = current_max
                        * self
                            .module_data
                            .strategy_center_hold_the_line_max_health_scalar;
                    if let Err(_err) = body_guard.set_max_health(
                        new_max,
                        self.module_data
                            .strategy_center_hold_the_line_max_health_change_type,
                    ) {
                        // Keep C++ flow (best-effort bonus application) even if body update fails.
                    }
                }
            }
        });
    }

    fn apply_search_and_destroy_bonuses(&self) {
        self.with_object_mut(|object| {
            if self.module_data.strategy_center_search_and_destroy_sight_range_scalar != 1.0 {
                let scalar = self.module_data.strategy_center_search_and_destroy_sight_range_scalar;
                object.set_vision_range(object.get_vision_range() * scalar);
                object.set_shroud_clearing_range(object.get_shroud_clearing_range() * scalar);
            }

            if self.module_data.strategy_center_search_and_destroy_detects_stealth {
                if let Some(stealth_detector) = object.find_update_module("StealthDetectorUpdate") {
                    let _ = stealth_detector.with_module_downcast::<
                        crate::object::behavior::stealth_detector_update::StealthDetectorUpdateModule,
                        _,
                        _,
                    >(|module| {
                        module.behavior_mut().set_sd_enabled(true);
                    });
                }
            }
        });
    }

    fn remove_building_bonuses(&self) {
        self.with_object_mut(|object| match self.plan_affecting_army {
            BattlePlanStatus::HoldTheLine => {
                if self.module_data.strategy_center_hold_the_line_max_health_scalar != 1.0 {
                    if let Some(body) = object.get_body_module() {
                        if let Ok(mut body_guard) = body.lock() {
                            let current_max = body_guard.get_max_health();
                            let new_max = current_max
                                / self.module_data.strategy_center_hold_the_line_max_health_scalar;
                            if let Err(_err) = body_guard.set_max_health(
                                new_max,
                                self.module_data
                                    .strategy_center_hold_the_line_max_health_change_type,
                            ) {
                                // Keep C++ flow (best-effort bonus removal) even if body update fails.
                            }
                        }
                    }
                }
            }
            BattlePlanStatus::SearchAndDestroy => {
                if self.module_data.strategy_center_search_and_destroy_sight_range_scalar != 1.0 {
                    let scalar = self.module_data.strategy_center_search_and_destroy_sight_range_scalar;
                    object.set_vision_range(object.get_vision_range() / scalar);
                    object.set_shroud_clearing_range(object.get_shroud_clearing_range() / scalar);
                }

                if self.module_data.strategy_center_search_and_destroy_detects_stealth {
                    if let Some(stealth_detector) = object.find_update_module("StealthDetectorUpdate") {
                        let _ = stealth_detector.with_module_downcast::<
                            crate::object::behavior::stealth_detector_update::StealthDetectorUpdateModule,
                            _,
                            _,
                        >(|module| {
                            module.behavior_mut().set_sd_enabled(false);
                        });
                    }
                }
            }
            _ => {}
        });
    }

    pub fn get_active_battle_plan(&self) -> BattlePlanStatus {
        if self.status == TransitionStatus::Active {
            self.plan_affecting_army
        } else {
            BattlePlanStatus::None
        }
    }
}

impl UpdateModuleInterface for BattlePlanUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if self.invalid_settings {
            return UpdateSleepTime::None;
        }

        if self.object_arc().is_none() {
            return UpdateSleepTime::Forever;
        }

        let now = TheGameLogic::get_frame();

        if self.next_ready_frame <= now {
            match self.status {
                TransitionStatus::Idle => {
                    if self.desired_plan != BattlePlanStatus::None {
                        self.current_plan = self.desired_plan;
                        self.set_status(TransitionStatus::Unpacking);
                    }
                }
                TransitionStatus::Unpacking => {
                    self.set_status(TransitionStatus::Active);
                    if self.current_plan == BattlePlanStatus::Bombardment {
                        self.enable_turret(true);
                    }
                }
                TransitionStatus::Active => {
                    if self.current_plan != self.desired_plan {
                        if self.current_plan == BattlePlanStatus::Bombardment {
                            let should_pack = self.with_object(|object| {
                                if let Some(ai) = object.get_ai() {
                                    if self.is_turret_in_natural_position() {
                                        true
                                    } else if !self.centering_turret {
                                        ai.ai_idle(CommandSourceType::FromAI);
                                        false
                                    } else {
                                        false
                                    }
                                } else {
                                    true
                                }
                            });

                            if should_pack.unwrap_or(true) {
                                self.set_status(TransitionStatus::Packing);
                                self.centering_turret = false;
                                self.enable_turret(false);
                            } else if !self.centering_turret {
                                self.recenter_turret();
                                self.centering_turret = true;
                            }
                        } else {
                            self.set_status(TransitionStatus::Packing);
                        }
                    }
                }
                TransitionStatus::Packing => {
                    self.set_status(TransitionStatus::Idle);
                }
            }
        }

        UpdateSleepTime::None
    }
}

impl SpecialPowerUpdateInterface for BattlePlanUpdate {
    fn initiate_intent_to_do_special_power(
        &mut self,
        special_power_template: &SpecialPowerTemplate,
        _target_obj: Option<ObjectID>,
        _target_pos: Option<&Coord3D>,
        _waypoint: Option<&Waypoint>,
        command_options: SpecialPowerCommandOptions,
    ) -> bool {
        let Some(expected_template) = self.module_data.special_power_template else {
            return false;
        };
        if special_power_template.get_id() != expected_template {
            return false;
        }

        if command_options.contains(SpecialPowerCommandOptions::OPTION_ONE) {
            self.desired_plan = BattlePlanStatus::Bombardment;
        } else if command_options.contains(SpecialPowerCommandOptions::OPTION_TWO) {
            self.desired_plan = BattlePlanStatus::HoldTheLine;
        } else if command_options.contains(SpecialPowerCommandOptions::OPTION_THREE) {
            self.desired_plan = BattlePlanStatus::SearchAndDestroy;
        } else {
            return false;
        }

        true
    }

    fn is_special_ability(&self) -> bool {
        false
    }

    fn is_special_power(&self) -> bool {
        true
    }

    fn is_active(&self) -> bool {
        self.get_active_battle_plan() != BattlePlanStatus::None
    }

    fn get_command_option(&self) -> crate::modules::SpecialPowerCommandOption {
        crate::modules::SpecialPowerCommandOptions::NONE
    }

    fn does_special_power_have_overridable_destination_active(&self) -> bool {
        false
    }

    fn does_special_power_have_overridable_destination(&self) -> bool {
        false
    }

    fn set_special_power_overridable_destination(&mut self, _location: &Coord3D) {}

    fn is_power_currently_in_use(
        &self,
        _command: Option<&crate::command_button::CommandButton>,
    ) -> bool {
        self.is_active()
    }
}

impl BehaviorModuleInterface for BattlePlanUpdate {
    fn get_module_name(&self) -> &'static str {
        "BattlePlanUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_special_power_update_interface(
        &mut self,
    ) -> Option<&mut dyn SpecialPowerUpdateInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.module_data.special_power_template.is_none() {
            self.invalid_settings = true;
            return Ok(());
        }

        let mut special_power_module = None;
        self.with_object_mut(|object| {
            if let Some(template_id) = self.module_data.special_power_template {
                special_power_module = object.get_special_power_module(template_id);
            }

            object.set_weapon_set_flag(WeaponSetType::Veteran);

            if object.get_ai().is_some() {
                object.set_weapon_lock(WeaponSlotType::Primary, WeaponLockType::LockedTemporarily);
            }
        });
        self.special_power_module = special_power_module;

        self.enable_turret(false);
        Ok(())
    }

    fn on_die(
        &mut self,
        _damage_info: &crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(vision_id) = self.vision_object_id {
            let _ = TheGameLogic::destroy_object_by_id(vision_id);
        }

        if self.plan_affecting_army != BattlePlanStatus::None {
            let Some(player) = self.with_object(|object| object.get_controlling_player()) else {
                return Ok(());
            };
            if let Some(player) = player {
                let plan_type = match self.plan_affecting_army {
                    BattlePlanStatus::Bombardment => BattlePlanType::Bombard,
                    BattlePlanStatus::HoldTheLine => BattlePlanType::HoldTheLine,
                    BattlePlanStatus::SearchAndDestroy => BattlePlanType::SearchAndDestroy,
                    BattlePlanStatus::None => unreachable!(),
                };
                player.change_battle_plan(plan_type, -1, &self.bonuses);
            }
        }

        Ok(())
    }
}

pub struct BattlePlanUpdateFactory;
impl BattlePlanUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(BattlePlanUpdate::new(thing, module_data)?))
    }
}

pub struct BattlePlanUpdateModule {
    behavior: BattlePlanUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<BattlePlanUpdateModuleData>,
}

impl BattlePlanUpdateModule {
    pub fn new(
        behavior: BattlePlanUpdate,
        module_name: &AsciiString,
        module_data: Arc<BattlePlanUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut BattlePlanUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for BattlePlanUpdateModule {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Module for BattlePlanUpdateModule {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

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
