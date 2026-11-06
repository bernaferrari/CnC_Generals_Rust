//! C++ support-state dispatcher and per-object update loop.
use super::super::super::*;
use super::guard_states::GUARD_CHASE_PHASE_INNER;
use super::special_abilities::{LeftoverSaTick, clear_raising_flag_model};

const INTERACT_RANGE: f32 = crate::game_logic::host_repair::HOST_REPAIR_INTERACT_RANGE;
const SPECIAL_ABILITY_RANGE_PADDING: f32 = 4.0;
// Authored capture durations are integral milliseconds, but the host
// channel stores the running remainder as `f32` seconds.  A sequence
// such as 20.0 - 19.9 - 0.1 can otherwise leave one floating-point
// ulp and defer the C++ frame-boundary trigger by another logic tick.
// This is far below one authored millisecond (and one 30 Hz logic
// frame), so it only removes representation residue—not gameplay
// time.
const CAPTURE_CHANNEL_COMPLETE_EPSILON: f32 = 0.000_1;
const HEAL_RATE: f32 = crate::game_logic::host_repair::HOST_HEAL_RATE_HP_PER_SEC;

fn same_live_object_birth(world: &GameLogic, id: ObjectId, birth: &std::sync::Weak<()>) -> bool {
    world.objects.get(&id).is_some_and(|object| {
        birth.ptr_eq(&std::sync::Arc::downgrade(
            &object.visual_draw_identity_token,
        ))
    })
}

mod capture_state;
mod enter_dock_state;
mod repair_states;
#[path = "update/rider_scuttle.rs"]
mod rider_scuttle;
mod special_ability_state;
mod supply_cycle_states;
#[path = "update/support_order_transitions.rs"]
mod support_order_transitions;

impl GameLogic {
    pub(in super::super::super) fn update_support_states(
        &mut self,
        object_ids: &[ObjectId],
        dt: f32,
    ) -> gamelogic::GameLogicResult<()> {
        self.update_support_states_with_effects(object_ids, dt, &mut Default::default())
    }

    pub(in super::super::super) fn update_support_states_with_effects(
        &mut self,
        object_ids: &[ObjectId],
        dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let tree_move_frame = self.frame;
        // Object::set_ai_state records DockUpdate's synchronous cancellation
        // callback as an epoch. Reconcile it at this regular mutable world
        // update boundary so a retasked active docker is released even when
        // no later selector or dock claimant touches that module.
        self.sync_host_dock_queue_cancellations();
        self.apply_pending_host_dock_cancellation_effects();
        self.update_leftover_laser_guided_channels_with_effects(dt, effects);
        self.expire_leftover_disable_fx(effects);

        for &object_id in object_ids {
            let snapshot = match self.objects.get(&object_id) {
                Some(obj) => (
                    obj.ai_state().clone(),
                    obj.team,
                    obj.owner_player_id,
                    obj.get_position(),
                    obj.target,
                    obj.active_guard_target(),
                    obj.can_move(),
                    obj.can_attack(),
                    obj.health.current,
                    obj.health.maximum,
                    obj.selection_radius,
                    obj.is_alive(),
                ),
                None => continue,
            };

            let (
                ai_state,
                team,
                owner_player_id,
                position,
                target_id,
                guard_target,
                can_move,
                can_attack,
                health_current,
                health_maximum,
                selection_radius,
                is_alive,
            ) = snapshot;
            if !is_alive {
                continue;
            }
            self.cancel_replaced_ability_channels_with_effects(object_id, &ai_state, effects);
            // The pre-locomotor invocation owns both temporary exit movement
            // and its suspended Guard. Do not observe this frame's movement
            // a second time from the later support pass.
            if matches!(ai_state, AIState::GuardingArea | AIState::GuardingObject)
                || self
                    .objects
                    .get(&object_id)
                    .is_some_and(Object::has_guard_quick_exit_overlay)
            {
                self.tick_host_guard_states_with_effects(&[object_id], effects)?;
                continue;
            }
            let quick_exit_finished =
                self.finish_quick_exit_if_due(object_id, guard_target, effects)?;
            let ai_state = if quick_exit_finished {
                self.objects
                    .get(&object_id)
                    .map(|o| o.ai_state().clone())
                    .unwrap_or(AIState::GuardingObject)
            } else {
                ai_state
            };
            match ai_state {
                AIState::Exiting => {
                    self.update_owned_passenger_exit_with_effects(object_id, effects);
                }
                AIState::GuardingArea | AIState::GuardingObject => {
                    self.tick_host_guard_states_with_effects(&[object_id], effects)?;
                }
                AIState::Repairing => {
                    self.tick_repairing_support_state(
                        object_id,
                        position,
                        target_id,
                        can_move,
                        selection_radius,
                        is_alive,
                        dt,
                        effects,
                    )?;
                }
                state @ (AIState::SeekingRepair | AIState::SeekingHealing) => {
                    self.tick_seeking_repair_support_state(
                        object_id,
                        ai_state,
                        state,
                        position,
                        target_id,
                        can_move,
                        health_current,
                        health_maximum,
                        selection_radius,
                        is_alive,
                        dt,
                        effects,
                    )?;
                }
                state @ (AIState::Entering | AIState::Docking) => {
                    self.tick_entering_docking_support_state(
                        object_id,
                        state,
                        team,
                        owner_player_id,
                        position,
                        target_id,
                        can_move,
                        selection_radius,
                        is_alive,
                        tree_move_frame,
                        effects,
                    )?;
                }
                AIState::Capturing => {
                    self.tick_capturing_support_state(
                        object_id,
                        team,
                        owner_player_id,
                        target_id,
                        can_move,
                        is_alive,
                        dt,
                        effects,
                    )?;
                }
                AIState::SpecialAbility => {
                    self.tick_special_ability_support_state(
                        object_id,
                        team,
                        owner_player_id,
                        position,
                        target_id,
                        can_move,
                        selection_radius,
                        is_alive,
                        dt,
                        effects,
                    )?;
                }
                AIState::Gathering => {
                    self.tick_gathering_support_state(
                        object_id,
                        team,
                        owner_player_id,
                        position,
                        target_id,
                        can_move,
                        is_alive,
                        dt,
                        tree_move_frame,
                        effects,
                    )?;
                }
                AIState::ReturningResources => {
                    self.tick_returning_resources_support_state(
                        object_id,
                        team,
                        owner_player_id,
                        position,
                        target_id,
                        can_move,
                        is_alive,
                        effects,
                    )?;
                }
                AIState::Docked | AIState::Garrisoned => {
                    self.tick_docked_support_state(
                        object_id,
                        ai_state,
                        target_id,
                        is_alive,
                        tree_move_frame,
                        effects,
                    )?;
                }
                AIState::Idle => {
                    // C++ SupplyTruckStateMachine Idle: isForcedIntoWantingState
                    // → ST_WANTING. Regrouping success → ST_WANTING
                    // (SupplyTruckAIUpdate.cpp:383-418).
                    self.tick_supply_force_wanting(
                        object_id,
                        team,
                        owner_player_id,
                        position,
                        can_move,
                    )?;
                }
                AIState::Attacking => {
                    // C++ parent stays AI_GUARD; live peels to Attacking for fire.
                    // hasAttackedMeAndICanReturnFire is registered on INNER.
                    let phase = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.guard_chase_phase())
                        .unwrap_or(GuardChasePhase::None);
                    if phase == GUARD_CHASE_PHASE_INNER
                        && can_attack
                        && self.try_guard_last_attacker(object_id, team, effects)?
                    {
                        continue;
                    }
                    let _ = self.tick_guard_chase_exits(object_id);
                }

                _ => {}
            }
        }

        // AIUpdate runs in PHASE_INITIAL; OpenContain::update uses the later
        // PHASE_NORMAL. Entries processed by the loop above must therefore be
        // cleared before the next frame's script evaluation.
        self.update_open_contain_state();
        self.tick_scuttled_riders_with_effects(effects);

        // C++ HealContain::update + TunnelContain::update → TunnelTracker::healObjects.
        self.tick_heal_contain_and_tunnel_with_effects(effects);
        // C++ TunnelContain::update nemesis + AITNGuard::lookForInnerTarget.
        self.tick_tunnel_network_nemesis_with_effects(effects);
        Ok(())
    }
}
