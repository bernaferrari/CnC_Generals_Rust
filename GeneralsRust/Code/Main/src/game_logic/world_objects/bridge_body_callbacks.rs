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
        if callbacks.health_changed {
            self.propagate_owned_bridge_body_change(callbacks);
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
        self.play_bridge_body_transition(
            callbacks.victim,
            callbacks.old_state.ordinal(),
            state.ordinal(),
        );
        self.update_owned_bridge_damage_states();
        if state == HostBodyDamageType::Rubble || callbacks.old_state == HostBodyDamageType::Rubble
        {
            crate::game_logic::host_radar::host_radar_queue_terrain_refresh();
        }
    }

    fn propagate_owned_bridge_body_change(&mut self, callbacks: BridgeBodyCallbacks) {
        if callbacks.input_amount <= 0.0
            || !callbacks.input_amount.is_finite()
            || callbacks.max_health <= 0.0
        {
            return;
        }
        let Some(span) = self.bridge_behavior.span_id_for(callbacks.victim) else {
            return;
        };
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
        let targets = self.bridge_behavior.mirror_targets(callbacks.victim);
        if targets.is_empty() {
            return;
        }
        self.bridge_behavior.record_mirror_applied();
        let fraction = callbacks.input_amount / callbacks.max_health;
        for target in targets {
            // Original resolves every recipient afresh, in tower slot order;
            // a tower's span is visited only after all sibling towers.
            let Some(maximum) = self.objects.get(&target).map(|o| o.health.maximum) else {
                continue;
            };
            let context = DamageHitContext::new(
                self.objects.get(&callbacks.victim),
                None,
                callbacks.damage_type,
            );
            let _ = self.apply_owned_damage(
                target,
                fraction * maximum,
                Some(callbacks.victim),
                callbacks.damage_type,
                callbacks.death_type,
                None,
                &context,
            );
        }
    }

    /// Full original terrain linked-list walk. Release its borrow before
    /// occupant callbacks, then read the next body's current state.
    fn update_owned_bridge_damage_states(&mut self) {
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
                self.pathfinding_system
                    .grid
                    .stamp_reserved_bridge_layer(layer as u8, true);
                let occupants: Vec<_> = self
                    .objects
                    .iter()
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
                    let _ = self.apply_owned_damage(
                        occupant,
                        crate::game_logic::host_bridge_behavior::BRIDGE_SPLAT_DAMAGE,
                        Some(occupant),
                        DamageType::Falling,
                        HostDeathType::Splatted,
                        None,
                        &context,
                    );
                }
            }
            if old == BodyDamageType::Rubble {
                self.bridge_behavior.on_leave_rubble(id);
                if !self.bridge_behavior.is_scaffold_present(id) {
                    self.pathfinding_system
                        .grid
                        .stamp_reserved_bridge_layer(layer as u8, false);
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
        let Some(span) = self.bridge_behavior.span_id_for(victim) else {
            return;
        };
        let targets = if span == victim {
            self.bridge_behavior.mirror_targets(victim)
        } else {
            vec![span]
        };
        self.bridge_behavior.record_death_link_applied();
        for target in targets {
            let _ = self.apply_owned_kill(target, DamageType::Unresistable, HostDeathType::Normal);
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
        let context = DamageHitContext::new(self.objects.get(&source), None, DamageType::Healing);
        let _ = self.apply_owned_damage(
            victim,
            amount,
            Some(source),
            DamageType::Healing,
            HostDeathType::Normal,
            None,
            &context,
        );
        true
    }
}
