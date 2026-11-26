//! Artillery's single visible payload per DeliverPayload carrier.

use super::*;
use crate::game_logic::PhysicsTurningType;
use crate::game_logic::combat::DamageType;
use crate::game_logic::host_artillery_barrage_flight::{
    ArtilleryDeliveryEvent, ArtilleryFlightFrame, ArtilleryMapExtents, ArtilleryPayload,
    HostArtilleryBarrageFlightData, VisibleArtilleryPayload,
};
use crate::game_logic::host_spectre_gunship_deployment::find_farthest_edge_point_residual;
use crate::game_logic::host_usa_pilot::HostDeathType;
use crate::game_logic::special_power_strikes::{
    ARTILLERY_BARRAGE_CANNON_MAX_HEALTH, ARTILLERY_BARRAGE_DECAL_RADIUS,
    ARTILLERY_BARRAGE_DELIVERY_DISTANCE, ARTILLERY_BARRAGE_FORMATION_SPACING,
    ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES, ARTILLERY_BARRAGE_TRANSPORT,
    ArtilleryBarrageScienceTier,
};
use game_engine::common::game_common::SECONDS_PER_LOGICFRAME_REAL;
use game_engine::common::thing::deliver_payload_ai_data::DeliverPayloadAIUpdateModuleData;
use gamelogic::object_creation_list::DeliverPayloadNugget;

use crate::game_logic::host_enum_table_residual::{
    door_1_closing_model_bit, door_1_opening_model_bit,
};

fn finish_artillery_aircraft_move(object: &mut Object) {
    object.movement.clear_path();
    object.movement.target_position = None;
    object.set_locomotor_goal_none();
    object.set_status_moving(false);
}

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

        let original_axes = |p: Vec3| Vec3::new(p.x, p.z, p.y);
        let edge = {
            let terrain = self
                .world_services
                .terrain()
                .read()
                .unwrap_or_else(|error| error.into_inner());
            (authored && terrain.has_height_map()).then(|| {
                let point = terrain.find_farthest_edge_point(&glam::Vec3::new(
                    target.x, target.z, target.y,
                ));
                Vec3::new(point.x, point.z, point.y)
            })
        }
        .unwrap_or_else(|| {
            let point = find_farthest_edge_point_residual(
                target,
                self.world_min.x,
                self.world_min.z,
                self.world_max.x,
                self.world_max.z,
                0.0,
            );
            point.with_y(self.raw_terrain_ground_height_at(point))
        });
        let edge = edge
            + Vec3::Y * crate::game_logic::host_ocl_special_power::OCL_CREATE_ABOVE_LOCATION_HEIGHT;
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
            let wing_physics = definition.is_some()
                && object.loco_appearance == LocomotorAppearance::Wings
                && object.selected_locomotor_definition.is_some()
                && object.authors_behavior_module("PhysicsBehavior")
                && object.authors_behavior_module("DeliverPayloadAIUpdate");
            let orientation = if wing_physics {
                -pose.orient
            } else {
                pose.orient
            };
            object.set_orientation_with_partition_owner(orientation, &mut self.partition_manager);
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
            if let Some(nugget) = definition {
                let door_delay = object
                    .get_template()
                    .authored_module_data::<DeliverPayloadAIUpdateModuleData>(
                        "DeliverPayloadAIUpdate",
                    )
                    .map_or(0, |module| module.door_delay);
                data.start_authored_delivery(
                    door_delay,
                    nugget.data.drop_delay,
                    nugget.data.max_attempts,
                );
                data.speed_per_frame =
                    object.selected_locomotor_definition.map_or(0.0, |binding| {
                        binding.movement.max_speed * SECONDS_PER_LOGICFRAME_REAL
                    });
            }
            data.self_destruct_on_departure =
                definition.is_some_and(|n| n.data.self_destruct_object);
            if max_delay > 0 {
                let delay = self
                    .logic_random
                    .next_int(0, max_delay.min(i32::MAX as u32) as i32)
                    as u32;
                data.delay_until_frame = self.frame.wrapping_add(delay);
            }
            if wing_physics {
                data.enable_wing_physics();
                if definition.is_some_and(|n| n.start_at_max_speed) {
                    // ObjectCreationList.cpp:396–404 queues a motive impulse
                    // before locomotion. Physics uses distance per logic frame.
                    let direction = object.unit_direction_vector_2d();
                    let speed = object.effective_max_speed() * SECONDS_PER_LOGICFRAME_REAL;
                    let force = speed * object.physics_get_mass();
                    object.apply_motive_force(
                        Vec3::new(direction.x, 0.0, direction.y) * force,
                        self.frame,
                    );
                }
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
            let approach = data.target;
            self.objects.get_mut(&id)?.artillery_barrage_transport = Some(data);
            if wing_physics {
                self.issue_artillery_aircraft_move(id, approach);
            }
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

    pub fn update_artillery_barrage_flights(&mut self) -> generals_sim::GameLogicResult<()> {
        self.update_artillery_barrage_flights_with_effects(&mut Default::default())
    }

    pub(in crate::game_logic) fn update_artillery_barrage_flights_with_effects(
        &mut self,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        // Preserve the world's registration order when several carriers are ready.
        let carriers: Vec<_> = self
            .objects
            .registered()
            .filter(|(_, object)| object.artillery_barrage_transport.is_some() && object.is_alive())
            .map(|(id, _)| *id)
            .collect();
        let map = if carriers.is_empty() {
            None
        } else {
            let terrain = self
                .world_services
                .terrain()
                .read()
                .unwrap_or_else(|error| error.into_inner());
            terrain.has_height_map().then(|| ArtilleryMapExtents {
                playable: terrain.get_extent(),
                with_border: terrain.get_extent_including_border(),
            })
        };
        let mut pending = std::collections::BTreeSet::new();
        for id in carriers {
            let Some(object) = self.objects.get_mut(&id).filter(|object| object.is_alive()) else {
                continue;
            };
            let position = object.get_position();
            let generation = object.visual_object_generation;
            let epoch = self.visual_world_epoch;
            let orientation = object.get_orientation();
            let forward = object.get_transform_matrix().x_axis.truncate();
            let turning = object.physics_turning != PhysicsTurningType::TurnNone;
            let maximum_speed = object.effective_max_speed();
            let maximum_turn_rate = object.effective_turn_rate();
            // DeliverPayload runs before base MoveTo. Both an arrived path and
            // an external Idle count as leaving MoveTo; a pending search does not.
            let move_finished = !object.waiting_for_path && object.movement.path.is_empty();
            let Some(data) = object.artillery_barrage_transport.as_mut() else {
                continue;
            };
            let strike = data.strike_id;
            if data.is_hold_for_delay(self.frame) {
                if data.may_release_payload() {
                    pending.extend(strike);
                }
                continue;
            }
            let authored = data.is_authored_delivery();
            let wing_physics = data.uses_wing_physics();
            let map = map.filter(|_| authored);
            let event = data.advance_delivery(ArtilleryFlightFrame {
                frame: self.frame,
                position,
                heading: if wing_physics {
                    -orientation
                } else {
                    orientation
                },
                forward,
                turning,
                maximum_speed,
                maximum_turn_rate,
                move_finished,
                map,
            });
            let recovering = data.is_recovering_off_map();
            let self_destruct = data.self_destruct_on_departure;
            if event == ArtilleryDeliveryEvent::Release {
                let payload = data.payload.take().expect("pending payload on release");
                let aim = data.aim;
                if !authored {
                    object.kill_delivery_radius_decal();
                }
                if let Some(shell) = self.release_artillery_payload(id, aim, payload, effects)? {
                    self.artillery_barrage_flight_reg.record_drop();
                    if let Some(strike) = strike {
                        self.special_power_strikes
                            .track_artillery_projectile(strike, shell);
                    }
                }
            } else {
                if data.may_release_payload() {
                    pending.extend(strike);
                }
                let opening = 1u128 << door_1_opening_model_bit();
                let closing = 1u128 << door_1_closing_model_bit();
                match event {
                    ArtilleryDeliveryEvent::OpenDoor => {
                        object.model_condition_bits =
                            (object.model_condition_bits & !closing) | opening;
                    }
                    ArtilleryDeliveryEvent::Retry { close_door: true }
                    | ArtilleryDeliveryEvent::BeginRecovery { close_door: true }
                    | ArtilleryDeliveryEvent::Depart { close_door: true }
                        if authored =>
                    {
                        object.model_condition_bits =
                            (object.model_condition_bits & !opening) | closing;
                    }
                    _ => {}
                }
                match event {
                    ArtilleryDeliveryEvent::BeginRecovery { .. } => {
                        object.reset_dynamic_physics();
                        object.maintain_pos_valid = false;
                        if wing_physics {
                            object.set_allow_invalid_position(true);
                            object.requested_destination = None;
                            finish_artillery_aircraft_move(object);
                        }
                        self.partition_manager.unregister_object(id.0);
                        if object.has_drawable {
                            object.set_drawable_hidden(true);
                            self.admit_object_drawable_deselection(id);
                        }
                        self.remove_registered_path_goal(id);
                    }
                    ArtilleryDeliveryEvent::Reenter => {
                        self.reenter_artillery_carrier(id, effects)?;
                        if !wing_physics {
                            continue;
                        }
                    }
                    ArtilleryDeliveryEvent::Retry { .. } if wing_physics => {
                        object.set_allow_invalid_position(true);
                    }
                    ArtilleryDeliveryEvent::Depart { .. } => {
                        object.kill_delivery_radius_decal();
                        if self_destruct {
                            self.destroy_object_with_effects(id, effects)?;
                            continue;
                        }
                        if wing_physics {
                            object.set_allow_invalid_position(true);
                            object.set_ultra_accurate(true);
                        }
                    }
                    ArtilleryDeliveryEvent::FinishDeparture => {
                        self.destroy_object_with_effects(id, effects)?;
                        continue;
                    }
                    ArtilleryDeliveryEvent::KillStalledDeparture => {
                        self.apply_owned_kill_with_effects(
                            id,
                            DamageType::Unresistable,
                            HostDeathType::Normal,
                            effects,
                        )?;
                        continue;
                    }
                    _ => {}
                }
            }
            if !self.spawn_identity_matches(id, generation, epoch) {
                continue;
            }
            if recovering {
                if wing_physics {
                    self.steer_artillery_wings(id, None, effects)?;
                }
                continue;
            }
            let Some(data) = self
                .objects
                .get(&id)
                .and_then(|object| object.artillery_barrage_transport.as_ref())
            else {
                continue;
            };
            let position = self.objects[&id].get_position();
            let (goal, at_exit) = data.course(position);
            if at_exit {
                self.destroy_object_with_effects(id, effects)?;
                continue;
            }
            if wing_physics {
                let new_order = matches!(
                    event,
                    ArtilleryDeliveryEvent::Retry { .. }
                        | ArtilleryDeliveryEvent::ResumeApproach
                        | ArtilleryDeliveryEvent::Reenter
                        | ArtilleryDeliveryEvent::Depart { .. }
                );
                self.advance_artillery_aircraft_move(id, new_order.then_some(goal), effects)?;
                continue;
            }
            let next = data.planar_step(position, goal);
            let velocity = next - position;
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

    fn issue_artillery_aircraft_move(&mut self, id: ObjectId, destination: Vec3) {
        let destination = self.prepare_aircraft_move_destination(id, destination);
        let Some(path) = self.aircraft_quick_route(id, destination) else {
            return;
        };
        let object = self.objects.get_mut(&id).expect("artillery aircraft");
        object.requested_destination = Some(destination);
        object.waiting_for_path = false;
        object.queue_for_path_frame = 0;
        object.pending_quick_path_goal = self.map_loaded;
        object.movement.path = path;
        object.movement.path_cursor = object.movement.path.cursor_at_ordinal(1);
        object.movement.target_position = object.movement.current_path_position();
        object.set_locomotor_goal_position_on_path();
        object.set_status_moving(true);
        object.path_timestamp = self.frame;
    }

    fn advance_artillery_aircraft_move(
        &mut self,
        id: ObjectId,
        new_destination: Option<Vec3>,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        if let Some(destination) = new_destination {
            self.issue_artillery_aircraft_move(id, destination);
        }
        let object = &self.objects[&id];
        if object.pending_quick_path_goal && !object.waiting_for_path && !object.is_disabled() {
            self.on_waited_path_arrived(id);
        }
        let object = self.objects.get_mut(&id).expect("artillery aircraft");
        let position = object.get_position();
        let movement = &mut object.movement;
        let mut move_goal = movement
            .path
            .flight_course(position, &mut movement.path_cursor);
        if let Some((goal, remaining)) = move_goal {
            let last = *movement.path.last().expect("path has a flight course");
            let offset = last - position;
            let distance = if object.host_uses_close_enough_dist_3d() {
                offset.length()
            } else {
                remaining.min(offset.with_y(0.0).length())
            };
            // AIInternalMoveToState completes before doLocomotor. Its
            // caller will observe the finished order on the following tick.
            if distance < object.host_close_enough_dist() {
                finish_artillery_aircraft_move(object);
                move_goal = None;
            } else {
                object.movement.target_position = Some(goal);
            }
        }
        self.steer_artillery_wings(id, move_goal, effects)
    }

    /// Wings use moveTowardsPositionOther (Locomotor.cpp:1821–1859,
    /// 2326–2399). Keep the entry direction for the force after rotating.
    /// PhysicsBehavior advances the pose in its normal Euler phase.
    fn steer_artillery_wings(
        &mut self,
        id: ObjectId,
        move_goal: Option<(Vec3, f32)>,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let object = &self.objects[&id];
        if object.is_disabled() || object.is_shock_stunned() {
            return Ok(());
        }
        let position = object.get_position();
        let ground = if move_goal.is_some() {
            self.terrain_height_at(position)
                .unwrap_or(object.ground_height)
        } else {
            // Thing::isAboveTerrain compares the pose to raw terrain height.
            self.raw_terrain_ground_height_at(position)
        };
        let airborne = object.height_treats_as_airborne(position.y - ground);
        let can_apply_motive = object.allow_motive_force_while_airborne || !airborne;
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);
        let frame = self.frame;
        let dt = SECONDS_PER_LOGICFRAME_REAL;
        let object = self.objects.get_mut(&id).expect("artillery carrier");
        object.set_locomotor_physics_options();
        let maximum_speed = object.effective_max_speed();
        let (course, altitude_goal) = if let Some((goal, _)) = move_goal {
            object.maintain_pos_valid = false;
            object.is_braking = false;
            object.physics_turning = crate::game_logic::PhysicsTurningType::TurnNone;
            object.apply_motive_force(Vec3::ZERO, frame);
            (
                can_apply_motive.then_some((goal, maximum_speed)),
                Some(goal.y),
            )
        } else {
            // RecoverFromOffMap requests the current position. The base
            // AI completes that quick path and runs its usual Wings hold
            // even while the carrier is hidden and out of the partition.
            object.begin_loco_maintain_appearance(frame);
            (
                object.wings_maintain_course(frame, position.y > ground),
                object.maintain_pos.map(|position| position.y),
            )
        };
        if let Some((goal, speed)) = course {
            let offset = (goal - position).with_y(0.0);
            let distance = move_goal.map_or(0.0, |(_, remaining)| remaining.max(offset.length()));
            let heading = object.unit_direction_vector_2d();
            let actual = object.forward_speed_2d();
            let mut desired = speed.min(maximum_speed) * dt;
            let slide = desired * object.ultra_accurate_slide_factor;
            let sliding =
                object.ultra_accurate && offset.x.abs() <= slide && offset.z.abs() <= slide;
            let direction = if sliding {
                offset.normalize_or_zero()
            } else {
                Vec3::new(heading.x, 0.0, heading.y)
            };
            let braking = object.braking * dt * dt;
            let minimum_speed = object.min_speed * dt;
            if !object.no_slow_down_as_approaching_dest
                && distance
                    < crate::game_logic::object::calc_slow_down_dist(actual, minimum_speed, braking)
            {
                desired = minimum_speed;
            }
            let speed_delta = desired - actual;
            let acceleration = if speed_delta > 0.0 {
                object.effective_acceleration() * dt * dt
            } else {
                -braking
            };
            if !sliding
                && self
                    .rotate_owned_towards_position_with_effects(id, goal, dt, frame, effects)
                    .is_none()
            {
                return Ok(());
            }
            if !self.object_birth_matches(id, &birth) {
                return Ok(());
            }
            let object = self.objects.get_mut(&id).expect("rotated carrier");
            if speed_delta != 0.0 {
                let applied = if acceleration.abs() > speed_delta.abs() {
                    speed_delta
                } else {
                    acceleration
                };
                object.apply_motive_force(direction * (applied * object.physics_get_mass()), frame);
            }
        }
        self.apply_owned_locomotor_z_with_effects(id, altitude_goal, frame, effects)?;
        Ok(())
    }

    /// RecoverFromOffMapState attaches before revealing and setting the new pose.
    fn reenter_artillery_carrier(
        &mut self,
        id: ObjectId,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<()> {
        let object = &self.objects[&id];
        let position = object.get_position();
        let entry = {
            let terrain = self
                .world_services
                .terrain()
                .read()
                .unwrap_or_else(|error| error.into_inner());
            if !terrain.has_height_map() {
                return Err(generals_sim::GameLogicError::Configuration(
                    "artillery recovery requires the receiving terrain".into(),
                ));
            }
            let current = glam::Vec3::new(position.x, position.z, position.y);
            let mut point = terrain.find_closest_edge_point(&current);
            if current.z > terrain.get_ground_height(current.x, current.y, None) {
                point.z = current.z;
            }
            Vec3::new(point.x, point.z, point.y)
        };
        let flight = object
            .artillery_barrage_transport
            .as_ref()
            .expect("recovering carrier");
        let goal = flight.target;
        let heading = (goal.z - entry.z).atan2(goal.x - entry.x);
        let orientation = if flight.uses_wing_physics() {
            -heading
        } else {
            heading
        };
        let birth = std::sync::Arc::downgrade(&object.visual_draw_identity_token);

        let object = self.objects.get_mut(&id).expect("recovering carrier");
        self.partition_manager.attach_receiving_object(object);
        if object.has_drawable {
            object.set_drawable_hidden(false);
        }
        if self.set_owned_object_position_with_effects(id, entry, self.frame, effects)
            && self.object_birth_matches(id, &birth)
        {
            let object = self.objects.get_mut(&id).expect("live returning carrier");
            object.set_orientation_with_partition_owner(orientation, &mut self.partition_manager);
            object.reset_dynamic_physics();
        }
        Ok(())
    }

    fn release_artillery_payload(
        &mut self,
        carrier: ObjectId,
        aim: Vec3,
        payload: ArtilleryPayload,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> generals_sim::GameLogicResult<Option<ObjectId>> {
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
        let wing_physics = object
            .artillery_barrage_transport
            .as_ref()
            .is_some_and(HostArtilleryBarrageFlightData::uses_wing_physics);
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
                .map_err(generals_sim::GameLogicError::Configuration)?;
            if let Some((position, _)) = bones.first() {
                launch.w_axis = position.extend(1.0);
            }
        }
        let context = shot
            .source_context
            .as_mut()
            .expect("captured carrier launch context");
        let inherited_velocity_per_frame = payload.inherit_velocity.then(|| {
            // DeliverPayloadAIUpdate backs up by one frame of carrier movement
            // before passing its velocity to the shell as a motive impulse.
            let delta = if wing_physics {
                context.source_velocity
            } else {
                context.source_velocity * SECONDS_PER_LOGICFRAME_REAL
            };
            launch.w_axis -= delta.extend(0.0);
            delta
        });
        context.source_velocity = Vec3::ZERO;
        context.launch_transform = Some(launch);
        shot.shooter_pos = launch.w_axis.truncate();
        self.release_visible_missile(shot, &payload.object, inherited_velocity_per_frame, effects)
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
    ) -> generals_sim::GameLogicResult<()> {
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
