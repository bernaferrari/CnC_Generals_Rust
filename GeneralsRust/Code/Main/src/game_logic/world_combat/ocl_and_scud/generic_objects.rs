//! Receiving GenericObjectCreationNugget::reallyCreate for ObjectNames.
use super::{GameLogic, OclPrimary, usable_ocl_particle_sys_name};
use crate::game_logic::{ObjectId, ScriptPresentationEffects, Team, VeterancyLevel};
use gamelogic::object_creation_list::{DebrisDisposition, GenericObjectCreationNugget};
use glam::{Mat4, Vec3};

/// The live source identity and the pose selected by the calling OCL overload.
pub(super) struct OclObjectSource {
    pub(super) id: ObjectId,
    pub(super) birth: (u64, u64),
    pub(super) owner: Option<u32>,
    pub(super) team: Team,
    pub(super) veterancy: VeterancyLevel,
    pub(super) orientation: f32,
    pub(super) velocity: Vec3,
    pub(super) origin: Vec3,
    pub(super) primary: OclPrimary,
    pub(super) transform: Option<Mat4>,
}

#[derive(Clone, Copy)]
enum OclObject {
    Child,
    Container(ObjectId, (u64, u64)),
}

fn supports_generic_object_creation(nugget: &GenericObjectCreationNugget) -> bool {
    let unsupported_disposition =
        DebrisDisposition::SEND_IT_OUT | DebrisDisposition::FLOATING | DebrisDisposition::WHIRLING;

    !nugget.names.is_empty()
        && !nugget.disposition.has(unsupported_disposition)
        && !(nugget.contain_inside_source_object && !nugget.put_in_container.is_empty())
        && !nugget.spread_formation
        && !nugget.dies_on_bad_land
}

impl GameLogic {
    pub(super) fn create_receiving_ocl_objects(
        &mut self,
        ocl_name: &str,
        generic: &GenericObjectCreationNugget,
        source: OclObjectSource,
        lifetime_frames: u32,
        interpreter: Option<&gamelogic::scripting::engine::ScriptEngine>,
        presentation_effects: &mut ScriptPresentationEffects<'_>,
        created: &mut Vec<(ObjectId, (u64, u64))>,
    ) -> gamelogic::GameLogicResult<()> {
        let OclObjectSource {
            id: source_id,
            birth: source_birth,
            owner: source_owner_player_id,
            team: source_team,
            veterancy: source_veterancy,
            orientation: source_orientation,
            velocity: source_velocity,
            origin,
            primary,
            transform: source_transform,
        } = source;
        if !supports_generic_object_creation(generic) {
            log::warn!(
                "receiving OCL `{ocl_name}` skipped unsupported GenericObjectCreationNugget configuration"
            );
            return Ok(());
        }

        let count = usize::try_from(generic.debris_to_generate).unwrap_or_default();
        let upper_pick = match i32::try_from(generic.names.len().saturating_sub(1)) {
            Ok(value) => value,
            Err(_) => return Ok(()),
        };
        let source_matches =
            |world: &Self| world.spawn_identity_matches(source_id, source_birth.0, source_birth.1);
        if !source_matches(self) {
            return Ok(());
        }

        // reallyCreate allocates the container at the factory pose before
        // choosing any child names. A missing template leaves standalone
        // children, as in the original.
        let container_name = generic.put_in_container.as_str();
        let container = if !container_name.is_empty()
            && let Some(template) = self.templates.get(container_name)
        {
            use crate::game_logic::ContainModuleKind;
            if !matches!(
                template.contain_module.kind,
                ContainModuleKind::None | ContainModuleKind::Parachute
            ) {
                log::warn!(
                    "receiving OCL `{ocl_name}` cannot create PutInContainer `{container_name}` with this contain module"
                );
                return Ok(());
            }
            match self.create_object_for_owner_or_team_with_birth_checked(
                container_name,
                source_team,
                source_owner_player_id,
                Vec3::ZERO,
                interpreter,
                presentation_effects,
            ) {
                Ok(Some(birth @ (id, (generation, epoch)))) => {
                    created.push(birth);
                    if source_matches(self) && self.spawn_identity_matches(id, generation, epoch) {
                        self.objects.get_mut(&id).unwrap().producer_id = Some(source_id);
                    }
                    Some(birth)
                }
                Ok(None) => return Ok(()),
                Err(error) => {
                    created.extend(error.committed);
                    return Err(error.error);
                }
            }
        } else {
            None
        };

        // C++ ObjectCreationList.cpp:953-960 uses each axis of an Object
        // overload's transform. Position overload offsets are global.
        let local_offset = Vec3::new(generic.offset.x, generic.offset.z, generic.offset.y);
        let offset = match primary {
            OclPrimary::Position => local_offset,
            OclPrimary::Object { .. } => source_transform.map_or(local_offset, |matrix| {
                matrix.transform_vector3(local_offset)
            }),
            OclPrimary::FrozenSource => {
                glam::Mat3::from_rotation_y(source_orientation) * local_offset
            }
        };
        let placement_origin = source_transform.map_or(origin, |matrix| matrix.w_axis.truncate());
        let on_ground = generic
            .disposition
            .has(DebrisDisposition::ON_GROUND_ALIGNED);
        let uses_flight = generic.disposition.has(
            DebrisDisposition::RANDOM_FORCE
                | DebrisDisposition::SEND_IT_FLYING
                | DebrisDisposition::SEND_IT_UP,
        );
        let like_existing = generic.disposition.has(DebrisDisposition::LIKE_EXISTING);
        let copy_basis = like_existing || (container.is_some() && uses_flight);

        // doStuffToObj applies to every child after admission, then to
        // the container itself, regardless of whether it accepted a rider.
        let objects = std::iter::repeat_n(OclObject::Child, count)
            .chain(container.map(|(id, birth)| OclObject::Container(id, birth)));
        for entry in objects {
            if !source_matches(self) {
                return Ok(());
            }
            let mut spawn_position = placement_origin + offset;
            if on_ground {
                spawn_position.y = self
                    .terrain_height_at(spawn_position)
                    .unwrap_or(spawn_position.y);
            }
            let (created_id, created_birth) = match entry {
                OclObject::Container(id, birth) => (id, birth),
                OclObject::Child => {
                    // C++ chooses once per child from ObjectNames using
                    // the game-logic random stream (ObjectCreationList.cpp:1326).
                    let raw_pick = self.logic_random.next_int(0, upper_pick);
                    let Ok(pick) = usize::try_from(raw_pick) else {
                        continue;
                    };
                    let Some(template_name) = generic.names.get(pick) else {
                        continue;
                    };
                    let template_name = template_name.trim();
                    if template_name.is_empty() || template_name.eq_ignore_ascii_case("None") {
                        continue;
                    }
                    if !self.templates.contains_key(template_name) {
                        log::warn!(
                            "receiving OCL `{ocl_name}` references unadmitted object template `{template_name}`"
                        );
                        continue;
                    }
                    let birth_position = if container.is_some() {
                        Vec3::ZERO
                    } else {
                        spawn_position
                    };
                    match self.create_object_for_owner_or_team_with_birth_checked(
                        template_name,
                        source_team,
                        source_owner_player_id,
                        birth_position,
                        interpreter,
                        presentation_effects,
                    ) {
                        Ok(Some(birth)) => {
                            // Keep the receipt if a callback retired the birth.
                            created.push(birth);
                            birth
                        }
                        Ok(None) => continue,
                        Err(error) => {
                            created.extend(error.committed);
                            return Err(error.error);
                        }
                    }
                }
            };
            let birth_matches = |world: &Self| {
                world.spawn_identity_matches(created_id, created_birth.0, created_birth.1)
            };
            if !birth_matches(self) {
                continue;
            }
            if !source_matches(self) {
                if generic.contain_inside_source_object {
                    self.destroy_object_with_effects(created_id, presentation_effects)?;
                }
                return Ok(());
            }
            if matches!(entry, OclObject::Child) {
                self.objects.get_mut(&created_id).unwrap().producer_id = Some(source_id);
                if let Some((chute, (generation, epoch))) = container
                    && self.spawn_identity_matches(chute, generation, epoch)
                    && self.parachute_can_contain(chute, created_id)
                {
                    self.attach_parachute_rider_owned(chute, created_id, presentation_effects)?;
                    if !birth_matches(self) {
                        continue;
                    }
                    if !source_matches(self) {
                        return Ok(());
                    }
                }
            }

            if let Some(mut transform) = source_transform.filter(|_| copy_basis) {
                transform.w_axis = spawn_position.extend(1.0);
                self.set_owned_object_transform_with_effects(
                    created_id,
                    transform,
                    self.frame,
                    presentation_effects,
                );
            } else {
                if like_existing {
                    self.objects
                        .get_mut(&created_id)
                        .unwrap()
                        .set_orientation_with_partition_owner(
                            source_orientation,
                            &mut self.partition_manager,
                        );
                }
                if container.is_some() && (copy_basis || on_ground) {
                    self.set_owned_object_position_with_effects(
                        created_id,
                        spawn_position,
                        self.frame,
                        presentation_effects,
                    );
                }
            }
            if !birth_matches(self) {
                continue;
            }
            if generic.ignore_primary_obstacle
                && source_matches(self)
                && let Some(object) = self.objects.get_mut(&created_id)
                && object.authors_behavior_module("PhysicsBehavior")
            {
                object.set_ignore_collisions_with(Some(source_id));
            }
            // C++ ObjectCreationList.cpp:985-993: any created object with a
            // SlavedUpdate is enslaved to the source (startSlavedEffects ->
            // UNSELECTABLE), independent of upgrade/OCL spelling.
            if self
                .objects
                .get(&created_id)
                .is_some_and(|object| object.thing().template.slaved_update_metadata.is_some())
            {
                self.enslave_owned_drone(created_id, source_id, source_birth);
            }
            let veterancy_health_bonus = self.veterancy_health_bonus;
            let inherit_name = self
                .with_object_veterancy_feedback_and_locomotors(
                    created_id,
                    |object, audio, feedback, locomotors| {
                        if generic.disposition.has(DebrisDisposition::INHERIT_VELOCITY) {
                            // The host's movement/physics state is the concrete
                            // equivalent of C++ PhysicsBehavior::applyForce for this
                            // disposition.  It also preserves DragonTank FireWall
                            // segment direction without inspecting an OCL name.
                            object.movement.velocity = source_velocity;
                        }
                        // C++ ObjectCreationList.cpp:996-1006 — inherit rank and
                        // transferObjectName only when the created tracker isTrainable.
                        // ExperienceTracker::setVeterancyLevel is exact: it replaces
                        // (and may lower) the receiving constructor/onCreate rank.
                        let inherit_name = generic.inherit_veterancy && object.is_trainable();
                        if inherit_name {
                            object.set_exact_veterancy_level_with_health_bonus_table(
                                source_veterancy,
                                &veterancy_health_bonus,
                                audio,
                                feedback,
                                locomotors,
                            );
                        }
                        inherit_name
                    },
                )
                .unwrap_or(false);
            if !birth_matches(self) {
                continue;
            }
            if !source_matches(self) {
                if generic.contain_inside_source_object && birth_matches(self) {
                    if let Err(error) =
                        self.destroy_object_with_effects(created_id, presentation_effects)
                    {
                        return Err(error);
                    }
                }
                return Ok(());
            }
            let Some(object) = self.objects.get_mut(&created_id) else {
                continue;
            };
            if lifetime_frames != 0 {
                // Caller lifetime overrides the nugget lifetime on an
                // existing LifetimeUpdate only; no timer is synthesized.
                let _ = object.set_native_lifetime_range(
                    self.frame,
                    crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                        minimum: lifetime_frames,
                        maximum: lifetime_frames,
                    },
                    &self.logic_random,
                );
            } else if generic.max_frames > 0 && object.native_lifecycle.is_some() {
                object.set_native_lifetime_range(
                    self.frame,
                    crate::game_logic::native_lifecycle_definition::LifetimeFrameRange {
                        minimum: generic.min_frames,
                        maximum: generic.max_frames,
                    },
                    &self.logic_random,
                );
            } else if generic.max_frames > 0 {
                let min_frames = i32::try_from(generic.min_frames).unwrap_or(i32::MAX);
                let max_frames = i32::try_from(generic.max_frames).unwrap_or(i32::MAX);
                let chosen_frames = self.logic_random.next_int(min_frames, max_frames);
                let delay_frames = u32::try_from(chosen_frames).unwrap_or(generic.min_frames);
                object.lifetime_update = Some(
                    crate::game_logic::host_lifetime_update::HostLifetimeUpdateData::from_delay_frames(
                        self.frame,
                        delay_frames,
                    ),
                );
            }

            // C++ calls BodyModuleInterface::setInitialHealth with a
            // game-logic random percent after the object is created.
            let health_percent = self
                .logic_random
                .next_real(generic.min_health, generic.max_health)
                .clamp(0.0, 1.0);
            let target_health = object.body_initial_health() * health_percent;
            object.internal_change_health(target_health - object.health.current);

            if generic.invulnerable_time > 0 {
                object.apply_eject_invulnerable_at(
                    self.frame.saturating_add(generic.invulnerable_time),
                    self.frame,
                );
            }
            if on_ground {
                // `ON_GROUND_ALIGNED` gives every spawned object a random
                // yaw after terrain placement (ObjectCreationList.cpp).
                object.set_orientation_with_partition_owner(
                    self.logic_random.next_real(0.0, std::f32::consts::TAU),
                    &mut self.partition_manager,
                );
            }
            // C++ ObjectCreationList.cpp:1086/1121-1124 ExtraFriction/Bounciness/BounceSound.
            if uses_flight {
                // C++ doStuffToObj guards the whole flight block on the
                // created object having a PhysicsBehavior: a physics-less
                // template still births but draws no force and keeps
                // default bounce/friction properties.
                if object.authors_behavior_module("PhysicsBehavior") {
                    object.set_extra_friction(generic.extra_friction);
                    object.set_extra_bounciness(generic.extra_bounciness);
                    object.set_allow_bouncing(true);
                    // The original assigns the authored bounce sound
                    // unconditionally here, even when the token is empty.
                    object.set_bounce_sound(generic.bounce_sound.clone());
                    let (force, [yaw, roll, pitch]) =
                        generic.draw_flight(|lo, hi| self.logic_random.next_real(lo, hi));
                    object.apply_physics_force(Vec3::new(force.x, force.z, force.y), self.frame);
                    object.shock_yaw_rate = yaw;
                    object.shock_roll_rate = roll;
                    object.shock_pitch_rate = pitch;
                    let orientation = if generic.orient_in_force_direction {
                        force.y.atan2(force.x)
                    } else {
                        source_orientation
                    };
                    object.set_orientation_with_partition_owner(
                        orientation,
                        &mut self.partition_manager,
                    );
                }
            } else {
                object.set_extra_friction(generic.extra_friction);
                if !generic.bounce_sound.is_empty() {
                    object.set_bounce_sound(generic.bounce_sound.clone());
                }
            }
            if matches!(entry, OclObject::Child) {
                if generic.fade_in {
                    object.start_drawable_fade_in(generic.fade_frames, self.frame);
                } else if generic.fade_out {
                    object.start_drawable_fade_out(generic.fade_frames, self.frame);
                }
            }
            drop(object);
            if inherit_name {
                if !source_matches(self) {
                    if generic.contain_inside_source_object && birth_matches(self) {
                        self.destroy_object_with_effects(created_id, presentation_effects)?;
                    }
                    return Ok(());
                }
                // C++ transfers the original object's script name to the ejected actor.
                let _ = self.transfer_script_object_name(source_id, created_id);
            }
            if !birth_matches(self) {
                continue;
            }
            // C++ ObjectCreationList.cpp:962-969 — ParticleSystem is extra
            // attached fire/smoke. The object still spawns.
            if let Some(particle) = usable_ocl_particle_sys_name(&generic.particle_sys_name) {
                let is_shrouded = self.host_fx_object_is_shrouded(created_id.0);
                let _ = self
                    .combat_particles
                    .attach_named_to_object_with_visibility(
                        created_id,
                        spawn_position,
                        self.frame,
                        particle,
                        is_shrouded,
                        presentation_effects,
                    );
            }
            // C++ GenericObjectCreationNugget::doStuffToObj performs
            // containment only after initialization, effects and particle
            // attachment. This bounded route is the retail Overlord / Helix
            // portable-structure case; other container kinds stay guarded.
            if generic.contain_inside_source_object {
                let admitted = birth_matches(self)
                    && self.admit_ocl_portable_structure(
                        source_id,
                        created_id,
                        source_birth,
                        created_birth,
                        presentation_effects,
                    );
                if !admitted && birth_matches(self) {
                    // C++ reallyCreate returns firstObject even when
                    // doStuffToObj schedules that birth for destruction;
                    // retain the receipt, but destroy only that birth.
                    if let Err(error) =
                        self.destroy_object_with_effects(created_id, presentation_effects)
                    {
                        return Err(error);
                    }
                }
            }
        }
        Ok(())
    }

    /// Admit the C++ `ContainInsideSourceObject` branch for the native
    /// OverlordContain and HelixContain portable-structure slot only.
    pub(super) fn admit_ocl_portable_structure(
        &mut self,
        source_id: ObjectId,
        child_id: ObjectId,
        source_birth: (u64, u64),
        child_birth: (u64, u64),
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> bool {
        if !self.spawn_identity_matches(source_id, source_birth.0, source_birth.1)
            || !self.spawn_identity_matches(child_id, child_birth.0, child_birth.1)
        {
            return false;
        }
        let Some((admits_portable, occupied)) = self.objects.get(&source_id).map(|source| {
            (
                source.admits_ocl_portable_structure(),
                source.overlord_portable_occupant,
            )
        }) else {
            return false;
        };
        let Some(child) = self.objects.get(&child_id) else {
            return false;
        };
        if !child.is_kind_of(crate::game_logic::KindOf::PortableStructure)
            || occupied.is_some()
            || !admits_portable
        {
            return false;
        }
        // HelixContain's portable override bypasses normal TransportContain
        // KindOf, relationship, controller, and slot-count checks; the
        // OverlordContain (Slots=1, PORTABLE_STRUCTURE) rider uses the same slot.
        if !self.attach_overlord_portable_with_effects(source_id, child_id, effects) {
            return false;
        }
        if self
            .objects
            .get(&source_id)
            .is_some_and(|source| source.drawable_is_effectively_hidden())
        {
            if let Some(child) = self.objects.get_mut(&child_id) {
                child.set_drawable_hidden(true);
            }
        }
        true
    }
}
