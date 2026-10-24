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
                self.bridge_behavior.on_enter_rubble(id, &[]);
                self.visit_owned_bridge_splat_recipients(raw_id, layer, |owner, occupant| {
                    let context = DamageHitContext::new(
                        owner.objects.get(&occupant),
                        None,
                        DamageType::Falling,
                    );
                    let _ = owner.apply_owned_damage_with_effects(
                        occupant,
                        crate::game_logic::host_bridge_behavior::BRIDGE_SPLAT_DAMAGE,
                        Some(occupant),
                        DamageType::Falling,
                        HostDeathType::Splatted,
                        None,
                        &context,
                        presentation_effects,
                    );
                });
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

    /// TerrainLogic.cpp852–905 visits the current Object list, checking
    /// membership immediately before each callback. Birth prepends to the list
    /// and is therefore not a new visit behind the current cursor. Original
    /// destroyObject only queues physical removal until the enclosing tick ends.
    /// Do not capture a qualifying set: a prior callback can change later pose,
    /// layer or geometry. The caller must not drain the physical destroy list.
    pub(in crate::game_logic) fn visit_owned_bridge_splat_recipients(
        &mut self,
        bridge_id: u32,
        layer: gamelogic::path::PathfindLayerEnum,
        mut deliver: impl FnMut(&mut Self, ObjectId),
    ) {
        let mut cursor = self.objects.ordered_ids().first().copied();
        while let Some(id) = cursor {
            let qualifies = self.objects.get(&id).is_some_and(|object| {
                if layer == gamelogic::path::PathfindLayerEnum::Ground
                    || object.pathfind_layer != layer as u8
                {
                    return false;
                }
                let position = object.get_position();
                let location = Coord3D::new(position.x, position.z, position.y);
                let radius = object.geometry_info().minor_radius + PATHFIND_CELL_SIZE_F * 0.5;
                let bounds = Region2D {
                    lo: gamelogic::common::Coord2D::new(location.x - radius, location.y - radius),
                    hi: gamelogic::common::Coord2D::new(location.x + radius, location.y + radius),
                };
                let mut interacts = false;
                self.world_services
                    .terrain()
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .for_each_bridge(|bridge| {
                        if bridge.get_bridge_info().bridge_object_id == bridge_id {
                            interacts = (bridge.is_point_on_bridge(&location)
                                || bridge.is_cell_on_end(&bounds))
                                && (position.y - bridge.get_bridge_height(&location, None)).abs()
                                    <= LAYER_Z_CLOSE_ENOUGH_F;
                        }
                    });
                interacts
            });
            if qualifies {
                self.bridge_behavior.splat_kills =
                    self.bridge_behavior.splat_kills.saturating_add(1);
                deliver(self, id);
            }
            // Look up next after the released callback, exactly as getNextObject.
            // A missing current object means a forbidden physical drain occurred;
            // silently scanning from a new head could visit unrelated births.
            let index = self
                .objects
                .ordered_ids()
                .iter()
                .position(|current| *current == id)
                .expect("bridge callback physically removed its installed Object cursor");
            cursor = self.objects.ordered_ids().get(index + 1).copied();
        }
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
        let is_span = self.bridge_behavior.span_id_for(victim) == Some(victim);
        self.visit_owned_bridge_death_recipients(victim, |owner, target| {
            let _ = owner.apply_owned_kill_with_effects(
                target,
                DamageType::Unresistable,
                HostDeathType::Normal,
                presentation_effects,
            );
        });
        if is_span {
            self.handle_owned_objects_on_bridge_on_die(victim, presentation_effects);
            self.bridge_behavior.mark_death(victim, self.frame);
        }
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
    }

    /// BridgeBehavior.cpp822–940 is a distinct onDie operation after tower
    /// kills. TerrainLogic's earlier Rubble splat is not a replacement for it.
    fn handle_owned_objects_on_bridge_on_die(
        &mut self,
        bridge_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        self.visit_owned_bridge_on_die_recipients(bridge_id, |owner, id, physics| {
            // setLayer precedes getPhysics/kill in the original callback.
            if let Some(object) = owner.objects.get_mut(&id) {
                object.pathfind_layer = gamelogic::path::PathfindLayerEnum::Ground as u8;
                if physics {
                    object.allow_to_fall = true;
                }
            }
            if !physics {
                let _ = owner.apply_owned_kill_with_effects(
                    id,
                    DamageType::Unresistable,
                    HostDeathType::Normal,
                    effects,
                );
            }
        });
    }

    /// The partition iterator freezes only its candidate pointers/range filter
    /// before callbacks. Kind, airborne, height, polygon, layer and Physics
    /// presence are read at the actual later visit, matching original onDie.
    pub(in crate::game_logic) fn visit_owned_bridge_on_die_recipients(
        &mut self,
        bridge_id: ObjectId,
        mut deliver: impl FnMut(&mut Self, ObjectId, bool),
    ) {
        let Some(position) = self
            .objects
            .get(&bridge_id)
            .map(|object| object.get_position())
        else {
            return;
        };
        let mut record = None;
        self.world_services
            .terrain()
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .for_each_bridge(|bridge| {
                if bridge.is_point_on_bridge(&Coord3D::new(position.x, position.z, position.y))
                    && record.is_none()
                {
                    record = Some(bridge.clone());
                }
            });
        let Some(record) = record else {
            return;
        };
        let info = record.get_bridge_info();
        let layer = record.get_layer() as u8;
        let polygon = [info.from_left, info.from_right, info.to_right, info.to_left];
        let low = polygon
            .iter()
            .map(|point| point.z)
            .fold(polygon[0].z, f32::min);
        let dx = info.to_left.x - position.x;
        let dz = info.to_left.y - position.z;
        let radius = (dx * dx + dz * dz).sqrt();
        // Original FROM_CENTER_2D uses strict less-than before iterator insert.
        let ids = self.partition_manager.ids_in_radius_iterator_order(
            position.x,
            position.z,
            radius,
            |raw_id| {
                self.objects.get(&ObjectId(raw_id)).is_some_and(|object| {
                    let point = object.get_position();
                    let dx = point.x - position.x;
                    let dz = point.z - position.z;
                    dx * dx + dz * dz < radius * radius
                })
            },
        );
        // Pointer identity is operation-local. Original physical deletion is
        // deferred, but an invalid host callback cannot retarget a reused ID.
        let recipients: Vec<_> = ids
            .into_iter()
            .filter_map(|raw_id| {
                self.objects.get(&ObjectId(raw_id)).map(|object| {
                    (
                        ObjectId(raw_id),
                        std::sync::Arc::downgrade(&object.visual_draw_identity_token),
                    )
                })
            })
            .collect();
        for (id, identity) in recipients {
            let Some(object) = self.objects.get(&id) else {
                continue;
            };
            if !std::sync::Weak::ptr_eq(
                &identity,
                &std::sync::Arc::downgrade(&object.visual_draw_identity_token),
            ) {
                continue;
            }
            let point = object.get_position();
            if object.is_kind_of(KindOf::Bridge)
                || object.is_kind_of(KindOf::BridgeTower)
                || object.is_above_terrain()
                || point.y < low
                || object.pathfind_layer != layer
                || !point_inside_original_bridge_polygon(point, &polygon)
            {
                continue;
            }
            // Actual admitted module declaration, never active residual values
            // or a vehicle/infantry/name heuristic. Legacy no-module is absent.
            let physics = object
                .thing()
                .template
                .receiving_object_definition
                .as_ref()
                .is_some_and(|definition| {
                    definition
                        .behavior_modules
                        .iter()
                        .any(|module| module.class_name.eq_ignore_ascii_case("PhysicsBehavior"))
                });
            deliver(self, id, physics);
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

/// Literal Line2D.cpp209–239/297–313; its finite ray and inclusive segment
/// endpoints intentionally differ from the usual horizontal-ray shortcut.
fn point_inside_original_bridge_polygon(point: glam::Vec3, polygon: &[Coord3D; 4]) -> bool {
    let a = [point.x, point.z];
    let b = [1_000_000_f32, 1_000_000_f32];
    let mut intersections = 0;
    for index in 0..4 {
        let c = [polygon[index].x, polygon[index].y];
        let d = [polygon[(index + 1) % 4].x, polygon[(index + 1) % 4].y];
        let denominator = (b[0] - a[0]) * (d[1] - c[1]) - (b[1] - a[1]) * (d[0] - c[0]);
        if denominator == 0. {
            continue;
        }
        let r = ((a[1] - c[1]) * (d[0] - c[0]) - (a[0] - c[0]) * (d[1] - c[1])) / denominator;
        let s = ((a[1] - c[1]) * (b[0] - a[0]) - (a[0] - c[0]) * (b[1] - a[1])) / denominator;
        if 0. <= r && r <= 1. && 0. <= s && s <= 1. {
            intersections += 1;
        }
    }
    intersections % 2 == 1
}
