//! Port of `GeneralsMD/Code/GameEngine/Source/GameLogic/Object/Behavior/RebuildHoleBehavior.cpp`.
//!
//! RebuildHoleBehavior - Rust conversion of C++ RebuildHoleBehavior
//!
//! GLA Hole behavior that reconstructs a building after death. This module manages
//! the lifecycle of reconstruction including worker spawning, building progress,
//! attacker transfer, and cleanup.

use std::any::Any;
use std::sync::{Arc, RwLock};

use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{
    AsciiString, ObjectID, ObjectStatusMaskType, ObjectStatusTypes, Real, UnsignedInt, INVALID_ID,
    LOGICFRAMES_PER_SECOND,
};
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::{
    BehaviorModuleInterface, BodyModuleInterfaceExt, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::{
    xfer_update_module_base_state, BehaviorModuleData, RebuildHoleBehaviorInterface,
};
use crate::object::behavior::sticky_bomb_update::StickyBombUpdate;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::Object;
use crate::scripting::engine::transfer_object_name;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType, Thing as ModuleThing};

// ============================================================================
// Module data
// ============================================================================

#[derive(Debug, Clone)]
pub struct RebuildHoleBehaviorModuleData {
    pub base: BehaviorModuleData,
    pub worker_template_name: AsciiString,
    pub worker_respawn_delay: UnsignedInt,
    pub hole_health_regen_percent_per_second: Real,
}

impl Default for RebuildHoleBehaviorModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            worker_template_name: AsciiString::default(),
            worker_respawn_delay: 0,
            hole_health_regen_percent_per_second: 0.1,
        }
    }
}

crate::impl_behavior_module_data_via_base!(RebuildHoleBehaviorModuleData, base);

impl RebuildHoleBehaviorModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, REBUILD_HOLE_FIELDS)
    }
}

fn first_value_token<'a>(tokens: &'a [&'a str]) -> Result<&'a str, INIError> {
    tokens
        .iter()
        .copied()
        .find(|token| *token != "=")
        .ok_or(INIError::InvalidData)
}

fn parse_ascii_string_field(
    setter: &mut dyn FnMut(AsciiString),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = first_value_token(tokens)?;
    setter(AsciiString::from(&INI::parse_ascii_string(token)?));
    Ok(())
}

fn parse_duration_real_frames(tokens: &[&str]) -> Result<UnsignedInt, INIError> {
    let token = first_value_token(tokens)?;
    Ok(INI::parse_duration_real(token)? as UnsignedInt)
}

fn parse_percent_to_real(tokens: &[&str]) -> Result<Real, INIError> {
    let token = first_value_token(tokens)?;
    INI::parse_percent_to_real(token)
}

const REBUILD_HOLE_FIELDS: &[FieldParse<RebuildHoleBehaviorModuleData>] = &[
    FieldParse {
        token: "WorkerObjectName",
        parse: |_, data, tokens| {
            parse_ascii_string_field(&mut |value| data.worker_template_name = value, tokens)
        },
    },
    FieldParse {
        token: "WorkerRespawnDelay",
        parse: |_, data, tokens| {
            data.worker_respawn_delay = parse_duration_real_frames(tokens)?;
            Ok(())
        },
    },
    FieldParse {
        token: "HoleHealthRegen%PerSecond",
        parse: |_, data, tokens| {
            data.hole_health_regen_percent_per_second = parse_percent_to_real(tokens)?;
            Ok(())
        },
    },
];

// ============================================================================
// Behavior implementation
// ============================================================================

#[derive(Debug)]
pub struct RebuildHoleBehavior {
    object_id: ObjectID,
    module_data: Arc<RebuildHoleBehaviorModuleData>,
    worker_id: ObjectID,
    reconstructing_id: ObjectID,
    spawner_object_id: ObjectID,
    next_call_frame_and_phase: UnsignedInt,
    worker_wait_counter: UnsignedInt,
    worker_template: Option<Arc<dyn crate::common::ThingTemplate>>,
    rebuild_template: Option<Arc<dyn crate::common::ThingTemplate>>,
}

impl RebuildHoleBehavior {
    pub fn new(object_id: ObjectID, module_data: Arc<RebuildHoleBehaviorModuleData>) -> Self {
        Self {
            object_id,
            module_data,
            worker_id: INVALID_ID,
            reconstructing_id: INVALID_ID,
            spawner_object_id: INVALID_ID,
            next_call_frame_and_phase: 0,
            worker_wait_counter: 0,
            worker_template: None,
            rebuild_template: None,
        }
    }

    pub fn from_module_thing(
        thing: Arc<dyn ModuleThing>,
        module_data: Arc<RebuildHoleBehaviorModuleData>,
    ) -> Self {
        let object_id = thing
            .as_object()
            .map(|obj| obj.get_object_id())
            .unwrap_or(INVALID_ID);
        Self::new(object_id, module_data)
    }

    fn get_object_id(&self) -> crate::common::ObjectID {
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
        TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
    }

    fn resolve_worker_template(&mut self) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        if self.worker_template.is_none() && !self.module_data.worker_template_name.is_empty() {
            self.worker_template =
                TheThingFactory::find_template(self.module_data.worker_template_name.as_str());
        }
        self.worker_template.clone()
    }

    fn new_worker_respawn_process(
        &mut self,
        existing_worker: Option<&Object>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(worker) = existing_worker {
            if worker.get_id() == self.worker_id {
                let _ = TheGameLogic::destroy_object(worker);
            }
        }

        self.worker_id = INVALID_ID;
        self.worker_wait_counter = self.module_data.worker_respawn_delay;

        let _ = self.with_object_mut(|hole_guard| {
            hole_guard.mask_object(false);
        });

        Ok(())
    }

    fn transfer_attackers(&self, from_id: ObjectID, to_id: ObjectID) {
        let Some(game_logic) = crate::system::game_logic::get_game_logic().lock().ok() else {
            return;
        };
        for &object_id in game_logic.get_all_object_ids() {
            let _ = OBJECT_REGISTRY.with_object(object_id, |guard| {
                if let Some(ai) = guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.try_lock() {
                        ai_guard.transfer_attack(from_id, to_id);
                    }
                }
            });
        }
    }

    fn transfer_bombs(&self, reconstruction: &Object) {
        let Some(game_logic) = crate::system::game_logic::get_game_logic().lock().ok() else {
            return;
        };
        let reconstruction_id = reconstruction.get_id();
        for &object_id in game_logic.get_all_object_ids() {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |guard| {
                if guard.is_kind_of(crate::common::KindOf::Mine) {
                    if let Some(module) = guard.find_update_module("StickyBombUpdate") {
                        module.with_module(|module| {
                            if let Some(sticky_bomb) = module.get_sticky_bomb_control_interface() {
                                if sticky_bomb.get_target() == self.object_id {
                                    sticky_bomb.set_target_object_id(reconstruction_id);
                                }
                            }
                        });
                    }
                }
            });
        }
    }

    fn spawn_worker_and_construct(&mut self, reconstructing: Option<Arc<RwLock<Object>>>) {
        let Some(worker_template) = self.resolve_worker_template() else {
            return;
        };

        let Some((hole_pos, hole_orient, hole_team, hole_player_id)) = self.with_object(|hole| {
            (
                *hole.get_position(),
                hole.get_orientation(),
                hole.get_team(),
                hole.get_controlling_player_id(),
            )
        }) else {
            return;
        };
        let hole_id = self.get_object_id();

        let factory = match TheThingFactory::get() {
            Ok(factory) => factory,
            Err(_) => return,
        };

        let worker_result = if let Some(team_arc) = hole_team.clone() {
            if let Ok(team_guard) = team_arc.read() {
                factory.new_object(worker_template, &*team_guard)
            } else {
                factory.new_object_optional_team(worker_template, None)
            }
        } else {
            factory.new_object_optional_team(worker_template, None)
        };

        let worker_arc = match worker_result {
            Ok(worker) => worker,
            Err(_) => return,
        };

        let worker_id = worker_arc
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(INVALID_ID);
        self.worker_id = worker_id;

        if let Ok(mut worker_guard) = worker_arc.write() {
            let _ = worker_guard.set_position(&hole_pos);
            worker_guard.set_status(
                ObjectStatusMaskType::from_status(ObjectStatusTypes::Unselectable),
                true,
            );
        }

        let mut reconstructing_arc = reconstructing;

        if let Some(existing) = reconstructing_arc.as_ref() {
            if let Ok(worker_guard) = worker_arc.write() {
                if let Some(ai) = worker_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.try_lock() {
                        let mut params = AiCommandParams::new(
                            AiCommandType::ResumeConstruction,
                            CommandSourceType::FromAi,
                        );
                        params.obj = Some(
                            existing
                                .read()
                                .ok()
                                .map(|g| g.get_id())
                                .unwrap_or(INVALID_ID),
                        );
                        let _ = ai_guard.execute_command(&params);
                    }
                }
            }
        } else {
            let Some(rebuild_template) = self.rebuild_template.clone() else {
                return;
            };

            let new_building = if let Some(team_arc) = hole_team.clone() {
                if let Ok(team_guard) = team_arc.read() {
                    factory.new_object(rebuild_template.clone(), &*team_guard)
                } else {
                    factory.new_object_optional_team(rebuild_template.clone(), None)
                }
            } else {
                factory.new_object_optional_team(rebuild_template.clone(), None)
            };

            let Ok(new_building_arc) = new_building else {
                return;
            };

            let mut build_max_health = 0.0;
            if let Ok(guard) = new_building_arc.read() {
                if let Some(body) = guard.get_body_module() {
                    build_max_health = body.get_max_health();
                }
            }

            if let Ok(mut guard) = new_building_arc.write() {
                let _ = guard.set_position(&hole_pos);
                if let Err(err) = guard.set_orientation(hole_orient) {
                    log::debug!("RebuildHoleBehavior::set_orientation failed: {err}");
                }
                guard.set_producer_id(hole_id);
                if let Ok(worker_guard) = worker_arc.read() {
                    guard.set_builder(Some(&*worker_guard));
                } else {
                    guard.set_builder(None);
                }
                guard.set_construction_percent(0.0);
                if build_max_health > 0.0 {
                    let _ = guard.set_health(1.0);
                }
            }

            if let Ok(worker_guard) = worker_arc.write() {
                if let Some(ai) = worker_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.try_lock() {
                        let total_build_frames = {
                            let player_opt = hole_player_id.and_then(|id| {
                                let player_list = crate::player::player_list();
                                let list = player_list.read().ok()?;
                                list.get_player(id as i32).cloned()
                            });
                            if let Some(player) = player_opt {
                                if let Ok(player_guard) = player.read() {
                                    rebuild_template
                                        .calc_time_to_build(Some(&*player_guard))
                                        .max(1) as u32
                                } else {
                                    rebuild_template.calc_time_to_build(None).max(1) as u32
                                }
                            } else {
                                rebuild_template.calc_time_to_build(None).max(1) as u32
                            }
                        };

                        let new_id = new_building_arc
                            .read()
                            .ok()
                            .map(|g| g.get_id())
                            .unwrap_or(INVALID_ID);
                        if let Some(worker_ai) = ai_guard.get_worker_ai_update_interface_mut() {
                            worker_ai.set_build_task(
                                new_id,
                                total_build_frames,
                                build_max_health,
                                true,
                            );
                        } else if let Some(dozer_ai) = ai_guard.get_dozer_ai_update_interface_mut()
                        {
                            dozer_ai.set_build_task(
                                new_id,
                                total_build_frames,
                                build_max_health,
                                true,
                            );
                        }
                    }
                }
            }

            reconstructing_arc = Some(new_building_arc);
        }

        let Some(reconstructing_arc) = reconstructing_arc else {
            return;
        };

        let recon_id = reconstructing_arc
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(INVALID_ID);
        self.reconstructing_id = recon_id;

        if let Ok(mut guard) = reconstructing_arc.write() {
            guard.set_producer_id(hole_id);
        }

        let _ = self.with_object_mut(|hole_guard| {
            hole_guard.mask_object(true);
        });

        self.transfer_attackers(self.object_id, recon_id);

        if let Ok(rebuild_guard) = reconstructing_arc.read() {
            self.transfer_bombs(&*rebuild_guard);
        };
    }

    fn handle_healing(&self) {
        let hole_id = self.get_object_id();
        let _ = self.with_object(|hole| {
            let Some(body) = hole.get_body_module() else {
                return;
            };
            let health = body.get_health();
            let max_health = body.get_max_health();
            if health >= max_health {
                return;
            }

            let amount = (self.module_data.hole_health_regen_percent_per_second
                / LOGICFRAMES_PER_SECOND as f32)
                * max_health;
            if amount <= 0.0 {
                return;
            }

            let mut healing_info =
                DamageInfo::with_simple(amount, hole_id, DamageType::Healing, DeathType::None);
            healing_info.sync_from_input();
            let _ = body.attempt_healing(&mut healing_info);
        });
    }

    fn finish_reconstruction(&mut self, reconstructing_id: ObjectID, worker_id: ObjectID) {
        let _ = self.with_object(|hole| {
            let _ = transfer_object_name(hole.get_name(), reconstructing_id);
        });

        if worker_id != INVALID_ID {
            let _ = TheGameLogic::destroy_object_by_id(worker_id);
        }

        let _ = TheGameLogic::destroy_object_by_id(self.get_object_id());
    }
}

impl UpdateModuleInterface for RebuildHoleBehavior {
    fn update(&mut self) -> Result<UpdateSleepTime, Box<dyn std::error::Error + Send + Sync>> {
        if self.get_object_id() == INVALID_ID {
            return Ok(UpdateSleepTime::Forever);
        }

        let mut worker_arc = None;
        if self.worker_id != INVALID_ID {
            worker_arc = TheGameLogic::find_object_by_id(self.worker_id);
            if worker_arc.is_none() {
                let _ = self.new_worker_respawn_process(None);
            }
        }

        let mut reconstructing_arc = None;
        if self.reconstructing_id != INVALID_ID {
            reconstructing_arc = TheGameLogic::find_object_by_id(self.reconstructing_id);
            if reconstructing_arc.is_none() {
                if let Some(worker_arc_ref) = worker_arc.as_ref() {
                    if let Ok(worker_guard) = worker_arc_ref.read() {
                        let _ = self.new_worker_respawn_process(Some(&*worker_guard));
                    } else {
                        let _ = self.new_worker_respawn_process(None);
                    }
                } else {
                    let _ = self.new_worker_respawn_process(None);
                }
                self.reconstructing_id = INVALID_ID;
            }
        }

        if worker_arc.is_none() && self.worker_wait_counter > 0 {
            self.worker_wait_counter = self.worker_wait_counter.saturating_sub(1);
            if self.worker_wait_counter == 0 {
                self.spawn_worker_and_construct(reconstructing_arc.clone());
            }
        }

        self.handle_healing();

        if let Some(reconstructing_arc) = reconstructing_arc.as_ref() {
            let done = reconstructing_arc
                .read()
                .ok()
                .map(|reconstructing_guard| {
                    !reconstructing_guard
                        .get_status_bits()
                        .test(ObjectStatusTypes::UnderConstruction)
                })
                .unwrap_or(false);
            if done {
                self.finish_reconstruction(self.reconstructing_id, self.worker_id);
            }
        }

        Ok(UpdateSleepTime::None)
    }
}

impl BehaviorModuleInterface for RebuildHoleBehavior {
    fn get_module_name(&self) -> &str {
        "RebuildHoleBehavior"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn on_die(
        &mut self,
        _damage_info: &crate::common::DamageInfo,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.worker_id != INVALID_ID {
            if let Some(worker) = TheGameLogic::find_object_by_id(self.worker_id) {
                if let Ok(worker_guard) = worker.read() {
                    let _ = TheGameLogic::destroy_object(&*worker_guard);
                }
            }
            self.worker_id = INVALID_ID;
        }
        let _ = TheGameLogic::destroy_object_by_id(self.get_object_id());
        Ok(())
    }

    fn get_rebuild_hole_behavior_interface(
        &mut self,
    ) -> Option<&mut dyn RebuildHoleBehaviorInterface> {
        Some(self)
    }
}

impl RebuildHoleBehaviorInterface for RebuildHoleBehavior {
    fn start_rebuild_process(
        &mut self,
        rebuild_template: Arc<dyn crate::common::ThingTemplate>,
        spawner_id: ObjectID,
    ) {
        self.rebuild_template = Some(rebuild_template);
        self.spawner_object_id = spawner_id;

        if let Some(worker_arc) = TheGameLogic::find_object_by_id(self.worker_id) {
            if let Ok(worker_guard) = worker_arc.read() {
                let _ = self.new_worker_respawn_process(Some(&*worker_guard));
            } else {
                let _ = self.new_worker_respawn_process(None);
            }
        } else {
            let _ = self.new_worker_respawn_process(None);
        }
    }

    fn get_spawner_id(&self) -> ObjectID {
        self.spawner_object_id
    }

    fn get_reconstructed_building_id(&self) -> ObjectID {
        self.reconstructing_id
    }

    fn get_rebuild_template(&self) -> Option<Arc<dyn crate::common::ThingTemplate>> {
        self.rebuild_template.clone()
    }
}

impl Snapshotable for RebuildHoleBehavior {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("version xfer: {e:?}"))?;

        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)?;

        let mut worker_id = self.worker_id;
        xfer.xfer_object_id(&mut worker_id)
            .map_err(|e| e.to_string())?;
        let mut reconstructing_id = self.reconstructing_id;
        xfer.xfer_object_id(&mut reconstructing_id)
            .map_err(|e| e.to_string())?;
        if version >= 2 {
            let mut spawner_object_id = self.spawner_object_id;
            xfer.xfer_object_id(&mut spawner_object_id)
                .map_err(|e| e.to_string())?;
        }
        let mut worker_wait_counter = self.worker_wait_counter;
        xfer.xfer_unsigned_int(&mut worker_wait_counter)
            .map_err(|e| e.to_string())?;

        let mut worker_name = self
            .worker_template
            .as_ref()
            .map(|t| t.get_name().to_string())
            .unwrap_or_default();
        let mut rebuild_name = self
            .rebuild_template
            .as_ref()
            .map(|t| t.get_name().to_string())
            .unwrap_or_default();

        game_engine::system::Xfer::xfer_ascii_string(xfer, &mut worker_name)
            .map_err(|e| e.to_string())?;
        game_engine::system::Xfer::xfer_ascii_string(xfer, &mut rebuild_name)
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("version xfer: {e:?}"))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_object_id(&mut self.worker_id)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.reconstructing_id)
            .map_err(|e| e.to_string())?;
        if version >= 2 {
            xfer.xfer_object_id(&mut self.spawner_object_id)
                .map_err(|e| e.to_string())?;
        }
        xfer.xfer_unsigned_int(&mut self.worker_wait_counter)
            .map_err(|e| e.to_string())?;

        let mut worker_name = self
            .worker_template
            .as_ref()
            .map(|t| t.get_name().to_string())
            .unwrap_or_default();
        let mut rebuild_name = self
            .rebuild_template
            .as_ref()
            .map(|t| t.get_name().to_string())
            .unwrap_or_default();

        game_engine::system::Xfer::xfer_ascii_string(xfer, &mut worker_name)
            .map_err(|e| e.to_string())?;
        game_engine::system::Xfer::xfer_ascii_string(xfer, &mut rebuild_name)
            .map_err(|e| e.to_string())?;

        if xfer.get_xfer_mode() == game_engine::system::XferMode::Load {
            self.worker_template = if worker_name.is_empty() {
                None
            } else {
                TheThingFactory::find_template(worker_name.as_str())
            };

            self.rebuild_template = if rebuild_name.is_empty() {
                None
            } else {
                TheThingFactory::find_template(rebuild_name.as_str())
            };
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// ============================================================================
// Module wrapper for engine registry
// ============================================================================

pub struct RebuildHoleBehaviorModule {
    behavior: RebuildHoleBehavior,
    module_name_key: NameKeyType,
    module_data: Arc<RebuildHoleBehaviorModuleData>,
}

impl RebuildHoleBehaviorModule {
    pub fn new(
        behavior: RebuildHoleBehavior,
        module_name: &AsciiString,
        module_data: Arc<RebuildHoleBehaviorModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior(&self) -> &RebuildHoleBehavior {
        &self.behavior
    }

    pub fn behavior_mut(&mut self) -> &mut RebuildHoleBehavior {
        &mut self.behavior
    }
}

impl Snapshotable for RebuildHoleBehaviorModule {
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

impl Module for RebuildHoleBehaviorModule {
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

    fn on_delete(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_duration_real_frames_accepts_duration_suffixes() {
        assert_eq!(
            parse_duration_real_frames(&["1500ms"]).expect("duration"),
            45
        );
        assert_eq!(parse_duration_real_frames(&["1.5s"]).expect("duration"), 45);
    }

    #[test]
    fn parse_duration_real_frames_truncates_fractional_frames() {
        assert_eq!(parse_duration_real_frames(&["50ms"]).expect("duration"), 1);
    }
}
