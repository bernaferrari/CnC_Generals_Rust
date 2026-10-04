//! Flight-state transitions, landing, takeoff and evacuation.

use super::{
    CHINOOK_ARRIVE_THRESH_SQR, CHINOOK_BIGNUM, ChinookAIState, ChinookAIUpdate,
    ChinookFlightStatus, chinook_dist_sqr, chinook_move_to_bldg_arrived,
    chinook_move_to_bldg_preferred_height,
};
use crate::ai::CommandSourceType;
use crate::common::{Coord3D, KindOf, ObjectID, ObjectStatusMaskType, PathfindLayerEnum};
use crate::helpers::{TheGameLogic, ThePartitionManager, TheTerrainLogic};
use crate::modules::AIUpdateInterface;

impl ChinookAIUpdate {
    fn set_flight_status(&mut self, status: ChinookFlightStatus, ai: &mut dyn AIUpdateInterface) {
        self.flight_status = status;
        match status {
            ChinookFlightStatus::Landed => {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Taxiing);
            }
            ChinookFlightStatus::TakingOff | ChinookFlightStatus::Landing => {
                let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Normal);
                let _ = ai.set_allow_invalid_position(false);
                ai.with_cur_locomotor_mut(&mut |loco| {
                    loco.set_precise_z_pos(true);
                    loco.set_ultra_accurate(true);
                });
            }
            ChinookFlightStatus::Flying => {
                ai.with_cur_locomotor_mut(&mut |loco| {
                    loco.set_precise_z_pos(false);
                    loco.set_ultra_accurate(false);
                });
            }
            _ => {}
        }
    }

    /// C++ `ChinookAIUpdate::setMyState`.
    pub(super) fn set_my_state(
        &mut self,
        state: ChinookAIState,
        target: Option<ObjectID>,
        pos: Option<Coord3D>,
        _cmd_source: CommandSourceType,
        ai: &mut dyn AIUpdateInterface,
    ) {
        self.machine_state = state;
        self.goal_object = target;
        if let Some(pos) = pos {
            self.goal_pos = pos;
        }
        self.enter_machine_state(ai);
    }

    fn enter_machine_state(&mut self, ai: &mut dyn AIUpdateInterface) {
        match self.machine_state {
            ChinookAIState::None => {}
            ChinookAIState::TakingOff | ChinookAIState::TakeoffAndExit => {
                self.enter_takeoff_or_landing(false, ai);
            }
            ChinookAIState::Landing
            | ChinookAIState::LandAndEvac
            | ChinookAIState::LandAndEvacAndExit => {
                self.enter_takeoff_or_landing(true, ai);
            }
            ChinookAIState::MoveToAndLand
            | ChinookAIState::MoveToAndEvac
            | ChinookAIState::MoveToAndEvacAndExit => {
                let _ = ai.set_movement_target(&self.goal_pos);
            }
            ChinookAIState::MoveToAndEvacAndExitInit => {
                let _ =
                    crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |guard| {
                        self.record_original_position(*guard.get_position());
                    });
                self.set_my_state(
                    ChinookAIState::MoveToAndEvacAndExit,
                    None,
                    Some(self.goal_pos),
                    CommandSourceType::FromAi,
                    ai,
                );
            }
            ChinookAIState::EvacAndTakeoff | ChinookAIState::EvacAndExit => {
                self.enter_evacuate();
                let next = if self.machine_state == ChinookAIState::EvacAndTakeoff {
                    ChinookAIState::TakingOff
                } else {
                    ChinookAIState::TakeoffAndExit
                };
                self.set_my_state(next, None, None, CommandSourceType::FromAi, ai);
            }
            ChinookAIState::HeadOffMap => {
                self.enter_head_off_map(ai);
            }
            ChinookAIState::MoveToCombatDrop => {
                self.enter_move_to_bldg(ai);
            }
            ChinookAIState::DoCombatDrop => {
                self.flight_status = ChinookFlightStatus::DoingCombatDrop;
                self.combat_drop_started = false;
                self.combat_drop_state = None;
            }
        }
    }

    /// C++ `ChinookEvacuateState::onEnter`: `removeAllContained(FALSE)` + `team->setActive()`.
    fn enter_evacuate(&mut self) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        if let Ok(owner_guard) = owner.read() {
            if let Some(contain) = owner_guard.get_contain() {
                if let Ok(mut contain_guard) = contain.lock() {
                    let _ = contain_guard.remove_all_contained(false);
                }
            }
            if let Some(team) = owner_guard.get_team() {
                if let Ok(mut team_guard) = team.write() {
                    team_guard.set_active();
                }
            }
        }
    }

    /// C++ `ChinookHeadOffMapState::onEnter`.
    fn enter_head_off_map(&mut self, ai: &mut dyn AIUpdateInterface) {
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
            self.object_id,
            |owner_guard| {
                owner_guard.set_status(ObjectStatusMaskType::RIDER8, true);
            },
        );
        let _ = ai.set_movement_target(&self.original_pos);
        let _ = ai.set_allow_invalid_position(true);
    }

    /// C++ `ChinookTakeoffOrLandingState::onEnter`.
    fn enter_takeoff_or_landing(&mut self, landing: bool, ai: &mut dyn AIUpdateInterface) {
        self.takeoff_landing_is_landing = landing;
        self.set_flight_status(
            if landing {
                ChinookFlightStatus::Landing
            } else {
                ChinookFlightStatus::TakingOff
            },
            ai,
        );
        if landing {
            // C++ ChinookTakeoffOrLandingState::onEnter: while (ai->loseOneBox()).
            while self.base.lose_one_box() {}
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        // Position search reads every admitted object, including this one.
        // Capture its inputs before invoking callbacks instead of holding the
        // owner write guard across a synchronous registry traversal.
        let (physics, mut dest, radius) = {
            let Ok(owner) = owner.read() else {
                return;
            };
            (
                owner.get_physics(),
                *owner.get_position(),
                owner.get_geometry_info().get_bounding_circle_radius(),
            )
        };
        let original_z = dest.z;
        if let Some(physics) = physics {
            if let Ok(mut physics_guard) = physics.access() {
                physics_guard.scrub_velocity_2d(0.0);
            }
        }

        let preferred = ai.get_preferred_height().unwrap_or(0.0);
        if let Some(terrain) = TheTerrainLogic::get() {
            let layer = terrain.get_highest_layer_for_destination(&dest);
            dest.z = terrain.get_layer_height(dest.x, dest.y, layer);
            if landing {
                let mut tmp = dest;
                let mut options = crate::helpers::FindPositionOptions::default();
                options.max_radius = radius * 100.0;
                if let Some(partition) = ThePartitionManager::get() {
                    if partition.find_position_around_with_options(&dest, &options, &mut tmp) {
                        dest = tmp;
                        let ai_store = crate::ai::the_ai();
                        if let Ok(ai_guard) = ai_store.read() {
                            if let Some(pathfinder) = ai_guard.pathfinder() {
                                if let Ok(pf) = pathfinder.read() {
                                    if let Ok(owner) = owner.read() {
                                        pf.adjust_to_landing_destination(&owner, &mut dest);
                                    }
                                }
                            }
                        }
                    }
                }
                let mut tmp = dest;
                tmp.z = original_z;
                let layer = terrain.get_highest_layer_for_destination(&tmp);
                dest.z = terrain.get_layer_height(dest.x, dest.y, layer);
                if let Ok(mut owner) = owner.write() {
                    owner.set_layer(layer);
                }
            } else {
                dest.z += preferred;
                if let Ok(mut owner) = owner.write() {
                    owner.set_layer(PathfindLayerEnum::Ground);
                }
            }
        } else if !landing {
            dest.z += preferred;
        }
        self.takeoff_landing_dest = dest;
    }

    /// C++ `ChinookTakeoffOrLandingState::onExit`.
    fn exit_takeoff_or_landing(&mut self, landing: bool, ai: &mut dyn AIUpdateInterface) {
        self.set_flight_status(
            if landing {
                ChinookFlightStatus::Landed
            } else {
                ChinookFlightStatus::Flying
            },
            ai,
        );
        let owner_dead = TheGameLogic::find_object_by_id(self.object_id)
            .and_then(|owner| owner.read().ok().map(|g| g.is_effectively_dead()))
            .unwrap_or(false);
        ai.with_cur_locomotor_mut(&mut |loco| {
            loco.set_precise_z_pos(false);
            loco.set_ultra_accurate(false);
            if !owner_dead {
                loco.set_max_lift(CHINOOK_BIGNUM);
            }
        });
        if landing {
            let _ = ai.choose_locomotor_set(crate::common::LocomotorSetType::Taxiing);
        } else {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
                self.object_id,
                |owner_guard| {
                    owner_guard.set_layer(PathfindLayerEnum::Ground);
                },
            );
        }
    }

    /// C++ `ChinookTakeoffOrLandingState::update` — 3-unit 3D threshold.
    fn update_takeoff_or_landing(&mut self, ai: &mut dyn AIUpdateInterface) -> bool {
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return true;
        };
        let Ok(owner_guard) = owner.read() else {
            return true;
        };
        if owner_guard.is_effectively_dead() {
            return true;
        }
        ai.set_locomotor_goal_position_explicit(self.takeoff_landing_dest);
        chinook_dist_sqr(owner_guard.get_position(), &self.takeoff_landing_dest)
            <= CHINOOK_ARRIVE_THRESH_SQR
    }

    fn update_move_to_goal(&self, ai: &mut dyn AIUpdateInterface) -> bool {
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return true;
        };
        let Ok(owner_guard) = owner.read() else {
            return true;
        };
        let pos = *owner_guard.get_position();
        let dx = pos.x - self.goal_pos.x;
        let dy = pos.y - self.goal_pos.y;
        let arrived_2d = dx * dx + dy * dy <= CHINOOK_ARRIVE_THRESH_SQR;
        if !arrived_2d {
            let _ = ai.set_movement_target(&self.goal_pos);
        }
        arrived_2d
    }

    /// C++ `ChinookMoveToBldgState::onEnter`.
    fn enter_move_to_bldg(&mut self, ai: &mut dyn AIUpdateInterface) {
        ai.with_cur_locomotor_mut(&mut |loco| {
            loco.set_ultra_accurate(true);
            self.move_to_bldg_old_preferred = loco.preferred_height;
        });
        self.move_to_bldg_new_preferred = self.move_to_bldg_old_preferred;
        let mut dest_pos = self.goal_pos;
        if let Some(target_id) = self.goal_object {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |bldg_guard| {
                if !bldg_guard.is_effectively_dead() && bldg_guard.is_kind_of(KindOf::Structure) {
                    dest_pos = *bldg_guard.get_position();
                    self.move_to_bldg_new_preferred = chinook_move_to_bldg_preferred_height(
                        self.move_to_bldg_old_preferred,
                        true,
                        bldg_guard
                            .get_geometry_info()
                            .get_max_height_above_position(),
                        self.data.min_drop_height,
                    );
                }
            });
        }
        ai.with_cur_locomotor_mut(&mut |loco| {
            loco.set_preferred_height(self.move_to_bldg_new_preferred);
        });
        let ground = TheTerrainLogic::get()
            .map(|terrain| terrain.get_ground_height(dest_pos.x, dest_pos.y, None))
            .unwrap_or(0.0);
        self.move_to_bldg_dest_z = ground + self.move_to_bldg_new_preferred;
        self.goal_pos = dest_pos;
        let _ = ai.set_movement_target(&self.goal_pos);
    }

    /// C++ `ChinookMoveToBldgState::update` — 2D arrival **and** `|z-destZ|<=3`.
    fn update_move_to_bldg(&self, ai: &mut dyn AIUpdateInterface) -> bool {
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return true;
        };
        let Ok(owner_guard) = owner.read() else {
            return true;
        };
        let pos = *owner_guard.get_position();
        let dx = pos.x - self.goal_pos.x;
        let dy = pos.y - self.goal_pos.y;
        let arrived_2d = dx * dx + dy * dy <= CHINOOK_ARRIVE_THRESH_SQR;
        if !arrived_2d {
            let _ = ai.set_movement_target(&self.goal_pos);
            return false;
        }
        chinook_move_to_bldg_arrived(true, pos.z, self.move_to_bldg_dest_z)
    }

    fn exit_move_to_bldg(&mut self, ai: &mut dyn AIUpdateInterface) {
        ai.with_cur_locomotor_mut(&mut |loco| {
            loco.set_preferred_height(self.move_to_bldg_old_preferred);
            loco.set_ultra_accurate(false);
        });
    }

    /// C++ `ChinookHeadOffMapState::update`.
    fn update_head_off_map(&mut self) -> bool {
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return true;
        };
        let Ok(owner_guard) = owner.read() else {
            return true;
        };
        let Some(terrain) = TheTerrainLogic::get() else {
            return false;
        };
        let extent = terrain.get_extent_including_border();
        let pos = owner_guard.get_position();
        if pos.x < extent.lo.x || pos.x > extent.hi.x || pos.y < extent.lo.y || pos.y > extent.hi.y
        {
            drop(owner_guard);
            let _ = TheGameLogic::destroy_object_by_id(self.object_id);
            return true;
        }
        false
    }

    fn exit_head_off_map(&mut self) {
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
            self.object_id,
            |owner_guard| {
                owner_guard.set_status(ObjectStatusMaskType::RIDER8, false);
            },
        );
    }

    fn succeed_machine_state(&mut self, ai: &mut dyn AIUpdateInterface) {
        let next = match self.machine_state {
            ChinookAIState::TakingOff => {
                self.exit_takeoff_or_landing(false, ai);
                ChinookAIState::None
            }
            ChinookAIState::Landing => {
                self.exit_takeoff_or_landing(true, ai);
                ChinookAIState::None
            }
            ChinookAIState::MoveToAndLand => ChinookAIState::Landing,
            ChinookAIState::MoveToAndEvac => ChinookAIState::LandAndEvac,
            ChinookAIState::LandAndEvac => {
                self.exit_takeoff_or_landing(true, ai);
                ChinookAIState::EvacAndTakeoff
            }
            ChinookAIState::EvacAndTakeoff => ChinookAIState::TakingOff,
            ChinookAIState::MoveToAndEvacAndExit => ChinookAIState::LandAndEvacAndExit,
            ChinookAIState::LandAndEvacAndExit => {
                self.exit_takeoff_or_landing(true, ai);
                ChinookAIState::EvacAndExit
            }
            ChinookAIState::EvacAndExit => ChinookAIState::TakeoffAndExit,
            ChinookAIState::TakeoffAndExit => {
                self.exit_takeoff_or_landing(false, ai);
                ChinookAIState::HeadOffMap
            }
            ChinookAIState::HeadOffMap => {
                self.exit_head_off_map();
                ChinookAIState::None
            }
            ChinookAIState::MoveToCombatDrop => {
                self.exit_move_to_bldg(ai);
                ChinookAIState::DoCombatDrop
            }
            ChinookAIState::DoCombatDrop => ChinookAIState::None,
            ChinookAIState::MoveToAndEvacAndExitInit | ChinookAIState::None => ChinookAIState::None,
        };
        if next != self.machine_state {
            self.set_my_state(
                next,
                self.goal_object,
                Some(self.goal_pos),
                CommandSourceType::FromAi,
                ai,
            );
        } else {
            self.machine_state = ChinookAIState::None;
        }
    }

    pub(super) fn update_machine_state(&mut self, ai: &mut dyn AIUpdateInterface) {
        let done = match self.machine_state {
            ChinookAIState::None => false,
            ChinookAIState::TakingOff
            | ChinookAIState::Landing
            | ChinookAIState::LandAndEvac
            | ChinookAIState::LandAndEvacAndExit
            | ChinookAIState::TakeoffAndExit => self.update_takeoff_or_landing(ai),
            ChinookAIState::MoveToAndLand
            | ChinookAIState::MoveToAndEvac
            | ChinookAIState::MoveToAndEvacAndExit => self.update_move_to_goal(ai),
            ChinookAIState::MoveToCombatDrop => self.update_move_to_bldg(ai),
            ChinookAIState::HeadOffMap => self.update_head_off_map(),
            ChinookAIState::DoCombatDrop
            | ChinookAIState::EvacAndTakeoff
            | ChinookAIState::EvacAndExit
            | ChinookAIState::MoveToAndEvacAndExitInit => false,
        };
        if done {
            self.succeed_machine_state(ai);
        }
    }
}
