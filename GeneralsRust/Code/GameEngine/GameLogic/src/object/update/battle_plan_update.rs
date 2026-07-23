// BattlePlanUpdate - Handle building states and battle plan execution & changes
// Author: Kris Morness, September 2002
// Ported to Rust

use crate::object::behavior::battle_plan_update::BattlePlanBonuses;
use crate::object::body::body_module::MaxHealthChangeType;
use crate::object::ObjectArcExt;
use crate::player::{BattlePlanType, PlayerArcExt};
use crate::prelude::*;

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
pub struct BattlePlanUpdateModuleData {
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

#[derive(Debug, Clone)]
pub struct BattlePlanUpdate {
    thing: ThingId,
    module_data: BattlePlanUpdateModuleData,
    status: TransitionStatus,
    current_plan: BattlePlanStatus,
    desired_plan: BattlePlanStatus,
    plan_affecting_army: BattlePlanStatus,
    next_ready_frame: u32,
    invalid_settings: bool,
    centering_turret: bool,
    bonuses: BattlePlanBonuses,
    vision_object_id: Option<ObjectId>,
    special_power_module: Option<SpecialPowerModuleId>,
}

impl BattlePlanUpdate {
    pub fn new(thing: ThingId, module_data: BattlePlanUpdateModuleData) -> Self {
        let bonuses = BattlePlanBonuses {
            armor_scalar: 1.0,
            sight_range_scalar: 1.0,
            bombardment: 0,
            search_and_destroy: 0,
            hold_the_line: 0,
            valid_kind_of: module_data.valid_member_kind_of,
            invalid_kind_of: module_data.invalid_member_kind_of,
        };

        Self {
            thing,
            module_data,
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
        }
    }

    pub fn on_object_created(&mut self, ctx: &mut UpdateContext<'_>) {
        if self.module_data.special_power_template.is_none() {
            self.invalid_settings = true;
            return;
        }

        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        // Find special power module
        if let Some(sp_template) = self.module_data.special_power_template {
            self.special_power_module = object.get_special_power_module(sp_template);
        }

        object.set_weapon_set_flag(crate::weapon::WeaponSetType::Veteran);

        if object.get_ai().is_some() {
            object.set_weapon_lock(
                crate::weapon::WeaponSlotType::Primary,
                crate::weapon::WeaponLockType::LockedTemporarily,
            );
        }

        self.enable_turret(false, ctx);
    }

    pub fn on_delete(&mut self, ctx: &mut UpdateContext<'_>) {
        // Delete vision object
        if let Some(vision_id) = self.vision_object_id {
            if let Some(vision_obj) = ctx.game_logic.find_object(vision_id) {
                ctx.game_logic.destroy_object(vision_obj.id());
            }
        }

        // Remove battle plan bonus
        if let Some(object) = ctx.game_logic.find_object(self.thing) {
            if let Some(player) = object.get_controlling_player() {
                if self.plan_affecting_army != BattlePlanStatus::None {
                    let plan_type = match self.plan_affecting_army {
                        BattlePlanStatus::Bombardment => BattlePlanType::Bombard,
                        BattlePlanStatus::HoldTheLine => BattlePlanType::HoldTheLine,
                        BattlePlanStatus::SearchAndDestroy => BattlePlanType::SearchAndDestroy,
                        BattlePlanStatus::None => unreachable!(),
                    };
                    player.change_battle_plan(plan_type, -1, &self.bonuses);
                }
            }
        }
    }

    pub fn initiate_intent_to_do_special_power(
        &mut self,
        special_power_template: SpecialPowerTemplateId,
        command_options: SpecialPowerCommandOptions,
    ) -> bool {
        if self.module_data.special_power_template != Some(special_power_template) {
            return false;
        }

        // Set desired plan based on command button option
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

    pub fn update(&mut self, ctx: &mut UpdateContext<'_>) -> UpdateSleepTime {
        if self.invalid_settings {
            return UpdateSleepTime::None;
        }

        let now = ctx.game_logic.get_frame();

        if self.next_ready_frame <= now {
            match self.status {
                TransitionStatus::Idle => {
                    if self.desired_plan != BattlePlanStatus::None {
                        self.current_plan = self.desired_plan;
                        self.set_status(TransitionStatus::Unpacking, ctx);
                    }
                }
                TransitionStatus::Unpacking => {
                    self.set_status(TransitionStatus::Active, ctx);
                    if self.current_plan == BattlePlanStatus::Bombardment {
                        self.enable_turret(true, ctx);
                    }
                }
                TransitionStatus::Active => {
                    if self.current_plan != self.desired_plan {
                        if self.current_plan == BattlePlanStatus::Bombardment {
                            if let Some(object) = ctx.game_logic.find_object(self.thing) {
                                if let Some(ai) = object.get_ai() {
                                    if self.is_turret_in_natural_position(ctx) {
                                        self.set_status(TransitionStatus::Packing, ctx);
                                        self.centering_turret = false;
                                        self.enable_turret(false, ctx);
                                    } else if !self.centering_turret {
                                        ai.ai_idle(CommandSource::FromAI);
                                        self.recenter_turret(ctx);
                                        self.centering_turret = true;
                                    }
                                }
                            }
                        } else {
                            self.set_status(TransitionStatus::Packing, ctx);
                        }
                    }
                }
                TransitionStatus::Packing => {
                    self.set_status(TransitionStatus::Idle, ctx);
                }
            }
        }

        UpdateSleepTime::None
    }

    fn set_status(&mut self, new_status: TransitionStatus, ctx: &mut UpdateContext<'_>) {
        if self.status == new_status {
            return;
        }

        let old_status = self.status;

        // Clear old states
        self.clear_old_status_states(old_status, ctx);

        let now = ctx.game_logic.get_frame();

        // Set new states
        match new_status {
            TransitionStatus::Idle => {
                self.current_plan = BattlePlanStatus::None;
                self.next_ready_frame = now + self.module_data.transition_idle_frames;
            }
            TransitionStatus::Unpacking => {
                self.apply_unpacking_states(ctx, now);
            }
            TransitionStatus::Active => {
                self.set_battle_plan(self.current_plan, ctx);
                self.apply_active_states(ctx);
            }
            TransitionStatus::Packing => {
                self.set_battle_plan(BattlePlanStatus::None, ctx);
                self.apply_packing_states(ctx, now);
            }
        }

        self.status = new_status;
    }

    fn clear_old_status_states(&self, old_status: TransitionStatus, ctx: &mut UpdateContext<'_>) {
        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        match old_status {
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
        }
    }

    fn apply_unpacking_states(&mut self, ctx: &mut UpdateContext<'_>, now: u32) {
        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        match self.current_plan {
            BattlePlanStatus::Bombardment => {
                object.set_model_condition_state(ModelConditionFlag::Door1Opening);
                self.next_ready_frame = now + self.module_data.bombardment_plan_animation_frames;
            }
            BattlePlanStatus::HoldTheLine => {
                object.set_model_condition_state(ModelConditionFlag::Door2Opening);
                self.next_ready_frame = now + self.module_data.hold_the_line_plan_animation_frames;
            }
            BattlePlanStatus::SearchAndDestroy => {
                object.set_model_condition_state(ModelConditionFlag::Door3Opening);
                self.next_ready_frame =
                    now + self.module_data.search_and_destroy_plan_animation_frames;
            }
            _ => {}
        }
    }

    fn apply_active_states(&self, ctx: &mut UpdateContext<'_>) {
        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        match self.current_plan {
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
        }
    }

    fn apply_packing_states(&mut self, ctx: &mut UpdateContext<'_>, now: u32) {
        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        match self.current_plan {
            BattlePlanStatus::Bombardment => {
                object.set_model_condition_state(ModelConditionFlag::Door1Closing);
                self.next_ready_frame = now + self.module_data.bombardment_plan_animation_frames;
            }
            BattlePlanStatus::HoldTheLine => {
                object.set_model_condition_state(ModelConditionFlag::Door2Closing);
                self.next_ready_frame = now + self.module_data.hold_the_line_plan_animation_frames;
            }
            BattlePlanStatus::SearchAndDestroy => {
                object.set_model_condition_state(ModelConditionFlag::Door3Closing);
                self.next_ready_frame =
                    now + self.module_data.search_and_destroy_plan_animation_frames;
            }
            _ => {}
        }
    }

    fn set_battle_plan(&mut self, plan: BattlePlanStatus, ctx: &mut UpdateContext<'_>) {
        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        let Some(player) = object.get_controlling_player() else {
            return;
        };

        // Remove old plan bonuses
        if self.plan_affecting_army != BattlePlanStatus::None {
            let plan_type = match self.plan_affecting_army {
                BattlePlanStatus::Bombardment => BattlePlanType::Bombard,
                BattlePlanStatus::HoldTheLine => BattlePlanType::HoldTheLine,
                BattlePlanStatus::SearchAndDestroy => BattlePlanType::SearchAndDestroy,
                BattlePlanStatus::None => unreachable!(),
            };
            player.change_battle_plan(plan_type, -1, &self.bonuses);
            self.remove_building_bonuses(ctx);
        }

        // Reset bonuses to default
        self.bonuses.armor_scalar = 1.0;
        self.bonuses.sight_range_scalar = 1.0;
        self.bonuses.bombardment = 0;
        self.bonuses.search_and_destroy = 0;
        self.bonuses.hold_the_line = 0;

        // Apply new plan
        match plan {
            BattlePlanStatus::None => {
                // Paralyze troops
                let _ = player.iterate_object_ids(|obj_id| {
                    let _ =
                        crate::object::registry::OBJECT_REGISTRY.with_object_mut(obj_id, |obj| {
                            let kind_of = obj.get_kind_of();
                            if (kind_of & self.module_data.valid_member_kind_of) != 0
                                && (kind_of & self.module_data.invalid_member_kind_of) == 0
                            {
                                obj.set_disabled_until(
                                    DisabledType::Paralyzed,
                                    ctx.game_logic.get_frame()
                                        + self.module_data.battle_plan_paralyze_frames,
                                );
                            }
                        });
                    Ok(())
                });
            }
            BattlePlanStatus::Bombardment => {
                self.bonuses.bombardment = 1;
                player.change_battle_plan(BattlePlanType::Bombard, 1, &self.bonuses);
            }
            BattlePlanStatus::HoldTheLine => {
                self.apply_hold_the_line_bonuses(ctx);
                self.bonuses.armor_scalar = self.module_data.hold_the_line_armor_damage_scalar;
                self.bonuses.hold_the_line = 1;
                player.change_battle_plan(BattlePlanType::HoldTheLine, 1, &self.bonuses);
            }
            BattlePlanStatus::SearchAndDestroy => {
                self.apply_search_and_destroy_bonuses(ctx);
                self.bonuses.search_and_destroy = 1;
                self.bonuses.sight_range_scalar =
                    self.module_data.search_and_destroy_sight_range_scalar;
                player.change_battle_plan(BattlePlanType::SearchAndDestroy, 1, &self.bonuses);
            }
        }

        self.plan_affecting_army = plan;
    }

    fn apply_hold_the_line_bonuses(&self, ctx: &UpdateContext<'_>) {
        if self
            .module_data
            .strategy_center_hold_the_line_max_health_scalar
            == 1.0
        {
            return;
        }

        let Some(object) = ctx.game_logic.find_object(self.thing) else {
            return;
        };

        if let Some(body) = object.get_body_module() {
            let current_max = body.get_max_health();
            let new_max = current_max
                * self
                    .module_data
                    .strategy_center_hold_the_line_max_health_scalar;
            body.set_max_health(
                new_max,
                self.module_data
                    .strategy_center_hold_the_line_max_health_change_type,
            );
        }
    }

    fn apply_search_and_destroy_bonuses(&self, ctx: &mut UpdateContext<'_>) {
        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        // Apply sight range scalar
        if self
            .module_data
            .strategy_center_search_and_destroy_sight_range_scalar
            != 1.0
        {
            let scalar = self
                .module_data
                .strategy_center_search_and_destroy_sight_range_scalar;
            object.set_vision_range(object.get_vision_range() * scalar);
            object.set_shroud_clearing_range(object.get_shroud_clearing_range() * scalar);
        }

        // Enable stealth detection
        if self
            .module_data
            .strategy_center_search_and_destroy_detects_stealth
        {
            if let Some(stealth_detector) = object.find_update_module("StealthDetectorUpdate") {
                stealth_detector.with_module(|module| {
                    if let Some(control) = module.get_stealth_detector_control_interface() {
                        control.set_sd_enabled(true);
                    }
                });
            }
        }
    }

    fn remove_building_bonuses(&self, ctx: &mut UpdateContext<'_>) {
        let Some(object) = ctx.game_logic.find_object_mut(self.thing) else {
            return;
        };

        match self.plan_affecting_army {
            BattlePlanStatus::HoldTheLine => {
                if self
                    .module_data
                    .strategy_center_hold_the_line_max_health_scalar
                    != 1.0
                {
                    if let Some(body) = object.get_body_module() {
                        let current_max = body.get_max_health();
                        let new_max = current_max
                            / self
                                .module_data
                                .strategy_center_hold_the_line_max_health_scalar;
                        body.set_max_health(
                            new_max,
                            self.module_data
                                .strategy_center_hold_the_line_max_health_change_type,
                        );
                    }
                }
            }
            BattlePlanStatus::SearchAndDestroy => {
                if self
                    .module_data
                    .strategy_center_search_and_destroy_sight_range_scalar
                    != 1.0
                {
                    let scalar = self
                        .module_data
                        .strategy_center_search_and_destroy_sight_range_scalar;
                    object.set_vision_range(object.get_vision_range() / scalar);
                    object.set_shroud_clearing_range(object.get_shroud_clearing_range() / scalar);
                }

                if self
                    .module_data
                    .strategy_center_search_and_destroy_detects_stealth
                {
                    if let Some(stealth_detector) =
                        object.find_update_module("StealthDetectorUpdate")
                    {
                        stealth_detector.with_module(|module| {
                            if let Some(control) = module.get_stealth_detector_control_interface() {
                                control.set_sd_enabled(false);
                            }
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn enable_turret(&self, enable: bool, ctx: &UpdateContext<'_>) {
        let Some(object) = ctx.game_logic.find_object(self.thing) else {
            return;
        };

        if let Some(ai) = object.get_ai() {
            let turret = ai.get_which_turret_for_cur_weapon();
            if turret != TurretType::Invalid {
                ai.set_turret_enabled(turret, enable);
            }
        }
    }

    fn recenter_turret(&self, ctx: &UpdateContext<'_>) {
        let Some(object) = ctx.game_logic.find_object(self.thing) else {
            return;
        };

        if let Some(ai) = object.get_ai() {
            let turret = ai.get_which_turret_for_cur_weapon();
            if turret != TurretType::Invalid {
                ai.recenter_turret(turret);
            }
        }
    }

    fn is_turret_in_natural_position(&self, ctx: &UpdateContext<'_>) -> bool {
        let Some(object) = ctx.game_logic.find_object(self.thing) else {
            return false;
        };

        if let Some(ai) = object.get_ai() {
            let turret = ai.get_which_turret_for_cur_weapon();
            if turret != TurretType::Invalid {
                return ai.is_turret_in_natural_position(turret);
            }
        }

        false
    }

    pub fn get_active_battle_plan(&self) -> BattlePlanStatus {
        if self.status == TransitionStatus::Active {
            self.plan_affecting_army
        } else {
            BattlePlanStatus::None
        }
    }

    pub fn save(&self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("BattlePlanUpdate::save failed to xfer {field}: {err}");
            }
        };

        xfer.xfer_version_write(1);
        let mut current_plan = self.current_plan as u32;
        xfer_io(xfer.xfer_u32(&mut current_plan), "current_plan");
        let mut desired_plan = self.desired_plan as u32;
        xfer_io(xfer.xfer_u32(&mut desired_plan), "desired_plan");
        let mut plan_affecting_army = self.plan_affecting_army as u32;
        xfer_io(
            xfer.xfer_u32(&mut plan_affecting_army),
            "plan_affecting_army",
        );
        let mut status = self.status as u32;
        xfer_io(xfer.xfer_u32(&mut status), "status");
        let mut next_ready_frame = self.next_ready_frame;
        xfer_io(xfer.xfer_u32(&mut next_ready_frame), "next_ready_frame");
        let mut invalid_settings = self.invalid_settings;
        xfer_io(xfer.xfer_bool(&mut invalid_settings), "invalid_settings");
        let mut centering_turret = self.centering_turret;
        xfer_io(xfer.xfer_bool(&mut centering_turret), "centering_turret");
        let mut armor_scalar = self.bonuses.armor_scalar;
        xfer_io(xfer.xfer_f32(&mut armor_scalar), "armor_scalar");
        let mut bombardment = self.bonuses.bombardment;
        xfer_io(xfer.xfer_i32(&mut bombardment), "bombardment");
        let mut search_and_destroy = self.bonuses.search_and_destroy;
        xfer_io(xfer.xfer_i32(&mut search_and_destroy), "search_and_destroy");
        let mut hold_the_line = self.bonuses.hold_the_line;
        xfer_io(xfer.xfer_i32(&mut hold_the_line), "hold_the_line");
        let mut sight_range_scalar = self.bonuses.sight_range_scalar;
        xfer_io(xfer.xfer_f32(&mut sight_range_scalar), "sight_range_scalar");
        let mut vision_object_id = self.vision_object_id;
        xfer.xfer_option_object_id("vision_object_id", &mut vision_object_id);
    }

    pub fn load(&mut self, xfer: &mut dyn Xfer) {
        let xfer_io = |result: std::io::Result<()>, field: &str| {
            if let Err(err) = result {
                panic!("BattlePlanUpdate::load failed to xfer {field}: {err}");
            }
        };

        let version = xfer.xfer_version_read();
        if version >= 1 {
            let mut plan_val = 0u32;
            xfer_io(xfer.xfer_u32(&mut plan_val), "current_plan");
            self.current_plan = match plan_val {
                0 => BattlePlanStatus::None,
                1 => BattlePlanStatus::Bombardment,
                2 => BattlePlanStatus::HoldTheLine,
                3 => BattlePlanStatus::SearchAndDestroy,
                _ => BattlePlanStatus::None,
            };

            xfer_io(xfer.xfer_u32(&mut plan_val), "desired_plan");
            self.desired_plan = match plan_val {
                0 => BattlePlanStatus::None,
                1 => BattlePlanStatus::Bombardment,
                2 => BattlePlanStatus::HoldTheLine,
                3 => BattlePlanStatus::SearchAndDestroy,
                _ => BattlePlanStatus::None,
            };

            xfer_io(xfer.xfer_u32(&mut plan_val), "plan_affecting_army");
            self.plan_affecting_army = match plan_val {
                0 => BattlePlanStatus::None,
                1 => BattlePlanStatus::Bombardment,
                2 => BattlePlanStatus::HoldTheLine,
                3 => BattlePlanStatus::SearchAndDestroy,
                _ => BattlePlanStatus::None,
            };

            let mut status_val = 0u32;
            xfer_io(xfer.xfer_u32(&mut status_val), "status");
            self.status = match status_val {
                0 => TransitionStatus::Idle,
                1 => TransitionStatus::Unpacking,
                2 => TransitionStatus::Active,
                3 => TransitionStatus::Packing,
                _ => TransitionStatus::Idle,
            };

            xfer_io(
                xfer.xfer_u32(&mut self.next_ready_frame),
                "next_ready_frame",
            );
            xfer_io(
                xfer.xfer_bool(&mut self.invalid_settings),
                "invalid_settings",
            );
            xfer_io(
                xfer.xfer_bool(&mut self.centering_turret),
                "centering_turret",
            );
            xfer_io(
                xfer.xfer_f32(&mut self.bonuses.armor_scalar),
                "armor_scalar",
            );
            xfer_io(xfer.xfer_i32(&mut self.bonuses.bombardment), "bombardment");
            xfer_io(
                xfer.xfer_i32(&mut self.bonuses.search_and_destroy),
                "search_and_destroy",
            );
            xfer_io(
                xfer.xfer_i32(&mut self.bonuses.hold_the_line),
                "hold_the_line",
            );
            xfer_io(
                xfer.xfer_f32(&mut self.bonuses.sight_range_scalar),
                "sight_range_scalar",
            );
            xfer.xfer_option_object_id("vision_object_id", &mut self.vision_object_id);
        }
    }
}
