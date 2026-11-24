//! Artillery's single visible payload per DeliverPayload carrier.

use super::*;
use crate::game_logic::host_artillery_barrage_flight::{
    ArtilleryPayload, HostArtilleryBarrageFlightData, VisibleArtilleryPayload,
};
use crate::game_logic::host_spectre_gunship_deployment::find_farthest_edge_point_residual;
use crate::game_logic::special_power_strikes::{
    ARTILLERY_BARRAGE_CANNON_MAX_HEALTH, ARTILLERY_BARRAGE_DECAL_RADIUS,
    ARTILLERY_BARRAGE_DELIVERY_DISTANCE, ARTILLERY_BARRAGE_FORMATION_SPACING,
    ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES, ARTILLERY_BARRAGE_TRANSPORT,
    ArtilleryBarrageScienceTier,
};
use game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
use gamelogic::object_creation_list::DeliverPayloadNugget;

impl GameLogic {
    pub fn spawn_artillery_barrage_flight(
        &mut self,
        source: ObjectId,
        target: Vec3,
        tier: ArtilleryBarrageScienceTier,
    ) -> Option<ObjectId> {
        self.spawn_artillery_barrage_flight_with_effects(
            source,
            target,
            tier,
            None,
            &mut Default::default(),
        )
    }

    pub(in crate::game_logic) fn spawn_artillery_barrage_flight_with_effects(
        &mut self,
        source_id: ObjectId,
        target: Vec3,
        tier: ArtilleryBarrageScienceTier,
        strike: Option<u32>,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::{KindOf, ThingTemplate};

        let source = self
            .objects
            .get(&source_id)
            .filter(|source| source.is_alive())?;
        let owner = source.owner_player_id?;
        let identity = source.visual_draw_identity_token.clone();
        let authored = source.authors_behavior_module("OCLSpecialPower")
            || self.receiving_module_content.is_some();
        let list = if authored {
            Some(self.receiving_special_power_ocl(source, "SuperweaponArtilleryBarrage")?)
        } else {
            None
        };
        let definition = if let Some(list) = &list {
            let [nugget] = list.nuggets() else {
                return None;
            };
            Some(nugget.as_any().downcast_ref::<DeliverPayloadNugget>()?)
        } else {
            None
        };
        let payload = if let Some(nugget) = definition {
            let data = &nugget.data;
            if data.visible_num_bones != 1
                || data.visible_items_dropped_per_interval != 1
                || data.visible_payload_template_name.as_str().is_empty()
                || nugget.visible_payload_weapon_name.is_empty()
            {
                return None;
            }
            ArtilleryPayload::Visible(VisibleArtilleryPayload {
                object: data.visible_payload_template_name.as_str().to_owned(),
                weapon: nugget.visible_payload_weapon_name.clone(),
                bone: data.visible_drop_bone_name.as_str().to_owned(),
                subobject: data.visible_sub_object_name.as_str().to_owned(),
                inherit_velocity: data.inherit_transport_velocity,
            })
        } else {
            ArtilleryPayload::TemplateOnly
        };
        let transport =
            definition.map_or(ARTILLERY_BARRAGE_TRANSPORT, |n| n.transport_name.as_str());
        if !authored && !self.templates.contains_key(transport) {
            let mut template = ThingTemplate::new(transport);
            template
                .set_health(ARTILLERY_BARRAGE_CANNON_MAX_HEALTH)
                .add_kind_of(KindOf::Aircraft)
                .add_kind_of(KindOf::Vehicle);
            self.templates.insert(transport.to_owned(), template);
        }

        let mut edge = find_farthest_edge_point_residual(
            target,
            self.world_min.x,
            self.world_min.z,
            self.world_max.x,
            self.world_max.z,
            0.0,
        );
        edge.y = self.raw_terrain_ground_height_at(edge)
            + crate::game_logic::host_ocl_special_power::OCL_CREATE_ABOVE_LOCATION_HEIGHT;
        let original_axes = |p: Vec3| Vec3::new(p.x, p.z, p.y);
        let primary = original_axes(edge);
        let secondary = original_axes(target);
        let count =
            i32::try_from(definition.map_or(tier.formation_size(), |n| n.formation_size)).ok()?;
        let distance = definition.map_or(ARTILLERY_BARRAGE_DELIVERY_DISTANCE, |n| {
            n.data.dist_to_target
        });
        let spacing =
            definition.map_or(ARTILLERY_BARRAGE_FORMATION_SPACING, |n| n.formation_spacing);
        let max_delay = definition.map_or(ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES, |n| {
            n.delay_delivery_frames_max
        });
        let mut first = None;
        for index in 0..count {
            if !self.objects.get(&source_id).is_some_and(|source| {
                std::sync::Arc::ptr_eq(&identity, &source.visual_draw_identity_token)
            }) {
                break;
            }
            let sample = definition
                .filter(|n| index > 0 && n.error_radius > 1.0)
                .map(|n| {
                    (
                        self.logic_random.next_real(0.0, n.error_radius),
                        self.logic_random.next_real(0.0, std::f32::consts::TAU),
                    )
                });
            let pose = DeliverPayloadNugget::formation_flight_pose(
                &primary,
                &secondary,
                index,
                count,
                spacing,
                definition.map_or(0.0, |n| n.convergence_factor),
                distance,
                sample,
            );
            let launch = original_axes(pose.start_pos);
            let Some(id) =
                self.create_object_for_player_with_effects(transport, owner, launch, None, effects)
            else {
                break;
            };
            let Some(object) = self.objects.get_mut(&id) else {
                continue;
            };
            let generation = object.visual_object_generation;
            let epoch = self.visual_world_epoch;
            object.note_producer(source_id);
            object.set_script_targetable(true);
            object.bind_special_power_completion_creator(if index == 0 {
                source_id.0
            } else {
                gamelogic::common::INVALID_ID
            });
            object.set_orientation_with_partition_owner(pose.orient, &mut self.partition_manager);
            let mut data = HostArtilleryBarrageFlightData::start(
                launch,
                original_axes(pose.move_to_pos),
                tier,
            );
            data.aim = original_axes(pose.target_pos);
            data.payload = Some(payload.clone());
            data.strike_id = strike;
            data.map_min = self.world_min;
            data.map_max = self.world_max;
            data.delivery_distance = distance;
            data.pre_open_distance = definition.map_or(0.0, |n| n.data.pre_open_distance);
            if definition.is_some() {
                data.speed_per_frame =
                    object.selected_locomotor_definition.map_or(0.0, |binding| {
                        binding.movement.max_speed * SECONDS_PER_LOGICFRAME_REAL
                    });
            }
            data.self_destruct_after_release =
                definition.is_some_and(|n| n.data.self_destruct_object);
            if max_delay > 0 {
                let delay = self
                    .logic_random
                    .next_int(0, max_delay.min(i32::MAX as u32) as i32)
                    as u32;
                data.delay_until_frame = self.frame.wrapping_add(delay);
            }
            if definition.is_some_and(|n| n.start_at_preferred_height) {
                let preferred = object.loco_preferred_height;
                data.launch.y = self.raw_terrain_ground_height_at(launch) + preferred;
                self.set_owned_object_position_with_effects(id, data.launch, self.frame, effects);
            }
            if !self.spawn_identity_matches(id, generation, epoch) {
                continue;
            }
            let aim = data.aim;
            self.objects.get_mut(&id)?.artillery_barrage_transport = Some(data);
            self.artillery_barrage_flight_reg.record_transport();
            first = first.or(Some(id));
            if index == 0 {
                if let Some(nugget) = definition {
                    self.create_authored_delivery_decal(id, aim, &nugget.data, effects);
                } else {
                    self.create_delivery_radius_decal_with_effects(
                        id,
                        target,
                        ARTILLERY_BARRAGE_DECAL_RADIUS,
                        effects,
                    );
                }
            }
        }
        first
    }

    pub fn update_artillery_barrage_flights(&mut self) -> gamelogic::GameLogicResult<()> {
        self.update_artillery_barrage_flights_with_effects(&mut Default::default())
    }

    pub(in crate::game_logic) fn update_artillery_barrage_flights_with_effects(
        &mut self,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        let mut carriers: Vec<_> = self
            .objects
            .registered()
            .filter(|(_, object)| object.artillery_barrage_transport.is_some() && object.is_alive())
            .map(|(id, _)| *id)
            .collect();
        carriers.sort_unstable();
        let mut pending = std::collections::BTreeSet::new();
        for id in carriers {
            let Some(object) = self.objects.get_mut(&id).filter(|object| object.is_alive()) else {
                continue;
            };
            let position = object.get_position();
            let generation = object.visual_object_generation;
            let epoch = self.visual_world_epoch;
            let Some(data) = object.artillery_barrage_transport.as_mut() else {
                continue;
            };
            let strike = data.strike_id;
            if data.payload.is_none() && data.self_destruct_after_release {
                self.mark_object_for_destruction_with_effects(id, None, effects);
                continue;
            }
            if data.payload.is_some() {
                if data.is_hold_for_delay(self.frame) || !data.in_delivery_band(position) {
                    pending.extend(strike);
                } else if let Some(payload) = data.payload.take() {
                    let aim = data.aim;
                    object.kill_delivery_radius_decal();
                    if let Some(shell) =
                        self.release_artillery_payload(id, aim, payload, effects)?
                    {
                        self.artillery_barrage_flight_reg.record_drop();
                        if let Some(strike) = strike {
                            self.special_power_strikes
                                .track_artillery_projectile(strike, shell);
                        }
                    }
                }
            }
            if !self.spawn_identity_matches(id, generation, epoch) {
                continue;
            }
            let Some(data) = self
                .objects
                .get_mut(&id)
                .and_then(|object| object.artillery_barrage_transport.as_mut())
                .filter(|data| !data.is_hold_for_delay(self.frame))
            else {
                continue;
            };
            let (next, velocity, at_exit) = data.tick_transport(position);
            if !self.set_owned_object_position_with_effects(id, next, self.frame, effects)
                || !self.spawn_identity_matches(id, generation, epoch)
            {
                continue;
            }
            if let Some(object) = self.objects.get_mut(&id) {
                object.movement.velocity = velocity / SECONDS_PER_LOGICFRAME_REAL;
                if velocity.length_squared() > 1e-6 {
                    object.set_orientation_with_partition_owner(
                        velocity.z.atan2(velocity.x),
                        &mut self.partition_manager,
                    );
                }
            }
            if at_exit {
                self.mark_object_for_destruction_with_effects(id, None, effects);
            }
        }
        if self.receiving_module_content.is_none() {
            self.update_template_only_artillery_shells(effects)?;
        }
        self.special_power_strikes
            .finish_artillery_deliveries(&pending, |id| {
                self.combat_system.get_projectiles().contains_key(&id)
                    || self
                        .objects
                        .get(&id)
                        .is_some_and(|object| object.artillery_barrage_shell && object.is_alive())
            });
        Ok(())
    }

    fn release_artillery_payload(
        &mut self,
        carrier: ObjectId,
        aim: Vec3,
        payload: ArtilleryPayload,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<Option<ObjectId>> {
        let ArtilleryPayload::Visible(payload) = payload else {
            return Ok(self.release_template_only_artillery_shell(carrier, aim, effects));
        };
        let object = self
            .objects
            .get_mut(&carrier)
            .expect("validated artillery carrier");
        if !object.has_drawable {
            return Ok(None);
        }
        if !payload.subobject.is_empty() {
            let subobject = format!("{}01", payload.subobject);
            object
                .sub_object_visibility
                .apply_show_hide(&[], &[&subobject]);
        }
        let object = &self.objects[&carrier];
        let Some(weapon) = self
            .weapon_definitions
            .as_ref()
            .and_then(|definitions| definitions.get(&payload.weapon.to_ascii_lowercase()))
        else {
            return Ok(None);
        };

        let mut shot = self.receiving_temporary_weapon_shot(object, weapon, None, aim);
        let mut launch = object.get_transform_matrix();
        if !payload.bone.is_empty()
            && let Some(assets) = effects.pristine_bones
        {
            let bones = self
                .query_pristine_world_bone_poses(object, Some(assets), &payload.bone, 1, 1, 0, 0)
                .map_err(gamelogic::GameLogicError::Configuration)?;
            if let Some((position, _)) = bones.first() {
                launch.w_axis = position.extend(1.0);
            }
        }
        let context = shot
            .source_context
            .as_mut()
            .expect("captured carrier launch context");
        if payload.inherit_velocity {
            // Host velocity is per second. C++ backs up one logic frame before physics.
            launch.w_axis -= (context.source_velocity * SECONDS_PER_LOGICFRAME_REAL).extend(0.0);
        } else {
            context.source_velocity = Vec3::ZERO;
        }
        context.launch_transform = Some(launch);
        shot.shooter_pos = launch.w_axis.truncate();
        self.release_visible_missile(shot, &payload.object, effects)
    }

    // Direct template-only callers do not have the receiving retail weapon catalog.
    fn release_template_only_artillery_shell(
        &mut self,
        carrier: ObjectId,
        aim: Vec3,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Option<ObjectId> {
        use crate::game_logic::special_power_strikes::ARTILLERY_BARRAGE_SHELL_OBJECT;
        let team = self.objects.get(&carrier)?.team;
        if !self.templates.contains_key(ARTILLERY_BARRAGE_SHELL_OBJECT) {
            let mut template = ThingTemplate::new(ARTILLERY_BARRAGE_SHELL_OBJECT);
            template.set_health(50.0).add_kind_of(KindOf::Projectile);
            self.templates
                .insert(ARTILLERY_BARRAGE_SHELL_OBJECT.to_owned(), template);
        }
        let id = self.create_object_with_script_effects(
            ARTILLERY_BARRAGE_SHELL_OBJECT,
            team,
            aim.with_y(100.0),
            None,
            effects,
        )?;
        let object = self.objects.get_mut(&id)?;
        object.note_producer(carrier);
        object.artillery_barrage_shell = true;
        object.movement.velocity = Vec3::new(0.0, -18.0, 0.0);
        Some(id)
    }

    fn update_template_only_artillery_shells(
        &mut self,
        presentation_effects: &mut ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        use crate::game_logic::combat::DamageType;
        use crate::game_logic::special_power_strikes::{
            ARTILLERY_BARRAGE_DAMAGE, ARTILLERY_BARRAGE_RADIUS,
        };
        let shells: Vec<ObjectId> = self
            .objects
            .registered()
            .filter(|(_, o)| o.artillery_barrage_shell && o.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let mut destroy = Vec::new();
        for id in shells {
            let (pos, producer, team) = {
                let Some(o) = self.objects.get_mut(&id) else {
                    continue;
                };
                if !o.is_alive() || !o.artillery_barrage_shell {
                    continue;
                }
                let mut p = o.get_position();
                p.y += o.movement.velocity.y;
                let _ = o;
                if !self.set_owned_object_position_with_effects(
                    id,
                    p,
                    self.frame,
                    presentation_effects,
                ) {
                    continue;
                }
                let Some(o) = self.objects.get(&id) else {
                    continue;
                };
                (p, o.producer_id, o.team)
            };
            if pos.y <= 5.0 {
                if let Some(object) = self.objects.get_mut(&id) {
                    object.artillery_barrage_shell = false;
                }
                let (damage, hits, destroyed) = self.apply_fuel_air_radius_damage_with_effects(
                    id,
                    producer,
                    team,
                    Vec3::new(pos.x, 0.0, pos.z),
                    ARTILLERY_BARRAGE_DAMAGE,
                    ARTILLERY_BARRAGE_RADIUS,
                    DamageType::Explosive,
                    presentation_effects,
                )?;
                let _ = self.combat_particles.spawn(
                    CombatParticleKind::DeathExplosion,
                    pos,
                    self.frame,
                    Some(id),
                    None,
                );
                self.artillery_barrage_flight_reg.record_impact();
                if let Some(strike) = self
                    .special_power_strikes
                    .artillery_strike_for_projectile(id)
                {
                    self.special_power_strikes.record_impact_wave(
                        strike,
                        damage,
                        hits,
                        destroyed,
                        1,
                        false,
                        &[pos.with_y(0.0)],
                    );
                }
                destroy.push(id);
            }
        }
        for id in destroy {
            self.mark_object_for_destruction_with_effects(id, None, presentation_effects);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
