//! Receiving Main SlavedUpdate: one original ordered callback and saved owner.
use super::super::*;
use crate::game_logic::host_slave_drones::{
    BattleDroneRepairState as Repair, SlavedUpdateMetadata as Data, SlavedUpdateState as State,
};

impl GameLogic {
    pub(in crate::game_logic) fn enslave_owned_drone(
        &mut self,
        drone: ObjectId,
        master: ObjectId,
    ) -> bool {
        let Some(data) = self
            .objects
            .get(&drone)
            .and_then(|o| o.thing().template.slaved_update_metadata.clone())
        else {
            return false;
        };
        let Some(master_stealthed) = self.objects.get(&master).map(|o| o.status.stealthed) else {
            return false;
        };
        let angle = self.slave_random_real(0.0, std::f32::consts::TAU);
        let Some(object) = self.objects.get_mut(&drone) else {
            return false;
        };
        object.slaved_update = Some(State {
            master: Some(master),
            guard_offset: Vec3::new(
                data.guard_max_range * angle.cos(),
                0.0,
                data.guard_max_range * angle.sin(),
            ),
            ..Default::default()
        });
        object.status.unselectable = true;
        if master_stealthed && object.innate_stealth {
            object.apply_grant_stealth();
        }
        true
    }
    fn slave_random_real(&self, low: f32, high: f32) -> f32 {
        low + (high - low) * (self.logic_random.next_u32() as f32 / u32::MAX as f32)
    }
    fn slave_random_frames(&self, low: i32, high: i32) -> i32 {
        if high <= low {
            low
        } else {
            low + (self.logic_random.next_u32() % (high - low + 1) as u32) as i32
        }
    }
    fn slave_boundary_distance(source: &Object, goal: Vec3) -> f32 {
        let delta = source.get_position() - goal;
        let radius = (*source.geometry_info()).bounding_sphere_radius();
        ((delta.x * delta.x + delta.z * delta.z).sqrt() - radius)
            .max(0.0)
            .powi(2)
    }
    fn slave_move(&mut self, object: ObjectId, goal: Vec3) {
        let _ = self.move_object_with_pathfinding(object, goal, Some(AIState::Moving));
    }
    fn slave_end_repair(&mut self, id: ObjectId, state: &mut State) {
        if state.repair_state == Repair::None {
            return;
        }
        state.end_repair();
        if let Some(object) = self.objects.get_mut(&id) {
            Self::slave_set_model(object, gamelogic::common::ModelConditionFlags::PACKING);
            object.set_precise_z_and_ultra_accurate(false);
            crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_kind(
                object,
                crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Normal,
            );
        }
    }
    pub(in crate::game_logic) fn update_owned_slaved_object(&mut self, id: ObjectId) {
        self.update_owned_slaved_object_with_effects(
            id,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    pub(in crate::game_logic) fn update_owned_slaved_object_with_effects(
        &mut self,
        id: ObjectId,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let Some((data, mut state)) = self
            .objects
            .get(&id)
            .filter(|o| o.is_alive() && o.selected_locomotor_definition.is_some())
            .and_then(|o| {
                Some((
                    o.thing().template.slaved_update_metadata.clone()?,
                    o.slaved_update.clone()?,
                ))
            })
        else {
            return;
        };
        if state.frames_to_wait > 0 {
            state.frames_to_wait -= 1;
        }
        if state.repair_state == Repair::None {
            if state.frames_to_wait > 0 {
                self.store_slave_state(id, state);
                return;
            }
            state.frames_to_wait = 7;
        }
        let Some(master_id) = state.master else {
            self.store_slave_state(id, state);
            return;
        };
        let Some(master) = self
            .objects
            .get(&master_id)
            .filter(|o| !o.status.effectively_dead && !o.is_unmanned())
        else {
            state.master = None;
            state.guard_offset = Vec3::ZERO;
            if let Some(drone) = self.objects.get_mut(&id) {
                drone.status.unselectable = false;
                drone.set_status_disabled_held(false);
                drone.set_status_disabled_unmanned(true);
                drone.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                    &mut self.queued_audio_events,
                ));
                drone.set_ai_state(
                    AIState::Idle,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
            self.admit_host_radar_disabled_edges(id);
            self.store_slave_state(id, state);
            return;
        };
        let mpos = master.get_position();
        let master_identity = master.team_instance_id;
        let master_layer = master.pathfind_layer;
        let hp_percent = if data.repair_rate > 0.0 {
            (master.health.current / master.health.maximum * 100.0) as i32
        } else {
            100
        };
        let victim = master.target.and_then(|target| {
            self.objects
                .get(&target)
                .map(|o| (target, o.get_position()))
        });
        let destination = master.movement.path.last().copied();
        let drone_identity = self
            .objects
            .get(&id)
            .and_then(|drone| drone.team_instance_id);
        if let (Some(master_team), Some(drone_team)) = (master_identity, drone_identity) {
            if self.host_team_relationship(master_team, drone_team)
                != Some(gamelogic::common::Relationship::Allies)
            {
                self.defect_host_object_with_effects(id, master_team, 0, presentation_effects);
            }
        }
        if let Some(drone) = self.objects.get_mut(&id) {
            if data.stay_on_master_layer {
                drone.pathfind_layer = master_layer;
            }
        }
        if let Some(master) = self.objects.get_mut(&master_id) {
            master.set_weapon_bonus_drone_spotting(false);
        }
        // CPP188: urgent repair precedes master's victim and scouting.
        if hp_percent <= data.repair_priority_percent {
            self.slave_repair_with_effects(id, master_id, &data, &mut state, presentation_effects);
            self.store_slave_state(id, state);
            return;
        }
        if let Some((_target, position)) = victim.filter(|_| data.attack_range != 0.0) {
            self.slave_end_repair(id, &mut state);
            let distance = Self::slave_boundary_distance(self.objects.get(&id).unwrap(), position);
            let mut goal = self.slave_clamped_goal(mpos, position, distance, data.attack_range);
            self.slave_wander(&mut goal, &mut state, data.attack_wander_range);
            self.store_slave_state(id, state);
            self.slave_move(id, goal);
            if distance < data.range_bonus_distance.powi(2) {
                if let Some(master) = self.objects.get_mut(&master_id) {
                    master.set_weapon_bonus_drone_spotting(true);
                }
            }
            return;
        }
        if let Some(destination) = destination.filter(|destination| {
            data.scout_range != 0.0
                && Self::slave_boundary_distance(
                    self.objects.get(&master_id).unwrap(),
                    *destination,
                ) > (data.guard_max_range * 0.5).powi(2)
        }) {
            self.slave_end_repair(id, &mut state);
            let distance =
                Self::slave_boundary_distance(self.objects.get(&id).unwrap(), destination);
            let mut goal = self.slave_clamped_goal(mpos, destination, distance, data.scout_range);
            self.slave_wander(&mut goal, &mut state, data.scout_wander_range);
            self.store_slave_state(id, state);
            self.slave_move(id, goal);
            return;
        }
        if hp_percent < 100 {
            self.slave_repair_with_effects(id, master_id, &data, &mut state, presentation_effects);
            self.store_slave_state(id, state);
            return;
        }
        let mut pinned = mpos + Vec3::new(state.guard_offset.x, 0.0, state.guard_offset.z);
        state.guard_offset.y = self.terrain_height_at(pinned).unwrap_or(0.0);
        let object = self.objects.get(&id).unwrap();
        if data.guard_max_range != 0.0
            && ((object.ai_state() == AIState::Idle
                && object.get_position().distance_squared(pinned) > 225.0)
                || object.get_position().distance_squared(mpos)
                    > (2.0 * data.guard_max_range).powi(2))
        {
            self.slave_end_repair(id, &mut state);
            // Original guard uses guardMaxRange, gated by guardWanderRange.
            if data.guard_wander_range != 0.0 {
                self.slave_wander(&mut pinned, &mut state, data.guard_max_range);
            }
            self.store_slave_state(id, state);
            self.slave_move(id, pinned);
        } else {
            self.store_slave_state(id, state);
        }
    }
    fn store_slave_state(&mut self, id: ObjectId, state: State) {
        if let Some(object) = self.objects.get_mut(&id) {
            object.slaved_update = Some(state);
        }
    }
    fn slave_clamped_goal(&self, master: Vec3, target: Vec3, distance: f32, range: f32) -> Vec3 {
        if distance > range * range {
            master + (target - master).normalize_or_zero() * range
        } else {
            target
        }
    }
    fn slave_wander(&self, goal: &mut Vec3, state: &mut State, range: f32) {
        if range == 0.0 {
            return;
        }
        let angle = self.slave_random_real(0.0, std::f32::consts::TAU);
        state.guard_offset = Vec3::new(range * angle.cos(), 0.0, range * angle.sin());
        goal.x += state.guard_offset.x;
        goal.z += state.guard_offset.z;
        state.guard_offset.y = self.terrain_height_at(*goal).unwrap_or(0.0);
    }
    fn slave_repair(&mut self, id: ObjectId, master: ObjectId, data: &Data, state: &mut State) {
        self.slave_repair_with_effects(
            id,
            master,
            data,
            state,
            &mut crate::game_logic::ScriptPresentationEffects::default(),
        )
    }

    fn slave_repair_with_effects(
        &mut self,
        id: ObjectId,
        master: ObjectId,
        data: &Data,
        state: &mut State,
        presentation_effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) {
        let mpos = self.objects.get(&master).unwrap().get_position();
        // CPP Partition object/object distance subtracts both bounding spheres.
        let source = self.objects.get(&id).unwrap();
        let target = self.objects.get(&master).unwrap();
        let delta = source.get_position() - mpos;
        let radii = (*source.geometry_info()).bounding_sphere_radius()
            + (*target.geometry_info()).bounding_sphere_radius();
        let distance = ((delta.x * delta.x + delta.z * delta.z).sqrt() - radii)
            .max(0.0)
            .powi(2);
        let close = distance < 144.0;
        if close {
            match state.repair_state {
                Repair::None => self.slave_set_repair(id, master, data, state, Repair::Ready),
                Repair::Ready | Repair::Extending if state.frames_to_wait == 0 => {
                    self.slave_set_repair(id, master, data, state, Repair::Welding)
                }
                Repair::Unpacking | Repair::Welding | Repair::Retracting
                    if state.frames_to_wait == 0 =>
                {
                    self.slave_set_repair(id, master, data, state, Repair::Ready)
                }
                _ => {}
            }
        } else {
            state.repairing = false;
            let radius = self
                .objects
                .get(&master)
                .unwrap()
                .thing()
                .template
                .geometry_info
                .bounding_sphere_radius();
            if let Some(object) = self.objects.get_mut(&id) {
                object.set_precise_z_pos(distance < (radius * 2.0).powi(2));
            }
            let mut goal = mpos;
            goal.y += self.slave_random_real(data.repair_min_altitude, data.repair_max_altitude);
            self.store_slave_state(id, state.clone());
            self.slave_move(id, goal);
            if state.frames_to_wait == 0 {
                self.slave_set_repair(id, master, data, state, Repair::Ready);
            }
        }
        if close && state.repairing {
            self.store_slave_state(id, state.clone());
            let before = self.objects.get(&master).unwrap().health.current;
            // CPP SlavedUpdate493 enters Body::attemptHealing directly.
            let _ = self.apply_owned_healing_with_effects(
                master,
                data.repair_rate / 30.0,
                None,
                presentation_effects,
            );
            let gain = self
                .objects
                .get(&master)
                .map_or(0.0, |o| o.health.current - before);
            if gain > 0.0 {
                self.battle_drone_residual_repairs += 1;
                self.battle_drone_residual_repair_amount += gain;
            }
        }
    }
    fn slave_set_model(object: &mut Object, flag: gamelogic::common::ModelConditionFlags) {
        use gamelogic::common::ModelConditionFlags as M;
        let clear = M::PACKING
            | M::UNPACKING
            | M::FIRING_B
            | M::FIRING_C
            | M::BETWEEN_FIRING_SHOTS_B
            | M::BETWEEN_FIRING_SHOTS_C
            | M::RELOADING_B
            | M::RELOADING_C;
        object.model_condition_bits =
            (object.model_condition_bits & !(clear.bits() as u128)) | flag.bits() as u128;
    }
    fn slave_set_repair(
        &mut self,
        id: ObjectId,
        master: ObjectId,
        data: &Data,
        state: &mut State,
        desired: Repair,
    ) {
        if state.repair_state == desired {
            return;
        }
        let mut move_spot = false;
        let mut sparks = false;
        let flag = match desired {
            Repair::Ready => match state.repair_state {
                Repair::None => {
                    state.repair_state = Repair::Unpacking;
                    state.frames_to_wait = 15;
                    Some(gamelogic::common::ModelConditionFlags::UNPACKING)
                }
                Repair::Welding => {
                    state.repair_state = Repair::Retracting;
                    state.frames_to_wait = 5;
                    move_spot = true;
                    Some(gamelogic::common::ModelConditionFlags::FIRING_C)
                }
                _ => {
                    state.repair_state = Repair::Ready;
                    state.frames_to_wait =
                        self.slave_random_frames(data.min_ready_frames, data.max_ready_frames);
                    None
                }
            },
            Repair::Welding if state.repair_state == Repair::Ready => {
                state.repair_state = Repair::Extending;
                state.frames_to_wait = 5;
                Some(gamelogic::common::ModelConditionFlags::FIRING_B)
            }
            Repair::Welding => {
                state.repair_state = Repair::Welding;
                state.frames_to_wait =
                    self.slave_random_frames(data.min_weld_frames, data.max_weld_frames);
                state.repairing = true;
                sparks = true;
                None
            }
            _ => None,
        };
        if let Some(flag) = flag {
            if let Some(object) = self.objects.get_mut(&id) {
                Self::slave_set_model(object, flag);
            }
        }
        self.store_slave_state(id, state.clone());
        if move_spot && data.repair_range != 0.0 {
            let angle = self.slave_random_real(0.0, std::f32::consts::TAU);
            let mut goal = self.objects.get(&master).unwrap().get_position();
            goal.x += data.repair_range * angle.cos();
            goal.z += data.repair_range * angle.sin();
            goal.y = self.terrain_height_at(goal).unwrap_or(0.0)
                + self.slave_random_real(data.repair_min_altitude, data.repair_max_altitude);
            state.guard_offset = goal;
            if let Some(object) = self.objects.get_mut(&id) {
                crate::game_logic::host_upgrade_module_residuals::apply_locomotor_set_kind(
                    object,
                    crate::game_logic::host_upgrade_module_residuals::HostLocomotorSetKind::Panic,
                );
                object.set_precise_z_and_ultra_accurate(true);
            }
            self.store_slave_state(id, state.clone());
            self.slave_move(id, goal);
        }
        if sparks && !data.welding_system.is_empty() {
            let object = self.objects.get(&id).unwrap();
            let pose = object.get_position();
            let bone = gamelogic::object::draw::lookup_pristine_bone_translation(
                &object
                    .thing()
                    .template
                    .model_name_for_conditions(object.model_condition_bits),
                object.thing().template.asset_scale,
                &data.welding_bone,
            );
            let pose = pose + bone.map_or(Vec3::ZERO, |p| Vec3::new(p.x, p.z, p.y));
            self.combat_particles.spawn_named(
                crate::game_logic::combat_particles::CombatParticleKind::ParticleSysBone,
                &data.welding_system,
                pose,
                self.frame,
                Some(id),
                None,
            );
            let event =
                crate::game_logic::host_economy_log::resolve_misc_audio_event("RepairSparks");
            self.queue_audio_event(AudioEventRequest::new(&event).with_position(pose));
            // Emitted-particle lifetime override remains a separately tracked
            // native presentation provider, not a fake system expiration.
        }
    }
}
