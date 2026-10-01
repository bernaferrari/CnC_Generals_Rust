//! MobMemberSlavedUpdate - Mob/horde member behavior
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, Bool, CommandSourceType, Coord3D, Int, ModuleData, ObjectID, Real, UnsignedInt,
};
use crate::modules::{
    BehaviorModuleInterface, SlavedUpdateInterface, UpdateModuleInterface,
    UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{BehaviorModuleData, xfer_update_module_base_state};
use crate::object::draw::draw_module::RGBColor;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::{INVALID_ID as OBJECT_INVALID_ID, Object as GameObject};
use crate::path::PATHFIND_CELL_SIZE_F;
use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::Arc;

/// Wave 374: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

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
    object_id: ObjectID,
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
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<MobMemberSlavedUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object_id: object_id,
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

    fn ai_goal_distance(ai: &dyn crate::modules::AIUpdateInterface) -> Real {
        ai.get_locomotor_distance_to_goal()
    }

    /// C++ AIUpdateInterface::aiMoveToPosition(pos, addWaypoint, cmdSource).
    fn ai_move_to_position(
        ai: &mut dyn crate::modules::AIUpdateInterface,
        pos: &Coord3D,
        add_waypoint: bool,
        cmd_source: CommandSourceType,
    ) {
        let mut params = crate::ai::AiCommandParams::new(
            if add_waypoint {
                crate::ai::AiCommandType::FollowPathAppend
            } else {
                crate::ai::AiCommandType::MoveToPosition
            },
            cmd_source,
        );
        params.pos = *pos;
        let _ = ai.execute_command(&params);
    }

    /// C++ AIUpdateInterface::aiIdle(cmdSource).
    fn ai_idle(ai: &mut dyn crate::modules::AIUpdateInterface, cmd_source: CommandSourceType) {
        let params = crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Idle, cmd_source);
        let _ = ai.execute_command(&params);
    }

    /// C++ AIUpdateInterface::aiAttackObject(victim, maxShots, cmdSource).
    fn ai_attack_object(
        ai: &mut dyn crate::modules::AIUpdateInterface,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        let mut params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::AttackObject, cmd_source);
        params.obj = Some(victim_id);
        params.int_value = max_shots_to_fire;
        let _ = ai.execute_command(&params);
    }

    /// C++ clears firing bits when the mob member is on the player weapon upgrade.
    fn clear_player_upgrade_weapon_flags(obj: &GameObject) {
        let Some(drawable) = obj.get_drawable() else {
            return;
        };
        let upgrade = crate::common::ModelConditionFlags::WEAPONSET_PLAYER_UPGRADE;
        let has_upgrade = drawable
            .read()
            .ok()
            .map(|draw| draw.get_model_conditions().contains(upgrade))
            .unwrap_or(false);
        if has_upgrade {
            let clear = crate::common::ModelConditionFlags::RELOADING_A
                | crate::common::ModelConditionFlags::BETWEEN_FIRING_SHOTS_A
                | crate::common::ModelConditionFlags::PREATTACK_A
                | crate::common::ModelConditionFlags::FIRING_A
                | crate::common::ModelConditionFlags::USING_WEAPON_A;
            if let Ok(mut draw) = drawable.write() {
                draw.clear_model_condition_flags(clear);
            }
        }
    }

    /// `may_spawn_self_task_ai` checks the parent out again, so the leader
    /// must already be back in the registry before this runs.
    fn master_may_spawn_self_task(master_id: ObjectID, ratio: Real) -> bool {
        use crate::object::behavior::spawn_behavior::{
            SpawnBehaviorInterface, SpawnBehaviorModule,
        };
        let Some(handles) =
            OBJECT_REGISTRY.with_object(master_id, |master| master.behavior_modules())
        else {
            return false;
        };
        for handle in handles {
            if let Some(allowed) =
                handle.with_module_downcast::<SpawnBehaviorModule, _, _>(|module| {
                    module.behavior_mut().may_spawn_self_task_ai(ratio)
                })
            {
                return allowed;
            }
        }
        false
    }

    fn command_catch_up(
        &mut self,
        master_is_moving: bool,
        master_path_dist: Real,
        my_path_dist: Real,
        master_pos: &Coord3D,
        master_goal: &Coord3D,
    ) -> bool {
        let object_id = self.object_id;
        OBJECT_REGISTRY
            .with_object_mut(object_id, |obj| {
                let Some(my_ai) = obj.get_ai_update_interface_mut() else {
                    return false;
                };
                if master_is_moving {
                    if master_path_dist > my_path_dist {
                        my_ai.choose_locomotor_set(crate::common::LocomotorSetType::Wander);
                    } else {
                        my_ai.choose_locomotor_set(crate::common::LocomotorSetType::Panic);
                    }
                    if master_goal.length() < 1.0 {
                        Self::ai_move_to_position(
                            my_ai,
                            master_pos,
                            false,
                            CommandSourceType::FromAi,
                        );
                    } else {
                        let my_goal = my_ai.get_goal_position().unwrap_or(Coord3D::ZERO);
                        let delta = my_goal - *master_goal;
                        if delta.length() > 5.0 * PATHFIND_CELL_SIZE_F {
                            Self::ai_move_to_position(
                                my_ai,
                                master_goal,
                                false,
                                CommandSourceType::FromAi,
                            );
                        }
                    }
                } else {
                    my_ai.choose_locomotor_set(crate::common::LocomotorSetType::Panic);
                    Self::ai_move_to_position(my_ai, master_pos, false, CommandSourceType::FromAi);
                }
                true
            })
            .unwrap_or(false)
    }
}

impl UpdateModuleInterface for MobMemberSlavedUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // Wave 374: empty dual-world → Forever.
        if dual_world_registry_unavailable() {
            return UpdateSleepTime::Forever;
        }

        struct LeaderView {
            has_ai: bool,
            has_drawable: bool,
            victim_id: Option<ObjectID>,
            path_dist: Real,
            pos: Coord3D,
            goal: Coord3D,
            is_moving: bool,
            has_spawn: bool,
        }
        struct MemberView {
            path_dist: Real,
            pos: Coord3D,
            is_moving: bool,
            victim_id: Option<ObjectID>,
        }

        if self.object_id == crate::common::INVALID_ID || !OBJECT_REGISTRY.contains(self.object_id)
        {
            return UpdateSleepTime::None;
        }

        let leader_id = self.mob_leader;
        if !OBJECT_REGISTRY.contains(leader_id) {
            let object_id = self.object_id;
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                self.stop_slaved_effects(obj);
                obj.kill(None, None);
            });
            return UpdateSleepTime::None;
        }

        // Copy the leader before checking this member out. One id at a time.
        let Some(leader) = OBJECT_REGISTRY.with_object(leader_id, |master| LeaderView {
            has_ai: master.get_ai_update_interface().is_some(),
            has_drawable: master.get_drawable().is_some(),
            victim_id: master.get_current_victim_id(),
            path_dist: master
                .get_ai_update_interface()
                .map(Self::ai_goal_distance)
                .unwrap_or(0.0),
            pos: *master.get_position(),
            goal: master
                .get_ai_update_interface()
                .and_then(|ai| ai.get_goal_position())
                .unwrap_or(Coord3D::ZERO),
            is_moving: master
                .get_ai_update_interface()
                .map(|ai| ai.is_moving())
                .unwrap_or(false),
            has_spawn: master
                .with_spawn_behavior_full_interface(|_| ())
                .is_some(),
        }) else {
            return UpdateSleepTime::None;
        };

        let object_id = self.object_id;
        let member = match OBJECT_REGISTRY.with_object_mut(object_id, |obj| -> Option<MemberView> {
            if obj.get_ai_update_interface().is_none()
                || !leader.has_ai
                || obj.get_drawable().is_none()
                || !leader.has_drawable
            {
                return None;
            }

            Self::clear_player_upgrade_weapon_flags(obj);

            self.frames_to_wait += 1;
            if self.frames_to_wait < 16 {
                return None;
            }
            self.frames_to_wait = 0;

            let mut has_loco = false;
            if let Some(ai) = obj.get_ai_update_interface() {
                ai.with_cur_locomotor(&mut |_| has_loco = true);
            }
            if !has_loco {
                return None;
            }

            if let Some(master_victim_id) = leader.victim_id {
                self.primary_victim_id = master_victim_id;
            }

            Some(MemberView {
                path_dist: obj
                    .get_ai_update_interface()
                    .map(Self::ai_goal_distance)
                    .unwrap_or(0.0),
                pos: *obj.get_position(),
                is_moving: obj
                    .get_ai_update_interface()
                    .map(|ai| ai.is_moving())
                    .unwrap_or(false),
                victim_id: obj.get_current_victim_id(),
            })
        }) {
            Some(Some(view)) => view,
            _ => return UpdateSleepTime::None,
        };

        let must_catch_up_radius = self.module_data.must_catch_up_radius;
        let bail_time = self.module_data.catch_up_crisis_bail_time;
        let catch_up_radius_sq = Self::distance_squared(&member.pos, &leader.pos);
        let catch_up = catch_up_radius_sq > (must_catch_up_radius as Real).powi(2);
        let self_task = !catch_up && !member.is_moving && leader.has_spawn;

        if catch_up {
            if !self.command_catch_up(
                leader.is_moving,
                leader.path_dist,
                member.path_dist,
                &leader.pos,
                &leader.goal,
            ) {
                return UpdateSleepTime::None;
            }

            if catch_up_radius_sq > (must_catch_up_radius as Real * 3.0).powi(2) {
                self.catch_up_crisis_timer += 1;
                if self.catch_up_crisis_timer > bail_time {
                    let object_id = self.object_id;
                    let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                        obj.kill(None, None);
                    });
                    return UpdateSleepTime::None;
                } else if self.catch_up_crisis_timer > bail_time / 3 {
                    let object_id = self.object_id;
                    let master_pos = leader.pos;
                    let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                        if let Some(my_ai) = obj.get_ai_update_interface_mut() {
                            Self::ai_move_to_position(
                                my_ai,
                                &master_pos,
                                false,
                                CommandSourceType::FromAi,
                            );
                        }
                    });
                }
            }
        } else if member.is_moving {
            self.catch_up_crisis_timer = 0;
            let set = match crate::GameLogicRandomValue!(0, 10) {
                1 => Some(crate::common::LocomotorSetType::Wander),
                2 => Some(crate::common::LocomotorSetType::Panic),
                3 => Some(crate::common::LocomotorSetType::Normal),
                _ => None,
            };
            if let Some(set) = set {
                let object_id = self.object_id;
                let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                    if let Some(my_ai) = obj.get_ai_update_interface_mut() {
                        my_ai.choose_locomotor_set(set);
                    }
                });
            }
        } else if self_task {
            self.catch_up_crisis_timer = 0;

            let master_is_idle = OBJECT_REGISTRY
                .with_object(leader_id, |master| {
                    master
                        .get_ai_update_interface()
                        .map(|ai| ai.is_idle())
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if master_is_idle {
                let object_id = self.object_id;
                let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                    if let Some(my_ai) = obj.get_ai_update_interface_mut() {
                        Self::ai_idle(my_ai, CommandSourceType::FromAi);
                    }
                });
                self.primary_victim_id = OBJECT_INVALID_ID;
                return UpdateSleepTime::None;
            }

            let ratio = self.squirrelliness_ratio;
            let may_self_task = Self::master_may_spawn_self_task(leader_id, ratio);
            let mut victim_id = member.victim_id;
            if may_self_task {
                let object_id = self.object_id;
                let not_from_ai = OBJECT_REGISTRY
                    .with_object(object_id, |obj| {
                        obj.get_ai_update_interface()
                            .map(|ai| ai.get_last_command_source() != CommandSourceType::FromAi)
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if not_from_ai {
                    let new_target_id = OBJECT_REGISTRY
                        .with_object_mut(object_id, |obj| {
                            obj.get_ai_update_interface_mut()
                                .map(|ai| ai.get_next_mood_target_id(false, false))
                                .unwrap_or(OBJECT_INVALID_ID)
                        })
                        .unwrap_or(OBJECT_INVALID_ID);
                    if new_target_id != OBJECT_INVALID_ID && victim_id != Some(new_target_id) {
                        let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                            if let Some(my_ai) = obj.get_ai_update_interface_mut() {
                                Self::ai_attack_object(
                                    my_ai,
                                    new_target_id,
                                    999,
                                    CommandSourceType::FromAi,
                                );
                            }
                        });
                        victim_id = Some(new_target_id);
                        self.is_self_tasking = true;
                    }
                }
            }

            if victim_id.is_none() {
                if self.primary_victim_id != OBJECT_INVALID_ID
                    && OBJECT_REGISTRY.contains(self.primary_victim_id)
                {
                    let primary = self.primary_victim_id;
                    let object_id = self.object_id;
                    let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                        if let Some(my_ai) = obj.get_ai_update_interface_mut() {
                            Self::ai_attack_object(
                                my_ai,
                                primary,
                                999,
                                CommandSourceType::FromAi,
                            );
                        }
                    });
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
        master_id: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 374: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        if OBJECT_REGISTRY
            .with_object(master_id, |master| {
                self.start_slaved_effects(master);
            })
            .is_none()
        {
            return Ok(());
        }
        Ok(())
    }

    fn is_self_tasking(&self) -> bool {
        self.is_self_tasking
    }

    fn on_slaver_die(
        &mut self,
        _damage_info: Option<&crate::damage::DamageInfo>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 374: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let object_id = self.object_id;
        if object_id != crate::common::INVALID_ID {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                self.stop_slaved_effects(obj);
            });
        }
        Ok(())
    }

    fn on_slaver_damage(
        &mut self,
        damage_info: &mut crate::damage::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 374: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        let object_id = self.object_id;
        if object_id != crate::common::INVALID_ID {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj| {
                if let Some(ai) = obj.get_ai_update_interface_mut() {
                    // C++ AIUpdateInterface::aiGoProne(damageInfo, FromAI).
                    ai.ai_go_prone(damage_info, CommandSourceType::FromAi);
                }
            });
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
        object_id: ObjectID,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(MobMemberSlavedUpdate::new(object_id, module_data)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slaver_id_reports_current_mob_leader() {
        let data = Arc::new(MobMemberSlavedUpdateModuleData::default());
        let mut update = MobMemberSlavedUpdate {
            object_id: crate::common::INVALID_ID,
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
