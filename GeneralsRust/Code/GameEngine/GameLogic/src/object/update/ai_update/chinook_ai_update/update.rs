//! Tick orchestration, repair-pad bookkeeping and rotor wash.

use super::{
    ChinookAIState, ChinookAIUpdate, ChinookFlightStatus, chinook_passenger_should_follow_attack,
    chinook_should_auto_land, chinook_should_auto_takeoff, dual_world_registry_unavailable,
};
use crate::ai::CommandSourceType;
use crate::common::{INVALID_ID, ObjectID};
use crate::helpers::{TheGameLogic, TheParticleSystemManager, TheTerrainLogic};
use crate::modules::{AIUpdateInterface, AIUpdateInterfaceExt, ContainModuleInterfaceExt};
use crate::player::player_list;
use crate::supply_system::SupplyTruckState;

impl ChinookAIUpdate {
    pub fn set_airfield_for_healing(&mut self, id: ObjectID) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if self.airfield_for_healing != INVALID_ID && self.airfield_for_healing != id {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(
                self.airfield_for_healing,
                |guard| {
                    let _ = guard.with_parking_place_behavior(|pp| {
                        pp.set_healee(Some(self.object_id), false);
                    });
                },
            );
        }
        self.airfield_for_healing = id;
    }
}

impl ChinookAIUpdate {
    fn update_rotor_wash(&self) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let local_index = player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(-1);
        if local_index < 0 {
            return;
        }
        if owner_guard.get_shrouded_status(local_index) != crate::common::ObjectShroudStatus::Clear
        {
            return;
        }
        if !matches!(
            self.flight_status,
            ChinookFlightStatus::Landing
                | ChinookFlightStatus::TakingOff
                | ChinookFlightStatus::Landed
        ) {
            return;
        }
        let mut pos = *owner_guard.get_position();
        let Some(terrain) = TheTerrainLogic::get() else {
            return;
        };
        let ground = terrain.get_ground_height(pos.x, pos.y, None);
        pos.z = ground + 3.0;
        let chopper_elevation = owner_guard.get_position().z - pos.z;
        if crate::helpers::game_client_random_value_real(0.0, chopper_elevation) < 5.0 {
            if let Some(ps_manager) = TheParticleSystemManager::get() {
                let template = if self.data.rotor_wash_particle_system.is_empty() {
                    None
                } else {
                    Some(self.data.rotor_wash_particle_system.as_str())
                };
                if let Some(id) = ps_manager.create_particle_system(template) {
                    ps_manager.set_particle_system_position(id, &pos);
                }
            }
        }
    }

    pub fn update(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Wave 349: empty dual-world → Ok(()).
        if dual_world_registry_unavailable() {
            return Ok(());
        }

        self.update_machine_state(ai);

        if self.airfield_for_healing != INVALID_ID {
            if let Some(airfield) = TheGameLogic::find_object_by_id(self.airfield_for_healing) {
                let healed = if self.flight_status == ChinookFlightStatus::Landed
                    && self.pending_command.is_none()
                {
                    crate::object::registry::OBJECT_REGISTRY
                        .with_object(self.object_id, |owner_guard| {
                            if let Some(body) = owner_guard.get_body_module() {
                                if let Ok(body_guard) = body.lock() {
                                    return body_guard.get_health() >= body_guard.get_max_health();
                                }
                            }
                            false
                        })
                        .unwrap_or(false)
                } else {
                    false
                };
                if let Ok(airfield_guard) = airfield.read() {
                    if healed {
                        let _ = airfield_guard.with_parking_place_behavior(|pp| {
                            pp.set_healee(Some(self.object_id), false);
                        });
                        self.set_my_state(
                            ChinookAIState::TakingOff,
                            None,
                            None,
                            CommandSourceType::FromAi,
                            ai,
                        );
                    } else {
                        let landed = self.flight_status == ChinookFlightStatus::Landed;
                        let _ = airfield_guard.with_parking_place_behavior(|pp| {
                            pp.set_healee(Some(self.object_id), landed);
                        });
                    }
                }
            } else {
                self.set_airfield_for_healing(INVALID_ID);
            }
        }

        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |guard| {
            if let Some(contain) = guard.get_contain() {
                if self.base.get_state() == SupplyTruckState::Idle {
                    let waiting = contain.has_objects_wanting_to_enter_or_exit();
                    if let Some(command) = self.pending_command.take() {
                        let _ = ai.execute_command(&command);
                    } else if chinook_should_auto_land(
                        true,
                        waiting,
                        self.flight_status == ChinookFlightStatus::Landed,
                    ) {
                        self.set_my_state(
                            ChinookAIState::Landing,
                            None,
                            None,
                            CommandSourceType::FromAi,
                            ai,
                        );
                    } else if chinook_should_auto_takeoff(
                        true,
                        waiting,
                        self.flight_status == ChinookFlightStatus::Landed,
                        self.airfield_for_healing != INVALID_ID,
                    ) {
                        self.set_my_state(
                            ChinookAIState::TakingOff,
                            None,
                            None,
                            CommandSourceType::FromAi,
                            ai,
                        );
                    }
                }

                if TheGameLogic::get_frame() % 10 == 1 {
                    if let Some(ai_update) = guard.get_ai_update_interface() {
                        if let Ok(ai_guard) = ai_update.lock() {
                            if let Some(victim_id) = ai_guard.get_current_victim() {
                                if contain.is_passenger_allowed_to_fire(None) {
                                    let passengers = contain.get_contained_objects();
                                    for passenger_id in passengers {
                                        if let Some(passenger) =
                                            TheGameLogic::find_object_by_id(passenger_id)
                                        {
                                            if let Ok(pass_guard) = passenger.read() {
                                                if let Some(pass_ai) =
                                                    pass_guard.get_ai_update_interface()
                                                {
                                                    if !chinook_passenger_should_follow_attack(
                                                        pass_ai.get_current_victim().is_some(),
                                                    ) {
                                                        continue;
                                                    }
                                                    pass_ai.ai_attack_object_id(
                                                        victim_id,
                                                        999,
                                                        CommandSourceType::FromAi,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if self.flight_status == ChinookFlightStatus::DoingCombatDrop {
                    if !self.combat_drop_started {
                        if !self.start_combat_drop() {
                            self.flight_status = ChinookFlightStatus::Flying;
                        }
                    }

                    let owner_dead = guard.is_effectively_dead();
                    if owner_dead {
                        self.finish_combat_drop(true);
                    } else if self.combat_drop_state.is_some() {
                        if self.update_combat_drop() {
                            self.finish_combat_drop(false);
                        }
                    }
                }
            }
        });

        self.update_rotor_wash();

        self.base.update();
        Ok(())
    }

    fn update_flight_status(&mut self, ai: &mut dyn AIUpdateInterface) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }
        // C++ ChinookTakeoffOrLandingState uses a 3-unit 3D pad dest, not height-only.
        self.update_machine_state(ai);
    }
}
