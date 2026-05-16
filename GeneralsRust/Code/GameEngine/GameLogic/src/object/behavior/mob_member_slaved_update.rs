//! MobMemberSlavedUpdate - Mob/horde member behavior
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, CommandSourceType, Coord3D, Int, ModuleData, ObjectID, Real, UnsignedInt,
};
use crate::modules::{
    AIUpdateInterfaceExt, BehaviorModuleInterface, SlavedUpdateInterface, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::draw::draw_module::RGBColor;
use crate::object::{Object as GameObject, INVALID_ID as OBJECT_INVALID_ID};
use crate::path::PATHFIND_CELL_SIZE_F;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

const MAX_SQUIRRELLINESS: Real = 1.0;
const DEFAULT_MUST_CATCH_UP_RADIUS: Int = 50;
const DEFAULT_NO_NEED_TO_CATCH_UP_RADIUS: Int = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MobStates {
    None,
    Idle,
    CatchupNow,
    CatchingUp,
    Attack,
}

#[derive(Clone, Debug)]
pub struct MobMemberSlavedUpdateModuleData {
    pub base: BehaviorModuleData,
    pub must_catch_up_radius: Int,
    pub no_need_to_catch_up_radius: Int,
    pub squirrelliness_ratio: Real,
    pub catch_up_crisis_bail_time: UnsignedInt,
}

impl Default for MobMemberSlavedUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            must_catch_up_radius: DEFAULT_MUST_CATCH_UP_RADIUS,
            no_need_to_catch_up_radius: DEFAULT_NO_NEED_TO_CATCH_UP_RADIUS,
            squirrelliness_ratio: 0.0,
            catch_up_crisis_bail_time: 999_999,
        }
    }
}

crate::impl_behavior_module_data_via_base!(MobMemberSlavedUpdateModuleData, base);

impl MobMemberSlavedUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, MOB_MEMBER_SLAVED_UPDATE_FIELDS)
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_must_catch_up_radius(
    _ini: &mut INI,
    data: &mut MobMemberSlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = first_value_token(tokens)?;
    data.must_catch_up_radius = INI::parse_int(token)?;
    Ok(())
}

fn parse_catch_up_crisis_bail_time(
    _ini: &mut INI,
    data: &mut MobMemberSlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = first_value_token(tokens)?;
    data.catch_up_crisis_bail_time = INI::parse_unsigned_int(token)?;
    Ok(())
}

fn parse_no_need_to_catch_up_radius(
    _ini: &mut INI,
    data: &mut MobMemberSlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = first_value_token(tokens)?;
    data.no_need_to_catch_up_radius = INI::parse_int(token)?;
    Ok(())
}

fn parse_squirrelliness(
    _ini: &mut INI,
    data: &mut MobMemberSlavedUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = first_value_token(tokens)?;
    data.squirrelliness_ratio = INI::parse_real(token)?;
    Ok(())
}

const MOB_MEMBER_SLAVED_UPDATE_FIELDS: &[FieldParse<MobMemberSlavedUpdateModuleData>] = &[
    FieldParse {
        token: "MustCatchUpRadius",
        parse: parse_must_catch_up_radius,
    },
    FieldParse {
        token: "CatchUpCrisisBailTime",
        parse: parse_catch_up_crisis_bail_time,
    },
    FieldParse {
        token: "NoNeedToCatchUpRadius",
        parse: parse_no_need_to_catch_up_radius,
    },
    FieldParse {
        token: "Squirrelliness",
        parse: parse_squirrelliness,
    },
];

pub struct MobMemberSlavedUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<MobMemberSlavedUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    mob_leader: ObjectID,
    frames_to_wait: Int,
    mob_state: MobStates,
    personal_color: RGBColor,
    primary_victim_id: ObjectID,
    squirrelliness_ratio: Real,
    is_self_tasking: Bool,
    catch_up_crisis_timer: UnsignedInt,
}

impl MobMemberSlavedUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<MobMemberSlavedUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            mob_leader: OBJECT_INVALID_ID,
            frames_to_wait: crate::GameLogicRandomValue!(0, 20),
            mob_state: MobStates::None,
            personal_color: RGBColor::new(
                (crate::GameLogicRandomValueReal!(0.2, 0.4) * 255.0) as u8,
                (crate::GameLogicRandomValueReal!(0.2, 0.4) * 255.0) as u8,
                (crate::GameLogicRandomValueReal!(0.2, 0.4) * 255.0) as u8,
            ),
            primary_victim_id: OBJECT_INVALID_ID,
            squirrelliness_ratio: 0.0,
            is_self_tasking: false,
            catch_up_crisis_timer: 0,
        })
    }

    fn clamp_squirrelliness(&mut self) {
        let data = &self.module_data;
        self.squirrelliness_ratio = data.squirrelliness_ratio.max(0.0).min(MAX_SQUIRRELLINESS);
    }

    fn start_slaved_effects(&mut self, slaver: &GameObject) {
        self.mob_leader = slaver.get_id();
    }

    fn stop_slaved_effects(&mut self, obj: &mut GameObject) {
        self.mob_leader = OBJECT_INVALID_ID;
        obj.clear_status(crate::MAKE_OBJECT_STATUS_MASK!(
            crate::common::ObjectStatusTypes::Unselectable
        ));
        obj.clear_disabled(crate::common::DisabledType::Held);
    }

    fn distance_squared(a: &Coord3D, b: &Coord3D) -> Real {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let dz = a.z - b.z;
        dx * dx + dy * dy + dz * dz
    }

    fn ai_goal_distance(ai: &Arc<std::sync::Mutex<dyn crate::modules::AIUpdateInterface>>) -> Real {
        ai.try_lock()
            .map(|guard| guard.get_locomotor_distance_to_goal())
            .unwrap_or(0.0)
    }
}

impl UpdateModuleInterface for MobMemberSlavedUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let obj_arc = match self.object.upgrade() {
            Some(arc) => arc,
            None => return UpdateSleepTime::None,
        };

        let data = &self.module_data;
        let mut obj_guard = match obj_arc.write() {
            Ok(guard) => guard,
            Err(_) => return UpdateSleepTime::None,
        };

        let master_arc = match crate::helpers::TheGameLogic::find_object_by_id(self.mob_leader) {
            Some(master) => master,
            None => {
                self.stop_slaved_effects(&mut obj_guard);
                obj_guard.kill(None, None);
                return UpdateSleepTime::None;
            }
        };

        let master_guard = match master_arc.read() {
            Ok(guard) => guard,
            Err(_) => return UpdateSleepTime::None,
        };

        let my_ai = match obj_guard.get_ai_update_interface() {
            Some(ai) => ai,
            None => return UpdateSleepTime::None,
        };
        let master_ai = match master_guard.get_ai_update_interface() {
            Some(ai) => ai,
            None => return UpdateSleepTime::None,
        };

        self.frames_to_wait += 1;
        if self.frames_to_wait < 16 {
            return UpdateSleepTime::None;
        }
        self.frames_to_wait = 0;

        if my_ai.get_cur_locomotor().is_none() {
            return UpdateSleepTime::None;
        }

        let victim = obj_guard.get_current_victim();
        let master_victim = master_guard.get_current_victim();

        if let Some(master_victim) = master_victim {
            if let Ok(victim_guard) = master_victim.read() {
                self.primary_victim_id = victim_guard.get_id();
            }
        }

        let primary_victim =
            crate::helpers::TheGameLogic::find_object_by_id(self.primary_victim_id);

        let master_path_dist_to_goal = Self::ai_goal_distance(&master_ai);
        let my_path_dist_to_goal = Self::ai_goal_distance(&my_ai);

        let catch_up_radius_sq =
            Self::distance_squared(obj_guard.get_position(), master_guard.get_position());
        let master_is_moving = master_ai
            .try_lock()
            .map(|guard| guard.is_moving())
            .unwrap_or(false);
        let my_is_moving = my_ai
            .try_lock()
            .map(|guard| guard.is_moving())
            .unwrap_or(false);

        if catch_up_radius_sq > (data.must_catch_up_radius as Real).powi(2) {
            if master_is_moving {
                if master_path_dist_to_goal > my_path_dist_to_goal {
                    my_ai.choose_locomotor_set(crate::common::LocomotorSetType::Wander);
                } else {
                    my_ai.choose_locomotor_set(crate::common::LocomotorSetType::Panic);
                }

                let master_goal = master_ai
                    .get_path_destination()
                    .unwrap_or(*master_guard.get_position());
                if master_goal.length() < 1.0 {
                    my_ai.ai_move_to_position(
                        master_guard.get_position(),
                        false,
                        CommandSourceType::FromAi,
                    );
                } else {
                    let my_goal = my_ai
                        .get_path_destination()
                        .unwrap_or(*obj_guard.get_position());
                    let delta = my_goal - master_goal;
                    if delta.length() > 5.0 * PATHFIND_CELL_SIZE_F {
                        my_ai.ai_move_to_position(&master_goal, false, CommandSourceType::FromAi);
                    }
                }
            } else {
                my_ai.choose_locomotor_set(crate::common::LocomotorSetType::Panic);
                my_ai.ai_move_to_position(
                    master_guard.get_position(),
                    false,
                    CommandSourceType::FromAi,
                );
            }

            if catch_up_radius_sq > (data.must_catch_up_radius as Real * 3.0).powi(2) {
                self.catch_up_crisis_timer += 1;
                if self.catch_up_crisis_timer > data.catch_up_crisis_bail_time {
                    obj_guard.kill(None, None);
                    return UpdateSleepTime::None;
                } else if self.catch_up_crisis_timer > data.catch_up_crisis_bail_time / 3 {
                    my_ai.ai_move_to_position(
                        master_guard.get_position(),
                        false,
                        CommandSourceType::FromAi,
                    );
                }
            }
        } else if my_is_moving {
            self.catch_up_crisis_timer = 0;
            match crate::GameLogicRandomValue!(0, 10) {
                1 => my_ai.choose_locomotor_set(crate::common::LocomotorSetType::Wander),
                2 => my_ai.choose_locomotor_set(crate::common::LocomotorSetType::Panic),
                3 => my_ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal),
                _ => {}
            }
        } else {
            self.catch_up_crisis_timer = 0;

            let mut may_self_task = false;
            let _ = master_guard.with_spawn_behavior_full_interface(|spawn_behavior| {
                may_self_task = spawn_behavior.may_spawn_self_task_ai(self.squirrelliness_ratio);
            });

            if master_ai.is_idle() {
                my_ai.ai_idle(CommandSourceType::FromAi);
                self.primary_victim_id = OBJECT_INVALID_ID;
                return UpdateSleepTime::None;
            }

            if may_self_task && my_ai.get_last_command_source() != CommandSourceType::FromAi {
                if let Some(new_target) = obj_guard.get_current_victim() {
                    if victim
                        .as_ref()
                        .map(|v| Arc::ptr_eq(v, &new_target))
                        .unwrap_or(false)
                        == false
                    {
                        my_ai.ai_attack_object(&new_target, 999, CommandSourceType::FromAi);
                        self.is_self_tasking = true;
                    }
                }
            }

            if victim.is_none() {
                if let Some(primary) = primary_victim {
                    my_ai.ai_attack_object(&primary, 999, CommandSourceType::FromAi);
                } else if !master_guard.is_attacking() {
                    // Auto acquire mode; leave idle.
                }
                self.is_self_tasking = false;
            }
        }

        UpdateSleepTime::None
    }
}

impl BehaviorModuleInterface for MobMemberSlavedUpdate {
    fn get_module_name(&self) -> &'static str {
        "MobMemberSlavedUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn on_object_created(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.clamp_squirrelliness();
        Ok(())
    }

    fn get_slaved_update_interface(&mut self) -> Option<&mut dyn SlavedUpdateInterface> {
        Some(self)
    }
}

impl SlavedUpdateInterface for MobMemberSlavedUpdate {
    fn slaved_update(&mut self, _object_id: ObjectID, _delta_time: Real) {
        let _ = self.update_simple();
    }

    fn slaver_id(&self) -> Option<ObjectID> {
        (self.mob_leader != OBJECT_INVALID_ID).then_some(self.mob_leader)
    }

    fn on_enslave(
        &mut self,
        master: &Arc<RwLock<GameObject>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let master_guard = master.read().map_err(|_| "slaver lock poisoned")?;
        self.start_slaved_effects(&*master_guard);
        Ok(())
    }

    fn is_self_tasking(&self) -> bool {
        self.is_self_tasking
    }

    fn on_slaver_die(
        &mut self,
        _damage_info: Option<&crate::damage::DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(obj_arc) = self.object.upgrade() {
            if let Ok(mut obj_guard) = obj_arc.write() {
                self.stop_slaved_effects(&mut obj_guard);
            }
        }
        Ok(())
    }

    fn on_slaver_damage(
        &mut self,
        damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(obj_arc) = self.object.upgrade() {
            if let Ok(obj_guard) = obj_arc.read() {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    ai.ai_go_prone(damage_info, CommandSourceType::FromAi);
                }
            }
        }
        Ok(())
    }
}

impl Snapshotable for MobMemberSlavedUpdate {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        let mut slaver = self.mob_leader;
        xfer.xfer_object_id(&mut slaver)
            .map_err(|e| e.to_string())?;
        self.mob_leader = slaver;

        let mut frames_to_wait = self.frames_to_wait;
        xfer.xfer_i32(&mut frames_to_wait)
            .map_err(|e| e.to_string())?;
        self.frames_to_wait = frames_to_wait;

        let mut mob_state = self.mob_state as u32;
        xfer.xfer_u32(&mut mob_state).map_err(|e| e.to_string())?;
        self.mob_state = match mob_state {
            1 => MobStates::Idle,
            2 => MobStates::CatchupNow,
            3 => MobStates::CatchingUp,
            4 => MobStates::Attack,
            _ => MobStates::None,
        };

        let mut r = self.personal_color.r as Real / 255.0;
        let mut g = self.personal_color.g as Real / 255.0;
        let mut b = self.personal_color.b as Real / 255.0;
        xfer.xfer_real(&mut r).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut g).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut b).map_err(|e| e.to_string())?;
        self.personal_color = RGBColor::new(
            (r.clamp(0.0, 1.0) * 255.0).round() as u8,
            (g.clamp(0.0, 1.0) * 255.0).round() as u8,
            (b.clamp(0.0, 1.0) * 255.0).round() as u8,
        );

        let mut primary_victim_id = self.primary_victim_id;
        xfer.xfer_object_id(&mut primary_victim_id)
            .map_err(|e| e.to_string())?;
        self.primary_victim_id = primary_victim_id;

        let mut squirrelliness = self.squirrelliness_ratio;
        xfer.xfer_real(&mut squirrelliness)
            .map_err(|e| e.to_string())?;
        self.squirrelliness_ratio = squirrelliness;

        let mut is_self_tasking = self.is_self_tasking;
        xfer.xfer_bool(&mut is_self_tasking)
            .map_err(|e| e.to_string())?;
        self.is_self_tasking = is_self_tasking;

        let mut catch_up_timer = self.catch_up_crisis_timer;
        xfer.xfer_unsigned_int(&mut catch_up_timer)
            .map_err(|e| e.to_string())?;
        self.catch_up_crisis_timer = catch_up_timer;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes MobMemberSlavedUpdate through the common Module trait.
pub struct MobMemberSlavedUpdateModule {
    behavior: MobMemberSlavedUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<MobMemberSlavedUpdateModuleData>,
}

impl MobMemberSlavedUpdateModule {
    pub fn new(
        behavior: MobMemberSlavedUpdate,
        module_name: &AsciiString,
        module_data: Arc<MobMemberSlavedUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut MobMemberSlavedUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for MobMemberSlavedUpdateModule {
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

impl Module for MobMemberSlavedUpdateModule {
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

pub struct MobMemberSlavedUpdateFactory;
impl MobMemberSlavedUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(MobMemberSlavedUpdate::new(thing, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slaver_id_reports_current_mob_leader() {
        let data = Arc::new(MobMemberSlavedUpdateModuleData::default());
        let mut update = MobMemberSlavedUpdate {
            object: Weak::new(),
            module_data: data,
            next_call_frame_and_phase: 0,
            mob_leader: OBJECT_INVALID_ID,
            frames_to_wait: 0,
            mob_state: MobStates::None,
            personal_color: RGBColor::new(0, 0, 0),
            primary_victim_id: OBJECT_INVALID_ID,
            squirrelliness_ratio: 0.0,
            is_self_tasking: false,
            catch_up_crisis_timer: 0,
        };

        assert_eq!(update.slaver_id(), None);
        update.mob_leader = 42;
        assert_eq!(update.slaver_id(), Some(42));
    }
}
