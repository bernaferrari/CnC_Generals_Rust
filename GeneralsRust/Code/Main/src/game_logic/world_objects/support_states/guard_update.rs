//! C++ AIGuard Return/Idle updates before the locomotor pass.
use super::super::super::*;
use crate::game_logic::object::unit_ai_runtime::GuardPhase;

impl GameLogic {
    pub(crate) fn tick_host_guard_states(&mut self, object_ids: &[ObjectId]) {
        for &object_id in object_ids {
            let Some(unit) = self.objects.get(&object_id) else {
                continue;
            };
            if !unit.is_alive()
                || !matches!(
                    unit.ai_state,
                    AIState::GuardingArea | AIState::GuardingObject
                )
            {
                continue;
            }
            let (
                ai_state,
                team,
                position,
                guard_position,
                guard_target,
                guard_radius,
                guard_mode,
                can_attack,
            ) = (
                unit.ai_state.clone(),
                unit.team,
                unit.get_position(),
                unit.guard_position,
                unit.guard_target,
                unit.guard_radius,
                unit.guard_mode,
                unit.can_attack(),
            );
            let now = self.frame;
            if !self
                .unit_ai_runtime_mut(object_id)
                .is_some_and(|runtime| runtime.begin_guard_update(now))
            {
                continue;
            }
            self.finish_quick_exit_if_due(object_id, guard_target);
            if self.objects.get(&object_id).is_none_or(|u| {
                !matches!(u.ai_state, AIState::GuardingArea | AIState::GuardingObject)
            }) {
                continue;
            }
            let suspended = self.guard_quick_exit_active(object_id);
            if suspended {
                continue;
            }
            if self
                .unit_ai_runtime(object_id)
                .is_some_and(|runtime| runtime.guard_phase().is_none())
            {
                // Callers which set the guard goal directly still enter the real
                // Return state. No distance-based inference of Idle is allowed.
                let deadline = self
                    .unit_ai_runtime(object_id)
                    .and_then(|runtime| runtime.guard_scan_deadline());
                self.return_guard_to_post(object_id);
                if let Some(deadline) = deadline {
                    self.unit_ai_runtime_mut(object_id)
                        .unwrap()
                        .set_guard_scan_deadline(Some(deadline));
                }
            }
            let returning = matches!(
                self.unit_ai_runtime(object_id)
                    .and_then(|r| r.guard_phase()),
                Some(GuardPhase::Return { .. })
            );
            // AIGuardMachine exit conditions observe retaliation independently
            // of the nested Idle scan sleep.
            if can_attack && self.try_guard_last_attacker(object_id, team) {
                continue;
            }
            let scan_due = self.guard_acquire_scan_due(object_id, returning);
            // Idle's entire body sleeps until its scan; Return still observes
            // movement completion on frames without an enemy scan.
            if !returning && !scan_due {
                continue;
            }
            const GUARD_MIN_RADIUS: f32 = 80.0;
            match ai_state {
                AIState::GuardingArea => {
                    let anchor = guard_position.unwrap_or(position);
                    let (std_inner, std_outer) = self.host_std_guard_ranges(object_id);
                    let mood = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.ai_attitude)
                        .unwrap_or(0);
                    // C++ Sleep mood → getStdGuardRange 0. Do not fall back to 80.
                    let inner = if mood <= -2 {
                        0.0
                    } else if std_inner > 0.0 {
                        std_inner
                    } else if guard_radius > 0.0 {
                        guard_radius
                    } else {
                        GUARD_MIN_RADIUS
                    };
                    let _outer = if std_outer > 0.0 {
                        std_outer
                    } else {
                        inner * 1.5
                    };
                    let flying_only =
                        matches!(guard_mode, crate::game_logic::GuardMode::FlyingUnitsOnly);
                    let polygon_name = self
                        .objects
                        .get(&object_id)
                        .and_then(|o| o.guard_area_trigger.clone());
                    let polygon = polygon_name
                        .as_deref()
                        .filter(|n| !n.is_empty())
                        .and_then(Self::host_named_guard_area_polygon);
                    // C++ lookForInnerTarget: inner ring, or polygon bounding radius + point-in-trigger.
                    let (scan_anchor, acquire_radius) = if let Some((c, r, _)) = polygon.as_ref() {
                        (*c, if *r > 0.0 { *r } else { inner })
                    } else {
                        (anchor, inner)
                    };
                    let enter_guard = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.thing().template.enter_guard)
                        .unwrap_or(false);
                    let hijack_guard = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.thing().template.hijack_guard)
                        .unwrap_or(false);
                    if scan_due && can_attack {
                        if let Some(team_id) = self.host_team_common_target(object_id) {
                            if self.engage_guard_target(object_id, team_id, false) {
                                continue;
                            }
                        }
                        if let Some(enemy_id) = self.scan_guard_inner_target(
                            object_id,
                            team,
                            scan_anchor,
                            acquire_radius,
                            flying_only,
                            enter_guard,
                            hijack_guard,
                            polygon.as_ref().map(|(_, _, t)| t),
                        ) {
                            self.set_host_team_common_target(object_id, Some(enemy_id));
                            if enter_guard {
                                if self.try_guard_enter_or_hijack(
                                    object_id,
                                    enemy_id,
                                    hijack_guard,
                                    team,
                                ) {
                                    continue;
                                }
                            } else if self.engage_guard_target(object_id, enemy_id, false) {
                                continue;
                            }
                        }
                    }
                }
                AIState::GuardingObject => {
                    let guard_target_id = match guard_target {
                        Some(id) => id,
                        None => {
                            if let Some(obj) = self.objects.get_mut(&object_id) {
                                obj.set_target(None);
                            }
                            continue;
                        }
                    };

                    let Some(guard_anchor) = self
                        .objects
                        .get(&guard_target_id)
                        .filter(|o| o.is_alive())
                        .map(|o| o.get_position())
                    else {
                        if let Some(obj) = self.objects.get_mut(&object_id) {
                            obj.set_guard_target(None);
                        }
                        self.clear_target_decision_aware(object_id);
                        continue;
                    };

                    let (std_inner, _) = self.host_std_guard_ranges(object_id);
                    let mood = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.ai_attitude)
                        .unwrap_or(0);
                    let inner = if mood <= -2 {
                        0.0
                    } else if std_inner > 0.0 {
                        std_inner
                    } else if guard_radius > 0.0 {
                        guard_radius
                    } else {
                        GUARD_MIN_RADIUS
                    };
                    let flying_only =
                        matches!(guard_mode, crate::game_logic::GuardMode::FlyingUnitsOnly);
                    // C++ lookForInnerTarget always uses getStdGuardRange (inner).
                    let acquire_radius = inner;
                    let picking_crate = self
                        .objects
                        .get(&object_id)
                        .and_then(|o| o.requested_victim_id)
                        .is_some();
                    let enter_guard = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.thing().template.enter_guard)
                        .unwrap_or(false);
                    let hijack_guard = self
                        .objects
                        .get(&object_id)
                        .map(|o| o.thing().template.hijack_guard)
                        .unwrap_or(false);
                    if scan_due && can_attack {
                        if let Some(team_id) = self.host_team_common_target(object_id) {
                            if self.engage_guard_target(object_id, team_id, false) {
                                continue;
                            }
                        }
                        if enter_guard {
                            if let Some(enemy_id) = self.scan_guard_inner_target(
                                object_id,
                                team,
                                guard_anchor,
                                acquire_radius,
                                flying_only,
                                true,
                                hijack_guard,
                                None,
                            ) {
                                if self.try_guard_enter_or_hijack(
                                    object_id,
                                    enemy_id,
                                    hijack_guard,
                                    team,
                                ) {
                                    continue;
                                }
                            }
                        } else {
                            let tunnel_nemesis = {
                                let guard_is_tunnel = self.objects.get(&guard_target_id).is_some_and(
                                    |g| {
                                        g.is_tunnel_network_style_container()
                                            || crate::game_logic::host_tunnel_network::is_tunnel_network_template(
                                                &g.template_name,
                                            )
                                    },
                                );
                                if guard_is_tunnel {
                                    let key = self
                                        .objects
                                        .get(&guard_target_id)
                                        .map(|g| g.tunnel_system_key());
                                    key.and_then(|k| self.resolved_tunnel_nemesis(k))
                                } else {
                                    None
                                }
                            };
                            if let Some(enemy_id) = tunnel_nemesis {
                                if self.engage_guard_target(object_id, enemy_id, false) {
                                    continue;
                                }
                            }
                            if let Some(enemy_id) = self.scan_guard_inner_target(
                                object_id,
                                team,
                                guard_anchor,
                                acquire_radius,
                                flying_only,
                                false,
                                false,
                                None,
                            ) {
                                if self.engage_guard_target(object_id, enemy_id, false) {
                                    continue;
                                }
                            }
                        }
                    }

                    // Return owns the goal captured at entry. Only a due
                    // Idle body observes guardee drift and enters Return again.
                    if !returning
                        && !picking_crate
                        && self
                            .unit_ai_runtime_mut(object_id)
                            .is_some_and(|r| r.observe_guard_anchor(guard_anchor))
                    {
                        self.return_guard_to_post(object_id);
                    }
                }
                _ => {}
            }
            if returning {
                self.finish_guard_return_if_arrived(object_id);
            }
        }
    }
}
