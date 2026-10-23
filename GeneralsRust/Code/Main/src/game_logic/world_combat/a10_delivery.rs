//! DeliverPayloadAIUpdate delivery → dive callbacks → parent movement.
//! Host transport/weapon math remains the existing residual, not a second simulation.
use super::*;

/// Facts admitted by one dive phase; the actor itself stays in its owner.
struct A10DiveContinuation {
    previous_state: u8,
    admitted_state: u8,
    cue: Option<(String, Vec3)>,
    strafe: Option<Vec3>,
}

impl GameLogic {
    pub(crate) fn update_a10_strike_flights_with_effects(
        &mut self,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::host_a10_strike_flight::{
            A10_START_DIVE_SOUND, A10_VULCAN_DELAY_FRAMES, tick_a10_dive,
        };
        use crate::game_logic::special_power_strikes::{
            A10_MISSILE_PRIMARY_DAMAGE, A10_MISSILE_PRIMARY_RADIUS, A10_PAYLOAD_TEMPLATE,
            A10_VULCAN_PRIMARY_DAMAGE, A10_VULCAN_PRIMARY_RADIUS,
        };

        // Preserve the receiving registered visitation order. This is not a
        // claim that the Main residual implements C++'s sleepy-update heap.
        // Retain identities, not Objects: a callback may retire/reuse an ID.
        let flights: Vec<_> = self
            .objects
            .registered()
            .filter(|(_, object)| object.a10_strike_transport.is_some() && object.is_alive())
            .map(|(id, object)| (*id, object.visual_draw_identity_token.clone()))
            .collect();
        for (id, identity) in flights {
            let delivery = self
                .a10_strike_flight_reg
                .delivery_for_transport(id)
                .expect("admitted A10 transport has a receiving delivery");
            // A historical ID is not permission to run another actor's lifetime.
            if !self
                .a10_strike_flight_reg
                .transport_lifetime_matches(delivery, id, &identity)
            {
                continue;
            }
            let same_actor = |object: &crate::game_logic::Object| {
                std::sync::Arc::ptr_eq(&object.visual_draw_identity_token, &identity)
            };
            let Some(object) = self
                .objects
                .get(&id)
                .filter(|o| same_actor(o) && o.is_alive())
            else {
                continue;
            };
            if object.a10_strike_transport.is_none() {
                continue;
            }

            // DeliveringState runs before the dive body and parent AI update.
            // The interval is admitted once; each item is counted only after
            // its synchronous constructor/configuration attempt has returned.
            let attempts = {
                let object = self.objects.get_mut(&id).unwrap();
                let position = object.get_position();
                let data = object.a10_strike_transport.as_mut().unwrap();
                if data.visible_payload_attempt_due(self.frame)
                    && data.is_close_enough_to_target(position)
                {
                    data.begin_visible_payload_interval(self.frame)
                } else {
                    0
                }
            };
            for _ in 0..attempts {
                let Some(owner) =
                    self.objects
                        .get(&id)
                        .filter(|o| same_actor(o))
                        .and_then(|object| {
                            object
                                .a10_strike_transport
                                .as_ref()
                                .filter(|data| data.missiles_remaining != 0)
                                .map(|_| object.owner_player_id)
                        })
                else {
                    break;
                };
                // C++ selects the current controller's default Team before
                // newObject. No faction or ambient-owner substitution.
                let payload = owner.and_then(|owner| {
                    self.create_object_for_player_with_effects(
                        A10_PAYLOAD_TEMPLATE,
                        owner,
                        Vec3::ZERO,
                        None,
                        presentation_effects,
                    )
                });
                if let Some(payload_id) = payload {
                    assert!(
                        self.a10_strike_flight_reg
                            .bind_missile(delivery, id, payload_id),
                        "new A10 missile must bind exactly once to its source delivery"
                    );
                    // newObject may synchronously change the producer. Read
                    // its current transform and target AFTER that callback.
                    let facts = self.objects.get(&id).filter(|o| same_actor(o))
                        .and_then(|object| object.a10_strike_transport.as_ref().map(|data| {
                            let slot = crate::game_logic::host_a10_strike_flight::A10_NUM_BONES
                                .saturating_sub(data.missiles_remaining).saturating_add(1);
                            let drop = crate::game_logic::host_a10_strike_drop_log::a10_weapon_a_world_pos(
                                object.get_position(), object.get_orientation(), slot);
                            (drop, data.target, object.movement.velocity, object.get_orientation())
                        }));
                    if let Some((drop, target, inherited_velocity, yaw)) = facts {
                        let velocity =
                            crate::game_logic::host_a10_strike_drop_log::a10_missile_fire_velocity(
                                drop,
                                target,
                                inherited_velocity,
                            );
                        if let Some(payload) = self.objects.get_mut(&payload_id) {
                            payload.producer_id = Some(id);
                            payload.a10_strike_missile = true;
                            payload.set_position_with_effects(
                                drop - velocity,
                                self.frame,
                                presentation_effects,
                            );
                            payload.set_orientation(yaw);
                            payload.movement.velocity = velocity;
                            let _ = payload.set_smart_bomb_target(target);
                        }
                    } else {
                        // Do not leave an unconfigured actor when a callback
                        // retired the original producer during newObject.
                        self.mark_object_for_destruction_with_effects(
                            payload_id,
                            None,
                            presentation_effects,
                        );
                    }
                }
                assert!(
                    self.a10_strike_flight_reg
                        .complete_visible_payload_attempt(delivery, id)
                );
                let Some(data) = self
                    .objects
                    .get_mut(&id)
                    .filter(|o| same_actor(o))
                    .and_then(|object| object.a10_strike_transport.as_mut())
                else {
                    break;
                };
                data.complete_visible_payload_attempt();
                let exhausted = data.missiles_remaining == 0;
                if exhausted {
                    self.a10_strike_flight_reg
                        .note_transport_payload_exhausted(delivery, id);
                }
            }

            // Bounded borrowed dive phase. No movement is published before
            // its cue/weapon callbacks. Keep only owned continuation facts.
            let Some(dive) = self
                .objects
                .get_mut(&id)
                .filter(|o| same_actor(o))
                .and_then(|object| {
                    let position = object.get_position();
                    let velocity = object.movement.velocity;
                    let cue = object
                        .thing()
                        .template
                        .receiving_client_template
                        .as_ref()
                        .and_then(|template| template.per_unit_sound_event(A10_START_DIVE_SOUND))
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned);
                    let data = object.a10_strike_transport.as_mut()?;
                    let previous = data.dive_state;
                    let dive =
                        tick_a10_dive(&mut data.dive_state, position, data.target, velocity, 22.0);
                    let cue = dive
                        .start_dive
                        .then_some(cue)
                        .flatten()
                        .map(|cue| (cue, position));
                    let strafe = if dive.should_strafe
                        && self.frame.saturating_sub(data.last_vulcan_frame)
                            >= A10_VULCAN_DELAY_FRAMES
                    {
                        data.last_vulcan_frame = self.frame;
                        Some(dive.strafe_point)
                    } else {
                        None
                    };
                    Some(A10DiveContinuation {
                        previous_state: previous,
                        admitted_state: data.dive_state,
                        cue,
                        strafe,
                    })
                })
            else {
                continue;
            };
            if let Some((cue, position)) = dive.cue {
                self.queue_audio_event(
                    crate::game_logic::AudioEventRequest::new(&cue)
                        .with_position(position)
                        .with_priority(160),
                );
            }
            if let Some(mut position) = dive.strafe {
                position.y = self.raw_terrain_ground_height_at(position);
                if self.objects.get(&id).filter(|o| same_actor(o)).is_some() {
                    let (damage, hits, destroyed) = self
                        .apply_radius_damage_from_source_with_effects(
                            id,
                            Some(id),
                            position,
                            A10_VULCAN_PRIMARY_DAMAGE,
                            A10_VULCAN_PRIMARY_RADIUS,
                            DamageType::Bullet,
                            presentation_effects,
                        );
                    if let Some(strike) = self.a10_strike_flight_reg.delivery_strike_id(delivery) {
                        self.special_power_strikes.credit_live_a10_delivery_impact(
                            strike, damage, hits, destroyed, false, position,
                        );
                    }
                }
            }

            // AIUpdateInterface follows delivery and dive effects. Reborrow
            // the same actual actor after callbacks, including changed pose,
            // target and velocity. The vertical helper is evaluated on a local
            // enum solely for its numeric result; canonical state advanced once.
            let at_exit = self
                .objects
                .get_mut(&id)
                .filter(|o| same_actor(o) && o.is_alive())
                .and_then(|object| {
                    let position = object.get_position();
                    let current_velocity = object.movement.velocity;
                    let data = object.a10_strike_transport.as_mut()?;
                    let mut vertical_state = if data.dive_state == dive.admitted_state {
                        dive.previous_state
                    } else {
                        data.dive_state
                    };
                    let vertical = tick_a10_dive(
                        &mut vertical_state,
                        position,
                        data.target,
                        current_velocity,
                        22.0,
                    );
                    let (mut next, mut velocity, at_exit) = data.tick_transport(position);
                    next.y = vertical.new_y;
                    velocity.y = next.y - position.y;
                    object.set_position_with_effects(next, self.frame, presentation_effects);
                    object.movement.velocity = velocity;
                    if velocity.length_squared() > 1e-6 {
                        object.set_orientation(velocity.z.atan2(velocity.x));
                    }
                    Some(at_exit)
                });
            if at_exit == Some(true) {
                self.a10_strike_flight_reg.note_transport_exit(delivery, id);
                self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
            }
        }

        self.reconcile_receiving_a10_delivery_actors();
        let missiles: Vec<_> = self
            .objects
            .registered()
            .filter(|(_, object)| object.a10_strike_missile && object.is_alive())
            .map(|(id, object)| (*id, object.visual_draw_identity_token.clone()))
            .collect();
        for (id, identity) in missiles {
            let same_actor = |object: &crate::game_logic::Object| {
                std::sync::Arc::ptr_eq(&object.visual_draw_identity_token, &identity)
            };
            let delivery = self
                .a10_strike_flight_reg
                .delivery_for_missile(id)
                .expect("admitted A10 missile has a receiving delivery");
            let Some((position, _producer, _team)) = self
                .objects
                .get_mut(&id)
                .filter(|object| {
                    same_actor(object) && object.a10_strike_missile && object.is_alive()
                })
                .map(|object| {
                    let position = object.get_position() + object.movement.velocity;
                    object.set_position_with_effects(position, self.frame, presentation_effects);
                    (position, object.producer_id, object.team)
                })
            else {
                continue;
            };
            if position.y <= 5.0 {
                let epicenter = Vec3::new(position.x, 0.0, position.z);
                let (damage, hits, destroyed) = self.apply_radius_damage_from_source_with_effects(
                    id,
                    None,
                    epicenter,
                    A10_MISSILE_PRIMARY_DAMAGE,
                    A10_MISSILE_PRIMARY_RADIUS,
                    DamageType::Explosive,
                    presentation_effects,
                );
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    position,
                    self.frame,
                    Some(id),
                    None,
                );
                self.reconcile_receiving_a10_delivery_actors();
                if self.a10_strike_flight_reg.note_missile_impact(delivery, id) {
                    let final_wave = self.a10_strike_flight_reg.delivery_is_final(delivery);
                    if let Some(strike) = self.a10_strike_flight_reg.delivery_strike_id(delivery) {
                        self.special_power_strikes.credit_live_a10_delivery_impact(
                            strike, damage, hits, destroyed, final_wave, epicenter,
                        );
                    }
                    if final_wave {
                        let _ = self
                            .a10_strike_flight_reg
                            .complete_delivery_if_final(delivery);
                    }
                }
                // Impact callbacks may have retired the original missile.
                // Never destroy another lifetime merely because its ID matches.
                if self
                    .objects
                    .get(&id)
                    .is_some_and(|object| same_actor(object))
                {
                    self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
                }
            }
        }
        self.finish_ready_a10_deliveries();
    }

    /// Reconcile only delivery capability receipts against the actual owner.
    /// No damage or synthetic final impact is admitted by this sweep.
    fn reconcile_receiving_a10_delivery_actors(&mut self) {
        for delivery in self.a10_strike_flight_reg.delivery_ids() {
            for transport in self.a10_strike_flight_reg.delivery_transport_ids(delivery) {
                let capable = self.objects.get(&transport).is_some_and(|object| {
                    self.a10_strike_flight_reg.transport_lifetime_matches(
                        delivery,
                        transport,
                        &object.visual_draw_identity_token,
                    ) && object.is_alive()
                        && object
                            .a10_strike_transport
                            .as_ref()
                            .is_some_and(|data| data.missiles_remaining != 0)
                });
                if !capable {
                    self.a10_strike_flight_reg
                        .note_transport_exit(delivery, transport);
                }
            }
            for missile in self.a10_strike_flight_reg.delivery_missile_ids(delivery) {
                let active = self
                    .objects
                    .get(&missile)
                    .is_some_and(|object| object.is_alive() && object.a10_strike_missile);
                if !active {
                    self.a10_strike_flight_reg
                        .note_missile_retired_without_impact(delivery, missile);
                }
            }
        }
    }

    fn finish_ready_a10_deliveries(&mut self) {
        self.reconcile_receiving_a10_delivery_actors();
        for delivery in self.a10_strike_flight_reg.finalizable_deliveries() {
            if let Some(Some(strike)) = self
                .a10_strike_flight_reg
                .complete_delivery_if_final(delivery)
            {
                self.special_power_strikes
                    .finish_live_a10_delivery_without_more_effects(strike);
            }
        }
    }
}
