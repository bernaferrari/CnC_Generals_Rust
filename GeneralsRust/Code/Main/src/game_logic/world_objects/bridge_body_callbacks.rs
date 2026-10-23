//! Immediate bridge callbacks, driven by the world that admitted the impact.
//! CPP ActiveBody585–653, BridgeBehavior409–487/600–678, BridgeTower68–214.
use super::super::*;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::object::{BridgeBodyCallbacks, DamageHitContext};
use gamelogic::common::{BodyDamageType, Coord3D, Region2D};
use gamelogic::path::{LAYER_Z_CLOSE_ENOUGH_F, PATHFIND_CELL_SIZE_F};

impl GameLogic {
    pub(in crate::game_logic) fn complete_owned_bridge_body_callbacks(
        &mut self,
        callbacks: BridgeBodyCallbacks,
    ) {
        self.complete_owned_bridge_body_callbacks_with_effects(
            callbacks,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn complete_owned_bridge_body_callbacks_with_effects(
        &mut self,
        callbacks: BridgeBodyCallbacks,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        if callbacks.health_changed {
            self.propagate_owned_bridge_body_change_with_effects(callbacks, presentation_effects);
        }
        // Nested onDamage/onHealing callbacks finish first. Original ActiveBody
        // compares the live body with its captured old state after that chain.
        let Some(state) = self
            .objects
            .get(&callbacks.victim)
            .map(|o| o.body_damage_state)
        else {
            return;
        };
        if state == callbacks.old_state || self.bridge_behavior.span(callbacks.victim).is_none() {
            return;
        }
        self.bridge_behavior
            .note_body_state(callbacks.victim, state.ordinal());
        self.play_bridge_body_transition_with_effects(
            callbacks.victim,
            callbacks.old_state.ordinal(),
            state.ordinal(),
            presentation_effects,
        );
        self.update_owned_bridge_damage_states_with_effects(presentation_effects);
        if state == HostBodyDamageType::Rubble || callbacks.old_state == HostBodyDamageType::Rubble
        {
            self.host_radar_queue_terrain_refresh();
        }
    }

    fn propagate_owned_bridge_body_change(&mut self, callbacks: BridgeBodyCallbacks) {
        self.propagate_owned_bridge_body_change_with_effects(
            callbacks,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    /// Bounded synchronous bridge delivery. Every recipient borrow ends before
    /// its callback; later slots and bodies are resolved after that callback.
    pub(in crate::game_logic) fn visit_owned_bridge_mirror_recipients(
        &mut self,
        callbacks: BridgeBodyCallbacks,
        mut deliver: impl FnMut(&mut Self, ObjectId, f32),
    ) {
        if callbacks.input_amount <= 0.0 || !callbacks.input_amount.is_finite() {
            return;
        }
        let Some(span) = self.bridge_behavior.span_id_for(callbacks.victim) else {
            return;
        };
        // CPP reads Body::getMaxHealth when this module callback starts,
        // after any earlier body callback, not when damage was admitted.
        let Some(maximum) = self
            .objects
            .get(&callbacks.victim)
            .map(|o| o.health.maximum)
        else {
            return;
        };
        if maximum <= 0.0 || !maximum.is_finite() {
            return;
        }
        let fraction = callbacks.input_amount / maximum;
        let source = callbacks.source.and_then(|id| self.objects.get(&id));
        // A span suppresses only tower-origin changes; a tower suppresses
        // both span- and tower-origin changes (not template-name guesses).
        let suppressed = source.is_some_and(|o| {
            o.is_kind_of(KindOf::BridgeTower)
                || (callbacks.victim != span && o.is_kind_of(KindOf::Bridge))
        });
        if suppressed {
            return;
        }
        if !(0..5).any(|slot| {
            self.bridge_behavior
                .callback_target_at(span, callbacks.victim, slot)
                .is_some()
        }) {
            return;
        }
        self.bridge_behavior.record_mirror_applied();
        for slot in 0..5 {
            let Some(target) =
                self.bridge_behavior
                    .callback_target_at(span, callbacks.victim, slot)
            else {
                continue;
            };
            // Original resolves every recipient afresh, in tower slot order;
            // a tower's span is visited only after all sibling towers.
            let Some(maximum) = self.objects.get(&target).map(|o| o.health.maximum) else {
                continue;
            };
            deliver(self, target, fraction * maximum);
        }
    }

    fn propagate_owned_bridge_body_change_with_effects(
        &mut self,
        callbacks: BridgeBodyCallbacks,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.visit_owned_bridge_mirror_recipients(callbacks, |owner, target, amount| {
            if callbacks.damage_type == DamageType::Healing {
                let _ = owner.apply_owned_healing_with_effects(
                    target,
                    amount,
                    Some(callbacks.victim),
                    presentation_effects,
                );
            } else {
                let context = DamageHitContext::new(
                    owner.objects.get(&callbacks.victim),
                    None,
                    callbacks.damage_type,
                );
                let _ = owner.apply_owned_damage_with_effects(
                    target,
                    amount,
                    Some(callbacks.victim),
                    callbacks.damage_type,
                    callbacks.death_type,
                    None,
                    &context,
                    presentation_effects,
                );
            }
        });
    }

    /// Full original terrain linked-list walk. Release its borrow before
    /// occupant callbacks, then read the next body's current state.
    fn update_owned_bridge_damage_states(&mut self) {
        self.update_owned_bridge_damage_states_with_effects(
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn update_owned_bridge_damage_states_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let owner = self.world_services.terrain().clone();
        let mut ids = Vec::new();
        owner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .for_each_bridge(|bridge| ids.push(bridge.get_bridge_info().bridge_object_id));
        for raw_id in ids {
            let mut bridge = None;
            owner
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .for_each_bridge(|record| {
                    if record.get_bridge_info().bridge_object_id == raw_id {
                        bridge = Some(record.clone());
                    }
                });
            let Some(bridge) = bridge else {
                continue;
            };
            let id = ObjectId(raw_id);
            let layer = bridge.get_layer();
            let state = self
                .objects
                .get(&id)
                .map(|o| Self::leftover_bridge_body_state(o.body_damage_state));
            let transition = owner
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .observe_host_bridge_body(raw_id, state);
            let Some((_, old, new)) = transition else {
                continue;
            };
            if new == BodyDamageType::Rubble {
                ({
                    let ground_owner = self.world_services.terrain();
                    let ground_loan = ground_owner
                        .read()
                        .unwrap_or_else(|error| error.into_inner());
                    let ground_source =
                        crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                            &ground_loan,
                            self.map_loaded,
                            self.terrain.as_ref(),
                            self.pathfinding_height_samples.as_ref(),
                            &self.pathfinding_system.grid,
                        );
                    self.pathfinding_system.grid.stamp_reserved_bridge_layer(
                        layer as u8,
                        true,
                        &ground_source,
                    )
                });
                let occupants: Vec<_> = self
                    .objects
                    .registered()
                    .filter_map(|(&id, object)| {
                        if layer == gamelogic::path::PathfindLayerEnum::Ground
                            || object.pathfind_layer != layer as u8
                        {
                            return None;
                        }
                        let p = object.get_position();
                        let location = Coord3D::new(p.x, p.z, p.y);
                        let radius = object.thing().template.geometry_info.minor_radius
                            + PATHFIND_CELL_SIZE_F * 0.5;
                        let bounds = Region2D {
                            lo: gamelogic::common::Coord2D::new(
                                location.x - radius,
                                location.y - radius,
                            ),
                            hi: gamelogic::common::Coord2D::new(
                                location.x + radius,
                                location.y + radius,
                            ),
                        };
                        ((bridge.is_point_on_bridge(&location) || bridge.is_cell_on_end(&bounds))
                            && (p.y - bridge.get_bridge_height(&location, None)).abs()
                                <= LAYER_Z_CLOSE_ENOUGH_F)
                            .then_some(id)
                    })
                    .collect();
                self.bridge_behavior.on_enter_rubble(id, &occupants);
                for occupant in occupants {
                    let context = DamageHitContext::new(
                        self.objects.get(&occupant),
                        None,
                        DamageType::Falling,
                    );
                    let _ = self.apply_owned_damage_with_effects(
                        occupant,
                        crate::game_logic::host_bridge_behavior::BRIDGE_SPLAT_DAMAGE,
                        Some(occupant),
                        DamageType::Falling,
                        HostDeathType::Splatted,
                        None,
                        &context,
                        presentation_effects,
                    );
                }
            }
            if old == BodyDamageType::Rubble {
                self.bridge_behavior.on_leave_rubble(id);
                if !self.bridge_behavior.is_scaffold_present(id) {
                    ({
                        let ground_owner = self.world_services.terrain();
                        let ground_loan = ground_owner
                            .read()
                            .unwrap_or_else(|error| error.into_inner());
                        let ground_source =
                            crate::game_logic::pathfinding::PathfindingGround::for_terrain(
                                &ground_loan,
                                self.map_loaded,
                                self.terrain.as_ref(),
                                self.pathfinding_height_samples.as_ref(),
                                &self.pathfinding_system.grid,
                            );
                        self.pathfinding_system.grid.stamp_reserved_bridge_layer(
                            layer as u8,
                            false,
                            &ground_source,
                        )
                    });
                }
            }
        }
        owner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .finish_host_bridge_damage_scan();
    }

    /// CPP tower onDie kills the span; span onDie kills towers in slot order.
    /// Existing onDie admission prevents recursive linked kills from repeating.
    pub(in crate::game_logic) fn apply_owned_bridge_death_links(&mut self, victim: ObjectId) {
        self.apply_owned_bridge_death_links_with_effects(
            victim,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn apply_owned_bridge_death_links_with_effects(
        &mut self,
        victim: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.visit_owned_bridge_death_recipients(victim, |owner, target| {
            let _ = owner.apply_owned_kill_with_effects(
                target,
                DamageType::Unresistable,
                HostDeathType::Normal,
                presentation_effects,
            );
        });
    }

    pub(in crate::game_logic) fn visit_owned_bridge_death_recipients(
        &mut self,
        victim: ObjectId,
        mut deliver: impl FnMut(&mut Self, ObjectId),
    ) {
        let Some(span) = self.bridge_behavior.span_id_for(victim) else {
            return;
        };
        self.bridge_behavior.record_death_link_applied();
        // Span onDie rereads each tower slot after the preceding immediate
        // kill; tower onDie addresses only its originally resolved span.
        let slots = if span == victim { 0..4 } else { 4..5 };
        for slot in slots {
            let Some(target) = self.bridge_behavior.callback_target_at(span, victim, slot) else {
                continue;
            };
            deliver(self, target);
        }
        if span == victim {
            self.bridge_behavior.mark_death(span, self.frame);
        }
    }

    pub(in crate::game_logic) fn attempt_owned_healing_from_sole_benefactor(
        &mut self,
        victim: ObjectId,
        amount: f32,
        source: ObjectId,
        duration_frames: u32,
        frame: u32,
    ) -> bool {
        self.attempt_owned_healing_from_sole_benefactor_with_effects(
            victim,
            amount,
            source,
            duration_frames,
            frame,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn attempt_owned_healing_from_sole_benefactor_with_effects(
        &mut self,
        victim: ObjectId,
        amount: f32,
        source: ObjectId,
        duration_frames: u32,
        frame: u32,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        if !self.objects.contains_key(&source) {
            return false;
        }
        if !self
            .objects
            .get_mut(&victim)
            .is_some_and(|o| o.claim_healing_benefactor(source, duration_frames, frame))
        {
            return false;
        }
        let _ = self.apply_owned_healing_with_effects(
            victim,
            amount,
            Some(source),
            presentation_effects,
        );
        true
    }
}
