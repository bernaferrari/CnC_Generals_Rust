//! GameLogic bridge for the Common build assistant backend.

use std::sync::Arc;

use crate::common::Coord3D as LogicCoord3D;
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::object::production::construction::FoundationValidator;
use crate::player::player_list;
use game_engine::common::system::build_assistant::{
    set_build_assistant_backend, BuildAssistantBackend, Coord3D, LegalBuildCode,
    LocalLegalToBuildOptions, ObjectID,
};

#[derive(Debug, Default)]
pub struct GameLogicBuildAssistantBackend;

impl BuildAssistantBackend for GameLogicBuildAssistantBackend {
    fn build_object_now(
        &self,
        builder_id: Option<ObjectID>,
        template_name: &str,
        pos: &Coord3D,
        angle: f32,
        owning_player: u32,
    ) -> Option<ObjectID> {
        let template = TheThingFactory::find_template(template_name)?;

        let team = if let Some(builder_id) = builder_id {
            if let Some(builder) = TheGameLogic::find_object_by_id(builder_id) {
                if let Ok(guard) = builder.read() {
                    guard.get_team()
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            if let Ok(list) = player_list().read() {
                if let Some(player) = list.get_player(owning_player as i32) {
                    if let Ok(player_guard) = player.read() {
                        player_guard.get_default_team()
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        }?;

        let team_guard = team.read().ok()?;
        let factory = TheThingFactory::get().ok()?;
        let new_object = factory.new_object(template.clone(), &*team_guard).ok()?;

        let mut build_max_health = 0.0;
        if let Ok(guard) = new_object.read() {
            if let Some(body) = guard.get_body_module() {
                if let Ok(body_guard) = body.lock() {
                    build_max_health = body_guard.get_max_health();
                }
            }
        }

        if let Ok(mut guard) = new_object.write() {
            let _ = guard.set_position(&LogicCoord3D::new(pos.x, pos.y, pos.z));
            let _ = guard.set_orientation(angle as f32);
            if let Some(builder_id) = builder_id {
                if let Some(builder) = TheGameLogic::find_object_by_id(builder_id) {
                    if let Ok(builder_guard) = builder.read() {
                        guard.set_producer(Some(&*builder_guard));
                        guard.set_builder(Some(&*builder_guard));
                    }
                }
            }
            guard.set_construction_percent(0.0);
            if build_max_health > 0.0 {
                let _ = guard.set_health(1.0);
            }
        }

        if let Some(builder_id) = builder_id {
            if let Some(builder) = TheGameLogic::find_object_by_id(builder_id) {
                if let Ok(builder_guard) = builder.read() {
                    let total_build_frames = {
                        let player_id_opt = builder_guard.get_controlling_player_id();
                        if let Some(id) = player_id_opt {
                            if let Ok(list) = player_list().read() {
                                if let Some(player) = list.get_player(id as i32) {
                                    if let Ok(player_guard) = player.read() {
                                        template.calc_time_to_build(Some(&*player_guard)).max(1)
                                            as u32
                                    } else {
                                        template.calc_time_to_build(None).max(1) as u32
                                    }
                                } else {
                                    template.calc_time_to_build(None).max(1) as u32
                                }
                            } else {
                                template.calc_time_to_build(None).max(1) as u32
                            }
                        } else {
                            template.calc_time_to_build(None).max(1) as u32
                        }
                    };

                    if let Some(ai) = builder_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.try_lock() {
                            if let Some(worker_ai) = ai_guard.get_worker_ai_update_interface_mut() {
                                worker_ai.set_build_task(
                                    new_object.read().map(|g| g.get_id()).unwrap_or(0),
                                    total_build_frames,
                                    build_max_health,
                                    false,
                                );
                            } else if let Some(dozer_ai) =
                                ai_guard.get_dozer_ai_update_interface_mut()
                            {
                                dozer_ai.set_build_task(
                                    new_object.read().map(|g| g.get_id()).unwrap_or(0),
                                    total_build_frames,
                                    build_max_health,
                                    false,
                                );
                            }
                        }
                    }
                }
            }
        }

        new_object.read().ok().map(|guard| guard.get_id())
    }

    fn is_location_legal_to_build(
        &self,
        world_pos: &Coord3D,
        template_name: &str,
        angle: f32,
        _options: LocalLegalToBuildOptions,
        _builder_id: Option<ObjectID>,
        player_id: Option<u32>,
    ) -> LegalBuildCode {
        let validator = FoundationValidator::from_build_options(_options);
        let logic_pos = LogicCoord3D::new(world_pos.x, world_pos.y, world_pos.z);
        let owner = player_id.unwrap_or(0);
        match validator.validate_placement(&logic_pos, template_name, angle, owner) {
            Ok(()) => LegalBuildCode::Ok,
            Err(err) => {
                let msg = err.to_ascii_lowercase();
                if msg.contains("shroud") {
                    LegalBuildCode::Shroud
                } else if msg.contains("object") {
                    LegalBuildCode::ObjectsInTheWay
                } else if msg.contains("path") {
                    LegalBuildCode::NoClearPath
                } else if msg.contains("flat") {
                    LegalBuildCode::NotFlatEnough
                } else if msg.contains("cliff")
                    || msg.contains("underwater")
                    || msg.contains("bridge")
                    || msg.contains("terrain")
                {
                    LegalBuildCode::RestrictedTerrain
                } else {
                    LegalBuildCode::GenericFailure
                }
            }
        }
    }
}

pub fn install_build_assistant_backend() {
    set_build_assistant_backend(Arc::new(GameLogicBuildAssistantBackend::default()));
}
