//! Chinook command dispatch and passenger attack forwarding.

use super::{
    ChinookAIState, ChinookAIUpdate, ChinookFlightStatus, chinook_dist_sqr,
    chinook_evac_needs_takeoff_first, chinook_kind_of_can_attack, dual_world_registry_unavailable,
};
use crate::action_manager::{ActionManager, CanEnterType};
use crate::ai::{AiCommandParams, AiCommandType, CommandSourceType};
use crate::common::{Coord3D, INVALID_ID, KindOf, ObjectID};
use crate::helpers::{TheGameLogic, ThePartitionManager};
use crate::modules::ai_state_runtime::{AiStateRuntime, AiUpdateRuntimeAdapter};
use crate::modules::{AIUpdateInterface, AIUpdateInterfaceExt, ContainModuleInterfaceExt};
use crate::object::unit::UnitAiStateRuntime;

impl ChinookAIUpdate {
    pub fn private_get_repaired_for_owner(
        &mut self,
        owner: &std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
        repair_depot_id: ObjectID,
        cmd_source: CommandSourceType,
        ai: &mut dyn AiStateRuntime,
    ) {
        if dual_world_registry_unavailable()
            || matches!(
                self.flight_status,
                ChinookFlightStatus::Landing | ChinookFlightStatus::Landed
            )
        {
            return;
        }
        let Some(repair_depot) = TheGameLogic::find_object_by_id(repair_depot_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(repair_guard)) = (owner.read(), repair_depot.read()) else {
            return;
        };
        if !ActionManager::can_get_repaired_at(&*owner_guard, &*repair_guard, cmd_source) {
            return;
        }
        drop(owner_guard);
        self.set_airfield_for_healing(repair_depot_id);
        let mut pos = *repair_guard.get_position();
        let mut tmp = pos;
        let mut options = crate::helpers::FindPositionOptions::default();
        options.max_radius = repair_guard
            .get_geometry_info()
            .get_bounding_circle_radius()
            * 100.0;
        if let Some(partition) = ThePartitionManager::get() {
            if partition.find_position_around_with_options(&pos, &options, &mut tmp) {
                pos = tmp;
            }
        }
        drop(repair_guard);
        self.set_my_state(
            ChinookAIState::MoveToAndLand,
            None,
            Some(pos),
            cmd_source,
            ai,
        );
    }

    pub fn private_combat_drop_for_owner(
        &mut self,
        owner: &std::sync::Arc<std::sync::RwLock<crate::object::Object>>,
        target_id: Option<ObjectID>,
        pos: Coord3D,
        cmd_source: CommandSourceType,
        ai: &mut dyn AiStateRuntime,
    ) {
        if dual_world_registry_unavailable() {
            return;
        }
        let target = target_id.and_then(TheGameLogic::find_object_by_id);
        if let Some(target_obj) = target.as_ref() {
            if cmd_source == CommandSourceType::FromPlayer {
                let allowed = owner
                    .read()
                    .ok()
                    .and_then(|owner_guard| {
                        let target_guard = target_obj.read().ok()?;
                        Some(ActionManager::can_enter_object(
                            &*owner_guard,
                            &*target_guard,
                            cmd_source,
                            CanEnterType::CombatDropInto,
                        ))
                    })
                    .unwrap_or(false);
                if !allowed {
                    return;
                }
            }
        }
        let mut local_pos = pos;
        if target.is_none() {
            let mut tmp = local_pos;
            let mut options = crate::helpers::FindPositionOptions::default();
            if let Ok(owner_guard) = owner.read() {
                options.max_radius =
                    owner_guard.get_geometry_info().get_bounding_circle_radius() * 100.0;
            }
            if let Some(partition) = ThePartitionManager::get() {
                if partition.find_position_around_with_options(&local_pos, &options, &mut tmp) {
                    local_pos = tmp;
                }
            }
        }
        self.combat_drop_target = target_id;
        self.combat_drop_pos = local_pos;
        self.set_my_state(
            ChinookAIState::MoveToCombatDrop,
            target_id,
            Some(local_pos),
            cmd_source,
            ai,
        );
    }

    pub fn handle_command_with_runtime(
        &mut self,
        params: &AiCommandParams,
        runtime: &mut UnitAiStateRuntime<'_>,
        driver: &mut crate::ai::states::AIStateMachineDriver<'_>,
    ) -> bool {
        // Wave 349: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        self.set_airfield_for_healing(INVALID_ID);

        let owner = runtime.owner();
        if owner.is_none() {
            return false;
        }
        let dead = owner
            .as_ref()
            .and_then(|owner| owner.read().ok().map(|guard| guard.is_effectively_dead()))
            .unwrap_or(false);
        let mood = runtime.get_mood_matrix_value();
        let asleep = (mood & crate::ai::mood_matrix_parameters::CONTROLLER_AI) != 0
            && (mood & crate::ai::mood_matrix_parameters::MOOD_SLEEP) != 0
            && params.cmd != AiCommandType::MoveToPositionEvenIfSleeping;
        if dead || asleep {
            return true;
        }

        if matches!(
            self.flight_status,
            ChinookFlightStatus::TakingOff
                | ChinookFlightStatus::Landing
                | ChinookFlightStatus::DoingCombatDrop
        ) {
            self.pending_command = Some(params.clone());
            return true;
        }

        match params.cmd {
            AiCommandType::Idle | AiCommandType::Busy | AiCommandType::FollowExitProductionPath => {
                self.pending_command = None;
                false
            }
            AiCommandType::MoveToPositionAndEvacuate
            | AiCommandType::MoveToPositionAndEvacuateAndExit => {
                let Some(owner) = owner.as_ref() else {
                    return true;
                };
                let Ok(owner_guard) = owner.read() else {
                    return true;
                };
                let dist_sqr = chinook_dist_sqr(owner_guard.get_position(), &params.pos);
                if chinook_evac_needs_takeoff_first(
                    self.flight_status == ChinookFlightStatus::Landed,
                    dist_sqr,
                ) {
                    self.pending_command = Some(params.clone());
                    self.set_my_state(
                        ChinookAIState::TakingOff,
                        None,
                        None,
                        CommandSourceType::FromAi,
                        runtime,
                    );
                    return true;
                }
                let evac_state = if params.cmd == AiCommandType::MoveToPositionAndEvacuate {
                    ChinookAIState::MoveToAndEvac
                } else {
                    ChinookAIState::MoveToAndEvacAndExitInit
                };
                self.set_my_state(
                    evac_state,
                    None,
                    Some(params.pos),
                    CommandSourceType::FromAi,
                    runtime,
                );
                true
            }
            AiCommandType::Exit | AiCommandType::Evacuate => {
                if self.flight_status != ChinookFlightStatus::Landed {
                    self.pending_command = Some(params.clone());
                    self.set_my_state(
                        ChinookAIState::Landing,
                        None,
                        None,
                        CommandSourceType::FromAi,
                        runtime,
                    );
                    return true;
                }
                self.pending_command = None;
                false
            }
            _ => {
                if self.flight_status != ChinookFlightStatus::Flying {
                    self.pending_command = Some(params.clone());
                    self.set_my_state(
                        ChinookAIState::TakingOff,
                        None,
                        None,
                        CommandSourceType::FromAi,
                        runtime,
                    );
                    return true;
                }
                self.pending_command = None;
                false
            }
        }
    }

    pub fn handle_command(
        &mut self,
        params: &AiCommandParams,
        ai: &mut dyn AIUpdateInterface,
    ) -> bool {
        let mut runtime = AiUpdateRuntimeAdapter(ai);
        // Wave 349: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        self.set_airfield_for_healing(INVALID_ID);

        let dead = TheGameLogic::find_object_by_id(self.object_id)
            .and_then(|owner| owner.read().ok().map(|guard| guard.is_effectively_dead()))
            .unwrap_or(false);
        let mood = runtime.get_mood_matrix_value();
        let asleep = (mood & crate::ai::mood_matrix_parameters::CONTROLLER_AI) != 0
            && (mood & crate::ai::mood_matrix_parameters::MOOD_SLEEP) != 0
            && params.cmd != AiCommandType::MoveToPositionEvenIfSleeping;
        if dead || asleep {
            return true;
        }

        if matches!(
            self.flight_status,
            ChinookFlightStatus::TakingOff
                | ChinookFlightStatus::Landing
                | ChinookFlightStatus::DoingCombatDrop
        ) {
            self.pending_command = Some(params.clone());
            return true;
        }

        match params.cmd {
            AiCommandType::Idle | AiCommandType::Busy | AiCommandType::FollowExitProductionPath => {
                self.pending_command = None;
                false
            }
            AiCommandType::MoveToPositionAndEvacuate
            | AiCommandType::MoveToPositionAndEvacuateAndExit => {
                let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
                    return true;
                };
                let Ok(owner_guard) = owner.read() else {
                    return true;
                };
                let dist_sqr = chinook_dist_sqr(owner_guard.get_position(), &params.pos);
                if chinook_evac_needs_takeoff_first(
                    self.flight_status == ChinookFlightStatus::Landed,
                    dist_sqr,
                ) {
                    self.pending_command = Some(params.clone());
                    self.set_my_state(
                        ChinookAIState::TakingOff,
                        None,
                        None,
                        CommandSourceType::FromAi,
                        &mut runtime,
                    );
                    return true;
                }
                let evac_state = if params.cmd == AiCommandType::MoveToPositionAndEvacuate {
                    ChinookAIState::MoveToAndEvac
                } else {
                    ChinookAIState::MoveToAndEvacAndExitInit
                };
                self.set_my_state(
                    evac_state,
                    None,
                    Some(params.pos),
                    CommandSourceType::FromAi,
                    &mut runtime,
                );
                true
            }
            AiCommandType::Exit | AiCommandType::Evacuate => {
                if self.flight_status != ChinookFlightStatus::Landed {
                    self.pending_command = Some(params.clone());
                    self.set_my_state(
                        ChinookAIState::Landing,
                        None,
                        None,
                        CommandSourceType::FromAi,
                        &mut runtime,
                    );
                    return true;
                }
                self.pending_command = None;
                false
            }
            _ => {
                if self.flight_status != ChinookFlightStatus::Flying {
                    self.pending_command = Some(params.clone());
                    self.set_my_state(
                        ChinookAIState::TakingOff,
                        None,
                        None,
                        CommandSourceType::FromAi,
                        &mut runtime,
                    );
                    return true;
                }
                self.pending_command = None;
                false
            }
        }
    }

    pub fn private_idle(&mut self, cmd_source: CommandSourceType) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |guard| {
            if let Some(contain) = guard.get_contain() {
                if let Some(rider_id) = contain.friend_get_rider() {
                    if let Some(rider) = TheGameLogic::find_object_by_id(rider_id) {
                        if let Ok(rider_guard) = rider.read() {
                            if let Some(ai) = rider_guard.get_ai_update_interface() {
                                ai.ai_idle(cmd_source);
                            }
                        }
                    }
                }
            }
        });
        self.base.private_idle(cmd_source);
    }

    pub fn private_dock(&mut self, dock_id: Option<ObjectID>, cmd_source: CommandSourceType) {
        self.base.private_dock(dock_id, cmd_source);
    }

    pub fn private_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |guard| {
            if !chinook_kind_of_can_attack(&*guard) {
                return;
            }
            if let Some(contain) = guard.get_contain() {
                if matches!(
                    cmd_source,
                    CommandSourceType::FromPlayer | CommandSourceType::FromScript
                ) {
                    let passengers = contain.get_contained_objects();
                    for passenger_id in passengers {
                        if !contain.is_passenger_allowed_to_fire(Some(passenger_id)) {
                            continue;
                        }
                        let Some(passenger) = TheGameLogic::find_object_by_id(passenger_id) else {
                            continue;
                        };
                        let Ok(pass_guard) = passenger.read() else {
                            continue;
                        };
                        if !pass_guard.is_kind_of(KindOf::Infantry) {
                            continue;
                        }
                        if pass_guard.is_kind_of(KindOf::PortableStructure)
                            && (pass_guard
                                .is_disabled_by_type(crate::common::DisabledType::DisabledHacked)
                                || pass_guard
                                    .is_disabled_by_type(crate::common::DisabledType::DisabledEmp)
                                || pass_guard.is_disabled_by_type(
                                    crate::common::DisabledType::DisabledSubdued,
                                )
                                || pass_guard
                                    .is_disabled_by_type(crate::common::DisabledType::Paralyzed))
                        {
                            continue;
                        }
                        if let Some(ai) = pass_guard.get_ai_update_interface() {
                            ai.ai_attack_object_id(victim_id, max_shots_to_fire, cmd_source);
                        }
                    }
                    self.tell_portable_structure_to_attack_with_me(
                        victim_id,
                        max_shots_to_fire,
                        cmd_source,
                    );
                }
            }
        });
    }

    pub fn private_force_attack_object(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let victim = TheGameLogic::find_object_by_id(victim_id);
        if victim.is_none() {
            return;
        }
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |guard| {
            if !chinook_kind_of_can_attack(&*guard) {
                return;
            }
            if let Some(contain) = guard.get_contain() {
                if matches!(
                    cmd_source,
                    CommandSourceType::FromPlayer | CommandSourceType::FromScript
                ) {
                    let passengers = contain.get_contained_objects();
                    for passenger_id in passengers {
                        if !contain.is_passenger_allowed_to_fire(Some(passenger_id)) {
                            continue;
                        }
                        let Some(passenger) = TheGameLogic::find_object_by_id(passenger_id) else {
                            continue;
                        };
                        let Ok(pass_guard) = passenger.read() else {
                            continue;
                        };
                        if !pass_guard.is_kind_of(KindOf::Infantry) {
                            continue;
                        }
                        if pass_guard.is_kind_of(KindOf::PortableStructure)
                            && (pass_guard
                                .is_disabled_by_type(crate::common::DisabledType::DisabledHacked)
                                || pass_guard
                                    .is_disabled_by_type(crate::common::DisabledType::DisabledEmp)
                                || pass_guard.is_disabled_by_type(
                                    crate::common::DisabledType::DisabledSubdued,
                                )
                                || pass_guard
                                    .is_disabled_by_type(crate::common::DisabledType::Paralyzed))
                        {
                            continue;
                        }
                        if let Some(ai) = pass_guard.get_ai_update_interface() {
                            if let Some(victim_arc) = victim.as_ref() {
                                ai.ai_force_attack_object(
                                    victim_arc.read().ok().map(|g| g.get_id()).unwrap_or(0),
                                    max_shots_to_fire,
                                    cmd_source,
                                );
                            }
                        }
                    }
                }
                if matches!(
                    cmd_source,
                    CommandSourceType::FromPlayer | CommandSourceType::FromScript
                ) {
                    if let Some(rider_id) = contain.friend_get_rider() {
                        if let Some(rider) = TheGameLogic::find_object_by_id(rider_id) {
                            if let Ok(rider_guard) = rider.read() {
                                if rider_guard.is_kind_of(KindOf::PortableStructure)
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledHacked,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledEmp,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledSubdued,
                                    )
                                    && !rider_guard
                                        .is_disabled_by_type(crate::common::DisabledType::Paralyzed)
                                {
                                    if let Some(ai) = rider_guard.get_ai_update_interface() {
                                        if let Some(victim_arc) = victim.as_ref() {
                                            ai.ai_force_attack_object(
                                                victim_arc
                                                    .read()
                                                    .ok()
                                                    .map(|g| g.get_id())
                                                    .unwrap_or(0),
                                                max_shots_to_fire,
                                                cmd_source,
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn private_attack_position(
        &self,
        pos: &Coord3D,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |guard| {
            if !chinook_kind_of_can_attack(&*guard) {
                return;
            }
            if let Some(contain) = guard.get_contain() {
                if matches!(
                    cmd_source,
                    CommandSourceType::FromPlayer | CommandSourceType::FromScript
                ) {
                    let passengers = contain.get_contained_objects();
                    for passenger_id in passengers {
                        if !contain.is_passenger_allowed_to_fire(Some(passenger_id)) {
                            continue;
                        }
                        let Some(passenger) = TheGameLogic::find_object_by_id(passenger_id) else {
                            continue;
                        };
                        let Ok(pass_guard) = passenger.read() else {
                            continue;
                        };
                        if !pass_guard.is_kind_of(KindOf::Infantry) {
                            continue;
                        }
                        if pass_guard.is_kind_of(KindOf::PortableStructure)
                            && (pass_guard
                                .is_disabled_by_type(crate::common::DisabledType::DisabledHacked)
                                || pass_guard
                                    .is_disabled_by_type(crate::common::DisabledType::DisabledEmp)
                                || pass_guard.is_disabled_by_type(
                                    crate::common::DisabledType::DisabledSubdued,
                                )
                                || pass_guard
                                    .is_disabled_by_type(crate::common::DisabledType::Paralyzed))
                        {
                            continue;
                        }
                        if let Some(ai) = pass_guard.get_ai_update_interface() {
                            ai.ai_attack_position(pos, max_shots_to_fire, cmd_source);
                        }
                    }
                }
                if matches!(
                    cmd_source,
                    CommandSourceType::FromPlayer | CommandSourceType::FromScript
                ) {
                    if let Some(rider_id) = contain.friend_get_rider() {
                        if let Some(rider) = TheGameLogic::find_object_by_id(rider_id) {
                            if let Ok(rider_guard) = rider.read() {
                                if rider_guard.is_kind_of(KindOf::PortableStructure)
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledHacked,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledEmp,
                                    )
                                    && !rider_guard.is_disabled_by_type(
                                        crate::common::DisabledType::DisabledSubdued,
                                    )
                                    && !rider_guard
                                        .is_disabled_by_type(crate::common::DisabledType::Paralyzed)
                                {
                                    if let Some(ai) = rider_guard.get_ai_update_interface() {
                                        ai.ai_attack_position(pos, max_shots_to_fire, cmd_source);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    fn tell_portable_structure_to_attack_with_me(
        &self,
        victim_id: ObjectID,
        max_shots_to_fire: i32,
        cmd_source: CommandSourceType,
    ) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |guard| {
            if let Some(contain) = guard.get_contain() {
                if let Some(rider_id) = contain.friend_get_rider() {
                    if let Some(rider) = TheGameLogic::find_object_by_id(rider_id) {
                        if let Ok(rider_guard) = rider.read() {
                            if rider_guard.is_kind_of(KindOf::PortableStructure)
                                && !rider_guard.is_disabled_by_type(
                                    crate::common::DisabledType::DisabledHacked,
                                )
                                && !rider_guard
                                    .is_disabled_by_type(crate::common::DisabledType::DisabledEmp)
                                && !rider_guard.is_disabled_by_type(
                                    crate::common::DisabledType::DisabledSubdued,
                                )
                                && !rider_guard
                                    .is_disabled_by_type(crate::common::DisabledType::Paralyzed)
                            {
                                if let Some(ai) = rider_guard.get_ai_update_interface() {
                                    ai.ai_attack_object_id(
                                        victim_id,
                                        max_shots_to_fire,
                                        cmd_source,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        });
    }

    pub fn private_get_repaired(
        &mut self,
        repair_depot_id: ObjectID,
        cmd_source: CommandSourceType,
        ai: &mut dyn AiStateRuntime,
    ) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if matches!(
            self.flight_status,
            ChinookFlightStatus::Landing | ChinookFlightStatus::Landed
        ) {
            return;
        }
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        let Some(repair_depot) = TheGameLogic::find_object_by_id(repair_depot_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(repair_guard)) = (owner.read(), repair_depot.read()) else {
            return;
        };
        if !ActionManager::can_get_repaired_at(&*owner_guard, &*repair_guard, cmd_source) {
            return;
        }

        self.set_airfield_for_healing(repair_depot_id);
        let mut pos = *repair_guard.get_position();
        let mut tmp = pos;
        let mut options = crate::helpers::FindPositionOptions::default();
        options.max_radius = repair_guard
            .get_geometry_info()
            .get_bounding_circle_radius()
            * 100.0;
        if let Some(partition) = ThePartitionManager::get() {
            if partition.find_position_around_with_options(&pos, &options, &mut tmp) {
                pos = tmp;
            }
        }
        self.set_my_state(
            ChinookAIState::MoveToAndLand,
            None,
            Some(pos),
            cmd_source,
            ai,
        );
    }

    pub fn private_combat_drop(
        &mut self,
        target_id: Option<ObjectID>,
        pos: Coord3D,
        cmd_source: CommandSourceType,
        ai: &mut dyn AiStateRuntime,
    ) {
        // Wave 349: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let target = target_id.and_then(TheGameLogic::find_object_by_id);
        if let Some(target_obj) = target.as_ref() {
            if cmd_source == CommandSourceType::FromPlayer {
                let allowed = TheGameLogic::find_object_by_id(self.object_id)
                    .and_then(|owner| {
                        let owner_guard = owner.read().ok()?;
                        let target_guard = target_obj.read().ok()?;
                        Some(ActionManager::can_enter_object(
                            &*owner_guard,
                            &*target_guard,
                            cmd_source,
                            CanEnterType::CombatDropInto,
                        ))
                    })
                    .unwrap_or(false);
                if !allowed {
                    return;
                }
            }
        }

        let mut local_pos = pos;
        if target.is_none() {
            let mut tmp = local_pos;
            let mut options = crate::helpers::FindPositionOptions::default();
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(
                self.object_id,
                |owner_guard| {
                    options.max_radius =
                        owner_guard.get_geometry_info().get_bounding_circle_radius() * 100.0;
                },
            );
            if let Some(partition) = ThePartitionManager::get() {
                if partition.find_position_around_with_options(&local_pos, &options, &mut tmp) {
                    local_pos = tmp;
                }
            }
        }

        self.combat_drop_target = target_id;
        self.combat_drop_pos = local_pos;
        self.set_my_state(
            ChinookAIState::MoveToCombatDrop,
            target_id,
            Some(local_pos),
            cmd_source,
            ai,
        );
    }
}
